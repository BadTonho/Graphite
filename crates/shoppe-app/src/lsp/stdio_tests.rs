//! Exercises the production worker, JSON framing and child-process lifecycle.
use super::*;
use std::time::Instant;

struct Server {
    commands: Sender<WorkerMessage>,
    events: Receiver<LspUiEvent>,
    worker: Option<thread::JoinHandle<()>>,
    _dir: crate::test_support::TempDir,
}
impl Server {
    fn new() -> Self {
        let dir = crate::test_support::TempDir::new();
        let executable = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("shoppe-test-lsp{}", std::env::consts::EXE_SUFFIX));
        assert!(
            executable.is_file(),
            "build shoppe-test-lsp before targeted tests"
        );
        let config = ServerConfig {
            extension: "rs".into(),
            language_id: "rust".into(),
            command: executable.to_str().unwrap().into(),
            args: vec![],
        };
        let (commands, receiver) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let root = dir.path().to_owned();
        let worker = Some(thread::spawn(move || {
            run_server(config, root, receiver, sender)
        }));
        let server = Self {
            commands,
            events,
            worker,
            _dir: dir,
        };
        server.until(|event| matches!(event, LspUiEvent::Status(s) if s.ends_with("server ready")));
        server
    }
    fn send(&self, command: LspCommand) {
        self.commands.send(WorkerMessage::Command(command)).unwrap();
    }
    fn execute(&self, request_id: u64, command: &str, arguments: Value) {
        let arguments = match arguments {
            Value::Array(values) => values,
            Value::Null => vec![],
            value => vec![value],
        };
        self.send(LspCommand::ExecuteCommand {
            request_id,
            command: command.into(),
            arguments,
        });
    }
    fn until(&self, predicate: impl Fn(&LspUiEvent) -> bool) -> Vec<LspUiEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = Vec::new();
        loop {
            let event = self
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("worker did not produce the expected event");
            let done = predicate(&event);
            seen.push(event);
            if done {
                return seen;
            }
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self
            .commands
            .send(WorkerMessage::Command(LspCommand::Shutdown));
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn stdio_initialization_execute_command_and_malformed_response() {
    let server = Server::new();
    let arguments = json!(["🌎", {"nested":[1,true]}]);
    server.execute(41, "echo", arguments.clone());
    let events =
        server.until(|e| matches!(e, LspUiEvent::ServerCommandCompleted { request_id: 41, .. }));
    assert!(events.iter().any(
        |e| matches!(e, LspUiEvent::ServerCommandCompleted { result:Ok(v), .. } if v == &arguments)
    ));
    server.send(LspCommand::Complete {
        request_id: 42,
        uri: "file:///fixture.rs".into(),
        position: Position::new(0, 0),
    });
    let events = server.until(|e| matches!(e, LspUiEvent::RequestFailed { request_id: 42, .. }));
    assert!(!events
        .iter()
        .any(|e| matches!(e, LspUiEvent::Completion { .. })));
    server.execute(43, "echo", Value::Null);
    server.until(|e| matches!(e, LspUiEvent::ServerCommandCompleted { request_id:43, result:Ok(v) } if v == &json!([])));
}

#[test]
fn stdio_cancellation_discards_late_reply_and_keeps_transport_alive() {
    let server = Server::new();
    server.execute(51, "hold", Value::Null);
    server.until(|e| matches!(e, LspUiEvent::Status(s) if s == "fixture:held"));
    server.send(LspCommand::Cancel { request_id: 51 });
    let events = server.until(|e| matches!(e, LspUiEvent::Status(s) if s == "fixture:cancelled"));
    assert!(!events
        .iter()
        .any(|e| matches!(e, LspUiEvent::ServerCommandCompleted { request_id: 51, .. })));
    server.execute(52, "echo", json!("alive"));
    server.until(|e| matches!(e, LspUiEvent::ServerCommandCompleted { request_id: 52, .. }));
}

#[test]
fn stdio_apply_edit_round_trip_and_crash_complete_pending_requests_once() {
    let server = Server::new();
    let params = json!({"edit":{"changes":{"file:///fixture.rs":[{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"newText":"🌎"}]}}});
    server.execute(61, "apply", json!([params]));
    let events = server.until(|e| matches!(e, LspUiEvent::ApplyEdit { .. }));
    match events.last().unwrap() {
        LspUiEvent::ApplyEdit {
            id,
            edit,
            reply,
            active,
        } => {
            assert!(active.load(std::sync::atomic::Ordering::Acquire));
            assert_eq!(
                edit.changes.as_ref().unwrap().values().next().unwrap()[0]
                    .range
                    .end
                    .character,
                3
            );
            reply
                .send(lsp_server::Response::new_ok(
                    id.clone(),
                    json!({"applied":true}),
                ))
                .unwrap();
        }
        _ => unreachable!(),
    }
    server.until(|e| matches!(e, LspUiEvent::ServerCommandCompleted { request_id:61, result:Ok(v) } if v["applied"] == true));
    server.execute(62, "hold", Value::Null);
    server.until(|e| matches!(e, LspUiEvent::Status(s) if s == "fixture:held"));
    server.execute(63, "crash", Value::Null);
    let events = server.until(|e| matches!(e, LspUiEvent::ServerStopped { .. }));
    for id in [62, 63] {
        assert_eq!(events.iter().filter(|e| matches!(e, LspUiEvent::RequestFailed { request_id, .. } if *request_id == id)).count(),1);
    }
}

#[test]
fn stdio_apply_edit_uses_event_loop_transactions_and_rejects_partial_batches() {
    let server = Server::new();
    let dir = crate::test_support::TempDir::new();
    let first = dir.path().join("open.txt");
    let second = dir.path().join("closed.txt");
    std::fs::write(&first, "a🌎b").unwrap();
    std::fs::write(&second, "closed").unwrap();
    let mut app = crate::App::new_with_options(dir.path().into(), None, true);
    app.open_file(first.clone());
    let uri = path_to_uri(&first).unwrap();
    let closed_uri = path_to_uri(&second).unwrap();
    for (id, end, applied) in [(71, 2, false), (72, 3, true)] {
        let mut changes = serde_json::Map::new();
        changes.insert(uri.clone(), json!([{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":end}},"newText":"é"}]));
        changes.insert(closed_uri.clone(), json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"newText":"Q"}]));
        server.execute(id, "apply", json!([{"edit":{"changes":changes}}]));
        let events = server.until(|e| matches!(e, LspUiEvent::ApplyEdit { .. }));
        app.lsp_queue.lock().unwrap().extend(events);
        app.process_lsp_events();
        server.until(|e| matches!(e, LspUiEvent::ServerCommandCompleted { request_id, result:Ok(v) } if *request_id == id && v["applied"] == applied));
        assert_eq!(app.editor().text(), if applied { "aéb" } else { "a🌎b" });
        assert_eq!(app.editor().revision(), u64::from(applied));
        assert_eq!(app.documents.len(), if applied { 2 } else { 1 });
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "a🌎b");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "closed");
    }
    app.editor_mut().undo();
    assert_eq!(app.editor().text(), "a🌎b");
}
