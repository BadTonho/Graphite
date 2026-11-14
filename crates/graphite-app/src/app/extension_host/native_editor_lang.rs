//! Native plugin API handlers for active editor state, selections, decorations,
//! diagnostics, and language intelligence providers (completion, hover, formatting).

use super::App;
use graphite_core_api::{
    char_to_position, position_to_char, protocol::RpcRequest, ActiveEditorSnapshot,
    CancellationToken, CompletionEntry, DiagnosticSeverity, DocumentEdit, DocumentId,
    DocumentService, DocumentUri, EditorSelection, LanguageProviderKind, ServiceError,
    ServiceRequest, ServiceResult, TextEdit,
};
use serde_json::Value;
use std::time::Instant;

/// Registered language intelligence provider metadata for an active native plugin.
#[derive(Clone, Debug)]
pub(crate) struct NativeLanguageProvider {
    pub kind: LanguageProviderKind,
    pub languages: Vec<String>,
}

/// Bookkeeping for an in-flight RPC call to a native plugin language provider.
#[derive(Clone, Debug)]
pub(crate) struct PendingLanguageRequest {
    pub kind: LanguageProviderKind,
    pub uri: String,
    pub version: u64,
}

impl App {
    pub(super) fn handle_native_editor_lang_service(
        &mut self,
        request: ServiceRequest,
    ) -> Option<ServiceResult<Value>> {
        match request {
            ServiceRequest::ActiveEditor => Some(self.native_active_editor()),
            ServiceRequest::SetEditorSelection { selections } => {
                Some(self.native_set_editor_selection(selections))
            }
            ServiceRequest::RegisterDecorationType { key, options } => {
                if key.trim().is_empty() {
                    return Some(Err(ServiceError::Failed(
                        "Decoration type key must not be empty".into(),
                    )));
                }
                self.extension_host.decorations.set_style(key, &options);
                Some(Ok(Value::Null))
            }
            ServiceRequest::SetDecorations {
                decoration_type,
                uri,
                ranges,
            } => {
                if decoration_type.trim().is_empty() {
                    return Some(Err(ServiceError::Failed(
                        "Decoration type key must not be empty".into(),
                    )));
                }
                let converted = ranges
                    .into_iter()
                    .map(|range| {
                        (
                            (range.start.line, range.start.character),
                            (range.end.line, range.end.character),
                        )
                    })
                    .collect();
                self.extension_host.decorations.set_mark_ranges(
                    uri.as_str(),
                    &decoration_type,
                    converted,
                );
                Some(Ok(Value::Null))
            }
            ServiceRequest::DisposeDecorationType { decoration_type } => {
                self.extension_host
                    .decorations
                    .remove_style(&decoration_type);
                Some(Ok(Value::Null))
            }
            ServiceRequest::SetDiagnostics {
                owner,
                uri,
                diagnostics,
            } => {
                if owner.trim().is_empty() {
                    return Some(Err(ServiceError::Failed(
                        "Diagnostic owner must not be empty".into(),
                    )));
                }
                let converted: Vec<lsp_types::Diagnostic> = diagnostics
                    .into_iter()
                    .map(|entry| {
                        let severity = match entry.severity {
                            DiagnosticSeverity::Error => lsp_types::DiagnosticSeverity::ERROR,
                            DiagnosticSeverity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                            DiagnosticSeverity::Information => {
                                lsp_types::DiagnosticSeverity::INFORMATION
                            }
                            DiagnosticSeverity::Hint => lsp_types::DiagnosticSeverity::HINT,
                        };
                        lsp_types::Diagnostic {
                            range: lsp_types::Range::new(
                                lsp_types::Position::new(entry.start.line, entry.start.character),
                                lsp_types::Position::new(entry.end.line, entry.end.character),
                            ),
                            severity: Some(severity),
                            code: entry.code.map(lsp_types::NumberOrString::String),
                            code_description: None,
                            source: entry.source.or_else(|| Some(owner.clone())),
                            message: entry.message,
                            related_information: None,
                            tags: None,
                            data: None,
                        }
                    })
                    .collect();
                let by_uri = self.extension_host.diagnostics.entry(owner).or_default();
                if converted.is_empty() {
                    by_uri.remove(uri.as_str());
                } else {
                    by_uri.insert(uri.as_str().to_owned(), converted);
                }
                Some(Ok(Value::Null))
            }
            ServiceRequest::RegisterLanguageProvider {
                provider_id,
                kind,
                languages,
            } => {
                if provider_id.trim().is_empty() {
                    return Some(Err(ServiceError::Failed(
                        "Language provider id must not be empty".into(),
                    )));
                }
                self.extension_host
                    .language_providers
                    .insert(provider_id, NativeLanguageProvider { kind, languages });
                Some(Ok(Value::Null))
            }
            ServiceRequest::DisposeLanguageProvider { provider_id } => {
                self.extension_host.language_providers.remove(&provider_id);
                Some(Ok(Value::Null))
            }
            _ => None,
        }
    }

    fn native_active_editor(&self) -> ServiceResult<Value> {
        if self.documents.len() == 0 {
            return Ok(Value::Null);
        }
        let doc_id = self.command_document();
        let document = super::super::services::snapshot_document(doc_id, &self.documents[doc_id])?;
        let selections = self
            .editor()
            .selections()
            .into_iter()
            .map(|sel| EditorSelection {
                anchor: char_to_position(&document.text, sel.anchor),
                active: char_to_position(&document.text, sel.head),
            })
            .collect();
        serde_json::to_value(ActiveEditorSnapshot {
            document,
            selections,
        })
        .map_err(|error| ServiceError::Failed(error.to_string()))
    }

    fn native_set_editor_selection(
        &mut self,
        selections: Vec<EditorSelection>,
    ) -> ServiceResult<Value> {
        if self.documents.len() == 0 {
            return Err(ServiceError::NotFound);
        }
        if selections.is_empty() {
            return Err(ServiceError::Failed(
                "At least one selection must be provided".into(),
            ));
        }
        let doc_id = self.command_document();
        let text = self.documents[doc_id].buffer.text();
        let mut mapped = Vec::with_capacity(selections.len());
        for selection in selections {
            let anchor = position_to_char(&text, selection.anchor).ok_or_else(|| {
                ServiceError::Failed("Invalid anchor position for active document".into())
            })?;
            let head = position_to_char(&text, selection.active).ok_or_else(|| {
                ServiceError::Failed("Invalid active position for active document".into())
            })?;
            mapped.push(graphite_text::Selection { anchor, head });
        }
        self.editor_mut().set_selections(mapped);
        Ok(Value::Null)
    }

    /// Diagnostics published by active native plugins for currently open documents.
    pub(crate) fn native_diagnostics(&self) -> Vec<(DocumentId, lsp_types::Diagnostic)> {
        self.plugins
            .hosts
            .values()
            .chain(std::iter::once(&self.extension_host))
            .flat_map(|host| host.diagnostics.values())
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

    /// Dispatches a language intelligence request (`Completion`, `Hover`, `Formatter`) to the first
    /// active native plugin with a matching provider. Returns `true` when handled.
    pub(crate) fn request_native_language(&mut self, kind: LanguageProviderKind) -> bool {
        if self.documents.len() == 0 {
            return false;
        }
        let doc_id = self.command_document();
        let Ok(snapshot) =
            super::super::services::snapshot_document(doc_id, &self.documents[doc_id])
        else {
            return false;
        };
        let doc_ext = snapshot.uri.to_path().ok().and_then(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
        });
        let doc_lang = snapshot.language.as_deref();
        let position = char_to_position(&snapshot.text, self.editor().cursor().char_idx);
        let tab_size = self.tab_size as u32;
        let insert_spaces = self.insert_spaces;

        let dispatch_to = |host: &mut super::ExtensionRuntime| -> bool {
            if !host.native_active || host.language_providers.is_empty() {
                return false;
            }
            let declared_lang = doc_lang.or_else(|| {
                let ext = doc_ext.as_deref()?;
                host.native_manifest.as_ref().and_then(|manifest| {
                    manifest.languages.iter().find_map(|lang| {
                        lang.extensions
                            .iter()
                            .any(|e| e.strip_prefix('.').unwrap_or(e).eq_ignore_ascii_case(ext))
                            .then_some(lang.id.as_str())
                    })
                })
            });
            let Some(provider_id) = host
                .language_providers
                .iter()
                .find(|(_, provider)| {
                    provider.kind == kind
                        && (provider.languages.is_empty()
                            || provider.languages.iter().any(|p| p == "*")
                            || declared_lang.is_some_and(|lang| {
                                provider
                                    .languages
                                    .iter()
                                    .any(|p| p.eq_ignore_ascii_case(lang))
                            }))
                })
                .map(|(id, _)| id.clone())
            else {
                return false;
            };
            let request = match kind {
                LanguageProviderKind::Completion => RpcRequest::ProvideCompletions {
                    provider_id,
                    uri: snapshot.uri.clone(),
                    position,
                },
                LanguageProviderKind::Hover => RpcRequest::ProvideHover {
                    provider_id,
                    uri: snapshot.uri.clone(),
                    position,
                },
                LanguageProviderKind::Formatter => RpcRequest::ProvideDocumentFormatting {
                    provider_id,
                    uri: snapshot.uri.clone(),
                    tab_size,
                    insert_spaces,
                },
            };
            let Some(supervisor) = host.supervisor.as_mut() else {
                return false;
            };
            match supervisor.request(request, CancellationToken::default(), Instant::now()) {
                Ok(rpc_id) => {
                    host.language_replies.insert(
                        rpc_id,
                        PendingLanguageRequest {
                            kind,
                            uri: snapshot.uri.as_str().to_owned(),
                            version: snapshot.version.unwrap_or(0),
                        },
                    );
                    true
                }
                Err(_) => false,
            }
        };

        if dispatch_to(&mut self.extension_host) {
            return true;
        }
        for host in self.plugins.hosts.values_mut() {
            if dispatch_to(host) {
                return true;
            }
        }
        false
    }

    pub(super) fn apply_native_language_reply(
        &mut self,
        pending: PendingLanguageRequest,
        result: Result<Value, ServiceError>,
    ) {
        let Ok(value) = result else {
            return;
        };
        match pending.kind {
            LanguageProviderKind::Completion => {
                let entries: Vec<CompletionEntry> =
                    serde_json::from_value(value).unwrap_or_default();
                self.completion_items = entries
                    .into_iter()
                    .map(|entry| lsp_types::CompletionItem {
                        label: entry.label,
                        detail: entry.detail,
                        insert_text: entry.insert_text,
                        documentation: entry.documentation.map(lsp_types::Documentation::String),
                        ..Default::default()
                    })
                    .collect();
                self.completion_index = 0;
            }
            LanguageProviderKind::Hover => {
                self.hover_text = match value {
                    Value::String(text) if !text.is_empty() => Some(text),
                    _ => None,
                };
            }
            LanguageProviderKind::Formatter => {
                let edits: Vec<TextEdit> = serde_json::from_value(value).unwrap_or_default();
                if edits.is_empty() {
                    return;
                }
                if let Ok(uri) = DocumentUri::parse(&pending.uri) {
                    let _ = self.apply_edits(vec![DocumentEdit {
                        uri,
                        version: Some(pending.version),
                        edits,
                    }]);
                }
            }
        }
    }
}
