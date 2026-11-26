//! DAP process framing and launch configuration parsing.
//!
//! The transport owns a single adapter process. It never blocks the workbench
//! thread: adapter output is framed on a reader thread and delivered through a
//! bounded channel.

use crate::process_util::HideConsole;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread,
    time::Duration,
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
    /// Label of a `tasks.json` task that must succeed before the session starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_launch_task: Option<String>,
    /// Label of a `tasks.json` task that runs once the session has ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_debug_task: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Graphite's executable declaration for a local stdio adapter.
    #[serde(default)]
    pub adapter: Option<AdapterCommand>,
    /// A listening adapter chosen at launch time (not part of `launch.json`).
    #[serde(skip)]
    pub server: Option<AdapterServer>,
    /// A named pipe (Windows) or local socket (Unix) an adapter listens on, chosen at launch time.
    #[serde(skip)]
    pub pipe: Option<PathBuf>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestKind {
    Launch,
    Attach,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AdapterCommand {
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory for the adapter; defaults to the workspace folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    /// Extra environment variables for the adapter process.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

/// A debug adapter that is already listening on a local TCP port.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AdapterServer {
    pub host: String,
    pub port: u16,
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
    child: Option<Child>,
    stdin: Option<Box<dyn Write + Send>>,
    /// The connection of a TCP session, kept to shut it down on stop.
    socket: Option<Arc<TcpStream>>,
    /// Ends the connection of a pipe session, which wakes its reader thread.
    closer: Option<Box<dyn Fn() + Send>>,
    events: Receiver<AdapterEvent>,
    sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum DebugState {
    #[default]
    Stopped,
    Initializing,
    Running,
    Paused {
        thread_id: i64,
        reason: String,
    },
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub name: String,
    pub reference: i64,
    pub expensive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub type_name: Option<String>,
    /// Zero when the variable has no children.
    pub reference: i64,
}

/// One visible line of the variables tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableRow {
    pub depth: usize,
    pub name: String,
    pub value: String,
    pub reference: i64,
    pub expanded: bool,
    pub is_scope: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchExpression {
    pub expression: String,
    pub result: Option<Result<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExceptionFilter {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingEvaluation {
    Watch(String),
    Repl,
    Hover(String),
}

const MAX_VARIABLE_DEPTH: usize = 16;

/// Marks a breakpoint that is kept in the list but not sent to the adapter.
pub const DISABLED_KEY: &str = "graphiteDisabled";

/// Whether a breakpoint entry has been turned off one by one.
pub fn is_disabled(point: &Value) -> bool {
    point[DISABLED_KEY].as_bool().unwrap_or(false)
}

/// One DAP data breakpoint (watchpoint on a memory address or variable `dataId`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataBreakpoint {
    pub data_id: String,
    pub access_type: Option<String>,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
}

#[derive(Default)]
pub struct DebugRuntime {
    pub session: Option<DapSession>,
    pub state: DebugState,
    pub output: Vec<DebugOutput>,
    pub frames: Vec<StackFrame>,
    pub selected_frame: usize,
    /// Bumped whenever the stack, scopes or variables change, so views can tell they are stale.
    pub version: u64,
    /// The file open in the editor when the session started, for `${file}`.
    pub active_file: Option<PathBuf>,
    pub scopes: Vec<Scope>,
    pub watches: Vec<WatchExpression>,
    pub exception_filters: Vec<ExceptionFilter>,
    pub breakpoints: BTreeMap<PathBuf, Vec<Value>>,
    pub breakpoints_disabled: bool,
    pub launch_configs: Vec<LaunchConfig>,
    /// Configurations extensions offer (dynamic configuration providers), listed after those of
    /// `launch.json`.
    pub extension_configs: Vec<LaunchConfig>,
    pub active_config: Option<LaunchConfig>,
    /// Name of the configuration chosen for the next start; the first one is used when unset.
    pub selected_config: Option<String>,
    children: BTreeMap<i64, Vec<Variable>>,
    expanded: BTreeSet<i64>,
    initialize_seq: Option<i64>,
    launch_seq: Option<i64>,
    stack_seq: Option<i64>,
    scopes_seq: Option<i64>,
    pending_variables: BTreeMap<i64, i64>,
    pending_evaluations: BTreeMap<i64, PendingEvaluation>,
    pending_breakpoints: BTreeMap<i64, PathBuf>,
    /// Names of the function breakpoints, sent with `setFunctionBreakpoints`.
    pub function_breakpoints: Vec<String>,
    /// Data breakpoints sent with `setDataBreakpoints`.
    pub data_breakpoints: Vec<DataBreakpoint>,
    /// Only console lines that contain this text are shown (empty shows everything).
    pub console_filter: String,
    /// Most recent `evaluate(..., "hover")` result ready to show in the editor hover popup.
    pub last_hover_result: Option<String>,
    /// `setVariable` requests in flight: the variable's parent reference and its name.
    pending_set_variables: BTreeMap<i64, (i64, String)>,
    /// `postDebugTask` of the running session, held until the session ends.
    post_debug_task: Option<String>,
    /// A `postDebugTask` whose session has ended and which has not been run yet.
    ready_post_task: Option<String>,
}

impl DebugRuntime {
    pub fn refresh_configs(&mut self, workspace: &Path) -> Result<(), DebugError> {
        let loaded = match load_launch_configs(workspace) {
            Ok(configs) => configs,
            // Without a usable `launch.json` the extensions' configurations are all there is.
            Err(DebugError::NoConfigurations | DebugError::Read(_))
                if !self.extension_configs.is_empty() =>
            {
                Vec::new()
            }
            Err(error) => return Err(error),
        };
        let mut configs = loaded;
        for config in &self.extension_configs {
            if !configs.iter().any(|existing| existing.name == config.name) {
                configs.push(config.clone());
            }
        }
        self.launch_configs = configs;
        Ok(())
    }

    /// The configuration F5 starts: the selected one if it still exists, otherwise the first.
    pub fn launch_config(&self) -> Option<&LaunchConfig> {
        self.selected_config
            .as_deref()
            .and_then(|name| {
                self.launch_configs
                    .iter()
                    .find(|config| config.name == name)
            })
            .or_else(|| self.launch_configs.first())
    }

    /// Chooses the configuration called `name`. Returns false when there is none.
    pub fn select_config(&mut self, name: &str) -> bool {
        let known = self.launch_configs.iter().any(|config| config.name == name);
        if known {
            self.selected_config = Some(name.to_owned());
        }
        known
    }

    pub fn start(&mut self, config: LaunchConfig, workspace: &Path) -> Result<(), DebugError> {
        self.stop()?;
        let session = DapSession::spawn(&config, workspace)?;
        self.start_with_session(config, session)
    }

    /// Starts debugging over a session that is already connected (an adapter that runs inside
    /// the extension host).
    pub fn start_with_session(
        &mut self,
        config: LaunchConfig,
        mut session: DapSession,
    ) -> Result<(), DebugError> {
        self.stop()?;
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
        self.pending_breakpoints.clear();
        self.pending_set_variables.clear();
        self.pending_evaluations.clear();
        self.clear_stop_state();
        self.output.clear();
        self.post_debug_task = config.post_debug_task.clone();
        self.active_config = Some(config);
        self.state = DebugState::Initializing;
        self.session = Some(session);
        Ok(())
    }

    /// Marks the session as over: its `postDebugTask`, if any, becomes ready to run once.
    fn end_session(&mut self) {
        if let Some(task) = self.post_debug_task.take() {
            self.ready_post_task = Some(task);
        }
    }

    #[cfg(test)]
    pub(crate) fn queue_post_debug_task_for_test(&mut self, label: &str) {
        self.ready_post_task = Some(label.to_owned());
    }

    /// The `postDebugTask` of a session that has just ended, handed out exactly once.
    pub fn take_post_debug_task(&mut self) -> Option<String> {
        self.ready_post_task.take()
    }

    pub fn poll(&mut self, workspace: &Path) -> Result<bool, DebugError> {
        let mut redraw = false;
        while let Some(session) = self.session.as_ref() {
            let Some(event) = session.try_event()? else {
                break;
            };
            redraw = true;
            match event {
                AdapterEvent::Closed => {
                    self.session = None;
                    self.state = DebugState::Stopped;
                    self.end_session();
                    break;
                }
                AdapterEvent::Failed(error) => {
                    self.state = DebugState::Failed(error.clone());
                    self.push_output("stderr", error);
                    self.session = None;
                    self.end_session();
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
            self.end_session();
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
        self.clear_stop_state();
        self.end_session();
        Ok(())
    }

    /// Drops everything that only describes the paused program.
    fn clear_stop_state(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.frames.clear();
        self.selected_frame = 0;
        self.scopes.clear();
        self.children.clear();
        self.expanded.clear();
        self.pending_variables.clear();
        self.stack_seq = None;
        self.scopes_seq = None;
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
        self.clear_stop_state();
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
        self.clear_stop_state();
        Ok(())
    }

    /// Collapses every expanded variable and scope in the variables tree.
    pub fn collapse_variables(&mut self) {
        self.expanded.clear();
    }

    pub fn set_breakpoints(
        &mut self,
        source: &Path,
        breakpoints: Vec<Value>,
    ) -> Result<(), DebugError> {
        self.breakpoints
            .insert(source.to_path_buf(), breakpoints.clone());
        let breakpoints: Vec<Value> = if self.breakpoints_disabled {
            Vec::new()
        } else {
            breakpoints
                .into_iter()
                .filter(|point| !is_disabled(point))
                .map(|mut point| {
                    if let Some(object) = point.as_object_mut() {
                        object.remove(DISABLED_KEY);
                    }
                    point
                })
                .collect()
        };
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

    /// Adds a function breakpoint (ignored when the name is empty or already there) and sends
    /// the list to a running session.
    pub fn add_function_breakpoint(&mut self, name: &str) -> Result<(), DebugError> {
        let name = name.trim();
        if name.is_empty() || self.function_breakpoints.iter().any(|known| known == name) {
            return Ok(());
        }
        self.function_breakpoints.push(name.to_owned());
        self.send_function_breakpoints()
    }

    #[cfg(test)]
    pub fn remove_function_breakpoints(&mut self) -> Result<(), DebugError> {
        self.function_breakpoints.clear();
        self.send_function_breakpoints()
    }

    /// Sends the function breakpoints; does nothing without a session.
    pub fn send_function_breakpoints(&mut self) -> Result<(), DebugError> {
        let Some(session) = self.session.as_mut() else {
            return Ok(());
        };
        let breakpoints: Vec<Value> = if self.breakpoints_disabled {
            Vec::new()
        } else {
            self.function_breakpoints
                .iter()
                .map(|name| serde_json::json!({"name": name}))
                .collect()
        };
        session.send_request(
            "setFunctionBreakpoints",
            serde_json::json!({"breakpoints": breakpoints}),
        )?;
        Ok(())
    }

    /// Adds or updates a data breakpoint (`dataId`) and syncs `setDataBreakpoints`.
    pub fn add_data_breakpoint(
        &mut self,
        data_id: &str,
        access_type: Option<&str>,
        condition: Option<&str>,
    ) -> Result<(), DebugError> {
        let data_id = data_id.trim();
        if data_id.is_empty() {
            return Ok(());
        }
        let entry = DataBreakpoint {
            data_id: data_id.to_owned(),
            access_type: access_type.map(str::to_owned),
            condition: condition.map(str::to_owned),
            hit_condition: None,
        };
        if let Some(existing) = self
            .data_breakpoints
            .iter_mut()
            .find(|bp| bp.data_id == data_id)
        {
            *existing = entry;
        } else {
            self.data_breakpoints.push(entry);
        }
        self.send_data_breakpoints()
    }

    pub fn remove_data_breakpoints(&mut self) -> Result<(), DebugError> {
        self.data_breakpoints.clear();
        self.send_data_breakpoints()
    }

    /// Sends the active data breakpoints; does nothing without a session.
    pub fn send_data_breakpoints(&mut self) -> Result<(), DebugError> {
        let Some(session) = self.session.as_mut() else {
            return Ok(());
        };
        let breakpoints: Vec<Value> = if self.breakpoints_disabled {
            Vec::new()
        } else {
            self.data_breakpoints
                .iter()
                .map(|bp| {
                    let mut obj = serde_json::json!({ "dataId": bp.data_id });
                    if let Some(access) = &bp.access_type {
                        obj["accessType"] = serde_json::json!(access);
                    }
                    if let Some(cond) = &bp.condition {
                        obj["condition"] = serde_json::json!(cond);
                    }
                    if let Some(hit) = &bp.hit_condition {
                        obj["hitCondition"] = serde_json::json!(hit);
                    }
                    obj
                })
                .collect()
        };
        session.send_request(
            "setDataBreakpoints",
            serde_json::json!({"breakpoints": breakpoints}),
        )?;
        Ok(())
    }

    /// Changes the value of a loaded variable named `name`, searching the lists of variables
    /// that have been fetched. The new value shows when the adapter answers.
    pub fn set_variable(&mut self, name: &str, value: &str) -> Result<(), DebugError> {
        let reference = self
            .children
            .iter()
            .find(|(_, variables)| variables.iter().any(|variable| variable.name == name))
            .map(|(reference, _)| *reference)
            .ok_or_else(|| DebugError::Transport(format!("No variable named {name} is loaded")))?;
        let seq = self
            .session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request(
                "setVariable",
                serde_json::json!({"variablesReference": reference, "name": name, "value": value}),
            )?;
        self.pending_set_variables
            .insert(seq, (reference, name.to_owned()));
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
        self.scopes_seq = Some(
            self.session
                .as_mut()
                .ok_or(DebugError::Stopped)?
                .send_request("scopes", serde_json::json!({"frameId":frame_id}))?,
        );
        Ok(())
    }

    pub fn selected_frame_id(&self) -> Option<i64> {
        self.frames.get(self.selected_frame).map(|frame| frame.id)
    }

    /// Makes `index` the frame whose scopes, variables and watches are shown.
    pub fn select_frame(&mut self, index: usize) -> Result<(), DebugError> {
        let Some(frame) = self.frames.get(index) else {
            return Ok(());
        };
        let frame_id = frame.id;
        self.selected_frame = index;
        self.version = self.version.wrapping_add(1);
        self.scopes.clear();
        self.children.clear();
        self.expanded.clear();
        self.pending_variables.clear();
        self.request_scopes(frame_id)?;
        self.refresh_watches()
    }

    /// Expands or collapses a variable, fetching its children on first expansion.
    pub fn toggle_variable(&mut self, reference: i64) -> Result<(), DebugError> {
        if reference <= 0 {
            return Ok(());
        }
        if !self.expanded.remove(&reference) {
            self.expanded.insert(reference);
            if !self.children.contains_key(&reference) {
                self.request_variables(reference)?;
            }
        }
        Ok(())
    }

    fn request_variables(&mut self, reference: i64) -> Result<(), DebugError> {
        if self.pending_variables.values().any(|r| *r == reference) {
            return Ok(());
        }
        let seq = self
            .session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request(
                "variables",
                serde_json::json!({"variablesReference":reference}),
            )?;
        self.pending_variables.insert(seq, reference);
        Ok(())
    }

    /// `(name, value)` of the variables directly inside the cheap scopes (normally the locals) of
    /// the selected frame, as far as they have been fetched. Used for inline values.
    pub fn local_variables(&self) -> Vec<(String, String)> {
        self.scopes
            .iter()
            .filter(|scope| !scope.expensive)
            .filter_map(|scope| self.children.get(&scope.reference))
            .flatten()
            .map(|variable| (variable.name.clone(), variable.value.clone()))
            .collect()
    }

    /// Gives the selected frame these local variables, as if the adapter had sent them.
    #[cfg(test)]
    pub(crate) fn set_locals_for_test(&mut self, locals: &[(&str, &str)]) {
        self.scopes = vec![Scope {
            name: "Locals".into(),
            reference: 1,
            expensive: false,
        }];
        self.children.insert(
            1,
            locals
                .iter()
                .map(|(name, value)| Variable {
                    name: (*name).into(),
                    value: (*value).into(),
                    type_name: None,
                    reference: 0,
                })
                .collect(),
        );
        self.version = self.version.wrapping_add(1);
    }

    /// The visible rows of the variables tree: scopes, then expanded children.
    pub fn variable_rows(&self) -> Vec<VariableRow> {
        fn visit(
            runtime: &DebugRuntime,
            reference: i64,
            depth: usize,
            path: &mut Vec<i64>,
            rows: &mut Vec<VariableRow>,
        ) {
            if depth > MAX_VARIABLE_DEPTH {
                return;
            }
            for variable in runtime.children.get(&reference).into_iter().flatten() {
                let expanded = variable.reference > 0
                    && runtime.expanded.contains(&variable.reference)
                    && !path.contains(&variable.reference);
                rows.push(VariableRow {
                    depth,
                    name: variable.name.clone(),
                    value: variable.value.clone(),
                    reference: variable.reference,
                    expanded,
                    is_scope: false,
                });
                if expanded {
                    path.push(variable.reference);
                    visit(runtime, variable.reference, depth + 1, path, rows);
                    path.pop();
                }
            }
        }
        let mut rows = Vec::new();
        for scope in &self.scopes {
            let expanded = self.expanded.contains(&scope.reference);
            rows.push(VariableRow {
                depth: 0,
                name: scope.name.clone(),
                value: String::new(),
                reference: scope.reference,
                expanded,
                is_scope: true,
            });
            if expanded {
                visit(
                    self,
                    scope.reference,
                    1,
                    &mut vec![scope.reference],
                    &mut rows,
                );
            }
        }
        rows
    }

    /// Evaluates `expression` in the selected frame as a debug console command.
    pub fn evaluate_repl(&mut self, expression: &str) -> Result<(), DebugError> {
        self.push_output("console", format!("> {expression}"));
        self.evaluate(expression, "repl", PendingEvaluation::Repl)
    }

    /// Evaluates `expression` for a hover and writes `expression = result` to the console.
    pub fn evaluate_hover(&mut self, expression: &str) -> Result<(), DebugError> {
        let expression = expression.trim();
        if expression.is_empty() {
            return Ok(());
        }
        self.evaluate(
            expression,
            "hover",
            PendingEvaluation::Hover(expression.to_owned()),
        )
    }

    pub fn add_watch(&mut self, expression: &str) -> Result<(), DebugError> {
        let expression = expression.trim();
        if expression.is_empty() || self.watches.iter().any(|w| w.expression == expression) {
            return Ok(());
        }
        self.watches.push(WatchExpression {
            expression: expression.to_owned(),
            result: None,
        });
        if matches!(self.state, DebugState::Paused { .. }) {
            self.evaluate(
                expression,
                "watch",
                PendingEvaluation::Watch(expression.to_owned()),
            )?;
        }
        Ok(())
    }

    pub fn remove_watch(&mut self, index: usize) {
        if index < self.watches.len() {
            self.watches.remove(index);
        }
    }

    /// Re-evaluates every watch against the selected frame.
    pub fn refresh_watches(&mut self) -> Result<(), DebugError> {
        if self.session.is_none() || !matches!(self.state, DebugState::Paused { .. }) {
            return Ok(());
        }
        let expressions: Vec<String> = self.watches.iter().map(|w| w.expression.clone()).collect();
        for expression in expressions {
            let pending = PendingEvaluation::Watch(expression.clone());
            self.evaluate(&expression, "watch", pending)?;
        }
        Ok(())
    }

    fn evaluate(
        &mut self,
        expression: &str,
        context: &str,
        pending: PendingEvaluation,
    ) -> Result<(), DebugError> {
        let frame_id = self.selected_frame_id();
        let seq = self
            .session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request(
                "evaluate",
                serde_json::json!({"expression":expression,"frameId":frame_id,"context":context}),
            )?;
        self.pending_evaluations.insert(seq, pending);
        Ok(())
    }

    pub fn toggle_exception_filter(&mut self, index: usize) -> Result<(), DebugError> {
        let Some(filter) = self.exception_filters.get_mut(index) else {
            return Ok(());
        };
        filter.enabled = !filter.enabled;
        if self.session.is_some() {
            self.send_exception_filters()?;
        }
        Ok(())
    }

    fn send_exception_filters(&mut self) -> Result<(), DebugError> {
        if self.exception_filters.is_empty() {
            return Ok(());
        }
        let filters: Vec<&str> = self
            .exception_filters
            .iter()
            .filter(|filter| filter.enabled)
            .map(|filter| filter.id.as_str())
            .collect();
        let arguments = serde_json::json!({"filters":filters});
        self.session
            .as_mut()
            .ok_or(DebugError::Stopped)?
            .send_request("setExceptionBreakpoints", arguments)?;
        Ok(())
    }

    /// Adopts the adapter's advertised exception filters, keeping choices already made.
    fn adopt_exception_filters(&mut self, capabilities: &Value) {
        let previous = std::mem::take(&mut self.exception_filters);
        self.exception_filters = capabilities["exceptionBreakpointFilters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|filter| {
                let id = filter["filter"].as_str()?.to_owned();
                let enabled = previous
                    .iter()
                    .find(|old| old.id == id)
                    .map(|old| old.enabled)
                    .unwrap_or_else(|| filter["default"].as_bool().unwrap_or(false));
                Some(ExceptionFilter {
                    label: filter["label"].as_str().unwrap_or(&id).to_owned(),
                    id,
                    enabled,
                })
            })
            .collect();
    }

    fn step(&mut self, command: &str) -> Result<(), DebugError> {
        let thread_id = match self.state {
            DebugState::Paused { thread_id, .. } => thread_id,
            _ => return Err(DebugError::Stopped),
        };
        self.request(command, serde_json::json!({"threadId":thread_id}))?;
        self.state = DebugState::Running;
        self.clear_stop_state();
        Ok(())
    }

    fn handle_message(&mut self, message: Value, workspace: &Path) -> Result<(), DebugError> {
        self.version = self.version.wrapping_add(1);
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
            self.adopt_exception_filters(&message["body"]);
            let config = self.active_config.as_ref().ok_or(DebugError::Stopped)?;
            let mut arguments = serde_json::to_value(config)?;
            if let Some(object) = arguments.as_object_mut() {
                // Graphite-side settings that the adapter must not see.
                for key in ["adapter", "preLaunchTask", "postDebugTask"] {
                    object.remove(key);
                }
            }
            let context = VariableContext {
                workspace: workspace.to_path_buf(),
                active_file: self.active_file.clone(),
            };
            expand_value(&mut arguments, &context);
            // `program` and `cwd` are resolved after expansion so `${workspaceFolder}/x` is absolute.
            if let Some(program) = arguments["program"].as_str() {
                arguments["program"] = serde_json::json!(absolute(workspace, Path::new(program)));
            }
            let cwd = arguments["cwd"].as_str().map(PathBuf::from);
            arguments["cwd"] = serde_json::json!(cwd
                .map(|cwd| absolute(workspace, &cwd))
                .unwrap_or_else(|| workspace.to_path_buf()));
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
            self.selected_frame = 0;
            self.scopes.clear();
            self.children.clear();
            self.expanded.clear();
            if let Some(frame) = self.frames.first() {
                self.request_scopes(frame.id)?;
            }
            self.refresh_watches()?;
        } else if self.scopes_seq == Some(seq) {
            self.scopes_seq = None;
            self.scopes = message["body"]["scopes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|scope| Scope {
                    name: scope["name"].as_str().unwrap_or("Scope").to_owned(),
                    reference: scope["variablesReference"].as_i64().unwrap_or_default(),
                    expensive: scope["expensive"].as_bool().unwrap_or(false),
                })
                .filter(|scope| scope.reference > 0)
                .collect();
            // Like VS Code, open the first cheap scope (normally "Locals").
            if let Some(reference) = self
                .scopes
                .iter()
                .find(|scope| !scope.expensive)
                .map(|scope| scope.reference)
            {
                self.toggle_variable(reference)?;
            }
        } else if let Some(reference) = self.pending_variables.remove(&seq) {
            if success {
                let variables = message["body"]["variables"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|variable| Variable {
                        name: variable["name"].as_str().unwrap_or_default().to_owned(),
                        value: variable["value"].as_str().unwrap_or_default().to_owned(),
                        type_name: variable["type"].as_str().map(str::to_owned),
                        reference: variable["variablesReference"].as_i64().unwrap_or_default(),
                    })
                    .collect();
                self.children.insert(reference, variables);
            } else {
                self.expanded.remove(&reference);
                self.push_output("stderr", dap_error(message));
            }
        } else if let Some((reference, name)) = self.pending_set_variables.remove(&seq) {
            if success {
                if let Some(variable) = self
                    .children
                    .get_mut(&reference)
                    .and_then(|list| list.iter_mut().find(|variable| variable.name == name))
                {
                    let body = &message["body"];
                    variable.value = body["value"].as_str().unwrap_or_default().to_owned();
                    if let Some(type_name) = body["type"].as_str() {
                        variable.type_name = Some(type_name.to_owned());
                    }
                    variable.reference = body["variablesReference"].as_i64().unwrap_or_default();
                }
            } else {
                self.push_output("stderr", dap_error(message));
            }
        } else if let Some(pending) = self.pending_evaluations.remove(&seq) {
            let outcome = if success {
                Ok(message["body"]["result"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned())
            } else {
                Err(dap_error(message))
            };
            match pending {
                PendingEvaluation::Watch(expression) => {
                    if let Some(watch) = self
                        .watches
                        .iter_mut()
                        .find(|watch| watch.expression == expression)
                    {
                        watch.result = Some(outcome);
                    }
                }
                PendingEvaluation::Hover(expression) => match outcome {
                    Ok(result) => {
                        let formatted = format!("{expression} = {result}");
                        self.last_hover_result = Some(formatted.clone());
                        self.push_output("console", formatted);
                    }
                    Err(error) => self.push_output("stderr", error),
                },
                PendingEvaluation::Repl => match outcome {
                    Ok(result) => self.push_output("console", result),
                    Err(error) => self.push_output("stderr", error),
                },
            }
        } else {
            self.pending_breakpoints.remove(&seq);
            if !success {
                self.push_output("stderr", dap_error(message));
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
                self.send_exception_filters()?;
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
                self.clear_stop_state();
                self.request_stack(thread_id)?;
            }
            "continued" => {
                self.state = DebugState::Running;
                self.clear_stop_state();
            }
            "terminated" | "exited" => {
                self.state = DebugState::Stopped;
                self.clear_stop_state();
                self.end_session();
            }
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

#[cfg(test)]
mod tests;
mod transport;
mod variables;

pub(crate) use variables::expand_value;
#[cfg(test)]
use variables::expand_variables;
pub use variables::VariableContext;
