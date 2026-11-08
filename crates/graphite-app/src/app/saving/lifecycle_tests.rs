use super::tests::{fixture, pause_writer};
use super::*;
use std::time::{Duration, Instant};
fn switch(app: &mut App, path: &std::path::Path, wait: bool) -> mpsc::Receiver<()> {
    let (acknowledge, receiver) = mpsc::channel();
    app.apply_cli_request(
        crate::cli::CliRequest {
            action: crate::cli::CliAction::Open {
                paths: vec![path.into()],
                location: None,
            },
            wait,
        },
        Some(acknowledge),
    );
    receiver
}
#[test]
fn workspace_save_preserves_tabs_and_focus_and_delays_cli_acknowledgement() {
    let (_dir, mut app, path) = fixture();
    let next = crate::test_support::TempDir::new();
    app.editor_mut().insert("new");
    let document = app.active_document();
    let view = app.command_view_ref().id;
    let acknowledge = switch(&mut app, next.path(), false);
    assert!(acknowledge.try_recv().is_err());
    app.run_ui_command("graphite.window.saveAndClose");
    assert!(acknowledge.try_recv().is_err());
    app.wait_for_saves();
    assert_eq!(app.tree.root(), next.path().canonicalize().unwrap());
    assert_eq!(app.command_view_ref().id, view);
    assert_eq!(app.active_document(), document);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "newbefore\n");
    assert!(acknowledge.try_recv().is_ok());
    assert!(!app.exit_requested);
}
#[test]
fn workspace_discard_reloads_saved_files_closes_dirty_untitled_and_keeps_file_view() {
    let (_dir, mut app, path) = fixture();
    let next = crate::test_support::TempDir::new();
    let document = app.active_document();
    let view = app.command_view_ref().id;
    app.editor_mut().insert("discard");
    let acknowledge = switch(&mut app, next.path(), false);
    app.run_ui_command("graphite.window.discardAndClose");
    app.wait_for_saves();
    assert_eq!(app.active_document(), document);
    assert_eq!(app.command_view_ref().id, view);
    assert_eq!(app.editor().text(), "before\n");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "before\n");
    assert_eq!(app.tree.root(), next.path().canonicalize().unwrap());
    assert!(acknowledge.try_recv().is_ok());
    let mut untitled = App::new_with_options(next.path().into(), None, true);
    untitled.editor_mut().insert("discard");
    let old_view = untitled.command_view_ref().id;
    let third = crate::test_support::TempDir::new();
    let acknowledge = switch(&mut untitled, third.path(), false);
    untitled.run_ui_command("graphite.window.discardAndClose");
    assert!(untitled.editor_groups.view(old_view).is_none());
    assert!(untitled.editor().text().is_empty());
    assert_eq!(untitled.tree.root(), third.path().canonicalize().unwrap());
    assert!(acknowledge.try_recv().is_ok());
}
#[test]
fn workspace_cancel_acknowledges_handling_and_preserves_everything() {
    let (_dir, mut app, _path) = fixture();
    let root = app.tree.root().to_path_buf();
    let next = crate::test_support::TempDir::new();
    app.editor_mut().insert("keep");
    let acknowledge = switch(&mut app, next.path(), true);
    app.run_ui_command("graphite.window.cancelClose");
    assert_eq!(app.tree.root(), root);
    assert_eq!(app.editor().text(), "keepbefore\n");
    assert!(acknowledge.try_recv().is_ok());
    assert!(!app.workspace_decision_pending());
}
#[test]
fn workspace_discard_read_failure_has_no_partial_buffer_effects() {
    let (dir, mut app, path) = fixture();
    app.editor_mut().insert("keep");
    let second = dir.path().join("second.txt");
    std::fs::write(&second, "second").unwrap();
    app.open_file(second.clone());
    app.editor_mut().insert("keep");
    let first = app.documents.id_for_path(&path).unwrap();
    let next = crate::test_support::TempDir::new();
    let root = app.tree.root().to_path_buf();
    let acknowledge = switch(&mut app, next.path(), false);
    std::fs::remove_file(second).unwrap();
    app.run_ui_command("graphite.window.discardAndClose");
    assert_eq!(app.tree.root(), root);
    assert_eq!(app.documents[first].buffer.text(), "keepbefore\n");
    assert_eq!(app.editor().text(), "keepsecond");
    assert!(app.documents.iter().all(|doc| doc.is_dirty()));
    assert!(acknowledge.try_recv().is_ok());
}
#[test]
fn workspace_save_wait_preserves_new_typing_and_cli_continuation_is_cancelled() {
    let (_dir, mut app, path) = fixture();
    let next = crate::test_support::TempDir::new();
    let root = app.tree.root().to_path_buf();
    app.set_save_test_text("snapshot");
    let acknowledge = switch(&mut app, next.path(), false);
    app.run_ui_command("graphite.window.saveAndClose");
    let document = app.active_document();
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.editor_mut().insert("newer");
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert_eq!(app.tree.root(), root);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "snapshot");
    assert!(app.editor().text().contains("newer"));
    assert!(app.editor().is_dirty());
    assert!(acknowledge.try_recv().is_ok());
}
#[test]
fn discard_waits_for_writer_and_preserves_edits_made_after_decision() {
    let (_dir, mut app, _path) = fixture();
    app.set_save_test_text("snapshot");
    let document = app.active_document();
    app.save_document(document);
    let (started, resume) = pause_writer(&mut app, document, true);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let quit = app.execute_command(graphite_core_api::CommandRequest {
        id: "workbench.action.quit".into(),
        target: graphite_core_api::CommandTarget::Global,
        arguments: serde_json::Value::Null,
    });
    app.run_ui_command("graphite.window.discardAndClose");
    assert!(!app.exit_requested);
    assert!(quit.try_result().is_none());
    app.editor_mut().insert("newer");
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert!(!app.exit_requested);
    assert!(app.editor().is_dirty());
    assert!(matches!(
        quit.try_result(),
        Some(Err(CommandError::Failed(_)))
    ));
}

#[test]
fn saving_before_close_waits_for_existing_trim_without_treating_it_as_new_typing() {
    let (_dir, mut app, _path) = fixture();
    app.trim_trailing_whitespace = true;
    app.set_save_test_text("snapshot  \n");
    let document = app.active_document();
    app.save_document(document);
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let quit = app.execute_command(graphite_core_api::CommandRequest {
        id: "workbench.action.quit".into(),
        target: graphite_core_api::CommandTarget::Global,
        arguments: serde_json::Value::Null,
    });
    app.run_ui_command("graphite.window.saveAndClose");
    assert!(!app.exit_requested);
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert!(app.exit_requested);
    assert_eq!(quit.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(app.editor().text(), "snapshot\n");
}

#[test]
fn close_detects_changes_to_previously_clean_documents_even_if_they_were_saved_again() {
    let (dir, mut app, _path) = fixture();
    app.set_save_test_text("first");
    let first = app.active_document();
    let second_path = dir.path().join("second.txt");
    std::fs::write(&second_path, "second").unwrap();
    app.open_file(second_path);
    let second = app.active_document();
    let quit = app.execute_command(graphite_core_api::CommandRequest {
        id: "workbench.action.quit".into(),
        target: graphite_core_api::CommandTarget::Global,
        arguments: serde_json::Value::Null,
    });
    app.run_ui_command("graphite.window.saveAndClose");
    let (started, resume) = pause_writer(&mut app, first, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.editor_mut().insert("new");
    app.save_document(second);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.saving.document_pending(second) {
        app.poll_saves(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(!app.documents[second].is_dirty());
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert!(!app.exit_requested);
    assert!(matches!(
        quit.try_result(),
        Some(Err(CommandError::Failed(_)))
    ));
    assert_eq!(app.editor().text(), "newsecond");
}
