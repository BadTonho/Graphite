use super::*;
use crate::keybindings::Resolution;

impl App {
    pub(super) fn snippet_scope_matches(&self, snippet: &config::Snippet) -> bool {
        let Some(scopes) = &snippet.scope else {
            return true;
        };
        let language = match self.tabs[self.active_tab].language {
            Some(Language::Rust) => "rust",
            Some(Language::Python) => "python",
            Some(Language::JavaScript) => "javascript",
            Some(Language::Json) => "json",
            None => return false,
        };
        scopes.iter().any(|scope| scope == language)
    }

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
                "suggestWidgetVisible".into(),
                (!self.completion_items.is_empty()).to_string(),
            ),
            (
                "findReplace".into(),
                self.local_search
                    .as_ref()
                    .is_some_and(|search| search.replace)
                    .to_string(),
            ),
            (
                "findWidgetVisible".into(),
                self.local_search.is_some().to_string(),
            ),
            (
                "quickOpenVisible".into(),
                self.quick_open.is_some().to_string(),
            ),
            (
                "projectSearchVisible".into(),
                self.search_query.is_some().to_string(),
            ),
            (
                "snippetVisible".into(),
                self.snippet_query.is_some().to_string(),
            ),
            ("quickFixVisible".into(), self.quick_fix_visible.to_string()),
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
        if !command.starts_with("suggest.") && command != "editor.action.triggerSuggest" {
            self.completion_items.clear();
        }
        use graphite_text::CursorMotion;
        let motion = match command {
            "cursorLeft" | "cursorLeftSelect" => Some(CursorMotion::Left),
            "cursorRight" | "cursorRightSelect" => Some(CursorMotion::Right),
            "cursorUp" | "cursorUpSelect" => Some(CursorMotion::Up),
            "cursorDown" | "cursorDownSelect" => Some(CursorMotion::Down),
            "cursorHome" | "cursorHomeSelect" => Some(CursorMotion::Home),
            "cursorEnd" | "cursorEndSelect" => Some(CursorMotion::End),
            _ => None,
        };
        if let Some(motion) = motion {
            self.smart_selection_history.clear();
            self.smart_selection_guard = None;
            let extend = command.ends_with("Select");
            if self.word_wrap
                && matches!(motion, CursorMotion::Up | CursorMotion::Down)
                && self.move_visual_selections(matches!(motion, CursorMotion::Down), extend)
            {
                return;
            }
            self.tabs[self.active_tab]
                .buffer
                .move_selections(motion, extend);
            return;
        }
        match command {
            "suggest.close" => self.completion_items.clear(),
            "suggest.accept" => self.accept_completion(),
            "suggest.previous" => {
                if !self.completion_items.is_empty() {
                    self.completion_index = self
                        .completion_index
                        .checked_sub(1)
                        .unwrap_or(self.completion_items.len() - 1);
                }
            }
            "suggest.next" => {
                if !self.completion_items.is_empty() {
                    self.completion_index =
                        (self.completion_index + 1) % self.completion_items.len();
                }
            }
            "quickFix.close" => self.quick_fix_visible = false,
            "quickFix.previous" => {
                if !self.quick_fix_actions.is_empty() {
                    self.quick_fix_index = self
                        .quick_fix_index
                        .checked_sub(1)
                        .unwrap_or(self.quick_fix_actions.len() - 1);
                }
            }
            "quickFix.next" => {
                if !self.quick_fix_actions.is_empty() {
                    self.quick_fix_index =
                        (self.quick_fix_index + 1) % self.quick_fix_actions.len();
                }
            }
            "quickFix.apply" => self.apply_quick_fix(),
            "snippet.close" => self.snippet_query = None,
            "snippet.deleteLeft" => {
                if let Some(query) = &mut self.snippet_query {
                    query.pop();
                }
            }
            "snippet.accept" => {
                if let Some(query) = self.snippet_query.take() {
                    if let Some(snippet) = self
                        .snippets
                        .iter()
                        .find(|s| s.prefix.starts_with(&query) && self.snippet_scope_matches(s))
                    {
                        self.tabs[self.active_tab].buffer.insert(&snippet.body);
                    }
                }
            }
            "quickOpen.close"
            | "quickOpen.accept"
            | "quickOpen.previous"
            | "quickOpen.next"
            | "quickOpen.deleteLeft" => self.execute_quick_open_command(command),
            "projectSearch.close"
            | "projectSearch.accept"
            | "projectSearch.previous"
            | "projectSearch.next"
            | "projectSearch.deleteLeft" => self.execute_project_search_command(command),
            "closeFindWidget"
            | "find.toggleField"
            | "editor.action.nextMatchFindAction"
            | "editor.action.previousMatchFindAction"
            | "editor.action.replaceAll"
            | "editor.action.replaceOne"
            | "find.deleteLeft"
            | "find.toggleRegex"
            | "find.toggleCase"
            | "find.toggleWholeWord"
            | "find.togglePreserveCase"
            | "find.toggleSelection" => self.execute_find_command(command),
            "deleteLeft" => self.tabs[self.active_tab].buffer.backspace(),
            "deleteRight" => self.tabs[self.active_tab].buffer.delete(),
            "typeNewline" => self.tabs[self.active_tab].buffer.insert_newline(),
            "tab" => {
                let tab = &mut self.tabs[self.active_tab];
                let indentation = if tab.indentation.insert_spaces {
                    " ".repeat(tab.indentation.tab_size)
                } else {
                    "\t".into()
                };
                tab.buffer.insert(&indentation);
            }
            "workbench.action.quickOpen" => self.start_quick_open(),
            "workbench.view.scm" => self.toggle_scm(),
            "shoppe.scm.stage" => self.perform_scm_action(super::scm_ui::ScmAction::Stage),
            "shoppe.scm.unstage" => self.perform_scm_action(super::scm_ui::ScmAction::Unstage),
            "shoppe.scm.stageLine" => {
                self.perform_scm_action(super::scm_ui::ScmAction::StageCursorLine)
            }
            "shoppe.scm.revertLine" => {
                self.perform_scm_action(super::scm_ui::ScmAction::RevertCursorLine)
            }
            "shoppe.scm.revertHunk" => {
                self.perform_scm_action(super::scm_ui::ScmAction::RevertFirstHunk)
            }
            "shoppe.scm.commitPrompt" => {
                self.scm_commit_prompt = true;
                self.scm_commit_message.clear();
                self.status = "Type a commit message and press Enter".into();
            }
            "workbench.action.toggleZenMode" => self.toggle_zen_mode(),
            "workbench.action.toggleCenteredEditor" => self.toggle_centered_editor(),
            "workbench.action.toggleOutline" => self.toggle_outline(),
            "workbench.action.showProcessExplorer" => self.toggle_process_explorer(),
            "workbench.action.toggleScreencastMode" => self.toggle_screencast_mode(),
            "shoppe.action.diffActiveFile" => {
                self.open_active_file_diff();
            }
            "shoppe.action.closeDiffEditor" => self.diff_view = None,
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
                let (snippets, warnings) = config::load_snippets_for_workspace(self.tree.root());
                self.snippets = snippets;
                self.snippet_load_warnings = warnings;
                self.snippet_query = Some(String::new());
                self.status = self
                    .snippet_load_warnings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Snippet: type prefix, Enter inserts, Esc closes".into());
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
                self.switch_recent_document(false);
            }
            "workbench.action.previousEditor" => self.switch_recent_document(true),
            "workbench.action.reopenClosedEditor" => self.reopen_closed_tab(),
            "workbench.action.navigateBack" => self.navigate_history(-1),
            "workbench.action.navigateForward" => self.navigate_history(1),
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
                let mut clipboard = super::clipboard::SystemClipboard;
                super::clipboard::copy_or_cut(
                    &mut self.tabs[self.active_tab].buffer,
                    &mut clipboard,
                    command == "editor.action.clipboardCutAction",
                );
            }
            "editor.action.clipboardPasteAction" => {
                let mut clipboard = super::clipboard::SystemClipboard;
                super::clipboard::paste(&mut self.tabs[self.active_tab].buffer, &mut clipboard);
            }
            "editor.action.triggerSuggest" => {
                self.completion_items.clear();
                self.request_lsp("completion");
            }
            "editor.action.revealDefinition" => self.request_lsp("definition"),
            "editor.action.showHover" => self.request_lsp("hover"),
            "editor.action.quickFix" => self.request_quick_fixes(),
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

    fn diagnostic(start: u32, end: u32, message: &str) -> lsp_types::Diagnostic {
        serde_json::from_value(serde_json::json!({
            "range": {
                "start": {"line": 0, "character": start},
                "end": {"line": 0, "character": end}
            },
            "severity": 1,
            "message": message
        }))
        .unwrap()
    }

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

    #[test]
    fn navigation_bindings_extend_and_collapse_unicode_selections() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("aé🙂b");
        app.tabs[0].buffer.set_selections(vec![
            graphite_text::Selection { anchor: 1, head: 1 },
            graphite_text::Selection { anchor: 3, head: 3 },
        ]);
        assert!(app.handle_keybinding_stroke("shift+right"));
        assert_eq!(app.tabs[0].buffer.selected_texts(), ["é", "b"]);
        assert!(app.handle_keybinding_stroke("left"));
        assert_eq!(
            app.tabs[0].buffer.selections(),
            vec![
                graphite_text::Selection { anchor: 1, head: 1 },
                graphite_text::Selection { anchor: 3, head: 3 },
            ]
        );
        assert!(app.handle_keybinding_stroke("backspace"));
        assert_eq!(app.tabs[0].buffer.text(), "éb");
        assert!(app.handle_keybinding_stroke("ctrl+z"));
        assert_eq!(app.tabs[0].buffer.text(), "aé🙂b");
    }

    #[test]
    fn suggestion_widget_keeps_arrow_and_enter_for_completion() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.completion_items.push(lsp_types::CompletionItem {
            label: "first".into(),
            ..Default::default()
        });
        app.completion_items.push(lsp_types::CompletionItem {
            label: "second".into(),
            ..Default::default()
        });
        assert!(app.handle_keybinding_stroke("down"));
        assert_eq!(app.completion_index, 1);
        assert!(app.handle_keybinding_stroke("enter"));
        assert_eq!(app.tabs[0].buffer.text(), "second");
        assert!(app.completion_items.is_empty());
    }

    #[test]
    fn snippets_and_suggestions_accept_remapped_commands() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.snippet_query = Some("hel".into());
        app.snippets.push(config::Snippet {
            scope: None,
            prefix: "hello".into(),
            body: "hello 🙂".into(),
        });
        app.keymap
            .reload(
                r#"[
            {"key":"enter","command":"-snippet.accept","when":"snippetVisible"},
            {"key":"ctrl+j","command":"snippet.accept","when":"snippetVisible"}
        ]"#,
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("enter"));
        assert_eq!(app.tabs[0].buffer.text(), "");
        assert!(app.handle_keybinding_stroke("ctrl+j"));
        assert_eq!(app.tabs[0].buffer.text(), "hello 🙂");
        assert!(app.snippet_query.is_none());
        app.tabs[0].buffer.undo();
        assert_eq!(app.tabs[0].buffer.text(), "");
    }

    #[test]
    fn find_commands_respect_context_remapping_and_invalid_regex() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("é one one");
        app.open_local_search(true);
        app.local_search.as_mut().unwrap().query = "one".into();
        assert!(app.handle_keybinding_stroke("enter"));
        assert_eq!(app.tabs[0].buffer.selected_text().as_deref(), Some("one"));
        app.keymap
            .reload(
                r#"[
          {"key":"alt+r","command":"-find.toggleRegex","when":"findWidgetVisible"},
          {"key":"ctrl+r","command":"find.toggleRegex","when":"findWidgetVisible"}
        ]"#,
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("alt+r"));
        assert!(!app.local_search.as_ref().unwrap().options.regex);
        assert!(app.handle_keybinding_stroke("ctrl+r"));
        app.local_search.as_mut().unwrap().query = "(".into();
        let revision = app.tabs[0].buffer.revision();
        assert!(app.handle_keybinding_stroke("ctrl+enter"));
        assert_eq!(app.tabs[0].buffer.revision(), revision);
        assert!(app
            .local_search
            .as_ref()
            .unwrap()
            .message
            .contains("Invalid regex"));
        assert!(app.handle_keybinding_stroke("escape"));
        assert!(app.local_search.is_none());
    }

    #[test]
    fn quick_open_keybindings_navigate_open_and_cancel_stale_requests() {
        let dir = crate::test_support::TempDir::new();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "alpha").unwrap();
        std::fs::write(&b, "beta").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.start_quick_open();
        let generation = app.search_generation;
        app.quick_open.as_mut().unwrap().paths = vec![a, b.clone()];
        assert!(app.handle_keybinding_stroke("down"));
        assert_eq!(app.quick_open.as_ref().unwrap().index, 1);
        assert!(app.handle_keybinding_stroke("enter"));
        assert!(app.quick_open.is_none());
        assert_eq!(app.tabs[app.active_tab].buffer.path(), Some(b.as_path()));
        assert!(!app.accept_quick_open("", generation, vec![b]));
    }

    #[test]
    fn project_search_modal_binding_can_be_removed_and_remapped() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.search_query = Some("abc".into());
        app.keymap
            .reload(
                r#"[
          {"key":"backspace", "command":"-projectSearch.deleteLeft"},
          {"key":"ctrl+j", "command":"projectSearch.deleteLeft", "when":"projectSearchVisible"}
        ]"#,
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("backspace"));
        assert_eq!(app.search_query.as_deref(), Some("abc"));
        assert!(app.handle_keybinding_stroke("ctrl+j"));
        assert_eq!(app.search_query.as_deref(), Some("ab"));
        assert!(app.handle_keybinding_stroke("escape"));
        assert!(app.search_query.is_none());
    }

    #[test]
    fn f8_visits_each_same_line_diagnostic_and_problems_include_schema_errors() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("one two");
        app.tabs[0].buffer.set_selection(0, 0);
        app.tabs[0].diagnostics = vec![diagnostic(0, 3, "first"), diagnostic(4, 7, "second")];
        app.tabs[0].schema_diagnostics = vec![diagnostic(0, 1, "schema")];
        assert_eq!(app.diagnostic_entries().len(), 3);

        assert!(app.handle_keybinding_stroke("f8"));
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 4);
    }

    #[test]
    fn f8_cycles_diagnostics_that_share_the_same_range() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("problem");
        app.tabs[0].buffer.set_selection(0, 0);
        app.tabs[0].diagnostics = vec![diagnostic(0, 7, "first"), diagnostic(0, 7, "second")];

        app.navigate_diagnostic(1);
        assert_eq!(
            app.diagnostic_navigation
                .as_ref()
                .map(|(_, _, message)| message.as_str()),
            Some("first")
        );
        app.navigate_diagnostic(1);
        assert_eq!(
            app.diagnostic_navigation
                .as_ref()
                .map(|(_, _, message)| message.as_str()),
            Some("second")
        );
    }

    #[test]
    fn navigation_history_and_recent_tab_bindings_restore_editor_locations() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.rs");
        let second = dir.path().join("second.rs");
        std::fs::write(&first, "a\u{1f642}b").unwrap();
        std::fs::write(&second, "second").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first.clone());
        app.tabs[0].buffer.set_selection(1, 1);
        app.push_navigation_location();
        app.tabs[0].buffer.set_selection(3, 3);
        app.push_navigation_location();
        app.handle_keybinding_stroke("alt+left");
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 1);
        app.handle_keybinding_stroke("alt+right");
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 3);

        app.open_file(second.clone());
        app.handle_keybinding_stroke("ctrl+tab");
        assert_eq!(
            app.tabs[app.active_tab].buffer.path(),
            Some(first.as_path())
        );
        app.handle_keybinding_stroke("ctrl+shift+tab");
        assert_eq!(
            app.tabs[app.active_tab].buffer.path(),
            Some(second.as_path())
        );
    }

    #[test]
    fn closed_tab_stack_reopens_the_file_and_keeps_its_contents() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("restore.rs");
        std::fs::write(&path, "restore me").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path.clone());
        app.close_active_tab();
        assert!(app.tabs[app.active_tab].buffer.path().is_none());
        assert!(app.handle_keybinding_stroke("ctrl+shift+t"));
        assert_eq!(app.tabs[app.active_tab].buffer.path(), Some(path.as_path()));
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "restore me");
    }

    #[test]
    fn snippet_language_scope_is_applied_to_suggestions() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let snippet = config::Snippet {
            prefix: "function".into(),
            body: "fn name() {}".into(),
            scope: Some(vec!["rust".into()]),
        };
        app.tabs[0].language = Some(Language::Rust);
        assert!(app.snippet_scope_matches(&snippet));
        app.tabs[0].language = Some(Language::Python);
        assert!(!app.snippet_scope_matches(&snippet));
    }

    #[test]
    fn terminal_tab_scrolling_reaches_sessions_beyond_the_visible_range() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.terminal_names = (0..8).map(|index| format!("Shell {index}")).collect();
        app.scroll_terminal_tabs(1);
        app.scroll_terminal_tabs(1);
        assert_eq!(app.terminal_tab_offset, 2);
        app.scroll_terminal_tabs(1);
        app.scroll_terminal_tabs(1);
        app.scroll_terminal_tabs(1);
        app.scroll_terminal_tabs(1);
        app.scroll_terminal_tabs(1);
        app.scroll_terminal_tabs(1);
        assert_eq!(app.terminal_tab_offset, 7);
        app.scroll_terminal_tabs(-1);
        assert_eq!(app.terminal_tab_offset, 6);
    }

    #[test]
    fn quick_fix_applies_utf16_workspace_edits_and_supports_undo_redo() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("unicode.rs");
        std::fs::write(&path, "a\u{1f642}b").unwrap();
        let uri = crate::lsp::path_to_uri(&path).unwrap();
        let mut changes = serde_json::Map::new();
        changes.insert(
            uri,
            serde_json::json!([{
                "range": {
                    "start": {"line": 0, "character": 1},
                    "end": {"line": 0, "character": 3}
                },
                "newText": "X"
            }]),
        );
        let action = serde_json::from_value(serde_json::json!({
            "title": "Replace emoji",
            "kind": "quickfix",
            "edit": {"changes": changes}
        }))
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.quick_fix_actions = vec![action];
        app.apply_quick_fix();
        assert_eq!(
            app.tabs[app.active_tab].buffer.text(),
            "aXb",
            "quick fix status: {}",
            app.status
        );
        app.tabs[app.active_tab].buffer.undo();
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "a\u{1f642}b");
        app.tabs[app.active_tab].buffer.redo();
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "aXb");
    }

    #[test]
    fn quick_fix_does_not_edit_a_read_only_document() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("readonly.rs");
        std::fs::write(&path, "original").unwrap();
        let uri = crate::lsp::path_to_uri(&path).unwrap();
        let mut changes = serde_json::Map::new();
        changes.insert(
            uri,
            serde_json::json!([{
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 8}
                },
                "newText": "changed"
            }]),
        );
        let action = serde_json::from_value(serde_json::json!({
            "title": "Change",
            "kind": "quickfix",
            "edit": {"changes": changes}
        }))
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.tabs[app.active_tab].buffer.set_read_only(true);
        app.quick_fix_actions = vec![action];
        app.apply_quick_fix();
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "original");
        assert!(app.status.contains("read-only"), "status: {}", app.status);
    }
}
