//! Links in terminal output: `https://` URLs and `file:line[:column]` references.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLink {
    Url(String),
    File {
        path: String,
        line: usize,
        column: usize,
    },
}

fn url_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"https?://[^\s<>"'`]+"#).expect("valid URL pattern"))
}

fn file_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?:[A-Za-z]:)?[\w./\\-]*[\w.-]\.[A-Za-z0-9]+:\d+(?::\d+)?")
            .expect("valid file pattern")
    })
}

/// The link under character column `column` of a terminal line, if any.
pub fn link_at(line: &str, column: usize) -> Option<TerminalLink> {
    // Byte offset of each character, plus the end of the line, for mapping matches to columns.
    let columns: Vec<usize> = line.char_indices().map(|(index, _)| index).collect();
    let char_of = |byte: usize| columns.iter().take_while(|index| **index < byte).count();
    for found in url_pattern().find_iter(line) {
        let text = trim_trailing_punctuation(found.as_str());
        let end = char_of(found.start()) + text.chars().count();
        if (char_of(found.start())..end).contains(&column) {
            return Some(TerminalLink::Url(text.to_owned()));
        }
    }
    for found in file_pattern().find_iter(line) {
        let start = char_of(found.start());
        let end = start + found.as_str().chars().count();
        if (start..end).contains(&column) {
            return parse_file_reference(found.as_str());
        }
    }
    None
}

/// Drops sentence punctuation after a URL; a closing bracket is dropped only when unmatched.
fn trim_trailing_punctuation(text: &str) -> &str {
    let mut end = text.len();
    loop {
        let head = &text[..end];
        let Some(last) = head.chars().last() else {
            break;
        };
        let drop = match last {
            '.' | ',' | ';' | ':' => true,
            ')' => head.matches(')').count() > head.matches('(').count(),
            ']' => head.matches(']').count() > head.matches('[').count(),
            '}' => head.matches('}').count() > head.matches('{').count(),
            _ => false,
        };
        if !drop {
            break;
        }
        end -= last.len_utf8();
    }
    &text[..end]
}

/// `path:line[:column]`; the path may contain a Windows drive letter.
fn parse_file_reference(text: &str) -> Option<TerminalLink> {
    let mut parts = text.rsplitn(3, ':');
    let last: usize = parts.next()?.parse().ok()?;
    let (path, line, column) = match parts.next().and_then(|middle| middle.parse().ok()) {
        // `path:line:column` (the middle part was the line number).
        Some(line) => (parts.next()?, line, last),
        // `path:line` where the path itself has a drive letter, e.g. `C:\dir\a.rs:12`.
        None => (text.strip_suffix(&format!(":{last}"))?, last, 1),
    };
    if path.is_empty() || line == 0 {
        return None;
    }
    Some(TerminalLink::File {
        path: path.to_owned(),
        line,
        column: column.max(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_found_and_trailing_punctuation_is_excluded() {
        let line = "see https://example.com/docs/a_(b). now";
        assert_eq!(
            link_at(line, 6),
            Some(TerminalLink::Url("https://example.com/docs/a_(b)".into()))
        );
        assert_eq!(link_at(line, 0), None);
    }

    #[test]
    fn file_references_with_line_and_column_are_parsed() {
        let line = "error at src/main.rs:12:5 here";
        assert_eq!(
            link_at(line, 10),
            Some(TerminalLink::File {
                path: "src/main.rs".into(),
                line: 12,
                column: 5,
            })
        );
    }

    #[test]
    fn windows_paths_with_drive_letters_and_line_only_references_work() {
        let line = r"at C:\code\lib.rs:7";
        assert_eq!(
            link_at(line, 6),
            Some(TerminalLink::File {
                path: r"C:\code\lib.rs".into(),
                line: 7,
                column: 1,
            })
        );
    }

    #[test]
    fn columns_after_wide_characters_are_counted_in_characters() {
        let line = "🌎 a.rs:3";
        assert!(matches!(
            link_at(line, 2),
            Some(TerminalLink::File { line: 3, .. })
        ));
        assert_eq!(link_at(line, 0), None);
    }

    #[test]
    fn plain_text_has_no_link() {
        assert_eq!(link_at("just words 12", 3), None);
    }
}
