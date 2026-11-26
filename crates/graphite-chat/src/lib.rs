//! Model service access, sessions, tools, and agent loop for Graphite's chat.
//!
//! One provider-neutral conversation model ([`Message`], [`Request`], [`StreamEvent`]) is sent
//! to either the Anthropic Messages API or any OpenAI-compatible server, and the streamed reply
//! comes back as the same events.

pub mod agent;
pub mod anthropic;
mod command;
mod message;
pub mod openai;
mod provider;
pub mod session;
mod sse;
mod store;
pub mod tools;

pub use agent::{
    build_system_prompt, run_turn, run_turn_with_tools, write_proposed_edit, ActiveFileContext,
    AgentTurnEvent, ChatContext, FileAttachment, HttpModelStreamer, ModelStreamer,
};
pub use command::{SystemCommandRunner, DEFAULT_COMMAND_TIMEOUT, MAX_COMMAND_OUTPUT_BYTES};
pub use message::{
    Block, ChatError, Message, Request, Role, StopReason, StreamEvent, ToolSpec, Usage,
};
pub use provider::{stream, ProviderConfig, ProviderKind};
pub use session::{
    extract_code_blocks, AgentLogEntry, AgentMode, ChatSession, ChatTurn, Checkpoint,
    CheckpointEntry, CodeBlock, EditReviewStatus, PendingFileEdit, TurnStatus, UsageSummary,
};
pub use sse::{SseEvent, SseOverflow, SseParser, MAX_SSE_EVENT_BYTES};
pub use store::{
    delete_session, load_approval_policy, load_sessions, save_approval_policy, save_session,
    APPROVAL_POLICY_FILE, SESSIONS_DIR,
};
pub use tools::{
    execute_tool, resolve_workspace_path, summarize_tool_call, tool_specs_for_mode,
    ApprovalDecision, CommandRunner, PendingToolApproval, ProposedEdit, ToolApprovalPolicy,
    ToolExecutionResult, WorkspaceApprovals, TOOL_LIST_FILES, TOOL_PROPOSE_EDIT, TOOL_READ_FILE,
    TOOL_RUN_COMMAND, TOOL_SEARCH_WORKSPACE,
};

/// Taken by tests that start processes and by tests that serve HTTP, so the two never overlap.
/// On Windows a process started while a test server holds an accepted connection inherits that
/// socket and keeps the connection open after the server closes it, which resets it later.
/// The guard can move to the server thread, which holds it until the connection is closed.
#[cfg(test)]
pub(crate) fn process_and_socket_test_lock() -> TestLockGuard {
    let mut busy = TEST_LOCK
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while *busy {
        busy = TEST_LOCK
            .1
            .wait(busy)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
    *busy = true;
    TestLockGuard
}

#[cfg(test)]
static TEST_LOCK: (std::sync::Mutex<bool>, std::sync::Condvar) =
    (std::sync::Mutex::new(false), std::sync::Condvar::new());

/// Releases [`process_and_socket_test_lock`] when dropped.
#[cfg(test)]
pub(crate) struct TestLockGuard;

#[cfg(test)]
impl Drop for TestLockGuard {
    fn drop(&mut self) {
        *TEST_LOCK
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
        TEST_LOCK.1.notify_one();
    }
}
