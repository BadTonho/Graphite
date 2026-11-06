//! Syntax highlighting built on tree-sitter.

use std::{ops::Range, path::Path};

mod full;
mod incremental;
pub use full::Highlighter;
pub use incremental::IncrementalHighlighter;

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

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn ts_language(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Json => tree_sitter_json::LANGUAGE.into(),
        }
    }
    pub(crate) fn highlight_query(self) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

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

    #[test]
    fn rope_chunks_and_unicode_edits_match_full_reparse() {
        let mut rope = Rope::from_str(&"let value = \"🌎\";\r\n".repeat(2_000));
        let mut incremental = IncrementalHighlighter::new();
        incremental.highlight_rope(Language::Rust, &rope, None);
        let char_offset = rope.line_to_char(1_000);
        let byte_offset = rope.char_to_byte(char_offset);
        rope.insert(char_offset, "/* é */ ");
        let changed = incremental.highlight_rope(
            Language::Rust,
            &rope,
            Some(TextEdit {
                start_byte: byte_offset,
                old_end_byte: byte_offset,
                new_end_byte: byte_offset + "/* é */ ".len(),
                start_position: (1_000, 0),
                old_end_position: (1_000, 0),
                new_end_position: (1_000, "/* é */ ".len()),
            }),
        );
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(changed, fresh.highlight_rope(Language::Rust, &rope, None));
    }
}
