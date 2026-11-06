use lsp_server::{Message, Notification, Request, RequestId};
use lsp_types::{
    CompletionItem, Diagnostic, DidChangeTextDocumentParams, Hover, Location, Position,
    PublishDiagnosticsParams, Range, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, VersionedTextDocumentIdentifier,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub extension: String,
    #[serde(rename = "languageId")]
    pub language_id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ServerConfigFile {
    #[serde(default)]
    servers: Vec<ServerConfig>,
}

#[derive(Debug, Clone)]
pub enum LspUiEvent {
    Status(String),
    Diagnostics {
        uri: String,
        version: Option<i32>,
        diagnostics: Vec<Diagnostic>,
    },
    Completion {
        request_id: u64,
        items: Vec<CompletionItem>,
    },
    Definition {
        request_id: u64,
        location: Option<Location>,
    },
    Hover {
        request_id: u64,
        text: Option<String>,
    },
}

#[derive(Debug)]
enum LspCommand {
    Open {
        uri: String,
        language_id: String,
        text: String,
        version: i32,
    },
    Change {
        uri: String,
        text: String,
        version: i32,
    },
    Close {
        uri: String,
    },
    Save {
        uri: String,
        text: String,
    },
    Complete {
        request_id: u64,
        uri: String,
        position: Position,
    },
    Definition {
        request_id: u64,
        uri: String,
        position: Position,
    },
    Hover {
        request_id: u64,
        uri: String,
        position: Position,
    },
    Shutdown,
}

#[derive(Debug)]
enum WorkerMessage {
    Command(LspCommand),
    Incoming(Message),
    IoError(String),
}

struct ServerHandle {
    sender: Sender<WorkerMessage>,
    worker: Option<thread::JoinHandle<()>>,
}

#[derive(Default)]
pub struct LspManager {
    configs: HashMap<String, ServerConfig>,
    servers: HashMap<String, ServerHandle>,
    documents: HashMap<PathBuf, String>,
}

impl LspManager {
    pub fn load_config(path: &Path) -> Result<Vec<ServerConfig>, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let config: ServerConfigFile = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let mut servers = HashMap::new();
        for mut server in config.servers {
            server.extension = server
                .extension
                .trim_start_matches('.')
                .to_ascii_lowercase();
            if server.extension.is_empty()
                || server.language_id.is_empty()
                || server.command.is_empty()
            {
                return Err("Each server needs extension, languageId, and command".into());
            }
            if servers.insert(server.extension.clone(), server).is_some() {
                return Err("Each file extension can have only one configured server".into());
            }
        }
        Ok(servers.into_values().collect())
    }

    pub fn from_config(configs: Vec<ServerConfig>) -> Self {
        Self {
            configs: configs
                .into_iter()
                .map(|c| (c.extension.to_ascii_lowercase(), c))
                .collect(),
            ..Default::default()
        }
    }

    pub fn supports_path(&self, path: &Path) -> bool {
        self.config_for(path).is_some()
    }

    fn config_for(&self, path: &Path) -> Option<&ServerConfig> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(|e| self.configs.get(&e.to_ascii_lowercase()))
    }

    fn ensure_server(
        &mut self,
        config: &ServerConfig,
        root: &Path,
        event_tx: Sender<LspUiEvent>,
    ) -> Sender<WorkerMessage> {
        if let Some(server) = self.servers.get(&config.extension) {
            return server.sender.clone();
        }
        let (sender, receiver) = mpsc::channel();
        let config = config.clone();
        let extension = config.extension.clone();
        let root = root.to_path_buf();
        let worker = thread::Builder::new()
            .name(format!("lsp-{}", config.language_id))
            .spawn(move || run_server(config, root, receiver, event_tx))
            .expect("failed to create LSP worker thread");
        self.servers.insert(
            extension,
            ServerHandle {
                sender: sender.clone(),
                worker: Some(worker),
            },
        );
        sender
    }

    pub fn open(
        &mut self,
        path: &Path,
        root: &Path,
        text: String,
        version: i32,
        event_tx: Sender<LspUiEvent>,
    ) {
        let Some(config) = self.config_for(path).cloned() else {
            return;
        };
        let Some(uri) = path_to_uri(path) else {
            let _ = event_tx.send(LspUiEvent::Status(format!(
                "Cannot create file URI for {}",
                path.display()
            )));
            return;
        };
        let sender = self.ensure_server(&config, root, event_tx);
        let _ = sender.send(WorkerMessage::Command(LspCommand::Open {
            uri: uri.clone(),
            language_id: config.language_id,
            text,
            version,
        }));
        self.documents.insert(path.to_path_buf(), uri);
    }

    pub fn change(&self, path: &Path, text: String, version: i32) {
        self.send(
            path,
            LspCommand::Change {
                uri: String::new(),
                text,
                version,
            },
        );
    }

    pub fn close(&mut self, path: &Path) {
        let uri = self.documents.remove(path).unwrap_or_default();
        self.send(path, LspCommand::Close { uri });
    }

    pub fn save(&self, path: &Path, text: String) {
        self.send(
            path,
            LspCommand::Save {
                uri: String::new(),
                text,
            },
        );
    }

    pub fn request_completion(&self, path: &Path, position: Position, request_id: u64) {
        self.send(
            path,
            LspCommand::Complete {
                request_id,
                uri: String::new(),
                position,
            },
        );
    }

    pub fn request_definition(&self, path: &Path, position: Position, request_id: u64) {
        self.send(
            path,
            LspCommand::Definition {
                request_id,
                uri: String::new(),
                position,
            },
        );
    }

    pub fn request_hover(&self, path: &Path, position: Position, request_id: u64) {
        self.send(
            path,
            LspCommand::Hover {
                request_id,
                uri: String::new(),
                position,
            },
        );
    }

    fn send(&self, path: &Path, mut command: LspCommand) {
        let Some(uri) = self.documents.get(path) else {
            return;
        };
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            return;
        };
        let Some(server) = self.servers.get(&extension.to_ascii_lowercase()) else {
            return;
        };
        match &mut command {
            LspCommand::Change { uri: target, .. }
            | LspCommand::Close { uri: target }
            | LspCommand::Save { uri: target, .. }
            | LspCommand::Complete { uri: target, .. }
            | LspCommand::Definition { uri: target, .. }
            | LspCommand::Hover { uri: target, .. } => *target = uri.clone(),
            _ => return,
        }
        let _ = server.sender.send(WorkerMessage::Command(command));
    }

    pub fn shutdown(&mut self) {
        for (path, uri) in &self.documents {
            if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                if let Some(server) = self.servers.get(&extension.to_ascii_lowercase()) {
                    let _ = server
                        .sender
                        .send(WorkerMessage::Command(LspCommand::Close {
                            uri: uri.clone(),
                        }));
                }
            }
        }
        for server in self.servers.values() {
            let _ = server
                .sender
                .send(WorkerMessage::Command(LspCommand::Shutdown));
        }
        for server in self.servers.values_mut() {
            if let Some(worker) = server.worker.take() {
                let _ = worker.join();
            }
        }
        self.servers.clear();
        self.documents.clear();
    }
}

fn run_server(
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

fn stop_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn write_message(writer: &mut impl Write, message: Message) -> std::io::Result<()> {
    message.write(writer)?;
    writer.flush()
}

fn dispatch_server_message(
    message: Message,
    pending: &mut HashMap<i32, (u64, &'static str)>,
    event_tx: &Sender<LspUiEvent>,
) {
    match message {
        Message::Notification(notification)
            if notification.method == "textDocument/publishDiagnostics" =>
        {
            if let Ok(params) =
                serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
            {
                let _ = event_tx.send(LspUiEvent::Diagnostics {
                    uri: params.uri.to_string(),
                    version: params.version,
                    diagnostics: params.diagnostics,
                });
            }
        }
        Message::Response(response) => {
            let Some((request_id, kind)) = serde_json::to_value(&response.id)
                .ok()
                .and_then(|id| serde_json::from_value::<i32>(id).ok())
                .and_then(|id| pending.remove(&id))
            else {
                return;
            };
            let result = response.result.unwrap_or(Value::Null);
            match kind {
                "completion" => {
                    let items = result
                        .as_array()
                        .cloned()
                        .or_else(|| result.get("items").and_then(Value::as_array).cloned())
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|v| serde_json::from_value::<CompletionItem>(v).ok())
                        .collect();
                    let _ = event_tx.send(LspUiEvent::Completion { request_id, items });
                }
                "definition" => {
                    let item = if result.is_array() {
                        result
                            .as_array()
                            .and_then(|a| a.first())
                            .cloned()
                            .unwrap_or(Value::Null)
                    } else {
                        result
                    };
                    let location = serde_json::from_value::<Location>(item.clone())
                        .ok()
                        .or_else(|| {
                            serde_json::from_value::<lsp_types::LocationLink>(item)
                                .ok()
                                .map(|link| {
                                    Location::new(link.target_uri, link.target_selection_range)
                                })
                        });
                    let _ = event_tx.send(LspUiEvent::Definition {
                        request_id,
                        location,
                    });
                }
                "hover" => {
                    let text = serde_json::from_value::<Hover>(result)
                        .ok()
                        .map(hover_to_string);
                    let _ = event_tx.send(LspUiEvent::Hover { request_id, text });
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn full_text_change(text: String) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text,
    }
}

fn text_sync_kind(capability: Option<&Value>) -> u64 {
    match capability {
        Some(value) if value.is_number() => value.as_u64().unwrap_or(0),
        Some(value) => value.get("change").and_then(Value::as_u64).unwrap_or(0),
        None => 0,
    }
}

fn change_params(
    uri: &str,
    previous: &str,
    text: &str,
    version: i32,
    sync_kind: u64,
) -> Option<DidChangeTextDocumentParams> {
    let change = match sync_kind {
        1 => full_text_change(text.to_owned()),
        2 => incremental_text_change(previous, text),
        _ => return None,
    };
    Some(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.parse().ok()?,
            version,
        },
        content_changes: vec![change],
    })
}

fn incremental_text_change(old: &str, new: &str) -> TextDocumentContentChangeEvent {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let mut prefix = 0;
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_chars.len().saturating_sub(prefix)
        && suffix < new_chars.len().saturating_sub(prefix)
        && old_chars[old_chars.len() - 1 - suffix] == new_chars[new_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let old_end = old_chars.len() - suffix;
    let new_end = new_chars.len() - suffix;
    let start = position_at_char(old, prefix);
    let end = position_at_char(old, old_end);
    let replacement: String = new_chars[prefix..new_end].iter().collect();
    let range_length = utf16_len(&old_chars[prefix..old_end]) as u32;
    TextDocumentContentChangeEvent {
        range: Some(Range::new(start, end)),
        range_length: Some(range_length),
        text: replacement,
    }
}

pub fn position_at_char(text: &str, char_index: usize) -> Position {
    let prefix: String = text.chars().take(char_index).collect();
    let line = prefix.bytes().filter(|b| *b == b'\n').count();
    let column = prefix.rsplit('\n').next().unwrap_or_default();
    Position::new(
        line as u32,
        utf16_len(&column.chars().collect::<Vec<_>>()) as u32,
    )
}

fn utf16_len(chars: &[char]) -> usize {
    chars.iter().map(|c| c.len_utf16()).sum()
}

pub fn path_to_uri(path: &Path) -> Option<String> {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut raw = absolute.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    if let Some(without_prefix) = raw.strip_prefix("//?/") {
        raw = without_prefix.to_owned();
    }
    let path_part = if cfg!(windows) && !raw.starts_with('/') {
        format!("/{raw}")
    } else {
        raw
    };
    let mut encoded = String::new();
    for byte in path_part.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    Some(format!("file://{encoded}"))
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    let mut path = PathBuf::from(String::from_utf8(decoded).ok()?);
    #[cfg(windows)]
    if path.to_string_lossy().starts_with("/:") || path.to_string_lossy().get(2..3) == Some(":") {
        let raw = path
            .to_string_lossy()
            .trim_start_matches('/')
            .replace('/', "\\");
        path = PathBuf::from(raw);
    }
    Some(path)
}

pub fn diagnostics_are_current(
    current_revision: Option<u64>,
    diagnostic_version: Option<i32>,
) -> bool {
    diagnostic_version.map_or(true, |version| {
        current_revision == Some(version.max(0) as u64)
    })
}

fn hover_to_string(hover: Hover) -> String {
    match hover.contents {
        lsp_types::HoverContents::Scalar(markup) => match markup {
            lsp_types::MarkedString::String(s) => s,
            lsp_types::MarkedString::LanguageString(s) => s.value,
        },
        lsp_types::HoverContents::Array(items) => items
            .into_iter()
            .map(|item| match item {
                lsp_types::MarkedString::String(s) => s,
                lsp_types::MarkedString::LanguageString(s) => s.value,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_positions_count_utf16_units() {
        let p = position_at_char("a🌎b\nx", 2);
        assert_eq!(p, Position::new(0, 3));
        assert_eq!(position_at_char("a🌎b\nx", 5), Position::new(1, 1));
    }

    #[test]
    fn incremental_change_keeps_unicode_boundaries_and_lines() {
        let event = incremental_text_change("a🌎b\nold", "a🌎b\nnew!");
        assert_eq!(
            event.range,
            Some(Range::new(Position::new(1, 0), Position::new(1, 3)))
        );
        assert_eq!(event.text, "new!");
        assert_eq!(event.range_length, Some(3));
    }

    #[test]
    fn server_config_normalizes_extensions_and_rejects_duplicates() {
        let path = std::env::temp_dir().join("graphite_lsp_config_test.json");
        std::fs::write(
            &path,
            r#"{"servers":[{"extension":".rs","languageId":"rust","command":"rust-analyzer"}]}"#,
        )
        .unwrap();
        let config = LspManager::load_config(&path).unwrap();
        assert_eq!(config[0].extension, "rs");
        std::fs::write(&path, r#"{"servers":[{"extension":"rs","languageId":"rust","command":"rust-analyzer"},{"extension":".rs","languageId":"rust","command":"other"}]}"#).unwrap();
        assert!(LspManager::load_config(&path).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_uri_round_trip_handles_spaces_and_unicode() {
        let root = std::env::temp_dir().join("shoppe uri 🌎");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("a file.rs");
        std::fs::write(&path, "").unwrap();
        let uri = path_to_uri(&path).unwrap();
        assert!(uri.contains("%20"));
        assert_eq!(uri_to_path(&uri), Some(path.clone()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_versioned_diagnostics_are_ignored_but_unversioned_are_accepted() {
        assert!(diagnostics_are_current(Some(4), Some(4)));
        assert!(!diagnostics_are_current(Some(4), Some(3)));
        assert!(diagnostics_are_current(Some(4), None));
    }

    #[test]
    fn change_sync_modes_build_correct_lsp_notifications() {
        let incremental = change_params("file:///workspace/a.rs", "a🌎b", "a🌎c", 5, 2).unwrap();
        assert_eq!(incremental.text_document.version, 5);
        assert_eq!(
            incremental.content_changes[0].range,
            Some(Range::new(Position::new(0, 3), Position::new(0, 4)))
        );
        assert_eq!(incremental.content_changes[0].text, "c");
        let full = change_params("file:///workspace/a.rs", "old", "new", 6, 1).unwrap();
        assert_eq!(full.content_changes[0].range, None);
        assert_eq!(full.content_changes[0].text, "new");
        assert!(change_params("file:///workspace/a.rs", "old", "new", 6, 0).is_none());
    }

    #[test]
    fn missing_sync_capability_does_not_send_document_changes() {
        assert_eq!(text_sync_kind(None), 0);
        assert_eq!(text_sync_kind(Some(&json!(2))), 2);
        assert_eq!(
            text_sync_kind(Some(&json!({"openClose": true, "change": 1}))),
            1
        );
        assert_eq!(text_sync_kind(Some(&json!({"openClose": true}))), 0);
    }

    #[test]
    fn publish_diagnostics_notification_is_parsed_for_its_document() {
        let (event_tx, event_rx) = mpsc::channel();
        let mut pending = HashMap::new();
        dispatch_server_message(
            Message::Notification(Notification {
                method: "textDocument/publishDiagnostics".into(),
                params: json!({
                    "uri": "file:///workspace/main.rs",
                    "version": 9,
                    "diagnostics": [{
                        "range": {"start": {"line": 2, "character": 1}, "end": {"line": 2, "character": 4}},
                        "message": "unexpected token",
                        "severity": 1
                    }]
                }),
            }),
            &mut pending,
            &event_tx,
        );
        match event_rx.recv().unwrap() {
            LspUiEvent::Diagnostics {
                uri,
                version,
                diagnostics,
            } => {
                assert_eq!(uri, "file:///workspace/main.rs");
                assert_eq!(version, Some(9));
                assert_eq!(diagnostics[0].message, "unexpected token");
            }
            event => panic!("unexpected LSP event: {event:?}"),
        }
    }

    #[test]
    fn completion_response_is_correlated_to_the_ui_request() {
        let (event_tx, event_rx) = mpsc::channel();
        let mut pending = HashMap::from([(4, (88, "completion"))]);
        dispatch_server_message(
            Message::Response(lsp_server::Response {
                id: RequestId::from(4),
                result: Some(json!([{"label": "println!"}])),
                error: None,
            }),
            &mut pending,
            &event_tx,
        );
        match event_rx.recv().unwrap() {
            LspUiEvent::Completion { request_id, items } => {
                assert_eq!(request_id, 88);
                assert_eq!(items[0].label, "println!");
            }
            event => panic!("unexpected LSP event: {event:?}"),
        }
    }

    #[test]
    fn missing_server_executable_reports_status_without_panicking() {
        let config = ServerConfig {
            extension: "none".into(),
            language_id: "test".into(),
            command: "graphite-no-such-lsp-executable-84729".into(),
            args: Vec::new(),
        };
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        run_server(config, std::env::temp_dir(), command_rx, event_tx);
        assert!(command_tx
            .send(WorkerMessage::Command(LspCommand::Shutdown))
            .is_err());
        assert!(
            matches!(event_rx.recv().unwrap(), LspUiEvent::Status(status) if status.contains("Could not start"))
        );
    }
}
