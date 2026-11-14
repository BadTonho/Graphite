use super::*;
use crate::Class;
use serde_json::{json, Value};

fn toy() -> Value {
    json!({
        "scopeName": "source.toy",
        "patterns": [
            {"include": "#comments"}, {"include": "#strings"}, {"include": "#heredoc"},
            {"include": "#quote"}, {"include": "#fn"}, {"include": "#keywords"}, {"include": "#numbers"},
            {"include": "#list"}
        ],
        "repository": {
            "comments": {"patterns": [
                {"match": "//.*$", "name": "comment.line.toy"},
                {"begin": "/\\*", "end": "\\*/", "name": "comment.block.toy"}
            ]},
            "strings": {
                "begin": "\"", "end": "\"", "name": "string.quoted.double.toy",
                "patterns": [{"match": "\\\\.", "name": "constant.character.escape.toy"}]
            },
            "numbers": {"match": "\\b\\d+(\\.\\d+)?\\b", "name": "constant.numeric.toy"},
            "keywords": {"match": "\\b(if|else|while)\\b", "name": "keyword.control.toy"},
            "fn": {"match": "\\b(fn)\\s+(\\w+)", "captures": {
                "1": {"name": "keyword.control.toy"}, "2": {"name": "entity.name.function.toy"}}},
            "heredoc": {
                "begin": "<<(\\w+)", "end": "^\\1$", "name": "string.unquoted.heredoc.toy",
                "beginCaptures": {"1": {"name": "keyword.operator.toy"}}
            },
            "quote": {
                "begin": "^>>> ", "while": "^>>>", "name": "markup.quote.toy"
            },
            "list": {
                "match": "(\\[)(.*?)(\\])", "captures": {
                    "2": {"patterns": [{"match": "\\d+", "name": "constant.numeric.toy"}]},
                    "0": {"name": "meta.list.toy"}}
            }
        }
    })
}

fn registry() -> (Registry, usize) {
    let mut registry = Registry::new(|_| None);
    let base = registry.add(&toy()).unwrap();
    (registry, base)
}

/// Spans of every line as `(text, class)` pairs, to read the expectations.
fn tokens(text: &str) -> Vec<Vec<(String, Class)>> {
    let (registry, base) = registry();
    let spans = DocumentHighlighter::new().highlight(&registry, base, text);
    text.split('\n')
        .zip(spans)
        .map(|(line, spans)| {
            spans
                .into_iter()
                .map(|span| (line[span.range].to_owned(), span.class))
                .collect()
        })
        .collect()
}

fn pair(text: &str, class: Class) -> (String, Class) {
    (text.to_owned(), class)
}

#[test]
fn matches_captures_and_keywords_are_classified() {
    let lines = tokens("fn main 42 if x // note");
    assert_eq!(
        lines[0],
        [
            pair("fn", Class::Keyword),
            pair("main", Class::Function),
            pair("42", Class::Number),
            pair("if", Class::Keyword),
            pair("// note", Class::Comment),
        ]
    );
}

#[test]
fn a_block_comment_and_a_string_carry_over_lines() {
    let lines = tokens("a /* one\ntwo 42\nthree */ 7\n\"x\ny\" 9");
    assert_eq!(lines[0], [pair("/* one", Class::Comment)]);
    assert_eq!(lines[1], [pair("two 42", Class::Comment)], "no number inside a comment");
    assert_eq!(lines[2], [pair("three */", Class::Comment), pair("7", Class::Number)]);
    assert_eq!(lines[3], [pair("\"x", Class::String)]);
    assert_eq!(lines[4], [pair("y\"", Class::String), pair("9", Class::Number)]);
}

#[test]
fn escapes_inside_strings_override_the_string_and_quotes_follow_it() {
    let lines = tokens(r#"s "a\"b" 1"#);
    assert_eq!(
        lines[0],
        [
            pair("\"a", Class::String),
            pair("\\\"", Class::Constant),
            pair("b\"", Class::String),
            pair("1", Class::Number),
        ]
    );
}

#[test]
fn an_end_pattern_can_use_the_text_the_begin_pattern_matched() {
    let lines = tokens("<<END\nEND is text here\nEND\nafter 5");
    assert_eq!(lines[0], [pair("<<", Class::String), pair("END", Class::Operator)]);
    assert_eq!(lines[1], [pair("END is text here", Class::String)], "END must be alone on its line");
    assert_eq!(lines[2], [pair("END", Class::String)]);
    assert_eq!(lines[3], [pair("5", Class::Number)], "the heredoc is closed");
    // A different marker does not close it.
    let lines = tokens("<<A\nB\nA\n1");
    assert_eq!(lines[1], [pair("B", Class::String)]);
    assert_eq!(lines[3], [pair("1", Class::Number)]);
}

#[test]
fn a_while_rule_ends_on_the_first_line_its_condition_fails() {
    let lines = tokens(">>> quoted\n>>>still 5\nplain 6");
    assert_eq!(lines[0], [pair(">>> quoted", Class::Comment)]);
    assert_eq!(lines[1], [pair(">>>still 5", Class::Comment)]);
    assert_eq!(lines[2], [pair("6", Class::Number)], "the quote ended");
}

#[test]
fn captures_can_be_tokenized_with_their_own_patterns() {
    let lines = tokens("[ 12 and 3 ] 4");
    assert_eq!(
        lines[0],
        [pair("12", Class::Number), pair("3", Class::Number), pair("4", Class::Number)],
        "{:?}",
        lines[0]
    );
}

#[test]
fn offsets_are_bytes_of_the_line_even_with_multibyte_text() {
    let text = "é \"ü\" 5";
    let (registry, base) = registry();
    let spans = DocumentHighlighter::new().highlight(&registry, base, text);
    let found: Vec<(&str, Class)> = spans[0].iter().map(|s| (&text[s.range.clone()], s.class)).collect();
    assert_eq!(found, [("\"ü\"", Class::String), ("5", Class::Number)]);
}

#[test]
fn windows_line_endings_do_not_change_the_spans() {
    let (registry, base) = registry();
    let unix = DocumentHighlighter::new().highlight(&registry, base, "a /* x\ny */ 3");
    let windows = DocumentHighlighter::new().highlight(&registry, base, "a /* x\r\ny */ 3");
    assert_eq!(unix, windows);
}

#[test]
fn grammars_can_include_other_grammars_and_the_base_grammar() {
    let mut registry = Registry::new(|scope| {
        (scope == "source.inner").then(|| {
            json!({"scopeName": "source.inner", "patterns": [{"include": "#word"}],
                "repository": {"word": {"match": "\\bfoo\\b", "name": "keyword.inner"}}})
        })
    });
    let base = registry
        .add(&json!({
            "scopeName": "source.host",
            "patterns": [
                {"begin": "<(?=i)", "end": ">", "name": "meta.embed",
                 "patterns": [{"include": "source.inner"}, {"include": "$base"}]},
                {"match": "\\d+", "name": "constant.numeric.host"}
            ]
        }))
        .unwrap();
    let text = "<ifoo 12> 3";
    let spans = DocumentHighlighter::new().highlight(&registry, base, text);
    let found: Vec<(&str, Class)> = spans[0].iter().map(|s| (&text[s.range.clone()], s.class)).collect();
    // `foo` follows `i`, so the word boundary fails; the number inside comes from `$base`.
    assert_eq!(found, [("12", Class::Number), ("3", Class::Number)]);
    let text = "<i foo>";
    let spans = DocumentHighlighter::new().highlight(&registry, base, text);
    assert_eq!(&text[spans[0][0].range.clone()], "foo");
    assert_eq!(spans[0][0].class, Class::Keyword, "the included grammar's rule applies");
}

#[test]
fn broken_patterns_and_empty_matches_never_hang_or_break_the_rest() {
    let mut registry = Registry::new(|_| None);
    let base = registry
        .add(&json!({
            "scopeName": "source.odd",
            "patterns": [
                {"match": "(unclosed", "name": "keyword.never"},
                {"match": "x*", "name": "keyword.empty"},
                {"begin": "(?=a)", "end": "(?=b)", "name": "string.loop"},
                {"match": "\\d", "name": "constant.numeric"}
            ]
        }))
        .unwrap();
    let text = "xx a 1 b 2\nzzz";
    let spans = DocumentHighlighter::new().highlight(&registry, base, text);
    assert_eq!(spans.len(), 2);
    let class_at = |line: usize, byte: usize| {
        spans[line].iter().find(|s| s.range.contains(&byte)).map(|s| s.class)
    };
    assert_eq!(class_at(0, 0), Some(Class::Keyword), "x* matched xx");
    assert_eq!(class_at(0, 2), None, "a space matches nothing");
}

#[test]
fn editing_retokenizes_only_what_changed_and_matches_a_fresh_run() {
    let (registry, base) = registry();
    let mut incremental = DocumentHighlighter::new();
    let versions = [
        "a /* start\nmiddle 1\nend */ 2\nx 3\n\"s\" 4\n",
        // Close the comment on the second line.
        "a /* start\nmiddle */ 1\nend */ 2\nx 3\n\"s\" 4\n",
        // Insert and remove lines.
        "a /* start\nmiddle */ 1\nnew line 9\nend */ 2\nx 3\n\"s\" 4\n",
        "fn f\nmiddle */ 1\nnew line 9\nend */ 2\nx 3\n\"s\" 4\n",
        // Open a string that swallows the rest.
        "fn f\nmiddle \" 1\nnew line 9\nend */ 2\nx 3\n\"s\" 4\n",
        "",
        "7",
    ];
    for text in versions {
        let fresh = DocumentHighlighter::new().highlight(&registry, base, text);
        assert_eq!(incremental.highlight(&registry, base, text), fresh, "{text:?}");
    }
    // Going back and forth keeps working too.
    for text in versions.iter().rev() {
        let fresh = DocumentHighlighter::new().highlight(&registry, base, text);
        assert_eq!(incremental.highlight(&registry, base, text), fresh, "{text:?}");
    }
}

#[test]
fn a_different_grammar_starts_over() {
    let mut registry = Registry::new(|_| None);
    let a = registry.add(&json!({"scopeName": "source.a", "patterns": [{"match": "x", "name": "keyword.a"}]})).unwrap();
    let b = registry.add(&json!({"scopeName": "source.b", "patterns": [{"match": "x", "name": "string.b"}]})).unwrap();
    let mut highlighter = DocumentHighlighter::new();
    assert_eq!(highlighter.highlight(&registry, a, "x")[0][0].class, Class::Keyword);
    assert_eq!(highlighter.highlight(&registry, b, "x")[0][0].class, Class::String);
}
