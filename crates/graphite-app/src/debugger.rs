//! DAP process framing and launch configuration parsing.
//!
//! The transport owns a single adapter process. It never blocks the workbench
//! thread: adapter output is framed on a reader thread and delivered through a
//! bounded channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread,
};

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const EVENT_CAPACITY: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub adapter_type: String,
    pub request: RequestKind,
    #[serde(default)]
    pub program: Option<PathBuf>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub stop_on_entry: bool,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Graphite's executable declaration for a local stdio adapter.
    #[serde(default)]
    pub adapter: Option<AdapterCommand>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestKind {
    Launch,
    Attach,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AdapterCommand {
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DebugError {
    #[error("could not read launch configuration: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid launch configuration: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no named debug configurations were found")]
    NoConfigurations,
    #[error("debug adapter executable is not configured for '{0}'")]
    MissingAdapter(String),
    #[error("debug adapter is already stopped")]
    Stopped,
    #[error("debug adapter transport failed: {0}")]
    Transport(String),
}

pub fn load_launch_configs(workspace: &Path) -> Result<Vec<LaunchConfig>, DebugError> {
    let path = workspace.join(".vscode").join("launch.json");
    let source = std::fs::read_to_string(path)?;
    let document = crate::config::parse_jsonc(&source)?;
    let values = document["configurations"]
        .as_array()
        .ok_or(DebugError::NoConfigurations)?;
    let configs = values
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<Vec<LaunchConfig>, _>>()?;
    if configs.is_empty() {
        return Err(DebugError::NoConfigurations);
    }
    Ok(configs)
}

#[derive(Debug)]
pub enum AdapterEvent {
    Message(Value),
    Closed,
    Failed(String),
}

pub struct DapSession {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Receiver<AdapterEvent>,
    sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugState {
    Stopped,
    Initializing,
    Running,
    Paused { thread_id: i64, reason: String },
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugOutput {
    pub category: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    pub source: Option<PathBuf>,
    pub line: u32,
}

#[derive(Default)]
pub struct DebugRuntime {
    pub session: Option<DapSession>,
    pub state: DebugState,
    pub output: Vec<DebugOutput>,
    pub frames: Vec<StackFrame>,
    pub variables: Vec<(String, String, Option<i64>)>,
    pub watches: Vec<String>,
    pub breakpoints: BTreeMap<PathBuf, Vec<Value>>,
    pub launch_configs: Vec<LaunchConfig>,
    pub active_config: Option<LaunchConfig>,
    initialize_seq: Option<i64>,
    launch_seq: Option<i64>,
    stack_seq: Option<i64>,
    scopes_seq: Option<i64>,
    variables_seq: Option<i64>,
    pending_breakpoints: BTreeMap<i64, PathBuf>,
    selected_frame: Option<i64>,
}

impl Default for DebugState {
    fn default() -> Self {
        Self::Stopped
    }
}

impl DebugRuntime {
    pub fn refresh_configs(&mut self, workspace: &Path) -> Result<(), DebugError> {
        self.launch_configs = load_launch_configs(workspace)?;
        Ok(())
    }

    pub fn start(&mut self, config: LaunchConfig, workspace: &Path) -> Result<(), DebugError> {
        self.stop()?;
        let mut session = DapSession::spawn(&config, workspace)?;
        let seq = session.send_request(
            "initialize",
            serde_json::json!({
                "clientID":"graphite",
                "clientName":"Graphite",
                "adapterID":config.adapter_type,
                "pathFormat":"path",
                "linesStartAt1":true,
                "columnsStartAt1":true,
                "supportsVariableType":true,
                "supportsVariablePaging":true,
                "supportsRunInTerminalRequest":false,
                "supportsProgressReporting":true,
            }),
        )?;
        self.initialize_seq = Some(seq);
        self.launch_seq = None;
        self.stack_seq = None;
        self.scopes_seq = None;
        self.variables_seq = None;
        self.pending_breakpoints.clear();
        self.frames.clear();
        self.variables.clear();
        self.output.clear();
        self.active_config = Some(config);
        self.state = DebugState::Initializing;
        self.session = Some(session);
        Ok(())
    }

    pub fn poll(&mut self, workspace: &Path) -> Result<bool, DebugError> {
        let mut redraw = false;
        loop {
            let Some(session) = self.session.as_ref() else {
                break;
            };
            let Some(event) = session.try_event()? else {
                break;
            };
            redraw = true;
            match event {
                AdapterEvent::Closed => {
                    self.session = None;
                    self.state = DebugState::Stopped;
                    break;
                }
                AdapterEvent::Failed(error) => {
                    self.state = DebugState::Failed(error.clone());
                    self.push_output("stderr", error);
                    self.session = None;
                    break;
                }
                AdapterEvent::Message(message) => self.handle_message(message, workspace)?,
            }
        }
        if self
            .session
            .as_mut()
            .is_some_and(|session| !session.is_running().unwrap_or(false))
        {
            self.session = None;
            self.state = DebugState::Stopped;
            redraw = true;
        }
        Ok(redraw)
    }

    pub fn stop(&mut self) -> Result<(), DebugError> {
        if let Some(mut session) = self.session.take() {
            session.stop()?;
        }
        self.state = DebugState::Stopped;
        self.active_config = None;
        self.frames.clear();
        self.variables.clear();
        Ok(())
    }

    pub fn request(&mut self, command: &str, arguments: Value) -> Result<(), DebugError> {
        self.session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request(command, arguments)?;
        Ok(())
    }

    pub fn continue_execution(&mut self) -> Result<(), DebugError> {
        let thread_id = match self.state {
            DebugState::Paused { thread_id, .. } => thread_id,
            _ => 0,
        };
        self.request("continue", serde_json::json!({"threadId":thread_id}))?;
        self.state = DebugState::Running;
        Ok(())
    }

    pub fn step_over(&mut self) -> Result<(), DebugError> {
        self.step("next")
    }

    pub fn step_into(&mut self) -> Result<(), DebugError> {
        self.step("stepIn")
    }

    pub fn step_out(&mut self) -> Result<(), DebugError> {
        self.step("stepOut")
    }

    pub fn restart(&mut self) -> Result<(), DebugError> {
        self.request("restart", serde_json::json!({}))?;
        self.state = DebugState::Running;
        Ok(())
    }

    pub fn set_breakpoints(
        &mut self,
        source: &Path,
        breakpoints: Vec<Value>,
    ) -> Result<(), DebugError> {
        self.breakpoints
            .insert(source.to_path_buf(), breakpoints.clone());
        let seq = self
            .session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request(
                "setBreakpoints",
                serde_json::json!({
                    "source":{"path":source},
                    "breakpoints":breakpoints,
                    "sourceModified":false,
                }),
            )?;
        self.pending_breakpoints.insert(seq, source.to_path_buf());
        Ok(())
    }

    pub fn request_stack(&mut self, thread_id: i64) -> Result<(), DebugError> {
        self.stack_seq = Some(
            self.session
                .as_mut()
                .ok_or(DebugError::Stopped)?
                .send_request(
                    "stackTrace",
                    serde_json::json!({"threadId":thread_id,"startFrame":0,"levels":50}),
                )?,
        );
        Ok(())
    }

    pub fn request_scopes(&mut self, frame_id: i64) -> Result<(), DebugError> {
        self.selected_frame = Some(frame_id);
        self.scopes_seq = Some(
            self.session
                .as_mut()
                .ok_or(DebugError::Stopped)?
                .send_request("scopes", serde_json::json!({"frameId":frame_id}))?,
        );
        Ok(())
    }

    pub fn evaluate_watch(
        &mut self,
        expression: &str,
        frame_id: Option<i64>,
    ) -> Result<(), DebugError> {
        self.session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request(
                "evaluate",
                serde_json::json!({"expression":expression,"frameId":frame_id,"context":"watch"}),
            )?;
        Ok(())
    }

    fn step(&mut self, command: &str) -> Result<(), DebugError> {
        let thread_id = match self.state {
            DebugState::Paused { thread_id, .. } => thread_id,
            _ => return Err(DebugError::Stopped),
        };
        self.request(command, serde_json::json!({"threadId":thread_id}))?;
        self.state = DebugState::Running;
        Ok(())
    }

    fn handle_message(&mut self, message: Value, workspace: &Path) -> Result<(), DebugError> {
        match message["type"].as_str() {
            Some("event") => self.handle_event(&message, workspace),
            Some("response") => self.handle_response(&message, workspace),
            Some("request") => {
                let seq = message["seq"].as_i64().unwrap_or_default();
                let command = message["command"].as_str().unwrap_or_default();
                self.session
                    .as_mut()
                    .ok_or(DebugError::Stopped)?
                    .send_response(
                        seq,
                        command,
                        false,
                        Value::Null,
                        Some("Graphite does not permit debug adapter UI requests"),
                    )?;
                Ok(())
            }
            _ => Err(DebugError::Transport("unknown DAP message type".into())),
        }
    }

    fn handle_response(&mut self, message: &Value, workspace: &Path) -> Result<(), DebugError> {
        let seq = message["request_seq"].as_i64().unwrap_or_default();
        let success = message["success"].as_bool().unwrap_or(false);
        if self.initialize_seq == Some(seq) {
            self.initialize_seq = None;
            if !success {
                return Err(DebugError::Transport(dap_error(message)));
            }
            let config = self.active_config.as_ref().ok_or(DebugError::Stopped)?;
            let mut arguments = serde_json::to_value(config)?;
            arguments.as_object_mut().unwrap().remove("adapter");
            if let Some(program) = config.program.as_ref() {
                arguments["program"] = serde_json::json!(absolute(workspace, program));
            }
            arguments["cwd"] = serde_json::json!(config
                .cwd
                .as_ref()
                .map(|cwd| absolute(workspace, cwd))
                .unwrap_or_else(|| workspace.to_path_buf()));
            if let Some(env) = arguments["env"].as_object_mut() {
                for value in env.values_mut() {
                    if let Some(path) = value.as_str() {
                        *value = serde_json::json!(expand_variables(path, workspace));
                    }
                }
            }
            self.launch_seq = Some(
                self.session
                    .as_mut()
                    .ok_or(DebugError::Stopped)?
                    .send_request(
                        match config.request {
                            RequestKind::Launch => "launch",
                            RequestKind::Attach => "attach",
                        },
                        arguments,
                    )?,
            );
        } else if self.launch_seq == Some(seq) {
            self.launch_seq = None;
            if !success {
                return Err(DebugError::Transport(dap_error(message)));
            }
            self.state = DebugState::Running;
        } else if self.stack_seq == Some(seq) {
            self.stack_seq = None;
            self.frames = message["body"]["stackFrames"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|frame| StackFrame {
                    id: frame["id"].as_i64().unwrap_or_default(),
                    name: frame["name"].as_str().unwrap_or("<frame>").to_owned(),
                    source: frame["source"]["path"].as_str().map(PathBuf::from),
                    line: frame["line"].as_u64().unwrap_or_default() as u32,
                })
                .collect();
            if let Some(frame) = self.frames.first() {
                self.request_scopes(frame.id)?;
            }
        } else if self.scopes_seq == Some(seq) {
            self.scopes_seq = None;
            if let Some(reference) = message["body"]["scopes"]
                .as_array()
                .and_then(|scopes| scopes.first())
                .and_then(|scope| scope["variablesReference"].as_i64())
            {
                self.variables_seq = Some(
                    self.session
                        .as_mut()
                        .ok_or(DebugError::Stopped)?
                        .send_request(
                            "variables",
                            serde_json::json!({"variablesReference":reference}),
                        )?,
                );
            }
        } else if self.variables_seq == Some(seq) {
            self.variables_seq = None;
            self.variables = message["body"]["variables"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|variable| {
                    (
                        variable["name"].as_str().unwrap_or_default().to_owned(),
                        variable["value"].as_str().unwrap_or_default().to_owned(),
                        variable["variablesReference"]
                            .as_i64()
                            .filter(|reference| *reference > 0),
                    )
                })
                .collect();
        } else if self.pending_breakpoints.remove(&seq).is_some() && !success {
            self.push_output("stderr", dap_error(message));
        } else if !success {
            self.push_output("stderr", dap_error(message));
        }
        if let Some(body) = message.get("body").filter(|_| success) {
            if message["command"] == "evaluate" {
                let expression = body["result"].as_str().unwrap_or("");
                self.push_output("console", expression.to_owned());
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, message: &Value, workspace: &Path) -> Result<(), DebugError> {
        match message["event"].as_str().unwrap_or_default() {
            "initialized" => {
                let breakpoints = self.breakpoints.clone();
                for (source, points) in breakpoints {
                    self.set_breakpoints(&source, points)?;
                }
                self.session
                    .as_mut()
                    .ok_or(DebugError::Stopped)?
                    .send_request("configurationDone", serde_json::json!({}))?;
            }
            "stopped" => {
                let body = &message["body"];
                let thread_id = body["threadId"].as_i64().unwrap_or_default();
                self.state = DebugState::Paused {
                    thread_id,
                    reason: body["reason"].as_str().unwrap_or("paused").to_owned(),
                };
                self.request_stack(thread_id)?;
            }
            "continued" => self.state = DebugState::Running,
            "terminated" | "exited" => self.state = DebugState::Stopped,
            "output" => {
                let body = &message["body"];
                self.push_output(
                    body["category"].as_str().unwrap_or("console"),
                    body["output"].as_str().unwrap_or_default().to_owned(),
                );
            }
            "breakpoint" => self.push_output("console", "Breakpoint status changed".into()),
            _ => {}
        }
        let _ = workspace;
        Ok(())
    }

    fn push_output(&mut self, category: &str, text: String) {
        self.output.push(DebugOutput {
            category: category.to_owned(),
            text,
        });
        if self.output.len() > 2000 {
            self.output.drain(..self.output.len() - 2000);
        }
    }
}

fn dap_error(message: &Value) -> String {
    message["message"]
        .as_str()
        .or_else(|| message["body"]["error"]["format"].as_str())
        .unwrap_or("Debug adapter request failed")
        .to_owned()
}

fn absolute(workspace: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
}

fn expand_variables(value: &str, workspace: &Path) -> String {
    value.replace("${workspaceFolder}", &workspace.display().to_string())
}

impl DapSession {
    pub fn spawn(config: &LaunchConfig, cwd: &Path) -> Result<Self, DebugError> {
        let adapter = config
            .adapter
            .as_ref()
            .ok_or_else(|| DebugError::MissingAdapter(config.adapter_type.clone()))?;
        let mut child = Command::new(&adapter.command)
            .args(&adapter.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or(DebugError::Stopped)?;
        let stderr = child.stderr.take().ok_or(DebugError::Stopped)?;
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        thread::Builder::new()
            .name("graphite-dap-reader".into())
            .spawn(move || read_messages(stdout, sender))?;
        let (stderr_sender, _stderr_receiver) = mpsc::sync_channel::<String>(64);
        thread::Builder::new()
            .name("graphite-dap-stderr".into())
            .spawn(move || read_stderr(stderr, stderr_sender))?;
        Ok(Self {
            child,
            stdin,
            events,
            sequence: 1,
        })
    }

    pub fn send_request(&mut self, command: &str, arguments: Value) -> Result<i64, DebugError> {
        let seq = self.sequence;
        self.sequence += 1;
        self.send_message(serde_json::json!({
            "seq": seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        }))?;
        Ok(seq)
    }

    pub fn send_response(
        &mut self,
        request_seq: i64,
        command: &str,
        success: bool,
        body: Value,
        message: Option<&str>,
    ) -> Result<i64, DebugError> {
        let seq = self.sequence;
        self.sequence += 1;
        let mut response = serde_json::json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "command": command,
            "success": success,
            "body": body,
        });
        if let Some(message) = message {
            response["message"] = Value::String(message.to_owned());
        }
        self.send_message(response)?;
        Ok(seq)
    }

    pub fn try_event(&self) -> Result<Option<AdapterEvent>, DebugError> {
        match self.events.try_recv() {
            Ok(AdapterEvent::Failed(error)) => Err(DebugError::Transport(error)),
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Ok(Some(AdapterEvent::Closed)),
        }
    }

    pub fn is_running(&mut self) -> Result<bool, DebugError> {
        Ok(self.child.try_wait()?.is_none())
    }

    pub fn stop(&mut self) -> Result<(), DebugError> {
        self.stdin.take();
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
        }
        let _ = self.child.wait();
        Ok(())
    }

    fn send_message(&mut self, value: Value) -> Result<(), DebugError> {
        let stdin = self.stdin.as_mut().ok_or(DebugError::Stopped)?;
        let payload = serde_json::to_vec(&value)?;
        write!(stdin, "Content-Length: {}\r\n\r\n", payload.len())?;
        stdin.write_all(&payload)?;
        stdin.flush()?;
        Ok(())
    }
}

impl Drop for DapSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn read_messages(stdout: impl Read, sender: SyncSender<AdapterEvent>) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => {
                if sender.send(AdapterEvent::Message(message)).is_err() {
                    break;
                }
            }
            Ok(None) => {
                let _ = sender.send(AdapterEvent::Closed);
                break;
            }
            Err(error) => {
                let _ = sender.send(AdapterEvent::Failed(error.to_string()));
                break;
            }
        }
    }
}

fn read_stderr(stderr: impl Read, sender: SyncSender<String>) {
    let reader = BufReader::new(stderr);
    for line in reader.lines().map_while(Result::ok) {
        let _ = sender.try_send(line);
    }
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, DebugError> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = String::new();
        let count = reader.read_line(&mut line)?;
        if count == 0 {
            return Ok(None);
        }
        header_bytes += count;
        if header_bytes > MAX_HEADER_BYTES {
            return Err(DebugError::Transport("DAP header is too large".into()));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.trim().split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| DebugError::Transport("invalid DAP Content-Length".into()))?,
                );
            }
        }
    }
    let length =
        length.ok_or_else(|| DebugError::Transport("missing DAP Content-Length".into()))?;
    if length > MAX_MESSAGE_BYTES {
        return Err(DebugError::Transport("DAP message is too large".into()));
    }
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    Ok(Some(serde_json::from_slice(&payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_jsonc_launch_profiles_and_defaults() {
        let root = crate::test_support::TempDir::new();
        std::fs::create_dir_all(root.path().join(".vscode")).unwrap();
        std::fs::write(root.path().join(".vscode/launch.json"), r#"{
          // A workspace launch profile
          "version": "0.2.0",
          "configurations": [{"name":"Debug app","type":"example","request":"launch", "adapter":{"command":"adapter"}, "args":["a"]}],
        }"#).unwrap();
        let configs = load_launch_configs(root.path()).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "Debug app");
        assert_eq!(configs[0].request, RequestKind::Launch);
        assert_eq!(configs[0].args, ["a"]);
        assert!(!configs[0].stop_on_entry);
    }

    #[test]
    fn frames_messages_and_rejects_invalid_or_oversized_lengths() {
        let payload = br#"{"type":"event","seq":1}"#;
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        let mut bytes = header.into_bytes();
        bytes.extend_from_slice(payload);
        assert_eq!(
            read_message(&mut Cursor::new(bytes)).unwrap().unwrap()["seq"],
            1
        );
        assert!(read_message(&mut Cursor::new(b"Content-Length: nope\r\n\r\n")).is_err());
        let large = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
        assert!(read_message(&mut Cursor::new(large.into_bytes())).is_err());
    }

    #[test]
    fn serializes_requests_using_content_length() {
        let mut bytes = Vec::new();
        let payload = serde_json::to_vec(
            &serde_json::json!({"seq":1,"type":"request","command":"continue","arguments":{}}),
        )
        .unwrap();
        write!(bytes, "Content-Length: {}\r\n\r\n", payload.len()).unwrap();
        bytes.extend_from_slice(&payload);
        let mut reader = BufReader::new(Cursor::new(bytes));
        assert_eq!(
            read_message(&mut reader).unwrap().unwrap()["command"],
            "continue"
        );
    }
}
