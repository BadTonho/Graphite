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
pub use sse::{SseEvent, SseParser};
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
