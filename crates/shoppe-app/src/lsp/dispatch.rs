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
            let result = response.result.unwrap_or(Value::Null);
            match kind {
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
                _ => {}
            }
        }
        _ => {}
    }
}
