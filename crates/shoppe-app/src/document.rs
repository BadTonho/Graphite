//! Shared document data; no cursor, selection, scroll or other view state.
use super::{file_format, Buffer, HighlightCache, IncrementalHighlighter, Language};

pub(crate) struct Document {
    pub(crate) buffer: Buffer,
    pub(crate) language: Option<Language>,
    pub(crate) highlights: Option<HighlightCache>,
    pub(crate) sticky_headers_cache: Option<(u64, usize, Vec<shoppe_syntax::ScopeHeader>)>,
    pub(crate) highlighter: IncrementalHighlighter,
    pub(crate) diagnostics: Vec<lsp_types::Diagnostic>,
    pub(crate) inlay_hints: Vec<lsp_types::InlayHint>,
    pub(crate) inlay_revision: Option<u64>,
    pub(crate) schema_diagnostics: Vec<lsp_types::Diagnostic>,
    pub(crate) schema_revision: Option<u64>,
    pub(crate) lsp_revision: Option<u64>,
    pub(crate) indentation: shoppe_text::Indentation,
    pub(crate) saved_format: (file_format::Encoding, file_format::LineEnding),
    pub(crate) encoding: file_format::Encoding,
    pub(crate) line_ending: file_format::LineEnding,
}

impl Document {
    pub(crate) fn is_dirty(&self) -> bool {
        self.buffer.is_dirty() || self.saved_format != (self.encoding, self.line_ending)
    }
}
