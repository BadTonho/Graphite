//! Language-server navigation features: references, implementations, peek,
//! rename, signature help, and call and type hierarchies.

use super::command_execution::CommandError;
use super::go_menu::{GoPicker, GoPickerKind, GoSymbolItem};
use super::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use winit::keyboard::{Key, NamedKey};

/// Lines of the target shown by peek definition.
const PEEK_LINES: usize = 12;

/// Pending rename typed into the prompt.
#[derive(Clone, Debug)]
pub(crate) struct RenamePrompt {
    pub(crate) path: PathBuf,
    pub(crate) position: lsp_types::Position,
    pub(crate) value: String,
}

impl App {
    /// Sends one LSP request for `path`, delivering its result as `LspUiEvent::Query`.
    fn lsp_request_at(
        &mut self,
        path: &Path,
        kind: &'static str,
        method: &'static str,
        params: Value,
    ) -> bool {
        if !self.lsp_manager.supports_path(path) {
            self.status = self.t("lsp.not_configured").into();
            return false;
        }
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.track_lsp_command(self.lsp_request_id, path);
        let sent = self
            .lsp_manager
            .request_query(path, self.lsp_request_id, kind, method, params);
        if !sent {
            self.fail_current_command(CommandError::Failed(self.t("lsp.unavailable").into()));
        }
        sent
    }

    /// Text-document and position parameters for the active editor caret.
    fn lsp_position_params(&self) -> Option<(PathBuf, Value)> {
        let (path, position) = self.active_lsp_position()?;
        let uri = self.lsp_manager.document_uri(&path)?;
        Some((path, json!({"textDocument": {"uri": uri}, "position": position})))
    }

    fn lsp_send_at_caret(
        &mut self,
        kind: &'static str,
        method: &'static str,
        extra: Option<(&'static str, Value)>,
    ) {
        let Some((path, mut params)) = self.lsp_position_params() else {
            self.status = self.t("lsp.save_for_features").into();
            return;
        };
        if let Some((key, value)) = extra {
            params[key] = value;
        }
        self.lsp_request_at(&path, kind, method, params);
    }

    pub(crate) fn lsp_references(&mut self) {
        self.lsp_send_at_caret(
            "references",
            "textDocument/references",
            Some(("context", json!({"includeDeclaration": true}))),
        );
    }

    pub(crate) fn lsp_implementations(&mut self) {
        self.lsp_send_at_caret("implementation", "textDocument/implementation", None);
    }

    pub(crate) fn lsp_signature_help(&mut self) {
        self.lsp_send_at_caret("signatureHelp", "textDocument/signatureHelp", None);
    }

    pub(crate) fn lsp_call_hierarchy(&mut self) {
        self.lsp_hierarchy.clear();
        self.lsp_send_at_caret(
            "prepareCallHierarchy",
            "textDocument/prepareCallHierarchy",
            None,
        );
    }

    pub(crate) fn lsp_type_hierarchy(&mut self) {
        self.lsp_hierarchy.clear();
        self.lsp_send_at_caret(
            "prepareTypeHierarchy",
            "textDocument/prepareTypeHierarchy",
            None,
        );
    }

    /// Peek reuses the definition request and shows the result in the hover popup.
    pub(crate) fn lsp_peek_definition(&mut self) {
        self.lsp_peek_request = Some(self.lsp_request_id.wrapping_add(1));
        self.request_lsp("definition");
    }

    /// Opens the rename prompt, pre-filled with the identifier under the caret.
    pub(crate) fn lsp_rename(&mut self) {
        let Some((path, position)) = self.active_lsp_position() else {
            self.status = self.t("lsp.save_for_features").into();
            return;
        };
        let line_text = self.editor().line(position.line as usize);
        let column =
            super::drawing_helpers::utf16_column_to_chars(&line_text, position.character);
        let value = word_at(&line_text, column);
        if value.is_empty() {
            self.status = self.t("lsp.rename.no_symbol").into();
            return;
        }
        self.rename_prompt = Some(RenamePrompt {
            path,
            position,
            value,
        });
    }

    /// Routes keys to the rename prompt while it is open.
    pub(crate) fn handle_rename_prompt_key(&mut self, key: &Key, text: Option<&str>) -> bool {
        let Some(prompt) = self.rename_prompt.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => self.rename_prompt = None,
            Key::Named(NamedKey::Enter) => {
                if let Some(prompt) = self.rename_prompt.take() {
                    self.send_rename(prompt);
                }
            }
            Key::Named(NamedKey::Backspace) => {
                prompt.value.pop();
            }
            _ => {
                if let Some(text) = text.filter(|text| !text.chars().any(char::is_control)) {
                    prompt.value.push_str(text);
                }
            }
        }
        true
    }

    fn send_rename(&mut self, prompt: RenamePrompt) {
        if prompt.value.trim().is_empty() {
            self.status = self.t("lsp.rename.empty").into();
            return;
        }
        let Some(uri) = self.lsp_manager.document_uri(&prompt.path) else {
            self.status = self.t("lsp.save_for_features").into();
            return;
        };
        let params = json!({
            "textDocument": {"uri": uri},
            "position": prompt.position,
            "newName": prompt.value.clone(),
        });
        self.lsp_request_at(&prompt.path, "rename", "textDocument/rename", params);
    }

    /// Handles a generic LSP result delivered as `LspUiEvent::Query`.
    pub(crate) fn handle_lsp_query(&mut self, kind: &str, result: Value) {
        match kind {
            "references" => self.show_lsp_locations(&result, "lsp.references.none"),
            "implementation" => self.show_lsp_locations(&result, "lsp.implementation.none"),
            "signatureHelp" => match signature_label(&result) {
                Some(label) => self.hover_text = Some(label),
                None => self.status = self.t("lsp.signature.none").into(),
            },
            "rename" => self.apply_rename_result(result),
            "formatting" | "rangeFormatting" => self.apply_format_result(result),
            "prepareCallHierarchy" => self.follow_hierarchy(
                &result,
                [
                    ("callHierarchyIncoming", "callHierarchy/incomingCalls"),
                    ("callHierarchyOutgoing", "callHierarchy/outgoingCalls"),
                ],
            ),
            "prepareTypeHierarchy" => self.follow_hierarchy(
                &result,
                [
                    ("typeHierarchySupertypes", "typeHierarchy/supertypes"),
                    ("typeHierarchySubtypes", "typeHierarchy/subtypes"),
                ],
            ),
            "callHierarchyIncoming" => {
                self.add_hierarchy_items(calls_from(&result, "from", "← "))
            }
            "callHierarchyOutgoing" => self.add_hierarchy_items(calls_from(&result, "to", "→ ")),
            "typeHierarchySupertypes" => self.add_hierarchy_items(items_from(&result, "↑ ")),
            "typeHierarchySubtypes" => self.add_hierarchy_items(items_from(&result, "↓ ")),
            _ => {}
        }
    }

    /// Formats the active document with the language server, if one is configured.
    pub(crate) fn lsp_format_document(&mut self) -> bool {
        let Some((path, _)) = self.active_lsp_position() else {
            self.status = self.t("lsp.save_for_features").into();
            return false;
        };
        let Some(uri) = self.lsp_manager.document_uri(&path) else {
            self.status = self.t("lsp.save_for_features").into();
            return false;
        };
        let params = json!({
            "textDocument": {"uri": uri},
            "options": crate::lsp::LspManager::formatting_options(self.tab_size, self.insert_spaces),
        });
        self.lsp_format_revision = Some(self.documents[self.command_document()].buffer.revision());
        self.lsp_request_at(&path, "formatting", "textDocument/formatting", params)
    }

    /// Formats the selected lines, or the caret's line when nothing is selected.
    pub(crate) fn lsp_format_selection(&mut self) -> bool {
        let range = self.lsp_selection_range();
        self.request_range_formatting(range)
    }

    /// LSP range of the active selection, or of the caret's whole line when nothing is selected.
    pub(crate) fn lsp_selection_range(&self) -> lsp_types::Range {
        let buffer = &self.documents[self.command_document()].buffer;
        let (start, end) = match self.editor().selection().filter(|s| s.anchor != s.head) {
            Some(selection) => (
                selection.anchor.min(selection.head),
                selection.anchor.max(selection.head),
            ),
            None => {
                let (line, _) = self.editor().cursor_line_col();
                let start = buffer.line_start_char(line);
                let end = if line + 1 < buffer.len_lines() {
                    buffer.line_start_char(line + 1)
                } else {
                    buffer.len_chars()
                };
                (start, end)
            }
        };
        self.lsp_char_range(start, end)
    }

    /// Converts character offsets of the active document into an LSP range.
    fn lsp_char_range(&self, start: usize, end: usize) -> lsp_types::Range {
        let text = self.documents[self.command_document()].buffer.text();
        lsp_types::Range {
            start: crate::lsp::position_at_char(&text, start),
            end: crate::lsp::position_at_char(&text, end),
        }
    }

    /// Sends a range formatting request for character offsets `start..end` of the active document.
    /// Edits are applied only if the document is unchanged when the server answers.
    pub(crate) fn lsp_format_chars(&mut self, start: usize, end: usize) -> bool {
        let range = self.lsp_char_range(start, end);
        self.request_range_formatting(range)
    }

    fn request_range_formatting(&mut self, range: lsp_types::Range) -> bool {
        let Some((path, _)) = self.active_lsp_position() else {
            self.status = self.t("lsp.save_for_features").into();
            return false;
        };
        let Some(uri) = self.lsp_manager.document_uri(&path) else {
            self.status = self.t("lsp.save_for_features").into();
            return false;
        };
        let params = json!({
            "textDocument": {"uri": uri},
            "range": range,
            "options": crate::lsp::LspManager::formatting_options(self.tab_size, self.insert_spaces),
        });
        self.lsp_format_revision = Some(self.documents[self.command_document()].buffer.revision());
        self.lsp_request_at(&path, "rangeFormatting", "textDocument/rangeFormatting", params)
    }

    fn apply_format_result(&mut self, result: Value) {
        let requested = self.lsp_format_revision.take();
        if requested.is_some_and(|revision| {
            revision != self.documents[self.command_document()].buffer.revision()
        }) {
            self.status = self.t("lsp.format.stale").into();
            return;
        }
        let Some(path) = self.documents[self.command_document()]
            .buffer
            .path()
            .map(PathBuf::from)
        else {
            self.status = self.t("lsp.save_for_features").into();
            return;
        };
        let Some(uri) = self.lsp_manager.document_uri(&path) else {
            self.status = self.t("lsp.save_for_features").into();
            return;
        };
        let Some(edit) = formatting_edit(&uri, &result) else {
            self.status = self.t("lsp.format.unchanged").into();
            return;
        };
        match serde_json::from_value::<lsp_types::WorkspaceEdit>(edit)
            .map_err(|error| error.to_string())
            .and_then(|edit| self.apply_workspace_edit(edit))
        {
            Ok(()) => self.status = self.t("lsp.format.done").into(),
            Err(error) => {
                self.status = self.tf("lsp.format.apply_failed", &[("error", &error)]);
            }
        }
    }

    fn show_lsp_locations(&mut self, result: &Value, none_key: &str) {
        let mut items = locations_from(result);
        match items.len() {
            0 => self.status = self.t(none_key).into(),
            1 => {
                if let Some(item) = items.pop() {
                    self.jump_to_lsp_location(item);
                }
            }
            _ => self.open_locations_picker(items),
        }
    }

    fn jump_to_lsp_location(&mut self, item: GoSymbolItem) {
        self.open_file(item.path);
        self.editor_mut().set_cursor_line_col(item.line, 0);
        self.push_navigation_location();
        self.reveal_cursor = true;
    }

    pub(crate) fn open_locations_picker(&mut self, items: Vec<GoSymbolItem>) {
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        self.go_picker = Some(GoPicker {
            kind: GoPickerKind::Locations,
            query: String::new(),
            items,
            selected: 0,
            loading: false,
            generation: self.go_picker_generation,
            message: None,
        });
    }

    fn apply_rename_result(&mut self, result: Value) {
        if result.is_null() {
            self.status = self.t("lsp.rename.none").into();
            return;
        }
        match serde_json::from_value::<lsp_types::WorkspaceEdit>(result) {
            Ok(edit) => match self.apply_workspace_edit(edit) {
                Ok(()) => self.status = self.t("lsp.rename.done").into(),
                Err(error) => self.status = error,
            },
            Err(_) => self.status = self.t("lsp.rename.none").into(),
        }
    }

    /// After a hierarchy root is prepared, asks for its related items in both directions.
    fn follow_hierarchy(&mut self, result: &Value, requests: [(&'static str, &'static str); 2]) {
        let Some(item) = result
            .as_array()
            .and_then(|items| items.first())
            .cloned()
        else {
            self.status = self.t("lsp.hierarchy.none").into();
            return;
        };
        let Some((path, _)) = self.active_lsp_position() else {
            return;
        };
        let params = json!({ "item": item });
        for (kind, method) in requests {
            self.lsp_request_at(&path, kind, method, params.clone());
        }
    }

    fn add_hierarchy_items(&mut self, items: Vec<GoSymbolItem>) {
        if items.is_empty() && self.lsp_hierarchy.is_empty() {
            self.status = self.t("lsp.hierarchy.none").into();
            return;
        }
        self.lsp_hierarchy.extend(items);
        let shown = self.lsp_hierarchy.clone();
        self.open_locations_picker(shown);
    }

    /// Peek shows the lines starting at the definition in the hover popup.
    pub(crate) fn show_lsp_peek(&mut self, location: Option<lsp_types::Location>) {
        let Some(location) = location else {
            self.status = self.t("lsp.peek.none").into();
            return;
        };
        let Some(path) = crate::lsp::uri_to_path(location.uri.as_str()) else {
            self.status = self.t("lsp.peek.none").into();
            return;
        };
        let line = location.range.start.line as usize;
        let Ok(source) = std::fs::read_to_string(&path) else {
            self.status = self.t("lsp.peek.none").into();
            return;
        };
        let snippet: Vec<&str> = source.lines().skip(line).take(PEEK_LINES).collect();
        self.hover_text = Some(format!(
            "{}:{}\n{}",
            path.display(),
            line + 1,
            snippet.join("\n")
        ));
    }
}

/// Identifier characters for the word under the caret.
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// The identifier that contains `column` (in characters) of `line`.
fn word_at(line: &str, column: usize) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut start = column.min(chars.len());
    let mut end = start;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }
    chars[start..end].iter().collect()
}

fn location_item(uri: &str, line: usize) -> Option<GoSymbolItem> {
    let path = crate::lsp::uri_to_path(uri)?;
    let file = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(GoSymbolItem {
        path,
        line,
        name: format!("{file}:{}", line + 1),
        scope: String::new(),
    })
}

fn range_line(range: &Value) -> Option<usize> {
    range
        .get("start")?
        .get("line")?
        .as_u64()
        .map(|line| line as usize)
}

/// Locations or location links returned by references and implementations.
fn locations_from(result: &Value) -> Vec<GoSymbolItem> {
    let entries: Vec<&Value> = match result {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![result],
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|entry| {
            let uri = entry
                .get("uri")
                .or_else(|| entry.get("targetUri"))?
                .as_str()?;
            let range = entry
                .get("range")
                .or_else(|| entry.get("targetSelectionRange"))?;
            location_item(uri, range_line(range)?)
        })
        .collect()
}

fn hierarchy_item(item: &Value, label: &str) -> Option<GoSymbolItem> {
    let name = item.get("name")?.as_str()?;
    let uri = item.get("uri")?.as_str()?;
    let line = range_line(item.get("selectionRange")?)?;
    let mut found = location_item(uri, line)?;
    found.name = format!("{label}{name}");
    Some(found)
}

/// Callers or callees, wrapped as `{ from: item }` or `{ to: item }`.
fn calls_from(result: &Value, key: &str, label: &str) -> Vec<GoSymbolItem> {
    result
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| call.get(key).and_then(|item| hierarchy_item(item, label)))
                .collect()
        })
        .unwrap_or_default()
}

/// Supertypes or subtypes, returned as bare items.
fn items_from(result: &Value, label: &str) -> Vec<GoSymbolItem> {
    result
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| hierarchy_item(item, label))
                .collect()
        })
        .unwrap_or_default()
}

/// Wraps the `TextEdit[]` of a formatting result as a workspace edit for `uri`.
/// Returns `None` when the server made no changes or sent a non-array result.
fn formatting_edit(uri: &str, result: &Value) -> Option<Value> {
    let edits = result.as_array().filter(|edits| !edits.is_empty())?;
    let mut changes = serde_json::Map::new();
    changes.insert(uri.to_owned(), Value::Array(edits.clone()));
    Some(json!({ "changes": changes }))
}

/// The label of the active signature from a `SignatureHelp` result.
fn signature_label(result: &Value) -> Option<String> {
    let signatures = result.get("signatures")?.as_array()?;
    let active = result
        .get("activeSignature")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let signature = signatures.get(active).or_else(|| signatures.first())?;
    Some(signature.get("label")?.as_str()?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_at_expands_identifier_around_the_caret() {
        assert_eq!(word_at("let value = 1", 6), "value");
        assert_eq!(word_at("let value = 1", 4), "value");
        assert_eq!(word_at("a + b", 2), "");
    }

    #[test]
    fn locations_accept_location_and_location_link_shapes() {
        let locations = locations_from(&json!([{
            "uri": "file:///C:/tmp/a.rs",
            "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 3}}
        }]));
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].line, 2);
        assert_eq!(locations[0].name, "a.rs:3");

        let links = locations_from(&json!([{
            "targetUri": "file:///C:/tmp/b.rs",
            "targetSelectionRange": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}
        }]));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].name, "b.rs:1");
    }

    #[test]
    fn signature_label_uses_the_active_signature() {
        let result = json!({
            "signatures": [{"label": "f(a, b)"}, {"label": "g()"}],
            "activeSignature": 1
        });
        assert_eq!(signature_label(&result), Some("g()".to_owned()));
        assert_eq!(signature_label(&json!({"signatures": []})), None);
    }

    #[test]
    fn formatting_result_becomes_a_workspace_edit_only_when_it_changes_text() {
        let edits = json!([{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2}},
            "newText": "  "
        }]);
        let edit = formatting_edit("file:///C:/tmp/f.rs", &edits).expect("changes");
        let parsed: lsp_types::WorkspaceEdit = serde_json::from_value(edit).expect("valid edit");
        let changes = parsed.changes.expect("changes map");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes.values().next().unwrap().len(), 1);

        assert!(formatting_edit("file:///C:/tmp/f.rs", &json!([])).is_none());
        assert!(formatting_edit("file:///C:/tmp/f.rs", &Value::Null).is_none());
        assert!(formatting_edit("file:///C:/tmp/f.rs", &json!({"a": 1})).is_none());
    }

    #[test]
    fn hierarchy_items_carry_their_direction_label() {
        let item = hierarchy_item(
            &json!({
                "name": "caller",
                "uri": "file:///C:/tmp/c.rs",
                "selectionRange": {"start": {"line": 4, "character": 1}, "end": {"line": 4, "character": 3}}
            }),
            "← ",
        )
        .expect("valid hierarchy item");
        assert_eq!(item.name, "← caller");
        assert_eq!(item.line, 4);
    }
}
