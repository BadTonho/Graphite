//! Incremental parser for `text/event-stream` bodies.

/// One server-sent event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Collects bytes as they arrive and yields whole events. Handles chunk boundaries anywhere,
/// LF and CRLF line ends, comments and multi-line `data` fields.
#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    current: SseEvent,
    has_data: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line).into_owned();
            self.line(&line, &mut events);
        }
        events
    }

    /// Flushes an event that the stream ended in the middle of.
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = String::from_utf8_lossy(&std::mem::take(&mut self.buffer)).into_owned();
            self.line(line.trim_end_matches('\r'), &mut events);
        }
        self.dispatch(&mut events);
        events
    }

    fn line(&mut self, line: &str, events: &mut Vec<SseEvent>) {
        if line.is_empty() {
            self.dispatch(events);
        } else if line.starts_with(':') {
            // A comment, used by servers as a keep-alive.
        } else {
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "event" => self.current.event = Some(value.to_string()),
                "data" => {
                    if self.has_data {
                        self.current.data.push('\n');
                    }
                    self.current.data.push_str(value);
                    self.has_data = true;
                }
                _ => {}
            }
        }
    }

    fn dispatch(&mut self, events: &mut Vec<SseEvent>) {
        if self.has_data || self.current.event.is_some() {
            events.push(std::mem::take(&mut self.current));
        }
        self.current = SseEvent::default();
        self.has_data = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_split_across_chunks_arrive_whole() {
        let mut parser = SseParser::new();
        assert!(parser.push(b"event: ping\nda").is_empty());
        let events = parser.push(b"ta: {\"a\":1}\n\nevent: x\ndata: 2\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            SseEvent {
                event: Some("ping".into()),
                data: "{\"a\":1}".into()
            }
        );
        assert_eq!(events[1].data, "2");
    }

    #[test]
    fn crlf_comments_and_multiline_data_are_handled() {
        let mut parser = SseParser::new();
        let events = parser.push(b": keep-alive\r\ndata: one\r\ndata: two\r\n\r\n");
        assert_eq!(
            events,
            vec![SseEvent {
                event: None,
                data: "one\ntwo".into()
            }]
        );
    }

    #[test]
    fn a_stream_cut_mid_event_is_flushed_on_finish() {
        let mut parser = SseParser::new();
        assert!(parser.push(b"data: tail").is_empty());
        assert_eq!(
            parser.finish(),
            vec![SseEvent {
                event: None,
                data: "tail".into()
            }]
        );
    }

    #[test]
    fn multibyte_text_split_between_chunks_survives() {
        let bytes = "data: a\u{e7}\u{e3}o\n\n".as_bytes();
        let mut parser = SseParser::new();
        let mut events = parser.push(&bytes[..8]);
        events.extend(parser.push(&bytes[8..]));
        assert_eq!(events[0].data, "a\u{e7}\u{e3}o");
    }
}
