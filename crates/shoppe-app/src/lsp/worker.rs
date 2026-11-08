use super::*;

pub(super) fn run_server(
    config: ServerConfig,
    root: PathBuf,
    receiver: Receiver<WorkerMessage>,
    event_tx: Sender<LspUiEvent>,
) {
    let mut child = match Command::new(&config.command)
        .args(&config.args)
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = event_tx.send(LspUiEvent::Status(format!(
                "Could not start {}: {error}",
                config.command
            )));
            return;
        }
    };
    let Some(stdin) = child.stdin.take() else {
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        return;
    };
    let mut writer = BufWriter::new(stdin);
    let (signal_tx, signal_rx) = mpsc::channel();
    let command_tx = signal_tx.clone();
    thread::spawn(move || {
        while let Ok(command) = receiver.recv() {
            if command_tx.send(command).is_err() {
                break;
            }
        }
    });
    let reader_tx = signal_tx.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match Message::read(&mut reader) {
                Ok(Some(message)) => {
                    if reader_tx.send(WorkerMessage::Incoming(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = reader_tx.send(WorkerMessage::IoError(error.to_string()));
                    break;
                }
            }
        }
    });
    if let Some(stderr) = child.stderr.take() {
        let event_tx = event_tx.clone();
        let language_id = config.language_id.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok() {
                let text = line.trim();
                if !text.is_empty() {
                    let _ = event_tx.send(LspUiEvent::Status(format!("{}: {text}", language_id)));
                }
                line.clear();
            }
        });
    }
    let mut deferred = Vec::new();
    let init = Request {
        id: RequestId::from(1),
        method: "initialize".into(),
        params: json!({
            "processId": std::process::id(),
            "rootUri": path_to_uri(&root),
            "capabilities": {
                "textDocument": {
                    "completion": { "completionItem": { "snippetSupport": false } },
                    "hover": {},
                    "definition": {},
                    "inlayHint": { "dynamicRegistration": false },
                    "codeAction": { "codeActionLiteralSupport": { "codeActionKind": { "valueSet": ["quickfix"] } } },
                    "publishDiagnostics": { "relatedInformation": true }
                }
            },
            "workspaceFolders": [{"uri": path_to_uri(&root), "name": root.file_name().unwrap_or_default().to_string_lossy()}]
        }),
    };
    if write_message(&mut writer, Message::Request(init)).is_err() {
        let _ = event_tx.send(LspUiEvent::Status(
            "Could not initialize language server".into(),
        ));
        stop_child(&mut child);
        return;
    }
    let sync_kind = loop {
        match signal_rx.recv_timeout(Duration::from_secs(20)) {
            Ok(WorkerMessage::Incoming(Message::Response(response)))
                if response.id == RequestId::from(1) =>
            {
                if let Some(error) = response.error {
                    let _ = event_tx.send(LspUiEvent::Status(format!(
                        "{} LSP initialization failed: {}",
                        config.language_id, error.message
                    )));
                    stop_child(&mut child);
                    return;
                }
                let result = response.result.unwrap_or(Value::Null);
                let sync = result.pointer("/capabilities/textDocumentSync");
                let kind = text_sync_kind(sync);
                let _ = write_message(
                    &mut writer,
                    Message::Notification(Notification {
                        method: "initialized".into(),
                        params: json!({}),
                    }),
                );
                let _ = event_tx.send(LspUiEvent::Status(format!(
                    "{} language server ready",
                    config.language_id
                )));
                break kind;
            }
            Ok(WorkerMessage::Incoming(_)) => {}
            Ok(WorkerMessage::IoError(error)) => {
                let _ = event_tx.send(LspUiEvent::Status(format!(
                    "{} LSP: {error}",
                    config.language_id
                )));
                stop_child(&mut child);
                return;
            }
            Ok(WorkerMessage::Command(LspCommand::Shutdown))
            | Err(RecvTimeoutError::Disconnected) => {
                stop_child(&mut child);
                return;
            }
            Ok(command @ WorkerMessage::Command(_)) => deferred.push(command),
            Err(RecvTimeoutError::Timeout) => {
                let _ = event_tx.send(LspUiEvent::Status(format!(
                    "{} LSP initialization timed out",
                    config.language_id
                )));
                stop_child(&mut child);
                return;
            }
        }
    };
    for message in deferred {
        let _ = signal_tx.send(message);
    }
    let mut text_cache = HashMap::<String, String>::new();
    let mut pending = HashMap::<i32, (u64, &'static str)>::new();
    let mut next_id = 2i32;
    let mut child_exited = false;
    loop {
        if child.try_wait().ok().flatten().is_some() {
            child_exited = true;
        }
        if child_exited {
            break;
        }
        match signal_rx.recv_timeout(Duration::from_millis(40)) {
            Ok(WorkerMessage::Command(command)) => {
                let messages = match command {
                    LspCommand::ExecuteCommand { command, arguments } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (0, "executeCommand"));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: "workspace/executeCommand".into(),
                            params: json!({"command": command, "arguments": arguments}),
                        })]
                    }
                    LspCommand::Open {
                        uri,
                        language_id,
                        text,
                        version,
                    } => {
                        text_cache.insert(uri.clone(), text.clone());
                        vec![Message::Notification(Notification {
                            method: "textDocument/didOpen".into(),
                            params: serde_json::to_value(lsp_types::DidOpenTextDocumentParams {
                                text_document: TextDocumentItem {
                                    uri: uri.parse().unwrap(),
                                    language_id,
                                    version,
                                    text,
                                },
                            })
                            .unwrap_or_default(),
                        })]
                    }
                    LspCommand::Change { uri, text, version } => {
                        let previous = text_cache
                            .insert(uri.clone(), text.clone())
                            .unwrap_or_default();
                        change_params(&uri, &previous, &text, version, sync_kind)
                            .map(|params| {
                                Message::Notification(Notification {
                                    method: "textDocument/didChange".into(),
                                    params: serde_json::to_value(params).unwrap_or_default(),
                                })
                            })
                            .into_iter()
                            .collect()
                    }
                    LspCommand::Close { uri } => {
                        text_cache.remove(&uri);
                        vec![Message::Notification(Notification {
                            method: "textDocument/didClose".into(),
                            params: json!({"textDocument": {"uri": uri}}),
                        })]
                    }
                    LspCommand::Save { uri, text } => vec![Message::Notification(Notification {
                        method: "textDocument/didSave".into(),
                        params: json!({"textDocument": {"uri": uri}, "text": text}),
                    })],
                    LspCommand::Complete {
                        request_id,
                        uri,
                        position,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, "completion"));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: "textDocument/completion".into(),
                            params: serde_json::to_value(TextDocumentPositionParams {
                                text_document: TextDocumentIdentifier {
                                    uri: uri.parse().unwrap(),
                                },
                                position,
                            })
                            .unwrap_or_default(),
                        })]
                    }
                    LspCommand::Definition {
                        request_id,
                        uri,
                        position,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, "definition"));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: "textDocument/definition".into(),
                            params: serde_json::to_value(TextDocumentPositionParams {
                                text_document: TextDocumentIdentifier {
                                    uri: uri.parse().unwrap(),
                                },
                                position,
                            })
                            .unwrap_or_default(),
                        })]
                    }
                    LspCommand::Hover {
                        request_id,
                        uri,
                        position,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, "hover"));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: "textDocument/hover".into(),
                            params: serde_json::to_value(TextDocumentPositionParams {
                                text_document: TextDocumentIdentifier {
                                    uri: uri.parse().unwrap(),
                                },
                                position,
                            })
                            .unwrap_or_default(),
                        })]
                    }
                    LspCommand::InlayHints {
                        request_id,
                        uri,
                        start,
                        end,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, "inlayHints"));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: "textDocument/inlayHint".into(),
                            params: json!({
                                "textDocument": { "uri": uri },
                                "range": { "start": start, "end": end }
                            }),
                        })]
                    }
                    LspCommand::CodeActions {
                        request_id,
                        uri,
                        range,
                        diagnostics,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, "codeActions"));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: "textDocument/codeAction".into(),
                            params: json!({
                                "textDocument": { "uri": uri },
                                "range": range,
                                "context": { "diagnostics": diagnostics, "only": ["quickfix"], "triggerKind": 1 }
                            }),
                        })]
                    }
                    LspCommand::Shutdown => {
                        let shutdown_id = next_id;
                        let _ = write_message(
                            &mut writer,
                            Message::Request(Request {
                                id: RequestId::from(shutdown_id),
                                method: "shutdown".into(),
                                params: Value::Null,
                            }),
                        );
                        let deadline = std::time::Instant::now() + Duration::from_millis(750);
                        while std::time::Instant::now() < deadline {
                            match signal_rx.recv_timeout(Duration::from_millis(25)) {
                                Ok(WorkerMessage::Incoming(Message::Response(response)))
                                    if response.id == RequestId::from(shutdown_id) =>
                                {
                                    break
                                }
                                Ok(WorkerMessage::IoError(_))
                                | Err(RecvTimeoutError::Disconnected) => break,
                                _ => {}
                            }
                            if child.try_wait().ok().flatten().is_some() {
                                return;
                            }
                        }
                        let _ = write_message(
                            &mut writer,
                            Message::Notification(Notification {
                                method: "exit".into(),
                                params: Value::Null,
                            }),
                        );
                        let deadline = std::time::Instant::now() + Duration::from_millis(500);
                        while std::time::Instant::now() < deadline {
                            if child.try_wait().ok().flatten().is_some() {
                                return;
                            }
                            thread::sleep(Duration::from_millis(20));
                        }
                        stop_child(&mut child);
                        return;
                    }
                };
                for message in messages {
                    let _ = write_message(&mut writer, message);
                }
            }
            Ok(WorkerMessage::Incoming(message)) => {
                dispatch_server_message(message, &mut pending, &event_tx);
            }
            Ok(WorkerMessage::IoError(error)) => {
                let _ = event_tx.send(LspUiEvent::Status(format!(
                    "{} LSP stopped: {error}",
                    config.language_id
                )));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    stop_child(&mut child);
    let _ = event_tx.send(LspUiEvent::Status(format!(
        "{} language server stopped",
        config.language_id
    )));
}

pub(super) fn stop_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn write_message(writer: &mut impl Write, message: Message) -> std::io::Result<()> {
    message.write(writer)?;
    writer.flush()
}
