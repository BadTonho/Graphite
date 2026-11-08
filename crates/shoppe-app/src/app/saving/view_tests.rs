use super::tests::{fixture, pause_writer};
use std::time::{Duration, Instant};
#[test]
fn trim_remaps_independent_views_and_undo_origin_without_moving_focus() {
    let (_dir, mut app, _path) = fixture();
    app.set_save_test_text("one  \ntwo  \nthree\n");
    let origin = app.command_view_ref().id;
    app.editor_mut().set_selection(6, 9);
    app.trim_trailing_whitespace = true;
    let right = app.split_editor_group(super::super::editor_groups::SplitDirection::Vertical);
    let focused = app.command_view_ref().id;
    app.editor_mut().set_selection(15, 15);
    app.editor_groups.active_view_mut().scroll_line = 1;
    let ticket = app.execute_command(shoppe_core_api::CommandRequest {
        id: "workbench.action.files.save".into(),
        arguments: serde_json::Value::Null,
        target: shoppe_core_api::CommandTarget::View(origin.0),
    });
    app.wait_for_saves();
    assert_eq!(ticket.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(app.command_view_ref().id, focused);
    assert_eq!(app.editor().cursor().char_idx, 11);
    assert_eq!(app.editor_groups.active_view_mut().scroll_line, 1);
    assert_eq!(
        app.editor_groups
            .view(origin)
            .unwrap()
            .selection_state
            .selections()[0],
        shoppe_text::Selection { anchor: 4, head: 7 }
    );
    app.editor_mut().undo();
    assert_eq!(app.command_view_ref().id, focused);
    assert_eq!(
        app.editor_groups
            .view(origin)
            .unwrap()
            .selection_state
            .selections()[0],
        shoppe_text::Selection { anchor: 6, head: 9 }
    );
    assert!(app.focus_editor_group(right));
    assert_eq!(app.editor().text(), "one  \ntwo  \nthree\n");
}
#[test]
fn unchanged_save_keeps_revision_and_redo_history() {
    let (_dir, mut app, _path) = fixture();
    app.editor_mut().insert("new");
    app.editor_mut().undo();
    let revision = app.editor().revision();
    assert!(app.editor().can_redo());
    app.save();
    assert_eq!(app.editor().revision(), revision);
    assert!(app.editor().can_redo());
    assert!(!app.editor().is_dirty());
    app.editor_mut().redo();
    assert_eq!(app.editor().text(), "newbefore\n");
}
#[test]
fn separate_documents_write_concurrently_without_waiting_for_other_document() {
    let (dir, mut app, _path) = fixture();
    let first = app.active_document();
    app.editor_mut().insert("first");
    app.save_document(first);
    let (started, resume) = pause_writer(&mut app, first, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let path = dir.path().join("second.txt");
    std::fs::write(&path, "second").unwrap();
    app.open_file(path.clone());
    app.editor_mut().insert("new");
    let second = app.active_document();
    app.save_document(second);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.saving.document_pending(second) {
        app.poll_saves(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(app.saving.document_pending(first));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "newsecond");
    resume.send(()).unwrap();
    app.wait_for_saves();
}
#[test]
fn filesystem_readonly_failure_preserves_format_history_and_content() {
    let (_dir, mut app, path) = fixture();
    app.trim_trailing_whitespace = true;
    app.set_save_test_text("new  \n");
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    app.save_document(app.active_document());
    app.wait_for_saves();
    std::fs::set_permissions(&path, original).unwrap();
    assert!(app.status.starts_with("Save failed:"));
    assert_eq!(app.editor().text(), "new  \n");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "before\n");
    app.editor_mut().undo();
    assert!(!app.editor().is_dirty());
}
