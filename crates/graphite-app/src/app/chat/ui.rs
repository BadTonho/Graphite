//! Keyboard, mouse, session, code-block, and provider-dialog interactions for Chat.

use super::state::{ChatSurface, ProviderDialogState};
use super::*;
use graphite_chat::{AgentMode, ApprovalDecision, ChatSession};
use winit::keyboard::{Key, NamedKey};

impl App {
    /// The width the secondary side bar would have for the selected view when the user has not
    /// resized it.
    fn secondary_sidebar_default_width(&self) -> f32 {
        if self.chat.visible && self.chat.surface == ChatSurface::SideBar {
            320.0
        } else if self.outline_visible {
            240.0
        } else {
            320.0
        }
    }

    /// Widest the secondary side bar may be. It may push the primary side bar down to its
    /// narrowest, but the editor keeps its minimum width.
    pub(crate) fn secondary_sidebar_max_width(&self) -> f32 {
        let window = self.sidebar_window_width_px;
        let primary_min = if self.view_preferences.primary_sidebar_visible {
            config::MIN_SIDEBAR_WIDTH * self.sidebar_scale_factor
        } else {
            0.0
        };
        (window - self.activity_bar_width_px() - primary_min - crate::app::layout::MIN_EDITOR_WIDTH)
            .min(window * 0.7)
            .max(config::MIN_SECONDARY_SIDEBAR_WIDTH)
    }

    pub(crate) fn secondary_sidebar_width(&self) -> f32 {
        if self.zen_mode || !self.view_preferences.secondary_sidebar_visible {
            return 0.0;
        }
        let wanted = self
            .secondary_sidebar_user_width
            .unwrap_or_else(|| self.secondary_sidebar_default_width())
            .clamp(
                config::MIN_SECONDARY_SIDEBAR_WIDTH,
                self.secondary_sidebar_max_width(),
            );
        if !self.sidebar_priority_primary {
            return wanted;
        }
        // The primary side bar is being shaped: it keeps its width and this one gives way.
        let room = self.sidebar_window_width_px
            - self.activity_bar_width_px()
            - self.primary_requested_px()
            - crate::app::layout::MIN_EDITOR_WIDTH;
        wanted.min(room.max(config::MIN_SECONDARY_SIDEBAR_WIDTH))
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
            connection: None,
            opencode_session: None,
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
        if self.chat_connection_busy() {
            return;
        }
        self.chat.new_session(AgentMode::Ask);
        self.chat.clear_outgoing_draft();
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = "Started new chat session".into();
    }

    pub(crate) fn cycle_chat_mode(&mut self) {
        if self.chat_connection_busy() {
            return;
        }
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
                if self.secondary_plugin.is_some() {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                let top = self.secondary_sidebar_top(lh);
                (
                    self.secondary_sidebar_x(width),
                    top,
                    self.secondary_sidebar_width(),
                    (height - top - lh).max(0.0),
                )
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
            Key::Named(NamedKey::F4) => {
                self.open_chat_provider_dialog();
                true
            }
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
        let field = dialog.focus_field;
        match key {
            Key::Named(NamedKey::Escape) => self.chat.provider_dialog = None,
            Key::Named(NamedKey::Tab | NamedKey::ArrowDown) => {
                dialog.focus_field = (field + 1) % ProviderDialogState::ROWS;
            }
            Key::Named(NamedKey::ArrowUp) => {
                dialog.focus_field =
                    (field + ProviderDialogState::ROWS - 1) % ProviderDialogState::ROWS;
            }
            Key::Named(NamedKey::Space | NamedKey::ArrowRight) if field == 0 => {
                self.cycle_chat_connection(false)
            }
            Key::Named(NamedKey::ArrowLeft) if field == 0 => self.cycle_chat_connection(true),
            Key::Named(NamedKey::Space | NamedKey::ArrowRight | NamedKey::ArrowLeft)
                if field == 2 =>
            {
                dialog.draft.provider = dialog
                    .draft
                    .provider
                    .cycle(matches!(key, Key::Named(NamedKey::ArrowLeft)));
            }
            Key::Named(NamedKey::F2) => self.add_chat_connection(),
            Key::Named(NamedKey::F3) => self.use_chat_connection(),
            Key::Named(NamedKey::Delete) if field == 0 => self.remove_chat_connection(),
            Key::Named(NamedKey::Enter) if ctrl && dialog.offer_session_key => {
                self.save_chat_provider_from_dialog(true)
            }
            Key::Named(NamedKey::Enter) if !ctrl => self.save_chat_provider_from_dialog(false),
            _ => {
                let input = match field {
                    1 => &mut dialog.draft.name,
                    3 if dialog.draft.provider == super::connections::Provider::Compatible => {
                        &mut dialog.draft.base_url
                    }
                    4 => &mut dialog.draft.model,
                    5 => &mut dialog.api_key,
                    _ => return true,
                };
                if matches!(key, Key::Named(NamedKey::Backspace)) {
                    input.pop();
                } else if ctrl && matches!(key, Key::Character(ch) if ch.eq_ignore_ascii_case("v"))
                {
                    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                        Ok(value) => input.extend(
                            value
                                .chars()
                                .filter(|ch| !ch.is_control())
                                .take(4_096usize.saturating_sub(input.len())),
                        ),
                        Err(_) => super::connection_ui::record_connection_error(
                            "Could not paste into the chat connection form",
                        ),
                    }
                } else if !ctrl {
                    if let Some(value) = text {
                        input.extend(
                            value
                                .chars()
                                .filter(|ch| !ch.is_control())
                                .take(4_096usize.saturating_sub(input.len())),
                        );
                    }
                }
            }
        }
        true
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
            let (bx, by, bw, bh) = ProviderDialogState::bounds(width, height, lh);
            if x >= bx && x < bx + bw && y >= by && y < by + bh {
                let row = ((y - by) / lh).floor() as usize;
                let col = (((x - bx) / bw) * 3.0) as usize;
                match row {
                    1..=6 => {
                        dialog.focus_field = row - 1;
                        match row {
                            1 => self.cycle_chat_connection(x < bx + bw * 0.5),
                            3 => {
                                dialog.draft.provider =
                                    dialog.draft.provider.cycle(x < bx + bw * 0.5)
                            }
                            _ => {}
                        }
                    }
                    8 => match col {
                        0 => self.add_chat_connection(),
                        1 => self.use_chat_connection(),
                        _ => self.remove_chat_connection(),
                    },
                    9 => match col {
                        0 => self.save_chat_provider_from_dialog(false),
                        1 if dialog.offer_session_key => self.save_chat_provider_from_dialog(true),
                        2 => self.chat.provider_dialog = None,
                        _ => {}
                    },
                    _ => {}
                }
            }
            return true;
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

        if y < by + lh {
            match (((x - bx) / bw) * 3.0) as usize {
                0 => self.cycle_chat_mode(),
                1 => self.new_chat_session(),
                _ => self.chat.show_agent_sessions = !self.chat.show_agent_sessions,
            }
            return true;
        }
        if y < by + lh * 2.0 {
            if self.chat.running_turn.is_some() && x >= bx + bw * 0.75 {
                self.cancel_chat_turn();
            } else {
                self.open_chat_provider_dialog();
            }
            return true;
        }
        if y >= by + bh - lh * 3.3 && y < by + bh - lh * 2.1 {
            self.toggle_chat_attachment((((x - bx) / bw) * 3.0) as usize);
            return true;
        }

        // Pending approval buttons near bottom above input box
        if self.chat.pending_approval.is_some() && y >= by + bh - lh * 5.3 && y < by + bh - lh * 3.3
        {
            let rel_x = (x - bx) / bw.max(1.0);
            if rel_x < 0.33 {
                self.respond_chat_approval(ApprovalDecision::AllowOnce);
            } else if rel_x < 0.67 {
                let ask_read = self.chat.active_session().mode == AgentMode::Ask
                    && self
                        .chat
                        .pending_approval
                        .as_ref()
                        .is_some_and(|(pending, _)| {
                            graphite_chat::ToolApprovalPolicy::is_read_only(&pending.name)
                        });
                if !ask_read {
                    self.respond_chat_approval(ApprovalDecision::AllowAlways);
                }
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
        let list_rows = if self.chat.show_agent_sessions {
            self.chat.filtered_sessions().len().min(6)
        } else {
            0
        };
        let list_y = by + lh * 2.25;
        let edits_y = list_y
            + lh * list_rows as f32
            + if self.chat.show_agent_sessions {
                4.0
            } else {
                0.0
            };
        if has_pending_edits && y >= edits_y && y < edits_y + lh * 1.4 {
            if x < bx + bw * 0.5 {
                self.accept_all_chat_edits();
            } else {
                self.discard_all_chat_edits();
            }
            return true;
        }

        // Sessions list rows
        if self.chat.show_agent_sessions && y >= list_y && y < list_y + lh * list_rows as f32 {
            let row = ((y - list_y) / lh).floor() as usize;
            let target_id = self.chat.filtered_sessions().get(row).map(|s| s.id.clone());
            if let Some(id) = target_id.filter(|_| !self.chat_connection_busy()) {
                self.chat.clear_outgoing_draft();
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

pub(super) fn mask_api_key(key: &str) -> String {
    "\u{2022}".repeat(key.chars().count().min(12))
}
