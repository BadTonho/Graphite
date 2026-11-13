//! Inline values: while the program is paused, the value of each local variable is shown at the
//! end of the lines of the stopped function that mention it, like VS Code's default behavior when
//! the adapter provides no inline-value information.
//!
//! The lines come from the stopped source file if it is open. A variable counts as mentioned when
//! its name appears as a whole identifier, so `count` is not found inside `counter`.

use super::*;
use std::collections::BTreeMap;

/// How many lines above the stop line are scanned.
const WINDOW: usize = 60;
/// Longest value shown, in characters.
const VALUE_LIMIT: usize = 40;
/// Most variables shown on one line.
const PER_LINE: usize = 4;

/// The inline values computed for one open document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InlineValues {
    pub uri: String,
    pub revision: u64,
    /// 0-based line to the text drawn after it.
    pub lines: BTreeMap<usize, String>,
    /// What the values were computed from; they are recomputed when it changes.
    source: (u64, usize, Option<usize>),
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

/// The identifiers of `line` in order of appearance.
fn identifiers(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = None;
    for (index, ch) in line.char_indices() {
        match (is_identifier_char(ch), start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                found.push(&line[from..index]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        found.push(&line[from..]);
    }
    found
}

fn shorten(value: &str) -> String {
    let single_line: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() > VALUE_LIMIT {
        let kept: String = single_line.chars().take(VALUE_LIMIT).collect();
        format!("{kept}…")
    } else {
        single_line
    }
}

/// Text to draw after the lines `stop_line - WINDOW..=stop_line` (0-based) that mention a
/// variable: `name = value` for up to [`PER_LINE`] variables, each once per line.
pub(crate) fn inline_values(
    lines: &[String],
    stop_line: usize,
    variables: &[(String, String)],
) -> BTreeMap<usize, String> {
    let mut result = BTreeMap::new();
    if lines.is_empty() || variables.is_empty() {
        return result;
    }
    let last = stop_line.min(lines.len() - 1);
    let first = last.saturating_sub(WINDOW);
    for (index, line) in lines.iter().enumerate().take(last + 1).skip(first) {
        let mut shown: Vec<&str> = Vec::new();
        let mut parts = Vec::new();
        for word in identifiers(line) {
            if shown.contains(&word) {
                continue;
            }
            if let Some((name, value)) = variables.iter().find(|(name, _)| name == word) {
                shown.push(word);
                parts.push(format!("{name} = {}", shorten(value)));
                if parts.len() == PER_LINE {
                    break;
                }
            }
        }
        if !parts.is_empty() {
            result.insert(index, parts.join(", "));
        }
    }
    result
}

impl App {
    /// Recomputes the inline values if the paused state, the document or the stop changed.
    pub(crate) fn refresh_debug_inline_values(&mut self) {
        let frame = self
            .debug
            .frames
            .get(self.debug.selected_frame)
            .filter(|_| matches!(self.debug.state, crate::debugger::DebugState::Paused { .. }));
        let Some(frame) = frame else {
            self.debug_inline = None;
            return;
        };
        let (Some(path), line) = (frame.source.clone(), frame.line as usize) else {
            self.debug_inline = None;
            return;
        };
        let Some(id) = self.documents.id_for_path(&path) else {
            self.debug_inline = None;
            return;
        };
        let revision = self.documents[id].buffer.revision();
        let source = (self.debug.version, line, Some(revision as usize));
        if self
            .debug_inline
            .as_ref()
            .is_some_and(|current| current.source == source)
        {
            return;
        }
        let Ok(uri) = graphite_core_api::DocumentUri::from_path(&path) else {
            self.debug_inline = None;
            return;
        };
        let buffer = &self.documents[id].buffer;
        let text: Vec<String> = (0..buffer.len_lines())
            .map(|line| buffer.line(line).trim_end_matches(['\r', '\n']).to_owned())
            .collect();
        let stop_line = line.saturating_sub(1);
        self.debug_inline = Some(InlineValues {
            uri: uri.as_str().to_owned(),
            revision,
            lines: inline_values(&text, stop_line, &self.debug.local_variables()),
            source,
        });
    }

    /// The text to draw after `line` of the document with this URI, if the values are current.
    pub(crate) fn debug_inline_value(&self, uri: &str, revision: u64, line: usize) -> Option<&str> {
        self.debug_inline
            .as_ref()
            .filter(|inline| inline.uri == uri && inline.revision == revision)
            .and_then(|inline| inline.lines.get(&line))
            .map(String::as_str)
    }

    pub(crate) fn draw_debug_inline_value(
        &self,
        r: &mut Renderer,
        uri: &str,
        revision: u64,
        line: usize,
        text: &str,
        tab_size: usize,
        geometry: (f32, f32, f32),
    ) {
        let Some(value) = self.debug_inline_value(uri, revision, line) else {
            return;
        };
        let (text_x, y, cw) = geometry;
        let shown = super::drawing_helpers::display_line(text, tab_size)
            .chars()
            .count();
        r.draw_text(
            value,
            text_x + (shown + 2) as f32 * cw,
            y,
            rgb(Theme::color(0x9a9a6a, 0x7a7a3a)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::{DebugState, StackFrame};

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    fn source(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| line.to_string()).collect()
    }

    #[test]
    fn identifiers_are_whole_words() {
        assert_eq!(
            identifiers("let counter = count + 1;"),
            ["let", "counter", "count", "1"]
        );
        assert_eq!(identifiers("a_b.c$d(é)"), ["a_b", "c$d", "é"]);
        assert!(identifiers("  ;; ").is_empty());
    }

    #[test]
    fn values_appear_on_lines_that_mention_a_variable() {
        let lines = source(&[
            "fn main() {",
            "    let x = 1;",
            "    let y = x + z;",
            "    done();",
        ]);
        let shown = inline_values(&lines, 3, &vars(&[("x", "1"), ("y", "2"), ("w", "9")]));
        assert_eq!(shown.get(&1).map(String::as_str), Some("x = 1"));
        assert_eq!(shown.get(&2).map(String::as_str), Some("y = 2, x = 1"));
        assert!(!shown.contains_key(&0) && !shown.contains_key(&3));
        assert!(
            shown.values().all(|text| !text.contains("w = ")),
            "unused variables stay hidden"
        );
    }

    #[test]
    fn a_name_inside_a_longer_identifier_does_not_match() {
        let lines = source(&["let counter = 1;"]);
        assert!(inline_values(&lines, 0, &vars(&[("count", "5")])).is_empty());
        assert!(inline_values(&lines, 0, &vars(&[("counter", "5")])).contains_key(&0));
    }

    #[test]
    fn only_the_function_before_the_stop_is_annotated() {
        let lines: Vec<String> = (0..200).map(|_| "x".to_owned()).collect();
        let shown = inline_values(&lines, 100, &vars(&[("x", "1")]));
        assert_eq!(
            *shown.keys().next().unwrap(),
            40,
            "window of 60 lines above the stop"
        );
        assert_eq!(
            *shown.keys().last().unwrap(),
            100,
            "nothing after the stop line"
        );
        assert_eq!(shown.len(), 61);
    }

    #[test]
    fn a_stop_line_past_the_end_is_clamped() {
        let lines = source(&["x", "x"]);
        assert_eq!(inline_values(&lines, 99, &vars(&[("x", "1")])).len(), 2);
        assert!(inline_values(&[], 0, &vars(&[("x", "1")])).is_empty());
        assert!(inline_values(&lines, 0, &[]).is_empty());
    }

    #[test]
    fn long_and_multiline_values_are_shortened_and_each_variable_shows_once() {
        let long = "word ".repeat(30);
        let lines = source(&["a a a b"]);
        let shown = inline_values(&lines, 0, &vars(&[("a", "line1\n  line2"), ("b", &long)]));
        let text = &shown[&0];
        assert!(text.starts_with("a = line1 line2, b = word word"), "{text}");
        assert_eq!(text.matches("a = ").count(), 1);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn at_most_four_variables_are_shown_per_line() {
        let lines = source(&["a b c d e f"]);
        let all = vars(&[
            ("a", "1"),
            ("b", "2"),
            ("c", "3"),
            ("d", "4"),
            ("e", "5"),
            ("f", "6"),
        ]);
        assert_eq!(
            inline_values(&lines, 0, &all)[&0],
            "a = 1, b = 2, c = 3, d = 4"
        );
    }

    fn paused_app(dir: &crate::test_support::TempDir) -> (App, std::path::PathBuf) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let file = dir.path().join("main.txt");
        std::fs::write(&file, "start\nlet total = 5;\nreturn total;\n").unwrap();
        app.open_file(file.clone());
        app.debug.state = DebugState::Paused {
            thread_id: 1,
            reason: "breakpoint".into(),
        };
        app.debug.frames = vec![StackFrame {
            id: 1,
            name: "main".into(),
            source: Some(
                app.documents[app.active_document()]
                    .buffer
                    .path()
                    .unwrap()
                    .to_path_buf(),
            ),
            line: 3,
        }];
        let path = app.documents[app.active_document()]
            .buffer
            .path()
            .unwrap()
            .to_path_buf();
        (app, path)
    }

    #[test]
    fn nothing_is_shown_unless_paused_in_an_open_file() {
        let dir = crate::test_support::TempDir::new();
        let (mut app, _) = paused_app(&dir);
        app.debug.state = DebugState::Running;
        app.refresh_debug_inline_values();
        assert!(app.debug_inline.is_none());

        app.debug.state = DebugState::Paused {
            thread_id: 1,
            reason: "step".into(),
        };
        app.debug.frames[0].source = Some(dir.path().join("not-open.txt"));
        app.refresh_debug_inline_values();
        assert!(app.debug_inline.is_none(), "the stopped file is not open");

        app.debug.frames[0].source = None;
        app.refresh_debug_inline_values();
        assert!(app.debug_inline.is_none(), "a frame without a source");
    }

    #[test]
    fn values_are_computed_for_the_stopped_document_and_dropped_when_it_changes() {
        let dir = crate::test_support::TempDir::new();
        let (mut app, path) = paused_app(&dir);
        app.debug.set_locals_for_test(&[("total", "5")]);
        app.refresh_debug_inline_values();
        let uri = graphite_core_api::DocumentUri::from_path(&path).unwrap();
        let revision = app.documents[app.active_document()].buffer.revision();
        assert_eq!(
            app.debug_inline_value(uri.as_str(), revision, 1),
            Some("total = 5")
        );
        assert_eq!(
            app.debug_inline_value(uri.as_str(), revision, 2),
            Some("total = 5")
        );
        assert_eq!(app.debug_inline_value(uri.as_str(), revision, 0), None);
        assert_eq!(app.debug_inline_value("file:///other", revision, 1), None);
        assert_eq!(
            app.debug_inline_value(uri.as_str(), revision + 1, 1),
            None,
            "values for an older revision are not drawn"
        );
    }

    #[test]
    fn a_changed_value_or_stop_recomputes_and_an_unchanged_state_is_reused() {
        let dir = crate::test_support::TempDir::new();
        let (mut app, path) = paused_app(&dir);
        app.debug.set_locals_for_test(&[("total", "5")]);
        app.refresh_debug_inline_values();
        let first = app.debug_inline.clone();
        app.refresh_debug_inline_values();
        assert_eq!(app.debug_inline, first, "same state, same values");

        app.debug.set_locals_for_test(&[("total", "7")]);
        app.refresh_debug_inline_values();
        let uri = graphite_core_api::DocumentUri::from_path(&path).unwrap();
        let revision = app.documents[app.active_document()].buffer.revision();
        assert_eq!(
            app.debug_inline_value(uri.as_str(), revision, 1),
            Some("total = 7")
        );

        app.debug.frames[0].line = 2;
        app.refresh_debug_inline_values();
        assert_eq!(
            app.debug_inline_value(uri.as_str(), revision, 2),
            None,
            "lines after the new stop line are not annotated"
        );
        assert_eq!(
            app.debug_inline_value(uri.as_str(), revision, 1),
            Some("total = 7")
        );
    }
}
