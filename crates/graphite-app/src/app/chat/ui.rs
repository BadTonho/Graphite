//! Keyboard, mouse, session, code-block, and provider-dialog interactions for Chat.

use super::state::{ChatSurface, ProviderDialogState, SessionsFilter};
use super::*;
use graphite_chat::{AgentMode, ApprovalDecision, ChatSession, ProviderKind};
use winit::keyboard::{Key, NamedKey};

impl App {
    pub(crate) fn secondary_sidebar_width(&self) -> f32 {
        if self.zen_mode || !self.view_preferences.secondary_sidebar_visible {
            return 0.0;
        }
        if self.chat.visible && self.chat.surface == ChatSurface::SideBar {
            320.0
        } else if self.outline_visible {
            240.0
        } else {
            0.0
        }
    }

    pub(crate) fn toggle_chat_sidebar(&mut self) {
        if self.chat.visible && self.chat.surface == ChatSurface::SideBar {
            self.chat.visible = false;
            self.chat.focused = false;
        } else {
            self.open_chat_sidebar();
        }
    }

    pub(crate) fn open_chat_in_editor(&mut self) {
        self.chat.visible = true;
        self.chat.focused = true;
        self.chat.surface = ChatSurface::Editor;
        self.explorer_focused = false;
        self.terminal_focused = false;
    }

    pub(crate) fn open_quick_chat(&mut self) {
        self.chat.visible = true;
        self.chat.focused = true;
        self.chat.surface = ChatSurface::QuickChat;
        self.explorer_focused = false;
        self.terminal_focused = false;
    }

    pub(crate) fn close_quick_chat(&mut self) {
        if self.chat.surface == ChatSurface::QuickChat {
            self.chat.visible = false;
            self.chat.focused = false;
            self.chat.surface = ChatSurface::SideBar;
        }
    }

    pub(crate) fn open_inline_chat(&mut self) {
        let doc = self.active_document();
        let selection = self
            .editor()
            .selection()
            .map(|s| (s.anchor.min(s.head), s.anchor.max(s.head)));
        let anchor_line = self.editor().cursor_line_col().0;
        self.chat.inline_chat = Some(super::state::InlineChatState {
            document: doc,
            anchor_line,
            original_selection: selection,
            prompt: String::new(),
            response: String::new(),
            proposed_replacement: None,
            running: false,
            error: None,
        });
        self.chat.focused = false;
    }

    pub(crate) fn new_chat_session(&mut self) {
        let mode = self.chat.active_session().mode;
        self.chat.new_session(mode);
        self.chat.input.clear();
        self.chat.input_history_cursor = None;
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = "Started new chat session".into();
    }

    pub(crate) fn cycle_chat_mode(&mut self) {
        let next = self.chat.active_session().mode.next();
        self.chat.active_session_mut().mode = next;
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = format!("Chat mode: {}", next.label());
    }

    pub(crate) fn toggle_pin_active_session(&mut self, pinned: Option<bool>) {
        let session = self.chat.active_session_mut();
        session.pinned = pinned.unwrap_or(!session.pinned);
        let state = session.pinned;
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = if state {
            "Pinned agent session"
        } else {
            "Unpinned agent session"
        }
        .into();
    }

    pub(crate) fn toggle_archive_active_session(&mut self, archived: Option<bool>) {
        let session = self.chat.active_session_mut();
        session.archived = archived.unwrap_or(!session.archived);
        let state = session.archived;
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = if state {
            "Archived agent session"
        } else {
            "Unarchived agent session"
        }
        .into();
    }

    pub(crate) fn navigate_chat_input_history(&mut self, previous: bool) {
        let len = self.chat.active_session().input_history.len();
        if len == 0 {
            return;
        }
        let next_idx = match (self.chat.input_history_cursor, previous) {
            (None, true) => Some(len - 1),
            (None, false) => None,
            (Some(0), true) => Some(0),
            (Some(i), true) => Some(i - 1),
            (Some(i), false) if i + 1 < len => Some(i + 1),
            (Some(_), false) => None,
        };
        let text = next_idx
            .and_then(|i| self.chat.active_session().input_history.get(i).cloned())
            .unwrap_or_default();
        self.chat.input_history_cursor = next_idx;
        self.chat.input = text;
    }

    pub(crate) fn active_chat_code_block(&self) -> Option<graphite_chat::CodeBlock> {
        let blocks = self.chat.active_session().all_code_blocks();
        if blocks.is_empty() {
            return None;
        }
        let idx = self.chat.selected_code_block.min(blocks.len() - 1);
        blocks.into_iter().nth(idx)
    }

    pub(crate) fn step_chat_code_block(&mut self, forward: bool) -> bool {
        let count = self.chat.active_session().all_code_blocks().len();
        if count == 0 {
            return false;
        }
        if forward {
            self.chat.selected_code_block = (self.chat.selected_code_block + 1) % count;
        } else {
            self.chat.selected_code_block = (self.chat.selected_code_block + count - 1) % count;
        }
        true
    }

    pub(crate) fn insert_active_code_block(&mut self, new_file: bool) -> bool {
        let Some(block) = self.active_chat_code_block() else {
            self.status = "No code block in active chat".into();
            return false;
        };
        if new_file {
            self.new_untitled_document();
        }
        if self.documents[self.active_document()].buffer.is_read_only() {
            self.status = self.t("editor.read_only").into();
            return false;
        }
        self.editor_mut().insert(&block.code);
        self.status = "Inserted chat code block".into();
        true
    }

    pub(crate) fn run_active_code_block_in_terminal(&mut self) -> bool {
        let Some(block) = self.active_chat_code_block() else {
            self.status = "No code block in active chat".into();
            return false;
        };
        if !self.terminal_visible {
            self.toggle_terminal();
        } else if self.terminals.is_empty() {
            let _ = self.spawn_terminal_session();
        }
        if let Some(term) = self.terminals.get_mut(self.active_terminal) {
            let mut cmd = block.code;
            if !cmd.ends_with('\n') {
                cmd.push('\n');
            }
            term.input(cmd.into_bytes());
            self.status = "Sent code block to terminal".into();
            true
        } else {
            false
        }
    }

    pub(crate) fn chat_panel_bounds(
        &self,
        width: f32,
        height: f32,
        lh: f32,
    ) -> (f32, f32, f32, f32) {
        match self.chat.surface {
            ChatSurface::SideBar => {
                let sec_w = self.secondary_sidebar_width();
                let top = self.activity_bar_vertical_offset();
                let x = if self.activity_bar_preferences.primary_sidebar_right
                    && self.view_preferences.primary_sidebar_visible
                {
                    self.sidebar_width_px() - sec_w
                } else {
                    width - sec_w
                };
                (x, top, sec_w, (height - top - lh).max(lh * 4.0))
            }
            ChatSurface::Editor => {
                let rect = self.focused_group_rect(width, height, lh);
                (rect.x, rect.y, rect.width, rect.height)
            }
            ChatSurface::QuickChat => {
                let box_w = (width * 0.52).clamp(320.0, 620.0);
                let box_h = (height * 0.56).clamp(240.0, 480.0);
                ((width - box_w) * 0.5, (height - box_h) * 0.2, box_w, box_h)
            }
        }
    }

    /// Handles keyboard input for provider dialog, inline chat, or focused chat panel.
    pub(crate) fn handle_chat_key(
        &mut self,
        key: &Key,
        text: Option<&str>,
        ctrl: bool,
        alt: bool,
    ) -> bool {
        if self.handle_provider_dialog_key(key, text, ctrl) {
            return true;
        }
        if self.handle_inline_chat_key(key, text, ctrl, alt) {
            return true;
        }
        if !self.chat.visible || !self.chat.focused {
            return false;
        }
        match key {
            Key::Named(NamedKey::Escape) => {
                if self.chat.running_turn.is_some() {
                    self.cancel_chat_turn();
                } else if self.chat.surface == ChatSurface::QuickChat {
                    self.close_quick_chat();
                } else {
                    self.chat.focused = false;
                }
                true
            }
            Key::Named(NamedKey::Enter) => {
                if self.chat.pending_approval.is_some() {
                    self.respond_chat_approval(ApprovalDecision::AllowOnce);
                } else {
                    let prompt = self.chat.input.clone();
                    self.submit_chat_prompt(prompt);
                }
                true
            }
            Key::Named(NamedKey::Backspace) => {
                self.chat.input.pop();
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.chat.input.is_empty() || self.chat.input_history_cursor.is_some() {
                    self.navigate_chat_input_history(true);
                } else {
                    self.chat.scroll = self.chat.scroll.saturating_sub(1);
                }
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.chat.input_history_cursor.is_some() {
                    self.navigate_chat_input_history(false);
                } else {
                    self.chat.scroll = self.chat.scroll.saturating_add(1);
                }
                true
            }
            Key::Named(NamedKey::PageUp) => {
                self.chat.scroll = self.chat.scroll.saturating_sub(8);
                true
            }
            Key::Named(NamedKey::PageDown) => {
                self.chat.scroll = self.chat.scroll.saturating_add(8);
                true
            }
            Key::Named(NamedKey::Tab) if !ctrl && !alt => {
                self.cycle_chat_mode();
                true
            }
            _ => {
                if !ctrl && !alt {
                    if let Some(ch) =
                        text.filter(|s| !s.is_empty() && !s.chars().all(char::is_control))
                    {
                        self.chat.input.push_str(ch);
                        return true;
                    }
                }
                false
            }
        }
    }

    fn handle_provider_dialog_key(&mut self, key: &Key, text: Option<&str>, ctrl: bool) -> bool {
        let Some(dialog) = self.chat.provider_dialog.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => {
                self.chat.provider_dialog = None;
                true
            }
            Key::Named(NamedKey::Tab) => {
                dialog.focus_field = (dialog.focus_field + 1) % 4;
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                dialog.focus_field = (dialog.focus_field + 3) % 4;
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                dialog.focus_field = (dialog.focus_field + 1) % 4;
                true
            }
            Key::Named(NamedKey::Space) if dialog.focus_field == 0 => {
                dialog.kind = match dialog.kind {
                    ProviderKind::Anthropic => ProviderKind::OpenAiCompatible,
                    ProviderKind::OpenAiCompatible => ProviderKind::Anthropic,
                };
                if dialog.model.is_empty()
                    || dialog.model == "claude-sonnet-5-5"
                    || dialog.model == "gpt-4o"
                {
                    dialog.model = graphite_chat::ProviderConfig::default_for(dialog.kind).model;
                }
                true
            }
            Key::Named(NamedKey::Enter) => {
                self.save_chat_provider_from_dialog();
                true
            }
            Key::Named(NamedKey::Backspace) => {
                match dialog.focus_field {
                    1 => {
                        dialog.base_url.pop();
                    }
                    2 => {
                        dialog.model.pop();
                    }
                    3 => {
                        dialog.api_key.pop();
                    }
                    _ => {}
                }
                true
            }
            _ => {
                if !ctrl {
                    if let Some(ch) =
                        text.filter(|s| !s.is_empty() && !s.chars().all(char::is_control))
                    {
                        match dialog.focus_field {
                            1 => dialog.base_url.push_str(ch),
                            2 => dialog.model.push_str(ch),
                            3 => dialog.api_key.push_str(ch),
                            0 => {
                                dialog.kind = match dialog.kind {
                                    ProviderKind::Anthropic => ProviderKind::OpenAiCompatible,
                                    ProviderKind::OpenAiCompatible => ProviderKind::Anthropic,
                                };
                            }
                            _ => {}
                        }
                        return true;
                    }
                }
                true
            }
        }
    }

    fn handle_inline_chat_key(
        &mut self,
        key: &Key,
        text: Option<&str>,
        ctrl: bool,
        alt: bool,
    ) -> bool {
        let Some(inline) = self.chat.inline_chat.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => {
                self.close_inline_chat();
                true
            }
            Key::Named(NamedKey::Enter) => {
                if inline.proposed_replacement.is_some() && !inline.running && ctrl {
                    self.keep_inline_chat();
                } else {
                    let prompt = inline.prompt.clone();
                    self.submit_inline_chat(prompt);
                }
                true
            }
            Key::Named(NamedKey::Backspace) => {
                inline.prompt.pop();
                true
            }
            _ => {
                if !ctrl && !alt {
                    if let Some(ch) =
                        text.filter(|s| !s.is_empty() && !s.chars().all(char::is_control))
                    {
                        inline.prompt.push_str(ch);
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Handles left mouse click in Chat overlays/sidebar. Returns `true` if consumed.
    pub(crate) fn handle_chat_click(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        lh: f32,
    ) -> bool {
        if let Some(dialog) = self.chat.provider_dialog.as_mut() {
            let box_w = (width * 0.56).clamp(340.0, 560.0);
            let box_h = lh * 9.0;
            let bx = (width - box_w) * 0.5;
            let by = (height - box_h) * 0.28;
            if x >= bx && x < bx + box_w && y >= by && y < by + box_h {
                let rel_row = ((y - by - lh * 1.4) / (lh * 1.3)).floor() as isize;
                if (0..=3).contains(&rel_row) {
                    dialog.focus_field = rel_row as usize;
                    if rel_row == 0 {
                        dialog.kind = match dialog.kind {
                            ProviderKind::Anthropic => ProviderKind::OpenAiCompatible,
                            ProviderKind::OpenAiCompatible => ProviderKind::Anthropic,
                        };
                    }
                } else if y >= by + box_h - lh * 1.8 {
                    if x < bx + box_w * 0.5 {
                        self.save_chat_provider_from_dialog();
                    } else {
                        self.chat.provider_dialog = None;
                    }
                }
                return true;
            }
        }

        if let Some(inline) = &self.chat.inline_chat {
            let rect = self.focused_group_rect(width, height, lh);
            let ix = rect.x + PAD * 2.0;
            let iy = rect.y + lh * 2.0;
            let iw = (rect.width - PAD * 4.0).max(260.0);
            let ih = lh
                * if inline.proposed_replacement.is_some() {
                    6.5
                } else {
                    3.5
                };
            if x >= ix && x < ix + iw && y >= iy && y < iy + ih {
                if y >= iy + ih - lh * 1.3 && inline.proposed_replacement.is_some() {
                    if x < ix + iw * 0.34 {
                        self.keep_inline_chat();
                    } else if x < ix + iw * 0.68 {
                        self.continue_inline_in_chat();
                    } else {
                        self.close_inline_chat();
                    }
                }
                return true;
            }
        }

        if !self.chat.visible {
            return false;
        }
        let (bx, by, bw, bh) = self.chat_panel_bounds(width, height, lh);
        if x < bx || x >= bx + bw || y < by || y >= by + bh {
            self.chat.focused = false;
            return false;
        }
        self.chat.focused = true;
        self.explorer_focused = false;
        self.terminal_focused = false;

        // Header toolbar row
        if y < by + lh * 1.5 {
            let rel_x = (x - bx) / bw.max(1.0);
            if rel_x < 0.25 {
                self.cycle_chat_mode();
            } else if rel_x < 0.48 {
                self.new_chat_session();
            } else if rel_x < 0.70 {
                self.chat.show_agent_sessions = !self.chat.show_agent_sessions;
            } else if rel_x < 0.86 {
                self.open_chat_provider_dialog();
            } else {
                self.chat.visible = false;
                self.chat.focused = false;
            }
            return true;
        }

        // Pending approval buttons near bottom above input box
        if self.chat.pending_approval.is_some() && y >= by + bh - lh * 4.2 && y < by + bh - lh * 2.2
        {
            let rel_x = (x - bx) / bw.max(1.0);
            if rel_x < 0.33 {
                self.respond_chat_approval(ApprovalDecision::AllowOnce);
            } else if rel_x < 0.67 {
                self.respond_chat_approval(ApprovalDecision::AllowAlways);
            } else {
                self.respond_chat_approval(ApprovalDecision::Deny);
            }
            return true;
        }

        // Pending file edits bar
        let has_pending_edits = self
            .chat
            .active_session()
            .pending_edits
            .iter()
            .any(|e| e.status == graphite_chat::EditReviewStatus::Pending);
        if has_pending_edits && y >= by + lh * 1.5 && y < by + lh * 3.0 {
            if x < bx + bw * 0.5 {
                self.accept_all_chat_edits();
            } else {
                self.discard_all_chat_edits();
            }
            return true;
        }

        // Sessions list rows
        if self.chat.show_agent_sessions && y >= by + lh * 1.6 && y < by + lh * 8.0 {
            let row = ((y - by - lh * 1.6) / lh).floor() as usize;
            let target_id = self.chat.filtered_sessions().get(row).map(|s| s.id.clone());
            if let Some(id) = target_id {
                self.chat.active_session_id = id.clone();
                if let Some(s) = self.chat.session_by_id_mut(&id) {
                    s.unread = false;
                }
                self.chat.show_agent_sessions = false;
            }
            return true;
        }

        true
    }
}

pub(crate) fn chat_session_badge(session: &ChatSession) -> &'static str {
    if session.pinned {
        "[pin]"
    } else if session.archived {
        "[arc]"
    } else if session.unread {
        "[*]"
    } else {
        match session.mode {
            AgentMode::Ask => "[ask]",
            AgentMode::Edit => "[edit]",
            AgentMode::Agent => "[agent]",
        }
    }
}

pub(crate) fn cycle_sessions_filter(current: SessionsFilter) -> SessionsFilter {
    match current {
        SessionsFilter::All => SessionsFilter::Pinned,
        SessionsFilter::Pinned => SessionsFilter::Archived,
        SessionsFilter::Archived => SessionsFilter::Unread,
        SessionsFilter::Unread => SessionsFilter::All,
    }
}

#[allow(dead_code)]
pub(crate) fn provider_kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::OpenAiCompatible => "OpenAI-compatible",
    }
}

#[allow(dead_code)]
pub(crate) fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return "<unset>".into();
    }
    let count = trimmed.chars().count();
    if count <= 4 {
        "•".repeat(count)
    } else {
        let tail: String = trimmed.chars().skip(count - 4).collect();
        format!("{}{}", "•".repeat((count - 4).min(12)), tail)
    }
}

#[allow(dead_code)]
pub(crate) fn default_provider_dialog() -> ProviderDialogState {
    ProviderDialogState::from_config(None)
}
