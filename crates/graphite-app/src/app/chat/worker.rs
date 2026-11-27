//! Background model execution and approval bridging.

use super::*;
use graphite_chat::{
    execute_tool, run_turn, run_turn_with_tools, AgentMode, AgentTurnEvent, ApprovalDecision,
    ChatContext, HttpModelStreamer, Message, PendingToolApproval, ProposedEdit,
    SystemCommandRunner, TurnStatus, TOOL_PROPOSE_EDIT,
};
use std::sync::{atomic::AtomicBool, mpsc, Arc};

/// Events posted back to the main winit event loop by background chat workers.
#[derive(Debug)]
pub(crate) enum ChatUiEvent {
    TurnEvent {
        run: Arc<AtomicBool>,
        session_id: String,
        turn_id: u64,
        event: AgentTurnEvent,
        approval_reply: Option<mpsc::Sender<ApprovalDecision>>,
    },
    InlineEvent {
        run: Arc<AtomicBool>,
        document: DocumentId,
        event: AgentTurnEvent,
    },
    PluginToolCall {
        run: Arc<AtomicBool>,
        name: String,
        input: serde_json::Value,
        reply: mpsc::Sender<Result<String, String>>,
    },
    /// Waits for the session to record the command before launching it.
    CommandStarting {
        run: Arc<AtomicBool>,
        session_id: String,
        turn_id: u64,
        id: String,
        reply: mpsc::Sender<bool>,
    },
    /// An approved `propose_edit` call. The worker waits for the reply, so the model learns
    /// whether the proposal was queued for review.
    StageEdit {
        run: Arc<AtomicBool>,
        session_id: String,
        turn_id: u64,
        edit: ProposedEdit,
        reply: mpsc::Sender<Result<String, String>>,
    },
}

impl App {
    /// Submits a user prompt to the active chat session.
    pub(crate) fn submit_chat_prompt(&mut self, prompt: String) {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.chat.running_turn.is_some() {
            self.status = "A chat response is already running".into();
            return;
        }
        let Some((connection, mut config)) = self.provider_for_chat(false) else {
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = "Event loop unavailable for background chat".into();
            return;
        };
        if config.kind.uses_opencode_session() {
            match self.chat.opencode_session(
                &connection,
                &self.profile_root,
                self.profiles.is_none(),
            ) {
                Ok(id) => config.opencode_session = Some(id),
                Err(error) => {
                    super::connection_ui::record_connection_error(error);
                    self.status = error.into();
                    return;
                }
            }
        }

        let prompt = trimmed.to_owned();
        let ctx = self.chat_context_for_turn();
        let mode = self.chat.active_session().mode;
        let session_id = self.chat.active_session_id.clone();
        self.chat.active_session_mut().connection_destination = Some(connection.destination());
        self.chat.active_session_mut().connection_id = Some(connection.id);
        let workspace = ctx.workspace_root.canonicalize().ok();
        let turn_id = self.chat.active_session_mut().start_turn(prompt);
        if let Some(turn) = self.chat.active_session_mut().turn_mut(turn_id) {
            turn.workspace = workspace;
        }
        self.chat.active_session_mut().log(
            "turn.start",
            format!("mode={}, turn={turn_id}", mode.label()),
        );
        if !self.persist_chat_turn_session(&session_id) {
            let message = self.t("chat.task.storage_failed").to_owned();
            if let Some(turn) = self.chat.active_session_mut().turn_mut(turn_id) {
                turn.status = TurnStatus::Error {
                    message: message.clone(),
                };
            }
            self.status = message;
            return;
        }
        let history = self.chat.active_session().to_request_messages();
        let plugin_tools = self.plugin_chat_tool_specs();
        let plugin_tool_names: std::collections::HashSet<String> =
            plugin_tools.iter().map(|t| t.name.clone()).collect();
        let mut policy = self.chat.policy.clone();
        if self.chat_workspace_restricted() {
            // Approvals remembered while the workspace was trusted do not apply in restricted mode.
            policy.reset_trusted();
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.chat.running_turn = Some((session_id.clone(), turn_id, cancel.clone()));
        let event_session_id = session_id.clone();
        let workspace_root = ctx.workspace_root.clone();
        let started = std::thread::Builder::new()
            .name("graphite-chat-turn".into())
            .spawn(move || {
                let mut policy = policy;
                let mut streamer = HttpModelStreamer;
                let runner = SystemCommandRunner::new().with_cancel(cancel.clone());
                run_turn_with_tools(
                    &config,
                    mode,
                    &ctx,
                    &history,
                    &mut policy,
                    &cancel,
                    &mut streamer,
                    &runner,
                    &plugin_tools,
                    |name, input| {
                        if name == TOOL_PROPOSE_EDIT {
                            // Capture the original bytes here and queue the proposal on the UI thread.
                            let outcome = execute_tool(&workspace_root, mode, name, input, &runner);
                            let Some(edit) = outcome.proposed_edit else {
                                crate::app::error_log::record(
                                    crate::app::error_log::ErrorSeverity::Medium,
                                    "chat.review",
                                    "The edit tool could not prepare a proposal",
                                );
                                return Some(Err(outcome.content));
                            };
                            let (tx, rx) = mpsc::channel();
                            if proxy
                                .send_event(UserEvent::Chat(ChatUiEvent::StageEdit {
                                    run: cancel.clone(),
                                    session_id: session_id.clone(),
                                    turn_id,
                                    edit,
                                    reply: tx,
                                }))
                                .is_err()
                            {
                                return Some(Err("Editor event loop unavailable".into()));
                            }
                            return Some(
                                rx.recv()
                                    .unwrap_or_else(|_| Err("Edit channel closed".into())),
                            );
                        }
                        if !plugin_tool_names.contains(name) {
                            return None;
                        }
                        let (tx, rx) = mpsc::channel();
                        if proxy
                            .send_event(UserEvent::Chat(ChatUiEvent::PluginToolCall {
                                run: cancel.clone(),
                                name: name.to_owned(),
                                input: input.clone(),
                                reply: tx,
                            }))
                            .is_err()
                        {
                            return Some(Err("Editor event loop unavailable".into()));
                        }
                        Some(
                            rx.recv()
                                .unwrap_or_else(|_| Err("Plugin tool channel closed".into())),
                        )
                    },
                    |pending: &PendingToolApproval| {
                        let (tx, rx) = mpsc::channel();
                        let _ = proxy.send_event(UserEvent::Chat(ChatUiEvent::TurnEvent {
                            run: cancel.clone(),
                            session_id: session_id.clone(),
                            turn_id,
                            event: AgentTurnEvent::ApprovalRequested(pending.clone()),
                            approval_reply: Some(tx),
                        }));
                        rx.recv().unwrap_or(ApprovalDecision::Deny)
                    },
                    |event| {
                        if matches!(event, AgentTurnEvent::ApprovalRequested(_)) {
                            return;
                        }
                        if let AgentTurnEvent::CommandUpdate {
                            id,
                            status: graphite_chat::CommandStatus::Started,
                        } = event
                        {
                            let (tx, rx) = mpsc::channel();
                            if proxy
                                .send_event(UserEvent::Chat(ChatUiEvent::CommandStarting {
                                    run: cancel.clone(),
                                    session_id: session_id.clone(),
                                    turn_id,
                                    id,
                                    reply: tx,
                                }))
                                .is_err()
                                || !rx.recv().unwrap_or(false)
                            {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            return;
                        }
                        let _ = proxy.send_event(UserEvent::Chat(ChatUiEvent::TurnEvent {
                            run: cancel.clone(),
                            session_id: session_id.clone(),
                            turn_id,
                            event,
                            approval_reply: None,
                        }));
                    },
                );
            });
        if started.is_ok() {
            self.chat.clear_outgoing_draft();
        } else {
            self.apply_chat_turn_event(
                &event_session_id,
                turn_id,
                AgentTurnEvent::Error("Could not start the chat worker".into()),
                None,
            );
        }
    }

    pub(crate) fn cancel_chat_turn(&mut self) {
        let cancelled = self.chat.running_turn.take();
        if let Some((id, turn, cancel)) = &cancelled {
            cancel.store(true, Ordering::Relaxed);
            if self.chat.stopping_turns.len() >= 32 {
                self.chat.stopping_turns.remove(0);
            }
            self.chat
                .stopping_turns
                .push((id.clone(), *turn, cancel.clone()));
        }
        if let Some((_, reply)) = self.chat.pending_approval.take() {
            let _ = reply.send(ApprovalDecision::Deny);
        }
        if let Some((session_id, turn_id, _)) = cancelled {
            self.apply_chat_turn_event(&session_id, turn_id, AgentTurnEvent::Cancelled, None);
        }
    }

    /// Whether the workspace is in restricted mode, where no approval is remembered or reused.
    pub(crate) fn chat_workspace_restricted(&self) -> bool {
        self.workspace_trust
            .as_ref()
            .is_some_and(|trust| !trust.is_trusted(self.tree.root()))
    }

    /// Whether "Always allow" may be remembered for `tool_name` now.
    pub(crate) fn chat_can_remember_approval(&self, tool_name: &str) -> bool {
        !self.chat_workspace_restricted()
            && !(self.chat.active_session().mode == AgentMode::Ask
                && graphite_chat::ToolApprovalPolicy::is_read_only(tool_name))
    }

    pub(crate) fn respond_chat_approval(&mut self, decision: ApprovalDecision) {
        let Some((pending, reply)) = self.chat.pending_approval.take() else {
            return;
        };
        let decision = if decision == ApprovalDecision::AllowAlways
            && !self.chat_can_remember_approval(&pending.name)
        {
            ApprovalDecision::AllowOnce
        } else {
            decision
        };
        if decision == ApprovalDecision::AllowAlways {
            let root = self.tree.root().to_path_buf();
            self.chat.policy.trust(&root, &pending.name, &pending.input);
            self.chat
                .persist_policy(&self.profile_root, self.profiles.is_none());
        }
        self.chat.active_session_mut().log(
            "tool.approval",
            format!("{} -> {:?}", pending.name, decision),
        );
        let session_id = self.chat.active_session_id.clone();
        self.persist_chat_turn_session(&session_id);
        let _ = reply.send(decision);
    }

    fn persist_chat_turn_session(&self, id: &str) -> bool {
        self.chat
            .sessions
            .iter()
            .find(|session| session.id == id)
            .is_some_and(|session| {
                self.chat
                    .persist_session(&self.profile_root, self.profiles.is_none(), session)
            })
    }

    pub(crate) fn retry_last_chat_turn(&mut self) {
        if let Some(last) = self
            .chat
            .active_session()
            .turns
            .last()
            .map(|t| t.prompt.clone())
        {
            self.submit_chat_prompt(last);
        }
    }

    pub(crate) fn handle_chat_ui_event(&mut self, event: ChatUiEvent) {
        match event {
            ChatUiEvent::TurnEvent {
                run,
                session_id,
                turn_id,
                event,
                approval_reply,
            } => {
                if self
                    .chat
                    .running_turn
                    .as_ref()
                    .is_some_and(|(session, turn, active)| {
                        session == &session_id && *turn == turn_id && Arc::ptr_eq(active, &run)
                    })
                {
                    self.apply_chat_turn_event(&session_id, turn_id, event, approval_reply);
                } else if let Some(index) =
                    self.chat
                        .stopping_turns
                        .iter()
                        .position(|(session, turn, token)| {
                            session == &session_id && *turn == turn_id && Arc::ptr_eq(token, &run)
                        })
                {
                    if matches!(
                        event,
                        AgentTurnEvent::Cancelled
                            | AgentTurnEvent::Finished { .. }
                            | AgentTurnEvent::Error(_)
                    ) {
                        self.chat.stopping_turns.remove(index);
                    } else {
                        self.record_stopped_chat_result(&session_id, turn_id, event);
                    }
                }
            }
            ChatUiEvent::InlineEvent {
                run,
                document,
                event,
            } => {
                if self
                    .chat
                    .inline_cancel
                    .as_ref()
                    .is_some_and(|active| Arc::ptr_eq(active, &run))
                {
                    self.apply_inline_chat_event(document, event);
                }
            }
            ChatUiEvent::PluginToolCall {
                run,
                name,
                input,
                reply,
            } => {
                if !self
                    .chat
                    .running_turn
                    .as_ref()
                    .is_some_and(|(_, _, active)| Arc::ptr_eq(active, &run))
                {
                    let _ = reply.send(Err("Chat turn has ended".into()));
                } else if let Err(err) =
                    self.invoke_plugin_chat_tool(&name, input, Some(reply.clone()))
                {
                    let _ = reply.send(Err(err));
                }
            }
            ChatUiEvent::CommandStarting {
                run,
                session_id,
                turn_id,
                id,
                reply,
            } => {
                let active =
                    self.chat
                        .running_turn
                        .as_ref()
                        .is_some_and(|(session, turn, token)| {
                            session == &session_id && *turn == turn_id && Arc::ptr_eq(token, &run)
                        });
                let ready = active
                    && self
                        .chat
                        .session_by_id_mut(&session_id)
                        .is_some_and(|session| {
                            session.record_command_status(
                                turn_id,
                                id,
                                graphite_chat::CommandStatus::Started,
                            )
                        })
                    && self.persist_chat_turn_session(&session_id);
                if active && !ready {
                    self.apply_chat_turn_event(
                        &session_id,
                        turn_id,
                        AgentTurnEvent::Error(self.t("chat.task.storage_failed").into()),
                        None,
                    );
                }
                let _ = reply.send(ready);
            }
            ChatUiEvent::StageEdit {
                run,
                session_id,
                turn_id,
                edit,
                reply,
            } => {
                let result = if self
                    .chat
                    .running_turn
                    .as_ref()
                    .is_some_and(|(_, _, active)| Arc::ptr_eq(active, &run))
                {
                    self.stage_chat_proposal(&session_id, turn_id, edit)
                } else {
                    Err("Chat turn has ended".into())
                };
                let _ = reply.send(result);
            }
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn record_stopped_chat_result(
        &mut self,
        session_id: &str,
        turn_id: u64,
        event: AgentTurnEvent,
    ) {
        let Some(session) = self.chat.session_by_id_mut(session_id) else {
            return;
        };
        let Some(turn) = session.turn_mut(turn_id) else {
            return;
        };
        let changed = match event {
            AgentTurnEvent::CommandUpdate { id, status }
                if status != graphite_chat::CommandStatus::Started
                    && turn.commands.iter().any(|command| command.id == id) =>
            {
                session.record_command_status(turn_id, id, status)
            }
            AgentTurnEvent::ToolResult {
                id,
                name,
                content,
                is_error,
                ..
            } if name == graphite_chat::TOOL_RUN_COMMAND
                && turn.commands.iter().any(|command| command.id == id) =>
            {
                session.record_tool_result(turn_id, id, content, is_error);
                true
            }
            _ => false,
        };
        if changed {
            self.persist_chat_turn_session(session_id);
        }
    }

    fn apply_chat_turn_event(
        &mut self,
        session_id: &str,
        turn_id: u64,
        event: AgentTurnEvent,
        approval_reply: Option<mpsc::Sender<ApprovalDecision>>,
    ) {
        let is_cancelled = self
            .chat
            .session_by_id_mut(session_id)
            .and_then(|session| session.turn_mut(turn_id))
            .is_some_and(|turn| turn.status == TurnStatus::Cancelled);
        if is_cancelled {
            return;
        }
        let save = !matches!(event, AgentTurnEvent::DeltaText(_));
        match event {
            AgentTurnEvent::DeltaText(delta) => {
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    session.append_text(turn_id, &delta);
                }
            }
            AgentTurnEvent::ToolUse { id, name, input } => {
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    session.log("tool.use", format!("{name} ({id})"));
                    session.record_tool_use(turn_id, id, name, input);
                }
            }
            AgentTurnEvent::ApprovalRequested(pending) => {
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    if let Some(turn) = session.turn_mut(turn_id) {
                        turn.status = TurnStatus::WaitingApproval;
                    }
                    session.log("tool.wait_approval", pending.summary.clone());
                }
                if let Some(reply) = approval_reply {
                    self.chat.pending_approval = Some((pending, reply));
                }
            }
            AgentTurnEvent::CommandUpdate { id, status } => {
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    session.record_command_status(turn_id, id, status);
                }
                if matches!(
                    status,
                    graphite_chat::CommandStatus::Failed | graphite_chat::CommandStatus::TimedOut
                ) {
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::Medium,
                        "chat.command",
                        format!("Command ended with {status:?}"),
                    );
                }
            }
            AgentTurnEvent::ToolResult {
                id,
                name,
                mut content,
                mut is_error,
                proposed_edit,
            } => {
                if let Some(edit) = proposed_edit {
                    match self.stage_chat_proposal(session_id, turn_id, edit) {
                        Ok(message) => content = message,
                        Err(message) => {
                            content = message;
                            is_error = true;
                        }
                    }
                }
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    if let Some(turn) = session.turn_mut(turn_id) {
                        turn.status = TurnStatus::Streaming;
                    }
                    session.log("tool.result", format!("{name} error={is_error}"));
                    session.record_tool_result(turn_id, id, content, is_error);
                }
            }
            AgentTurnEvent::Finished { usage } => {
                let is_inactive = session_id != self.chat.active_session_id;
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    if let Some(turn) = session.turn_mut(turn_id) {
                        turn.status = TurnStatus::Done;
                        turn.usage = usage.into();
                    }
                    if is_inactive {
                        session.unread = true;
                    }
                    session.log(
                        "turn.done",
                        format!("in={} out={}", usage.input_tokens, usage.output_tokens),
                    );
                }
                self.chat.running_turn = None;
                self.refresh_scm_async();
            }
            AgentTurnEvent::Error(message) => {
                super::connection_ui::record_connection_error("The chat turn could not complete");
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    if let Some(turn) = session.turn_mut(turn_id) {
                        turn.status = TurnStatus::Error {
                            message: crate::app::error_log::sanitize_message(&message),
                        };
                    }
                    session.log("turn.error", "Chat turn did not complete");
                }
                self.chat.running_turn = None;
                self.status = format!(
                    "Chat error: {}",
                    crate::app::error_log::sanitize_message(&message)
                );
            }
            AgentTurnEvent::Cancelled => {
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    if let Some(turn) = session.turn_mut(turn_id) {
                        turn.status = TurnStatus::Cancelled;
                    }
                    session.log("turn.cancelled", "Cancelled by user");
                }
                self.chat.running_turn = None;
                self.status = "Chat cancelled".into();
            }
        }
        if save {
            self.persist_chat_turn_session(session_id);
        }
    }

    /// Starts an inline chat request on the active editor selection or line.
    pub(crate) fn submit_inline_chat(&mut self, prompt: String) {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return;
        }
        if self
            .chat
            .inline_chat
            .as_ref()
            .is_some_and(|inline| inline.running)
        {
            return;
        }
        let Some((connection, mut config)) = self.provider_for_chat(true) else {
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = "Event loop unavailable".into();
            return;
        };
        let doc_id = self.active_document();
        let selection = self
            .editor()
            .selection()
            .map(|s| (s.anchor.min(s.head), s.anchor.max(s.head)));
        let cursor_line = self.editor().cursor_line_col().0;
        let state = self
            .chat
            .inline_chat
            .get_or_insert_with(|| super::state::InlineChatState {
                connection: None,
                opencode_session: None,
                document: doc_id,
                anchor_line: cursor_line,
                original_selection: selection,
                prompt: String::new(),
                response: String::new(),
                proposed_replacement: None,
                running: false,
                error: None,
            });
        let destination = connection.destination();
        let binding = (connection.id, destination);
        if config.kind.uses_opencode_session() {
            match state.opencode_session_for(&binding) {
                Ok(id) => config.opencode_session = Some(id),
                Err(error) => {
                    super::connection_ui::record_connection_error(error);
                    self.status = error.into();
                    return;
                }
            }
        } else {
            state.opencode_session = None;
        }
        state.connection = Some(binding);
        state.prompt = trimmed.to_owned();
        state.response.clear();
        state.proposed_replacement = None;
        state.running = true;
        state.error = None;

        // Inline Chat explicitly targets the selected text or cursor line.
        let mut ctx = self.gather_chat_context();
        ctx.diagnostics.clear();
        if let Some(file) = &mut ctx.active_file {
            file.content_excerpt = if file.selection.is_some() {
                String::new()
            } else {
                self.documents[doc_id]
                    .buffer
                    .line(cursor_line)
                    .chars()
                    .take(8_000)
                    .collect()
            };
            if let Some(selection) = &mut file.selection {
                *selection = selection.chars().take(8_000).collect();
            }
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.chat.inline_cancel = Some(cancel.clone());
        let user_prompt = format!(
            "Inline editor request (reply with only the replacement code block or concise answer):\n{trimmed}"
        );

        let started = std::thread::Builder::new()
            .name("graphite-inline-chat".into())
            .spawn(move || {
                let mut policy = graphite_chat::ToolApprovalPolicy::default();
                let mut streamer = HttpModelStreamer;
                let runner = SystemCommandRunner::new();
                run_turn(
                    &config,
                    AgentMode::Ask,
                    &ChatContext {
                        workspace_root: ctx.workspace_root,
                        active_file: ctx.active_file,
                        diagnostics: ctx.diagnostics,
                        referenced_files: Vec::new(),
                    },
                    &[Message::user(user_prompt)],
                    &mut policy,
                    &cancel,
                    &mut streamer,
                    &runner,
                    |_| ApprovalDecision::Deny,
                    |event| {
                        let _ = proxy.send_event(UserEvent::Chat(ChatUiEvent::InlineEvent {
                            run: cancel.clone(),
                            document: doc_id,
                            event,
                        }));
                    },
                );
            });
        if started.is_err() {
            self.apply_inline_chat_event(
                doc_id,
                AgentTurnEvent::Error("Could not start the inline chat worker".into()),
            );
        }
    }

    fn apply_inline_chat_event(&mut self, document: DocumentId, event: AgentTurnEvent) {
        let Some(inline) = self.chat.inline_chat.as_mut() else {
            return;
        };
        if inline.document != document {
            return;
        }
        match event {
            AgentTurnEvent::DeltaText(delta) => inline.response.push_str(&delta),
            AgentTurnEvent::Finished { .. } => {
                inline.running = false;
                let blocks = graphite_chat::extract_code_blocks(&inline.response);
                inline.proposed_replacement =
                    blocks.into_iter().next().map(|b| b.code).or_else(|| {
                        let text = inline.response.trim();
                        (!text.is_empty()).then(|| text.to_owned())
                    });
                self.chat.inline_cancel = None;
            }
            AgentTurnEvent::Error(err) => {
                super::connection_ui::record_connection_error(
                    "The inline chat turn could not complete",
                );
                inline.running = false;
                inline.error = Some(err);
                self.chat.inline_cancel = None;
            }
            AgentTurnEvent::Cancelled => {
                inline.running = false;
                self.chat.inline_cancel = None;
            }
            _ => {}
        }
    }

    pub(crate) fn keep_inline_chat(&mut self) -> bool {
        let Some(inline) = self.chat.inline_chat.take() else {
            return false;
        };
        let Some(replacement) = inline.proposed_replacement else {
            return false;
        };
        if self.documents[inline.document].buffer.is_read_only() {
            self.status = self.t("editor.read_only").into();
            return false;
        }
        {
            let mut editor = self.editor_mut();
            if let Some((start, end)) = inline.original_selection {
                editor.set_selection(start, end);
            }
            editor.insert(&replacement);
        }
        self.status = "Applied inline chat change".into();
        true
    }

    pub(crate) fn close_inline_chat(&mut self) {
        if let Some(cancel) = self.chat.inline_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.chat.inline_chat = None;
    }

    pub(crate) fn continue_inline_in_chat(&mut self) {
        if self.chat_connection_busy() {
            return;
        }
        let Some(inline) = self.chat.inline_chat.take() else {
            return;
        };
        self.open_chat_sidebar();
        if !inline.prompt.is_empty() {
            let current = self.chat.active_session();
            if !inline.connection.as_ref().is_some_and(|(id, destination)| {
                current.connection_id.as_ref() == Some(id)
                    && current.connection_destination.as_ref() == Some(destination)
            }) {
                self.new_chat_session();
                let session = self.chat.active_session_mut();
                let (id, destination) = inline.connection.unzip();
                session.connection_id = id;
                session.connection_destination = destination;
            }
            let session = self.chat.active_session_mut();
            if session.opencode_session.is_none() {
                session.opencode_session = inline.opencode_session;
            }
            let turn_id = session.start_turn(inline.prompt);
            if !inline.response.is_empty() {
                session.append_text(turn_id, &inline.response);
            }
            if let Some(turn) = session.turn_mut(turn_id) {
                turn.status = TurnStatus::Done;
            }
            self.chat
                .persist_active_session(&self.profile_root, self.profiles.is_none());
        }
    }

    pub(crate) fn open_chat_sidebar(&mut self) {
        if self.ai_mode.is_some() {
            self.focus_ai_chat();
            return;
        }
        self.secondary_plugin = None;
        self.chat.visible = true;
        self.chat.focused = true;
        self.chat.surface = super::state::ChatSurface::SideBar;
        self.view_preferences.secondary_sidebar_visible = true;
        self.explorer_focused = false;
        self.terminal_focused = false;
    }
}
