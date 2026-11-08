mod dispatch;
mod protocol;
mod worker;
use dispatch::dispatch_server_message;
#[cfg(test)]
use protocol::incremental_text_change;
use protocol::{change_params, hover_to_string, text_sync_kind};
pub use protocol::{diagnostics_are_current, path_to_uri, position_at_char, uri_to_path};
use worker::run_server;

use lsp_server::{Message, Notification, Request, RequestId};
use lsp_types::{
    CodeActionOrCommand, CompletionItem, Diagnostic, DidChangeTextDocumentParams, Hover, InlayHint,
    Location, Position, PublishDiagnosticsParams, Range, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
    VersionedTextDocumentIdentifier,
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
    InlayHints {
        request_id: u64,
        hints: Vec<InlayHint>,
    },
    CodeActions {
        request_id: u64,
        actions: Vec<CodeActionOrCommand>,
        error: Option<String>,
    },
    JsonDiagnostics {
        uri: String,
        revision: u64,
        diagnostics: Vec<Diagnostic>,
    },
    JsonCompletion {
        request_id: u64,
        items: Vec<CompletionItem>,
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
    InlayHints {
        request_id: u64,
        uri: String,
        start: Position,
        end: Position,
    },
    CodeActions {
        request_id: u64,
        uri: String,
        range: Range,
        diagnostics: Vec<Diagnostic>,
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

    pub fn request_inlay_hints(
        &self,
        path: &Path,
        start: Position,
        end: Position,
        request_id: u64,
    ) {
        self.send(
            path,
            LspCommand::InlayHints {
                request_id,
                uri: String::new(),
                start,
                end,
            },
        );
    }

    pub fn request_code_actions(
        &self,
        path: &Path,
        range: Range,
        diagnostics: Vec<Diagnostic>,
        request_id: u64,
    ) {
        self.send(
            path,
            LspCommand::CodeActions {
                request_id,
                uri: String::new(),
                range,
                diagnostics,
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
            | LspCommand::Hover { uri: target, .. }
            | LspCommand::InlayHints { uri: target, .. }
            | LspCommand::CodeActions { uri: target, .. } => *target = uri.clone(),
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
    fn incremental_change_replaces_multiline_astral_text_in_utf16() {
        let event = incremental_text_change("prefix 🌎\nold end", "prefix é\nnew end");
        assert_eq!(
            event.range,
            Some(Range::new(Position::new(0, 7), Position::new(1, 3)))
        );
        assert_eq!(event.range_length, Some(6));
        assert_eq!(event.text, "é\nnew");
    }

    #[test]
    fn server_config_normalizes_extensions_and_rejects_duplicates() {
        let temp = crate::test_support::TempDir::new();
        let path = temp.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"servers":[{"extension":".rs","languageId":"rust","command":"rust-analyzer"}]}"#,
        )
        .unwrap();
        let config = LspManager::load_config(&path).unwrap();
        assert_eq!(config[0].extension, "rs");
        std::fs::write(&path, r#"{"servers":[{"extension":"rs","languageId":"rust","command":"rust-analyzer"},{"extension":".rs","languageId":"rust","command":"other"}]}"#).unwrap();
        assert!(LspManager::load_config(&path).is_err());
    }

    #[test]
    fn file_uri_round_trip_handles_spaces_and_unicode() {
        let temp = crate::test_support::TempDir::new();
        let path = temp.path().join("a file 🌎.rs");
        std::fs::write(&path, "").unwrap();
        let uri = path_to_uri(&path).unwrap();
        assert!(uri.contains("%20"));
        assert_eq!(uri_to_path(&uri), Some(path.clone()));
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
    fn code_action_response_parses_workspace_edits_for_the_requested_diagnostic() {
        let (event_tx, event_rx) = mpsc::channel();
        let mut pending = HashMap::from([(7, (91, "codeActions"))]);
        dispatch_server_message(
            Message::Response(lsp_server::Response {
                id: RequestId::from(7),
                result: Some(json!([{
                    "title": "Replace token",
                    "kind": "quickfix",
                    "edit": {"changes": {"file:///workspace/main.rs": [{
                        "range": {"start": {"line": 0, "character": 1}, "end": {"line": 0, "character": 2}},
                        "newText": "x"
                    }]}}
                }])),
                error: None,
            }),
            &mut pending,
            &event_tx,
        );
        match event_rx.recv().unwrap() {
            LspUiEvent::CodeActions {
                request_id,
                actions,
                error,
            } => {
                assert_eq!(request_id, 91);
                assert_eq!(actions.len(), 1);
                assert!(error.is_none());
                let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
                    panic!("expected a code action")
                };
                assert_eq!(action.title, "Replace token");
                assert!(action.edit.is_some());
            }
            event => panic!("unexpected LSP event: {event:?}"),
        }
    }

    #[test]
    fn code_action_server_errors_reach_the_ui_instead_of_looking_like_no_fixes() {
        let (event_tx, event_rx) = mpsc::channel();
        let mut pending = HashMap::from([(8, (92, "codeActions"))]);
        dispatch_server_message(
            Message::Response(lsp_server::Response {
                id: RequestId::from(8),
                result: None,
                error: Some(lsp_server::ResponseError {
                    code: -32603,
                    message: "server failed".into(),
                    data: None,
                }),
            }),
            &mut pending,
            &event_tx,
        );
        match event_rx.recv().unwrap() {
            LspUiEvent::CodeActions {
                request_id,
                actions,
                error,
            } => {
                assert_eq!(request_id, 92);
                assert!(actions.is_empty());
                assert_eq!(error.as_deref(), Some("server failed"));
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
