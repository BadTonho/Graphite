//! Cell outputs as text lines.

use serde_json::Value;

/// A text field of an output: a string or an array of strings.
pub(crate) fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => Some(parts.iter().filter_map(Value::as_str).collect()),
        _ => None,
    }
}

/// Removes ANSI escape sequences (colors and cursor movement) from terminal text.
pub(crate) fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                // A control sequence ends with a byte from `@` to `~`.
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                // An operating system command ends with BEL or ESC \.
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        chars.next();
                        break;
                    }
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// The text of one output.
fn output_text(output: &Value) -> String {
    match output.get("output_type").and_then(Value::as_str) {
        Some("stream") => output.get("text").and_then(text_of).unwrap_or_default(),
        Some("execute_result") | Some("display_data") => output
            .get("data")
            .and_then(|data| data.get("text/plain"))
            .and_then(text_of)
            .unwrap_or_default(),
        Some("error") => {
            let traceback = output
                .get("traceback")
                .and_then(Value::as_array)
                .map(|lines| {
                    lines
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            if traceback.is_empty() {
                let name = output.get("ename").and_then(Value::as_str).unwrap_or("");
                let value = output.get("evalue").and_then(Value::as_str).unwrap_or("");
                format!("{name}: {value}")
            } else {
                traceback
            }
        }
        _ => String::new(),
    }
}

/// All outputs of a cell as display lines, without ANSI sequences.
pub(crate) fn output_lines(outputs: &[Value]) -> Vec<String> {
    let mut lines = Vec::new();
    for output in outputs {
        let text = strip_ansi(&output_text(output));
        let text = text.replace("\r\n", "\n");
        // A carriage return rewrites the line (progress bars): the last write wins.
        lines.extend(
            text.trim_end_matches('\n')
                .split('\n')
                .map(|line| line.rsplit('\r').next().unwrap_or("").to_owned()),
        );
        if text.is_empty() {
            // Nothing to show for this output; drop the empty line the split produced.
            lines.pop();
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stream_results_and_errors_become_lines() {
        let lines = output_lines(&[
            json!({"output_type": "stream", "name": "stdout", "text": ["a\n", "b\n"]}),
            json!({"output_type": "execute_result", "data": {"text/plain": "42"}, "execution_count": 1, "metadata": {}}),
            json!({"output_type": "error", "ename": "ValueError", "evalue": "bad", "traceback": ["Trace\n", "  line 2"]}),
        ]);
        assert_eq!(lines, ["a", "b", "42", "Trace", "", "  line 2"]);
    }

    #[test]
    fn an_error_without_a_traceback_shows_its_name_and_value() {
        let lines =
            output_lines(&[json!({"output_type": "error", "ename": "KeyError", "evalue": "'x'"})]);
        assert_eq!(lines, ["KeyError: 'x'"]);
    }

    #[test]
    fn ansi_sequences_are_removed() {
        assert_eq!(strip_ansi("\u{1b}[31mred\u{1b}[0m plain"), "red plain");
        assert_eq!(strip_ansi("\u{1b}[1;32;40mx\u{1b}[K"), "x");
        assert_eq!(strip_ansi("\u{1b}]0;title\u{7}after"), "after");
        assert_eq!(strip_ansi("trailing\u{1b}"), "trailing");
        assert_eq!(strip_ansi("no escapes é🌎"), "no escapes é🌎");
    }

    #[test]
    fn carriage_returns_keep_the_last_write_of_a_line() {
        let lines = output_lines(&[
            json!({"output_type": "stream", "name": "stderr", "text": "10%\r50%\r100%\ndone\n"}),
        ]);
        assert_eq!(lines, ["100%", "done"]);
    }

    #[test]
    fn outputs_without_text_add_no_lines() {
        let lines = output_lines(&[
            json!({"output_type": "display_data", "data": {"image/png": "AAAA"}, "metadata": {}}),
            json!({"output_type": "stream", "name": "stdout", "text": "kept\n"}),
            json!({"output_type": "unknown"}),
        ]);
        assert_eq!(lines, ["kept"]);
    }

    #[test]
    fn text_fields_accept_strings_and_arrays() {
        assert_eq!(text_of(&json!("x")).as_deref(), Some("x"));
        assert_eq!(text_of(&json!(["a", "b"])).as_deref(), Some("ab"));
        assert_eq!(text_of(&json!(5)), None);
    }
}
