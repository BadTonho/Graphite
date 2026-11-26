use super::*;
use graphite_text::Rope;

#[derive(Default)]
pub(super) struct SaveCapabilities {
    pub will_save: bool,
    pub wait_until: bool,
    pub save: bool,
    pub include_text: bool,
}
impl SaveCapabilities {
    pub fn from_sync(sync: Option<&Value>) -> Self {
        Self {
            will_save: sync
                .and_then(|v| v.get("willSave"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            wait_until: sync
                .and_then(|v| v.get("willSaveWaitUntil"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            save: sync
                .and_then(|v| v.get("save"))
                .is_some_and(|v| v == &Value::Bool(true) || v.is_object()),
            include_text: sync
                .and_then(|v| v.pointer("/save/includeText"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
    pub fn did_save(&self, uri: &str, text: String) -> Option<Message> {
        if !self.save {
            return None;
        }
        let mut params = json!({"textDocument":{"uri":uri}});
        if self.include_text {
            params["text"] = json!(text);
        }
        Some(Message::Notification(Notification {
            method: "textDocument/didSave".into(),
            params,
        }))
    }
}
pub(super) fn save_edits(
    value: Value,
) -> graphite_core_api::ServiceResult<Vec<graphite_core_api::TextEdit>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let edits: Vec<lsp_types::TextEdit> = serde_json::from_value(value)
        .map_err(|e| graphite_core_api::ServiceError::Protocol(e.to_string()))?;
    Ok(edits
        .into_iter()
        .map(|e| graphite_core_api::TextEdit {
            start: graphite_core_api::Position {
                line: e.range.start.line,
                character: e.range.start.character,
            },
            end: graphite_core_api::Position {
                line: e.range.end.line,
                character: e.range.end.character,
            },
            text: e.new_text,
        })
        .collect())
}

pub(super) fn full_text_change(text: String) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text,
    }
}

pub(super) fn text_sync_kind(capability: Option<&Value>) -> u64 {
    match capability {
        Some(value) if value.is_number() => value.as_u64().unwrap_or(0),
        Some(value) => value.get("change").and_then(Value::as_u64).unwrap_or(0),
        None => 0,
    }
}

/// The `didChange` notification for a server's sync kind. An incremental server gets `edits`,
/// the exact changes since its previous version, when they are known; otherwise the one range
/// between `previous` and `text` that differs.
pub(super) fn change_params(
    uri: &str,
    previous: &Rope,
    text: &Rope,
    version: i32,
    sync_kind: u64,
    edits: Option<Vec<TextDocumentContentChangeEvent>>,
) -> Option<DidChangeTextDocumentParams> {
    let content_changes = match sync_kind {
        1 => vec![full_text_change(text.to_string())],
        2 => match edits {
            Some(edits) if !edits.is_empty() => edits,
            _ => vec![incremental_text_change(previous, text)],
        },
        _ => return None,
    };
    Some(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.parse().ok()?,
            version,
        },
        content_changes,
    })
}

/// A change recorded by the buffer, as the protocol describes it.
pub(crate) fn recorded_change(
    edit: &graphite_text::RecordedEdit,
) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: Some(Range::new(
            Position::new(edit.change.start_row as u32, edit.start_utf16_column as u32),
            Position::new(
                edit.change.old_end_row as u32,
                edit.old_end_utf16_column as u32,
            ),
        )),
        range_length: None,
        text: edit.inserted.to_string(),
    }
}

/// The single range that differs between `old` and `new`: the text between their common start
/// and their common end. Used when the exact changes are not known.
pub(super) fn incremental_text_change(old: &Rope, new: &Rope) -> TextDocumentContentChangeEvent {
    let prefix = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let limit = old.len_chars().min(new.len_chars()) - prefix;
    let suffix = old
        .chars_at(old.len_chars())
        .reversed()
        .zip(new.chars_at(new.len_chars()).reversed())
        .take(limit)
        .take_while(|(a, b)| a == b)
        .count();
    let old_end = old.len_chars() - suffix;
    let new_end = new.len_chars() - suffix;
    let range_length = (old.char_to_utf16_cu(old_end) - old.char_to_utf16_cu(prefix)) as u32;
    TextDocumentContentChangeEvent {
        range: Some(Range::new(
            rope_position(old, prefix),
            rope_position(old, old_end),
        )),
        range_length: Some(range_length),
        text: new.slice(prefix..new_end).to_string(),
    }
}

/// The protocol position of a char index: its line and its column in UTF-16 code units.
pub(super) fn rope_position(rope: &Rope, char_index: usize) -> Position {
    let line = rope.char_to_line(char_index);
    let column = rope.char_to_utf16_cu(char_index) - rope.char_to_utf16_cu(rope.line_to_char(line));
    Position::new(line as u32, column as u32)
}

pub fn position_at_char(text: &str, char_index: usize) -> Position {
    let mut line = 0u32;
    let mut column = 0u32;
    for ch in text.chars().take(char_index) {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += ch.len_utf16() as u32;
        }
    }
    Position::new(line, column)
}

pub fn path_to_uri(path: &Path) -> Option<String> {
    graphite_core_api::DocumentUri::from_path(path)
        .ok()
        .map(|uri| uri.as_str().to_owned())
}
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    graphite_core_api::DocumentUri::parse(uri)
        .ok()?
        .to_path()
        .ok()
}

pub fn diagnostics_are_current(
    current_revision: Option<u64>,
    diagnostic_version: Option<i32>,
) -> bool {
    diagnostic_version.is_none_or(|version| current_revision == Some(version.max(0) as u64))
}

pub(super) fn hover_to_string(hover: Hover) -> String {
    match hover.contents {
        lsp_types::HoverContents::Scalar(markup) => match markup {
            lsp_types::MarkedString::String(s) => s,
            lsp_types::MarkedString::LanguageString(s) => s.value,
        },
        lsp_types::HoverContents::Array(items) => items
            .into_iter()
            .map(|item| match item {
                lsp_types::MarkedString::String(s) => s,
                lsp_types::MarkedString::LanguageString(s) => s.value,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value,
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;
    #[test]
    fn save_notifications_respect_negotiated_capabilities_and_text_inclusion() {
        assert!(SaveCapabilities::from_sync(Some(&json!(2)))
            .did_save("file:///a", "x".into())
            .is_none());
        let caps = SaveCapabilities::from_sync(Some(
            &json!({"willSave":true,"willSaveWaitUntil":true,"save":true}),
        ));
        assert!(caps.will_save && caps.wait_until);
        let Message::Notification(message) = caps.did_save("file:///a", "x".into()).unwrap() else {
            panic!()
        };
        assert!(message.params.get("text").is_none());
        let caps = SaveCapabilities::from_sync(Some(&json!({"save":{"includeText":true}})));
        let Message::Notification(message) = caps.did_save("file:///a", "written".into()).unwrap()
        else {
            panic!()
        };
        assert_eq!(message.params["text"], "written");
        assert!(save_edits(json!([{"newText":"invalid"}])).is_err());
        assert!(save_edits(Value::Null).unwrap().is_empty());
    }
}
