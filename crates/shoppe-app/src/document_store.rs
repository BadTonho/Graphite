use super::{same_file_path, DocumentId, Tab};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::Path,
};

/// Owns open documents and keeps stable identities separate from presentation order.
pub(crate) struct DocumentStore {
    documents: Vec<Tab>,
    indices: HashMap<DocumentId, usize>,
}

impl DocumentStore {
    pub fn new(document: Tab) -> Self {
        Self {
            indices: HashMap::from([(document.id, 0)]),
            documents: vec![document],
        }
    }

    pub fn index_for(&self, id: DocumentId) -> Option<usize> {
        self.indices.get(&id).copied()
    }

    pub fn index_for_path(&self, path: &Path) -> Option<usize> {
        self.documents.iter().position(|document| {
            document
                .buffer
                .path()
                .is_some_and(|open| same_file_path(open, path))
        })
    }

    pub fn push(&mut self, document: Tab) {
        assert!(
            !self.indices.contains_key(&document.id),
            "duplicate document identity"
        );
        self.indices.insert(document.id, self.documents.len());
        self.documents.push(document);
    }

    pub fn remove(&mut self, index: usize) -> Tab {
        let document = self.documents.remove(index);
        self.indices.remove(&document.id);
        for (position, document) in self.documents.iter().enumerate().skip(index) {
            self.indices.insert(document.id, position);
        }
        document
    }

    pub fn replace(&mut self, index: usize, document: Tab) -> Tab {
        assert!(
            !self.indices.contains_key(&document.id) || self.documents[index].id == document.id,
            "duplicate document identity"
        );
        let previous = std::mem::replace(&mut self.documents[index], document);
        self.indices.remove(&previous.id);
        self.indices.insert(self.documents[index].id, index);
        previous
    }
}

impl Deref for DocumentStore {
    type Target = [Tab];
    fn deref(&self) -> &Self::Target {
        &self.documents
    }
}

impl DerefMut for DocumentStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.documents
    }
}

impl<'a> IntoIterator for &'a DocumentStore {
    type Item = &'a Tab;
    type IntoIter = std::slice::Iter<'a, Tab>;
    fn into_iter(self) -> Self::IntoIter {
        self.documents.iter()
    }
}
