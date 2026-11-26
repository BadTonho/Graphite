//! Syntax highlighting built on tree-sitter.

use std::{ops::Range, path::Path};

mod full;
mod incremental;
pub mod textmate;
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
    BracketLevel1,
    BracketLevel2,
    BracketLevel3,
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
    ("tag", Class::Type),
    ("punctuation.special", Class::Punctuation),
    ("keyword.directive", Class::Keyword),
    ("string.special.key", Class::Property),
    ("markup.heading", Class::Keyword),
    ("markup.heading.1", Class::Keyword),
    ("markup.heading.2", Class::Keyword),
    ("markup.heading.3", Class::Keyword),
    ("markup.bold", Class::Constant),
    ("markup.strong", Class::Constant),
    ("markup.italic", Class::String),
    ("markup.raw", Class::String),
    ("markup.link", Class::Function),
    ("markup.list", Class::Operator),
    ("embedded", Class::Default),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    Json,
    TypeScript,
    Tsx,
    Html,
    Css,
    C,
    Cpp,
    Go,
    Toml,
    Yaml,
    Bash,
    Markdown,
    Java,
    CSharp,
    Php,
    Kotlin,
    Scala,
    Ruby,
    Sql,
    PowerShell,
    Lua,
    Zig,
    Elixir,
    Scss,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "py" | "pyi" => Some(Self::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "json" | "jsonc" => Some(Self::Json),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "html" | "htm" => Some(Self::Html),
            "css" => Some(Self::Css),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some(Self::Cpp),
            "go" => Some(Self::Go),
            "toml" => Some(Self::Toml),
            "yml" | "yaml" => Some(Self::Yaml),
            "sh" | "bash" | "zsh" => Some(Self::Bash),
            "md" | "markdown" => Some(Self::Markdown),
            "java" => Some(Self::Java),
            "cs" => Some(Self::CSharp),
            "php" | "phtml" => Some(Self::Php),
            "kt" | "kts" => Some(Self::Kotlin),
            "scala" | "sc" => Some(Self::Scala),
            "rb" | "rake" | "gemspec" => Some(Self::Ruby),
            "sql" => Some(Self::Sql),
            "ps1" | "psm1" | "psd1" => Some(Self::PowerShell),
            "lua" => Some(Self::Lua),
            "zig" => Some(Self::Zig),
            "ex" | "exs" => Some(Self::Elixir),
            "scss" => Some(Self::Scss),
            _ => None,
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// Every built-in syntax language in declaration order (matches `index`).
    pub const ALL: &'static [Self] = &[
        Self::Rust,
        Self::Python,
        Self::JavaScript,
        Self::Json,
        Self::TypeScript,
        Self::Tsx,
        Self::Html,
        Self::Css,
        Self::C,
        Self::Cpp,
        Self::Go,
        Self::Toml,
        Self::Yaml,
        Self::Bash,
        Self::Markdown,
        Self::Java,
        Self::CSharp,
        Self::Php,
        Self::Kotlin,
        Self::Scala,
        Self::Ruby,
        Self::Sql,
        Self::PowerShell,
        Self::Lua,
        Self::Zig,
        Self::Elixir,
        Self::Scss,
    ];

    /// Canonical lowercase identifier for this language.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::Json => "json",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Html => "html",
            Self::Css => "css",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Go => "go",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Bash => "bash",
            Self::Markdown => "markdown",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Php => "php",
            Self::Kotlin => "kotlin",
            Self::Scala => "scala",
            Self::Ruby => "ruby",
            Self::Sql => "sql",
            Self::PowerShell => "powershell",
            Self::Lua => "lua",
            Self::Zig => "zig",
            Self::Elixir => "elixir",
            Self::Scss => "scss",
        }
    }

    pub(crate) fn ts_language(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Json => tree_sitter_json::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Html => tree_sitter_html::LANGUAGE.into(),
            Self::Css => tree_sitter_css::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
            Self::Yaml => tree_sitter_yaml::LANGUAGE.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
            Self::Markdown => tree_sitter_md::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Kotlin => tree_sitter_kotlin::language(),
            Self::Scala => tree_sitter_scala::LANGUAGE.into(),
            Self::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Self::Sql => tree_sitter_sequel::LANGUAGE.into(),
            Self::PowerShell => tree_sitter_powershell::LANGUAGE.into(),
            Self::Lua => tree_sitter_lua::LANGUAGE.into(),
            Self::Zig => tree_sitter_zig::LANGUAGE.into(),
            Self::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            Self::Scss => tree_sitter_scss::language(),
        }
    }
    pub(crate) fn highlight_query(self) -> &'static str {
        match self {
            Self::Rust => tree_sitter_rust::HIGHLIGHTS_QUERY,
            Self::Python => tree_sitter_python::HIGHLIGHTS_QUERY,
            Self::JavaScript => tree_sitter_javascript::HIGHLIGHT_QUERY,
            Self::Json => tree_sitter_json::HIGHLIGHTS_QUERY,
            Self::TypeScript => tree_sitter_typescript::HIGHLIGHTS_QUERY,
            Self::Tsx => tree_sitter_typescript::HIGHLIGHTS_QUERY,
            Self::Html => tree_sitter_html::HIGHLIGHTS_QUERY,
            Self::Css => tree_sitter_css::HIGHLIGHTS_QUERY,
            Self::C => tree_sitter_c::HIGHLIGHT_QUERY,
            Self::Cpp => tree_sitter_cpp::HIGHLIGHT_QUERY,
            Self::Go => tree_sitter_go::HIGHLIGHTS_QUERY,
            Self::Toml => tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            Self::Yaml => tree_sitter_yaml::HIGHLIGHTS_QUERY,
            Self::Bash => tree_sitter_bash::HIGHLIGHT_QUERY,
            Self::Markdown => tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            Self::Java => tree_sitter_java::HIGHLIGHTS_QUERY,
            Self::CSharp => tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
            Self::Php => tree_sitter_php::HIGHLIGHTS_QUERY,
            Self::Kotlin => tree_sitter_kotlin::HIGHLIGHTS_QUERY,
            Self::Scala => tree_sitter_scala::HIGHLIGHTS_QUERY,
            Self::Ruby => tree_sitter_ruby::HIGHLIGHTS_QUERY,
            Self::Sql => tree_sitter_sequel::HIGHLIGHTS_QUERY,
            Self::PowerShell => tree_sitter_powershell::HIGHLIGHTS_QUERY,
            Self::Lua => tree_sitter_lua::HIGHLIGHTS_QUERY,
            Self::Zig => tree_sitter_zig::HIGHLIGHTS_QUERY,
            Self::Elixir => tree_sitter_elixir::HIGHLIGHTS_QUERY,
            Self::Scss => tree_sitter_scss::HIGHLIGHTS_QUERY,
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
    let Some(tree) = parse_source(language, source) else {
        return Vec::new();
    };
    let lines: Vec<_> = source.lines().collect();
    declarations_in(&tree, |row| lines.get(row).map(|text| (*text).to_owned()))
}

/// Return enclosing declaration headers from outermost to innermost.
pub fn enclosing_scope_headers(language: Language, source: &str, line: usize) -> Vec<ScopeHeader> {
    let Some(tree) = parse_source(language, source) else {
        return Vec::new();
    };
    let lines: Vec<_> = source.lines().collect();
    enclosing_scopes_in(&tree, line, |row| {
        lines.get(row).map(|text| (*text).to_owned())
    })
}

fn parse_source(language: Language, source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language.ts_language()).ok()?;
    parser.parse(source, None)
}

/// The declarations of a parsed document in source order; `line_text` gives a line's text.
pub(crate) fn declarations_in(
    tree: &tree_sitter::Tree,
    line_text: impl Fn(usize) -> Option<String>,
) -> Vec<ScopeHeader> {
    let mut nodes = Vec::new();
    collect_all_scope_nodes(tree.root_node(), &mut nodes);
    nodes.sort_by_key(|node| (node.start_position().row, node.start_position().column));
    scope_headers(nodes, line_text)
}

/// The declarations of a parsed document that enclose `line`, outermost first.
pub(crate) fn enclosing_scopes_in(
    tree: &tree_sitter::Tree,
    line: usize,
    line_text: impl Fn(usize) -> Option<String>,
) -> Vec<ScopeHeader> {
    let mut nodes = Vec::new();
    collect_scope_nodes(tree.root_node(), line, &mut nodes);
    nodes.sort_by_key(|node| node.start_position().row);
    scope_headers(nodes, line_text)
}

fn scope_headers(
    nodes: Vec<tree_sitter::Node<'_>>,
    line_text: impl Fn(usize) -> Option<String>,
) -> Vec<ScopeHeader> {
    nodes
        .into_iter()
        .filter_map(|node| {
            let row = node.start_position().row;
            line_text(row).map(|text| ScopeHeader {
                line: row,
                text: text.trim().to_owned(),
            })
        })
        .collect()
}

fn is_scope_node_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_definition"
            | "function_declaration"
            | "method_definition"
            | "method_declaration"
            | "class_definition"
            | "class_declaration"
            | "class_specifier"
            | "interface_declaration"
            | "trait_declaration"
            | "trait_definition"
            | "object_definition"
            | "object_declaration"
            | "record_declaration"
            | "struct_declaration"
            | "constructor_declaration"
            | "namespace_declaration"
            | "function_statement"
            | "test_declaration"
            | "module"
            | "class"
            | "method"
            | "singleton_method"
            | "type_alias_declaration"
            | "type_declaration"
            | "enum_declaration"
            | "enum_specifier"
            | "struct_specifier"
            | "namespace_definition"
            | "struct_item"
            | "enum_item"
            | "impl_item"
            | "trait_item"
            | "mod_item"
            | "table"
            | "table_array_element"
            | "atx_heading"
    )
}

fn collect_scope_nodes<'tree>(
    node: tree_sitter::Node<'tree>,
    line: usize,
    output: &mut Vec<tree_sitter::Node<'tree>>,
) {
    if node.start_position().row > line || node.end_position().row < line {
        return;
    }
    if is_scope_node_kind(node.kind()) {
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
    if is_scope_node_kind(node.kind()) {
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
    fn scopes_from_the_highlighter_tree_match_a_fresh_parse() {
        let source = "mod outer {\n    struct S;\n    impl S {\n        fn method(&self) {\n            let x = 1;\n        }\n    }\n}\nfn free() {}\n";
        let rope = ropey::Rope::from_str(source);
        let mut highlighter = IncrementalHighlighter::new();
        assert!(highlighter.enclosing_scopes(&rope, 4).is_none());
        highlighter.highlight_rope(Language::Rust, &rope, None);

        assert_eq!(
            highlighter.enclosing_scopes(&rope, 4).unwrap(),
            enclosing_scope_headers(Language::Rust, source, 4)
        );
        let enclosing: Vec<_> = highlighter
            .enclosing_scopes(&rope, 4)
            .unwrap()
            .into_iter()
            .map(|header| header.line)
            .collect();
        assert_eq!(enclosing, vec![0, 2, 3]);
        assert_eq!(
            highlighter.declarations(&rope).unwrap(),
            declaration_headers(Language::Rust, source)
        );
        assert_eq!(highlighter.declarations(&rope).unwrap().len(), 5);
    }

    #[test]
    fn scopes_follow_incremental_edits() {
        let before = "fn a() {\n    1\n}\n";
        let mut rope = ropey::Rope::from_str(before);
        let mut highlighter = IncrementalHighlighter::new();
        highlighter.highlight_rope(Language::Rust, &rope, None);
        // Insert a second function at the end.
        let insert = "fn b() {}\n";
        let at = rope.len_chars();
        rope.insert(at, insert);
        highlighter.highlight_rope(
            Language::Rust,
            &rope,
            Some(TextEdit {
                start_byte: before.len(),
                old_end_byte: before.len(),
                new_end_byte: before.len() + insert.len(),
                start_position: (3, 0),
                old_end_position: (3, 0),
                new_end_position: (4, 0),
            }),
        );
        let lines: Vec<_> = highlighter
            .declarations(&rope)
            .unwrap()
            .into_iter()
            .map(|header| header.text)
            .collect();
        assert_eq!(lines, vec!["fn a() {", "fn b() {}"]);
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
        assert_eq!(*changed, full.highlight(Language::Rust, new));
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
        assert_eq!(*changed, full.highlight(Language::Rust, &next));
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
        assert_eq!(*changed, full.highlight(Language::Rust, new));
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

    #[test]
    fn detects_all_expanded_extensions() {
        let cases = [
            ("src/main.rs", Language::Rust),
            ("script.pyi", Language::Python),
            ("index.mjs", Language::JavaScript),
            ("settings.jsonc", Language::Json),
            ("app.ts", Language::TypeScript),
            ("Button.tsx", Language::Tsx),
            ("index.html", Language::Html),
            ("styles.css", Language::Css),
            ("kernel.c", Language::C),
            ("header.hpp", Language::Cpp),
            ("server.go", Language::Go),
            ("Cargo.toml", Language::Toml),
            ("ci.yml", Language::Yaml),
            ("build.sh", Language::Bash),
            ("README.md", Language::Markdown),
            ("App.java", Language::Java),
            ("Program.cs", Language::CSharp),
            ("index.php", Language::Php),
            ("Main.kt", Language::Kotlin),
            ("Main.scala", Language::Scala),
            ("app.rb", Language::Ruby),
            ("schema.sql", Language::Sql),
            ("deploy.ps1", Language::PowerShell),
            ("init.lua", Language::Lua),
            ("build.zig", Language::Zig),
            ("app.ex", Language::Elixir),
            ("theme.scss", Language::Scss),
        ];
        for (path, expected) in cases {
            assert_eq!(
                Language::from_path(Path::new(path)),
                Some(expected),
                "path {path}"
            );
        }
    }

    #[test]
    fn all_languages_highlight_consistently_between_full_and_incremental() {
        let samples = [
            (Language::Rust, "fn main() { let x = \"hi\"; }\n"),
            (Language::Python, "def greet(name):\n    return \"hi\"\n"),
            (Language::JavaScript, "const x = () => 'hello';\n"),
            (Language::Json, "{\"key\": [1, true, null]}\n"),
            (
                Language::TypeScript,
                "interface User { name: string }\nconst u: User = { name: 'a' };\n",
            ),
            (
                Language::Tsx,
                "export function App() { return <div className=\"box\">hi</div>; }\n",
            ),
            (
                Language::Html,
                "<!DOCTYPE html><html><body><p>Hello</p></body></html>\n",
            ),
            (Language::Css, "body { color: #ff00aa; margin: 0px; }\n"),
            (
                Language::C,
                "#include <stdio.h>\nint add(int a, int b) { return a + b; }\n",
            ),
            (
                Language::Cpp,
                "class Box { public: int width() const { return 1; } };\n",
            ),
            (
                Language::Go,
                "package main\nfunc main() { msg := \"ok\"; _ = msg }\n",
            ),
            (
                Language::Toml,
                "[package]\nname = \"graphite\"\nversion = \"0.1.0\"\n",
            ),
            (
                Language::Yaml,
                "services:\n  api:\n    ports: [8080]\n    enabled: true\n",
            ),
            (
                Language::Bash,
                "#!/usr/bin/env bash\nfunction hi() { echo \"ok\"; }\n",
            ),
            (Language::Markdown, "# Heading\n\n- item one\n"),
            (
                Language::Java,
                "public class App { public static void main(String[] args) { System.out.println(\"hi\"); } }\n",
            ),
            (
                Language::CSharp,
                "namespace Demo { public class Program { public static int Main() => 0; } }\n",
            ),
            (
                Language::Php,
                "<?php\nfunction greet(string $name): string { return \"hi $name\"; }\n",
            ),
            (
                Language::Kotlin,
                "fun main(args: Array<String>) { val msg = \"hi\"; println(msg) }\n",
            ),
            (
                Language::Scala,
                "object Main { def greet(name: String): String = s\"hi $name\" }\n",
            ),
            (
                Language::Ruby,
                "class Greeter\n  def hi(name)\n    \"hi #{name}\"\n  end\nend\n",
            ),
            (
                Language::Sql,
                "SELECT id, name FROM users WHERE active = true ORDER BY id DESC;\n",
            ),
            (
                Language::PowerShell,
                "function Get-Greeting { param([string]$Name) return \"hi $Name\" }\n",
            ),
            (
                Language::Lua,
                "local function greet(name)\n  return \"hi \" .. name\nend\n",
            ),
            (
                Language::Zig,
                "const std = @import(\"std\");\npub fn main() void { _ = std; }\n",
            ),
            (
                Language::Elixir,
                "defmodule Demo do\n  def greet(name), do: \"hi #{name}\"\nend\n",
            ),
            (
                Language::Scss,
                "$accent: #056ef0;\n.card { color: $accent; &:hover { opacity: 0.8; } }\n",
            ),
        ];

        let mut full = Highlighter::new();
        for (lang, src) in samples {
            let mut inc = IncrementalHighlighter::new();
            let f_spans = full.highlight(lang, src);
            let i_spans = inc.highlight(lang, src, None);
            assert!(
                f_spans.iter().any(|line| !line.is_empty()),
                "expected full highlighter tokens for {:?}",
                lang
            );
            assert!(
                i_spans.iter().any(|line| !line.is_empty()),
                "expected incremental highlighter tokens for {:?}",
                lang
            );
            let mut fresh = IncrementalHighlighter::new();
            assert_eq!(i_spans, fresh.highlight(lang, src, None));
        }
    }

    #[test]
    fn outline_and_sticky_scroll_work_for_expanded_languages() {
        let ts = "interface Config {\n  port: number;\n}\nfunction boot() {\n  const x = 1;\n}\n";
        let ts_headers = declaration_headers(Language::TypeScript, ts);
        assert_eq!(ts_headers.len(), 2);

        let go = "package main\ntype Server struct {}\nfunc (s *Server) Start() {\n\tprintln(\"hi\")\n}\n";
        let go_sticky = enclosing_scope_headers(Language::Go, go, 3);
        assert!(go_sticky.iter().any(|h| h.text.contains("Start")));

        let md = "# Title\n\n## Section\n\nParagraph\n";
        let md_headers = declaration_headers(Language::Markdown, md);
        assert!(md_headers.iter().any(|h| h.text.contains("# Title")));

        let java = "public class Service {\n  public void run() {\n    int x = 1;\n  }\n}\n";
        let java_sticky = enclosing_scope_headers(Language::Java, java, 2);
        assert!(java_sticky.iter().any(|h| h.text.contains("Service")));
        assert!(java_sticky.iter().any(|h| h.text.contains("run")));

        let ruby = "class Worker\n  def perform\n    puts 'ok'\n  end\nend\n";
        let ruby_headers = declaration_headers(Language::Ruby, ruby);
        assert!(ruby_headers.iter().any(|h| h.text.contains("Worker")));
        assert!(ruby_headers.iter().any(|h| h.text.contains("perform")));
    }
}
