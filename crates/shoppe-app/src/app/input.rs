use super::*;

impl App {
    pub(crate) fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        let ctrl = self.modifiers.control_key();
        if self.scm_commit_prompt {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.scm_commit_prompt = false;
                    self.status = "Commit cancelled".into();
                }
                Key::Named(NamedKey::Enter) => {
                    self.scm_commit_prompt = false;
                    self.perform_scm_action(super::scm_ui::ScmAction::Commit);
                }
                Key::Named(NamedKey::Backspace) => {
                    self.scm_commit_message.pop();
                }
                _ if !ctrl && !self.modifiers.alt_key() => {
                    if let Some(text) = event
                        .text
                        .as_ref()
                        .filter(|text| !text.chars().any(char::is_control))
                    {
                        self.scm_commit_message.push_str(text);
                    }
                }
                _ => {}
            }
            return;
        }
        if self.branch_picker {
            match event.logical_key {
                Key::Named(NamedKey::Escape) => self.branch_picker = false,
                Key::Named(NamedKey::ArrowDown) => {
                    self.branch_selected = (self.branch_selected + 1)
                        .min(self.scm_snapshot.branches.len().saturating_sub(1))
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.branch_selected = self.branch_selected.saturating_sub(1)
                }
                Key::Named(NamedKey::Enter) => self.checkout_selected_branch(),
                _ => {}
            }
            return;
        }
        if self.conflict_view.is_some() {
            match event
                .text
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "o" => self.resolve_conflict_choice(1),
                "t" => self.resolve_conflict_choice(2),
                "b" => self.resolve_conflict_choice(0),
                "e" => self.resolve_conflict_choice(3),
                _ if matches!(event.logical_key, Key::Named(NamedKey::Escape)) => {
                    self.conflict_view = None
                }
                _ => {}
            }
            return;
        }
        if self.diff_view.is_some() {
            if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                self.diff_view = None;
                self.status = "Diff view closed".into();
                self.check_cli_waiters();
            } else if matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
                if let Some(diff) = &mut self.diff_view {
                    diff.inline = !diff.inline;
                }
            }
            return;
        }
        if self.scm_visible && !ctrl && !self.modifiers.alt_key() {
            match event
                .text
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "s" => {
                    self.perform_scm_action(super::scm_ui::ScmAction::Stage);
                    return;
                }
                "u" => {
                    self.perform_scm_action(super::scm_ui::ScmAction::Unstage);
                    return;
                }
                "c" => {
                    self.scm_commit_prompt = true;
                    self.scm_commit_message.clear();
                    return;
                }
                "h" => {
                    self.perform_scm_action(super::scm_ui::ScmAction::StageHunk(0));
                    return;
                }
                "r" => {
                    self.open_selected_conflict();
                    return;
                }
                _ => {}
            }
            match event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.scm_selected = (self.scm_selected + 1)
                        .min(self.scm_snapshot.files.len().saturating_sub(1));
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.scm_selected = self.scm_selected.saturating_sub(1);
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(path) = self.scm_selected_path() {
                        self.open_file(path);
                    }
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    self.scm_visible = false;
                    return;
                }
                _ => {}
            }
        }
        if self.close_window_confirm {
            match &event.logical_key {
                Key::Character(c) if c.eq_ignore_ascii_case("s") => self.save_all_before_close(),
                Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                    self.close_window_confirm = false;
                    self.exit_requested = true;
                }
                Key::Named(NamedKey::Escape) => {
                    self.close_window_confirm = false;
                    self.status = "Close cancelled".into();
                }
                _ => {}
            }
            return;
        }
        if !self.close_confirm
            && !self.editor_groups.active_view_mut().ime_composing
            && self.handle_custom_keybinding(event)
        {
            return;
        }
        if self.terminal_focused {
            self.handle_terminal_key(event);
            return;
        }
        if self.quick_open.is_some() {
            self.handle_quick_open(event);
            return;
        }
        if self.local_search.is_some() {
            self.handle_local_search(event);
            return;
        }
        self.editor_groups
            .active_view_mut()
            .smart_selection_history
            .clear();
        if !self.completion_items.is_empty() {
            self.completion_items.clear();
        }
        if self.close_confirm {
            let key = event.text.as_deref().unwrap_or("").to_ascii_lowercase();
            if key == "d" {
                self.close_confirm = false;
                self.close_active_tab();
                return;
            }
            if key == "s" {
                self.save();
                self.close_confirm = false;
                if !self.documents[self.active_document()].is_dirty() {
                    self.close_active_tab();
                }
                return;
            }
            if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                self.close_confirm = false;
                self.status = "Close cancelled".into();
            }
            return;
        }
        if self.snippet_query.is_some() {
            if !ctrl && !self.modifiers.alt_key() {
                if let Some(text) = event
                    .text
                    .as_ref()
                    .filter(|_| !self.editor_groups.active_view_mut().ime_composing)
                {
                    if !text.chars().any(char::is_control) {
                        if let Some(q) = &mut self.snippet_query {
                            q.push_str(text);
                        }
                    }
                }
            }
            return;
        }
        if self.search_query.is_some() {
            if !self.editor_groups.active_view_mut().ime_composing
                && !ctrl
                && !self.modifiers.alt_key()
            {
                if let Some(text) = &event.text {
                    if !text.chars().any(char::is_control) {
                        if let Some(q) = &mut self.search_query {
                            q.push_str(text);
                        }
                        self.start_project_search();
                    }
                }
            }
            return;
        }
        if !ctrl && !self.editor_groups.active_view_mut().ime_composing {
            if let Some(text) = &event.text {
                if !text.chars().any(char::is_control) {
                    self.editor_mut().insert(text);
                }
            }
        }
    }
}
