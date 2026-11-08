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
            "json" | "jsonc" => Some(Self::Json),
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

/// A declaration that encloses a source line and can be shown in Sticky Scroll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeHeader {
    pub line: usize,
    pub text: String,
}

/// Return all supported declarations in source order for an Outline view.
pub fn declaration_headers(language: Language, source: &str) -> Vec<ScopeHeader> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&language.ts_language()).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut nodes = Vec::new();
    collect_all_scope_nodes(tree.root_node(), &mut nodes);
    nodes.sort_by_key(|node| (node.start_position().row, node.start_position().column));
    let lines: Vec<_> = source.lines().collect();
    nodes
        .into_iter()
        .filter_map(|node| {
            let row = node.start_position().row;
            lines.get(row).map(|text| ScopeHeader {
                line: row,
                text: text.trim().to_owned(),
            })
        })
        .collect()
}

/// Return enclosing declaration headers from outermost to innermost.
pub fn enclosing_scope_headers(language: Language, source: &str, line: usize) -> Vec<ScopeHeader> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&language.ts_language()).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut nodes = Vec::new();
    collect_scope_nodes(tree.root_node(), line, &mut nodes);
    nodes.sort_by_key(|node| node.start_position().row);
    let lines: Vec<_> = source.lines().collect();
    nodes
        .into_iter()
        .filter_map(|node| {
            let row = node.start_position().row;
            lines.get(row).map(|text| ScopeHeader {
                line: row,
                text: text.trim().to_owned(),
            })
        })
        .collect()
}

fn collect_scope_nodes<'tree>(
    node: tree_sitter::Node<'tree>,
    line: usize,
    output: &mut Vec<tree_sitter::Node<'tree>>,
) {
    if node.start_position().row > line || node.end_position().row < line {
        return;
    }
    if matches!(
        node.kind(),
        "function_item"
            | "function_definition"
            | "function_declaration"
            | "method_definition"
            | "class_definition"
            | "class_declaration"
            | "struct_item"
            | "enum_item"
            | "impl_item"
            | "trait_item"
            | "mod_item"
    ) {
        output.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_scope_nodes(child, line, output);
    }
}

fn collect_all_scope_nodes<'tree>(
    node: tree_sitter::Node<'tree>,
    output: &mut Vec<tree_sitter::Node<'tree>>,
) {
    if matches!(
        node.kind(),
        "function_item"
            | "function_definition"
            | "function_declaration"
            | "method_definition"
            | "class_definition"
            | "class_declaration"
            | "struct_item"
            | "enum_item"
            | "impl_item"
            | "trait_item"
            | "mod_item"
    ) {
        output.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_all_scope_nodes(child, output);
    }
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
    fn sticky_scroll_returns_nested_declarations_containing_the_line() {
        let source =
            "struct Store;\nimpl Store {\n    fn load(&self) {\n        let x = 1;\n    }\n}\n";
        let headers = enclosing_scope_headers(Language::Rust, source, 3);
        assert_eq!(headers.iter().map(|h| h.line).collect::<Vec<_>>(), [1, 2]);
        assert!(headers[0].text.contains("impl Store"));
        assert!(headers[1].text.contains("fn load"));
    }

    #[test]
    fn sticky_scroll_is_empty_outside_supported_declarations() {
        assert!(enclosing_scope_headers(Language::Json, "{\n  \"x\": 1\n}\n", 1).is_empty());
    }

    #[test]
    fn outline_lists_all_declarations_in_source_order() {
        let source = "struct Store;\nimpl Store {\n    fn load(&self) {}\n}\nfn main() {}\n";
        let headers = declaration_headers(Language::Rust, source);
        assert_eq!(
            headers.iter().map(|header| header.line).collect::<Vec<_>>(),
            [0, 1, 2, 4]
        );
        assert!(headers[2].text.contains("fn load"));
        assert!(headers[3].text.contains("fn main"));
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

    #[test]
    fn incremental_deletion_across_lines_matches_full_reparse() {
        let old = "fn main() {\n    /* 🌎\n       comment */\n    let x = 1;\n}\n";
        let start = old.find("/*").unwrap();
        let end = old.find("let x").unwrap();
        let new = format!("{}{}", &old[..start], &old[end..]);
        let mut incremental = IncrementalHighlighter::new();
        incremental.highlight(Language::Rust, old, None);
        let actual = incremental.highlight(
            Language::Rust,
            &new,
            Some(TextEdit {
                start_byte: start,
                old_end_byte: end,
                new_end_byte: start,
                start_position: (1, 4),
                old_end_position: (3, 4),
                new_end_position: (1, 4),
            }),
        );
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(actual, fresh.highlight(Language::Rust, &new, None));
    }
}
