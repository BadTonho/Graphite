use super::requests::PendingClientRequests;
pub(super) use super::requests::ServerRequests;
use super::*;

pub(super) fn run_server(
    config: ServerConfig,
    root: PathBuf,
    receiver: Receiver<WorkerMessage>,
    event_tx: Sender<LspUiEvent>,
    translator: Translator,
) {
    let _lifetime = ServerLifetime {
        events: event_tx.clone(),
        extension: config.extension.clone(),
    };
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
            let _ = event_tx.send(LspUiEvent::Status(translator.format(
                "lsp.status.start_failed",
                &[("command", &config.command), ("error", &error)],
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
                Ok(None) => {
                    let _ = reader_tx.send(WorkerMessage::IoError(
                        translator.text("lsp.error.stdout_closed").into(),
                    ));
                    break;
                }
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
                "workspace": {"applyEdit": true,"configuration":true,"workspaceFolders":true},
                "textDocument": {
                    "synchronization": { "willSave": true, "willSaveWaitUntil": true, "didSave": true },
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
            translator.text("lsp.status.initialize_failed").into(),
        ));
        stop_child(&mut child);
        return;
    }
    let initialize_deadline = std::time::Instant::now() + Duration::from_secs(20);
    let (sync_kind, save_capabilities) = loop {
        if std::time::Instant::now() >= initialize_deadline {
            let _ = event_tx.send(LspUiEvent::Status(
                translator.text("lsp.status.initialize_timeout").into(),
            ));
            stop_child(&mut child);
            return;
        }
        match signal_rx.recv_timeout(
            initialize_deadline
                .saturating_duration_since(std::time::Instant::now())
                .min(Duration::from_millis(40)),
        ) {
            Ok(WorkerMessage::Incoming(Message::Response(response)))
                if response.id == RequestId::from(1) =>
            {
                if let Some(error) = response.error {
                    let _ = event_tx.send(LspUiEvent::Status(translator.format(
                        "lsp.status.initialize_failed_detail",
                        &[("language", &config.language_id), ("error", &error.message)],
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
                let _ = event_tx.send(LspUiEvent::Status(
                    translator.format("lsp.status.ready", &[("language", &config.language_id)]),
                ));
                break (kind, protocol::SaveCapabilities::from_sync(sync));
            }
            Ok(WorkerMessage::Incoming(_)) => {}
            Ok(WorkerMessage::IoError(error)) => {
                let _ = event_tx.send(LspUiEvent::Status(translator.format(
                    "lsp.status.io_error",
                    &[("language", &config.language_id), ("error", &error)],
                )));
                stop_child(&mut child);
                return;
            }
            Ok(WorkerMessage::Command(LspCommand::Shutdown))
            | Err(RecvTimeoutError::Disconnected) => {
                stop_child(&mut child);
                return;
            }
            Ok(command @ WorkerMessage::Command(_)) => {
                if deferred.len() < graphite_core_api::protocol::MAX_PENDING {
                    deferred.push(command);
                } else if let WorkerMessage::Command(command) = command {
                    if let Some(request_id) = command.request_id() {
                        if let LspCommand::BeforeSave { reply, .. } = &command {
                            let _ = reply.send(Err(graphite_core_api::ServiceError::Busy));
                        }
                        let _ = event_tx.send(LspUiEvent::RequestRejected {
                            request_id,
                            error: graphite_core_api::ServiceError::Busy,
                        });
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) if std::time::Instant::now() < initialize_deadline => {
                continue
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = event_tx.send(LspUiEvent::Status(translator.format(
                    "lsp.status.initialize_timeout_detail",
                    &[("language", &config.language_id)],
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
    let mut pending = PendingClientRequests::new(event_tx.clone(), translator);
    let mut server_requests = ServerRequests::default();
    let mut next_id = 2i32;
    let mut save_replies = HashMap::<
        i32,
        (
            Sender<graphite_core_api::ServiceResult<Vec<graphite_core_api::TextEdit>>>,
            std::time::Instant,
        ),
    >::new();
    let mut child_exited = false;
    loop {
        for response in server_requests.poll(std::time::Instant::now()) {
            let _ = write_message(&mut writer, Message::Response(response));
        }
        if child.try_wait().ok().flatten().is_some() {
            child_exited = true;
        }
        if child_exited {
            break;
        }
        for message in pending.expire(std::time::Instant::now()) {
            if write_message(&mut writer, message).is_err() {
                stop_child(&mut child);
                return;
            }
        }
        let expired: Vec<_> = save_replies
            .iter()
            .filter(|(_, (_, deadline))| std::time::Instant::now() >= *deadline)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some((reply, _)) = save_replies.remove(&id) {
                let _ = reply.send(Err(graphite_core_api::ServiceError::Timeout));
            }
            pending.remove(&id);
            let _ = write_message(
                &mut writer,
                Message::Notification(Notification {
                    method: "$/cancelRequest".into(),
                    params: json!({"id":id}),
                }),
            );
        }
        match signal_rx.recv_timeout(Duration::from_millis(40)) {
            Ok(WorkerMessage::Command(command)) => {
                if let Some(request_id) = command.request_id() {
                    if pending.len() >= graphite_core_api::protocol::MAX_PENDING {
                        if let LspCommand::BeforeSave { reply, .. } = &command {
                            let _ = reply.send(Err(graphite_core_api::ServiceError::Busy));
                        }
                        let _ = event_tx.send(LspUiEvent::RequestRejected {
                            request_id,
                            error: graphite_core_api::ServiceError::Busy,
                        });
                        continue;
                    }
                }
                let messages = match command {
                    LspCommand::Cancel { request_id } => {
                        let ids: Vec<_> = pending
                            .iter()
                            .filter(|(_, (ui, _))| *ui == request_id)
                            .map(|(id, _)| *id)
                            .collect();
                        for id in &ids {
                            pending.remove(id);
                            save_replies.remove(id);
                        }
                        ids.into_iter()
                            .map(|id| {
                                Message::Notification(Notification {
                                    method: "$/cancelRequest".into(),
                                    params: json!({"id": id}),
                                })
                            })
                            .collect()
                    }
                    LspCommand::ExecuteCommand {
                        request_id,
                        command,
                        arguments,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, "executeCommand"));
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
                    LspCommand::BeforeSave {
                        request_id,
                        uri,
                        reason,
                        deadline,
                        reply,
                    } => {
                        if std::time::Instant::now() >= deadline {
                            let _ = reply.send(Err(graphite_core_api::ServiceError::Timeout));
                            continue;
                        }
                        let mut messages = Vec::new();
                        if save_capabilities.will_save {
                            messages.push(Message::Notification(Notification {
                                method: "textDocument/willSave".into(),
                                params: json!({"textDocument":{"uri":uri},"reason":reason}),
                            }));
                        }
                        if save_capabilities.wait_until {
                            let id = next_id;
                            next_id += 1;
                            pending.insert(id, (request_id, "willSaveWaitUntil"));
                            save_replies.insert(id, (reply, deadline));
                            messages.push(Message::Request(Request {
                                id: RequestId::from(id),
                                method: "textDocument/willSaveWaitUntil".into(),
                                params: json!({"textDocument":{"uri":uri},"reason":reason}),
                            }));
                        } else {
                            let _ = reply.send(Ok(Vec::new()));
                        }
                        messages
                    }
                    LspCommand::Save { uri, text } => {
                        save_capabilities.did_save(&uri, text).into_iter().collect()
                    }
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
                    LspCommand::Query {
                        request_id,
                        method,
                        kind,
                        params,
                    } => {
                        let id = next_id;
                        next_id += 1;
                        pending.insert(id, (request_id, kind));
                        vec![Message::Request(Request {
                            id: RequestId::from(id),
                            method: method.into(),
                            params,
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
                    if write_message(&mut writer, message).is_err() {
                        stop_child(&mut child);
                        return;
                    }
                }
            }
            Ok(WorkerMessage::Incoming(Message::Request(request))) => {
                if let Some(response) = server_requests.handle(request, &event_tx) {
                    let _ = write_message(&mut writer, Message::Response(response));
                }
            }
            Ok(WorkerMessage::Incoming(Message::Notification(notification)))
                if notification.method == "$/cancelRequest" =>
            {
                if let Some(id) = notification
                    .params
                    .get("id")
                    .and_then(|id| serde_json::from_value::<RequestId>(id.clone()).ok())
                {
                    if let Some(response) = server_requests.cancel(&id) {
                        let _ = write_message(&mut writer, Message::Response(response));
                    }
                }
            }
            Ok(WorkerMessage::Incoming(message)) => {
                if let Message::Response(response) = &message {
                    let wire = serde_json::to_value(&response.id)
                        .ok()
                        .and_then(|v| v.as_i64())
                        .and_then(|id| i32::try_from(id).ok());
                    if let Some((reply, deadline)) = wire.and_then(|id| save_replies.remove(&id)) {
                        pending.remove(&wire.unwrap());
                        let result = if std::time::Instant::now() >= deadline {
                            Err(graphite_core_api::ServiceError::Timeout)
                        } else if let Some(error) = &response.error {
                            Err(graphite_core_api::ServiceError::Failed(
                                error.message.clone(),
                            ))
                        } else {
                            protocol::save_edits(response.result.clone().unwrap_or(Value::Null))
                        };
                        let _ = reply.send(result);
                        continue;
                    }
                }
                dispatch_server_message(message, &mut pending, &event_tx, &translator);
            }
            Ok(WorkerMessage::IoError(error)) => {
                let _ = event_tx.send(LspUiEvent::Status(translator.format(
                    "lsp.status.stopped",
                    &[("language", &config.language_id), ("error", &error)],
                )));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    stop_child(&mut child);
    let _ = event_tx.send(LspUiEvent::Status(translator.format(
        "lsp.status.server_stopped",
        &[("language", &config.language_id)],
    )));
}

struct ServerLifetime {
    events: Sender<LspUiEvent>,
    extension: String,
}
impl Drop for ServerLifetime {
    fn drop(&mut self) {
        let _ = self.events.send(LspUiEvent::ServerStopped {
            extension: self.extension.clone(),
        });
    }
}

pub(super) fn stop_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn write_message(writer: &mut impl Write, message: Message) -> std::io::Result<()> {
    message.write(writer)?;
    writer.flush()
}
