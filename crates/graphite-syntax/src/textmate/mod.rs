//! A TextMate grammar engine: highlights the languages extensions contribute grammars for, the
//! ones the built-in tree-sitter grammars do not cover.
//!
//! * [`Registry`] loads grammars (`.tmLanguage.json` or plist) and the grammars they include.
//! * [`Registry::tokenize_line`] tokenizes one line from the state the previous line ended in.
//! * [`DocumentHighlighter`] keeps those states per line, so after an edit only the lines up to
//!   where the state matches again are tokenized.
//! * [`class_for_scopes`] maps scopes onto the classes themes color.

mod grammar;
mod scopes;
mod tokenizer;

pub use grammar::{parse_grammar_text, Registry};
pub use scopes::class_for_scopes;
pub use tokenizer::State;

use crate::Span;

/// The highlighting of one document, kept between edits.
#[derive(Default)]
pub struct DocumentHighlighter {
    scope: String,
    lines: Vec<String>,
    /// The state at the end of each line.
    states: Vec<State>,
    spans: Vec<Vec<Span>>,
}

impl DocumentHighlighter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Highlights `text` with the grammar `base` of `registry`; `scope` names it, so a different
    /// grammar starts over. Returns the spans of every line.
    pub fn highlight(&mut self, registry: &Registry, base: usize, text: &str) -> Vec<Vec<Span>> {
        let scope = registry.scope_name(base);
        let lines: Vec<String> = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
            .collect();
        if self.scope != scope {
            self.scope = scope.to_owned();
            self.lines.clear();
            self.states.clear();
            self.spans.clear();
        }
        // The lines that did not change at either end keep their spans, as long as the state
        // they start in is the one they were tokenized in.
        let prefix = self
            .lines
            .iter()
            .zip(&lines)
            .take_while(|(old, new)| old == new)
            .count();
        let suffix = self
            .lines
            .iter()
            .rev()
            .zip(lines.iter().rev())
            .take(self.lines.len().min(lines.len()) - prefix)
            .take_while(|(old, new)| old == new)
            .count();
        let old_len = self.lines.len();
        let mut states: Vec<State> = self.states[..prefix.min(self.states.len())].to_vec();
        let mut spans: Vec<Vec<Span>> = self.spans[..prefix.min(self.spans.len())].to_vec();
        let mut index = prefix;
        while index < lines.len() {
            let start = if index == 0 {
                State::default()
            } else {
                states[index - 1].clone()
            };
            let (line_spans, end) = registry.tokenize_line(base, &lines[index], &start, index == 0);
            // In the unchanged tail, a line that ends in the state it ended in before ends the
            // difference: the rest is as it was.
            let from_end = lines.len() - 1 - index;
            if from_end < suffix {
                let old_index = old_len - 1 - from_end;
                if self.states.get(old_index) == Some(&end) {
                    spans.push(line_spans);
                    states.push(end);
                    for rest in index + 1..lines.len() {
                        let old_rest = old_len - (lines.len() - rest);
                        spans.push(self.spans[old_rest].clone());
                        states.push(self.states[old_rest].clone());
                    }
                    break;
                }
            }
            spans.push(line_spans);
            states.push(end);
            index += 1;
        }
        self.lines = lines;
        self.states = states;
        self.spans = spans;
        self.spans.clone()
    }
}

#[cfg(test)]
mod tests;
