use crate::{DocumentId, DocumentSnapshot, TextEdit};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveReason {
    Manual,
    SaveAll,
    Close,
    Workspace,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveRequest {
    pub uri: crate::DocumentUri,
    pub reason: SaveReason,
}
pub trait SaveService {
    /// Completes with a serialized SaveOutcome; no event-loop blocking.
    fn save(
        &mut self,
        request: SaveRequest,
        cancellation: crate::CancellationToken,
    ) -> crate::CommandTicket;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveParticipantMetadata {
    pub id: String,
    /// Empty matches all languages. This API currently saves local file documents.
    pub languages: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveParticipantRequest {
    pub invocation: u64,
    pub reason: SaveReason,
    /// The version is the real document version; invocation identifies the staged text.
    pub document: DocumentSnapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveParticipantResult {
    pub invocation: u64,
    pub version: u64,
    pub edits: Vec<TextEdit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveOutcome {
    pub document: DocumentId,
    pub version: u64,
    pub dirty: bool,
    pub warnings: Vec<String>,
}
