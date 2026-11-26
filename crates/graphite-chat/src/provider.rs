//! Provider settings and the streaming HTTP call.

use crate::message::{ChatError, Request, StreamEvent};
use crate::sse::SseParser;
use crate::{anthropic, openai};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    /// Any server that speaks the OpenAI `/chat/completions` protocol.
    OpenAiCompatible,
    OpenCodeConsole,
    OpenCodeGo,
    Gemini,
    OpenRouter,
}

impl ProviderKind {
    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => anthropic::DEFAULT_BASE_URL,
            Self::OpenAiCompatible => openai::DEFAULT_BASE_URL,
            Self::OpenCodeConsole => "https://opencode.ai/inference/openai/v1",
            Self::OpenCodeGo => "https://opencode.ai/inference/go/openai/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    pub fn uses_opencode_session(self) -> bool {
        matches!(self, Self::OpenCodeConsole | Self::OpenCodeGo)
    }
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
    #[serde(skip)]
    pub opencode_session: Option<String>,
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
            _ => "",
        };
        Self {
            kind,
            base_url: String::new(),
            model: model.into(),
            api_key: None,
            opencode_session: None,
        }
    }

    fn base(&self) -> &str {
        let configured = self.base_url.trim().trim_end_matches('/');
        if !configured.is_empty()
            && matches!(
                self.kind,
                ProviderKind::Anthropic | ProviderKind::OpenAiCompatible
            )
        {
            return configured;
        }
        self.kind.default_base_url()
    }

    fn url(&self) -> String {
        match self.kind {
            ProviderKind::Anthropic => format!("{}/v1/messages", self.base()),
            _ => format!("{}/chat/completions", self.base()),
        }
    }

    /// Whether a call can be attempted. Local OpenAI-compatible servers need no key.
    pub fn check(&self) -> Result<(), ChatError> {
        self.checked_url()?;
        if self.model.trim().is_empty() {
            return Err(ChatError::NotConfigured("No model is chosen".into()));
        }
        let has_key = self
            .api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty());
        if self.kind != ProviderKind::OpenAiCompatible && !has_key {
            return Err(ChatError::NotConfigured(
                "No API key is set for the model service".into(),
            ));
        }
        Ok(())
    }

    fn checked_url(&self) -> Result<reqwest::Url, ChatError> {
        let invalid = || ChatError::NotConfigured("Use HTTPS or a local HTTP address".into());
        let base = reqwest::Url::parse(self.base()).map_err(|_| invalid())?;
        if !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.host().is_none()
            || !(base.scheme() == "https" || (base.scheme() == "http" && is_loopback(&base)))
        {
            return Err(invalid());
        }
        reqwest::Url::parse(&self.url()).map_err(|_| invalid())
    }
}

fn is_loopback(url: &reqwest::Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
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
    stream_at_url(config, config.checked_url()?, request, cancel, on_event)
}

fn stream_at_url(
    config: &ProviderConfig,
    url: reqwest::Url,
    request: &Request,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(StreamEvent),
) -> Result<(), ChatError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ChatError::Cancelled);
    }
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Graphite");
    if is_loopback(&url) {
        builder = builder.no_proxy();
    }
    let client = builder.build().map_err(network_error)?;
    let key = config
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    let (body, mut http, mut decoder) = match config.kind {
        ProviderKind::Anthropic => {
            let http = client
                .post(url)
                .header("anthropic-version", anthropic::API_VERSION)
                .header("x-api-key", key.unwrap_or(""));
            (
                anthropic::build_body(&config.model, request),
                http,
                Decoder::Anthropic(anthropic::Decoder::new()),
            )
        }
        _ => {
            let mut http = client.post(url);
            if let Some(key) = key {
                http = http.bearer_auth(key);
            }
            if config.kind.uses_opencode_session() {
                let session = config
                    .opencode_session
                    .as_deref()
                    .filter(|id| {
                        id.strip_prefix("ses_").is_some_and(|suffix| {
                            suffix.len() == 32
                                && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                        })
                    })
                    .ok_or_else(|| {
                        ChatError::NotConfigured(
                            "OpenCode requires a valid conversation identifier".into(),
                        )
                    })?;
                http = http.header("x-opencode-session", session);
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
    let mut response = http.send().map_err(network_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(ChatError::Http {
            status: status.as_u16(),
            message: error_message(config.kind, status.as_u16()),
        });
    }

    let mut parser = SseParser::new();
    let mut buffer = [0u8; 4096];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(ChatError::Cancelled);
        }
        let read = response.read(&mut buffer);
        if cancel.load(Ordering::Relaxed) {
            return Err(ChatError::Cancelled);
        }
        let read =
            read.map_err(|_| ChatError::Network("Could not read the model response".into()))?;
        let events = if read == 0 {
            parser.finish()
        } else {
            parser.push(&buffer[..read])
        }
        .map_err(|_| {
            ChatError::Network("The model service sent an event that is too large".into())
        })?;
        for event in events {
            let decoded = match &mut decoder {
                Decoder::Anthropic(decoder) => decoder.push(&event),
                Decoder::OpenAi(decoder) => decoder.push(&event),
            }
            .map_err(|_| {
                ChatError::Network("The model service returned an invalid stream".into())
            })?;
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
            for item in decoder.finish().map_err(|_| {
                ChatError::Network("The model service returned an invalid stream".into())
            })? {
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

fn network_error(error: reqwest::Error) -> ChatError {
    ChatError::Network(
        if error.is_timeout() {
            "The model service timed out"
        } else {
            "Could not connect to the model service"
        }
        .into(),
    )
}

fn error_message(kind: ProviderKind, status: u16) -> String {
    match status {
        401 if kind.uses_opencode_session() => "Check your OpenCode API key".into(),
        402 | 403 if kind == ProviderKind::OpenCodeConsole => {
            "OpenCode Console denied access. Check your API key, credits, and model access".into()
        }
        402 | 403 if kind == ProviderKind::OpenCodeGo => {
            "OpenCode Go denied access. Check your API key and Go subscription".into()
        }
        401 | 403 => "The model service rejected the API key".into(),
        404 => "The model service has no such address or model".into(),
        429 => "The model service is limiting requests".into(),
        300..=399 => "The model address returned a redirect".into(),
        _ => "The model service returned an error".into(),
    }
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

    #[test]
    fn hosted_presets_route_to_the_official_endpoints_and_only_opencode_sends_a_session() {
        const SESSION: &str = "ses_00000000000000000000000000000001";
        let cases = [
            (
                ProviderKind::OpenCodeConsole,
                "https://opencode.ai/inference/openai/v1/chat/completions",
            ),
            (
                ProviderKind::OpenCodeGo,
                "https://opencode.ai/inference/go/openai/v1/chat/completions",
            ),
            (
                ProviderKind::Gemini,
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            ),
            (
                ProviderKind::OpenRouter,
                "https://openrouter.ai/api/v1/chat/completions",
            ),
        ];
        for (kind, official) in cases {
            let mut config = ProviderConfig {
                base_url: "https://override.invalid/ignored".into(),
                model: "fixture/model".into(),
                opencode_session: Some(SESSION.into()),
                ..ProviderConfig::default_for(kind)
            };
            assert!(config.check().is_err());
            config.api_key = Some("key-fixture".into());
            config.check().unwrap();
            let url = config.checked_url().unwrap();
            assert_eq!(url.as_str(), official);
            let (fixture, server) = serve(
                "200 OK", "text/event-stream",
                "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".into(),
            );
            let local = reqwest::Url::parse(&fixture)
                .unwrap()
                .join(url.path())
                .unwrap();
            let mut events = Vec::new();
            stream_at_url(
                &config,
                local,
                &request(),
                &AtomicBool::new(false),
                &mut |event| events.push(event),
            )
            .unwrap();
            assert_eq!(events.first(), Some(&StreamEvent::Text("hello".into())));
            let received = server.join().unwrap();
            assert!(received.starts_with(&format!("POST {} ", url.path())));
            assert!(received.contains("authorization: Bearer key-fixture"));
            assert!(received.contains("\"model\":\"fixture/model\""));
            assert_eq!(
                received.contains(&format!("x-opencode-session: {SESSION}")),
                kind.uses_opencode_session()
            );
        }
    }

    #[test]
    fn opencode_requires_an_identifier_and_reports_access_errors_without_the_body() {
        let mut config = ProviderConfig {
            model: "fixture-model".into(),
            api_key: Some("key-fixture".into()),
            ..ProviderConfig::default_for(ProviderKind::OpenCodeGo)
        };
        assert!(matches!(run(&config).0, Err(ChatError::NotConfigured(_))));
        config.opencode_session = Some("ses_00000000000000000000000000000001".into());
        for (kind, message) in [
            (ProviderKind::OpenCodeConsole, "credits"),
            (ProviderKind::OpenCodeGo, "subscription"),
        ] {
            config.kind = kind;
            let (fixture, server) = serve(
                "403 Forbidden",
                "application/json",
                "private-service-response".into(),
            );
            let result = stream_at_url(
                &config,
                reqwest::Url::parse(&fixture).unwrap(),
                &request(),
                &AtomicBool::new(false),
                &mut |_| {},
            );
            server.join().unwrap();
            assert!(
                matches!(result, Err(ChatError::Http { status: 403, message: text }) if text.contains(message) && !text.contains("private-service-response"))
            );
        }
    }

    #[test]
    fn endpoints_reject_remote_plaintext_and_credentials_in_urls() {
        for base_url in [
            "http://example.invalid/v1",
            "https://user:secret@example.invalid/v1",
            "https://example.invalid/v1?token=fixture",
            "http://localhost.example.invalid/v1",
        ] {
            let config = ProviderConfig {
                base_url: base_url.into(),
                ..ProviderConfig::default_for(ProviderKind::OpenAiCompatible)
            };
            assert!(matches!(config.check(), Err(ChatError::NotConfigured(_))));
        }
    }

    #[test]
    fn the_client_does_not_follow_redirects() {
        let (target, server) = serve(
            "307 Temporary Redirect\r\nLocation: http://127.0.0.1:1/elsewhere",
            "text/plain",
            String::new(),
        );
        let config = ProviderConfig {
            base_url: target,
            ..ProviderConfig::default_for(ProviderKind::OpenAiCompatible)
        };
        let (result, _) = run(&config);
        server.join().unwrap();
        assert!(matches!(result, Err(ChatError::Http { status: 307, .. })));
    }

    #[test]
    fn only_attached_editor_content_appears_in_the_http_request() {
        use crate::agent::{build_system_prompt, ChatContext, FileAttachment};
        use crate::session::AgentMode;

        for attached in [false, true] {
            let mut context = ChatContext {
                workspace_root: "private-root-marker".into(),
                ..ChatContext::default()
            };
            if attached {
                context.referenced_files.push(FileAttachment {
                    path: "example.txt".into(),
                    content: "editor-body-marker".into(),
                });
            }
            let (base_url, server) =
                serve("200 OK", "text/event-stream", "data: [DONE]\n\n".into());
            let config = ProviderConfig {
                base_url,
                ..ProviderConfig::default_for(ProviderKind::OpenAiCompatible)
            };
            let mut request = request();
            request.system = build_system_prompt(AgentMode::Ask, &context);
            stream(&config, &request, &AtomicBool::new(false), &mut |_| {}).unwrap();
            let received = server.join().unwrap();
            assert_eq!(received.contains("editor-body-marker"), attached);
            assert!(!received.contains("private-root-marker"));
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
    fn an_error_status_does_not_reveal_the_service_body() {
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
                message: "The model service rejected the API key".into()
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
