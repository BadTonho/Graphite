//! UI-independent contracts shared by protocol adapters and the editor.
mod commands;
mod documents;
mod saving;
mod ui;
pub use saving::*;
pub use ui::*;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageScope {
    Global,
    Workspace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickItem {
    pub label: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServiceRequest {
    Documents,
    Snapshot {
        uri: DocumentUri,
    },
    ApplyEdits {
        edits: Vec<DocumentEdit>,
    },
    Commands,
    Execute(CommandRequest),
    Save(SaveRequest),
    Workspace,
    Configuration,
    Open {
        uri: DocumentUri,
    },
    Focus {
        view: u64,
    },
    Message {
        text: String,
    },
    PromptInput {
        #[serde(default)]
        prompt: Option<String>,
        #[serde(default)]
        place_holder: Option<String>,
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        password: bool,
    },
    QuickPick {
        items: Vec<QuickPickItem>,
        #[serde(default)]
        place_holder: Option<String>,
        #[serde(default)]
        can_pick_many: bool,
    },
    SetStatusBarItem {
        id: String,
        text: String,
        #[serde(default)]
        command: Option<String>,
    },
    DisposeStatusBarItem {
        id: String,
    },
    CreateOutputChannel {
        name: String,
    },
    AppendOutput {
        channel: String,
        text: String,
        #[serde(default)]
        replace: bool,
    },
    ShowOutput {
        channel: String,
    },
    StorageGet {
        scope: StorageScope,
        #[serde(default)]
        key: Option<String>,
    },
    StorageSet {
        scope: StorageScope,
        key: String,
        #[serde(default)]
        value: Option<Value>,
    },
    RegisterTreeDataProvider {
        #[serde(rename = "viewId", alias = "view_id")]
        view_id: String,
    },
    RefreshTreeView {
        #[serde(rename = "viewId", alias = "view_id")]
        view_id: String,
        #[serde(rename = "elementId", alias = "element_id", default)]
        element_id: Option<String>,
    },
    CreateWebviewPanel {
        handle: String,
        #[serde(rename = "viewType", alias = "view_type")]
        view_type: String,
        title: String,
        #[serde(default)]
        html: String,
    },
    SetWebviewHtml {
        handle: String,
        html: String,
    },
    SetWebviewTitle {
        handle: String,
        title: String,
    },
    PostWebviewMessage {
        handle: String,
        message: Value,
    },
    DisposeWebviewPanel {
        handle: String,
    },
}
