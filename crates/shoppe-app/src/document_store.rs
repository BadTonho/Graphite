//! Stable document ownership. Presentation order belongs to editor groups.
use super::{next_document_id, same_file_path, Document, DocumentId};
use std::{
    collections::HashMap,
    ops::{Deref, Index, IndexMut},
    path::Path,
    sync::Arc,
};

pub(crate) struct ClosedDocument {
    pub id: DocumentId,
    pub document: Document,
}
pub(crate) struct DocumentRef<'a> {
    pub id: DocumentId,
    document: &'a Document,
}
impl Deref for DocumentRef<'_> {
    type Target = Document;
    fn deref(&self) -> &Document {
        self.document
    }
}

pub(crate) struct DocumentLease {
    _retainer: Arc<()>,
}
pub(crate) struct DocumentStore {
    documents: HashMap<DocumentId, Document>,
    order: Vec<DocumentId>,
    retainers: HashMap<DocumentId, Arc<()>>,
}
impl DocumentStore {
    pub fn new(id: DocumentId, document: Document) -> Self {
        let mut store = Self {
            documents: HashMap::new(),
            order: Vec::new(),
            retainers: HashMap::new(),
        };
        store.reopen(ClosedDocument { id, document });
        store
    }
    pub fn lease(&self, id: DocumentId) -> Option<DocumentLease> {
        self.retainers.get(&id).map(|retainer| DocumentLease {
            _retainer: retainer.clone(),
        })
    }
    pub fn is_retained(&self, id: DocumentId) -> bool {
        self.retainers
            .get(&id)
            .is_some_and(|retainer| Arc::strong_count(retainer) > 1)
    }
    pub fn len(&self) -> usize {
        self.documents.len()
    }
    pub fn ids(&self) -> impl Iterator<Item = DocumentId> + '_ {
        self.order.iter().copied()
    }
    pub fn resolve(&self, id: DocumentId) -> Option<DocumentId> {
        self.documents.contains_key(&id).then_some(id)
    }
    pub fn id_for_path(&self, path: &Path) -> Option<DocumentId> {
        self.iter()
            .find(|d| d.buffer.path().is_some_and(|p| same_file_path(p, path)))
            .map(|d| d.id)
    }
    pub fn get(&self, id: DocumentId) -> Option<&Document> {
        self.documents.get(&id)
    }
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.documents.get_mut(&id)
    }
    pub fn iter(&self) -> impl Iterator<Item = DocumentRef<'_>> {
        self.order.iter().map(|id| DocumentRef {
            id: *id,
            document: &self.documents[id],
        })
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (DocumentId, &mut Document)> {
        self.documents.iter_mut().map(|(id, d)| (*id, d))
    }
    pub fn push(&mut self, document: Document) -> DocumentId {
        let id = next_document_id();
        self.reopen(ClosedDocument { id, document });
        id
    }
    pub fn reopen(&mut self, closed: ClosedDocument) -> DocumentId {
        assert!(
            !self.documents.contains_key(&closed.id),
            "duplicate document identity"
        );
        let id = closed.id;
        self.retainers.insert(id, Arc::new(()));
        self.documents.insert(id, closed.document);
        self.order.push(id);
        id
    }
    pub fn replace(&mut self, id: DocumentId, document: Document) -> (DocumentId, ClosedDocument) {
        let closed = self.remove(id);
        let id = self.push(document);
        (id, closed)
    }
    pub fn remove(&mut self, id: DocumentId) -> ClosedDocument {
        assert!(!self.is_retained(id), "pending operation retains document");
        self.retainers.remove(&id);
        let document = self.documents.remove(&id).expect("document exists");
        self.order.retain(|entry| *entry != id);
        ClosedDocument { id, document }
    }
}
impl Index<DocumentId> for DocumentStore {
    type Output = Document;
    fn index(&self, id: DocumentId) -> &Document {
        &self.documents[&id]
    }
}
impl IndexMut<DocumentId> for DocumentStore {
    fn index_mut(&mut self, id: DocumentId) -> &mut Document {
        self.documents.get_mut(&id).expect("document exists")
    }
}
