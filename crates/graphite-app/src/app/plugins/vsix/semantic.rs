//! Semantic tokens from extensions (`registerDocumentSemanticTokensProvider`).
//!
//! The provider describes the text as a flat list of `(line, column, length, type, modifiers)`
//! tokens, delta-encoded. Graphite decodes them (see `crate::semantic_tokens`), maps each token
//! type to the syntax class the editor already knows how to color, and draws them over the
//! tree-sitter highlighting, as VS Code does. Tokens describe one revision of the text; after an
//! edit the syntax colors show until the provider answers again.
//!
//! Only whole-document "full" answers are understood. Delta answers and range providers are not
//! used.

use super::languages::Feature;
use super::*;
use crate::semantic_tokens::{self, Tokens};
use graphite_core_api::DocumentSnapshot;
use graphite_syntax::Span;

/// The semantic tokens of the active document at one revision.
pub(crate) struct SemanticTokens {
    pub uri: String,
    pub revision: u64,
    lines: Tokens,
}

/// Decodes the words of a full-document answer: `[resultId, kind (1 = full), count, data...]`.
/// Returns `None` for any other kind, a truncated list or an overflowing position.
pub(super) fn decode(words: &[u32], legend: &[String]) -> Option<Tokens> {
    const FULL: u32 = 1;
    if words.len() < 3 || words[1] != FULL {
        return None;
    }
    let count = words[2] as usize;
    let data = words.get(3..3usize.checked_add(count)?)?;
    semantic_tokens::decode_data(data, legend)
}

impl Runtime {
    /// The legend (token type names) of the semantic token provider with this handle.
    fn semantic_legend(&self, handle: u64) -> Vec<String> {
        self.languages.legend(handle)
    }
}

impl App {
    /// Semantic spans to draw over the syntax colors of `line`, when the tokens describe this
    /// revision of the document with this URI. Language server tokens are used when no extension
    /// provides any.
    pub(crate) fn semantic_spans(
        &self,
        uri: &str,
        revision: u64,
        line: usize,
        text: &str,
    ) -> Vec<Span> {
        let from_extension = self
            .plugins
            .vsix
            .as_ref()
            .and_then(|runtime| runtime.passive.semantic.as_ref())
            .filter(|tokens| tokens.uri == uri && tokens.revision == revision)
            .and_then(|tokens| tokens.lines.get(&line));
        let from_server = self
            .lsp_semantic
            .as_ref()
            .filter(|tokens| tokens.uri == uri && tokens.revision == revision)
            .and_then(|tokens| tokens.lines.get(&line));
        from_extension
            .or(from_server)
            .map(|tokens| semantic_tokens::byte_spans(text, tokens))
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
            revision: request.key.revision,
            lines,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_syntax::Class;

    fn legend() -> Vec<String> {
        ["keyword", "variable"].map(String::from).into()
    }

    /// `[resultId, full, count, data...]`
    fn answer(data: &[u32]) -> Vec<u32> {
        let mut words = vec![7, 1, data.len() as u32];
        words.extend_from_slice(data);
        words
    }

    #[test]
    fn a_full_answer_is_decoded_after_its_header() {
        let tokens = decode(&answer(&[0, 0, 3, 0, 0, 0, 4, 1, 1, 0]), &legend()).unwrap();
        assert_eq!(
            tokens[&0],
            [(0, 3, Class::Keyword), (4, 1, Class::Variable)]
        );
        assert_eq!(decode(&answer(&[]), &legend()).unwrap().len(), 0);
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
        // A start column that would overflow ends the decoding instead of panicking.
        let overflow = [0, u32::MAX, 1, 0, 0, 0, 5, 1, 0, 0];
        assert!(decode(&answer(&overflow), &legend()).is_none());
    }
}
