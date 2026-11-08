//! Simulated transport tests; no external language server or network is used.
use super::*;
use crate::app::command_execution::{CommandError, CommandRequest};
use crate::app::command_registry::CommandTarget;
use std::sync::Arc;

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
            instance: 0,
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

fn host_executable() -> PathBuf {
    let current = std::env::current_exe().unwrap();
    current
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "graphite-extension-host.exe"
        } else {
            "graphite-extension-host"
        })
}
fn pump_host(app: &mut crate::App, wakes: &Receiver<()>, done: impl Fn(&crate::App) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_extension_host(std::time::Instant::now());
        if done(app) {
            return;
        }
        wakes
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .expect("host did not wake editor");
    }
}
#[test]
fn real_host_crash_cleans_resources_but_editor_and_lsp_continue_then_restart() {
    let (_dir, mut app, messages, _path) = fixture();
    let (wake, wakes) = mpsc::channel();
    app.extension_host.enabled = true;
    app.extension_host.executable = Some(host_executable());
    app.extension_host.wake = Some(Arc::new(move || {
        let _ = wake.send(());
    }));
    app.restart_extension_host().unwrap();
    pump_host(&mut app, &wakes, |app| {
        app.extension_host.commands.len() == 4 && !app.extension_host.subscriptions.is_empty()
    });
    let session = app.extension_host.supervisor.as_ref().unwrap().session;
    let remote = |id: &str| CommandRequest {
        id: format!("shoppe.simulated.{id}"),
        arguments: Value::Null,
        target: CommandTarget::Global,
    };
    let pending = app.execute_command(remote("wait"));
    let lsp = app.execute_command(request(&app, "editor.action.triggerSuggest"));
    let ui = match messages.recv_timeout(Duration::from_secs(1)).unwrap() {
        WorkerMessage::Command(LspCommand::Complete { request_id, .. }) => request_id,
        other => panic!("unexpected {other:?}"),
    };
    let crash = app.execute_command(remote("crash"));
    pump_host(&mut app, &wakes, |app| {
        app.extension_host.supervisor.as_ref().unwrap().state
            == graphite_extension_host::HostState::Stopped
    });
    let outcome = pending.try_result();
    assert!(
        matches!(outcome, Some(Err(CommandError::HostStopped))),
        "{outcome:?}"
    );
    assert!(crash.try_result().is_some());
    assert!(app.extension_host.commands.is_empty());
    assert!(app.extension_host.subscriptions.is_empty());
    assert!(lsp.try_result().is_none());
    respond(&mut app, ui, "completion", json!([{"label":"still alive"}]));
    assert!(matches!(lsp.try_result(), Some(Ok(_))));
    app.run_ui_command("type"); // Invalid arguments must still be rejected locally.
    app.run_ui_command_args("type", json!({"text":"X"}));
    assert_eq!(app.editor().text(), "Xabc");
    app.run_ui_command("shoppe.extensions.restartHost");
    pump_host(&mut app, &wakes, |app| {
        app.extension_host.commands.len() == 4
    });
    assert_ne!(
        app.extension_host.supervisor.as_ref().unwrap().session,
        session
    );
    let echo = app.execute_command(CommandRequest {
        id: "shoppe.simulated.echo".into(),
        arguments: json!({"ok":true}),
        target: CommandTarget::Global,
    });
    pump_host(&mut app, &wakes, |app| {
        !app.command_runtime.pending.contains_key(&echo.id)
    });
    assert_eq!(echo.try_result(), Some(Ok(json!({"ok":true}))));
}
#[test]
fn stale_lsp_instance_cannot_apply_edit_or_stop_current_requests() {
    let (_dir, mut app, messages, path) = fixture();
    app.lsp_manager.servers.get_mut("rs").unwrap().instance = 9;
    let ticket = app.execute_command(request(&app, "editor.action.triggerSuggest"));
    let _ = messages.recv_timeout(Duration::from_secs(1)).unwrap();
    let (reply, replies) = mpsc::channel();
    let active = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let edit=serde_json::from_value(json!({"changes":{path_to_uri(&path).unwrap():[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"newText":"Q"}]}})).unwrap();
    let mut queue = app.lsp_queue.lock().unwrap();
    queue.push(LspUiEvent::Instance {
        extension: "rs".into(),
        instance: 8,
        event: Box::new(LspUiEvent::ApplyEdit {
            id: RequestId::from(5),
            edit,
            reply,
            active,
        }),
    });
    queue.push(LspUiEvent::Instance {
        extension: "rs".into(),
        instance: 8,
        event: Box::new(LspUiEvent::ServerStopped {
            extension: "rs".into(),
        }),
    });
    drop(queue);
    app.process_lsp_events();
    assert_eq!(app.editor().text(), "abc");
    assert!(replies.try_recv().is_err());
    assert!(ticket.try_result().is_none());
    assert!(app.lsp_manager.current_instance("rs", 9));
    app.cancel_command(ticket.id);
}

fn attach_host(app: &mut crate::App) -> Receiver<()> {
    let (tx, rx) = mpsc::channel();
    app.extension_host.enabled = true;
    app.extension_host.executable = Some(host_executable());
    app.extension_host.wake = Some(Arc::new(move || {
        let _ = tx.send(());
    }));
    app.restart_extension_host().unwrap();
    pump_host(app, &rx, |app| {
        app.extension_host.commands.len() == 4 && !app.extension_host.subscriptions.is_empty()
    });
    rx
}
#[test]
fn nested_host_service_edits_keep_origin_view_and_typed_command_errors() {
    use graphite_core_api::{
        DocumentEdit, DocumentUri, Position as ApiPosition, ServiceRequest, TextEdit,
    };
    let (_dir, mut app, _messages, path) = fixture();
    app.editor_mut().set_selection(1, 1);
    let first = app.command_view_ref().id;
    app.run_ui_command("workbench.action.splitEditorRight");
    app.editor_mut().set_selection(3, 3);
    let focused = app.command_view_ref().id;
    let wakes = attach_host(&mut app);
    let service = ServiceRequest::ApplyEdits {
        edits: vec![DocumentEdit {
            uri: DocumentUri::from_path(&path).unwrap(),
            version: Some(0),
            edits: vec![TextEdit {
                start: ApiPosition {
                    line: 0,
                    character: 1,
                },
                end: ApiPosition {
                    line: 0,
                    character: 1,
                },
                text: "X".into(),
            }],
        }],
    };
    let ticket = app.execute_command(CommandRequest {
        id: "shoppe.simulated.service".into(),
        arguments: serde_json::to_value(service).unwrap(),
        target: CommandTarget::View(first.0),
    });
    pump_host(&mut app, &wakes, |app| {
        !app.command_runtime.pending.contains_key(&ticket.id)
    });
    assert_eq!(ticket.try_result(), Some(Ok(Value::Null)));
    assert_eq!(app.command_view_ref().id, focused);
    assert_eq!(app.editor().text(), "aXbc");
    assert_eq!(app.editor().cursor().char_idx, 4);
    app.run_ui_command("undo");
    assert_eq!(app.command_view_ref().id, focused);
    assert_eq!(
        app.editor_groups
            .view(first)
            .unwrap()
            .selection_state
            .cursor()
            .char_idx,
        1
    );
    let ticket = app.execute_command(CommandRequest {
        id: "shoppe.simulated.service".into(),
        arguments: serde_json::to_value(ServiceRequest::Execute(CommandRequest {
            id: "does.not.exist".into(),
            arguments: Value::Null,
            target: CommandTarget::Global,
        }))
        .unwrap(),
        target: CommandTarget::Global,
    });
    pump_host(&mut app, &wakes, |app| {
        !app.command_runtime.pending.contains_key(&ticket.id)
    });
    assert!(matches!(
        ticket.try_result(),
        Some(Err(CommandError::UnknownCommand(_)))
    ));
}
#[test]
fn host_command_awaiting_user_confirmation_has_no_rpc_timeout() {
    use graphite_core_api::ServiceRequest;
    let (_dir, mut app, _messages, _path) = fixture();
    app.editor_mut().insert("dirty");
    let view = app.command_view_ref().id;
    let wakes = attach_host(&mut app);
    let service = ServiceRequest::Execute(CommandRequest {
        id: "workbench.action.closeActiveEditor".into(),
        arguments: Value::Null,
        target: CommandTarget::View(view.0),
    });
    let ticket = app.execute_command(CommandRequest {
        id: "shoppe.simulated.service".into(),
        arguments: serde_json::to_value(service).unwrap(),
        target: CommandTarget::View(view.0),
    });
    pump_host(&mut app, &wakes, |app| {
        app.close_confirm
            && app
                .extension_host
                .supervisor
                .as_ref()
                .unwrap()
                .pending
                .get(*app.extension_host.outbound.keys().next().unwrap())
                .is_some_and(|state| state.deadline.is_none())
    });
    app.poll_extension_host(std::time::Instant::now() + Duration::from_secs(31));
    assert!(ticket.try_result().is_none());
    assert!(app.close_confirm);
    app.run_ui_command("shoppe.close.cancel");
    pump_host(&mut app, &wakes, |app| {
        !app.command_runtime.pending.contains_key(&ticket.id)
    });
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    assert_eq!(app.editor().text(), "dirtyabc");
}

#[test]
fn host_resource_disposal_old_sessions_and_closed_targets_are_inert() {
    use graphite_core_api::{protocol::RpcRequest, ServiceRequest};
    let (_dir, mut app, _messages, _path) = fixture();
    let wakes = attach_host(&mut app);
    let session = app.extension_host.supervisor.as_ref().unwrap().session;
    let echo = app.extension_host.commands["shoppe.simulated.echo"].resource;
    let id = app.extension_host.last_request + 1;
    app.handle_host_request(
        session,
        id,
        RpcRequest::Dispose { resource: echo },
        std::time::Instant::now(),
    );
    assert!(!app
        .extension_host
        .commands
        .contains_key("shoppe.simulated.echo"));
    let text = app.editor().text();
    app.handle_host_request(
        session - 1,
        id + 1,
        RpcRequest::Service {
            request: ServiceRequest::Message {
                text: "stale session".into(),
            },
            parent: None,
        },
        std::time::Instant::now(),
    );
    assert_ne!(app.status, "stale session");
    assert_eq!(app.editor().text(), text);
    let view = app.command_view_ref().id;
    app.run_ui_command("workbench.action.splitEditorRight");
    let focused = app.command_view_ref().id;
    let wait = app.execute_command(CommandRequest {
        id: "shoppe.simulated.wait".into(),
        arguments: Value::Null,
        target: CommandTarget::View(view.0),
    });
    let close = app.execute_command(CommandRequest {
        id: "workbench.action.closeActiveEditor".into(),
        arguments: Value::Null,
        target: CommandTarget::View(view.0),
    });
    assert_eq!(close.try_result(), Some(Ok(Value::Null)));
    assert_eq!(wait.try_result(), Some(Err(CommandError::TargetClosed)));
    assert_eq!(app.command_view_ref().id, focused);
    pump_host(&mut app, &wakes, |app| {
        app.extension_host.outbound.is_empty()
    });
}

#[test]
fn replacing_server_instance_finishes_old_tickets_without_cancelling_the_replacement() {
    let (_dir, mut app, messages, _path) = fixture();
    let ticket = app.execute_command(request(&app, "editor.action.triggerSuggest"));
    let _ = messages.recv_timeout(Duration::from_secs(1)).unwrap();
    app.lsp_manager.servers.get_mut("rs").unwrap().instance = 3;
    app.process_lsp_events();
    assert!(matches!(
        ticket.try_result(),
        Some(Err(CommandError::Failed(_)))
    ));
    assert!(app.command_runtime.lsp_jobs.is_empty());
    assert!(messages.try_recv().is_err());
    assert!(app.lsp_manager.current_instance("rs", 3));
}
