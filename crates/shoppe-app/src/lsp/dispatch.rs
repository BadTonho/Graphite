use super::*;

pub(super) fn dispatch_server_message(
    message: Message,
    pending: &mut HashMap<i32, (u64, &'static str)>,
    event_tx: &Sender<LspUiEvent>,
) {
    match message {
        Message::Notification(notification)
            if notification.method == "textDocument/publishDiagnostics" =>
        {
            if let Ok(params) =
                serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
            {
                let _ = event_tx.send(LspUiEvent::Diagnostics {
                    uri: params.uri.to_string(),
                    version: params.version,
                    diagnostics: params.diagnostics,
                });
            }
        }
        Message::Response(response) => {
            let Some((request_id, kind)) = serde_json::to_value(&response.id)
                .ok()
                .and_then(|id| serde_json::from_value::<i32>(id).ok())
                .and_then(|id| pending.remove(&id))
            else {
                return;
            };
            let response_error = response.error.map(|error| error.message);
            let result = response.result.unwrap_or(Value::Null);
            if response_error.is_none() && !valid_response(kind, &result) {
                let _ = event_tx.send(LspUiEvent::RequestFailed {
                    request_id,
                    message: format!("Invalid {kind} response"),
                });
                return;
            }
            if let Some(message) = response_error
                .as_ref()
                .filter(|_| kind != "codeActions" && kind != "executeCommand")
            {
                let _ = event_tx.send(LspUiEvent::RequestFailed {
                    request_id,
                    message: message.clone(),
                });
                return;
            }
            match kind {
                "executeCommand" => {
                    if request_id != 0 {
                        let outcome = response_error
                            .clone()
                            .map_or_else(|| Ok(result.clone()), Err);
                        let _ = event_tx.send(LspUiEvent::ServerCommandCompleted {
                            request_id,
                            result: outcome,
                        });
                    }
                    let message = response_error
                        .map(|error| format!("Server command failed: {error}"))
                        .unwrap_or_else(|| "Server command completed".into());
                    if request_id == 0 {
                        let _ = event_tx.send(LspUiEvent::Status(message));
                    }
                }
                "completion" => {
                    let items = result
                        .as_array()
                        .cloned()
                        .or_else(|| result.get("items").and_then(Value::as_array).cloned())
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|v| serde_json::from_value::<CompletionItem>(v).ok())
                        .collect();
                    let _ = event_tx.send(LspUiEvent::Completion { request_id, items });
                }
                "definition" => {
                    let item = if result.is_array() {
                        result
                            .as_array()
                            .and_then(|a| a.first())
                            .cloned()
                            .unwrap_or(Value::Null)
                    } else {
                        result
                    };
                    let location = serde_json::from_value::<Location>(item.clone())
                        .ok()
                        .or_else(|| {
                            serde_json::from_value::<lsp_types::LocationLink>(item)
                                .ok()
                                .map(|link| {
                                    Location::new(link.target_uri, link.target_selection_range)
                                })
                        });
                    let _ = event_tx.send(LspUiEvent::Definition {
                        request_id,
                        location,
                    });
                }
                "hover" => {
                    let text = serde_json::from_value::<Hover>(result)
                        .ok()
                        .map(hover_to_string);
                    let _ = event_tx.send(LspUiEvent::Hover { request_id, text });
                }
                "inlayHints" => {
                    let hints = result
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|value| {
                            serde_json::from_value::<lsp_types::InlayHint>(value).ok()
                        })
                        .collect();
                    let _ = event_tx.send(LspUiEvent::InlayHints { request_id, hints });
                }
                "codeActions" => {
                    let actions = result
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|value| {
                            serde_json::from_value::<lsp_types::CodeActionOrCommand>(value).ok()
                        })
                        .collect();
                    let _ = event_tx.send(LspUiEvent::CodeActions {
                        request_id,
                        actions,
                        error: response_error,
                    });
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn valid_response(kind: &str, value: &Value) -> bool {
    if value.is_null() {
        return true;
    }
    match kind {
        "completion" => value
            .as_array()
            .or_else(|| value.get("items").and_then(Value::as_array))
            .is_some_and(|items| {
                items
                    .iter()
                    .all(|item| serde_json::from_value::<CompletionItem>(item.clone()).is_ok())
            }),
        "definition" => {
            let valid = |value: &Value| {
                serde_json::from_value::<Location>(value.clone()).is_ok()
                    || serde_json::from_value::<lsp_types::LocationLink>(value.clone()).is_ok()
            };
            value
                .as_array()
                .map_or_else(|| valid(value), |items| items.iter().all(valid))
        }
        "hover" => serde_json::from_value::<Hover>(value.clone()).is_ok(),
        "codeActions" => value.as_array().is_some_and(|items| {
            items.iter().all(|item| {
                serde_json::from_value::<lsp_types::CodeActionOrCommand>(item.clone()).is_ok()
            })
        }),
        _ => true,
    }
}
