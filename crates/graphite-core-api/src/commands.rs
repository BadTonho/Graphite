use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::Receiver,
    Arc,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum CommandTarget {
    #[default]
    Global,
    View(u64),
    Widget(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetKind {
    Global,
    View,
    Widget,
}
impl CommandTarget {
    pub fn kind(self) -> TargetKind {
        match self {
            Self::Global => TargetKind::Global,
            Self::View(_) => TargetKind::View,
            Self::Widget(_) => TargetKind::Widget,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandRequest {
    pub id: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub target: CommandTarget,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum CommandError {
    UnknownCommand(String),
    InvalidArguments(String),
    InvalidTarget(String),
    TargetClosed,
    Disabled(String),
    Cancelled,
    Failed(String),
    StaleDocument,
    Timeout,
    Busy,
    HostStopped,
}
impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCommand(id) => write!(f, "Unknown command: {id}"),
            Self::InvalidArguments(message)
            | Self::InvalidTarget(message)
            | Self::Failed(message) => f.write_str(message),
            Self::Disabled(id) => write!(f, "Command disabled: {id}"),
            Self::TargetClosed => f.write_str("Command target closed"),
            Self::Cancelled => f.write_str("Command cancelled"),
            Self::StaleDocument => f.write_str("Document changed while command was pending"),
            Self::Timeout => f.write_str("Command timed out"),
            Self::Busy => f.write_str("Too many pending commands"),
            Self::HostStopped => f.write_str("Extension host stopped"),
        }
    }
}
pub type CommandResult = Result<Value, CommandError>;
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), CommandError> {
        if self.is_cancelled() {
            Err(CommandError::Cancelled)
        } else {
            Ok(())
        }
    }
}
pub struct CommandTicket {
    pub id: u64,
    pub cancellation: CancellationToken,
    pub receiver: Receiver<CommandResult>,
}
impl CommandTicket {
    pub fn try_result(&self) -> Option<CommandResult> {
        self.receiver.try_recv().ok()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub id: String,
    pub title: String,
    pub category: String,
    pub targets: Vec<TargetKind>,
    pub enablement: String,
}
pub trait CommandService {
    fn commands(&self) -> Vec<CommandMetadata>;
    fn execute(&mut self, request: CommandRequest) -> CommandTicket;
    fn cancel(&mut self, execution: u64);
}
