use super::*;
use crate::keybindings::Resolution;

impl App {
    pub(crate) fn handle_custom_keybinding(&mut self, event: &winit::event::KeyEvent) -> bool {
        let key = match &event.logical_key {
            Key::Character(value) if value == " " => "space".to_owned(),
            Key::Character(value) => value.to_lowercase(),
            Key::Named(key) => format!("{key:?}").to_lowercase(),
            _ => return false,
        };
        let mut stroke = String::new();
        if self.modifiers.control_key() {
            stroke.push_str("ctrl+");
        }
        if self.modifiers.shift_key() {
            stroke.push_str("shift+");
        }
        if self.modifiers.alt_key() {
            stroke.push_str("alt+");
        }
        if self.modifiers.super_key() {
            stroke.push_str("meta+");
        }
        stroke.push_str(&key);
        let buffer = &self.tabs[self.active_tab].buffer;
        let editor_focus = !self.terminal_focused
            && self.quick_open.is_none()
            && self.local_search.is_none()
            && self.search_query.is_none()
            && self.snippet_query.is_none();
        let context = HashMap::from([
            ("editorTextFocus".into(), editor_focus.to_string()),
            ("editorReadonly".into(), buffer.is_read_only().to_string()),
            (
                "editorHasSelection".into(),
                buffer.selection().is_some().to_string(),
            ),
            (
                "multipleSelections".into(),
                (buffer.selections().len() > 1).to_string(),
            ),
            ("terminalFocus".into(), self.terminal_focused.to_string()),
            (
                "findWidgetVisible".into(),
                self.local_search.is_some().to_string(),
            ),
            (
                "quickOpenVisible".into(),
                self.quick_open.is_some().to_string(),
            ),
        ]);
        match self.keymap.resolve(&stroke, &context) {
            Resolution::Unmatched => false,
            Resolution::Pending => {
                self.status = "Waiting for next key in chord".into();
                true
            }
            Resolution::Unbound => true,
            Resolution::Command(command) => {
                self.execute_custom_command(&command);
                true
            }
        }
    }

    pub(crate) fn execute_custom_command(&mut self, command: &str) {
        match command {
            "workbench.action.quickOpen" => self.start_quick_open(),
            "actions.find" => self.open_local_search(false),
            "editor.action.startFindReplaceAction" => self.open_local_search(true),
            "workbench.action.files.save" => self.save(),
            "workbench.action.terminal.toggleTerminal" => self.toggle_terminal(),
            "editor.action.smartSelect.expand" => self.smart_select(true),
            "editor.action.smartSelect.shrink" => self.smart_select(false),
            "editor.action.addSelectionToNextFindMatch" => {
                self.tabs[self.active_tab].buffer.select_occurrences(false)
            }
            "editor.action.selectHighlights" => {
                self.tabs[self.active_tab].buffer.select_occurrences(true)
            }
            "editor.action.moveLinesUpAction" => self.tabs[self.active_tab].buffer.move_line(-1),
            "editor.action.moveLinesDownAction" => self.tabs[self.active_tab].buffer.move_line(1),
            "editor.action.copyLinesUpAction" => {
                self.tabs[self.active_tab].buffer.duplicate_line(false)
            }
            "editor.action.copyLinesDownAction" => {
                self.tabs[self.active_tab].buffer.duplicate_line(true)
            }
            "editor.action.deleteLines" => self.tabs[self.active_tab].buffer.delete_line(),
            "editor.action.sortLinesAscending" => self.tabs[self.active_tab].buffer.sort_lines(),
            "editor.action.removeDuplicateLines" => {
                self.tabs[self.active_tab].buffer.deduplicate_lines()
            }
            "undo" => self.tabs[self.active_tab].buffer.undo(),
            "redo" => self.tabs[self.active_tab].buffer.redo(),
            "editor.action.triggerSuggest" => self.request_lsp("completion"),
            "editor.action.revealDefinition" => self.request_lsp("definition"),
            "editor.action.showHover" => self.request_lsp("hover"),
            "editor.action.marker.next" => self.navigate_diagnostic(1),
            "editor.action.marker.prev" => self.navigate_diagnostic(-1),
            "workbench.action.closeActiveEditor" => {
                if self.tabs[self.active_tab].buffer.is_dirty() {
                    self.close_confirm = true;
                    self.status = "Unsaved tab: S save, D discard, Esc cancel".into();
                } else {
                    self.close_active_tab();
                }
            }
            _ => self.status = format!("Unknown keybinding command: {command}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_preserve_readonly_and_unsaved_close_protection() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("original\n");
        app.tabs[0].buffer.set_read_only(true);
        app.execute_custom_command("editor.action.deleteLines");
        app.execute_custom_command("undo");
        assert_eq!(app.tabs[0].buffer.text(), "original\n");
        app.execute_custom_command("workbench.action.closeActiveEditor");
        assert!(app.close_confirm);
        assert_eq!(app.tabs[0].buffer.text(), "original\n");
    }
}
