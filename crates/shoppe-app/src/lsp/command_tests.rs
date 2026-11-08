//! Simulated transport tests; no external language server or network is used.
use super::*;
use crate::app::command_execution::{CommandError, CommandRequest};
use crate::app::command_registry::CommandTarget;

fn fixture() -> (
    crate::test_support::TempDir,
    crate::App,
    Receiver<WorkerMessage>,
    PathBuf,
) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("test.rs");
    std::fs::write(&path, "abc").unwrap();
    let mut app = crate::App::new_with_options(dir.path().into(), None, true);
    app.open_file(path.clone());
    let config = ServerConfig {
        extension: "rs".into(),
        language_id: "rust".into(),
        command: "simulated".into(),
        args: Vec::new(),
    };
    app.lsp_manager = LspManager::from_config(vec![config]);
    let (sender, receiver) = mpsc::channel();
    app.lsp_manager.servers.insert(
        "rs".into(),
        ServerHandle {
            sender,
            worker: None,
        },
    );
    app.lsp_manager
        .documents
        .insert(path.clone(), path_to_uri(&path).unwrap());
    let document = app.active_document();
    app.documents[document].lsp_revision = Some(0);
    (dir, app, receiver, path)
}
fn request(app: &crate::App, id: &str) -> CommandRequest {
    CommandRequest {
        id: id.into(),
        arguments: Value::Null,
        target: CommandTarget::View(app.command_view_ref().id.0),
    }
}
fn respond(app: &mut crate::App, ui_id: u64, kind: &'static str, value: Value) {
    let (sender, receiver) = mpsc::channel();
    let mut pending = HashMap::from([(9, (ui_id, kind))]);
    dispatch_server_message(
        Message::Response(lsp_server::Response::new_ok(RequestId::from(9), value)),
        &mut pending,
        &sender,
    );
    assert!(pending.is_empty());
    app.lsp_queue
        .lock()
        .unwrap()
        .push(receiver.recv_timeout(Duration::from_secs(1)).unwrap());
    app.process_lsp_events();
}

#[test]
fn completion_ticket_targets_an_inactive_view_and_returns_serializable_result() {
    let (_dir, mut app, messages, _path) = fixture();
    let origin = app.command_view_ref().id;
    app.run_ui_command("workbench.action.splitEditorRight");
    let focused = app.command_view_ref().id;
    let ticket = app.execute_command(CommandRequest {
        id: "editor.action.triggerSuggest".into(),
        arguments: Value::Null,
        target: CommandTarget::View(origin.0),
    });
    assert!(ticket.try_result().is_none());
    let request_id = match messages.recv_timeout(Duration::from_secs(1)).unwrap() {
        WorkerMessage::Command(LspCommand::Complete { request_id, .. }) => request_id,
        other => panic!("unexpected request: {other:?}"),
    };
    respond(
        &mut app,
        request_id,
        "completion",
        json!([{"label":"alpha"}]),
    );
    assert_eq!(ticket.try_result().unwrap().unwrap()[0]["label"], "alpha");
    assert_eq!(app.command_view_ref().id, focused);
    let widget = app
        .context_keys
        .widget_id(crate::app::context_keys::WidgetKind::Suggest)
        .unwrap();
    assert_eq!(
        app.context_keys.widget(widget).unwrap().parent,
        Some(origin)
    );
}

#[test]
fn cancelled_lsp_command_sends_protocol_cancel_and_ignores_queued_response() {
    let (_dir, mut app, messages, _path) = fixture();
    let ticket = app.execute_command(request(&app, "editor.action.triggerSuggest"));
    let request_id = match messages.recv().unwrap() {
        WorkerMessage::Command(LspCommand::Complete { request_id, .. }) => request_id,
        other => panic!("unexpected request: {other:?}"),
    };
    app.cancel_command(ticket.id);
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    assert!(
        matches!(messages.recv().unwrap(), WorkerMessage::Command(LspCommand::Cancel { request_id: id }) if id == request_id)
    );
    app.lsp_queue.lock().unwrap().push(LspUiEvent::Completion {
        request_id,
        items: vec![CompletionItem {
            label: "late".into(),
            ..Default::default()
        }],
    });
    app.process_lsp_events();
    assert!(app.completion_items.is_empty());
}

#[test]
fn malformed_response_or_server_shutdown_finishes_pending_command_with_failure() {
    for shutdown in [false, true] {
        let (_dir, mut app, messages, _path) = fixture();
        let ticket = app.execute_command(request(&app, "editor.action.triggerSuggest"));
        let request_id = match messages.recv().unwrap() {
            WorkerMessage::Command(LspCommand::Complete { request_id, .. }) => request_id,
            other => panic!("unexpected request: {other:?}"),
        };
        if shutdown {
            app.lsp_queue
                .lock()
                .unwrap()
                .push(LspUiEvent::ServerStopped {
                    extension: "rs".into(),
                });
            app.process_lsp_events();
        } else {
            respond(
                &mut app,
                request_id,
                "completion",
                json!({"unexpected":true}),
            );
        }
        assert!(matches!(
            ticket.try_result(),
            Some(Err(CommandError::Failed(_)))
        ));
        assert!(app.completion_items.is_empty());
        assert!(app.command_runtime.pending.is_empty());
    }
}

#[test]
fn server_quick_fix_stays_on_execute_command_and_keeps_ticket_after_popup_closes() {
    let (_dir, mut app, messages, _path) = fixture();
    app.quick_fix_visible = true;
    app.quick_fix_actions = vec![lsp_types::CodeActionOrCommand::Command(
        lsp_types::Command {
            title: "Server fix".into(),
            command: "server.only.fix".into(),
            arguments: Some(vec![json!(42)]),
        },
    )];
    app.refresh_command_context();
    let widget = app
        .context_keys
        .widget_id(crate::app::context_keys::WidgetKind::QuickFix)
        .unwrap();
    let ticket = app.execute_command(CommandRequest {
        id: "quickFix.apply".into(),
        arguments: Value::Null,
        target: CommandTarget::Widget(widget),
    });
    assert!(!app.quick_fix_visible);
    assert!(ticket.try_result().is_none());
    let request_id = match messages.recv_timeout(Duration::from_secs(1)).unwrap() {
        WorkerMessage::Command(LspCommand::ExecuteCommand {
            request_id,
            command,
            arguments,
        }) => {
            assert_eq!(command, "server.only.fix");
            assert_eq!(arguments, vec![json!(42)]);
            request_id
        }
        other => panic!("unexpected request: {other:?}"),
    };
    assert_ne!(request_id, 0);
    respond(
        &mut app,
        request_id,
        "executeCommand",
        json!({"fixed":true}),
    );
    assert_eq!(ticket.try_result(), Some(Ok(json!({"fixed":true}))));
    let unknown = app.execute_command(request(&app, "server.only.fix"));
    assert!(matches!(
        unknown.try_result(),
        Some(Err(CommandError::UnknownCommand(_)))
    ));
    assert!(messages.try_recv().is_err());
}

#[test]
fn disconnected_provider_fails_the_command_instead_of_leaking_a_ticket() {
    let (_dir, mut app, messages, _path) = fixture();
    drop(messages);
    let ticket = app.execute_command(request(&app, "editor.action.triggerSuggest"));
    assert!(matches!(
        ticket.try_result(),
        Some(Err(CommandError::Failed(_)))
    ));
    assert!(app.command_runtime.pending.is_empty());
}
