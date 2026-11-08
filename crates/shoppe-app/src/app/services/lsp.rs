//! LSP conversion stays here; the shared API has no LSP dependency.
use super::*;
pub(in crate::app) fn workspace_edits(
    edit: lsp_types::WorkspaceEdit,
) -> ServiceResult<Vec<DocumentEdit>> {
    fn convert(
        uri: &str,
        version: Option<i32>,
        edits: Vec<lsp_types::TextEdit>,
    ) -> ServiceResult<DocumentEdit> {
        let version = version
            .map(|v| u64::try_from(v).map_err(|_| ServiceError::StaleDocument))
            .transpose()?;
        Ok(DocumentEdit {
            uri: DocumentUri::parse(uri)?,
            version,
            edits: edits
                .into_iter()
                .map(|e| shoppe_core_api::TextEdit {
                    start: Position {
                        line: e.range.start.line,
                        character: e.range.start.character,
                    },
                    end: Position {
                        line: e.range.end.line,
                        character: e.range.end.character,
                    },
                    text: e.new_text,
                })
                .collect(),
        })
    }
    let mut result = Vec::new();
    if let Some(changes) = edit.changes {
        for (uri, edits) in changes {
            result.push(convert(uri.as_str(), None, edits)?);
        }
    }
    if let Some(changes) = edit.document_changes {
        let docs = match changes {
            lsp_types::DocumentChanges::Edits(docs) => docs,
            lsp_types::DocumentChanges::Operations(ops) => ops
                .into_iter()
                .map(|op| match op {
                    lsp_types::DocumentChangeOperation::Edit(doc) => Ok(doc),
                    _ => Err(ServiceError::Failed(
                        "unsupported resource operations".into(),
                    )),
                })
                .collect::<ServiceResult<Vec<_>>>()?,
        };
        for doc in docs {
            result.push(convert(
                doc.text_document.uri.as_str(),
                doc.text_document.version,
                doc.edits
                    .into_iter()
                    .map(|e| match e {
                        lsp_types::OneOf::Left(e) => e,
                        lsp_types::OneOf::Right(e) => e.text_edit,
                    })
                    .collect(),
            )?);
        }
    }
    Ok(result)
}
pub(in crate::app) fn publish_document(
    manager: &mut LspManager,
    document: &DocumentSnapshot,
    workspace: &WorkspaceSnapshot,
    sender: std::sync::mpsc::Sender<LspUiEvent>,
    opened: bool,
) -> ServiceResult<()> {
    if document.uri.is_untitled() {
        return Ok(());
    }
    let path = document.uri.to_path()?;
    let root = workspace.root.to_path()?;
    let version = i32::try_from(document.version.ok_or(ServiceError::NotFound)?)
        .map_err(|_| ServiceError::Failed("LSP version exceeds i32".into()))?;
    if opened {
        manager.change(&path, document.text.clone(), version)
    } else {
        manager.open(&path, &root, document.text.clone(), version, sender)
    };
    Ok(())
}

/// Read-only workspace adapter accepts the service interface, not application state.
pub(in crate::app) fn workspace_request(
    service: &dyn WorkspaceService,
    method: &str,
    params: &serde_json::Value,
) -> ServiceResult<serde_json::Value> {
    use serde_json::{json, Value};
    match method {
        "workspace/workspaceFolders" => {
            let root = service.workspace()?.root;
            let path = root.to_path()?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("workspace");
            Ok(json!([{"uri":root.as_str(),"name":name}]))
        }
        "workspace/configuration" => {
            let configuration = service.configuration()?;
            let items = params
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ServiceError::InvalidArguments("Missing configuration items".into())
                })?;
            let mut result = Vec::new();
            for item in items {
                let section = item.get("section").and_then(Value::as_str).unwrap_or("");
                if section.is_empty() {
                    result.push(configuration.clone());
                    continue;
                }
                if let Some(value) = configuration.get(section) {
                    result.push(value.clone());
                    continue;
                }
                let prefix = format!("{section}.");
                let mut object = serde_json::Map::new();
                if let Some(settings) = configuration.as_object() {
                    for (key, value) in settings {
                        if let Some(key) = key.strip_prefix(&prefix) {
                            object.insert(key.into(), value.clone());
                        }
                    }
                }
                result.push(if object.is_empty() {
                    Value::Null
                } else {
                    Value::Object(object)
                });
            }
            Ok(Value::Array(result))
        }
        _ => Err(ServiceError::Failed("Unsupported workspace method".into())),
    }
}
