use super::*;

impl App {
    pub(crate) fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        if self.plugin_key(event) {
            return;
        }
        let ctrl = self.modifiers.control_key();
        if self.scm_commit_prompt {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.run_ui_command("graphite.commit.cancel");
                }
                Key::Named(NamedKey::Enter) => {
                    self.run_ui_command("graphite.scm.commit");
                }
                Key::Named(NamedKey::Backspace) => {
                    self.run_ui_command("graphite.commit.deleteLeft");
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
                Key::Named(NamedKey::Escape) => self.run_ui_command("graphite.branch.close"),
                Key::Named(NamedKey::ArrowDown) => self.run_ui_command("graphite.branch.next"),
                Key::Named(NamedKey::ArrowUp) => self.run_ui_command("graphite.branch.previous"),
                Key::Named(NamedKey::Enter) => self.run_ui_command("graphite.scm.checkout"),
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
                "o" => self.run_ui_command_args(
                    "graphite.conflict.resolve",
                    serde_json::json!({"index":1}),
                ),
                "t" => self.run_ui_command_args(
                    "graphite.conflict.resolve",
                    serde_json::json!({"index":2}),
                ),
                "b" => self.run_ui_command_args(
                    "graphite.conflict.resolve",
                    serde_json::json!({"index":0}),
                ),
                "e" => self.run_ui_command_args(
                    "graphite.conflict.resolve",
                    serde_json::json!({"index":3}),
                ),
                _ if matches!(event.logical_key, Key::Named(NamedKey::Escape)) => {
                    self.run_ui_command("graphite.conflict.close")
                }
                _ => {}
            }
            return;
        }
        if self.diff_view.is_some() {
            if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                self.run_ui_command("graphite.action.closeDiffEditor");
                self.status = self.t("diff.closed").into();
                self.check_cli_waiters();
            } else if matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
                self.run_ui_command("graphite.diff.toggleLayout");
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
                    self.run_ui_command("graphite.scm.stage");
                    return;
                }
                "u" => {
                    self.run_ui_command("graphite.scm.unstage");
                    return;
                }
                "c" => {
                    self.run_ui_command("graphite.scm.promptCommit");
                    return;
                }
                "h" => {
                    self.run_ui_command("graphite.scm.stageHunk");
                    return;
                }
                "r" => {
                    self.run_ui_command("graphite.scm.openConflict");
                    return;
                }
                _ => {}
            }
            match event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.run_ui_command("graphite.scm.next");
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.run_ui_command("graphite.scm.previous");
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    self.run_ui_command("graphite.scm.open");
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    self.run_ui_command("graphite.scm.close");
                    return;
                }
                _ => {}
            }
        }
        if self.close_window_confirm {
            match &event.logical_key {
                Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                    self.run_ui_command("graphite.window.saveAndClose")
                }
                Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                    self.run_ui_command("graphite.window.discardAndClose")
                }
                Key::Named(NamedKey::Escape) => self.run_ui_command("graphite.window.cancelClose"),
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
                self.run_ui_command("graphite.close.discard");
            } else if key == "s" {
                self.run_ui_command("graphite.close.save");
            } else if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                self.run_ui_command("graphite.close.cancel");
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
