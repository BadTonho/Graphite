//! Keyboard, mouse, session, code-block, and provider-dialog interactions for Chat.

use super::layout::{ComposerMenu, PanelLayout, Rect};
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
        if self.ai_mode.is_some() {
            self.focus_ai_chat();
            return;
        }
        if self.chat.visible && self.chat.surface == ChatSurface::SideBar {
            self.chat.visible = false;
            self.chat.focused = false;
        } else {
            self.open_chat_sidebar();
        }
    }

    pub(crate) fn open_chat_in_editor(&mut self) {
        if self.ai_mode.is_some() {
            self.focus_ai_chat();
            return;
        }
        self.chat.visible = true;
        self.chat.focused = true;
        self.chat.surface = ChatSurface::Editor;
        self.explorer_focused = false;
        self.terminal_focused = false;
    }

    pub(crate) fn open_quick_chat(&mut self) {
        if self.ai_mode.is_some() {
            self.focus_ai_chat();
            return;
        }
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
        self.set_chat_mode(self.chat.active_session().mode.next());
    }

    fn set_chat_mode(&mut self, mode: AgentMode) {
        if self.chat_connection_busy() {
            return;
        }
        self.chat.active_session_mut().mode = mode;
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = self.t(super::drawing::mode_label(mode)).into();
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
        if self.ai_mode.is_some() {
            let chat = self.ai_layout(width, height, lh).chat;
            return (chat.x, chat.y, chat.width, chat.height);
        }
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

    pub(super) fn chat_panel_layout(&self, w: f32, h: f32, lh: f32) -> PanelLayout {
        let (x, y, w, h) = self.chat_panel_bounds(w, h, lh);
        let history = if self.chat.show_agent_sessions {
            self.chat.filtered_sessions().len()
        } else {
            0
        };
        PanelLayout::new(
            Rect::new(x, y, w, h),
            lh,
            history,
            !self.chat.active_session().proposals.is_empty()
                || !self.chat.reviewed_in_batch.is_empty()
                || self
                    .chat
                    .active_session()
                    .pending_edits
                    .iter()
                    .any(|e| e.status == graphite_chat::EditReviewStatus::Pending),
            self.chat.pending_approval.is_some(),
            self.chat.attachments.selected(),
            self.chat.composer_menu,
        )
    }

    pub(super) fn chat_allows_always(&self) -> bool {
        self.chat
            .pending_approval
            .as_ref()
            .is_some_and(|(pending, _)| self.chat_can_remember_approval(&pending.name))
    }

    fn toggle_composer_menu(&mut self, menu: ComposerMenu) {
        if self.chat.active_connection().is_none() || self.chat_connection_busy() {
            return;
        }
        if self.chat.composer_menu == Some(menu) {
            self.chat.composer_menu = None;
        } else {
            self.chat.composer_menu = Some(menu);
            self.chat.composer_choice = if menu == ComposerMenu::Mode {
                match self.chat.active_session().mode {
                    AgentMode::Ask => 0,
                    AgentMode::Edit => 1,
                    AgentMode::Agent => 2,
                }
            } else {
                0
            };
        }
    }

    fn choose_composer_option(&mut self, index: usize) {
        match self.chat.composer_menu.take() {
            Some(ComposerMenu::Mode) => {
                self.set_chat_mode([AgentMode::Ask, AgentMode::Edit, AgentMode::Agent][index])
            }
            Some(ComposerMenu::Context) => self.toggle_chat_attachment(index),
            None => {}
        }
    }

    fn send_chat_draft(&mut self) {
        if self.chat.active_connection().is_some() && self.chat.running_turn.is_none() {
            self.submit_chat_prompt(self.chat.input.clone());
        }
    }

    pub(crate) fn handle_chat_wheel(&mut self, x: f32, y: f32, lines: f32) -> bool {
        if self.chat.provider_dialog.is_some() {
            return true;
        }
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let (w, h) = gpu.renderer.size();
        let layout = self.chat_panel_layout(w, h, gpu.renderer.line_height());
        if !self.chat.visible || !layout.bounds.contains(x, y) {
            return false;
        }
        if layout.body.contains(x, y) {
            let capacity = Self::review_rows(layout.body, gpu.renderer.line_height());
            let count =
                self.chat.active_session().proposals.len() + self.chat.reviewed_in_batch.len();
            if let Some(offset) = &mut self.chat.review_list {
                *offset = offset
                    .saturating_add_signed(lines.round() as isize)
                    .min(count.saturating_sub(capacity));
            } else {
                self.scroll_chat_transcript(-lines.round() as isize);
            }
        }
        true
    }

    fn scroll_chat_transcript(&mut self, older: isize) {
        let limit = self.gpu.as_ref().map_or(usize::MAX, |gpu| {
            let (w, h) = gpu.renderer.size();
            let lh = gpu.renderer.line_height();
            let body = self.chat_panel_layout(w, h, lh).body;
            let (_, body) = self.chat_task_layout(body, lh);
            let capacity = super::drawing::text_capacity(body, gpu.renderer.char_width());
            let read_only = self.chat.active_connection().is_none()
                && !self.chat.active_session().turns.is_empty();
            self.chat_message_lines(capacity, read_only)
                .len()
                .saturating_sub((body.h / lh).floor() as usize)
        });
        self.chat.transcript_offset = self
            .chat
            .transcript_offset
            .min(limit)
            .saturating_add_signed(older)
            .min(limit);
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
        if self.ai_mode.is_none() && self.handle_inline_chat_key(key, text, ctrl, alt) {
            return true;
        }
        if !self.chat.visible || !self.chat.focused {
            return false;
        }
        if self.chat.composer_menu.is_some() {
            match key {
                Key::Named(NamedKey::ArrowUp) => {
                    self.chat.composer_choice = (self.chat.composer_choice + 2) % 3
                }
                Key::Named(NamedKey::ArrowDown | NamedKey::Tab) => {
                    self.chat.composer_choice = (self.chat.composer_choice + 1) % 3
                }
                Key::Named(NamedKey::Enter | NamedKey::Space) => {
                    self.choose_composer_option(self.chat.composer_choice)
                }
                Key::Named(NamedKey::Escape) => self.chat.composer_menu = None,
                Key::Named(NamedKey::F4) => self.open_chat_provider_dialog(),
                _ => {}
            }
            return true;
        }
        match key {
            Key::Named(NamedKey::F4) => {
                self.open_chat_provider_dialog();
                true
            }
            Key::Named(NamedKey::F6) if !ctrl && !alt => {
                self.toggle_composer_menu(ComposerMenu::Context);
                true
            }
            Key::Named(NamedKey::Escape) => {
                if self.chat.running_turn.is_some() {
                    self.cancel_chat_turn();
                } else if self.chat.surface == ChatSurface::QuickChat {
                    self.close_quick_chat();
                } else if self.ai_file_group().is_some() {
                    self.focus_region(crate::app::focus_regions::FocusRegion::Editor);
                } else if self.ai_mode.is_none() {
                    self.chat.focused = false;
                }
                true
            }
            Key::Named(NamedKey::Enter) => {
                if self.chat.pending_approval.is_some() {
                    self.respond_chat_approval(ApprovalDecision::AllowOnce);
                } else {
                    self.send_chat_draft();
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
                    self.scroll_chat_transcript(1);
                }
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.chat.input_history_cursor.is_some() {
                    self.navigate_chat_input_history(false);
                } else {
                    self.scroll_chat_transcript(-1);
                }
                true
            }
            Key::Named(NamedKey::PageUp) => {
                self.scroll_chat_transcript(8);
                true
            }
            Key::Named(NamedKey::PageDown) => {
                self.scroll_chat_transcript(-8);
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
                        if self.chat.active_connection().is_some()
                            || self.chat.active_session().turns.is_empty()
                        {
                            self.chat.input.push_str(ch);
                        }
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
        if dialog.confirmation.is_some() {
            match key {
                Key::Named(NamedKey::Escape) => dialog.confirmation = None,
                Key::Named(NamedKey::Enter) => self.confirm_chat_connection_change(),
                _ => {}
            }
            return true;
        }
        if let Some(picker) = dialog.picker {
            let count = self.connection_picker_len(picker);
            let index = picker.index();
            match key {
                Key::Named(NamedKey::Escape) => self.close_connection_picker(),
                Key::Named(NamedKey::F4) => self.open_chat_provider_dialog(),
                Key::Named(NamedKey::Enter | NamedKey::Space) => {
                    self.pick_connection_option(picker, index)
                }
                Key::Named(NamedKey::ArrowDown | NamedKey::Tab) if count > 0 => {
                    self.chat.provider_dialog.as_mut().unwrap().picker =
                        Some(picker.at((index + 1) % count));
                }
                Key::Named(NamedKey::ArrowUp) if count > 0 => {
                    self.chat.provider_dialog.as_mut().unwrap().picker =
                        Some(picker.at((index + count - 1) % count));
                }
                _ => {}
            }
            return true;
        }
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
            Key::Named(NamedKey::Space) if field == 0 || field == 2 => {
                self.open_connection_picker(field)
            }
            Key::Named(NamedKey::ArrowRight) if field == 0 => self.cycle_chat_connection(false),
            Key::Named(NamedKey::ArrowLeft) if field == 0 => self.cycle_chat_connection(true),
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowLeft) if field == 2 => {
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
                if dialog
                    .field_error
                    .is_some_and(|(error_field, _)| field == error_field)
                {
                    dialog.field_error = None;
                }
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
        if self.chat.provider_dialog.is_some() {
            self.click_connection_dialog(x, y, width, height, lh);
            return true;
        }

        if let Some(inline) = self
            .chat
            .inline_chat
            .as_ref()
            .filter(|_| self.ai_mode.is_none())
        {
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
        let layout = self.chat_panel_layout(width, height, lh);
        if !layout.bounds.contains(x, y) {
            if self.ai_mode.is_none() {
                self.chat.focused = false;
            }
            self.chat.composer_menu = None;
            return false;
        }
        self.focus_region(crate::app::focus_regions::FocusRegion::Chat);

        if let Some(menu) = layout.menu {
            if let Some(index) = menu.iter().position(|rect| rect.contains(x, y)) {
                self.choose_composer_option(index);
                return true;
            }
            if !layout.mode.contains(x, y) && !layout.context.contains(x, y) {
                self.chat.composer_menu = None;
            }
        }
        if self
            .chat_task_layout(layout.body, lh)
            .0
            .is_some_and(|button| button.contains(x, y))
        {
            self.continue_chat_task();
            return true;
        }
        if self.chat.review_list.is_some()
            && layout.body.contains(x, y)
            && (!self.chat.active_session().proposals.is_empty()
                || !self.chat.reviewed_in_batch.is_empty())
        {
            let row = ((y - layout.body.y) / (lh * 1.5)) as usize;
            let count =
                self.chat.active_session().proposals.len() + self.chat.reviewed_in_batch.len();
            let capacity = Self::review_rows(layout.body, lh);
            let start = self
                .chat
                .review_list
                .unwrap_or_default()
                .min(count.saturating_sub(capacity));
            if let Some(id) = self
                .chat
                .active_session()
                .proposals
                .get(start + row)
                .filter(|_| row < capacity)
                .map(|entry| entry.id)
            {
                self.open_chat_proposal(id);
            }
        } else if layout.toolbar[0].contains(x, y) {
            self.new_chat_session();
        } else if layout.toolbar[1].contains(x, y) {
            self.chat.show_agent_sessions = !self.chat.show_agent_sessions;
        } else if layout.connection.contains(x, y)
            || (self.chat.active_connection().is_none()
                && self.chat.active_session().turns.is_empty()
                && layout.setup.bottom() <= layout.body.bottom()
                && layout.setup.contains(x, y))
        {
            self.choose_chat_connection();
        } else if layout.send.contains(x, y) {
            if self.chat.running_turn.is_some() {
                self.cancel_chat_turn();
            } else {
                self.send_chat_draft();
            }
        } else if layout.mode.contains(x, y) {
            self.toggle_composer_menu(ComposerMenu::Mode);
        } else if layout.context.contains(x, y) {
            self.toggle_composer_menu(ComposerMenu::Context);
        } else if let Some((index, _)) = layout
            .attachments
            .iter()
            .find(|(_, rect)| rect.contains(x, y))
        {
            self.toggle_chat_attachment(*index);
        } else if let Some((_, buttons)) = layout.approval.filter(|(title, buttons)| {
            title.contains(x, y) || buttons.iter().any(|rect| rect.contains(x, y))
        }) {
            if buttons[0].contains(x, y) {
                self.respond_chat_approval(ApprovalDecision::AllowOnce);
            } else if buttons[1].contains(x, y) && self.chat_allows_always() {
                self.respond_chat_approval(ApprovalDecision::AllowAlways);
            } else if buttons[2].contains(x, y) {
                self.respond_chat_approval(ApprovalDecision::Deny);
            }
        } else if let Some([title, accept, discard]) = layout
            .edits
            .filter(|[title, a, d]| title.contains(x, y) || a.contains(x, y) || d.contains(x, y))
        {
            if title.contains(x, y) {
                if self.chat.active_session().proposals.is_empty()
                    && self.chat.reviewed_in_batch.is_empty()
                {
                    self.open_next_chat_review();
                    return true;
                }
                self.chat.review_list = if self.chat.review_list.is_some() {
                    None
                } else {
                    Some(0)
                };
            } else if accept.contains(x, y) {
                self.accept_all_chat_edits();
            } else if discard.contains(x, y) {
                self.discard_all_chat_edits();
            }
        } else if let Some(row) = layout.history.iter().position(|rect| rect.contains(x, y)) {
            let target_id = self.chat.filtered_sessions().get(row).map(|s| s.id.clone());
            if let Some(id) = target_id.filter(|_| !self.chat_connection_busy()) {
                self.chat.clear_outgoing_draft();
                self.chat.active_session_id = id.clone();
                if let Some(s) = self.chat.session_by_id_mut(&id) {
                    s.unread = false;
                }
                self.chat.show_agent_sessions = false;
            }
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
