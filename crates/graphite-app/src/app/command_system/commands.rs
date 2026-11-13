use super::*;
use crate::keybindings::Resolution;

impl App {
    pub(crate) fn snippet_scope_matches(&self, snippet: &config::Snippet) -> bool {
        let Some(scopes) = &snippet.scope else {
            return true;
        };
        let language = match self.documents[self.command_document()].language {
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

    pub(crate) fn handle_keybinding_stroke(&mut self, stroke: &str) -> bool {
        self.refresh_command_context();
        let target =
            super::command_registry::CommandTarget::View(self.editor_groups.active_view().id.0);
        let context = self.context_keys.resolve_target(target);
        match self.keymap.resolve(stroke, &context) {
            Resolution::Unmatched => false,
            Resolution::Pending => {
                self.status = self.t("status.waiting_chord").into();
                true
            }
            Resolution::Unbound => true,
            Resolution::Command { id, arguments } => {
                let target = self.ui_target_for_command(&id);
                let ticket = self.execute_command(super::command_execution::CommandRequest {
                    id,
                    arguments,
                    target,
                });
                if let Some(Err(error)) = ticket.try_result() {
                    self.status = self.command_error_text(&error);
                }
                true
            }
        }
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
        app.test_editor_mut(0).insert("original\n");
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .buffer
            .set_read_only(true);
        app.run_ui_command("editor.action.deleteLines");
        app.run_ui_command("undo");
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "original\n"
        );
        app.run_ui_command("workbench.action.closeActiveEditor");
        assert!(app.close_confirm);
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "original\n"
        );
    }

    #[test]
    fn split_commands_create_groups_and_focus_cycles_between_them() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("shared");
        app.run_ui_command("workbench.action.splitEditorRight");
        let second = app.editor_groups.focused();
        assert_eq!(app.editor_groups.group_count(), 2);
        app.run_ui_command("workbench.action.splitEditorDown");
        assert_eq!(app.editor_groups.group_count(), 3);
        let rects = app.group_rectangles(SIDEBAR_WIDTH + 1000.0, 820.0, 20.0);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].width, 500.0);
        let expected_half_height = (820.0 - 20.0 - app.activity_bar_vertical_offset()) / 2.0;
        assert_eq!(rects[1].height, expected_half_height);
        app.run_ui_command("workbench.action.focusNextGroup");
        assert_eq!(app.editor_groups.focused(), rects[0].id);
        app.run_ui_command("workbench.action.focusNextGroup");
        assert_eq!(app.editor_groups.focused(), second);
        assert_eq!(app.documents[app.active_document()].buffer.text(), "shared");
    }

    #[test]
    fn user_bindings_override_and_remove_default_commands_in_the_editor() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("hello");
        assert!(app.handle_keybinding_stroke("ctrl+a"));
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("hello"));
        app.test_editor_mut(0).set_selection(0, 0);
        app.keymap
            .reload(
                r#"[
            {"key":"ctrl+a", "command":"-editor.action.selectAll"},
            {"key":"ctrl+q", "command":"editor.action.selectAll", "when":"editorTextFocus"}
        ]"#,
                &graphite_i18n::Translator::english(),
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("ctrl+a"));
        assert!(app.test_editor(0).selection().is_none());
        assert!(app.handle_keybinding_stroke("ctrl+q"));
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("hello"));
        assert!(app.handle_keybinding_stroke("ctrl+k"));
        assert!(app.handle_keybinding_stroke("ctrl+\\"));
        assert_eq!(app.editor_groups.group_count(), 2);
    }

    #[test]
    fn navigation_bindings_extend_and_collapse_unicode_selections() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("aé🙂b");
        app.test_editor_mut(0).set_selections(vec![
            graphite_text::Selection { anchor: 1, head: 1 },
            graphite_text::Selection { anchor: 3, head: 3 },
        ]);
        assert!(app.handle_keybinding_stroke("shift+right"));
        assert_eq!(app.test_editor(0).selected_texts(), ["é", "b"]);
        assert!(app.handle_keybinding_stroke("left"));
        assert_eq!(
            app.test_editor(0).selections(),
            vec![
                graphite_text::Selection { anchor: 1, head: 1 },
                graphite_text::Selection { anchor: 3, head: 3 },
            ]
        );
        assert!(app.handle_keybinding_stroke("backspace"));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "éb");
        assert!(app.handle_keybinding_stroke("ctrl+z"));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "aé🙂b");
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
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "second");
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
                &graphite_i18n::Translator::english(),
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("enter"));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "");
        assert!(app.handle_keybinding_stroke("ctrl+j"));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "hello 🙂");
        assert!(app.snippet_query.is_none());
        app.test_editor_mut(0).undo();
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "");
    }

    #[test]
    fn find_commands_respect_context_remapping_and_invalid_regex() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("é one one");
        app.open_local_search(true);
        app.local_search.as_mut().unwrap().query = "one".into();
        assert!(app.handle_keybinding_stroke("enter"));
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("one"));
        app.keymap
            .reload(
                r#"[
          {"key":"alt+r","command":"-find.toggleRegex","when":"findWidgetVisible"},
          {"key":"ctrl+r","command":"find.toggleRegex","when":"findWidgetVisible"}
        ]"#,
                &graphite_i18n::Translator::english(),
            )
            .unwrap();
        assert!(app.handle_keybinding_stroke("alt+r"));
        assert!(!app.local_search.as_ref().unwrap().options.regex);
        assert!(app.handle_keybinding_stroke("ctrl+r"));
        app.local_search.as_mut().unwrap().query = "(".into();
        let revision = app.documents[app.document_at(0)].buffer.revision();
        assert!(app.handle_keybinding_stroke("ctrl+enter"));
        assert_eq!(
            app.documents[app.document_at(0)].buffer.revision(),
            revision
        );
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
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(b.as_path())
        );
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
                &graphite_i18n::Translator::english(),
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
        app.test_editor_mut(0).insert("one two");
        app.test_editor_mut(0).set_selection(0, 0);
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .diagnostics = vec![diagnostic(0, 3, "first"), diagnostic(4, 7, "second")];
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .schema_diagnostics = vec![diagnostic(0, 1, "schema")];
        assert_eq!(app.diagnostic_entries().len(), 3);

        assert!(app.handle_keybinding_stroke("f8"));
        assert_eq!(app.test_editor(0).cursor().char_idx, 4);
    }

    #[test]
    fn f8_cycles_diagnostics_that_share_the_same_range() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("problem");
        app.test_editor_mut(0).set_selection(0, 0);
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .diagnostics = vec![diagnostic(0, 7, "first"), diagnostic(0, 7, "second")];

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
        app.test_editor_mut(0).set_selection(1, 1);
        app.push_navigation_location();
        app.test_editor_mut(0).set_selection(3, 3);
        app.push_navigation_location();
        app.handle_keybinding_stroke("alt+left");
        assert_eq!(app.test_editor(0).cursor().char_idx, 1);
        app.handle_keybinding_stroke("alt+right");
        assert_eq!(app.test_editor(0).cursor().char_idx, 3);

        app.open_file(second.clone());
        app.handle_keybinding_stroke("ctrl+tab");
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(first.as_path())
        );
        app.handle_keybinding_stroke("ctrl+shift+tab");
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
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
        assert!(app.documents[app.active_document()].buffer.path().is_none());
        assert!(app.handle_keybinding_stroke("ctrl+shift+t"));
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(path.as_path())
        );
        assert_eq!(
            app.documents[app.active_document()].buffer.text(),
            "restore me"
        );
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
        app.documents.get_mut(app.document_at(0)).unwrap().language = Some(Language::Rust);
        assert!(app.snippet_scope_matches(&snippet));
        app.documents.get_mut(app.document_at(0)).unwrap().language = Some(Language::Python);
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
            app.documents[app.active_document()].buffer.text(),
            "aXb",
            "quick fix status: {}",
            app.status
        );
        app.editor_mut().undo();
        assert_eq!(
            app.documents[app.active_document()].buffer.text(),
            "a\u{1f642}b"
        );
        app.editor_mut().redo();
        assert_eq!(app.documents[app.active_document()].buffer.text(), "aXb");
    }

    #[test]
    fn quick_fix_keeps_previously_closed_encoded_documents_dirty_in_memory() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.rs");
        let second = dir.path().join("second.rs");
        std::fs::write(&first, "abc").unwrap();
        let original = [0xff, 0xfe, b'x', 0, b'y', 0, b'z', 0];
        std::fs::write(&second, original).unwrap();
        let edit = |value| serde_json::json!({"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}, "newText": value});
        let mut changes = serde_json::Map::new();
        changes.insert(
            crate::lsp::path_to_uri(&first).unwrap(),
            serde_json::json!([edit("A")]),
        );
        changes.insert(
            crate::lsp::path_to_uri(&second).unwrap(),
            serde_json::json!([edit("X")]),
        );
        let action = serde_json::from_value(
            serde_json::json!({"title": "Edit", "edit": {"changes": changes}}),
        )
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first);
        app.quick_fix_actions = vec![action];
        app.apply_quick_fix();
        assert_eq!(app.status, "Quick fix applied");
        assert_eq!(app.documents[app.active_document()].buffer.text(), "Abc");
        let (_, tab) = app
            .documents
            .iter_mut()
            .find(|(_, tab)| {
                tab.buffer
                    .path()
                    .is_some_and(|path| super::super::super::same_file_path(path, &second))
            })
            .unwrap();
        assert_eq!(tab.buffer.text(), "Xyz");
        assert!(tab.is_dirty());
        assert_eq!(std::fs::read(&second).unwrap(), original);
        tab.buffer.undo();
        assert_eq!(tab.buffer.text(), "xyz");
        assert!(!tab.is_dirty());
    }

    #[test]
    fn quick_fix_rejects_version_older_than_local_buffer() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("version.rs");
        std::fs::write(&path, "abc").unwrap();
        let uri = crate::lsp::path_to_uri(&path).unwrap();
        let action = serde_json::from_value(serde_json::json!({
            "title": "Stale",
            "edit": {"documentChanges": [{
                "textDocument": {"uri": uri, "version": 0},
                "edits": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}, "newText": "X"}]
            }]}
        })).unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        let document = app.active_document();
        app.documents[document].lsp_revision = Some(0);
        app.editor_mut().insert("!");
        app.quick_fix_actions = vec![action];
        app.apply_quick_fix();
        assert_eq!(app.documents[app.active_document()].buffer.text(), "!abc");
        assert!(app.status.contains("stale"), "{}", app.status);
    }

    #[test]
    fn quick_fix_rejects_all_documents_when_one_has_overlapping_edits() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.rs");
        let second = dir.path().join("second.rs");
        std::fs::write(&first, "abc").unwrap();
        std::fs::write(&second, "xyz").unwrap();
        let mut changes = serde_json::Map::new();
        let edit = |start, end, value| {
            serde_json::json!({
                "range": {"start": {"line": 0, "character": start}, "end": {"line": 0, "character": end}},
                "newText": value
            })
        };
        changes.insert(
            crate::lsp::path_to_uri(&first).unwrap(),
            serde_json::json!([edit(0, 1, "A")]),
        );
        changes.insert(
            crate::lsp::path_to_uri(&second).unwrap(),
            serde_json::json!([edit(0, 2, "X"), edit(1, 3, "Y")]),
        );
        let action = serde_json::from_value(
            serde_json::json!({"title": "Invalid", "edit": {"changes": changes}}),
        )
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first.clone());
        app.quick_fix_actions = vec![action];
        app.apply_quick_fix();
        assert_eq!(app.documents[app.active_document()].buffer.text(), "abc");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "xyz");
        assert!(app.status.contains("overlap"), "{}", app.status);
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
        app.documents
            .get_mut(app.active_document())
            .unwrap()
            .buffer
            .set_read_only(true);
        app.quick_fix_actions = vec![action];
        app.apply_quick_fix();
        assert_eq!(
            app.documents[app.active_document()].buffer.text(),
            "original"
        );
        assert!(app.status.contains("read-only"), "status: {}", app.status);
    }
}
