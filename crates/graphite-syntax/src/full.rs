use super::*;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter};

pub struct Highlighter {
    inner: TsHighlighter,
    configs: Vec<HighlightConfiguration>,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    pub fn new() -> Self {
        let names: Vec<&str> = CAPTURES.iter().map(|(n, _)| *n).collect();
        let make = |lang: tree_sitter::Language, name: &str, query: &str| {
            let mut cfg = HighlightConfiguration::new(lang, name, query, "", "")
                .expect("bundled highlight query must be valid");
            cfg.configure(&names);
            cfg
        };
        // Order must match `Language::index` (declaration order of the enum).
        let configs = vec![
            make(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                tree_sitter_rust::HIGHLIGHTS_QUERY,
            ),
            make(
                tree_sitter_python::LANGUAGE.into(),
                "python",
                tree_sitter_python::HIGHLIGHTS_QUERY,
            ),
            make(
                tree_sitter_javascript::LANGUAGE.into(),
                "javascript",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            ),
            make(
                tree_sitter_json::LANGUAGE.into(),
                "json",
                tree_sitter_json::HIGHLIGHTS_QUERY,
            ),
        ];
        Self {
            inner: TsHighlighter::new(),
            configs,
        }
    }

    /// Highlights `source` and returns spans grouped per line.
    /// Unhighlighted text is simply absent from the result.
    pub fn highlight(&mut self, lang: Language, source: &str) -> Vec<Vec<Span>> {
        let mut lines: Vec<Vec<Span>> = Vec::new();
        let mut line_starts = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        lines.resize(line_starts.len(), Vec::new());

        let config = &self.configs[lang.index()];
        let Ok(events) = self
            .inner
            .highlight(config, source.as_bytes(), None, None, |_| None)
        else {
            return lines;
        };

        let mut stack: Vec<Class> = Vec::new();
        for event in events {
            let Ok(event) = event else { break };
            match event {
                HighlightEvent::HighlightStart(h) => stack.push(CAPTURES[h.0].1),
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
                HighlightEvent::Source { start, end } => {
                    let Some(&class) = stack.last() else { continue };
                    let mut pos = start;
                    while pos < end {
                        let line = line_starts.partition_point(|&s| s <= pos) - 1;
                        let line_end = line_starts.get(line + 1).copied().unwrap_or(source.len());
                        let seg_end = end.min(line_end);
                        // Drop the line terminator from the span.
                        let text_end = if seg_end == line_end
                            && seg_end > line_starts[line]
                            && source.as_bytes()[seg_end - 1] == b'\n'
                        {
                            seg_end - 1
                        } else {
                            seg_end
                        };
                        if text_end > pos {
                            lines[line].push(Span {
                                range: pos - line_starts[line]..text_end - line_starts[line],
                                class,
                            });
                        }
                        pos = seg_end;
                    }
                }
            }
        }
        lines
    }
}
