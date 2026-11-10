use super::*;
use graphite_core_api::{DocumentSnapshot, DocumentUri};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Feature {
    Completion,
    Hover,
    Format,
}

struct Provider {
    feature: Feature,
    selector: Vec<Value>,
}

struct Request {
    id: u64,
    handle: u64,
    feature: Feature,
    document: DocumentSnapshot,
    cursor: usize,
    deadline: Instant,
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
    pub(crate) fn request_vsix_language(&mut self, feature: Feature) -> bool {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return false;
        };
        let result = self.start_vsix_language_request(&mut runtime, feature);
        self.plugins.vsix = Some(runtime);
        match result {
            Ok(started) => started,
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn start_vsix_language_request(&self, runtime: &mut Runtime, feature: Feature) -> Result<bool> {
        if !runtime.ready || !self.vsix_workspace_trusted() {
            return Ok(false);
        }
        let id = self.active_document();
        let document = crate::app::services::snapshot_document(id, &self.documents[id])
            .map_err(|e| e.to_string())?;
        let Some((&handle, _)) = runtime.languages.providers.iter().find(|(_, provider)| {
            provider.feature == feature
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
            Feature::Format if result.is_array() => {
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
            Feature::Format => {}
        }
        Ok(())
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
        languages
            .providers
            .insert(handle, Provider { feature, selector });
        Ok(Value::Null)
    };
    matches!(
        actor,
        "MainThreadLanguageFeatures" | "MainThreadDiagnostics"
    )
    .then(operation)
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
