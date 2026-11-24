//! Background model execution, approval bridging, and edit/checkpoint application.

use super::*;
use graphite_chat::{
    run_turn, run_turn_with_tools, write_proposed_edit, AgentMode, AgentTurnEvent,
    ApprovalDecision, ChatContext, HttpModelStreamer, Message, PendingToolApproval,
    SystemCommandRunner, TurnStatus,
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
        let turn_id = self.chat.active_session_mut().start_turn(prompt);
        self.chat.active_session_mut().log(
            "turn.start",
            format!("mode={}, turn={turn_id}", mode.label()),
        );
        let history = self.chat.active_session().to_request_messages();
        let plugin_tools = self.plugin_chat_tool_specs();
        let plugin_tool_names: std::collections::HashSet<String> =
            plugin_tools.iter().map(|t| t.name.clone()).collect();
        let policy = self.chat.policy.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.chat.running_turn = Some((session_id.clone(), turn_id, cancel.clone()));
        let event_session_id = session_id.clone();
        let started = std::thread::Builder::new()
            .name("graphite-chat-turn".into())
            .spawn(move || {
                let mut policy = policy;
                let mut streamer = HttpModelStreamer;
                let runner = SystemCommandRunner;
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
        let cancelled = self.chat.running_turn.take().map(|(id, turn, cancel)| {
            cancel.store(true, Ordering::Relaxed);
            (id, turn)
        });
        if let Some((_, reply)) = self.chat.pending_approval.take() {
            let _ = reply.send(ApprovalDecision::Deny);
        }
        if let Some((session_id, turn_id)) = cancelled {
            self.apply_chat_turn_event(&session_id, turn_id, AgentTurnEvent::Cancelled, None);
        }
    }

    pub(crate) fn respond_chat_approval(&mut self, decision: ApprovalDecision) {
        let Some((pending, reply)) = self.chat.pending_approval.take() else {
            return;
        };
        let decision = if self.chat.active_session().mode == AgentMode::Ask
            && graphite_chat::ToolApprovalPolicy::is_read_only(&pending.name)
            && decision == ApprovalDecision::AllowAlways
        {
            ApprovalDecision::AllowOnce
        } else {
            decision
        };
        if decision == ApprovalDecision::AllowAlways {
            self.chat.policy.trust_tool(&pending.name);
            let _ = self
                .chat
                .persist_policy(&self.profile_root, self.profiles.is_none());
        }
        self.chat.active_session_mut().log(
            "tool.approval",
            format!("{} -> {:?}", pending.name, decision),
        );
        let _ = reply.send(decision);
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
                    .is_some_and(|(_, _, active)| Arc::ptr_eq(active, &run))
                {
                    self.apply_chat_turn_event(&session_id, turn_id, event, approval_reply);
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
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
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
            AgentTurnEvent::ToolResult {
                id,
                name,
                content,
                is_error,
                proposed_edit,
            } => {
                if let Some(edit) = proposed_edit {
                    if write_proposed_edit(&edit.path, &edit.after).is_ok() {
                        self.sync_open_document_text(&edit.path, Some(&edit.after));
                    }
                    if let Some(session) = self.chat.session_by_id_mut(session_id) {
                        session.stage_edit(turn_id, edit.path, edit.before, edit.after);
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
                self.chat
                    .persist_active_session(&self.profile_root, self.profiles.is_none());
                self.refresh_scm_async();
            }
            AgentTurnEvent::Error(message) => {
                super::connection_ui::record_connection_error("The chat turn could not complete");
                if let Some(session) = self.chat.session_by_id_mut(session_id) {
                    if let Some(turn) = session.turn_mut(turn_id) {
                        turn.status = TurnStatus::Error {
                            message: message.clone(),
                        };
                    }
                    session.log("turn.error", message.clone());
                }
                self.chat.running_turn = None;
                self.status = format!("Chat error: {message}");
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
                self.chat
                    .persist_active_session(&self.profile_root, self.profiles.is_none());
            }
        }
    }

    /// Updates an already-open document buffer when a file on disk is updated or reverted by chat.
    pub(crate) fn sync_open_document_text(&mut self, path: &Path, new_text: Option<&str>) {
        let Some(doc_id) = self
            .documents
            .iter()
            .find(|tab| {
                tab.buffer
                    .path()
                    .is_some_and(|open| same_file_path(open, path))
            })
            .map(|tab| tab.id)
        else {
            return;
        };
        if let Some(text) = new_text {
            let tab = &mut self.documents[doc_id];
            let outcome = tab.buffer.reload_text(text);
            let line_count = tab.buffer.len_lines();
            tab.highlights = None;
            tab.sticky_headers_cache = None;
            tab.highlighter = IncrementalHighlighter::new();
            self.editor_groups.for_each_view_mut(|_, view| {
                if view.document == doc_id {
                    view.selection_state
                        .remap(&outcome.positions, outcome.len_chars);
                    view.scroll_line = view.scroll_line.min(line_count.saturating_sub(1));
                }
            });
        }
    }

    pub(crate) fn accept_chat_edit(&mut self, path: &Path) -> bool {
        let accepted = self.chat.active_session_mut().accept_edit(path);
        if accepted {
            self.chat
                .persist_active_session(&self.profile_root, self.profiles.is_none());
            self.status = format!("Accepted changes to {}", path.display());
        }
        accepted
    }

    pub(crate) fn discard_chat_edit(&mut self, path: &Path) -> bool {
        let Some(before) = self.chat.active_session_mut().discard_edit(path) else {
            return false;
        };
        Self::restore_file_content(path, before.as_deref());
        self.sync_open_document_text(path, before.as_deref());
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.refresh_scm_async();
        self.status = format!("Discarded changes to {}", path.display());
        true
    }

    pub(crate) fn accept_all_chat_edits(&mut self) -> usize {
        let count = self.chat.active_session_mut().accept_all_edits();
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = format!("Accepted {count} file edit(s)");
        count
    }

    pub(crate) fn discard_all_chat_edits(&mut self) -> usize {
        let reverted = self.chat.active_session_mut().discard_all_edits();
        let count = reverted.len();
        for (path, before) in reverted {
            Self::restore_file_content(&path, before.as_deref());
            self.sync_open_document_text(&path, before.as_deref());
        }
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.refresh_scm_async();
        self.status = format!("Discarded {count} file edit(s)");
        count
    }

    pub(crate) fn undo_chat_edit(&mut self) -> bool {
        let Some(cp) = self.chat.active_session_mut().undo_checkpoint() else {
            return false;
        };
        for entry in &cp.entries {
            Self::restore_file_content(&entry.path, entry.before.as_deref());
            self.sync_open_document_text(&entry.path, entry.before.as_deref());
        }
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.refresh_scm_async();
        self.status = format!("Undid {}", cp.label);
        true
    }

    pub(crate) fn redo_chat_edit(&mut self) -> bool {
        let Some(cp) = self.chat.active_session_mut().redo_checkpoint() else {
            return false;
        };
        for entry in &cp.entries {
            Self::restore_file_content(&entry.path, entry.after.as_deref());
            self.sync_open_document_text(&entry.path, entry.after.as_deref());
        }
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.refresh_scm_async();
        self.status = format!("Redid {}", cp.label);
        true
    }

    fn restore_file_content(path: &Path, content: Option<&str>) {
        match content {
            Some(text) => {
                let _ = write_proposed_edit(path, text);
            }
            None => {
                let _ = std::fs::remove_file(path);
            }
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
                let runner = SystemCommandRunner;
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
        self.secondary_plugin = None;
        self.chat.visible = true;
        self.chat.focused = true;
        self.chat.surface = super::state::ChatSurface::SideBar;
        self.view_preferences.secondary_sidebar_visible = true;
        self.explorer_focused = false;
        self.terminal_focused = false;
    }
}
