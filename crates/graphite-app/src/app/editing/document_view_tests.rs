use super::editor_groups::SplitDirection;
use super::*;
use graphite_text::Selection;

#[test]
fn multicursor_transactions_noop_and_history_keep_peer_state_independent() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("abc\ndef\nghi\n");
    app.editor_mut().set_selection(10, 9);
    let peer = app.editor_groups.active_view().id;
    app.editor_groups.active_view_mut().scroll_line = 2;
    app.ime_preedit("日".into());
    app.split_editor_group(SplitDirection::Vertical);
    let origin = app.editor_groups.active_view().id;
    app.editor_mut().set_selections(vec![
        Selection { anchor: 0, head: 0 },
        Selection { anchor: 4, head: 4 },
    ]);
    let before = app.editor().selections();
    let version = app.editor().revision();
    app.ime_commit("🌎");
    assert_eq!(app.editor().revision(), version + 1);
    assert_eq!(app.editor().text(), "🌎abc\n🌎def\nghi\n");
    let after = app.editor().selections();
    assert_eq!(after.len(), 2);
    let view = app.editor_groups.view(peer).unwrap();
    assert_eq!(view.scroll_line, 2);
    assert_eq!(view.ime_preedit, "日");
    assert_eq!(
        view.selection_state.selections()[0],
        Selection {
            anchor: 12,
            head: 11
        }
    );
    app.focus_editor_group(0);
    app.run_ui_command("undo");
    assert_eq!(app.editor_groups.active_view().id, peer);
    assert_eq!(
        app.editor_groups
            .view(origin)
            .unwrap()
            .selection_state
            .selections(),
        before
    );
    let revision = app.editor().revision();
    app.editor_mut().apply_transaction(&[]).unwrap();
    assert_eq!(app.editor().revision(), revision);
    assert!(app.editor().can_redo());
    app.run_ui_command("redo");
    assert_eq!(app.editor_groups.active_view().id, peer);
    assert_eq!(
        app.editor_groups
            .view(origin)
            .unwrap()
            .selection_state
            .selections(),
        after
    );
    assert_eq!(app.editor_groups.active_view().scroll_line, 2);
}

#[test]
fn reopening_a_file_already_open_activates_its_existing_document() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("doc.txt");
    std::fs::write(&path, "abc").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_file(path.clone());
    app.close_active_tab();
    app.open_file(path);
    let id = app.active_document();
    app.editor_mut().insert("!");
    app.reopen_closed_tab();
    assert_eq!(app.active_document(), id);
    assert_eq!(app.documents.len(), 1);
    assert_eq!(app.editor().text(), "!abc");
    app.editor_mut().undo();
    assert_eq!(app.editor().text(), "abc");
}

#[test]
fn opening_a_retained_closed_document_prevents_collection_of_its_new_view() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("doc.txt");
    std::fs::write(&path, "abc").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_file(path.clone());
    let id = app.active_document();
    let lease = app.documents.lease(id).unwrap();
    app.close_active_tab();
    app.open_file(path);
    drop(lease);
    app.collect_closed_documents();
    assert_eq!(app.active_document(), id);
    assert!(app.documents.get(id).is_some());
    assert_eq!(app.editor().text(), "abc");
    assert!(app.closed_tabs.is_empty());
}

#[test]
fn search_completion_clipboard_and_line_commands_remap_peer_views() {
    struct Clipboard(String);
    impl super::clipboard::ClipboardAccess for Clipboard {
        fn get_text(&mut self) -> Result<String, ()> {
            Ok(self.0.clone())
        }
        fn set_text(&mut self, text: &str) -> Result<(), ()> {
            self.0 = text.into();
            Ok(())
        }
    }
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("a bc a def");
    app.editor_mut().set_selection(9, 9);
    let second = app.split_editor_group(SplitDirection::Vertical);
    app.editor_mut().set_selection(0, 0);
    let mut search = LocalSearch {
        query: "a".into(),
        replacement: "🌎🌎🌎".into(),
        ..Default::default()
    };
    app.local_replace_all(&mut search);
    app.focus_editor_group(0);
    assert_eq!(app.editor().cursor().char_idx, 13);
    app.focus_editor_group(second);
    app.run_ui_command("undo");
    app.editor_mut().set_selection(1, 1);
    app.completion_items = vec![lsp_types::CompletionItem {
        label: "alpha".into(),
        ..Default::default()
    }];
    app.accept_completion();
    app.focus_editor_group(0);
    assert_eq!(app.editor().cursor().char_idx, 13);
    app.focus_editor_group(second);
    app.run_ui_command("undo");
    app.editor_mut().set_selection(0, 1);
    let mut clipboard = Clipboard(String::new());
    assert!(super::clipboard::copy_or_cut(
        &mut app.editor_mut(),
        &mut clipboard,
        true
    ));
    app.focus_editor_group(0);
    assert_eq!(app.editor().cursor().char_idx, 8);
    app.focus_editor_group(second);
    assert!(super::clipboard::paste(
        &mut app.editor_mut(),
        &mut clipboard
    ));
    app.run_ui_command("editor.action.copyLinesDownAction");
    app.run_ui_command("editor.action.deleteLines");
    app.focus_editor_group(0);
    assert_eq!(app.editor().text(), "a bc a def");
    assert_eq!(app.editor().cursor().char_idx, 9);
}

#[test]
fn undo_restores_origin_selections_without_changing_focus_or_scroll() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("abc\ndef\nghi\n");
    app.editor_mut().set_selection(8, 8);
    app.editor_groups.active_view_mut().scroll_line = 2;
    let second = app.split_editor_group(SplitDirection::Vertical);
    app.editor_groups.active_view_mut().scroll_line = 1;
    app.editor_mut().set_selection(3, 1);
    app.editor_mut().insert("🌎");
    let after = app.editor().selections();
    app.focus_editor_group(0);
    let before_undo = app.editor().selections();
    app.run_ui_command("undo");
    assert_eq!(app.editor_groups.focused(), 0);
    assert_eq!(app.editor_groups.active_view().scroll_line, 2);
    assert_ne!(app.editor().selections(), before_undo);
    app.focus_editor_group(second);
    assert_eq!(
        app.editor().selection(),
        Some(Selection { anchor: 3, head: 1 })
    );
    assert_eq!(app.editor_groups.active_view().scroll_line, 1);
    app.focus_editor_group(0);
    app.run_ui_command("redo");
    assert_eq!(app.editor_groups.focused(), 0);
    app.focus_editor_group(second);
    assert_eq!(app.editor().selections(), after);
}

#[test]
fn closed_origin_is_not_resurrected_by_undo_and_reopened_view_has_new_identity() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("abcdef");
    let document = app.active_document();
    app.editor_mut().set_selection(5, 5);
    app.split_editor_group(SplitDirection::Vertical);
    app.editor_mut().set_selection(1, 1);
    let origin = app.editor_groups.active_view().id;
    app.editor_mut().insert("🌎");
    app.close_active_tab();
    app.run_ui_command("undo");
    assert_eq!(app.editor_groups.group_count(), 1);
    assert_eq!(app.editor().cursor().char_idx, 5);
    assert!(!app.editor_groups.view_ids().contains(&origin));
    app.close_active_tab();
    app.reopen_closed_tab();
    assert_eq!(app.active_document(), document);
    assert_ne!(app.editor_groups.active_view().id, origin);
    assert_eq!(app.editor().cursor().char_idx, 5);
    app.run_ui_command("redo");
    assert_eq!(app.editor().text(), "a🌎bcdef");
    assert_eq!(app.editor().cursor().char_idx, 6);
}

#[test]
fn ime_composition_and_selection_navigation_belong_to_each_view() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("abc abc");
    app.editor_mut().set_selection(1, 1);
    app.ime_preedit("日".into());
    app.editor_groups
        .active_view_mut()
        .smart_selection_history
        .push(Selection { anchor: 0, head: 3 });
    let second = app.split_editor_group(SplitDirection::Vertical);
    assert!(app.editor_groups.active_view().ime_preedit.is_empty());
    assert!(app
        .editor_groups
        .active_view()
        .smart_selection_history
        .is_empty());
    app.editor_mut().set_selections(vec![
        Selection { anchor: 0, head: 0 },
        Selection { anchor: 4, head: 4 },
    ]);
    app.ime_preedit("本".into());
    app.ime_commit("本");
    assert_eq!(app.editor().text(), "本abc 本abc");
    app.focus_editor_group(0);
    assert_eq!(app.editor_groups.active_view().ime_preedit, "日");
    assert_eq!(
        app.editor_groups
            .active_view()
            .smart_selection_history
            .len(),
        1
    );
    assert_eq!(app.editor().cursor().char_idx, 2);
    app.focus_editor_group(second);
    assert!(!app.editor_groups.active_view().ime_composing);
    assert_eq!(app.editor().selections().len(), 2);
}

#[test]
fn session_maps_each_transaction_and_indentation_for_peer_views() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("    abc\n    def");
    app.editor_mut().set_selection(14, 14);
    app.split_editor_group(SplitDirection::Vertical);
    app.editor_mut().set_selection(0, 0);
    {
        let mut editor = app.editor_mut();
        editor.insert("🌎");
        editor.insert("é");
    }
    app.focus_editor_group(0);
    assert_eq!(app.editor().cursor().char_idx, 16);
    app.focus_editor_group(1);
    let id = app.active_document();
    app.documents[id].indentation = graphite_text::Indentation {
        insert_spaces: true,
        tab_size: 4,
    };
    app.run_ui_command("editor.action.toggleIndentation");
    app.focus_editor_group(0);
    assert!(app.editor().cursor().char_idx <= app.editor().len_chars());
    assert_eq!(app.editor().cursor_line_col(), (1, 3));
}

#[test]
fn save_metadata_controls_dirty_and_reload_preserves_monotonic_version() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("doc.txt");
    std::fs::write(&path, "abc").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_file(path.clone());
    let id = app.active_document();
    app.documents[id].encoding = file_format::Encoding::Utf8Bom;
    assert!(app.documents[id].is_dirty());
    assert!(app.active_view_needs_confirmation());
    app.save();
    assert!(!app.documents[id].is_dirty());
    assert!(std::fs::read(&path)
        .unwrap()
        .starts_with(&[0xef, 0xbb, 0xbf]));
    app.editor_mut().insert("!");
    let revision = app.editor().revision();
    app.reload_after_branch_checkout(vec![(
        path,
        Ok(file_format::DecodedFile {
            text: "xyz".into(),
            encoding: file_format::Encoding::Utf8Bom,
            line_ending: file_format::LineEnding::Lf,
        }),
    )]);
    assert_eq!(app.editor().revision(), revision + 1);
    assert!(!app.documents[id].is_dirty());
    app.run_ui_command("undo");
    assert_eq!(app.editor().text(), "xyz");
}

#[test]
fn document_store_identity_survives_data_replacement_and_retention_release() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let id = app.active_document();
    let lease = app.documents.lease(id).unwrap();
    assert!(app.documents.is_retained(id));
    app.close_active_tab();
    assert!(app.documents.get(id).is_some());
    assert_eq!(app.editor_groups.view_count(id), 0);
    drop(lease);
    app.collect_closed_documents();
    assert!(app.documents.get(id).is_none());
    app.reopen_closed_tab();
    assert_eq!(app.active_document(), id);
    let other = app.documents.push(Document {
        buffer: Buffer::new(),
        language: None,
        highlights: None,
        sticky_headers_cache: None,
        highlighter: IncrementalHighlighter::new(),
        diagnostics: Vec::new(),
        inlay_hints: Vec::new(),
        inlay_revision: None,
        provided_folds: None,
        schema_diagnostics: Vec::new(),
        schema_revision: None,
        lsp_revision: None,
        indentation: app.documents[id].indentation,
        encoding: file_format::Encoding::Utf8,
        line_ending: file_format::LineEnding::Lf,
        notebook: None,
        saved_format: (file_format::Encoding::Utf8, file_format::LineEnding::Lf),
    });
    let replacement = app.documents.remove(other).document;
    *app.documents.get_mut(id).unwrap() = replacement;
    assert_eq!(app.documents.resolve(id), Some(id));
    assert_eq!(app.documents.resolve(other), None);
}
