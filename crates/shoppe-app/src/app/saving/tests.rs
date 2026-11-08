use super::*;
use std::time::{Duration, Instant};
impl App {
    pub(super) fn set_save_test_text(&mut self, text: &str) {
        let len = self.editor().len_chars();
        self.editor_mut()
            .apply_transaction(&[shoppe_text::TextReplacement {
                range: 0..len,
                text: text.into(),
            }])
            .unwrap();
    }
    /// Headless tests drive the same event-loop reconciliation as production.
    pub(crate) fn wait_for_saves(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.saving.pending() {
            self.poll_extension_host(Instant::now());
            self.poll_saves(Instant::now());
            assert!(Instant::now() < deadline, "save did not complete");
            std::thread::yield_now();
        }
    }
}
pub(super) fn fixture() -> (crate::test_support::TempDir, App, PathBuf) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("save.txt");
    std::fs::write(&path, "before\n").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_file(path.clone());
    (dir, app, path)
}
#[test]
fn writing_snapshot_preserves_new_edits_and_old_saved_baseline() {
    let (_dir, mut app, path) = fixture();
    app.set_save_test_text("snapshot\n");
    let document = app.active_document();
    app.save_document(document);
    app.poll_saves(Instant::now()); // No participants: snapshot is now owned by worker.
    app.set_save_test_text("newer\n");
    let version = app.editor().revision();
    app.wait_for_saves();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "snapshot\n");
    assert_eq!(app.editor().text(), "newer\n");
    assert_eq!(app.editor().revision(), version);
    assert!(app.editor().is_dirty());
    app.editor_mut().undo();
    assert_eq!(app.editor().text(), "snapshot\n");
    assert!(!app.editor().is_dirty());
}
#[test]
fn trim_is_committed_only_after_success_and_is_undoable() {
    let (_dir, mut app, path) = fixture();
    app.trim_trailing_whitespace = true;
    app.set_save_test_text("after  \n");
    let version = app.editor().revision();
    app.save_document(app.active_document());
    assert_eq!(app.editor().text(), "after  \n");
    app.wait_for_saves();
    assert_eq!(app.editor().text(), "after\n");
    assert_eq!(app.editor().revision(), version + 1);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "after\n");
    assert!(!app.editor().is_dirty());
    app.editor_mut().undo();
    assert_eq!(app.editor().text(), "after  \n");
    assert!(app.editor().is_dirty());
}
#[test]
fn failure_preserves_text_history_and_reference() {
    let (_dir, mut app, path) = fixture();
    app.trim_trailing_whitespace = true;
    app.set_save_test_text("after  \n");
    let version = app.editor().revision();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    app.save_document(app.active_document());
    app.wait_for_saves();
    assert!(app.status.starts_with("Save failed:"));
    assert_eq!(app.editor().text(), "after  \n");
    assert_eq!(app.editor().revision(), version);
    app.editor_mut().undo();
    assert_eq!(app.editor().text(), "before\n");
    assert!(!app.editor().is_dirty());
}
#[test]
fn queued_requests_capture_latest_text_after_previous_completion() {
    let (_dir, mut app, path) = fixture();
    app.set_save_test_text("first");
    let document = app.active_document();
    app.save_document(document);
    app.poll_saves(Instant::now());
    app.set_save_test_text("second");
    app.save_document(document);
    app.save_document(document);
    assert!(app.documents.is_retained(document));
    app.wait_for_saves();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "second");
    assert!(!app.editor().is_dirty());
    assert!(!app.documents.is_retained(document));
}

pub(super) fn pause_writer(
    app: &mut App,
    document: DocumentId,
    after_claim: bool,
) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (ready, started) = mpsc::channel();
    let (resume, proceed) = mpsc::channel();
    app.saving
        .active
        .get_mut(&document)
        .unwrap()
        .snapshot
        .control = Some(Arc::new(writer::WriteControl {
        ready,
        proceed: std::sync::Mutex::new(proceed),
        after_claim,
    }));
    (started, resume)
}
fn save_command(app: &mut App) -> shoppe_core_api::CommandTicket {
    app.execute_command(shoppe_core_api::CommandRequest {
        id: "workbench.action.files.save".into(),
        arguments: serde_json::Value::Null,
        target: shoppe_core_api::CommandTarget::View(app.command_view_ref().id.0),
    })
}
#[test]
fn cancellation_before_commit_cleans_temporary_and_preserves_reference() {
    let (dir, mut app, path) = fixture();
    app.set_save_test_text("new");
    let ticket = save_command(&mut app);
    let document = app.active_document();
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    ticket.cancellation.cancel();
    app.poll_saves(Instant::now());
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "before\n");
    assert!(app.editor().is_dirty());
    assert!(!std::fs::read_dir(dir.path()).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".shoppe-")));
}
#[test]
fn cancellation_after_commit_claim_still_reconciles_the_saved_snapshot() {
    let (_dir, mut app, path) = fixture();
    app.set_save_test_text("new");
    let ticket = save_command(&mut app);
    let document = app.active_document();
    let (started, resume) = pause_writer(&mut app, document, true);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    ticket.cancellation.cancel();
    app.poll_saves(Instant::now());
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "new");
    assert!(!app.editor().is_dirty());
}
#[test]
fn cancelled_queued_consumer_does_not_cancel_another_consumer() {
    let (_dir, mut app, path) = fixture();
    app.set_save_test_text("first");
    let first = save_command(&mut app);
    let document = app.active_document();
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.set_save_test_text("second");
    let cancelled = save_command(&mut app);
    let remaining = save_command(&mut app);
    cancelled.cancellation.cancel();
    app.poll_saves(Instant::now());
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert_eq!(cancelled.try_result(), Some(Err(CommandError::Cancelled)));
    assert_eq!(first.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(remaining.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "second");
}
#[test]
fn typing_during_save_and_close_keeps_view_open_and_focus_unchanged() {
    let (_dir, mut app, path) = fixture();
    app.set_save_test_text("snapshot");
    let view = app.command_view_ref().id;
    let close = app.execute_command(shoppe_core_api::CommandRequest {
        id: "workbench.action.closeActiveEditor".into(),
        arguments: serde_json::Value::Null,
        target: shoppe_core_api::CommandTarget::View(view.0),
    });
    app.run_ui_command("shoppe.close.save");
    let document = app.active_document();
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.editor_mut().insert("newer");
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert!(matches!(
        close.try_result(),
        Some(Err(CommandError::Failed(_)))
    ));
    assert!(app.editor_groups.view(view).is_some());
    assert_eq!(app.command_view_ref().id, view);
    assert!(app.editor().is_dirty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "snapshot");
}
#[test]
fn changing_format_during_write_keeps_new_format_dirty() {
    let (_dir, mut app, path) = fixture();
    app.set_save_test_text("snapshot\n");
    let document = app.active_document();
    app.save_document(document);
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    app.documents[document].encoding = Encoding::Utf16Le;
    app.documents[document].line_ending = LineEnding::Crlf;
    resume.send(()).unwrap();
    app.wait_for_saves();
    assert_eq!(std::fs::read(path).unwrap(), b"snapshot\n");
    assert!(app.documents[document].is_dirty());
    app.save();
    assert!(!app.documents[document].is_dirty());
}

fn start_save_host(app: &mut App, scenario: &str) -> mpsc::Receiver<()> {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "shoppe-extension-host.exe"
        } else {
            "shoppe-extension-host"
        });
    let (tx, wakes) = mpsc::channel();
    app.extension_host.enabled = true;
    app.extension_host.supervisor = Some(
        shoppe_extension_host::Supervisor::spawn_with_args(
            &executable,
            &[scenario.into()],
            900,
            Instant::now(),
            Arc::new(move || {
                let _ = tx.send(());
            }),
        )
        .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_extension_host(Instant::now());
        if !app.extension_host.participants.is_empty() {
            break;
        }
        wakes
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
    }
    wakes
}
#[test]
fn real_host_participant_stages_text_and_commits_one_undo_step() {
    let (_dir, mut app, path) = fixture();
    let _wakes = start_save_host(&mut app, "--save-edit");
    app.set_save_test_text("body  \n");
    app.trim_trailing_whitespace = true;
    let revision = app.editor().revision();
    let ticket = save_command(&mut app);
    assert!(ticket.try_result().is_none());
    assert_eq!(app.editor().text(), "body  \n");
    app.wait_for_saves();
    assert_eq!(ticket.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(app.editor().text(), "// simulated save\nbody\n");
    assert_eq!(app.editor().revision(), revision + 1);
    assert_eq!(std::fs::read_to_string(path).unwrap(), app.editor().text());
    app.editor_mut().undo();
    assert_eq!(app.editor().text(), "body  \n");
}
#[test]
fn host_crash_stale_malformed_and_mutating_participants_do_not_prevent_save() {
    for scenario in [
        "--save-crash",
        "--save-stale",
        "--save-invalid",
        "--save-mutate",
    ] {
        let (_dir, mut app, path) = fixture();
        let _wakes = start_save_host(&mut app, scenario);
        app.set_save_test_text("body\n");
        let ticket = save_command(&mut app);
        app.wait_for_saves();
        assert_eq!(
            ticket.try_result(),
            Some(Ok(serde_json::Value::Null)),
            "{scenario}"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "body\n");
        assert_eq!(app.editor().text(), "body\n");
        assert!(app.status.contains("participant"), "{}", app.status);
        assert!(app.extension_host.save_replies.is_empty());
        if scenario == "--save-crash" {
            assert!(app.extension_host.participants.is_empty());
        }
    }
}
#[test]
fn participant_deadline_uses_controlled_clock_and_rejects_late_replies() {
    let (_dir, mut app, path) = fixture();
    let _wakes = start_save_host(&mut app, "--save-wait");
    app.set_save_test_text("body\n");
    let ticket = save_command(&mut app);
    let start = Instant::now();
    app.poll_saves(start);
    assert_eq!(app.extension_host.save_replies.len(), 1);
    app.poll_saves(start + Duration::from_secs(1));
    app.wait_for_saves();
    assert_eq!(ticket.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "body\n");
    assert!(app.status.contains("Timeout"));
    assert!(app.extension_host.save_replies.is_empty());
}
#[test]
fn save_service_reports_snapshot_outcome_without_implying_a_focused_view() {
    use shoppe_core_api::{
        CancellationToken, DocumentUri, SaveOutcome, SaveReason, SaveRequest, SaveService,
    };
    let (_dir, mut app, path) = fixture();
    app.editor_mut().insert("new");
    let document = app.active_document();
    let ticket = SaveService::save(
        &mut app,
        SaveRequest {
            uri: DocumentUri::from_path(&path).unwrap(),
            reason: SaveReason::Manual,
        },
        CancellationToken::default(),
    );
    app.wait_for_saves();
    let outcome: SaveOutcome =
        serde_json::from_value(ticket.try_result().unwrap().unwrap()).unwrap();
    assert_eq!(outcome.document, document);
    assert!(!outcome.dirty);
    assert!(outcome.warnings.is_empty());
}

#[test]
fn session_registration_duplicates_and_disposal_invalidate_pending_save_participant() {
    use shoppe_core_api::{protocol::RpcRequest, SaveParticipantMetadata};
    let (_dir, mut app, path) = fixture();
    let _wakes = start_save_host(&mut app, "--save-wait");
    let resource = *app.extension_host.participants.keys().next().unwrap();
    let session = app.extension_host.supervisor.as_ref().unwrap().session;
    let now = Instant::now();
    app.handle_host_request(
        session,
        100,
        RpcRequest::RegisterSaveParticipant(SaveParticipantMetadata {
            id: "shoppe.simulated.save".into(),
            languages: Vec::new(),
        }),
        now,
    );
    assert_eq!(app.extension_host.participants.len(), 1);
    app.set_save_test_text("body\n");
    let ticket = save_command(&mut app);
    app.poll_saves(now);
    assert_eq!(app.extension_host.save_replies.len(), 1);
    app.handle_host_request(session, 101, RpcRequest::Dispose { resource }, now);
    assert!(app.extension_host.participants.is_empty());
    app.wait_for_saves();
    assert_eq!(ticket.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "body\n");
    assert!(app.extension_host.save_replies.is_empty());
}
