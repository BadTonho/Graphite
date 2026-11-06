//! Syntax highlighting built on tree-sitter.

use std::{ops::Range, path::Path};
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter};

/// Semantic class of a highlighted span; the UI maps classes to colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Class {
    #[default]
    Default,
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Constant,
    Property,
    Operator,
    Punctuation,
    Variable,
}

/// Capture names we recognise, in the order handed to tree-sitter.
const CAPTURES: &[(&str, Class)] = &[
    ("keyword", Class::Keyword),
    ("function", Class::Function),
    ("function.method", Class::Function),
    ("function.macro", Class::Function),
    ("constructor", Class::Type),
    ("type", Class::Type),
    ("type.builtin", Class::Type),
    ("string", Class::String),
    ("string.special", Class::String),
    ("escape", Class::Constant),
    ("number", Class::Number),
    ("comment", Class::Comment),
    ("constant", Class::Constant),
    ("constant.builtin", Class::Constant),
    ("boolean", Class::Constant),
    ("property", Class::Property),
    ("attribute", Class::Property),
    ("label", Class::Property),
    ("operator", Class::Operator),
    ("punctuation", Class::Punctuation),
    ("punctuation.bracket", Class::Punctuation),
    ("punctuation.delimiter", Class::Punctuation),
    ("variable", Class::Variable),
    ("variable.builtin", Class::Constant),
    ("variable.parameter", Class::Variable),
    ("module", Class::Type),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    Json,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    fn ts_language(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Json => tree_sitter_json::LANGUAGE.into(),
        }
    }
    fn highlight_query(self) -> &'static str {
        match self {
            Self::Rust => tree_sitter_rust::HIGHLIGHTS_QUERY,
            Self::Python => tree_sitter_python::HIGHLIGHTS_QUERY,
            Self::JavaScript => tree_sitter_javascript::HIGHLIGHT_QUERY,
            Self::Json => tree_sitter_json::HIGHLIGHTS_QUERY,
        }
    }
}

/// A highlighted byte range within a single line (excluding the line terminator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub class: Class,
}

#[derive(Debug, Clone, Copy)]
pub struct TextEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_position: (usize, usize),
    pub old_end_position: (usize, usize),
    pub new_end_position: (usize, usize),
}

/// Stateful tree-sitter highlighting that reuses and edits the previous syntax tree.
pub struct IncrementalHighlighter {
    parser: Parser,
    language: Option<Language>,
    query: Option<Query>,
    tree: Option<Tree>,
    lines: Vec<Vec<Span>>,
}
impl Default for IncrementalHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
impl IncrementalHighlighter {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            language: None,
            query: None,
            tree: None,
            lines: Vec::new(),
        }
    }
    pub fn highlight(
        &mut self,
        language: Language,
        source: &str,
        edit: Option<TextEdit>,
    ) -> Vec<Vec<Span>> {
        let ts_language = language.ts_language();
        if self.language != Some(language) {
            if self.parser.set_language(&ts_language).is_err() {
                return Vec::new();
            }
            self.query = Query::new(&ts_language, language.highlight_query()).ok();
            self.language = Some(language);
            self.tree = None;
        }
        let mut invalidate_from = 0usize;
        if let (Some(tree), Some(edit)) = (&mut self.tree, edit) {
            let root = tree.root_node();
            let mut node = root
                .descendant_for_byte_range(
                    edit.start_byte,
                    edit.old_end_byte.max(edit.start_byte.saturating_add(1)),
                )
                .unwrap_or(root);
            while let Some(parent) = node.parent() {
                if parent.parent().is_none() {
                    break;
                }
                if parent.is_named() {
                    node = parent
                } else {
                    break;
                }
            }
            invalidate_from = node.start_byte();
            tree.edit(&InputEdit {
                start_byte: edit.start_byte,
                old_end_byte: edit.old_end_byte,
                new_end_byte: edit.new_end_byte,
                start_position: Point::new(edit.start_position.0, edit.start_position.1),
                old_end_position: Point::new(edit.old_end_position.0, edit.old_end_position.1),
                new_end_position: Point::new(edit.new_end_position.0, edit.new_end_position.1),
            });
        } else if self.tree.is_some() {
            self.tree = None;
            self.lines.clear();
        }
        if self.tree.is_none() {
            invalidate_from = 0;
            self.lines.clear();
        }
        if invalidate_from > 0 {
            invalidate_from = source.as_bytes()[..invalidate_from.min(source.len())]
                .iter()
                .rposition(|b| *b == b'\n')
                .map(|i| i + 1)
                .unwrap_or(0);
        }
        let base_row = source.as_bytes()[..invalidate_from.min(source.len())]
            .iter()
            .filter(|b| **b == b'\n')
            .count();
        self.lines.truncate(base_row);
        self.lines.resize(
            source.bytes().filter(|b| *b == b'\n').count() + 1,
            Vec::new(),
        );
        for row in self.lines.iter_mut().skip(base_row) {
            row.clear();
        }
        let Some(tree) = self.parser.parse(source, self.tree.as_ref()) else {
            return Vec::new();
        };
        self.tree = Some(tree);
        let Some(query) = &self.query else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(invalidate_from..source.len());
        let mut captures = cursor.captures(
            query,
            self.tree.as_ref().unwrap().root_node(),
            source.as_bytes(),
        );
        while let Some((query_match, capture_index)) = captures.next() {
            let capture = query_match.captures()[*capture_index];
            let name = query
                .capture_names()
                .get(capture.index as usize)
                .copied()
                .unwrap_or("");
            let Some(class) = CAPTURES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c) else {
                continue;
            };
            let start = capture.node.start_byte();
            let end = capture.node.end_byte();
            let mut line_start = 0;
            for (row, line) in source.split_inclusive('\n').enumerate() {
                let content_end = line_start + line.trim_end_matches(['\n', '\r']).len();
                let a = start.max(line_start);
                let b = end.min(content_end);
                if a < b && row >= base_row {
                    self.lines[row].push(Span {
                        range: a - line_start..b - line_start,
                        class,
                    });
                }
                line_start += line.len();
            }
        }
        for spans in &mut self.lines {
            spans.sort_by_key(|s| (s.range.start, s.range.end));
        }
        self.lines.clone()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_language_by_extension() {
        assert_eq!(
            Language::from_path(Path::new("a/b.rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(Path::new("x.PY")),
            Some(Language::Python)
        );
        assert_eq!(Language::from_path(Path::new("README")), None);
    }

    #[test]
    fn highlights_rust_keywords_and_strings() {
        let mut h = Highlighter::new();
        let lines = h.highlight(Language::Rust, "fn main() {\n    let s = \"hi\";\n}\n");
        assert_eq!(lines.len(), 4);
        assert!(lines[0]
            .iter()
            .any(|s| s.class == Class::Keyword && s.range == (0..2)));
        assert!(lines[1].iter().any(|s| s.class == Class::String));
    }

    #[test]
    fn multiline_spans_split_per_line() {
        let mut h = Highlighter::new();
        let lines = h.highlight(Language::Rust, "/* a\nb */\n");
        assert!(lines[0].iter().any(|s| s.class == Class::Comment));
        assert!(lines[1].iter().any(|s| s.class == Class::Comment));
    }

    #[test]
    fn incremental_parse_matches_full_highlight_after_edit() {
        let old = "fn main() {\n let value = 1;\n}\n";
        let new = "// note\nfn main() {\n let value = 1;\n}\n";
        let mut incremental = IncrementalHighlighter::new();
        let _ = incremental.highlight(Language::Rust, old, None);
        let changed = incremental.highlight(
            Language::Rust,
            new,
            Some(TextEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: 8,
                start_position: (0, 0),
                old_end_position: (0, 0),
                new_end_position: (1, 0),
            }),
        );
        let mut full = Highlighter::new();
        assert_eq!(changed, full.highlight(Language::Rust, new));
        let next = new.replace("value", "count");
        let offset = new.find("value").unwrap();
        let row = new[..offset].bytes().filter(|b| *b == b'\n').count();
        let column = offset - new[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let changed = incremental.highlight(
            Language::Rust,
            &next,
            Some(TextEdit {
                start_byte: offset,
                old_end_byte: offset + 5,
                new_end_byte: offset + 5,
                start_position: (row, column),
                old_end_position: (row, column + 5),
                new_end_position: (row, column + 5),
            }),
        );
        assert_eq!(changed, full.highlight(Language::Rust, &next));
    }

    #[test]
    fn incremental_edit_rehighlights_multiline_context_through_eof() {
        let old = "fn f() {\n    let a = 1;\n    let b = 2;\n}\n";
        let new = "fn f() {\n    /* let a = 1;\n    let b = 2;\n}\n";
        let start = old.find("let a").unwrap();
        let mut incremental = IncrementalHighlighter::new();
        let _ = incremental.highlight(Language::Rust, old, None);
        let changed = incremental.highlight(
            Language::Rust,
            new,
            Some(TextEdit {
                start_byte: start,
                old_end_byte: start,
                new_end_byte: start + 3,
                start_position: (1, 4),
                old_end_position: (1, 4),
                new_end_position: (1, 7),
            }),
        );
        let mut full = Highlighter::new();
        assert_eq!(changed, full.highlight(Language::Rust, new));
    }
}
