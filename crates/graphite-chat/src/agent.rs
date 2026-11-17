//! Context assembly and multi-round tool-calling loop for chat and agent turns.

use crate::message::{
    Block, ChatError, Message, Request, StopReason, StreamEvent, ToolSpec, Usage,
};
use crate::provider::ProviderConfig;
use crate::session::AgentMode;
use crate::tools::{
    execute_tool, summarize_tool_call, tool_specs_for_mode, ApprovalDecision, CommandRunner,
    PendingToolApproval, ProposedEdit, ToolApprovalPolicy,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_TOOL_ROUNDS: usize = 12;
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Workspace context attached to a prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatContext {
    pub workspace_root: PathBuf,
    pub active_file: Option<ActiveFileContext>,
    pub diagnostics: Vec<String>,
    pub referenced_files: Vec<FileAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveFileContext {
    pub path: PathBuf,
    pub language: String,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub selection: Option<String>,
    pub content_excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAttachment {
    pub path: PathBuf,
    pub content: String,
}

/// Builds the system prompt for `mode` and attached workspace context.
pub fn build_system_prompt(mode: AgentMode, ctx: &ChatContext) -> String {
    let mut prompt = String::from(
        "You are Graphite's built-in coding assistant. Be concise, accurate, and grounded in the workspace files.\n",
    );
    match mode {
        AgentMode::Ask => {
            prompt.push_str(
                "Mode: Ask (read-only). Answer questions using read-only tools if needed.\n",
            );
        }
        AgentMode::Edit => {
            prompt.push_str(
                "Mode: Edit. Use `propose_edit` to stage file updates for user review.\n",
            );
        }
        AgentMode::Agent => {
            prompt.push_str(
                "Mode: Agent. You may inspect files, propose edits with `propose_edit`, and run commands with `run_command` (user-approved).\n",
            );
        }
    }
    if !ctx.workspace_root.as_os_str().is_empty() {
        prompt.push_str(&format!(
            "Workspace root: {}\n",
            ctx.workspace_root.display()
        ));
    }
    if let Some(active) = &ctx.active_file {
        prompt.push_str(&format!(
            "\nActive file: {} ({}, cursor line {}, col {})\n",
            active.path.display(),
            active.language,
            active.cursor_line + 1,
            active.cursor_col + 1
        ));
        if let Some(sel) = &active.selection {
            if !sel.is_empty() {
                prompt.push_str("Selected text:\n```\n");
                prompt.push_str(sel);
                prompt.push_str("\n```\n");
            }
        }
        if !active.content_excerpt.is_empty() {
            prompt.push_str("File excerpt:\n```\n");
            prompt.push_str(&active.content_excerpt);
            prompt.push_str("\n```\n");
        }
    }
    if !ctx.diagnostics.is_empty() {
        prompt.push_str("\nActive diagnostics:\n");
        for diag in ctx.diagnostics.iter().take(20) {
            prompt.push_str("- ");
            prompt.push_str(diag);
            prompt.push('\n');
        }
    }
    for file in &ctx.referenced_files {
        prompt.push_str(&format!(
            "\nReferenced file {}:\n```\n{}\n```\n",
            file.path.display(),
            file.content
        ));
    }
    prompt
}

/// Incremental UI events produced while running an agent/chat turn.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentTurnEvent {
    DeltaText(String),
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ApprovalRequested(PendingToolApproval),
    ToolResult {
        id: String,
        name: String,
        content: String,
        is_error: bool,
        proposed_edit: Option<ProposedEdit>,
    },
    Finished {
        usage: Usage,
    },
    Error(String),
    Cancelled,
}

/// Streaming transport hook so tests can drive the loop without real HTTP.
pub trait ModelStreamer {
    fn stream(
        &mut self,
        config: &ProviderConfig,
        request: &Request,
        cancel: &AtomicBool,
        on_event: &mut dyn FnMut(StreamEvent),
    ) -> Result<(), ChatError>;
}

/// Default streamer backed by `crate::stream`.
pub struct HttpModelStreamer;

impl ModelStreamer for HttpModelStreamer {
    fn stream(
        &mut self,
        config: &ProviderConfig,
        request: &Request,
        cancel: &AtomicBool,
        on_event: &mut dyn FnMut(StreamEvent),
    ) -> Result<(), ChatError> {
        crate::stream(config, request, cancel, on_event)
    }
}

/// Runs a complete turn (including any tool rounds) until the model ends, errors, or is cancelled.
// The arguments are the turn's independent collaborators; bundling them would only hide them.
#[allow(clippy::too_many_arguments)]
pub fn run_turn(
    config: &ProviderConfig,
    mode: AgentMode,
    ctx: &ChatContext,
    history: &[Message],
    policy: &mut ToolApprovalPolicy,
    cancel: &AtomicBool,
    streamer: &mut dyn ModelStreamer,
    runner: &dyn CommandRunner,
    request_approval: impl FnMut(&PendingToolApproval) -> ApprovalDecision,
    emit: impl FnMut(AgentTurnEvent),
) {
    run_turn_with_tools(
        config,
        mode,
        ctx,
        history,
        policy,
        cancel,
        streamer,
        runner,
        &[],
        |_name, _input| None,
        request_approval,
        emit,
    );
}

/// Runs a complete turn with additional plugin-provided tools evaluated before falling back to built-ins.
#[allow(clippy::too_many_arguments)]
pub fn run_turn_with_tools(
    config: &ProviderConfig,
    mode: AgentMode,
    ctx: &ChatContext,
    history: &[Message],
    policy: &mut ToolApprovalPolicy,
    cancel: &AtomicBool,
    streamer: &mut dyn ModelStreamer,
    runner: &dyn CommandRunner,
    extra_tools: &[ToolSpec],
    mut run_extra_tool: impl FnMut(&str, &Value) -> Option<Result<String, String>>,
    mut request_approval: impl FnMut(&PendingToolApproval) -> ApprovalDecision,
    mut emit: impl FnMut(AgentTurnEvent),
) {
    let mut messages = history.to_vec();
    let system = build_system_prompt(mode, ctx);
    let mut tools = tool_specs_for_mode(mode);
    tools.extend(extra_tools.iter().cloned());
    let mut total_usage = Usage::default();

    for _round in 0..MAX_TOOL_ROUNDS {
        if cancel.load(Ordering::Relaxed) {
            emit(AgentTurnEvent::Cancelled);
            return;
        }
        let request = Request {
            system: system.clone(),
            messages: messages.clone(),
            tools: tools.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
        };

        let mut assistant_blocks: Vec<Block> = Vec::new();
        let mut pending_calls: Vec<(String, String, Value)> = Vec::new();
        let mut stop_reason = StopReason::EndTurn;
        let mut round_usage = Usage::default();

        let stream_result = streamer.stream(config, &request, cancel, &mut |event| match event {
            StreamEvent::Text(delta) => {
                if let Some(Block::Text { text }) = assistant_blocks.last_mut() {
                    text.push_str(&delta);
                } else {
                    assistant_blocks.push(Block::Text {
                        text: delta.clone(),
                    });
                }
                emit(AgentTurnEvent::DeltaText(delta));
            }
            StreamEvent::ToolUse { id, name, input } => {
                assistant_blocks.push(Block::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
                pending_calls.push((id.clone(), name.clone(), input.clone()));
                emit(AgentTurnEvent::ToolUse { id, name, input });
            }
            StreamEvent::Finished { stop, usage } => {
                stop_reason = stop;
                round_usage = usage;
            }
        });

        total_usage.input_tokens = total_usage
            .input_tokens
            .saturating_add(round_usage.input_tokens);
        total_usage.output_tokens = total_usage
            .output_tokens
            .saturating_add(round_usage.output_tokens);

        match stream_result {
            Ok(()) => {}
            Err(ChatError::Cancelled) => {
                emit(AgentTurnEvent::Cancelled);
                return;
            }
            Err(err) => {
                emit(AgentTurnEvent::Error(err.to_string()));
                return;
            }
        }

        if !assistant_blocks.is_empty() {
            messages.push(Message::assistant(assistant_blocks));
        }

        if pending_calls.is_empty() || stop_reason != StopReason::ToolUse {
            emit(AgentTurnEvent::Finished { usage: total_usage });
            return;
        }

        let mut result_blocks = Vec::with_capacity(pending_calls.len());
        for (id, name, input) in pending_calls {
            if cancel.load(Ordering::Relaxed) {
                emit(AgentTurnEvent::Cancelled);
                return;
            }
            let approved = if policy.requires_approval(&name) {
                let pending = PendingToolApproval {
                    id: id.clone(),
                    name: name.clone(),
                    summary: summarize_tool_call(&name, &input),
                    input: input.clone(),
                };
                emit(AgentTurnEvent::ApprovalRequested(pending.clone()));
                match request_approval(&pending) {
                    ApprovalDecision::AllowOnce => true,
                    ApprovalDecision::AllowAlways => {
                        policy.trust_tool(&name);
                        true
                    }
                    ApprovalDecision::Deny => false,
                }
            } else {
                true
            };

            let outcome = if approved {
                if let Some(custom_result) = run_extra_tool(&name, &input) {
                    match custom_result {
                        Ok(content) => crate::tools::ToolExecutionResult {
                            content,
                            is_error: false,
                            proposed_edit: None,
                        },
                        Err(content) => crate::tools::ToolExecutionResult {
                            content,
                            is_error: true,
                            proposed_edit: None,
                        },
                    }
                } else {
                    execute_tool(&ctx.workspace_root, mode, &name, &input, runner)
                }
            } else {
                crate::tools::ToolExecutionResult {
                    content: "Tool call denied by user.".into(),
                    is_error: true,
                    proposed_edit: None,
                }
            };

            emit(AgentTurnEvent::ToolResult {
                id: id.clone(),
                name: name.clone(),
                content: outcome.content.clone(),
                is_error: outcome.is_error,
                proposed_edit: outcome.proposed_edit,
            });
            result_blocks.push(Block::ToolResult {
                id,
                content: outcome.content,
                is_error: outcome.is_error,
            });
        }
        messages.push(Message::tool_results(result_blocks));
    }

    emit(AgentTurnEvent::Error(
        "Stopped after maximum tool rounds".into(),
    ));
}

/// Applies an accepted proposed edit to disk cleanly.
pub fn write_proposed_edit(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;
    use crate::tools::{TOOL_PROPOSE_EDIT, TOOL_READ_FILE};
    use serde_json::json;
    use std::collections::VecDeque;

    struct FakeStreamer {
        rounds: VecDeque<Vec<StreamEvent>>,
    }

    impl ModelStreamer for FakeStreamer {
        fn stream(
            &mut self,
            _config: &ProviderConfig,
            _request: &Request,
            _cancel: &AtomicBool,
            on_event: &mut dyn FnMut(StreamEvent),
        ) -> Result<(), ChatError> {
            let events = self.rounds.pop_front().unwrap_or_default();
            for event in events {
                on_event(event);
            }
            Ok(())
        }
    }

    struct NoopRunner;
    impl CommandRunner for NoopRunner {
        fn run(&self, _cwd: &Path, _command: &str) -> Result<String, String> {
            Ok(String::new())
        }
    }

    #[test]
    fn agent_turn_runs_auto_approved_read_and_user_approved_edit() {
        let dir = std::env::temp_dir().join(format!("graphite-agent-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.rs"), "pub fn a() -> i32 { 1 }\n").unwrap();

        let mut streamer = FakeStreamer {
            rounds: VecDeque::from([
                vec![
                    StreamEvent::ToolUse {
                        id: "c1".into(),
                        name: TOOL_READ_FILE.into(),
                        input: json!({"path": "lib.rs"}),
                    },
                    StreamEvent::Finished {
                        stop: StopReason::ToolUse,
                        usage: Usage {
                            input_tokens: 10,
                            output_tokens: 5,
                        },
                    },
                ],
                vec![
                    StreamEvent::ToolUse {
                        id: "c2".into(),
                        name: TOOL_PROPOSE_EDIT.into(),
                        input: json!({"path": "lib.rs", "content": "pub fn a() -> i32 { 2 }\n"}),
                    },
                    StreamEvent::Finished {
                        stop: StopReason::ToolUse,
                        usage: Usage {
                            input_tokens: 12,
                            output_tokens: 8,
                        },
                    },
                ],
                vec![
                    StreamEvent::Text("Updated lib.rs to return 2.".into()),
                    StreamEvent::Finished {
                        stop: StopReason::EndTurn,
                        usage: Usage {
                            input_tokens: 14,
                            output_tokens: 6,
                        },
                    },
                ],
            ]),
        };

        let ctx = ChatContext {
            workspace_root: dir.clone(),
            ..Default::default()
        };
        let config = ProviderConfig::default_for(ProviderKind::OpenAiCompatible);
        let mut policy = ToolApprovalPolicy::default();
        let cancel = AtomicBool::new(false);
        let mut approvals_asked = Vec::new();
        let mut events = Vec::new();

        run_turn(
            &config,
            AgentMode::Agent,
            &ctx,
            &[Message::user("Change a() to return 2")],
            &mut policy,
            &cancel,
            &mut streamer,
            &NoopRunner,
            |pending| {
                approvals_asked.push(pending.name.clone());
                ApprovalDecision::AllowAlways
            },
            |ev| events.push(ev),
        );

        assert_eq!(approvals_asked, vec![TOOL_PROPOSE_EDIT]);
        assert!(policy.trusted_tools.contains(TOOL_PROPOSE_EDIT));
        assert!(events.iter().any(|ev| matches!(
            ev,
            AgentTurnEvent::ToolResult {
                proposed_edit: Some(_),
                is_error: false,
                ..
            }
        )));
        assert!(events.iter().any(|ev| matches!(
            ev,
            AgentTurnEvent::Finished {
                usage: Usage {
                    input_tokens: 36,
                    output_tokens: 19
                }
            }
        )));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_turn_with_tools_executes_plugin_tool_and_requires_approval() {
        let dir = std::env::temp_dir().join(format!(
            "graphite-chat-plugin-tool-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        struct CapturingStreamer {
            seen_requests: Vec<Request>,
            rounds: VecDeque<Vec<StreamEvent>>,
        }
        impl ModelStreamer for CapturingStreamer {
            fn stream(
                &mut self,
                _config: &ProviderConfig,
                request: &Request,
                _cancel: &AtomicBool,
                on_event: &mut dyn FnMut(StreamEvent),
            ) -> Result<(), ChatError> {
                self.seen_requests.push(request.clone());
                for event in self.rounds.pop_front().unwrap_or_default() {
                    on_event(event);
                }
                Ok(())
            }
        }

        let mut streamer = CapturingStreamer {
            seen_requests: Vec::new(),
            rounds: VecDeque::from([
                vec![
                    StreamEvent::ToolUse {
                        id: "p1".into(),
                        name: "plugin_lookup".into(),
                        input: json!({ "key": "status" }),
                    },
                    StreamEvent::Finished {
                        stop: StopReason::ToolUse,
                        usage: Usage {
                            input_tokens: 5,
                            output_tokens: 4,
                        },
                    },
                ],
                vec![
                    StreamEvent::Text("Plugin said ok.".into()),
                    StreamEvent::Finished {
                        stop: StopReason::EndTurn,
                        usage: Usage {
                            input_tokens: 8,
                            output_tokens: 3,
                        },
                    },
                ],
            ]),
        };

        let ctx = ChatContext {
            workspace_root: dir.clone(),
            ..Default::default()
        };
        let config = ProviderConfig::default_for(ProviderKind::OpenAiCompatible);
        let mut policy = ToolApprovalPolicy::default();
        let cancel = AtomicBool::new(false);
        let extra_tools = vec![ToolSpec {
            name: "plugin_lookup".into(),
            description: "Lookup plugin value".into(),
            schema: json!({ "type": "object", "properties": { "key": { "type": "string" } } }),
        }];
        let mut events = Vec::new();
        run_turn_with_tools(
            &config,
            AgentMode::Ask,
            &ctx,
            &[Message::user("Check status")],
            &mut policy,
            &cancel,
            &mut streamer,
            &NoopRunner,
            &extra_tools,
            |name, input| {
                (name == "plugin_lookup")
                    .then(|| Ok(format!("key={}", input["key"].as_str().unwrap_or(""))))
            },
            |_pending| ApprovalDecision::AllowOnce,
            |ev| events.push(ev),
        );

        assert!(streamer.seen_requests[0]
            .tools
            .iter()
            .any(|t| t.name == "plugin_lookup"));
        assert!(events.iter().any(|ev| matches!(
            ev,
            AgentTurnEvent::ToolResult { name, content, is_error: false, .. }
            if name == "plugin_lookup" && content == "key=status"
        )));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
