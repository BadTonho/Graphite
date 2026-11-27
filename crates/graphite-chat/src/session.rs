//! Chat session state, turns, checkpoints, edit staging, and export/import.

use crate::message::{Block, Message, Role, Usage};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// What the assistant is permitted to do in a session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Ask,
    Edit,
    #[default]
    Agent,
}

impl AgentMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::Edit => "Edit",
            Self::Agent => "Agent",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Ask => Self::Edit,
            Self::Edit => Self::Agent,
            Self::Agent => Self::Ask,
        }
    }
}

/// Lifecycle state of one user-prompt turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TurnStatus {
    Streaming,
    WaitingApproval,
    Done,
    Error { message: String },
    Cancelled,
    Interrupted,
}

impl TurnStatus {
    pub fn can_continue(&self) -> bool {
        matches!(
            self,
            Self::Error { .. } | Self::Cancelled | Self::Interrupted
        )
    }
}

/// Status of one command associated with a tool-call identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandActivity {
    pub id: String,
    pub status: crate::CommandStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileActivityStatus {
    Proposed,
    Applied,
    Discarded,
    Conflict,
    Replaced,
    Reverted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileActivity {
    pub path: PathBuf,
    pub status: FileActivityStatus,
}

/// Extracted fenced code block from an assistant reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    pub language: String,
    pub code: String,
}

/// One user prompt and the model/tool exchange it produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatTurn {
    pub id: u64,
    pub prompt: String,
    /// Alternating assistant replies (`Role::Assistant`) and tool result batches (`Role::User`).
    #[serde(default)]
    pub steps: Vec<Message>,
    #[serde(default)]
    pub usage: UsageSummary,
    pub status: TurnStatus,
    #[serde(default)]
    pub helpful: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandActivity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileActivity>,
    #[serde(default)]
    pub created_at_epoch_s: u64,
}

/// Serializable token usage summary for a turn or session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSummary {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl From<Usage> for UsageSummary {
    fn from(usage: Usage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
        }
    }
}

impl ChatTurn {
    pub fn assistant_text(&self) -> String {
        let mut parts = Vec::new();
        for msg in &self.steps {
            if msg.role == Role::Assistant {
                let text = msg.text();
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
        parts.join("\n\n")
    }

    /// Extracts fenced Markdown code blocks (` ```lang ... ``` `) from the assistant reply.
    pub fn code_blocks(&self) -> Vec<CodeBlock> {
        extract_code_blocks(&self.assistant_text())
    }
}

/// Extracts fenced code blocks from Markdown text.
pub fn extract_code_blocks(markdown: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut in_fence = false;
    let mut lang = String::new();
    let mut lines: Vec<&str> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            if !in_fence {
                in_fence = true;
                lang = rest.trim().to_owned();
                lines.clear();
            } else {
                blocks.push(CodeBlock {
                    language: std::mem::take(&mut lang),
                    code: lines.join("\n"),
                });
                lines.clear();
                in_fence = false;
            }
        } else if in_fence {
            lines.push(line);
        }
    }
    if in_fence && !lines.is_empty() {
        blocks.push(CodeBlock {
            language: lang,
            code: lines.join("\n"),
        });
    }
    blocks
}

/// Review state of a file edit proposed during chat/agent execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditReviewStatus {
    Pending,
    Accepted,
    Discarded,
}

/// A workspace file change staged or applied by the assistant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingFileEdit {
    pub path: PathBuf,
    /// `None` if the file did not exist before the edit.
    pub before: Option<String>,
    pub after: String,
    pub status: EditReviewStatus,
    pub checkpoint_id: u64,
}

/// A proposal awaiting an explicit user decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileProposal {
    pub id: u64,
    pub turn_id: u64,
    pub workspace: PathBuf,
    pub path: PathBuf,
    pub before: Option<String>,
    pub fingerprint: Option<[u8; 32]>,
    pub after: String,
    #[serde(skip)]
    pub document: Option<(u64, u64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

/// Exact disk snapshots for a reviewed edit, including the original encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewedFile {
    pub workspace: PathBuf,
    pub before: Option<Vec<u8>>,
    pub after: Vec<u8>,
}

/// Snapshot of files before an edit batch so checkpoints and undo/redo work deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: u64,
    pub turn_id: u64,
    pub label: String,
    /// `(path, previous_content, next_content)`
    pub entries: Vec<CheckpointEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointEntry {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<ReviewedFile>,
}

/// Diagnostic trace entry recorded for agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLogEntry {
    pub timestamp_epoch_s: u64,
    pub kind: String,
    pub detail: String,
}

/// A full chat/agent session, serializable for history, export/import, and recovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opencode_session: Option<String>,
    #[serde(default)]
    pub mode: AgentMode,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub unread: bool,
    pub created_at_epoch_s: u64,
    pub updated_at_epoch_s: u64,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub turns: Vec<ChatTurn>,
    #[serde(default)]
    pub pending_edits: Vec<PendingFileEdit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposals: Vec<FileProposal>,
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    pub undone_checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    pub input_history: Vec<String>,
    #[serde(default)]
    pub debug_log: Vec<AgentLogEntry>,
    #[serde(default = "default_next_id")]
    next_turn_id: u64,
    #[serde(default = "default_next_id")]
    next_checkpoint_id: u64,
}

fn default_next_id() -> u64 {
    1
}

pub fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl ChatSession {
    pub fn new(id: impl Into<String>, title: impl Into<String>, mode: AgentMode) -> Self {
        let now = now_epoch_s();
        Self {
            id: id.into(),
            title: title.into(),
            connection_id: None,
            connection_destination: None,
            opencode_session: None,
            mode,
            pinned: false,
            archived: false,
            unread: false,
            created_at_epoch_s: now,
            updated_at_epoch_s: now,
            summary: None,
            turns: Vec::new(),
            pending_edits: Vec::new(),
            proposals: Vec::new(),
            checkpoints: Vec::new(),
            undone_checkpoints: Vec::new(),
            input_history: Vec::new(),
            debug_log: Vec::new(),
            next_turn_id: 1,
            next_checkpoint_id: 1,
        }
    }

    pub fn log(&mut self, kind: impl Into<String>, detail: impl Into<String>) {
        self.debug_log.push(AgentLogEntry {
            timestamp_epoch_s: now_epoch_s(),
            kind: kind.into(),
            detail: detail.into(),
        });
        if self.debug_log.len() > 500 {
            let drain = self.debug_log.len() - 500;
            self.debug_log.drain(0..drain);
        }
    }

    /// Adds a new user turn, updates the session title if still default, and records prompt history.
    pub fn start_turn(&mut self, prompt: impl Into<String>) -> u64 {
        let prompt = prompt.into();
        let trimmed = prompt.trim();
        if !trimmed.is_empty() && self.input_history.last().map(String::as_str) != Some(trimmed) {
            self.input_history.push(trimmed.to_owned());
        }
        if (self.title.is_empty() || self.title == "New Chat") && !trimmed.is_empty() {
            let first_line = trimmed.lines().next().unwrap_or(trimmed);
            let mut title: String = first_line.chars().take(48).collect();
            if first_line.chars().count() > 48 {
                title.push_str("...");
            }
            self.title = title;
        }
        let id = self.next_turn_id;
        self.next_turn_id += 1;
        let now = now_epoch_s();
        self.updated_at_epoch_s = now;
        self.turns.push(ChatTurn {
            id,
            prompt,
            steps: Vec::new(),
            usage: UsageSummary::default(),
            status: TurnStatus::Streaming,
            workspace: None,
            commands: Vec::new(),
            files: Vec::new(),
            helpful: None,
            created_at_epoch_s: now,
        });
        id
    }

    pub fn turn_mut(&mut self, turn_id: u64) -> Option<&mut ChatTurn> {
        self.turns.iter_mut().find(|turn| turn.id == turn_id)
    }

    /// Appends streamed text to the current assistant step of `turn_id`.
    pub fn append_text(&mut self, turn_id: u64, delta: &str) {
        self.updated_at_epoch_s = now_epoch_s();
        let Some(turn) = self.turn_mut(turn_id) else {
            return;
        };
        let needs_new_assistant = !matches!(
            turn.steps.last(),
            Some(Message {
                role: Role::Assistant,
                ..
            })
        );
        if needs_new_assistant {
            turn.steps.push(Message::assistant(vec![Block::Text {
                text: delta.to_owned(),
            }]));
            return;
        }
        if let Some(last) = turn.steps.last_mut() {
            if let Some(Block::Text { text }) = last.blocks.last_mut() {
                text.push_str(delta);
            } else {
                last.blocks.push(Block::Text {
                    text: delta.to_owned(),
                });
            }
        }
    }

    /// Records a completed tool invocation block on the active assistant step.
    pub fn record_tool_use(
        &mut self,
        turn_id: u64,
        id: String,
        name: String,
        input: serde_json::Value,
    ) {
        self.updated_at_epoch_s = now_epoch_s();
        let Some(turn) = self.turn_mut(turn_id) else {
            return;
        };
        let block = Block::ToolUse { id, name, input };
        if let Some(last) = turn.steps.last_mut() {
            if last.role == Role::Assistant {
                last.blocks.push(block);
                return;
            }
        }
        turn.steps.push(Message::assistant(vec![block]));
    }

    /// Records a tool execution result batch as a `Role::User` tool-result message.
    pub fn record_tool_result(
        &mut self,
        turn_id: u64,
        id: String,
        content: String,
        is_error: bool,
    ) {
        self.updated_at_epoch_s = now_epoch_s();
        let Some(turn) = self.turn_mut(turn_id) else {
            return;
        };
        let block = Block::ToolResult {
            id,
            content,
            is_error,
        };
        if let Some(last) = turn.steps.last_mut() {
            if last.role == Role::User {
                last.blocks.push(block);
                return;
            }
        }
        turn.steps.push(Message::tool_results(vec![block]));
    }

    pub fn recover_interrupted_turns(&mut self) -> bool {
        let mut changed = false;
        for turn in &mut self.turns {
            if matches!(
                turn.status,
                TurnStatus::Streaming | TurnStatus::WaitingApproval
            ) {
                turn.status = TurnStatus::Interrupted;
                changed = true;
            }
        }
        changed
    }

    pub fn record_command_status(
        &mut self,
        turn_id: u64,
        id: String,
        status: crate::CommandStatus,
    ) -> bool {
        let Some(turn) = self.turn_mut(turn_id) else {
            return false;
        };
        if let Some(command) = turn.commands.iter_mut().find(|command| command.id == id) {
            if command.status != crate::CommandStatus::Started {
                return false;
            }
            command.status = status;
        } else {
            turn.commands.push(CommandActivity { id, status });
        }
        true
    }

    pub fn record_file_status(&mut self, turn_id: u64, path: &Path, status: FileActivityStatus) {
        if let Some(turn) = self.turn_mut(turn_id) {
            if let Some(file) = turn.files.iter_mut().find(|file| file.path == path) {
                file.status = status;
            } else {
                turn.files.push(FileActivity {
                    path: path.to_path_buf(),
                    status,
                });
            }
        }
    }

    pub fn queue_proposal(&mut self, mut proposal: FileProposal) {
        if let Some(previous_turn) = self
            .proposals
            .iter()
            .find(|entry| entry.path == proposal.path && entry.workspace == proposal.workspace)
            .map(|entry| entry.turn_id)
            .filter(|id| *id != proposal.turn_id)
        {
            self.record_file_status(previous_turn, &proposal.path, FileActivityStatus::Replaced);
        }
        self.record_file_status(
            proposal.turn_id,
            &proposal.path,
            FileActivityStatus::Proposed,
        );
        proposal.id = self.next_checkpoint_id;
        self.next_checkpoint_id += 1;
        if let Some(previous) = self
            .proposals
            .iter_mut()
            .find(|entry| entry.path == proposal.path && entry.workspace == proposal.workspace)
        {
            *previous = proposal;
        } else {
            self.proposals.push(proposal);
        }
        self.updated_at_epoch_s = now_epoch_s();
    }

    pub fn discard_proposal(&mut self, id: u64) -> bool {
        let Some(index) = self.proposals.iter().position(|proposal| proposal.id == id) else {
            return false;
        };
        let proposal = self.proposals.remove(index);
        self.record_file_status(
            proposal.turn_id,
            &proposal.path,
            FileActivityStatus::Discarded,
        );
        true
    }

    pub fn record_reviewed_file(&mut self, proposal: &FileProposal, reviewed: ReviewedFile) {
        self.proposals.retain(|entry| entry.id != proposal.id);
        self.record_file_status(
            proposal.turn_id,
            &proposal.path,
            FileActivityStatus::Applied,
        );
        if reviewed.before.as_ref() != Some(&reviewed.after) {
            self.checkpoints.push(Checkpoint {
                id: proposal.id,
                turn_id: proposal.turn_id,
                label: format!("Edit {}", proposal.path.display()),
                entries: vec![CheckpointEntry {
                    path: proposal.path.clone(),
                    before: proposal.before.clone(),
                    after: Some(proposal.after.clone()),
                    reviewed: Some(reviewed),
                }],
            });
            self.undone_checkpoints.clear();
        }
        self.updated_at_epoch_s = now_epoch_s();
    }

    /// Records an already-applied edit for compatibility with older session history.
    pub fn stage_edit(
        &mut self,
        turn_id: u64,
        path: PathBuf,
        before: Option<String>,
        after: String,
    ) -> u64 {
        let cp_id = self.next_checkpoint_id;
        self.next_checkpoint_id += 1;
        self.checkpoints.push(Checkpoint {
            id: cp_id,
            turn_id,
            label: format!("Edit {}", path.display()),
            entries: vec![CheckpointEntry {
                path: path.clone(),
                before: before.clone(),
                after: Some(after.clone()),
                reviewed: None,
            }],
        });
        self.undone_checkpoints.clear();

        if let Some(existing) = self
            .pending_edits
            .iter_mut()
            .find(|edit| edit.path == path && edit.status == EditReviewStatus::Pending)
        {
            existing.after = after;
            existing.checkpoint_id = cp_id;
        } else {
            self.pending_edits.push(PendingFileEdit {
                path,
                before,
                after,
                status: EditReviewStatus::Pending,
                checkpoint_id: cp_id,
            });
        }
        cp_id
    }

    pub fn accept_edit(&mut self, path: &Path) -> bool {
        let Some(edit) = self
            .pending_edits
            .iter_mut()
            .find(|e| e.path == path && e.status == EditReviewStatus::Pending)
        else {
            return false;
        };
        edit.status = EditReviewStatus::Accepted;
        true
    }

    pub fn discard_edit(&mut self, path: &Path) -> Option<Option<String>> {
        let edit = self
            .pending_edits
            .iter_mut()
            .find(|e| e.path == path && e.status == EditReviewStatus::Pending)?;
        edit.status = EditReviewStatus::Discarded;
        Some(edit.before.clone())
    }

    pub fn accept_all_edits(&mut self) -> usize {
        let mut count = 0;
        for edit in &mut self.pending_edits {
            if edit.status == EditReviewStatus::Pending {
                edit.status = EditReviewStatus::Accepted;
                count += 1;
            }
        }
        count
    }

    pub fn discard_all_edits(&mut self) -> Vec<(PathBuf, Option<String>)> {
        let mut reverted = Vec::new();
        for edit in &mut self.pending_edits {
            if edit.status == EditReviewStatus::Pending {
                edit.status = EditReviewStatus::Discarded;
                reverted.push((edit.path.clone(), edit.before.clone()));
            }
        }
        reverted
    }

    /// Pops the most recent checkpoint and moves it to `undone_checkpoints`.
    pub fn undo_checkpoint(&mut self) -> Option<Checkpoint> {
        let cp = self.checkpoints.pop()?;
        for edit in &mut self.pending_edits {
            if edit.checkpoint_id == cp.id && edit.status == EditReviewStatus::Pending {
                edit.status = EditReviewStatus::Discarded;
            }
        }
        self.undone_checkpoints.push(cp.clone());
        Some(cp)
    }

    /// Reapplies the last undone checkpoint.
    pub fn redo_checkpoint(&mut self) -> Option<Checkpoint> {
        let cp = self.undone_checkpoints.pop()?;
        for edit in &mut self.pending_edits {
            if edit.checkpoint_id == cp.id && edit.status == EditReviewStatus::Discarded {
                edit.status = EditReviewStatus::Pending;
            }
        }
        self.checkpoints.push(cp.clone());
        Some(cp)
    }

    /// Converts the session turns into provider-neutral messages for the next model request.
    pub fn to_request_messages(&self) -> Vec<Message> {
        let mut out = Vec::new();
        if let Some(summary) = &self.summary {
            if !summary.trim().is_empty() {
                out.push(Message::user(format!(
                    "[Earlier conversation summary]\n{summary}"
                )));
            }
        }
        for turn in &self.turns {
            if !turn.prompt.is_empty() {
                out.push(Message::user(&turn.prompt));
            }
            let mut steps = turn.steps.iter().peekable();
            while let Some(step) = steps.next() {
                out.push(step.clone());
                if step.role != Role::Assistant {
                    continue;
                }
                let mut results = Vec::new();
                if let Some(next) = steps.peek().filter(|next| next.role == Role::User) {
                    results.clone_from(&next.blocks);
                    steps.next();
                }
                for block in &step.blocks {
                    if let Block::ToolUse { id, .. } = block {
                        if !results.iter().any(|block| matches!(block, Block::ToolResult { id: received, .. } if received == id)) {
                            results.push(Block::ToolResult {
                                id: id.clone(),
                                content: "Interrupted before a result was recorded. Execution outcome is unknown.".into(),
                                is_error: true,
                            });
                        }
                    }
                }
                if !results.is_empty() {
                    out.push(Message::tool_results(results));
                }
            }
        }
        out
    }

    /// Compacts older turns into a single text summary while retaining the most recent `keep_recent` turns.
    pub fn compact_conversation(&mut self, keep_recent: usize) -> usize {
        if self.turns.len() <= keep_recent {
            return 0;
        }
        let split_at = self.turns.len() - keep_recent;
        let removed: Vec<ChatTurn> = self.turns.drain(0..split_at).collect();
        let mut parts = Vec::new();
        if let Some(existing) = self.summary.take() {
            if !existing.trim().is_empty() {
                parts.push(existing);
            }
        }
        for turn in &removed {
            let reply = turn.assistant_text();
            let snippet: String = reply.chars().take(200).collect();
            parts.push(format!(
                "- User: {}\n  Assistant: {}",
                turn.prompt.trim(),
                snippet.trim()
            ));
        }
        self.summary = Some(parts.join("\n"));
        self.updated_at_epoch_s = now_epoch_s();
        removed.len()
    }

    /// Estimates context token usage (~4 chars per token) across summary and turns.
    pub fn estimated_tokens(&self) -> u32 {
        let mut chars = self.summary.as_ref().map_or(0, String::len);
        for turn in &self.turns {
            chars += turn.prompt.len();
            for step in &turn.steps {
                for block in &step.blocks {
                    chars += match block {
                        Block::Text { text } => text.len(),
                        Block::ToolUse { name, input, .. } => name.len() + input.to_string().len(),
                        Block::ToolResult { content, .. } => content.len(),
                    };
                }
            }
        }
        ((chars as f64) / 4.0).ceil() as u32
    }

    pub fn export_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn import_json(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| e.to_string())
    }

    pub fn export_markdown(&self) -> String {
        let mut out = format!("# {}\n\n", self.title);
        for turn in &self.turns {
            out.push_str("## User\n\n");
            out.push_str(&turn.prompt);
            out.push_str("\n\n## Assistant\n\n");
            let reply = turn.assistant_text();
            if reply.is_empty() {
                out.push_str("_(no text response)_");
            } else {
                out.push_str(&reply);
            }
            out.push_str("\n\n");
        }
        out
    }

    /// Returns all fenced code blocks across all assistant turns, in chronological order.
    pub fn all_code_blocks(&self) -> Vec<CodeBlock> {
        self.turns
            .iter()
            .flat_map(|turn| turn.code_blocks())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recovered_tool_batch_retains_real_results_and_closes_missing_calls() {
        let mut session = ChatSession::new("fixture", "Task", AgentMode::Agent);
        let turn = session.start_turn("Run checks");
        for id in ["ran", "missing"] {
            session.record_tool_use(
                turn,
                id.into(),
                crate::TOOL_RUN_COMMAND.into(),
                serde_json::json!({}),
            );
        }
        session.record_command_status(turn, "ran".into(), crate::CommandStatus::Exited(3));
        session.record_tool_result(turn, "ran".into(), "a real result".into(), true);
        assert!(session.recover_interrupted_turns());
        let json = session.export_json().unwrap();
        let mut restored = ChatSession::import_json(&json).unwrap();
        assert!(!restored.recover_interrupted_turns());
        assert_eq!(restored.turns[0].status, TurnStatus::Interrupted);
        assert_eq!(
            restored.turns[0].commands[0].status,
            crate::CommandStatus::Exited(3)
        );
        let messages = restored.to_request_messages();
        assert!(matches!(&messages[2].blocks[..], [
            Block::ToolResult { id: first, content, .. },
            Block::ToolResult { id: second, is_error: true, .. },
        ] if first == "ran" && content == "a real result" && second == "missing"));
        assert_eq!(restored.turns[0].steps[1].blocks.len(), 1);
    }

    #[test]
    fn session_turns_code_blocks_and_export_round_trip() {
        let mut session = ChatSession::new("s1", "New Chat", AgentMode::Agent);
        let t1 = session.start_turn("Explain hello world in Rust");
        assert_eq!(session.title, "Explain hello world in Rust");
        session.append_text(
            t1,
            "Here you go:\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n",
        );
        session.record_tool_use(
            t1,
            "call_1".into(),
            "read_file".into(),
            json!({"path": "main.rs"}),
        );
        session.record_tool_result(t1, "call_1".into(), "fn main() {}".into(), false);
        session.append_text(t1, "Done.");

        let blocks = session.all_code_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, "rust");
        assert!(blocks[0].code.contains("println!(\"hi\");"));

        let msgs = session.to_request_messages();
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, Role::User);
        assert_eq!(msgs[1].role, Role::Assistant);
        assert_eq!(msgs[2].role, Role::User);
        assert_eq!(msgs[3].role, Role::Assistant);

        let raw = session.export_json().unwrap();
        let restored = ChatSession::import_json(&raw).unwrap();
        assert_eq!(restored.turns.len(), 1);
        assert_eq!(restored.title, session.title);
    }

    #[test]
    fn checkpoints_staging_and_compaction_work() {
        let mut session = ChatSession::new("s2", "New Chat", AgentMode::Edit);
        let t1 = session.start_turn("Edit foo");
        let path = PathBuf::from("src/foo.rs");
        session.stage_edit(t1, path.clone(), Some("old".into()), "new".into());
        assert_eq!(session.pending_edits.len(), 1);

        let cp = session.undo_checkpoint().unwrap();
        assert_eq!(cp.entries[0].before.as_deref(), Some("old"));
        assert_eq!(session.pending_edits[0].status, EditReviewStatus::Discarded);
        let redone = session.redo_checkpoint().unwrap();
        assert_eq!(redone.entries[0].after.as_deref(), Some("new"));

        session.append_text(t1, "Updated foo.");
        let t2 = session.start_turn("Second turn");
        session.append_text(t2, "Second reply.");
        let compacted = session.compact_conversation(1);
        assert_eq!(compacted, 1);
        assert_eq!(session.turns.len(), 1);
        assert!(session.summary.as_deref().unwrap().contains("Edit foo"));
    }
}
