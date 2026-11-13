use super::*;
use graphite_core_api::{DocumentSnapshot, DocumentUri};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Feature {
    Completion,
    Hover,
    Format,
    /// Format only the selection (or the cursor line).
    FormatRange,
    Definition,
    Declaration,
    Implementation,
    TypeDefinition,
    References,
    DocumentSymbols,
    CodeActions,
    Rename,
    SignatureHelp,
    /// Inline hints (parameter names, types). Requested automatically, not by a command.
    InlayHints,
    /// Occurrences of the symbol at the cursor. Requested automatically.
    DocumentHighlights,
    /// Code folding regions. Requested automatically.
    FoldingRanges,
    /// Expand Selection: the enclosing ranges of the cursor.
    SelectionRanges,
    /// Formatting after a trigger character was typed (`editor.formatOnType`).
    FormatOnType,
}

struct Provider {
    feature: Feature,
    selector: Vec<Value>,
    /// Characters that trigger on-type formatting (empty for other features).
    triggers: Vec<String>,
}

struct Request {
    id: u64,
    handle: u64,
    feature: Feature,
    document: DocumentSnapshot,
    cursor: usize,
    deadline: Instant,
    /// Text the request was made with (the new name of a rename).
    argument: Option<String>,
}

#[derive(Default)]
pub(super) struct Languages {
    providers: BTreeMap<u64, Provider>,
    request: Option<Request>,
    next_id: u64,
    diagnostics: BTreeMap<String, BTreeMap<String, Vec<lsp_types::Diagnostic>>>,
}

impl Languages {
    pub(super) fn pending(&self) -> bool {
        self.request.is_some()
    }

    /// Whether any extension registered one of `features`, for any document.
    pub(super) fn has_provider(&self, features: &[Feature]) -> bool {
        self.providers
            .values()
            .any(|provider| features.contains(&provider.feature))
    }

    /// The handle of a provider of `feature` whose selector matches `document`.
    pub(super) fn provider_for(
        &self,
        feature: Feature,
        document: &DocumentSnapshot,
    ) -> Option<u64> {
        self.providers.iter().find_map(|(handle, provider)| {
            (provider.feature == feature
                && provider
                    .selector
                    .iter()
                    .any(|selector| matches_document(selector, document)))
            .then_some(*handle)
        })
    }

    pub(super) fn update(&mut self, session: &Session) -> Result<()> {
        if self
            .request
            .as_ref()
            .is_some_and(|request| Instant::now() >= request.deadline)
        {
            let request = self.request.take().unwrap();
            session.send(json!({"type":"cancelCall","id":request.id}))?;
        }
        Ok(())
    }
}

impl App {
    /// After a character was typed: when `editor.formatOnType` is on and an extension formats on
    /// that character, asks it to format around the cursor.
    pub(crate) fn format_on_type(&mut self, typed: &str) {
        if !self.format_on_type
            || typed.chars().count() != 1
            || self.editor().selections().len() > 1
            || !self
                .plugins
                .vsix
                .as_ref()
                .is_some_and(|runtime| runtime.languages.has_provider(&[Feature::FormatOnType]))
        {
            return;
        }
        self.request_vsix_language_with(Feature::FormatOnType, Some(typed.to_owned()));
    }

    pub(crate) fn request_vsix_language(&mut self, feature: Feature) -> bool {
        self.request_vsix_language_with(feature, None)
    }

    /// Whether an active extension provider could answer `feature` for the current document.
    pub(crate) fn has_vsix_language_provider(&self, feature: Feature) -> bool {
        let Some(runtime) = self.plugins.vsix.as_ref() else {
            return false;
        };
        if !runtime.ready || !self.vsix_workspace_trusted() {
            return false;
        }
        let id = self.active_document();
        let Ok(document) = crate::app::services::snapshot_document(id, &self.documents[id]) else {
            return false;
        };
        runtime.languages.providers.values().any(|provider| {
            provider.feature == feature
                && provider
                    .selector
                    .iter()
                    .any(|selector| matches_document(selector, &document))
        })
    }

    pub(crate) fn request_vsix_language_with(
        &mut self,
        feature: Feature,
        argument: Option<String>,
    ) -> bool {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return false;
        };
        let result = self.start_vsix_language_request(&mut runtime, feature, argument);
        self.plugins.vsix = Some(runtime);
        match result {
            Ok(started) => started,
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn start_vsix_language_request(
        &self,
        runtime: &mut Runtime,
        feature: Feature,
        argument: Option<String>,
    ) -> Result<bool> {
        if !runtime.ready || !self.vsix_workspace_trusted() {
            return Ok(false);
        }
        let id = self.active_document();
        let document = crate::app::services::snapshot_document(id, &self.documents[id])
            .map_err(|e| e.to_string())?;
        let Some((&handle, _)) = runtime.languages.providers.iter().find(|(_, provider)| {
            provider.feature == feature
                && (feature != Feature::FormatOnType
                    || argument
                        .as_deref()
                        .is_some_and(|typed| provider.triggers.iter().any(|t| t == typed)))
                && provider
                    .selector
                    .iter()
                    .any(|selector| matches_document(selector, &document))
        }) else {
            return Ok(false);
        };
        self.sync_vsix_documents(runtime)?;
        let cursor = self.editor().cursor().char_idx;
        let position = documents::position(&document.text, cursor);
        let position = json!({"lineNumber":position.line + 1, "column":position.character + 1});
        let uri = json!({"$graphiteUri":document.uri});
        let selection = self.editor().selections().first().map(|selection| {
            let (start, end) = (
                selection.anchor.min(selection.head),
                selection.anchor.max(selection.head),
            );
            let (start, end) = (
                documents::position(&document.text, start),
                documents::position(&document.text, end),
            );
            json!({
                "startLineNumber": start.line + 1, "startColumn": start.character + 1,
                "endLineNumber": end.line + 1, "endColumn": end.character + 1,
            })
        });
        let (method, args) = match feature {
            Feature::Completion => (
                "$provideCompletionItems",
                json!([handle, uri, position, {"triggerKind":0}]),
            ),
            Feature::Hover => ("$provideHover", json!([handle, uri, position, null])),
            Feature::Format => (
                "$provideDocumentFormattingEdits",
                json!([handle, uri, {"tabSize":self.tab_size,"insertSpaces":self.insert_spaces}]),
            ),
            Feature::FormatOnType => (
                "$provideOnTypeFormattingEdits",
                json!([
                    handle,
                    uri,
                    position,
                    argument.clone().unwrap_or_default(),
                    {"tabSize":self.tab_size,"insertSpaces":self.insert_spaces}
                ]),
            ),
            Feature::FormatRange => (
                "$provideDocumentRangeFormattingEdits",
                json!([
                    handle,
                    uri,
                    selection
                        .clone()
                        .filter(|range| range["startLineNumber"] != range["endLineNumber"]
                            || range["startColumn"] != range["endColumn"])
                        .unwrap_or_else(|| line_range(&document.text, position["lineNumber"].as_u64().unwrap_or(1) as usize)),
                    {"tabSize":self.tab_size,"insertSpaces":self.insert_spaces}
                ]),
            ),
            // Requested by the passive drivers in `passive.rs`, never by a command.
            Feature::InlayHints | Feature::DocumentHighlights | Feature::FoldingRanges => {
                return Ok(false)
            }
            Feature::SelectionRanges => {
                ("$provideSelectionRanges", json!([handle, uri, [position]]))
            }
            Feature::Definition => ("$provideDefinition", json!([handle, uri, position])),
            Feature::Declaration => ("$provideDeclaration", json!([handle, uri, position])),
            Feature::Implementation => ("$provideImplementation", json!([handle, uri, position])),
            Feature::TypeDefinition => ("$provideTypeDefinition", json!([handle, uri, position])),
            Feature::References => (
                "$provideReferences",
                json!([handle, uri, position, {"includeDeclaration":true}]),
            ),
            Feature::DocumentSymbols => ("$provideDocumentSymbols", json!([handle, uri])),
            Feature::CodeActions => (
                "$provideCodeActions",
                json!([handle, uri, selection.unwrap_or_else(|| json!({
                    "startLineNumber": position["lineNumber"], "startColumn": position["column"],
                    "endLineNumber": position["lineNumber"], "endColumn": position["column"],
                })), {"trigger": 1}]),
            ),
            Feature::Rename => (
                "$provideRenameEdits",
                json!([handle, uri, position, argument.clone().unwrap_or_default()]),
            ),
            Feature::SignatureHelp => (
                "$provideSignatureHelp",
                json!([handle, uri, position, {"triggerKind":1, "isRetrigger":false}]),
            ),
        };
        if let Some(previous) = runtime.languages.request.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall", "id":previous.id}))?;
        }
        runtime.languages.next_id += 1;
        let id = runtime.languages.next_id + 1;
        runtime.session.send(json!({"type":"call", "id":id, "actor":"ExtHostLanguageFeatures", "method":method,"args":args,"cancellable":true}))?;
        runtime.languages.request = Some(Request {
            id,
            handle,
            feature,
            document,
            cursor,
            deadline: Instant::now() + Duration::from_secs(10),
            argument,
        });
        Ok(true)
    }

    pub(super) fn finish_vsix_language_request(
        &mut self,
        runtime: &mut Runtime,
        event: &Value,
    ) -> Result<()> {
        let Some(request) = runtime
            .languages
            .request
            .take_if(|request| event["id"].as_u64() == Some(request.id))
        else {
            return Ok(());
        };
        let result = &event["result"];
        let id = self.active_document();
        let current = request.document.id == Some(id)
            && request.document.version == Some(self.documents[id].buffer.revision())
            && self.editor().cursor().char_idx == request.cursor
            && runtime.languages.providers.contains_key(&request.handle);
        if request.feature == Feature::Completion {
            if let Some(cache) = result["x"].as_u64() {
                runtime.session.send(json!({"type":"call","actor":"ExtHostLanguageFeatures","method":"$releaseCompletionItems","args":[request.handle,cache]}))?;
            }
        }
        if !current {
            return Ok(());
        }
        if let Some(error) = event["error"].as_str() {
            self.status = error.to_owned();
            return Ok(());
        }
        match request.feature {
            Feature::Format | Feature::FormatRange | Feature::FormatOnType if result.is_array() => {
                self.apply_vsix_editor_edits(
                    request.document.uri.as_str(),
                    request.document.version.unwrap(),
                    result,
                )?;
            }
            Feature::Hover => {
                self.hover_text = result["contents"].as_array().map(|contents| {
                    contents
                        .iter()
                        .filter_map(|item| item.as_str().or_else(|| item["value"].as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
            Feature::Completion => {
                self.completion_items = result["b"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|item| {
                        let label = item["a"]
                            .as_str()
                            .or_else(|| item["a"]["label"].as_str())?
                            .to_owned();
                        Some(lsp_types::CompletionItem {
                            insert_text: item["h"].as_str().map(str::to_owned),
                            detail: item["c"].as_str().map(str::to_owned),
                            label,
                            ..Default::default()
                        })
                    })
                    .collect();
                self.completion_index = 0;
            }
            Feature::Definition | Feature::Declaration | Feature::TypeDefinition => {
                self.show_lsp_locations(
                    &language_results::locations(result),
                    "lsp.definition.none",
                );
            }
            Feature::Implementation => {
                self.show_lsp_locations(
                    &language_results::locations(result),
                    "lsp.implementation.none",
                );
            }
            Feature::References => {
                self.show_lsp_locations(
                    &language_results::locations(result),
                    "lsp.references.none",
                );
            }
            Feature::DocumentSymbols => self.show_vsix_symbols(&request.document, result),
            Feature::CodeActions => {
                let actions = language_results::code_actions(result)
                    .into_iter()
                    .filter_map(|action| serde_json::from_value(action).ok())
                    .map(lsp_types::CodeActionOrCommand::CodeAction)
                    .collect();
                self.quick_fix_actions = actions;
                self.quick_fix_index = 0;
                self.quick_fix_visible = true;
            }
            Feature::SelectionRanges => self.apply_vsix_selection_ranges(&request.document, result),
            Feature::Rename => self.finish_vsix_rename(result),
            Feature::SignatureHelp => {
                self.handle_lsp_query("signatureHelp", language_results::signature_help(result));
            }
            Feature::Format
            | Feature::FormatRange
            | Feature::FormatOnType
            | Feature::InlayHints
            | Feature::DocumentHighlights
            | Feature::FoldingRanges => {}
        }
        Ok(())
    }

    fn show_vsix_symbols(&mut self, document: &DocumentSnapshot, result: &Value) {
        let Ok(path) = document.uri.to_path() else {
            return;
        };
        let items = language_results::symbols(result)
            .into_iter()
            .map(|(name, scope, line)| crate::app::go_menu::GoSymbolItem {
                path: path.clone(),
                line,
                name,
                scope,
            })
            .collect::<Vec<_>>();
        self.open_go_picker(crate::app::go_menu::GoPickerKind::EditorSymbols);
        let empty = self.t("go.no_symbols").to_owned();
        if let Some(picker) = &mut self.go_picker {
            picker.message = items.is_empty().then_some(empty);
            picker.items = items;
        }
    }

    fn finish_vsix_rename(&mut self, result: &Value) {
        if let Some(reason) = result["rejectReason"].as_str() {
            self.status = reason.to_owned();
            return;
        }
        let edit = language_results::workspace_edit_changes(result)
            .and_then(|edit| serde_json::from_value::<lsp_types::WorkspaceEdit>(edit).ok());
        let Some(edit) = edit else {
            self.status = self.t("lsp.rename.none").into();
            return;
        };
        match self.apply_workspace_edit(edit) {
            Ok(()) => self.status = self.t("lsp.rename.done").into(),
            Err(error) => self.status = error,
        }
    }

    pub(crate) fn vsix_diagnostics(
        &self,
    ) -> Vec<(graphite_core_api::DocumentId, lsp_types::Diagnostic)> {
        self.plugins
            .vsix
            .iter()
            .flat_map(|runtime| runtime.languages.diagnostics.values())
            .flat_map(|uris| uris.iter())
            .filter_map(|(uri, diagnostics)| {
                let id = self.service_document_id(&DocumentUri::parse(uri).ok()?)?;
                Some(
                    diagnostics
                        .iter()
                        .cloned()
                        .map(move |diagnostic| (id, diagnostic)),
                )
            })
            .flatten()
            .collect()
    }
}

pub(super) fn language_rpc(
    languages: &mut Languages,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    let operation = || -> Result<Value> {
        let arg = |index| args.get(index).ok_or("Missing Code OSS language argument");
        let feature = match (actor, method) {
            ("MainThreadLanguageFeatures", "$registerHoverProvider") => Feature::Hover,
            ("MainThreadLanguageFeatures", "$registerCompletionsProvider") => Feature::Completion,
            ("MainThreadLanguageFeatures", "$registerDocumentFormattingSupport") => Feature::Format,
            ("MainThreadLanguageFeatures", "$registerRangeFormattingSupport") => {
                Feature::FormatRange
            }
            ("MainThreadLanguageFeatures", "$registerInlayHintsProvider") => Feature::InlayHints,
            ("MainThreadLanguageFeatures", "$registerOnTypeFormattingSupport") => {
                Feature::FormatOnType
            }
            ("MainThreadLanguageFeatures", "$registerSelectionRangeProvider") => {
                Feature::SelectionRanges
            }
            ("MainThreadLanguageFeatures", "$registerFoldingRangeProvider") => {
                Feature::FoldingRanges
            }
            ("MainThreadLanguageFeatures", "$registerDocumentHighlightProvider") => {
                Feature::DocumentHighlights
            }
            ("MainThreadLanguageFeatures", "$registerDefinitionSupport") => Feature::Definition,
            ("MainThreadLanguageFeatures", "$registerDeclarationSupport") => Feature::Declaration,
            ("MainThreadLanguageFeatures", "$registerImplementationSupport") => {
                Feature::Implementation
            }
            ("MainThreadLanguageFeatures", "$registerTypeDefinitionSupport") => {
                Feature::TypeDefinition
            }
            ("MainThreadLanguageFeatures", "$registerReferenceSupport") => Feature::References,
            ("MainThreadLanguageFeatures", "$registerDocumentSymbolProvider") => {
                Feature::DocumentSymbols
            }
            ("MainThreadLanguageFeatures", "$registerCodeActionSupport") => Feature::CodeActions,
            ("MainThreadLanguageFeatures", "$registerRenameSupport") => Feature::Rename,
            ("MainThreadLanguageFeatures", "$registerSignatureHelpProvider") => {
                Feature::SignatureHelp
            }
            ("MainThreadLanguageFeatures", "$unregister") => {
                languages
                    .providers
                    .remove(&arg(0)?.as_u64().ok_or("Invalid provider handle")?);
                return Ok(Value::Null);
            }
            ("MainThreadDiagnostics", "$clear") => {
                languages
                    .diagnostics
                    .remove(arg(0)?.as_str().ok_or("Invalid diagnostic owner")?);
                return Ok(Value::Null);
            }
            ("MainThreadDiagnostics", "$changeMany") => {
                let owner = arg(0)?.as_str().ok_or("Invalid diagnostic owner")?;
                let diagnostics = languages.diagnostics.entry(owner.to_owned()).or_default();
                for entry in arg(1)?.as_array().ok_or("Invalid diagnostic entries")? {
                    let uri = entry[0].as_str().ok_or("Invalid diagnostic URI")?;
                    let items = entry[1]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(diagnostic)
                        .collect::<Vec<_>>();
                    if items.is_empty() {
                        diagnostics.remove(uri);
                    } else {
                        diagnostics.insert(uri.to_owned(), items);
                    }
                }
                return Ok(Value::Null);
            }
            _ => return Err(format!("Graphite does not support {actor}.{method}")),
        };
        let handle = arg(0)?.as_u64().ok_or("Invalid provider handle")?;
        let selector = arg(1)?
            .as_array()
            .ok_or("Invalid provider selector")?
            .clone();
        let triggers = if feature == Feature::FormatOnType {
            args.get(2)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|trigger| trigger.as_str().map(str::to_owned))
                .collect()
        } else {
            Vec::new()
        };
        languages.providers.insert(
            handle,
            Provider {
                feature,
                selector,
                triggers,
            },
        );
        Ok(Value::Null)
    };
    matches!(
        actor,
        "MainThreadLanguageFeatures" | "MainThreadDiagnostics"
    )
    .then(operation)
}

/// The whole of the 1-based `line` as a Code OSS range.
pub(super) fn line_range(text: &str, line: usize) -> Value {
    let length = text
        .split('\n')
        .nth(line.saturating_sub(1))
        .map_or(0, |line| line.trim_end_matches('\r').encode_utf16().count());
    json!({
        "startLineNumber": line, "startColumn": 1,
        "endLineNumber": line, "endColumn": length + 1,
    })
}

fn diagnostic(marker: &Value) -> Option<lsp_types::Diagnostic> {
    let edit = documents::text_edit(&json!({"range":marker,"text":""})).ok()?;
    let severity = match marker["severity"].as_u64()? {
        8 => 1,
        4 => 2,
        2 => 3,
        1 => 4,
        _ => return None,
    };
    serde_json::from_value(json!({
        "range":{"start":edit.start,"end":edit.end},
        "severity":severity,"source":marker["source"].as_str().unwrap_or("VSIX"),
        "message":marker["message"].as_str()?,
    }))
    .ok()
}

fn matches_document(selector: &Value, document: &DocumentSnapshot) -> bool {
    let scheme = if document.uri.is_untitled() {
        "untitled"
    } else {
        "file"
    };
    if selector["notebookType"].is_string()
        || selector["language"].as_str().is_some_and(|language| {
            language != "*" && language != document.language.as_deref().unwrap_or("plaintext")
        })
        || selector["scheme"]
            .as_str()
            .is_some_and(|value| value != "*" && value != scheme)
    {
        return false;
    }
    if selector["pattern"].is_null() {
        return true;
    }
    let Some(pattern) = selector["pattern"].as_str() else {
        return false;
    };
    let Ok(path) = document.uri.to_path() else {
        return false;
    };
    globset::Glob::new(pattern).is_ok_and(|glob| glob.compile_matcher().is_match(path))
}
