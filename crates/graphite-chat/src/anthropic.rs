//! Wire format of the Anthropic Messages API.

use crate::message::{Block, ChatError, Request, StopReason, StreamEvent, Usage};
use crate::sse::SseEvent;
use serde_json::{json, Value};

pub const API_VERSION: &str = "2023-06-01";
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// The JSON body of a streaming `/v1/messages` call.
pub fn build_body(model: &str, request: &Request) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|message| {
            let content: Vec<Value> = message
                .blocks
                .iter()
                .map(|block| match block {
                    Block::Text { text } => json!({"type": "text", "text": text}),
                    Block::ToolUse { id, name, input } => {
                        json!({"type": "tool_use", "id": id, "name": name, "input": input})
                    }
                    Block::ToolResult { id, content, is_error } => json!({
                        "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error
                    }),
                })
                .collect();
            json!({"role": message.role, "content": content})
        })
        .collect();
    let mut body = json!({
        "model": model,
        "max_tokens": if request.max_tokens == 0 { 4096 } else { request.max_tokens },
        "stream": true,
        "messages": messages,
    });
    if !request.system.is_empty() {
        body["system"] = json!(request.system);
    }
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|tool| json!({"name": tool.name, "description": tool.description, "input_schema": tool.schema}))
            .collect();
    }
    body
}

/// Turns the server-sent events of one reply into [`StreamEvent`]s.
#[derive(Default)]
pub struct Decoder {
    /// The tool call being assembled: id, name and the JSON text received so far.
    tool: Option<(String, String, String)>,
    usage: Usage,
    stop: Option<StopReason>,
    finished: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: &SseEvent) -> Result<Vec<StreamEvent>, ChatError> {
        if event.data.is_empty() || event.event.as_deref() == Some("ping") {
            return Ok(Vec::new());
        }
        let data: Value = serde_json::from_str(&event.data)
            .map_err(|_| ChatError::Protocol("The server sent an event that is not JSON".into()))?;
        let kind = data["type"]
            .as_str()
            .or(event.event.as_deref())
            .unwrap_or("");
        let mut out = Vec::new();
        match kind {
            "message_start" => {
                self.usage.input_tokens = data["message"]["usage"]["input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
            }
            "content_block_start" => {
                let block = &data["content_block"];
                if block["type"] == "tool_use" {
                    self.tool = Some((
                        block["id"].as_str().unwrap_or("").to_string(),
                        block["name"].as_str().unwrap_or("").to_string(),
                        String::new(),
                    ));
                }
            }
            "content_block_delta" => {
                let delta = &data["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        if let Some(text) = delta["text"].as_str().filter(|text| !text.is_empty()) {
                            out.push(StreamEvent::Text(text.to_string()));
                        }
                    }
                    Some("input_json_delta") => {
                        if let (Some((_, _, json)), Some(part)) =
                            (self.tool.as_mut(), delta["partial_json"].as_str())
                        {
                            json.push_str(part);
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let Some((id, name, json)) = self.tool.take() {
                    let input = if json.trim().is_empty() {
                        json!({})
                    } else {
                        serde_json::from_str(&json).map_err(|_| {
                            ChatError::Protocol(format!(
                                "The arguments of tool call {name} are not valid JSON"
                            ))
                        })?
                    };
                    out.push(StreamEvent::ToolUse { id, name, input });
                }
            }
            "message_delta" => {
                if let Some(reason) = data["delta"]["stop_reason"].as_str() {
                    self.stop = Some(match reason {
                        "end_turn" | "stop_sequence" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        _ => StopReason::Other,
                    });
                }
                if let Some(tokens) = data["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = tokens as u32;
                }
            }
            "message_stop" => {
                self.finished = true;
                out.push(StreamEvent::Finished {
                    stop: self.stop.unwrap_or(StopReason::EndTurn),
                    usage: self.usage,
                });
            }
            "error" => {
                let message = data["error"]["message"]
                    .as_str()
                    .unwrap_or("The model service reported an error");
                return Err(ChatError::Http {
                    status: 0,
                    message: message.to_string(),
                });
            }
            _ => {}
        }
        Ok(out)
    }

    /// Whether `message_stop` was seen. A stream that ends without it was cut short.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Message, ToolSpec};

    fn ev(name: &str, data: Value) -> SseEvent {
        SseEvent {
            event: Some(name.into()),
            data: data.to_string(),
        }
    }

    #[test]
    fn the_body_carries_system_tools_and_tool_results() {
        let request = Request {
            system: "be brief".into(),
            messages: vec![
                Message::user("hi"),
                Message::assistant(vec![Block::ToolUse {
                    id: "t1".into(),
                    name: "read".into(),
                    input: json!({"path": "a"}),
                }]),
                Message::tool_results(vec![Block::ToolResult {
                    id: "t1".into(),
                    content: "text".into(),
                    is_error: false,
                }]),
            ],
            tools: vec![ToolSpec {
                name: "read".into(),
                description: "reads".into(),
                schema: json!({"type": "object"}),
            }],
            max_tokens: 0,
        };
        let body = build_body("m", &request);
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["system"], "be brief");
        assert_eq!(body["tools"][0]["input_schema"], json!({"type": "object"}));
        assert_eq!(body["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "t1");
        assert_eq!(body["messages"][2]["role"], "user");
    }

    #[test]
    fn text_and_a_tool_call_split_over_deltas_are_assembled() {
        let mut decoder = Decoder::new();
        let mut events = Vec::new();
        for event in [
            ev(
                "message_start",
                json!({"type": "message_start", "message": {"usage": {"input_tokens": 12}}}),
            ),
            ev(
                "content_block_start",
                json!({"type": "content_block_start", "content_block": {"type": "text"}}),
            ),
            ev(
                "content_block_delta",
                json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hel"}}),
            ),
            ev(
                "content_block_delta",
                json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "lo"}}),
            ),
            ev("content_block_stop", json!({"type": "content_block_stop"})),
            ev(
                "content_block_start",
                json!({"type": "content_block_start", "content_block": {"type": "tool_use", "id": "t1", "name": "read"}}),
            ),
            ev(
                "content_block_delta",
                json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": "{\"pa"}}),
            ),
            ev(
                "content_block_delta",
                json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": "th\":\"a\"}"}}),
            ),
            ev("content_block_stop", json!({"type": "content_block_stop"})),
            ev(
                "message_delta",
                json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 7}}),
            ),
            ev("message_stop", json!({"type": "message_stop"})),
        ] {
            events.extend(decoder.push(&event).unwrap());
        }
        assert_eq!(
            events,
            vec![
                StreamEvent::Text("Hel".into()),
                StreamEvent::Text("lo".into()),
                StreamEvent::ToolUse {
                    id: "t1".into(),
                    name: "read".into(),
                    input: json!({"path": "a"})
                },
                StreamEvent::Finished {
                    stop: StopReason::ToolUse,
                    usage: Usage {
                        input_tokens: 12,
                        output_tokens: 7
                    }
                },
            ]
        );
        assert!(decoder.is_finished());
    }

    #[test]
    fn a_tool_call_without_arguments_gets_an_empty_object() {
        let mut decoder = Decoder::new();
        decoder.push(&ev("content_block_start", json!({"type": "content_block_start", "content_block": {"type": "tool_use", "id": "t", "name": "list"}}))).unwrap();
        let out = decoder
            .push(&ev(
                "content_block_stop",
                json!({"type": "content_block_stop"}),
            ))
            .unwrap();
        assert_eq!(
            out,
            vec![StreamEvent::ToolUse {
                id: "t".into(),
                name: "list".into(),
                input: json!({})
            }]
        );
    }

    #[test]
    fn broken_tool_arguments_and_error_events_are_errors() {
        let mut decoder = Decoder::new();
        decoder.push(&ev("content_block_start", json!({"type": "content_block_start", "content_block": {"type": "tool_use", "id": "t", "name": "x"}}))).unwrap();
        decoder.push(&ev("content_block_delta", json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": "{bad"}}))).unwrap();
        assert!(matches!(
            decoder.push(&ev(
                "content_block_stop",
                json!({"type": "content_block_stop"})
            )),
            Err(ChatError::Protocol(_))
        ));
        let error = Decoder::new().push(&ev(
            "error",
            json!({"type": "error", "error": {"message": "overloaded"}}),
        ));
        assert!(matches!(error, Err(ChatError::Http { message, .. }) if message == "overloaded"));
    }

    #[test]
    fn pings_and_unknown_events_are_ignored() {
        let mut decoder = Decoder::new();
        assert!(decoder
            .push(&SseEvent {
                event: Some("ping".into()),
                data: "{}".into()
            })
            .unwrap()
            .is_empty());
        assert!(decoder
            .push(&ev("future_event", json!({"type": "future_event"})))
            .unwrap()
            .is_empty());
        assert!(!decoder.is_finished());
    }
}
