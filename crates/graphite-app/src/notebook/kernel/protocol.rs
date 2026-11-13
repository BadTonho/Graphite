//! The Jupyter messaging protocol: signed multipart messages and the events a kernel reports.

use ring::{hmac, rand::{SecureRandom, SystemRandom}};
use serde_json::{json, Value};

pub(crate) const DELIMITER: &[u8] = b"<IDS|MSG>";
const PROTOCOL_VERSION: &str = "5.3";

/// A message as exchanged on the shell, control and IOPub channels.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Message {
    pub header: Value,
    pub parent_header: Value,
    pub metadata: Value,
    pub content: Value,
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).ok())
        .collect()
}

/// A random 128-bit identifier as 32 hex digits.
pub(crate) fn random_id() -> String {
    let mut bytes = [0u8; 16];
    if SystemRandom::new().fill(&mut bytes).is_err() {
        // The system generator failed; fall back to the clock so ids still differ.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        bytes.copy_from_slice(&nanos.to_be_bytes());
    }
    hex(&bytes)
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs() as i64;
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        now.subsec_millis()
    )
}

impl Message {
    /// A request of `msg_type` for the client session `session`.
    pub(crate) fn request(msg_type: &str, session: &str, content: Value) -> Self {
        Self {
            header: json!({
                "msg_id": random_id(),
                "session": session,
                "username": "graphite",
                "date": timestamp(),
                "msg_type": msg_type,
                "version": PROTOCOL_VERSION,
            }),
            parent_header: json!({}),
            metadata: json!({}),
            content,
        }
    }

    /// A message that answers `parent` (what a kernel sends).
    #[cfg(test)]
    pub(crate) fn reply_to(parent: &Message, msg_type: &str, content: Value) -> Self {
        let mut reply = Self::request(
            msg_type,
            parent.header["session"].as_str().unwrap_or(""),
            content,
        );
        reply.parent_header = parent.header.clone();
        reply
    }

    pub(crate) fn id(&self) -> &str {
        self.header["msg_id"].as_str().unwrap_or("")
    }

    pub(crate) fn msg_type(&self) -> &str {
        self.header["msg_type"].as_str().unwrap_or("")
    }

    /// Id of the request this message answers, if any.
    pub(crate) fn parent_id(&self) -> Option<&str> {
        self.parent_header["msg_id"].as_str().filter(|id| !id.is_empty())
    }
}

fn signature(key: &[u8], parts: [&[u8]; 4]) -> String {
    if key.is_empty() {
        return String::new();
    }
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    let mut context = hmac::Context::with_key(&key);
    for part in parts {
        context.update(part);
    }
    hex(context.sign().as_ref())
}

/// The frames to send for `message`, with identities (routing frames) in front.
pub(crate) fn encode(key: &[u8], identities: &[Vec<u8>], message: &Message) -> Vec<Vec<u8>> {
    let header = message.header.to_string().into_bytes();
    let parent = message.parent_header.to_string().into_bytes();
    let metadata = message.metadata.to_string().into_bytes();
    let content = message.content.to_string().into_bytes();
    let signed = signature(key, [&header, &parent, &metadata, &content]);
    let mut frames: Vec<Vec<u8>> = identities.to_vec();
    frames.push(DELIMITER.to_vec());
    frames.push(signed.into_bytes());
    frames.extend([header, parent, metadata, content]);
    frames
}

/// Reads the frames of a received message and checks its signature.
pub(crate) fn decode(key: &[u8], frames: &[Vec<u8>]) -> Result<Message, String> {
    let delimiter = frames
        .iter()
        .position(|frame| frame == DELIMITER)
        .ok_or("a message has no delimiter")?;
    let body = &frames[delimiter + 1..];
    if body.len() < 5 {
        return Err("a message is incomplete".into());
    }
    let received = std::str::from_utf8(&body[0]).map_err(|_| "a signature is not text")?;
    if !key.is_empty() {
        let expected = signature(key, [&body[1], &body[2], &body[3], &body[4]]);
        let matches = unhex(received)
            .zip(unhex(&expected))
            .is_some_and(|(received, expected)| received == expected);
        if !matches {
            return Err("a message has a wrong signature".into());
        }
    }
    let parse = |bytes: &[u8]| -> Result<Value, String> {
        serde_json::from_slice(bytes).map_err(|error| format!("a message part is not JSON: {error}"))
    };
    Ok(Message {
        header: parse(&body[1])?,
        parent_header: parse(&body[2])?,
        metadata: parse(&body[3])?,
        content: parse(&body[4])?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KernelState {
    Starting,
    Idle,
    Busy,
    /// The kernel process is gone.
    Dead,
}

/// What a kernel tells us, with output messages already in notebook form.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum KernelEvent {
    Status {
        state: KernelState,
        parent: Option<String>,
    },
    /// The kernel started running the code of a request.
    ExecuteInput { parent: String, count: u64 },
    /// An output of the request, as stored in a notebook cell.
    Output { parent: String, output: Value },
    /// An output that replaces the one with the same `display_id`.
    UpdateDisplay {
        parent: String,
        display_id: String,
        output: Value,
    },
    ClearOutput { parent: String, wait: bool },
    /// The request finished.
    Reply {
        parent: String,
        ok: bool,
        count: Option<u64>,
    },
    /// The kernel process ended (or never came up); `details` is what it printed to stderr.
    Exited { details: String },
    /// The kernel answered `kernel_info_request`.
    Info { language: Option<String>, banner: Option<String> },
}

/// Turns a message received from the kernel into an event, if it is one we use.
pub(crate) fn event_from(message: &Message) -> Option<KernelEvent> {
    let content = &message.content;
    let parent = message.parent_id().map(str::to_owned);
    let with_parent = |parent: Option<String>| parent.unwrap_or_default();
    match message.msg_type() {
        "status" => Some(KernelEvent::Status {
            state: match content["execution_state"].as_str()? {
                "idle" => KernelState::Idle,
                "busy" => KernelState::Busy,
                "starting" => KernelState::Starting,
                _ => return None,
            },
            parent,
        }),
        "execute_input" => Some(KernelEvent::ExecuteInput {
            parent: with_parent(parent),
            count: content["execution_count"].as_u64()?,
        }),
        "stream" => Some(KernelEvent::Output {
            parent: with_parent(parent),
            output: json!({
                "output_type": "stream",
                "name": content["name"].as_str().unwrap_or("stdout"),
                "text": content["text"].as_str().unwrap_or(""),
            }),
        }),
        "display_data" => Some(KernelEvent::Output {
            parent: with_parent(parent),
            output: json!({
                "output_type": "display_data",
                "data": content["data"],
                "metadata": content.get("metadata").cloned().unwrap_or_else(|| json!({})),
            }),
        }),
        "execute_result" => Some(KernelEvent::Output {
            parent: with_parent(parent),
            output: json!({
                "output_type": "execute_result",
                "execution_count": content["execution_count"],
                "data": content["data"],
                "metadata": content.get("metadata").cloned().unwrap_or_else(|| json!({})),
            }),
        }),
        "error" => Some(KernelEvent::Output {
            parent: with_parent(parent),
            output: json!({
                "output_type": "error",
                "ename": content["ename"],
                "evalue": content["evalue"],
                "traceback": content.get("traceback").cloned().unwrap_or_else(|| json!([])),
            }),
        }),
        "update_display_data" => Some(KernelEvent::UpdateDisplay {
            parent: with_parent(parent),
            display_id: content["transient"]["display_id"].as_str()?.to_owned(),
            output: json!({
                "output_type": "display_data",
                "data": content["data"],
                "metadata": content.get("metadata").cloned().unwrap_or_else(|| json!({})),
            }),
        }),
        "clear_output" => Some(KernelEvent::ClearOutput {
            parent: with_parent(parent),
            wait: content["wait"].as_bool().unwrap_or(false),
        }),
        "execute_reply" => Some(KernelEvent::Reply {
            parent: with_parent(parent),
            ok: content["status"].as_str() == Some("ok"),
            count: content["execution_count"].as_u64(),
        }),
        "kernel_info_reply" => Some(KernelEvent::Info {
            language: content["language_info"]["name"].as_str().map(str::to_owned),
            banner: content["banner"].as_str().map(str::to_owned),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"secret-key";

    #[test]
    fn a_message_survives_encoding_and_decoding_with_a_signature() {
        let message = Message::request("execute_request", "session-1", json!({"code": "1+1"}));
        let frames = encode(KEY, &[b"id1".to_vec()], &message);
        assert_eq!(frames[0], b"id1");
        assert_eq!(frames[1], DELIMITER);
        assert_eq!(frames[2].len(), 64);
        assert_eq!(decode(KEY, &frames).unwrap(), message);
        assert_eq!(message.msg_type(), "execute_request");
        assert_eq!(message.header["session"], "session-1");
        assert_eq!(message.header["version"], PROTOCOL_VERSION);
    }

    #[test]
    fn the_signature_matches_a_known_hmac_sha256() {
        // HMAC-SHA256("key", "The quick brown fox jumps over the lazy dog"), split into four parts.
        let digest = signature(b"key", [b"The quick ", b"brown fox ", b"jumps over ", b"the lazy dog"]);
        assert_eq!(
            digest,
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn tampering_or_a_wrong_key_is_detected() {
        let message = Message::request("status", "s", json!({"execution_state": "idle"}));
        let mut frames = encode(KEY, &[], &message);
        assert!(decode(b"another-key", &frames).unwrap_err().contains("signature"));
        *frames.last_mut().unwrap() = br#"{"execution_state":"busy"}"#.to_vec();
        assert!(decode(KEY, &frames).unwrap_err().contains("signature"));
    }

    #[test]
    fn an_empty_key_means_no_signing() {
        let message = Message::request("x", "s", json!({}));
        let frames = encode(b"", &[], &message);
        assert_eq!(frames[1], b"");
        assert_eq!(decode(b"", &frames).unwrap(), message);
    }

    #[test]
    fn malformed_messages_are_errors_not_panics() {
        assert!(decode(KEY, &[b"no delimiter".to_vec()]).is_err());
        assert!(decode(KEY, &[DELIMITER.to_vec(), b"sig".to_vec()]).unwrap_err().contains("incomplete"));
        let frames = vec![
            DELIMITER.to_vec(),
            Vec::new(),
            b"{".to_vec(),
            b"{}".to_vec(),
            b"{}".to_vec(),
            b"{}".to_vec(),
        ];
        assert!(decode(b"", &frames).unwrap_err().contains("JSON"));
    }

    #[test]
    fn identities_before_the_delimiter_are_ignored_when_reading() {
        let message = Message::request("status", "s", json!({}));
        let frames = encode(KEY, &[b"a".to_vec(), b"b".to_vec()], &message);
        assert_eq!(decode(KEY, &frames).unwrap(), message);
    }

    #[test]
    fn ids_are_unique_and_the_date_is_iso_8601() {
        assert_ne!(random_id(), random_id());
        assert_eq!(random_id().len(), 32);
        let date = timestamp();
        assert_eq!(date.len(), 24);
        assert!(date.starts_with("20") && date.ends_with('Z') && date.contains('T'));
    }

    fn event(msg_type: &str, content: Value) -> Option<KernelEvent> {
        let parent = Message::request("execute_request", "s", json!({}));
        event_from(&Message::reply_to(&parent, msg_type, content))
    }

    #[test]
    fn iopub_messages_become_notebook_outputs() {
        let Some(KernelEvent::Output { output, .. }) =
            event("stream", json!({"name": "stderr", "text": "warn\n"}))
        else {
            panic!("expected an output");
        };
        assert_eq!(output, json!({"output_type": "stream", "name": "stderr", "text": "warn\n"}));
        let Some(KernelEvent::Output { output, .. }) = event(
            "execute_result",
            json!({"execution_count": 3, "data": {"text/plain": "2"}, "metadata": {}}),
        ) else {
            panic!("expected an output");
        };
        assert_eq!(output["output_type"], "execute_result");
        assert_eq!(output["execution_count"], 3);
        let Some(KernelEvent::Output { output, .. }) = event(
            "error",
            json!({"ename": "NameError", "evalue": "x", "traceback": ["t"]}),
        ) else {
            panic!("expected an output");
        };
        assert_eq!(output["traceback"], json!(["t"]));
        let Some(KernelEvent::Output { output, .. }) = event("display_data", json!({"data": {"image/png": "AA"}})) else {
            panic!("expected an output");
        };
        assert_eq!(output["metadata"], json!({}));
    }

    #[test]
    fn status_replies_and_housekeeping_messages() {
        assert!(matches!(
            event("status", json!({"execution_state": "busy"})),
            Some(KernelEvent::Status { state: KernelState::Busy, parent: Some(_) })
        ));
        assert_eq!(event("status", json!({"execution_state": "mystery"})), None);
        assert!(matches!(
            event("execute_input", json!({"execution_count": 7, "code": "x"})),
            Some(KernelEvent::ExecuteInput { count: 7, .. })
        ));
        assert!(matches!(
            event("execute_reply", json!({"status": "error", "execution_count": 7})),
            Some(KernelEvent::Reply { ok: false, count: Some(7), .. })
        ));
        assert!(matches!(
            event("clear_output", json!({"wait": true})),
            Some(KernelEvent::ClearOutput { wait: true, .. })
        ));
        assert!(matches!(
            event("update_display_data", json!({"data": {}, "transient": {"display_id": "d1"}})),
            Some(KernelEvent::UpdateDisplay { .. })
        ));
        assert_eq!(event("update_display_data", json!({"data": {}})), None);
        assert!(matches!(
            event("kernel_info_reply", json!({"language_info": {"name": "python"}, "banner": "b"})),
            Some(KernelEvent::Info { language: Some(_), .. })
        ));
        assert_eq!(event("comm_open", json!({})), None);
    }
}
