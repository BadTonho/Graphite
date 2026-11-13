//! Wire format of OpenAI-compatible `/chat/completions` servers (OpenAI, Ollama, LM Studio, ...).

use crate::message::{Block, ChatError, Request, Role, StopReason, StreamEvent, Usage};
use crate::sse::SseEvent;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// The JSON body of a streaming `/chat/completions` call.
pub fn build_body(model: &str, request: &Request) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if !request.system.is_empty() {
        messages.push(json!({"role": "system", "content": request.system}));
    }
    for message in &request.messages {
        match message.role {
            Role::User => {
                // Tool results are messages of their own in this format.
                let mut text = String::new();
                for block in &message.blocks {
                    match block {
                        Block::ToolResult { id, content, .. } => {
                            messages.push(json!({"role": "tool", "tool_call_id": id, "content": content}));
                        }
                        Block::Text { text: part } => text.push_str(part),
                        Block::ToolUse { .. } => {}
                    }
                }
                if !text.is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                }
            }
            Role::Assistant => {
                let calls: Vec<Value> = message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::ToolUse { id, name, input } => Some(json!({
                            "id": id, "type": "function",
                            "function": {"name": name, "arguments": input.to_string()}
                        })),
                        _ => None,
                    })
                    .collect();
                let text = message.text();
                let mut value = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) }});
                if !calls.is_empty() {
                    value["tool_calls"] = json!(calls);
                }
                messages.push(value);
            }
        }
    }
    let mut body = json!({
        "model": model,
        "stream": true,
        "stream_options": {"include_usage": true},
        "messages": messages,
    });
    if request.max_tokens > 0 {
        body["max_tokens"] = json!(request.max_tokens);
    }
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|tool| json!({"type": "function", "function": {"name": tool.name, "description": tool.description, "parameters": tool.schema}}))
            .collect();
    }
    body
}

/// Turns the chunks of one reply into [`StreamEvent`]s.
#[derive(Default)]
pub struct Decoder {
    /// Tool calls by index: id, name and the argument text received so far.
    calls: BTreeMap<u64, (String, String, String)>,
    stop: Option<StopReason>,
    usage: Usage,
    finished: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: &SseEvent) -> Result<Vec<StreamEvent>, ChatError> {
        let data = event.data.trim();
        if data.is_empty() {
            return Ok(Vec::new());
        }
        if data == "[DONE]" {
            return Ok(self.finish()?);
        }
        let value: Value = serde_json::from_str(data)
            .map_err(|_| ChatError::Protocol("The server sent a chunk that is not JSON".into()))?;
        if let Some(message) = value["error"]["message"].as_str() {
            return Err(ChatError::Http { status: 0, message: message.to_string() });
        }
        if let Some(usage) = value.get("usage").filter(|usage| usage.is_object()) {
            self.usage.input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0) as u32;
            self.usage.output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0) as u32;
        }
        let mut out = Vec::new();
        let Some(choice) = value["choices"].get(0) else { return Ok(out) };
        let delta = &choice["delta"];
        if let Some(text) = delta["content"].as_str().filter(|text| !text.is_empty()) {
            out.push(StreamEvent::Text(text.to_string()));
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for call in calls {
                let index = call["index"].as_u64().unwrap_or(0);
                let entry = self.calls.entry(index).or_default();
                if let Some(id) = call["id"].as_str() {
                    entry.0 = id.to_string();
                }
                if let Some(name) = call["function"]["name"].as_str() {
                    entry.1.push_str(name);
                }
                if let Some(arguments) = call["function"]["arguments"].as_str() {
                    entry.2.push_str(arguments);
                }
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.stop = Some(match reason {
                "stop" => StopReason::EndTurn,
                "tool_calls" | "function_call" => StopReason::ToolUse,
                "length" => StopReason::MaxTokens,
                _ => StopReason::Other,
            });
            out.extend(self.flush_calls()?);
        }
        Ok(out)
    }

    fn flush_calls(&mut self) -> Result<Vec<StreamEvent>, ChatError> {
        let mut out = Vec::new();
        for (_, (id, name, arguments)) in std::mem::take(&mut self.calls) {
            let input = if arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&arguments)
                    .map_err(|_| ChatError::Protocol(format!("The arguments of tool call {name} are not valid JSON")))?
            };
            out.push(StreamEvent::ToolUse { id, name, input });
        }
        Ok(out)
    }

    /// Ends the reply. Called by `[DONE]`; servers that omit it are finished by [`Self::finish`].
    pub fn finish(&mut self) -> Result<Vec<StreamEvent>, ChatError> {
        if self.finished {
            return Ok(Vec::new());
        }
        self.finished = true;
        let mut out = self.flush_calls()?;
        out.push(StreamEvent::Finished { stop: self.stop.unwrap_or(StopReason::EndTurn), usage: self.usage });
        Ok(out)
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Message, ToolSpec};

    fn chunk(value: Value) -> SseEvent {
        SseEvent { event: None, data: value.to_string() }
    }

    #[test]
    fn the_body_puts_system_first_and_tool_results_in_their_own_messages() {
        let request = Request {
            system: "sys".into(),
            messages: vec![
                Message::user("hi"),
                Message::assistant(vec![
                    Block::Text { text: "ok".into() },
                    Block::ToolUse { id: "c1".into(), name: "read".into(), input: json!({"path": "a"}) },
                ]),
                Message::tool_results(vec![Block::ToolResult { id: "c1".into(), content: "data".into(), is_error: false }]),
            ],
            tools: vec![ToolSpec { name: "read".into(), description: "d".into(), schema: json!({"type": "object"}) }],
            max_tokens: 100,
        };
        let body = build_body("m", &request);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][2]["tool_calls"][0]["function"]["arguments"], "{\"path\":\"a\"}");
        assert_eq!(body["messages"][3], json!({"role": "tool", "tool_call_id": "c1", "content": "data"}));
        assert_eq!(body["tools"][0]["function"]["parameters"], json!({"type": "object"}));
        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn an_assistant_message_with_only_tool_calls_has_null_content() {
        let request = Request {
            messages: vec![Message::assistant(vec![Block::ToolUse { id: "c".into(), name: "n".into(), input: json!({}) }])],
            ..Request::default()
        };
        assert_eq!(build_body("m", &request)["messages"][0]["content"], Value::Null);
    }

    #[test]
    fn text_and_tool_call_fragments_are_assembled_in_order() {
        let mut decoder = Decoder::new();
        let mut events = Vec::new();
        for event in [
            chunk(json!({"choices": [{"delta": {"content": "He"}}]})),
            chunk(json!({"choices": [{"delta": {"content": "llo"}}]})),
            chunk(json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "c1", "function": {"name": "read", "arguments": "{\"pa"}}]}}]})),
            chunk(json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "th\":1}"}}]}}]})),
            chunk(json!({"choices": [{"delta": {"tool_calls": [{"index": 1, "id": "c2", "function": {"name": "list", "arguments": ""}}]}}]})),
            chunk(json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]})),
            chunk(json!({"choices": [], "usage": {"prompt_tokens": 9, "completion_tokens": 4}})),
            SseEvent { event: None, data: "[DONE]".into() },
        ] {
            events.extend(decoder.push(&event).unwrap());
        }
        assert_eq!(
            events,
            vec![
                StreamEvent::Text("He".into()),
                StreamEvent::Text("llo".into()),
                StreamEvent::ToolUse { id: "c1".into(), name: "read".into(), input: json!({"path": 1}) },
                StreamEvent::ToolUse { id: "c2".into(), name: "list".into(), input: json!({}) },
                StreamEvent::Finished { stop: StopReason::ToolUse, usage: Usage { input_tokens: 9, output_tokens: 4 } },
            ]
        );
    }

    #[test]
    fn a_server_without_done_is_finished_by_the_caller_once() {
        let mut decoder = Decoder::new();
        decoder.push(&chunk(json!({"choices": [{"delta": {"content": "x"}, "finish_reason": "length"}]}))).unwrap();
        let end = decoder.finish().unwrap();
        assert_eq!(end, vec![StreamEvent::Finished { stop: StopReason::MaxTokens, usage: Usage::default() }]);
        assert!(decoder.finish().unwrap().is_empty());
    }

    #[test]
    fn errors_and_broken_arguments_are_reported() {
        let mut decoder = Decoder::new();
        assert!(matches!(decoder.push(&chunk(json!({"error": {"message": "quota"}}))), Err(ChatError::Http { message, .. }) if message == "quota"));
        let mut decoder = Decoder::new();
        decoder.push(&chunk(json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "c", "function": {"name": "n", "arguments": "{x"}}]}}]}))).unwrap();
        assert!(matches!(decoder.push(&chunk(json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}))), Err(ChatError::Protocol(_))));
    }
}
