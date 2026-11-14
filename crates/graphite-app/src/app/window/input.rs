use super::*;

impl App {
    pub(crate) fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        if self.help_modal_key(
            &event.logical_key,
            event.text.as_deref(),
            self.modifiers.control_key(),
        ) {
            return;
        }
        if self.handle_github_dialog_key(&event.logical_key) {
            return;
        }
        if self.handle_branch_name_prompt_key(
            &event.logical_key,
            (!self.modifiers.control_key() && !self.modifiers.alt_key())
                .then_some(event.text.as_deref())
                .flatten(),
        ) {
            return;
        }
        if self.handle_rename_prompt_key(
            &event.logical_key,
            (!self.modifiers.control_key() && !self.modifiers.alt_key())
                .then_some(event.text.as_deref())
                .flatten(),
        ) {
            return;
        }
        if self.handle_explorer_prompt_key(
            &event.logical_key,
            (!self.modifiers.control_key() && !self.modifiers.alt_key())
                .then_some(event.text.as_deref())
                .flatten(),
        ) {
            return;
        }
        if self.handle_outline_key(
            &event.logical_key,
            (!self.modifiers.control_key() && !self.modifiers.alt_key())
                .then_some(event.text.as_deref())
                .flatten(),
        ) {
            return;
        }
        if self.handle_breakpoint_prompt_key(
            &event.logical_key,
            (!self.modifiers.control_key() && !self.modifiers.alt_key())
                .then_some(event.text.as_deref())
                .flatten(),
        ) {
            return;
        }
        if self.workbench_menu_key(&event.logical_key) {
            return;
        }
        if self.profile_menu_key(&event.logical_key) {
            return;
        }
        if self.settings_menu_key(&event.logical_key) {
            return;
        }
        if self.handle_command_center_key(&event.logical_key, event.text.as_deref()) {
            return;
        }
        if self.activity_bar_menu_key(&event.logical_key) {
            return;
        }
        if self.graphite_detail_key(event) {
            return;
        }
        if self.plugin_key(event) {
            return;
        }
        if self.handle_chat_key(
            &event.logical_key,
            event.text.as_deref(),
            self.modifiers.control_key(),
            self.modifiers.alt_key(),
        ) {
            return;
        }
        let ctrl = self.modifiers.control_key();
        if self
            .file_shortcut_chord
            .is_some_and(|deadline| std::time::Instant::now() <= deadline)
        {
            self.file_shortcut_chord = None;
            if ctrl
                && event
                    .text
                    .as_deref()
                    .is_some_and(|text| text.eq_ignore_ascii_case("o"))
            {
                self.open_folder_dialog();
                return;
            }
            if ctrl
                && event
                    .text
                    .as_deref()
                    .is_some_and(|text| text.eq_ignore_ascii_case("s"))
            {
                if self.modifiers.shift_key() {
                    self.open_preference_file("keybindings.json");
                } else {
                    self.run_ui_command("workbench.action.files.saveAll");
                }
                return;
            }
            if ctrl
                && event
                    .text
                    .as_deref()
                    .is_some_and(|text| text.eq_ignore_ascii_case("r"))
            {
                self.open_help_modal(super::help_menu::HelpPage::KeyboardShortcuts);
                return;
            }
        } else {
            self.file_shortcut_chord = None;
        }
        if ctrl && !self.editor_groups.active_view_mut().ime_composing {
            if let Key::Character(key) = &event.logical_key {
                match key.to_ascii_lowercase().as_str() {
                    "k" if !self.modifiers.shift_key() => {
                        self.file_shortcut_chord =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
                        self.status = self.t("status.waiting_chord").into();
                        return;
                    }
                    "n" if self.modifiers.shift_key() => {
                        self.spawn_window(None);
                        return;
                    }
                    "n" => {
                        self.new_untitled_document();
                        return;
                    }
                    "o" if !self.modifiers.shift_key() => {
                        self.open_file_dialog();
                        return;
                    }
                    "s" if self.modifiers.shift_key() => {
                        self.save_as_dialog();
                        return;
                    }
                    "s" if !self.modifiers.shift_key() => {
                        if self.documents[self.active_document()]
                            .buffer
                            .path()
                            .is_some()
                        {
                            self.run_ui_command("workbench.action.files.save");
                        } else {
                            self.save_as_dialog();
                        }
                        return;
                    }
                    _ => {}
                }
            }
            if matches!(event.logical_key, Key::Named(NamedKey::F4)) && !self.modifiers.alt_key() {
                self.run_ui_command("workbench.action.closeActiveEditor");
                return;
            }
        }
        if self.scm_extension_input.is_some()
            && self.handle_extension_scm_input_key(
                &event.logical_key,
                event.text.as_deref(),
                ctrl,
                self.modifiers.alt_key(),
            )
        {
            return;
        }
        if self.scm_commit_prompt {
            if ctrl && matches!(event.logical_key, Key::Named(NamedKey::Enter)) {
                if !self.scm_commit_message.trim().is_empty() {
                    self.run_ui_command("graphite.scm.commit");
                }
                return;
            }
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.run_ui_command("graphite.commit.cancel");
                }
                Key::Named(NamedKey::Enter) => {
                    self.scm_commit_prompt = false;
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
        if self.multi_diff.is_some() {
            self.handle_multi_diff_key(event);
            return;
        }
        if self.merge_editor.is_some() && self.diff_view.is_none() {
            self.handle_merge_key(event);
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
                "m" => self.run_ui_command("graphite.conflict.openMergeEditor"),
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
        if self.debug_input_focused {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => self.debug_input_focused = false,
                Key::Named(NamedKey::Enter) => {
                    let result = if ctrl {
                        self.add_debug_watch_from_input()
                    } else {
                        self.evaluate_debug_input()
                    };
                    if let Err(error) = result {
                        self.status = error;
                    }
                }
                Key::Named(NamedKey::Backspace) => {
                    self.debug_input.pop();
                }
                _ if !ctrl && !self.modifiers.alt_key() => {
                    if let Some(text) = event
                        .text
                        .as_ref()
                        .filter(|text| !text.chars().any(char::is_control))
                    {
                        self.debug_input.push_str(text);
                    }
                }
                _ => {}
            }
            return;
        }
        if self.tests_visible {
            match event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.tests_visible = false;
                }
                Key::Named(NamedKey::Enter) => {
                    if let Err(error) = self.run_selected_test() {
                        self.status = error;
                    }
                }
                Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowUp) => {
                    let roots = self
                        .test_explorer
                        .controllers
                        .values()
                        .flat_map(|controller| controller.roots.iter().cloned())
                        .collect::<Vec<_>>();
                    let items = super::phase6_drawing::test_items(&roots);
                    if !items.is_empty() {
                        let current = self
                            .test_explorer
                            .selected
                            .as_ref()
                            .and_then(|id| items.iter().position(|(_, item)| &item.id == id))
                            .unwrap_or(0);
                        let next = if matches!(event.logical_key, Key::Named(NamedKey::ArrowDown)) {
                            (current + 1) % items.len()
                        } else {
                            (current + items.len() - 1) % items.len()
                        };
                        self.test_explorer.selected = Some(items[next].1.id.clone());
                    }
                }
                _ => {}
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
                "d" => {
                    self.run_ui_command("graphite.scm.openAllChanges");
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
        if self.command_palette.is_some() {
            self.handle_command_palette(event);
            return;
        }
        if self.go_picker.is_some() {
            self.handle_go_picker(event);
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
                        self.push_project_search_text(text);
                    }
                }
            }
            return;
        }
        if self.active_notebook().is_some() && !self.editor_groups.active_view_mut().ime_composing {
            self.handle_notebook_key(event);
            return;
        }
        if self.custom_editor_state(self.active_document()).is_some() {
            // The text behind an extension's editor is not typed into.
            return;
        }
        if !ctrl && !self.editor_groups.active_view_mut().ime_composing {
            if let Some(text) = &event.text {
                if !text.chars().any(char::is_control) {
                    self.editor_mut().insert(text);
                    self.format_on_type(text);
                }
            }
        }
    }
}
