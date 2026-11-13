//! Semantic tokens from extensions (`registerDocumentSemanticTokensProvider`).
//!
//! The provider describes the text as a flat list of `(line, column, length, type, modifiers)`
//! tokens, delta-encoded. Graphite decodes them, maps each token type to the syntax class the
//! editor already knows how to color, and draws them over the tree-sitter highlighting, as VS Code
//! does. Tokens describe one revision of the text; after an edit the syntax colors show until the
//! provider answers again.
//!
//! Only whole-document "full" answers are understood. Delta answers and range providers are not
//! used.

use super::languages::Feature;
use super::*;
use graphite_core_api::{DocumentId, DocumentSnapshot};
use graphite_syntax::{Class, Span};
use std::collections::BTreeMap;

/// One decoded token on a line: UTF-16 start column, length, and the class it is drawn with.
type LineToken = (u32, u32, Class);

/// The semantic tokens of the active document at one revision.
pub(crate) struct SemanticTokens {
    pub uri: String,
    pub document: DocumentId,
    pub revision: u64,
    lines: BTreeMap<usize, Vec<LineToken>>,
}

/// The syntax class used for a token type of the LSP/Code OSS legend, or `None` for types that
/// keep the syntax color.
pub(super) fn class_for_token_type(name: &str) -> Option<Class> {
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

/// Decodes the words of a full-document answer: `[resultId, kind (1 = full), count, data...]`
/// with `data` in groups of five (`deltaLine, deltaStart, length, type, modifiers`). Returns the
/// tokens by 0-based line, or `None` for any other kind or a truncated list.
pub(super) fn decode(words: &[u32], legend: &[String]) -> Option<BTreeMap<usize, Vec<LineToken>>> {
    const FULL: u32 = 1;
    if words.len() < 3 || words[1] != FULL {
        return None;
    }
    let count = words[2] as usize;
    let data = words.get(3..3usize.checked_add(count)?)?;
    let mut lines: BTreeMap<usize, Vec<LineToken>> = BTreeMap::new();
    let (mut line, mut start) = (0usize, 0u32);
    for token in data.chunks_exact(5) {
        let (delta_line, delta_start, length, kind) = (token[0], token[1], token[2], token[3]);
        if delta_line > 0 {
            line += delta_line as usize;
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
fn byte_spans(text: &str, tokens: &[LineToken]) -> Vec<Span> {
    // Byte offset of every UTF-16 column that starts a character.
    let mut offsets = Vec::with_capacity(text.len() + 1);
    for (byte, ch) in text.char_indices() {
        for _ in 0..ch.len_utf16() {
            offsets.push(byte);
        }
    }
    offsets.push(text.trim_end_matches(['\r', '\n']).len());
    let line_end = text.trim_end_matches(['\r', '\n']).len();
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

impl Runtime {
    /// The legend (token type names) of the semantic token provider with this handle.
    fn semantic_legend(&self, handle: u64) -> Vec<String> {
        self.languages.legend(handle)
    }
}

impl App {
    /// Semantic spans to draw over the syntax colors of `line`, when the tokens describe this
    /// revision of the document with this URI.
    pub(crate) fn vsix_semantic_spans(
        &self,
        uri: &str,
        revision: u64,
        line: usize,
        text: &str,
    ) -> Vec<Span> {
        self.plugins
            .vsix
            .as_ref()
            .and_then(|runtime| runtime.passive.semantic.as_ref())
            .filter(|tokens| tokens.uri == uri && tokens.revision == revision)
            .and_then(|tokens| tokens.lines.get(&line))
            .map(|tokens| byte_spans(text, tokens))
            .unwrap_or_default()
    }

    pub(super) fn request_vsix_semantic_tokens(
        &self,
        runtime: &mut Runtime,
        snapshot: &DocumentSnapshot,
        key: super::passive::Key,
    ) -> Result<()> {
        runtime.passive.semantic_done = Some((key.document, key.revision));
        let Some(handle) = runtime.provider(Feature::SemanticTokens, snapshot) else {
            return Ok(());
        };
        if let Some(previous) = runtime.passive.semantic_request.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall","id":previous.id}))?;
        }
        let id = runtime.passive.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideDocumentSemanticTokens", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri}, 0],
        }))?;
        runtime.passive.semantic_request = Some(super::passive::InFlight {
            id,
            key,
            handle,
            uri: snapshot.uri.as_str().to_owned(),
            deadline: Instant::now() + std::time::Duration::from_secs(10),
        });
        Ok(())
    }

    /// Stores the tokens of an answer if the document has not moved on. The bridge sends the
    /// binary answer as `{ "$u32": [...] }`.
    pub(super) fn finish_vsix_semantic_tokens(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(request) = runtime.passive.semantic_request.take() else {
            return;
        };
        if event["error"].is_string() {
            // Not asked again until the document changes.
            return;
        }
        let current = self.active_document() == request.key.document
            && self.documents[request.key.document].buffer.revision() == request.key.revision;
        if !current {
            runtime.passive.semantic_done = None;
            return;
        }
        let words: Vec<u32> = event["result"]["$u32"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|word| u32::try_from(word.as_u64()?).ok())
            .collect();
        let legend = runtime.semantic_legend(request.handle);
        runtime.passive.semantic = decode(&words, &legend).map(|lines| SemanticTokens {
            uri: request.uri,
            document: request.key.document,
            revision: request.key.revision,
            lines,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legend() -> Vec<String> {
        ["keyword", "variable", "function", "weird", "comment"]
            .map(String::from)
            .into()
    }

    /// `[resultId, full, count, data...]`
    fn answer(data: &[u32]) -> Vec<u32> {
        let mut words = vec![7, 1, data.len() as u32];
        words.extend_from_slice(data);
        words
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
        let tokens = decode(&answer(&data), &legend()).unwrap();
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
        let tokens = decode(&answer(&data), &legend()).unwrap();
        assert_eq!(tokens[&0], [(8, 1, Class::Keyword)]);
    }

    #[test]
    fn only_complete_full_answers_are_decoded() {
        assert!(decode(&[], &legend()).is_none());
        assert!(decode(&[1, 1], &legend()).is_none(), "no length word");
        assert!(decode(&[1, 2, 0], &legend()).is_none(), "a delta answer");
        // Declares 10 data words but carries 5.
        assert!(decode(&[1, 1, 10, 0, 0, 1, 0, 0], &legend()).is_none());
        assert!(
            decode(&[1, 1, u32::MAX, 0], &legend()).is_none(),
            "no overflow"
        );
        assert_eq!(decode(&answer(&[]), &legend()).unwrap().len(), 0);
        // A start column that would overflow ends the decoding instead of panicking.
        let overflow = [0, u32::MAX, 1, 0, 0, 0, 5, 1, 0, 0];
        assert!(decode(&answer(&overflow), &legend()).is_none());
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
