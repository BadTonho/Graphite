//! Incremental parser for `text/event-stream` bodies.

/// One server-sent event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// The largest line or event the parser holds. A server that sends more without ending it is
/// refused instead of filling memory.
pub const MAX_SSE_EVENT_BYTES: usize = 8 * 1024 * 1024;

/// A line or event exceeded [`MAX_SSE_EVENT_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseOverflow;

impl std::fmt::Display for SseOverflow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("server-sent event exceeds the size limit")
    }
}

impl std::error::Error for SseOverflow {}

/// Collects bytes as they arrive and yields whole events. Handles chunk boundaries anywhere,
/// LF and CRLF line ends, comments and multi-line `data` fields.
#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    /// How much of `buffer` is known to hold no line end, so it is not searched again.
    scanned: usize,
    current: SseEvent,
    has_data: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, SseOverflow> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        let mut consumed = 0;
        while let Some(offset) = self.buffer[self.scanned..]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            let end = self.scanned + offset;
            let mut line = &self.buffer[consumed..end];
            if line.last() == Some(&b'\r') {
                line = &line[..line.len() - 1];
            }
            let line = String::from_utf8_lossy(line).into_owned();
            consumed = end + 1;
            self.scanned = consumed;
            self.line(&line, &mut events)?;
        }
        self.buffer.drain(..consumed);
        self.scanned = self.buffer.len();
        if self.buffer.len() > MAX_SSE_EVENT_BYTES {
            return Err(SseOverflow);
        }
        Ok(events)
    }

    /// Flushes an event that the stream ended in the middle of.
    pub fn finish(&mut self) -> Result<Vec<SseEvent>, SseOverflow> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = String::from_utf8_lossy(&std::mem::take(&mut self.buffer)).into_owned();
            self.scanned = 0;
            self.line(line.trim_end_matches('\r'), &mut events)?;
        }
        self.dispatch(&mut events);
        Ok(events)
    }

    fn line(&mut self, line: &str, events: &mut Vec<SseEvent>) -> Result<(), SseOverflow> {
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
                    if self.current.data.len() + value.len() + 1 > MAX_SSE_EVENT_BYTES {
                        return Err(SseOverflow);
                    }
                    if self.has_data {
                        self.current.data.push('\n');
                    }
                    self.current.data.push_str(value);
                    self.has_data = true;
                }
                _ => {}
            }
        }
        Ok(())
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
        assert!(parser.push(b"event: ping\nda").unwrap().is_empty());
        let events = parser
            .push(b"ta: {\"a\":1}\n\nevent: x\ndata: 2\n\n")
            .unwrap();
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
        let events = parser
            .push(b": keep-alive\r\ndata: one\r\ndata: two\r\n\r\n")
            .unwrap();
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
        assert!(parser.push(b"data: tail").unwrap().is_empty());
        assert_eq!(
            parser.finish().unwrap(),
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
        let mut events = parser.push(&bytes[..8]).unwrap();
        events.extend(parser.push(&bytes[8..]).unwrap());
        assert_eq!(events[0].data, "a\u{e7}\u{e3}o");
    }

    #[test]
    fn a_line_that_never_ends_is_refused_once_it_exceeds_the_limit() {
        let mut parser = SseParser::new();
        let chunk = vec![b'x'; 1024 * 1024];
        let mut result = Ok(Vec::new());
        for _ in 0..=MAX_SSE_EVENT_BYTES / chunk.len() {
            result = parser.push(&chunk);
            if result.is_err() {
                break;
            }
        }
        assert_eq!(result, Err(SseOverflow));
    }

    #[test]
    fn an_event_whose_data_lines_exceed_the_limit_is_refused() {
        let mut parser = SseParser::new();
        let line = format!("data: {}\n", "y".repeat(1024 * 1024));
        let mut result = Ok(Vec::new());
        for _ in 0..=MAX_SSE_EVENT_BYTES / (1024 * 1024) {
            result = parser.push(line.as_bytes());
            if result.is_err() {
                break;
            }
        }
        assert_eq!(result, Err(SseOverflow));
    }

    #[test]
    fn a_line_arriving_one_byte_at_a_time_is_parsed_once() {
        let mut parser = SseParser::new();
        let mut events = Vec::new();
        for byte in b"data: slow\n\n" {
            events.extend(parser.push(std::slice::from_ref(byte)).unwrap());
        }
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "slow");
    }
}
