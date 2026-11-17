//! Pure-Rust SDK for building native compiled plugins (`"runtime": "native"`) for Graphite.

use graphite_core_api::protocol::{read_message, write_message, Message, RpcRequest, VERSION};
pub use graphite_core_api::*;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};

type CommandCallback = Arc<dyn Fn(&PluginContext, Value) -> ServiceResult<Value> + Send + Sync>;
type ChatToolCallback =
    Arc<dyn Fn(&PluginContext, Value) -> ServiceResult<ChatToolOutcome> + Send + Sync>;
type CompletionCallback = Arc<
    dyn Fn(&PluginContext, &DocumentUri, Position) -> ServiceResult<Vec<CompletionEntry>>
        + Send
        + Sync,
>;
type HoverCallback = Arc<
    dyn Fn(&PluginContext, &DocumentUri, Position) -> ServiceResult<Option<String>> + Send + Sync,
>;
type FormattingCallback = Arc<
    dyn Fn(&PluginContext, &DocumentUri, u32, bool) -> ServiceResult<Vec<TextEdit>> + Send + Sync,
>;
type SaveCallback = Arc<
    dyn Fn(&PluginContext, SaveParticipantRequest) -> ServiceResult<Vec<TextEdit>> + Send + Sync,
>;
type DocumentEventCallback = Arc<dyn Fn(&PluginContext, DocumentEvent) + Send + Sync>;

std::thread_local! {
    static CURRENT_PARENT: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

fn with_parent<T>(parent: u64, f: impl FnOnce() -> T) -> T {
    let prev = CURRENT_PARENT.with(|cell| cell.replace(Some(parent)));
    let result = f();
    CURRENT_PARENT.with(|cell| cell.set(prev));
    result
}

/// Trait implemented by native Rust plugins passed to [`run_plugin`].
pub trait Plugin: Send + Sync + 'static {
    fn activate(&self, ctx: &PluginContext) -> ServiceResult<()>;
    fn deactivate(&self) {}
}

struct SharedState {
    session: u64,
    next_id: AtomicU64,
    next_counter: AtomicU64,
    shutting_down: AtomicBool,
    writer: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<u64, mpsc::Sender<ServiceResult<Value>>>>,
    cancelled: Mutex<HashSet<u64>>,
    commands: Mutex<HashMap<String, CommandCallback>>,
    chat_tools: Mutex<HashMap<String, ChatToolCallback>>,
    completions: Mutex<HashMap<String, CompletionCallback>>,
    hovers: Mutex<HashMap<String, HoverCallback>>,
    formatters: Mutex<HashMap<String, FormattingCallback>>,
    save_participants: Mutex<HashMap<u64, SaveCallback>>,
    document_listeners: Mutex<HashMap<u64, DocumentEventCallback>>,
}

/// Thread-safe handle giving native Rust plugins access to all Graphite host services.
#[derive(Clone)]
pub struct PluginContext {
    state: Arc<SharedState>,
    plugin_root: Option<PathBuf>,
}

impl PluginContext {
    /// Returns the plugin installation directory passed as `argv[1]`.
    pub fn plugin_root(&self) -> Option<&std::path::Path> {
        self.plugin_root.as_deref()
    }

    fn write_frame(&self, message: &Message) -> ServiceResult<()> {
        let mut writer = self
            .state
            .writer
            .lock()
            .map_err(|_| ServiceError::Failed("Writer lock poisoned".into()))?;
        write_message(&mut *writer, message).map_err(|e| ServiceError::Io(e.to_string()))?;
        writer.flush().map_err(|e| ServiceError::Io(e.to_string()))
    }

    /// Sends a raw [`RpcRequest`] to the Graphite editor and awaits its response.
    pub fn request(&self, request: RpcRequest) -> ServiceResult<Value> {
        if self.state.shutting_down.load(Ordering::Relaxed) {
            return Err(ServiceError::HostStopped);
        }
        let id = self.state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = mpsc::channel();
        {
            let mut pending = self
                .state
                .pending
                .lock()
                .map_err(|_| ServiceError::Failed("Pending map poisoned".into()))?;
            if self.state.shutting_down.load(Ordering::Relaxed) {
                return Err(ServiceError::HostStopped);
            }
            pending.insert(id, tx);
        }
        if let Err(err) = self.write_frame(&Message::Request {
            session: self.state.session,
            id,
            request,
        }) {
            if let Ok(mut map) = self.state.pending.lock() {
                map.remove(&id);
            }
            return Err(err);
        }
        rx.recv().unwrap_or(Err(ServiceError::HostStopped))
    }

    /// Sends a typed [`ServiceRequest`] and deserializes its result.
    pub fn service<T: DeserializeOwned>(&self, request: ServiceRequest) -> ServiceResult<T> {
        let parent = CURRENT_PARENT.with(|c| c.get());
        let val = self.request(RpcRequest::Service { request, parent })?;
        serde_json::from_value(val).map_err(|e| ServiceError::Protocol(e.to_string()))
    }

    fn next_token(&self, prefix: &str) -> String {
        let n = self.state.next_counter.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{n}")
    }

    /// Registers an editor command declared in `graphite-plugin.json`.
    pub fn register_command<F>(&self, id: &str, title: &str, handler: F) -> ServiceResult<u64>
    where
        F: Fn(&PluginContext, Value) -> ServiceResult<Value> + Send + Sync + 'static,
    {
        self.state
            .commands
            .lock()
            .map_err(|_| ServiceError::Failed("Commands lock poisoned".into()))?
            .insert(id.to_owned(), Arc::new(handler));
        let val = self.request(RpcRequest::RegisterCommand(CommandMetadata {
            id: id.to_owned(),
            title: title.to_owned(),
            category: "Plugin".into(),
            targets: vec![TargetKind::Global],
            enablement: "true".into(),
        }))?;
        Ok(val["resource"].as_u64().unwrap_or(0))
    }

    /// Registers a chat tool callable by Graphite AI agents (`"chat"` permission).
    pub fn register_chat_tool<F>(
        &self,
        name: &str,
        description: &str,
        schema: Value,
        handler: F,
    ) -> ServiceResult<String>
    where
        F: Fn(&PluginContext, Value) -> ServiceResult<ChatToolOutcome> + Send + Sync + 'static,
    {
        let tool_id = self.next_token("rust-tool");
        self.state
            .chat_tools
            .lock()
            .map_err(|_| ServiceError::Failed("Chat tools lock poisoned".into()))?
            .insert(tool_id.clone(), Arc::new(handler));
        let _: Value = self.service(ServiceRequest::RegisterChatTool {
            tool_id: tool_id.clone(),
            name: name.to_owned(),
            description: description.to_owned(),
            schema,
        })?;
        Ok(tool_id)
    }

    /// Registers a completion provider (`"languages"` permission).
    pub fn register_completion_provider<F>(
        &self,
        languages: Vec<String>,
        handler: F,
    ) -> ServiceResult<String>
    where
        F: Fn(&PluginContext, &DocumentUri, Position) -> ServiceResult<Vec<CompletionEntry>>
            + Send
            + Sync
            + 'static,
    {
        let provider_id = self.next_token("rust-lang");
        self.state
            .completions
            .lock()
            .map_err(|_| ServiceError::Failed("Completions lock poisoned".into()))?
            .insert(provider_id.clone(), Arc::new(handler));
        let _: Value = self.service(ServiceRequest::RegisterLanguageProvider {
            provider_id: provider_id.clone(),
            kind: LanguageProviderKind::Completion,
            languages,
        })?;
        Ok(provider_id)
    }

    /// Registers a hover provider (`"languages"` permission).
    pub fn register_hover_provider<F>(
        &self,
        languages: Vec<String>,
        handler: F,
    ) -> ServiceResult<String>
    where
        F: Fn(&PluginContext, &DocumentUri, Position) -> ServiceResult<Option<String>>
            + Send
            + Sync
            + 'static,
    {
        let provider_id = self.next_token("rust-lang");
        self.state
            .hovers
            .lock()
            .map_err(|_| ServiceError::Failed("Hovers lock poisoned".into()))?
            .insert(provider_id.clone(), Arc::new(handler));
        let _: Value = self.service(ServiceRequest::RegisterLanguageProvider {
            provider_id: provider_id.clone(),
            kind: LanguageProviderKind::Hover,
            languages,
        })?;
        Ok(provider_id)
    }

    /// Registers a document formatting provider (`"languages"` permission).
    pub fn register_formatting_provider<F>(
        &self,
        languages: Vec<String>,
        handler: F,
    ) -> ServiceResult<String>
    where
        F: Fn(&PluginContext, &DocumentUri, u32, bool) -> ServiceResult<Vec<TextEdit>>
            + Send
            + Sync
            + 'static,
    {
        let provider_id = self.next_token("rust-lang");
        self.state
            .formatters
            .lock()
            .map_err(|_| ServiceError::Failed("Formatters lock poisoned".into()))?
            .insert(provider_id.clone(), Arc::new(handler));
        let _: Value = self.service(ServiceRequest::RegisterLanguageProvider {
            provider_id: provider_id.clone(),
            kind: LanguageProviderKind::Formatter,
            languages,
        })?;
        Ok(provider_id)
    }

    /// Registers an `onWillSave` participant (`"documents.read"` permission).
    pub fn register_save_participant<F>(
        &self,
        id: &str,
        languages: Vec<String>,
        handler: F,
    ) -> ServiceResult<u64>
    where
        F: Fn(&PluginContext, SaveParticipantRequest) -> ServiceResult<Vec<TextEdit>>
            + Send
            + Sync
            + 'static,
    {
        let val = self.request(RpcRequest::RegisterSaveParticipant(
            SaveParticipantMetadata {
                id: id.to_owned(),
                languages,
            },
        ))?;
        let resource = val["resource"]
            .as_u64()
            .ok_or_else(|| ServiceError::Protocol("Missing save participant resource".into()))?;
        self.state
            .save_participants
            .lock()
            .map_err(|_| ServiceError::Failed("Save participants lock poisoned".into()))?
            .insert(resource, Arc::new(handler));
        Ok(resource)
    }

    /// Subscribes to open/change/close/save document events (`"documents.read"` permission).
    pub fn subscribe_documents<F>(&self, handler: F) -> ServiceResult<u64>
    where
        F: Fn(&PluginContext, DocumentEvent) + Send + Sync + 'static,
    {
        let val = self.request(RpcRequest::SubscribeDocuments)?;
        let resource = val["resource"]
            .as_u64()
            .ok_or_else(|| ServiceError::Protocol("Missing subscription resource".into()))?;
        self.state
            .document_listeners
            .lock()
            .map_err(|_| ServiceError::Failed("Listeners lock poisoned".into()))?
            .insert(resource, Arc::new(handler));
        Ok(resource)
    }

    // Convenience service wrappers

    pub fn list_commands(&self) -> ServiceResult<Vec<CommandMetadata>> {
        self.service(ServiceRequest::Commands)
    }

    pub fn execute_command(
        &self,
        id: &str,
        arguments: Value,
        target: CommandTarget,
    ) -> ServiceResult<Value> {
        self.service(ServiceRequest::Execute(CommandRequest {
            id: id.to_owned(),
            arguments,
            target,
        }))
    }

    pub fn list_documents(&self) -> ServiceResult<Vec<DocumentSnapshot>> {
        self.service(ServiceRequest::Documents)
    }

    pub fn open_document(&self, uri: DocumentUri) -> ServiceResult<DocumentSnapshot> {
        self.service(ServiceRequest::Open { uri })
    }

    pub fn snapshot_document(&self, uri: DocumentUri) -> ServiceResult<DocumentSnapshot> {
        self.service(ServiceRequest::Snapshot { uri })
    }

    pub fn apply_edits(&self, edits: Vec<DocumentEdit>) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::ApplyEdits { edits })?;
        Ok(())
    }

    pub fn show_message(&self, text: impl Into<String>) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::Message { text: text.into() })?;
        Ok(())
    }

    pub fn set_status_bar_item(
        &self,
        id: impl Into<String>,
        text: impl Into<String>,
        command: Option<String>,
    ) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::SetStatusBarItem {
            id: id.into(),
            text: text.into(),
            command,
        })?;
        Ok(())
    }

    pub fn active_editor(&self) -> ServiceResult<Option<ActiveEditorSnapshot>> {
        self.service(ServiceRequest::ActiveEditor)
    }

    pub fn set_diagnostics(
        &self,
        owner: impl Into<String>,
        uri: DocumentUri,
        diagnostics: Vec<DiagnosticEntry>,
    ) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::SetDiagnostics {
            owner: owner.into(),
            uri,
            diagnostics,
        })?;
        Ok(())
    }

    pub fn workspace_info(&self) -> ServiceResult<WorkspaceSnapshot> {
        self.service(ServiceRequest::Workspace)
    }

    pub fn find_files(
        &self,
        include: Option<String>,
        exclude: Option<String>,
        max_results: Option<usize>,
    ) -> ServiceResult<Vec<String>> {
        self.service(ServiceRequest::WorkspaceFindFiles {
            include,
            exclude,
            max_results,
        })
    }

    pub fn read_workspace_file(&self, path: impl Into<String>) -> ServiceResult<String> {
        self.service(ServiceRequest::WorkspaceReadFile { path: path.into() })
    }

    pub fn write_workspace_file(
        &self,
        path: impl Into<String>,
        content: impl Into<String>,
    ) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::WorkspaceWriteFile {
            path: path.into(),
            content: content.into(),
        })?;
        Ok(())
    }

    pub fn create_terminal(&self, name: Option<String>, cwd: Option<String>) -> ServiceResult<u64> {
        self.service(ServiceRequest::CreateTerminal {
            name,
            shell_path: None,
            shell_args: Vec::new(),
            cwd,
        })
    }

    pub fn send_terminal_text(
        &self,
        id: u64,
        text: impl Into<String>,
        add_newline: bool,
    ) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::SendTerminalText {
            id,
            text: text.into(),
            add_newline,
        })?;
        Ok(())
    }

    pub fn scm_status(&self) -> ServiceResult<ScmStatusSnapshot> {
        self.service(ServiceRequest::ScmStatus)
    }

    pub fn storage_get(&self, scope: StorageScope, key: Option<String>) -> ServiceResult<Value> {
        self.service(ServiceRequest::StorageGet { scope, key })
    }

    pub fn storage_set(
        &self,
        scope: StorageScope,
        key: impl Into<String>,
        value: Option<Value>,
    ) -> ServiceResult<()> {
        let _: Value = self.service(ServiceRequest::StorageSet {
            scope,
            key: key.into(),
            value,
        })?;
        Ok(())
    }
}

fn dispatch_inbound_request(ctx: &PluginContext, request: RpcRequest) -> ServiceResult<Value> {
    match request {
        RpcRequest::Invoke { command, arguments } => {
            let handler = ctx
                .state
                .commands
                .lock()
                .ok()
                .and_then(|map| map.get(&command).cloned())
                .ok_or_else(|| ServiceError::Failed(format!("Unknown command: {command}")))?;
            handler(ctx, arguments)
        }
        RpcRequest::InvokeChatTool { tool_id, input } => {
            let handler = ctx
                .state
                .chat_tools
                .lock()
                .ok()
                .and_then(|map| map.get(&tool_id).cloned())
                .ok_or_else(|| ServiceError::Failed(format!("Unknown chat tool: {tool_id}")))?;
            let outcome = handler(ctx, input)?;
            serde_json::to_value(outcome).map_err(|e| ServiceError::Failed(e.to_string()))
        }
        RpcRequest::ProvideCompletions {
            provider_id,
            uri,
            position,
        } => {
            let handler = ctx
                .state
                .completions
                .lock()
                .ok()
                .and_then(|map| map.get(&provider_id).cloned())
                .ok_or(ServiceError::NotFound)?;
            let items = handler(ctx, &uri, position)?;
            serde_json::to_value(items).map_err(|e| ServiceError::Failed(e.to_string()))
        }
        RpcRequest::ProvideHover {
            provider_id,
            uri,
            position,
        } => {
            let handler = ctx
                .state
                .hovers
                .lock()
                .ok()
                .and_then(|map| map.get(&provider_id).cloned())
                .ok_or(ServiceError::NotFound)?;
            let hover = handler(ctx, &uri, position)?;
            serde_json::to_value(hover).map_err(|e| ServiceError::Failed(e.to_string()))
        }
        RpcRequest::ProvideDocumentFormatting {
            provider_id,
            uri,
            tab_size,
            insert_spaces,
        } => {
            let handler = ctx
                .state
                .formatters
                .lock()
                .ok()
                .and_then(|map| map.get(&provider_id).cloned())
                .ok_or(ServiceError::NotFound)?;
            let edits = handler(ctx, &uri, tab_size, insert_spaces)?;
            serde_json::to_value(edits).map_err(|e| ServiceError::Failed(e.to_string()))
        }
        RpcRequest::ParticipateInSave { resource, request } => {
            let handler = ctx
                .state
                .save_participants
                .lock()
                .ok()
                .and_then(|map| map.get(&resource).cloned())
                .ok_or(ServiceError::NotFound)?;
            let edits = handler(ctx, request.clone())?;
            Ok(json!({
                "invocation": request.invocation,
                "version": request.document.version,
                "edits": edits,
            }))
        }
        _ => Err(ServiceError::Failed("Unsupported inbound host RPC".into())),
    }
}

/// Runs `plugin` over standard input/output using Graphite's framed v2 plugin RPC protocol.
pub fn run_plugin<P: Plugin>(plugin: P) -> ServiceResult<()> {
    let plugin_root = std::env::args_os().nth(1).map(PathBuf::from);
    let stdin = Box::new(BufReader::new(std::io::stdin()));
    let stdout = Box::new(BufWriter::new(std::io::stdout()));
    run_plugin_over_streams(plugin, stdin, stdout, plugin_root)
}

/// Runs `plugin` over arbitrary streams (useful for standalone unit tests without spawning processes).
pub fn run_plugin_over_streams<P: Plugin>(
    plugin: P,
    mut reader: Box<dyn Read + Send>,
    mut writer: Box<dyn Write + Send>,
    plugin_root: Option<PathBuf>,
) -> ServiceResult<()> {
    let Some(Message::Hello { version, session }) =
        read_message(&mut reader).map_err(|e| ServiceError::Io(e.to_string()))?
    else {
        return Err(ServiceError::Protocol("Expected Hello handshake".into()));
    };
    if version != VERSION {
        return Err(ServiceError::Protocol(format!(
            "Unsupported RPC protocol version {version}"
        )));
    }
    write_message(
        &mut writer,
        &Message::Ready {
            version: VERSION,
            session,
        },
    )
    .map_err(|e| ServiceError::Io(e.to_string()))?;
    writer
        .flush()
        .map_err(|e| ServiceError::Io(e.to_string()))?;

    let state = Arc::new(SharedState {
        session,
        next_id: AtomicU64::new(0),
        next_counter: AtomicU64::new(0),
        shutting_down: AtomicBool::new(false),
        writer: Mutex::new(writer),
        pending: Mutex::new(HashMap::new()),
        cancelled: Mutex::new(HashSet::new()),
        commands: Mutex::new(HashMap::new()),
        chat_tools: Mutex::new(HashMap::new()),
        completions: Mutex::new(HashMap::new()),
        hovers: Mutex::new(HashMap::new()),
        formatters: Mutex::new(HashMap::new()),
        save_participants: Mutex::new(HashMap::new()),
        document_listeners: Mutex::new(HashMap::new()),
    });
    let ctx = PluginContext {
        state: state.clone(),
        plugin_root,
    };
    let plugin = Arc::new(plugin);

    // Activation runs concurrently with the reader loop so `activate()` can call host services immediately.
    let activate_ctx = ctx.clone();
    let activate_plugin = plugin.clone();
    let activation_handle = std::thread::Builder::new()
        .name("graphite-plugin-activate".into())
        .spawn(move || {
            if let Err(err) = activate_plugin.activate(&activate_ctx) {
                eprintln!("Plugin activation error: {err}");
                return;
            }
            let _ = activate_ctx.request(RpcRequest::Activated);
        })
        .map_err(|e| ServiceError::Io(e.to_string()))?;

    let mut worker_handles = Vec::new();
    while let Ok(Some(msg)) = read_message(&mut reader) {
        if msg.session() != session {
            continue;
        }
        match msg {
            Message::Response { id, result, .. } => {
                if let Ok(mut pending) = state.pending.lock() {
                    if let Some(sender) = pending.remove(&id) {
                        let _ = sender.send(result);
                    }
                }
            }
            Message::Cancel { id, .. } => {
                if let Ok(mut cancelled) = state.cancelled.lock() {
                    cancelled.insert(id);
                }
            }
            Message::Request { id, request, .. } => {
                let req_ctx = ctx.clone();
                let handle = std::thread::Builder::new()
                    .name(format!("graphite-plugin-rpc-{id}"))
                    .spawn(move || {
                        let is_cancelled = req_ctx
                            .state
                            .cancelled
                            .lock()
                            .is_ok_and(|mut c| c.remove(&id));
                        let result = if is_cancelled {
                            Err(ServiceError::Cancelled)
                        } else {
                            with_parent(id, || dispatch_inbound_request(&req_ctx, request))
                        };
                        let _ = req_ctx.write_frame(&Message::Response {
                            session: req_ctx.state.session,
                            id,
                            result,
                        });
                    });
                if let Ok(h) = handle {
                    worker_handles.push(h);
                }
            }
            Message::Event {
                resource, event, ..
            } => {
                if let Some(listener) = state
                    .document_listeners
                    .lock()
                    .ok()
                    .and_then(|map| map.get(&resource).cloned())
                {
                    let evt_ctx = ctx.clone();
                    let h = std::thread::spawn(move || listener(&evt_ctx, event));
                    worker_handles.push(h);
                }
            }
            Message::Shutdown { .. } => {
                state.shutting_down.store(true, Ordering::Relaxed);
                break;
            }
            Message::Hello { .. }
            | Message::Ready { .. }
            | Message::WaitingForUser { .. }
            | Message::Resumed { .. } => {}
        }
    }

    state.shutting_down.store(true, Ordering::Relaxed);
    if let Ok(mut pending) = state.pending.lock() {
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(ServiceError::HostStopped));
        }
    }
    plugin.deactivate();
    let _ = activation_handle.join();
    for handle in worker_handles {
        let _ = handle.join();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct EchoPlugin;

    impl Plugin for EchoPlugin {
        fn activate(&self, ctx: &PluginContext) -> ServiceResult<()> {
            ctx.register_command("local.echo.say", "Echo Say", |_ctx, args| {
                Ok(json!({ "echo": args }))
            })?;
            Ok(())
        }
    }

    struct PipeEnd {
        rx: mpsc::Receiver<Vec<u8>>,
        buf: VecDeque<u8>,
    }

    impl Read for PipeEnd {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            while self.buf.is_empty() {
                let Ok(chunk) = self.rx.recv() else {
                    return Ok(0);
                };
                self.buf.extend(chunk);
            }
            let n = out.len().min(self.buf.len());
            for byte in out.iter_mut().take(n) {
                *byte = self.buf.pop_front().unwrap();
            }
            Ok(n)
        }
    }

    struct PipeWriter(mpsc::Sender<Vec<u8>>);

    impl Write for PipeWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .send(buf.to_vec())
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))?;
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn pipe() -> (PipeWriter, PipeEnd) {
        let (tx, rx) = mpsc::channel();
        (
            PipeWriter(tx),
            PipeEnd {
                rx,
                buf: VecDeque::new(),
            },
        )
    }

    #[test]
    fn sdk_handshake_activates_and_handles_command_rpc() {
        let (mut host_tx, plugin_rx) = pipe();
        let (plugin_tx, mut host_rx) = pipe();

        let thread = std::thread::spawn(move || {
            run_plugin_over_streams(EchoPlugin, Box::new(plugin_rx), Box::new(plugin_tx), None)
        });

        write_message(
            &mut host_tx,
            &Message::Hello {
                version: VERSION,
                session: 42,
            },
        )
        .unwrap();
        assert!(matches!(
            read_message(&mut host_rx).unwrap(),
            Some(Message::Ready {
                version: VERSION,
                session: 42
            })
        ));

        let Some(Message::Request {
            id: reg_id,
            request: RpcRequest::RegisterCommand(meta),
            ..
        }) = read_message(&mut host_rx).unwrap()
        else {
            panic!("expected RegisterCommand");
        };
        assert_eq!(meta.id, "local.echo.say");
        write_message(
            &mut host_tx,
            &Message::Response {
                session: 42,
                id: reg_id,
                result: Ok(json!({ "resource": 10 })),
            },
        )
        .unwrap();

        let Some(Message::Request {
            id: act_id,
            request: RpcRequest::Activated,
            ..
        }) = read_message(&mut host_rx).unwrap()
        else {
            panic!("expected Activated");
        };
        write_message(
            &mut host_tx,
            &Message::Response {
                session: 42,
                id: act_id,
                result: Ok(Value::Null),
            },
        )
        .unwrap();

        write_message(
            &mut host_tx,
            &Message::Request {
                session: 42,
                id: 99,
                request: RpcRequest::Invoke {
                    command: "local.echo.say".into(),
                    arguments: json!({ "msg": "hi" }),
                },
            },
        )
        .unwrap();
        let Some(Message::Response {
            id: 99,
            result: Ok(val),
            ..
        }) = read_message(&mut host_rx).unwrap()
        else {
            panic!("expected Invoke response");
        };
        assert_eq!(val["echo"]["msg"], "hi");

        write_message(&mut host_tx, &Message::Shutdown { session: 42 }).unwrap();
        thread.join().unwrap().unwrap();
    }
}
