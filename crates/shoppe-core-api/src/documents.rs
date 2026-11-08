use crate::{DocumentUri, ServiceResult};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub u64);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub id: Option<DocumentId>,
    pub uri: DocumentUri,
    pub text: String,
    pub version: Option<u64>,
    pub language: Option<String>,
    pub dirty: bool,
    pub read_only: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextEdit {
    pub start: Position,
    pub end: Position,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentEdit {
    pub uri: DocumentUri,
    pub version: Option<u64>,
    pub edits: Vec<TextEdit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "document", rename_all = "camelCase")]
pub enum DocumentEvent {
    Opened(DocumentSnapshot),
    Changed(DocumentSnapshot),
    Closed(DocumentSnapshot),
}
pub struct DocumentSubscription {
    pub id: u64,
    pub receiver: std::sync::mpsc::Receiver<DocumentEvent>,
    pub cancellation: crate::CancellationToken,
    pub overflow: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for DocumentSubscription {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
pub trait DocumentService {
    fn subscribe_documents(&mut self) -> ServiceResult<DocumentSubscription>;
    fn documents(&self) -> ServiceResult<Vec<DocumentSnapshot>>;
    fn snapshot(&self, uri: &DocumentUri) -> ServiceResult<DocumentSnapshot>;
    fn apply_edits(&mut self, edits: Vec<DocumentEdit>) -> ServiceResult<()>;
}

/// Strict UTF-16 position conversion, rejecting surrogate halves and CRLF interiors.
pub fn position_to_char(text: &str, position: Position) -> Option<usize> {
    let mut offset = 0;
    let mut lines = text.split('\n');
    for _ in 0..position.line {
        offset += lines.next()?.chars().count() + 1;
    }
    let line = lines.next()?;
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut utf16 = 0;
    for ch in line.chars() {
        if utf16 == position.character {
            return Some(offset);
        }
        utf16 += ch.len_utf16() as u32;
        if utf16 > position.character {
            return None;
        }
        offset += 1;
    }
    (utf16 == position.character).then_some(offset)
}
