use super::*;
pub(in crate::app) fn document_uri(
    id: DocumentId,
    document: &Document,
) -> ServiceResult<DocumentUri> {
    document
        .buffer
        .path()
        .map_or_else(|| Ok(DocumentUri::untitled(id)), DocumentUri::from_path)
}
pub(in crate::app) fn snapshot_document(
    id: DocumentId,
    document: &Document,
) -> ServiceResult<DocumentSnapshot> {
    Ok(DocumentSnapshot {
        id: Some(id),
        uri: document_uri(id, document)?,
        text: document.buffer.text(),
        version: Some(document.buffer.revision()),
        language: language(document.language),
        dirty: document.is_dirty(),
        read_only: document.buffer.is_read_only(),
    })
}
fn language(language: Option<Language>) -> Option<String> {
    language.map(|lang| {
        match lang {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::Json => "json",
        }
        .into()
    })
}
impl App {
    pub(crate) fn service_document_id(&self, uri: &DocumentUri) -> Option<DocumentId> {
        if uri.is_untitled() {
            self.documents
                .iter()
                .find(|d| DocumentUri::untitled(d.id) == *uri && d.buffer.path().is_none())
                .map(|d| d.id)
        } else {
            uri.to_path()
                .ok()
                .and_then(|path| self.documents.id_for_path(&path))
        }
    }
}
impl DocumentService for App {
    fn subscribe_documents(&mut self) -> ServiceResult<DocumentSubscription> {
        self.reconcile_document_events();
        self.document_events.subscribe()
    }
    fn documents(&self) -> ServiceResult<Vec<DocumentSnapshot>> {
        self.documents
            .iter()
            .map(|d| snapshot_document(d.id, &d))
            .collect()
    }
    fn snapshot(&self, uri: &DocumentUri) -> ServiceResult<DocumentSnapshot> {
        if let Some(id) = self.service_document_id(uri) {
            return snapshot_document(id, &self.documents[id]);
        }
        if uri.is_untitled() {
            return Err(ServiceError::NotFound);
        }
        let bytes = LocalFileSystem.read(uri)?;
        let decoded =
            crate::file_format::decode(&bytes, &self.translator).map_err(ServiceError::Failed)?;
        let path = uri.to_path()?;
        let relative = path.strip_prefix(self.tree.root()).unwrap_or(&path);
        let configured = self
            .read_only_patterns
            .iter()
            .any(|p| globset::Glob::new(p).is_ok_and(|g| g.compile_matcher().is_match(relative)));
        Ok(DocumentSnapshot {
            id: None,
            uri: DocumentUri::from_path(&path)?,
            text: decoded.text,
            version: None,
            language: language(Language::from_path(&path)),
            dirty: false,
            read_only: configured || LocalFileSystem.read_only(uri)?,
        })
    }
    fn apply_edits(&mut self, edits: Vec<DocumentEdit>) -> ServiceResult<()> {
        let prepared = self.prepare_document_edits(edits).map_err(edit_error)?;
        self.commit_workspace_edit(prepared).map_err(edit_error)?;
        self.reconcile_document_events();
        Ok(())
    }
}
