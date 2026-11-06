use super::*;
use crate::keybindings::Resolution;

impl App {
    pub(crate) fn handle_custom_keybinding(&mut self, event: &winit::event::KeyEvent) -> bool {
        let key = match &event.logical_key {
            Key::Character(value) if value == " " => "space".to_owned(),
            Key::Character(_)
                if self.modifiers.control_key()
                    && matches!(event.physical_key, PhysicalKey::Code(KeyCode::Backquote)) =>
            {
                "`".into()
            }
            Key::Character(_)
                if self.modifiers.control_key()
                    && matches!(event.physical_key, PhysicalKey::Code(KeyCode::Backslash)) =>
            {
                "\\".into()
            }
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
        self.handle_keybinding_stroke(&stroke)
    }

    pub(super) fn handle_keybinding_stroke(&mut self, stroke: &str) -> bool {
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
        match self.keymap.resolve(stroke, &context) {
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
            "workbench.action.splitEditorRight" => {
                self.split_editor_group(super::editor_groups::SplitDirection::Vertical);
            }
            "workbench.action.splitEditorDown" => {
                self.split_editor_group(super::editor_groups::SplitDirection::Horizontal);
            }
            "workbench.action.focusNextGroup" => self.focus_next_editor_group(),
            "workbench.action.findInFiles" => {
                self.search_query = Some(String::new());
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_cancel
                    .store(self.search_generation, Ordering::Relaxed);
                self.search_results.clear();
                self.search_index = 0;
                self.status = "Search: type query; Up/Down select, Enter opens result".into();
            }
            "shoppe.action.insertSnippet" => {
                self.snippets = config::load_snippets();
                self.snippet_query = Some(String::new());
                self.status = "Snippet: type prefix, Enter inserts, Esc closes".into();
            }
            "shoppe.action.importVSCode" => self.import_vscode_settings(),
            "shoppe.action.toggleTheme" => {
                let dark = !DARK_THEME.load(Ordering::Relaxed);
                DARK_THEME.store(dark, Ordering::Relaxed);
                config::save_dark_theme(dark);
            }
            "workbench.actions.view.problems" => {
                self.terminal_visible = true;
                self.terminal_problems = true;
                self.terminal_focused = false;
                self.resize_terminal();
            }
            "editor.action.toggleIndentation" => {
                let tab = &mut self.tabs[self.active_tab];
                let mut target = tab.indentation;
                target.insert_spaces = !target.insert_spaces;
                match tab.buffer.convert_indentation(target) {
                    Ok(_) => {
                        tab.indentation = target;
                        self.status = if target.insert_spaces {
                            "Indentation converted to spaces"
                        } else {
                            "Indentation converted to tabs"
                        }
                        .into();
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            "workbench.action.nextEditor" => {
                let (views, active) = self
                    .editor_groups
                    .views(self.editor_groups.focused())
                    .unwrap();
                let next = views[(active + 1) % views.len()].document;
                self.activate_editor_document(next);
            }
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
            "editor.action.selectAll" => {
                let buffer = &mut self.tabs[self.active_tab].buffer;
                buffer.set_selection(0, buffer.len_chars());
            }
            "editor.action.clipboardCopyAction" | "editor.action.clipboardCutAction" => {
                let selected = self.tabs[self.active_tab].buffer.selected_texts();
                if !selected.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if clipboard.set_text(selected.join("\n")).is_ok()
                            && command == "editor.action.clipboardCutAction"
                        {
                            self.tabs[self.active_tab].buffer.insert("");
                        }
                    }
                }
            }
            "editor.action.clipboardPasteAction" => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.tabs[self.active_tab].buffer.paste(&text);
                    }
                }
            }
            "editor.action.triggerSuggest" => {
                self.completion_items.clear();
                self.request_lsp("completion");
            }
            "editor.action.revealDefinition" => self.request_lsp("definition"),
            "editor.action.showHover" => self.request_lsp("hover"),
            "editor.action.marker.next" => self.navigate_diagnostic(1),
            "editor.action.marker.prev" => self.navigate_diagnostic(-1),
            "workbench.action.closeActiveEditor" => {
                if self.active_view_needs_confirmation() {
                    self.close_confirm = true;
                    self.status = "Unsaved tab: S save, D discard, Esc cancel".into();
                } else {
                    self.close_active_tab();
                }
            }
            _ => self.status = format!("Unknown keybinding command: {command}"),
        }
    }

    pub(super) fn reload_keybindings(&mut self) -> Result<(), String> {
        let path = config::app_config_dir().join("keybindings.json");
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "[]".into(),
            Err(error) => return Err(format!("Cannot read {}: {error}", path.display())),
        };
        self.keymap.reload(&source)
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

    #[test]
    fn split_commands_create_groups_and_focus_cycles_between_them() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("shared");
        app.execute_custom_command("workbench.action.splitEditorRight");
        let second = app.editor_groups.focused();
        assert_eq!(app.editor_groups.group_count(), 2);
        app.execute_custom_command("workbench.action.splitEditorDown");
        assert_eq!(app.editor_groups.group_count(), 3);
        let rects = app.group_rectangles(1260.0, 820.0, 20.0);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].width, 500.0);
        assert_eq!(rects[1].height, 400.0);
        app.execute_custom_command("workbench.action.focusNextGroup");
        assert_eq!(app.editor_groups.focused(), rects[0].id);
        app.execute_custom_command("workbench.action.focusNextGroup");
        assert_eq!(app.editor_groups.focused(), second);
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "shared");
    }

    #[test]
    fn user_bindings_override_and_remove_default_commands_in_the_editor() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("hello");
        assert!(app.handle_keybinding_stroke("ctrl+a"));
        assert_eq!(app.tabs[0].buffer.selected_text().as_deref(), Some("hello"));
        app.tabs[0].buffer.set_selection(0, 0);
        app.keymap
            .reload(
                r#"[
            {"key":"ctrl+a", "command":"-editor.action.selectAll"},
            {"key":"ctrl+q", "command":"editor.action.selectAll", "when":"editorTextFocus"}
        ]"#,
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("ctrl+a"));
        assert!(app.tabs[0].buffer.selection().is_none());
        assert!(app.handle_keybinding_stroke("ctrl+q"));
        assert_eq!(app.tabs[0].buffer.selected_text().as_deref(), Some("hello"));
        assert!(app.handle_keybinding_stroke("ctrl+k"));
        assert!(app.handle_keybinding_stroke("ctrl+\\"));
        assert_eq!(app.editor_groups.group_count(), 2);
    }
}
