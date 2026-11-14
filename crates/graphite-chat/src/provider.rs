//! Provider settings and the streaming HTTP call.

use crate::message::{ChatError, Request, StreamEvent};
use crate::sse::SseParser;
use crate::{anthropic, openai};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    /// Any server that speaks the OpenAI `/chat/completions` protocol.
    OpenAiCompatible,
}

/// Which model service to call. The key is a secret: `Debug` never prints it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    /// Empty means the default address of the kind.
    #[serde(default)]
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl ProviderConfig {
    /// Default settings for a kind, without a key.
    pub fn default_for(kind: ProviderKind) -> Self {
        let model = match kind {
            ProviderKind::Anthropic => "claude-sonnet-5-5",
            ProviderKind::OpenAiCompatible => "gpt-4o",
        };
        Self {
            kind,
            base_url: String::new(),
            model: model.into(),
            api_key: None,
        }
    }

    fn base(&self) -> &str {
        let configured = self.base_url.trim().trim_end_matches('/');
        if !configured.is_empty() {
            return configured;
        }
        match self.kind {
            ProviderKind::Anthropic => anthropic::DEFAULT_BASE_URL,
            ProviderKind::OpenAiCompatible => openai::DEFAULT_BASE_URL,
        }
    }

    fn url(&self) -> String {
        match self.kind {
            ProviderKind::Anthropic => format!("{}/v1/messages", self.base()),
            ProviderKind::OpenAiCompatible => format!("{}/chat/completions", self.base()),
        }
    }

    /// Whether a call can be attempted. Local OpenAI-compatible servers need no key.
    pub fn check(&self) -> Result<(), ChatError> {
        if self.model.trim().is_empty() {
            return Err(ChatError::NotConfigured("No model is chosen".into()));
        }
        let has_key = self
            .api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty());
        if self.kind == ProviderKind::Anthropic && !has_key {
            return Err(ChatError::NotConfigured(
                "No API key is set for the model service".into(),
            ));
        }
        Ok(())
    }
}

enum Decoder {
    Anthropic(anthropic::Decoder),
    OpenAi(openai::Decoder),
}

/// Sends `request` and reports the reply to `on_event` as it streams in. Blocks until the reply
/// ends, so call it from a background thread. `cancel` is checked between chunks of the reply.
pub fn stream(
    config: &ProviderConfig,
    request: &Request,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(StreamEvent),
) -> Result<(), ChatError> {
    config.check()?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ChatError::Cancelled);
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent("Graphite")
        .build()
        .map_err(|error| ChatError::Network(error.to_string()))?;
    let key = config
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    let (body, mut http, mut decoder) = match config.kind {
        ProviderKind::Anthropic => {
            let http = client
                .post(config.url())
                .header("anthropic-version", anthropic::API_VERSION)
                .header("x-api-key", key.unwrap_or(""));
            (
                anthropic::build_body(&config.model, request),
                http,
                Decoder::Anthropic(anthropic::Decoder::new()),
            )
        }
        ProviderKind::OpenAiCompatible => {
            let mut http = client.post(config.url());
            if let Some(key) = key {
                http = http.bearer_auth(key);
            }
            (
                openai::build_body(&config.model, request),
                http,
                Decoder::OpenAi(openai::Decoder::new()),
            )
        }
    };
    http = http
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json")
        .body(body.to_string());
    let mut response = http
        .send()
        .map_err(|error| ChatError::Network(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().unwrap_or_default();
        return Err(ChatError::Http {
            status: status.as_u16(),
            message: error_message(&text, status.as_u16()),
        });
    }

    let mut parser = SseParser::new();
    let mut buffer = [0u8; 4096];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(ChatError::Cancelled);
        }
        let read = response
            .read(&mut buffer)
            .map_err(|error| ChatError::Network(error.to_string()))?;
        let events = if read == 0 {
            parser.finish()
        } else {
            parser.push(&buffer[..read])
        };
        for event in events {
            let decoded = match &mut decoder {
                Decoder::Anthropic(decoder) => decoder.push(&event)?,
                Decoder::OpenAi(decoder) => decoder.push(&event)?,
            };
            for item in decoded {
                on_event(item);
            }
        }
        if read == 0 {
            break;
        }
    }
    match decoder {
        // Some servers end the stream without a `[DONE]` line.
        Decoder::OpenAi(mut decoder) if !decoder.is_finished() => {
            for item in decoder.finish()? {
                on_event(item);
            }
            Ok(())
        }
        Decoder::Anthropic(decoder) if !decoder.is_finished() => Err(ChatError::Network(
            "The connection closed before the reply was complete".into(),
        )),
        _ => Ok(()),
    }
}

/// The message of an error body in either provider format, or a generic line.
fn error_message(body: &str, status: u16) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| match status {
            401 | 403 => "The model service rejected the API key".into(),
            404 => "The model service has no such address or model".into(),
            429 => "The model service is limiting requests".into(),
            _ => "The model service returned an error".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Message, StopReason, Usage};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// Serves one canned HTTP response and returns the request it received.
    fn serve(
        status: &str,
        content_type: &'static str,
        body: String,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_string();
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let mut payload = vec![0u8; length];
            reader.read_exact(&mut payload).unwrap();
            head.push_str(&String::from_utf8_lossy(&payload));
            let reply = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}");
            socket.write_all(reply.as_bytes()).unwrap();
            head
        });
        (address, handle)
    }

    fn request() -> Request {
        Request {
            messages: vec![Message::user("hi")],
            max_tokens: 50,
            ..Request::default()
        }
    }

    fn run(config: &ProviderConfig) -> (Result<(), ChatError>, Vec<StreamEvent>) {
        let mut events = Vec::new();
        let result = stream(config, &request(), &AtomicBool::new(false), &mut |event| {
            events.push(event)
        });
        (result, events)
    }

    fn sse(parts: &[(&str, &str)]) -> String {
        parts
            .iter()
            .map(|(name, data)| format!("event: {name}\ndata: {data}\n\n"))
            .collect()
    }

    #[test]
    fn an_anthropic_reply_streams_text_and_finishes() {
        let body = sse(&[
            (
                "message_start",
                r#"{"type":"message_start","message":{"usage":{"input_tokens":3}}}"#,
            ),
            (
                "content_block_delta",
                r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}"#,
            ),
            (
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#,
            ),
            ("message_stop", r#"{"type":"message_stop"}"#),
        ]);
        let (address, server) = serve("200 OK", "text/event-stream", body);
        let config = ProviderConfig {
            base_url: address,
            api_key: Some("sk-test".into()),
            ..ProviderConfig::default_for(ProviderKind::Anthropic)
        };
        let (result, events) = run(&config);
        result.unwrap();
        assert_eq!(
            events,
            vec![
                StreamEvent::Text("Hi".into()),
                StreamEvent::Finished {
                    stop: StopReason::EndTurn,
                    usage: Usage {
                        input_tokens: 3,
                        output_tokens: 1
                    }
                },
            ]
        );
        let received = server.join().unwrap().to_ascii_lowercase();
        assert!(received.starts_with("post /v1/messages "));
        assert!(received.contains("x-api-key: sk-test"));
        assert!(received.contains("anthropic-version: 2023-06-01"));
        assert!(received.contains("\"stream\":true"));
    }

    #[test]
    fn an_openai_compatible_reply_streams_without_a_key_and_without_done() {
        let body =
            "data: {\"choices\":[{\"delta\":{\"content\":\"Oi\"},\"finish_reason\":\"stop\"}]}\n\n"
                .to_string();
        let (address, server) = serve("200 OK", "text/event-stream", body);
        let config = ProviderConfig {
            base_url: format!("{address}/"),
            ..ProviderConfig::default_for(ProviderKind::OpenAiCompatible)
        };
        let (result, events) = run(&config);
        result.unwrap();
        assert_eq!(events[0], StreamEvent::Text("Oi".into()));
        assert!(matches!(
            events[1],
            StreamEvent::Finished {
                stop: StopReason::EndTurn,
                ..
            }
        ));
        let received = server.join().unwrap().to_ascii_lowercase();
        assert!(received.starts_with("post /chat/completions "));
        assert!(!received.contains("authorization"));
    }

    #[test]
    fn an_error_status_reports_the_message_of_the_service() {
        let (address, server) = serve(
            "401 Unauthorized",
            "application/json",
            r#"{"error":{"message":"invalid x-api-key"}}"#.into(),
        );
        let config = ProviderConfig {
            base_url: address,
            api_key: Some("bad".into()),
            ..ProviderConfig::default_for(ProviderKind::Anthropic)
        };
        let (result, events) = run(&config);
        server.join().unwrap();
        assert!(events.is_empty());
        assert_eq!(
            result,
            Err(ChatError::Http {
                status: 401,
                message: "invalid x-api-key".into()
            })
        );
    }

    #[test]
    fn an_anthropic_stream_cut_before_the_end_is_an_error() {
        let body = sse(&[(
            "content_block_delta",
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}"#,
        )]);
        let (address, server) = serve("200 OK", "text/event-stream", body);
        let config = ProviderConfig {
            base_url: address,
            api_key: Some("k".into()),
            ..ProviderConfig::default_for(ProviderKind::Anthropic)
        };
        let (result, events) = run(&config);
        server.join().unwrap();
        assert_eq!(events, vec![StreamEvent::Text("Hi".into())]);
        assert!(matches!(result, Err(ChatError::Network(_))));
    }

    #[test]
    fn a_missing_key_or_model_is_reported_before_any_request() {
        let config = ProviderConfig::default_for(ProviderKind::Anthropic);
        assert!(matches!(run(&config).0, Err(ChatError::NotConfigured(_))));
        let config = ProviderConfig {
            model: " ".into(),
            ..ProviderConfig::default_for(ProviderKind::OpenAiCompatible)
        };
        assert!(matches!(run(&config).0, Err(ChatError::NotConfigured(_))));
    }

    #[test]
    fn a_cancelled_call_does_not_connect() {
        let config = ProviderConfig {
            api_key: Some("k".into()),
            base_url: "http://127.0.0.1:1".into(),
            ..ProviderConfig::default_for(ProviderKind::Anthropic)
        };
        let result = stream(&config, &request(), &AtomicBool::new(true), &mut |_| {});
        assert_eq!(result, Err(ChatError::Cancelled));
    }

    #[test]
    fn debug_output_never_contains_the_key() {
        let config = ProviderConfig {
            api_key: Some("sk-secret".into()),
            ..ProviderConfig::default_for(ProviderKind::Anthropic)
        };
        assert!(!format!("{config:?}").contains("sk-secret"));
    }
}
