//! Semantic tokens: the delta-encoded token list that language servers (LSP) and extension
//! providers (Code OSS) both use, its mapping to syntax classes, and the conversion of a line's
//! tokens to byte spans for drawing. Shared by `lsp` and `app::plugins::vsix::semantic`.

use graphite_syntax::{Class, Span};
use std::collections::BTreeMap;

/// One token on a line: UTF-16 start column, length, and the class it is drawn with.
pub(crate) type LineToken = (u32, u32, Class);

/// Tokens by 0-based line.
pub(crate) type Tokens = BTreeMap<usize, Vec<LineToken>>;

/// The tokens a language server sent for one revision of a document.
#[derive(Debug, Clone)]
pub(crate) struct DocumentTokens {
    pub uri: String,
    pub revision: u64,
    pub lines: Tokens,
}

/// The syntax class used for a token type of the LSP legend, or `None` for types that keep the
/// syntax color.
pub(crate) fn class_for_token_type(name: &str) -> Option<Class> {
    Some(match name {
        "namespace" | "type" | "class" | "enum" | "interface" | "struct" | "typeParameter" => {
            Class::Type
        }
        "function" | "method" | "macro" | "decorator" | "event" => Class::Function,
        "variable" | "parameter" => Class::Variable,
        "property" | "enumMember" => Class::Property,
        "keyword" | "modifier" => Class::Keyword,
        "comment" => Class::Comment,
        "string" | "regexp" => Class::String,
        "number" => Class::Number,
        "operator" => Class::Operator,
        _ => return None,
    })
}

/// Decodes `data`, groups of five words `deltaLine, deltaStart, length, tokenType, modifiers`,
/// against the legend's token type names. Returns `None` when a position would overflow.
pub(crate) fn decode_data(data: &[u32], legend: &[String]) -> Option<Tokens> {
    let mut lines = Tokens::new();
    let (mut line, mut start) = (0usize, 0u32);
    for token in data.as_chunks::<5>().0 {
        let (delta_line, delta_start, length, kind) = (token[0], token[1], token[2], token[3]);
        if delta_line > 0 {
            line = line.checked_add(delta_line as usize)?;
            start = delta_start;
        } else {
            start = start.checked_add(delta_start)?;
        }
        let class = legend
            .get(kind as usize)
            .and_then(|name| class_for_token_type(name));
        if let (Some(class), true) = (class, length > 0) {
            lines.entry(line).or_default().push((start, length, class));
        }
    }
    Some(lines)
}

/// The byte spans of `tokens` on a line of text (columns are UTF-16 units).
pub(crate) fn byte_spans(text: &str, tokens: &[LineToken]) -> Vec<Span> {
    // Byte offset of every UTF-16 column that starts a character.
    let mut offsets = Vec::with_capacity(text.len() + 1);
    for (byte, ch) in text.char_indices() {
        for _ in 0..ch.len_utf16() {
            offsets.push(byte);
        }
    }
    let line_end = text.trim_end_matches(['\r', '\n']).len();
    offsets.push(line_end);
    let at = |column: u32| offsets.get(column as usize).copied().unwrap_or(line_end);
    tokens
        .iter()
        .filter_map(|&(start, length, class)| {
            let (from, to) = (at(start), at(start.saturating_add(length)));
            (from < to).then_some(Span {
                range: from..to,
                class,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legend() -> Vec<String> {
        ["keyword", "variable", "function", "weird", "comment"]
            .map(String::from)
            .into()
    }

    #[test]
    fn token_types_map_to_syntax_classes() {
        assert_eq!(class_for_token_type("class"), Some(Class::Type));
        assert_eq!(class_for_token_type("method"), Some(Class::Function));
        assert_eq!(class_for_token_type("parameter"), Some(Class::Variable));
        assert_eq!(class_for_token_type("enumMember"), Some(Class::Property));
        assert_eq!(class_for_token_type("regexp"), Some(Class::String));
        assert_eq!(class_for_token_type("customType"), None);
    }

    #[test]
    fn deltas_decode_to_absolute_positions() {
        // Line 0: `let` at 0 (3 wide), `x` at 4 (1). Line 2: `f` at 2 (1), a comment at 5 (4).
        let data = [0, 0, 3, 0, 0, 0, 4, 1, 1, 0, 2, 2, 1, 2, 0, 0, 3, 4, 4, 0];
        let tokens = decode_data(&data, &legend()).unwrap();
        assert_eq!(
            tokens[&0],
            [(0, 3, Class::Keyword), (4, 1, Class::Variable)]
        );
        assert_eq!(
            tokens[&2],
            [(2, 1, Class::Function), (5, 4, Class::Comment)]
        );
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn unknown_types_and_empty_tokens_keep_the_syntax_color() {
        // Type 3 ("weird") has no class; the second token has length zero.
        let data = [0, 0, 2, 3, 0, 0, 3, 0, 0, 0, 0, 5, 1, 0, 0];
        let tokens = decode_data(&data, &legend()).unwrap();
        assert_eq!(tokens[&0], [(8, 1, Class::Keyword)]);
    }

    #[test]
    fn overflowing_positions_and_partial_groups_do_not_panic() {
        assert!(decode_data(&[0, u32::MAX, 1, 0, 0, 0, 5, 1, 0, 0], &legend()).is_none());
        // A trailing partial group is ignored.
        let tokens = decode_data(&[0, 0, 1, 0, 0, 9, 9], &legend()).unwrap();
        assert_eq!(tokens[&0], [(0, 1, Class::Keyword)]);
        assert!(decode_data(&[], &legend()).unwrap().is_empty());
    }

    #[test]
    fn spans_convert_utf16_columns_to_byte_ranges() {
        // `é` is two bytes and one UTF-16 unit; the emoji is four bytes and two units.
        let text = "é\u{1F30E}ab\n";
        let spans = byte_spans(
            text,
            &[
                (0, 1, Class::Type),
                (1, 2, Class::String),
                (3, 2, Class::Keyword),
                (3, 99, Class::Number),
            ],
        );
        let ranges: Vec<_> = spans.iter().map(|s| (s.range.clone(), s.class)).collect();
        assert_eq!(ranges[0], (0..2, Class::Type));
        assert_eq!(ranges[1], (2..6, Class::String));
        assert_eq!(ranges[2], (6..8, Class::Keyword));
        assert_eq!(
            ranges[3],
            (6..8, Class::Number),
            "clamped to the line end, not the newline"
        );
    }

    #[test]
    fn an_empty_or_out_of_range_token_draws_nothing() {
        assert!(byte_spans("abc", &[(5, 2, Class::Type)]).is_empty());
        assert!(byte_spans("abc", &[(1, 0, Class::Type)]).is_empty());
        assert!(byte_spans("", &[(0, 3, Class::Type)]).is_empty());
    }
}
