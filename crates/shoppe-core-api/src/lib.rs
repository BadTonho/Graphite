//! UI-independent contracts shared by protocol adapters and the editor.
mod commands;
mod documents;
pub mod protocol;
mod uri;
pub use commands::*;
pub use documents::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use uri::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum ServiceError {
    InvalidArguments(String),
    UnsupportedUri(String),
    NotFound,
    ReadOnly,
    StaleDocument,
    InvalidRange,
    Cancelled,
    Timeout,
    Busy,
    HostStopped,
    Protocol(String),
    Io(String),
    Failed(String),
    Command(CommandError),
}
impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => f.write_str("target is read-only"),
            Self::InvalidRange => f.write_str("invalid or overlapping text range"),
            Self::StaleDocument => f.write_str("stale or unavailable document version"),
            Self::InvalidArguments(s)
            | Self::UnsupportedUri(s)
            | Self::Protocol(s)
            | Self::Io(s)
            | Self::Failed(s) => f.write_str(s),
            Self::Command(error) => std::fmt::Display::fmt(error, f),
            other => write!(f, "{other:?}"),
        }
    }
}
impl std::error::Error for ServiceError {}
pub type ServiceResult<T> = Result<T, ServiceError>;

pub trait WorkspaceService {
    fn workspace(&self) -> ServiceResult<WorkspaceSnapshot>;
    fn configuration(&self) -> ServiceResult<Value>;
}
pub trait WindowService {
    fn open_document(&mut self, uri: &DocumentUri) -> ServiceResult<DocumentSnapshot>;
    fn focus_view(&mut self, view: u64) -> ServiceResult<()>;
    fn show_message(&mut self, message: String) -> ServiceResult<()>;
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub root: DocumentUri,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServiceRequest {
    Documents,
    Snapshot { uri: DocumentUri },
    ApplyEdits { edits: Vec<DocumentEdit> },
    Commands,
    Execute(CommandRequest),
    Workspace,
    Configuration,
    Open { uri: DocumentUri },
    Focus { view: u64 },
    Message { text: String },
}
