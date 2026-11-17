//! DAP process framing and launch configuration parsing.
//!
//! The transport owns a single adapter process. It never blocks the workbench
//! thread: adapter output is framed on a reader thread and delivered through a
//! bounded channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
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
    socket: Option<TcpStream>,
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

/// What the `${...}` variables of a launch configuration resolve against.
#[derive(Clone, Debug, Default)]
pub struct VariableContext {
    pub workspace: PathBuf,
    /// The file open in the editor, for `${file}` and its relatives.
    pub active_file: Option<PathBuf>,
}

impl VariableContext {
    fn resolve(&self, name: &str) -> Option<String> {
        let text = |path: &Path| path.display().to_string();
        let file = self.active_file.as_deref();
        let file_part = |part: fn(&Path) -> Option<String>| file.and_then(part);
        match name {
            "workspaceFolder" | "cwd" => Some(text(&self.workspace)),
            "workspaceFolderBasename" => self
                .workspace
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            "userHome" => std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).display().to_string()),
            "pathSeparator" | "/" => Some(std::path::MAIN_SEPARATOR.to_string()),
            "file" => file.map(text),
            "fileBasename" => {
                file_part(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()))
            }
            "fileBasenameNoExtension" => {
                file_part(|f| f.file_stem().map(|n| n.to_string_lossy().into_owned()))
            }
            "fileExtname" => file_part(|f| {
                f.extension()
                    .map(|extension| format!(".{}", extension.to_string_lossy()))
            }),
            "fileDirname" => file_part(|f| f.parent().map(|dir| dir.display().to_string())),
            "relativeFile" => file.map(|f| text(f.strip_prefix(&self.workspace).unwrap_or(f))),
            "relativeFileDirname" => file.and_then(|f| {
                f.parent()
                    .map(|dir| text(dir.strip_prefix(&self.workspace).unwrap_or(dir)))
            }),
            _ => name
                .strip_prefix("env:")
                .map(|variable| std::env::var(variable).unwrap_or_default()),
        }
    }
}

/// Replaces `${variable}` references. Unknown variables, and `${file}`-style variables when no
/// file is open, are left in the text so the failure is visible instead of silently empty.
pub fn expand_variables(value: &str, context: &VariableContext) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            result.push_str(&rest[start..]);
            return result;
        };
        match context.resolve(&after[..end]) {
            Some(replacement) => result.push_str(&replacement),
            None => result.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    result.push_str(rest);
    result
}

pub(crate) fn expand_value(value: &mut Value, context: &VariableContext) {
    match value {
        Value::String(text) => *text = expand_variables(text, context),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| expand_value(item, context)),
        Value::Object(map) => map
            .values_mut()
            .for_each(|item| expand_value(item, context)),
        _ => {}
    }
}

impl DapSession {
    pub fn spawn(config: &LaunchConfig, cwd: &Path) -> Result<Self, DebugError> {
        if let Some(server) = &config.server {
            return Self::connect(server);
        }
        if let Some(pipe) = &config.pipe {
            return Self::connect_pipe(pipe);
        }
        let adapter = config
            .adapter
            .as_ref()
            .ok_or_else(|| DebugError::MissingAdapter(config.adapter_type.clone()))?;
        let mut child = Command::new(&adapter.command);
        child.args(&adapter.args).envs(&adapter.env);
        let mut child = child
            .current_dir(adapter.cwd.as_deref().unwrap_or(cwd))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .map(|stdin| Box::new(stdin) as Box<dyn Write + Send>);
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
            child: Some(child),
            stdin,
            socket: None,
            closer: None,
            events,
            sequence: 1,
        })
    }

    /// Connects to an adapter listening on a named pipe (Windows) or a local socket (Unix).
    /// Only local pipes are accepted: the path may come from an extension.
    pub fn connect_pipe(path: &Path) -> Result<Self, DebugError> {
        if !is_local_pipe(path) {
            return Err(DebugError::Transport(format!(
                "Debug adapter pipe {} is not a local pipe",
                path.display()
            )));
        }
        #[cfg(windows)]
        let stream = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        #[cfg(unix)]
        let stream = std::os::unix::net::UnixStream::connect(path)?;
        let reader = stream.try_clone()?;
        #[cfg(unix)]
        let closer: Option<Box<dyn Fn() + Send>> = {
            let handle = stream.try_clone()?;
            Some(Box::new(move || {
                let _ = handle.shutdown(Shutdown::Both);
            }))
        };
        #[cfg(windows)]
        let closer: Option<Box<dyn Fn() + Send>> = None;
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        thread::Builder::new()
            .name("graphite-dap-reader".into())
            .spawn(move || read_messages(reader, sender))?;
        Ok(Self {
            child: None,
            stdin: Some(Box::new(stream)),
            socket: None,
            closer,
            events,
            sequence: 1,
        })
    }

    /// A session whose adapter runs inside the extension host: the messages Graphite sends come
    /// out of `outgoing`, and what the adapter says is fed to the returned sender.
    pub fn inline(outgoing: mpsc::Sender<Value>) -> (Self, SyncSender<AdapterEvent>) {
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        let session = Self {
            child: None,
            stdin: Some(Box::new(InlineWriter {
                buffer: Vec::new(),
                sink: outgoing,
            })),
            socket: None,
            closer: None,
            events,
            sequence: 1,
        };
        (session, sender)
    }

    /// Connects to an adapter that is already listening. Only loopback addresses are accepted:
    /// the address may come from an extension, which must not make Graphite dial out.
    pub fn connect(server: &AdapterServer) -> Result<Self, DebugError> {
        let address = (server.host.as_str(), server.port)
            .to_socket_addrs()?
            .find(|address| address.ip().is_loopback())
            .ok_or_else(|| {
                DebugError::Transport(format!(
                    "Debug adapter server {}:{} is not a local address",
                    server.host, server.port
                ))
            })?;
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        let reader = stream.try_clone()?;
        let socket = stream.try_clone()?;
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        thread::Builder::new()
            .name("graphite-dap-reader".into())
            .spawn(move || read_messages(reader, sender))?;
        Ok(Self {
            child: None,
            stdin: Some(Box::new(stream)),
            socket: Some(socket),
            closer: None,
            events,
            sequence: 1,
        })
    }

    /// A session without an adapter process, for tests that drive the protocol directly.
    #[cfg(test)]
    fn detached(writer: Box<dyn Write + Send>) -> Self {
        let (_sender, events) = mpsc::sync_channel(1);
        Self {
            child: None,
            stdin: Some(writer),
            socket: None,
            closer: None,
            events,
            sequence: 1,
        }
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
        match self.child.as_mut() {
            Some(child) => Ok(child.try_wait()?.is_none()),
            None => Ok(true),
        }
    }

    pub fn stop(&mut self) -> Result<(), DebugError> {
        self.stdin.take();
        if let Some(socket) = self.socket.take() {
            // Wakes the reader thread, which would otherwise wait for the adapter forever.
            let _ = socket.shutdown(Shutdown::Both);
        }
        if let Some(close) = self.closer.take() {
            close();
        }
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            let _ = child.wait();
        }
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

/// Whether `path` names a pipe on this machine: `\\.\pipe\name` on Windows, an absolute path
/// elsewhere.
fn is_local_pipe(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let text = path
            .to_string_lossy()
            .to_ascii_lowercase()
            .replace('/', "\\");
        text.starts_with("\\\\.\\pipe\\") && text.len() > "\\\\.\\pipe\\".len()
    }
    #[cfg(not(windows))]
    {
        path.is_absolute()
    }
}

/// Receives the framed messages Graphite writes (`Content-Length` header and JSON body) and
/// hands each to the extension host as a value.
struct InlineWriter {
    buffer: Vec<u8>,
    sink: mpsc::Sender<Value>,
}

impl Write for InlineWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        while let Some(header_end) = self
            .buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            let header = String::from_utf8_lossy(&self.buffer[..header_end]).into_owned();
            let length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "a message has no length")
                })?;
            let end = header_end + 4 + length;
            if self.buffer.len() < end {
                break;
            }
            let message: Value = serde_json::from_slice(&self.buffer[header_end + 4..end])
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            self.buffer.drain(..end);
            self.sink.send(message).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the adapter is gone")
            })?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    fn frame(value: &Value) -> Vec<u8> {
        let payload = serde_json::to_vec(value).unwrap();
        let mut bytes = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
        bytes.extend(payload);
        bytes
    }

    #[test]
    fn an_inline_adapter_receives_whole_messages_however_the_bytes_arrive() {
        let (sender, messages) = mpsc::channel();
        let (mut session, _events) = DapSession::inline(sender);
        let first = serde_json::json!({"seq": 1, "type": "request", "command": "initialize"});
        let second = serde_json::json!({"seq": 2, "type": "request", "command": "launch", "arguments": {"text": "é\n"}});
        // The writer gets the header and the body apart, and two messages in one piece.
        let mut stream = frame(&first);
        stream.extend(frame(&second));
        let stdin = session.stdin.as_mut().unwrap();
        for chunk in stream.chunks(7) {
            stdin.write_all(chunk).unwrap();
        }
        assert_eq!(messages.try_iter().collect::<Vec<_>>(), [first, second]);
        // A send through the session is framed and unframed again.
        let seq = session
            .send_request("threads", serde_json::json!({}))
            .unwrap();
        let sent = messages.try_recv().unwrap();
        assert_eq!(sent["seq"], seq);
        assert_eq!(sent["command"], "threads");
    }

    #[test]
    fn an_inline_adapter_reports_a_closed_session_and_garbage() {
        let (sender, messages) = mpsc::channel();
        let (mut session, events) = DapSession::inline(sender);
        events
            .send(AdapterEvent::Message(serde_json::json!({"type": "event"})))
            .unwrap();
        assert!(matches!(
            session.events.try_recv(),
            Ok(AdapterEvent::Message(_))
        ));
        // A message the extension host side dropped is a broken pipe for the writer.
        drop(messages);
        let error = session
            .send_request("next", serde_json::json!({}))
            .unwrap_err();
        assert!(
            matches!(error, DebugError::Read(ref io) if io.kind() == std::io::ErrorKind::BrokenPipe),
            "{error}"
        );
        // A header without a length is invalid.
        let (sender, _messages) = mpsc::channel();
        let (mut session, _events) = DapSession::inline(sender);
        let error = session
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"X: 1\r\n\r\n")
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn only_local_pipes_are_connected() {
        #[cfg(windows)]
        {
            assert!(is_local_pipe(Path::new(r"\\.\pipe\dap")));
            assert!(is_local_pipe(Path::new("//./pipe/dap")));
            assert!(!is_local_pipe(Path::new(r"\\server\pipe\dap")));
            assert!(!is_local_pipe(Path::new(r"C:\Users\me\file.txt")));
            assert!(!is_local_pipe(Path::new(r"\\.\pipe\")));
        }
        #[cfg(not(windows))]
        {
            assert!(is_local_pipe(Path::new("/tmp/dap.sock")));
            assert!(!is_local_pipe(Path::new("relative.sock")));
        }
        assert!(DapSession::connect_pipe(Path::new("relative-or-remote")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_session_talks_to_an_adapter_over_a_local_socket() {
        use std::io::Read;
        use std::os::unix::net::UnixListener;
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("dap.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let adapter = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = BufReader::new(stream.try_clone().unwrap());
            let message = read_message(&mut request).unwrap().unwrap();
            stream
                .write_all(&frame(&serde_json::json!({"seq": 1, "type": "response", "command": message["command"]})))
                .unwrap();
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest);
        });
        let mut session = DapSession::connect_pipe(&path).unwrap();
        session
            .send_request("initialize", serde_json::json!({}))
            .unwrap();
        let reply = loop {
            match session.events.recv_timeout(Duration::from_secs(5)).unwrap() {
                AdapterEvent::Message(message) => break message,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(reply["command"], "initialize");
        session.stop().unwrap();
        adapter.join().unwrap();
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

    #[derive(Clone, Default)]
    struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl SharedWriter {
        /// Every request sent so far, oldest first.
        fn requests(&self) -> Vec<Value> {
            let bytes = self.0.lock().unwrap().clone();
            let mut reader = BufReader::new(Cursor::new(bytes));
            let mut requests = Vec::new();
            while let Ok(Some(message)) = read_message(&mut reader) {
                requests.push(message);
            }
            requests
        }

        fn last(&self) -> Value {
            self.requests().pop().expect("a request was sent")
        }
    }

    fn paused_runtime() -> (DebugRuntime, SharedWriter) {
        let writer = SharedWriter::default();
        let runtime = DebugRuntime {
            session: Some(DapSession::detached(Box::new(writer.clone()))),
            state: DebugState::Paused {
                thread_id: 1,
                reason: "breakpoint".into(),
            },
            ..DebugRuntime::default()
        };
        (runtime, writer)
    }

    fn respond(runtime: &mut DebugRuntime, request_seq: i64, body: Value) {
        runtime
            .handle_message(
                serde_json::json!({"type":"response","request_seq":request_seq,"success":true,"body":body}),
                Path::new("."),
            )
            .unwrap();
    }

    fn respond_error(runtime: &mut DebugRuntime, request_seq: i64, message: &str) {
        runtime
            .handle_message(
                serde_json::json!({"type":"response","request_seq":request_seq,"success":false,"message":message}),
                Path::new("."),
            )
            .unwrap();
    }

    /// Drives stackTrace and scopes so the runtime shows locals `a` and expandable `obj`.
    fn paused_with_locals() -> (DebugRuntime, SharedWriter) {
        let (mut runtime, writer) = paused_runtime();
        runtime.request_stack(1).unwrap();
        respond(
            &mut runtime,
            1,
            serde_json::json!({"stackFrames":[
                {"id":10,"name":"main","source":{"path":"a.rs"},"line":3},
                {"id":11,"name":"caller","source":{"path":"b.rs"},"line":9}
            ]}),
        );
        respond(
            &mut runtime,
            2,
            serde_json::json!({"scopes":[
                {"name":"Locals","variablesReference":5},
                {"name":"Globals","variablesReference":6,"expensive":true}
            ]}),
        );
        respond(
            &mut runtime,
            3,
            serde_json::json!({"variables":[
                {"name":"a","value":"1","type":"int","variablesReference":0},
                {"name":"obj","value":"Obj","variablesReference":7}
            ]}),
        );
        (runtime, writer)
    }

    fn config_with_tasks(pre: Option<&str>, post: Option<&str>) -> LaunchConfig {
        LaunchConfig {
            name: "x".into(),
            adapter_type: "t".into(),
            request: RequestKind::Launch,
            program: None,
            args: Vec::new(),
            cwd: None,
            stop_on_entry: false,
            pre_launch_task: pre.map(str::to_owned),
            post_debug_task: post.map(str::to_owned),
            env: BTreeMap::new(),
            adapter: None,
            server: None,
            pipe: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn the_selected_configuration_wins_and_the_first_is_the_fallback() {
        let mut runtime = DebugRuntime::default();
        assert!(runtime.launch_config().is_none());
        runtime.launch_configs = vec![
            LaunchConfig {
                name: "first".into(),
                ..config_with_tasks(None, None)
            },
            LaunchConfig {
                name: "second".into(),
                ..config_with_tasks(None, None)
            },
        ];
        assert_eq!(runtime.launch_config().unwrap().name, "first");
        assert!(runtime.select_config("second"));
        assert_eq!(runtime.launch_config().unwrap().name, "second");
        assert!(
            !runtime.select_config("missing"),
            "unknown names are refused"
        );
        assert_eq!(
            runtime.launch_config().unwrap().name,
            "second",
            "choice is kept"
        );
        // The file changed and the selected configuration is gone.
        runtime.launch_configs.remove(1);
        assert_eq!(runtime.launch_config().unwrap().name, "first");
    }

    #[test]
    fn launch_profiles_read_pre_launch_and_post_debug_tasks() {
        let root = crate::test_support::TempDir::new();
        std::fs::create_dir_all(root.path().join(".vscode")).unwrap();
        std::fs::write(
            root.path().join(".vscode/launch.json"),
            r#"{"configurations":[
                {"name":"a","type":"t","request":"launch","preLaunchTask":"build","postDebugTask":"clean"},
                {"name":"b","type":"t","request":"launch"}
            ]}"#,
        )
        .unwrap();
        let configs = load_launch_configs(root.path()).unwrap();
        assert_eq!(configs[0].pre_launch_task.as_deref(), Some("build"));
        assert_eq!(configs[0].post_debug_task.as_deref(), Some("clean"));
        assert_eq!(configs[1].pre_launch_task, None);
        assert!(
            !configs[0].extra.contains_key("preLaunchTask"),
            "task fields are modelled, not passed through"
        );
    }

    #[test]
    fn the_adapter_never_sees_graphite_side_launch_settings() {
        let (mut runtime, writer) = paused_runtime();
        runtime.initialize_seq = Some(1);
        runtime.active_config = Some(config_with_tasks(Some("build"), Some("clean")));
        respond(&mut runtime, 1, serde_json::json!({}));
        let launch = writer.last();
        assert_eq!(launch["command"], "launch");
        let arguments = launch["arguments"].as_object().unwrap();
        for key in ["preLaunchTask", "postDebugTask", "adapter"] {
            assert!(!arguments.contains_key(key), "{key} leaked to the adapter");
        }
        assert_eq!(arguments["name"], "x");
    }

    #[test]
    fn the_post_debug_task_is_ready_once_when_the_session_is_stopped() {
        let (mut runtime, _writer) = paused_runtime();
        runtime.post_debug_task = Some("clean".into());
        assert_eq!(runtime.take_post_debug_task(), None, "not while running");
        runtime.stop().unwrap();
        assert_eq!(runtime.take_post_debug_task().as_deref(), Some("clean"));
        assert_eq!(runtime.take_post_debug_task(), None, "handed out once");
        runtime.stop().unwrap();
        assert_eq!(
            runtime.take_post_debug_task(),
            None,
            "a second stop adds nothing"
        );
    }

    #[test]
    fn the_post_debug_task_is_ready_when_the_program_terminates() {
        let (mut runtime, _writer) = paused_runtime();
        runtime.post_debug_task = Some("clean".into());
        runtime
            .handle_message(
                serde_json::json!({"type":"event","event":"terminated"}),
                Path::new("."),
            )
            .unwrap();
        assert_eq!(runtime.take_post_debug_task().as_deref(), Some("clean"));
    }

    #[test]
    fn the_post_debug_task_is_ready_when_the_adapter_connection_closes() {
        // A detached session's channel has no sender, so polling sees the adapter close.
        let (mut runtime, _writer) = paused_runtime();
        runtime.post_debug_task = Some("clean".into());
        runtime.poll(Path::new(".")).unwrap();
        assert!(runtime.session.is_none());
        assert_eq!(runtime.take_post_debug_task().as_deref(), Some("clean"));
    }

    #[test]
    fn sessions_without_a_post_debug_task_hand_out_nothing() {
        let (mut runtime, _writer) = paused_runtime();
        runtime.stop().unwrap();
        assert_eq!(runtime.take_post_debug_task(), None);
    }

    fn context() -> VariableContext {
        let workspace = std::env::temp_dir().join("proj");
        VariableContext {
            active_file: Some(workspace.join("src").join("main.py")),
            workspace,
        }
    }

    #[test]
    fn launch_variables_expand_workspace_and_file_parts() {
        let context = context();
        let workspace = context.workspace.display().to_string();
        let sep = std::path::MAIN_SEPARATOR;
        let expand = |text: &str| expand_variables(text, &context);
        assert_eq!(expand("${workspaceFolder}"), workspace);
        assert_eq!(expand("${workspaceFolderBasename}"), "proj");
        assert_eq!(expand("${cwd}"), workspace);
        assert_eq!(expand("${fileBasename}"), "main.py");
        assert_eq!(expand("${fileBasenameNoExtension}"), "main");
        assert_eq!(expand("${fileExtname}"), ".py");
        assert_eq!(expand("${relativeFile}"), format!("src{sep}main.py"));
        assert_eq!(expand("${relativeFileDirname}"), "src");
        assert_eq!(expand("${fileDirname}"), format!("{workspace}{sep}src"));
        assert_eq!(
            expand("${file}"),
            format!("{workspace}{sep}src{sep}main.py")
        );
        assert_eq!(expand("a${pathSeparator}b${/}c"), format!("a{sep}b{sep}c"));
        assert_eq!(
            expand("${workspaceFolder}/${fileBasename}"),
            format!("{workspace}/main.py"),
            "several variables in one string"
        );
    }

    #[test]
    fn environment_variables_expand_and_missing_ones_become_empty() {
        let context = context();
        std::env::set_var("GRAPHITE_DEBUG_TEST_VALUE", "from-env");
        assert_eq!(
            expand_variables("${env:GRAPHITE_DEBUG_TEST_VALUE}!", &context),
            "from-env!"
        );
        assert_eq!(
            expand_variables("[${env:GRAPHITE_DEBUG_TEST_UNSET_VALUE}]", &context),
            "[]"
        );
    }

    #[test]
    fn unknown_or_unavailable_variables_stay_visible() {
        let mut context = context();
        context.active_file = None;
        for text in [
            "${unknown}",
            "${file}",
            "${relativeFile}",
            "${input:name}",
            "${command:pick}",
            "${unterminated",
            "$notavariable",
            "${}",
        ] {
            assert_eq!(expand_variables(text, &context), text);
        }
        assert_eq!(expand_variables("", &context), "");
        assert_eq!(
            expand_variables("é${file}ü${workspaceFolderBasename}", &context),
            "é${file}üproj",
            "multi-byte text around variables is kept"
        );
    }

    #[test]
    fn expansion_reaches_nested_launch_values() {
        let context = context();
        let mut value = serde_json::json!({
            "args": ["--root", "${workspaceFolder}", 3],
            "env": {"A": "${fileBasename}"},
            "n": 1, "flag": true, "nothing": null
        });
        expand_value(&mut value, &context);
        assert_eq!(value["args"][1], context.workspace.display().to_string());
        assert_eq!(value["args"][2], 3);
        assert_eq!(value["env"]["A"], "main.py");
        assert_eq!(value["flag"], true);
    }

    #[test]
    fn launch_arguments_get_expanded_absolute_program_and_cwd() {
        let (mut runtime, writer) = paused_runtime();
        runtime.initialize_seq = Some(1);
        runtime.active_file = Some(PathBuf::from("/w/app.py"));
        let mut config = config_with_tasks(None, None);
        config.program = Some(PathBuf::from("${workspaceFolder}/main.py"));
        config.args = vec!["--file".into(), "${fileBasename}".into()];
        config
            .env
            .insert("ROOT".into(), "${workspaceFolder}".into());
        runtime.active_config = Some(config);
        let workspace = std::env::temp_dir().join("proj");
        runtime
            .handle_message(
                serde_json::json!({"type":"response","request_seq":1,"success":true,"body":{}}),
                &workspace,
            )
            .unwrap();
        let launch = writer.last();
        let arguments = &launch["arguments"];
        let root = workspace.display().to_string();
        assert_eq!(arguments["program"], format!("{root}/main.py"));
        assert_eq!(arguments["args"][1], "app.py");
        assert_eq!(arguments["env"]["ROOT"], root);
        assert_eq!(arguments["cwd"], root, "defaults to the workspace");
    }

    #[test]
    fn a_relative_program_and_cwd_resolve_against_the_workspace() {
        let (mut runtime, writer) = paused_runtime();
        runtime.initialize_seq = Some(1);
        let mut config = config_with_tasks(None, None);
        config.program = Some(PathBuf::from("bin/run"));
        config.cwd = Some(PathBuf::from("sub"));
        runtime.active_config = Some(config);
        let workspace = std::env::temp_dir().join("proj");
        runtime
            .handle_message(
                serde_json::json!({"type":"response","request_seq":1,"success":true,"body":{}}),
                &workspace,
            )
            .unwrap();
        let arguments = &writer.last()["arguments"];
        assert_eq!(
            arguments["program"],
            workspace.join("bin/run").display().to_string()
        );
        assert_eq!(
            arguments["cwd"],
            workspace.join("sub").display().to_string()
        );
    }

    #[test]
    fn a_stop_loads_the_first_cheap_scope_and_lists_its_variables() {
        let (runtime, writer) = paused_with_locals();
        let requests = writer.requests();
        assert_eq!(requests[1]["command"], "scopes");
        assert_eq!(requests[1]["arguments"]["frameId"], 10);
        assert_eq!(requests[2]["command"], "variables");
        assert_eq!(requests[2]["arguments"]["variablesReference"], 5);
        let rows = runtime.variable_rows();
        let labels: Vec<_> = rows
            .iter()
            .map(|row| (row.depth, row.name.as_str(), row.expanded))
            .collect();
        assert_eq!(
            labels,
            [
                (0, "Locals", true),
                (1, "a", false),
                (1, "obj", false),
                (0, "Globals", false)
            ]
        );
        assert_eq!(rows[1].value, "1");
        assert!(rows[0].is_scope && !rows[1].is_scope);
    }

    #[test]
    fn expanding_a_variable_fetches_children_once_and_collapsing_hides_them() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.toggle_variable(7).unwrap();
        assert_eq!(writer.last()["arguments"]["variablesReference"], 7);
        respond(
            &mut runtime,
            4,
            serde_json::json!({"variables":[{"name":"field","value":"2","variablesReference":0}]}),
        );
        let rows = runtime.variable_rows();
        assert_eq!(rows[3].name, "field");
        assert_eq!(rows[3].depth, 2);
        assert!(rows[2].expanded);

        runtime.toggle_variable(7).unwrap();
        assert_eq!(runtime.variable_rows().len(), 4);
        let sent = writer.requests().len();
        runtime.toggle_variable(7).unwrap();
        assert_eq!(
            writer.requests().len(),
            sent,
            "cached children are not fetched again"
        );
        assert_eq!(runtime.variable_rows().len(), 5);
    }

    #[test]
    fn self_referencing_variables_do_not_loop_forever() {
        let (mut runtime, _writer) = paused_with_locals();
        runtime.toggle_variable(7).unwrap();
        respond(
            &mut runtime,
            4,
            serde_json::json!({"variables":[{"name":"me","value":"Obj","variablesReference":7}]}),
        );
        let rows = runtime.variable_rows();
        assert!(rows.len() < 10, "cycle was cut: {} rows", rows.len());
    }

    #[test]
    fn a_failed_variables_request_reports_and_collapses() {
        let (mut runtime, _writer) = paused_with_locals();
        runtime.toggle_variable(7).unwrap();
        respond_error(&mut runtime, 4, "cannot read");
        assert!(!runtime.variable_rows()[2].expanded);
        assert_eq!(runtime.output.last().unwrap().text, "cannot read");
    }

    #[test]
    fn selecting_another_frame_reloads_scopes_and_watches() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.add_watch("a + 1").unwrap();
        runtime.select_frame(1).unwrap();
        assert_eq!(runtime.selected_frame_id(), Some(11));
        assert!(runtime.variable_rows().is_empty(), "old locals are dropped");
        let requests = writer.requests();
        let scopes = requests
            .iter()
            .rev()
            .find(|request| request["command"] == "scopes")
            .unwrap();
        assert_eq!(scopes["arguments"]["frameId"], 11);
        assert_eq!(writer.last()["command"], "evaluate");
        assert_eq!(writer.last()["arguments"]["frameId"], 11);
        // Out-of-range indexes are ignored instead of panicking.
        runtime.select_frame(99).unwrap();
        assert_eq!(runtime.selected_frame_id(), Some(11));
    }

    #[test]
    fn watches_show_results_errors_and_survive_frame_changes() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.add_watch("a + 1").unwrap();
        runtime.add_watch("a + 1").unwrap();
        runtime.add_watch("  ").unwrap();
        assert_eq!(
            runtime.watches.len(),
            1,
            "duplicates and blanks are ignored"
        );
        let request = writer.last();
        assert_eq!(request["arguments"]["context"], "watch");
        assert_eq!(request["arguments"]["frameId"], 10);
        respond(&mut runtime, 4, serde_json::json!({"result":"2"}));
        assert_eq!(runtime.watches[0].result, Some(Ok("2".into())));

        runtime.add_watch("missing").unwrap();
        respond_error(&mut runtime, 5, "not defined");
        assert_eq!(runtime.watches[1].result, Some(Err("not defined".into())));
        assert!(
            runtime.output.iter().all(|line| line.text != "not defined"),
            "watch errors stay in the watch row"
        );

        runtime.remove_watch(0);
        assert_eq!(runtime.watches[0].expression, "missing");
    }

    #[test]
    fn repl_input_is_echoed_and_its_result_lands_in_the_console() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.evaluate_repl("a").unwrap();
        assert_eq!(writer.last()["arguments"]["context"], "repl");
        respond(&mut runtime, 4, serde_json::json!({"result":"1"}));
        let lines: Vec<_> = runtime
            .output
            .iter()
            .map(|line| (line.category.as_str(), line.text.as_str()))
            .collect();
        assert_eq!(lines, [("console", "> a"), ("console", "1")]);

        runtime.evaluate_repl("nope").unwrap();
        respond_error(&mut runtime, 5, "undefined: nope");
        assert_eq!(
            runtime.output.last().map(|line| line.category.as_str()),
            Some("stderr")
        );
        assert!(runtime.watches.is_empty(), "repl input is not a watch");
    }

    #[test]
    fn exception_filters_come_from_the_adapter_and_are_sent_when_toggled() {
        let (mut runtime, writer) = paused_runtime();
        runtime.initialize_seq = Some(1);
        runtime.active_config = Some(config_with_tasks(None, None));
        respond(
            &mut runtime,
            1,
            serde_json::json!({"exceptionBreakpointFilters":[
                {"filter":"raised","label":"Raised Exceptions","default":false},
                {"filter":"uncaught","label":"Uncaught Exceptions","default":true}
            ]}),
        );
        let enabled: Vec<_> = runtime
            .exception_filters
            .iter()
            .map(|filter| (filter.label.as_str(), filter.enabled))
            .collect();
        assert_eq!(
            enabled,
            [("Raised Exceptions", false), ("Uncaught Exceptions", true)]
        );

        runtime.toggle_exception_filter(0).unwrap();
        let request = writer.last();
        assert_eq!(request["command"], "setExceptionBreakpoints");
        assert_eq!(
            request["arguments"]["filters"],
            serde_json::json!(["raised", "uncaught"])
        );
        runtime.toggle_exception_filter(1).unwrap();
        assert_eq!(
            writer.last()["arguments"]["filters"],
            serde_json::json!(["raised"])
        );

        // The choices survive a restart that advertises the same filters.
        runtime.adopt_exception_filters(&serde_json::json!({"exceptionBreakpointFilters":[
            {"filter":"raised","label":"Raised","default":false},
            {"filter":"uncaught","label":"Uncaught","default":true}
        ]}));
        assert!(runtime.exception_filters[0].enabled);
        assert!(!runtime.exception_filters[1].enabled);
    }

    #[test]
    fn the_initialized_event_sends_exception_filters_before_configuration_done() {
        let (mut runtime, writer) = paused_runtime();
        runtime.exception_filters = vec![ExceptionFilter {
            id: "all".into(),
            label: "All".into(),
            enabled: true,
        }];
        runtime
            .handle_message(
                serde_json::json!({"type":"event","event":"initialized"}),
                Path::new("."),
            )
            .unwrap();
        let commands: Vec<_> = writer
            .requests()
            .iter()
            .map(|request| request["command"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(commands, ["setExceptionBreakpoints", "configurationDone"]);
    }

    #[test]
    fn adapters_without_exception_filters_get_no_exception_request() {
        let (mut runtime, writer) = paused_runtime();
        runtime.toggle_exception_filter(0).unwrap();
        runtime
            .handle_message(
                serde_json::json!({"type":"event","event":"initialized"}),
                Path::new("."),
            )
            .unwrap();
        let commands: Vec<_> = writer
            .requests()
            .iter()
            .map(|request| request["command"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(commands, ["configurationDone"]);
    }

    #[test]
    fn stepping_sends_the_paused_thread_and_clears_the_stale_stop() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.step_over().unwrap();
        let request = writer.last();
        assert_eq!(request["command"], "next");
        assert_eq!(request["arguments"]["threadId"], 1);
        assert_eq!(runtime.state, DebugState::Running);
        assert!(runtime.frames.is_empty());
        assert!(runtime.variable_rows().is_empty());
        assert!(runtime.step_over().is_err(), "cannot step while running");
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

    #[test]
    fn a_disabled_breakpoint_is_kept_but_not_sent_to_the_adapter() {
        let (mut runtime, writer) = paused_runtime();
        let file = PathBuf::from("a.rs");
        runtime
            .set_breakpoints(
                &file,
                vec![
                    serde_json::json!({"line": 3}),
                    serde_json::json!({"line": 5, "graphiteDisabled": true}),
                    serde_json::json!({"line": 8, "condition": "x > 1"}),
                ],
            )
            .unwrap();
        let sent = writer.last();
        assert_eq!(sent["command"], "setBreakpoints");
        assert_eq!(
            sent["arguments"]["breakpoints"],
            serde_json::json!([{"line": 3}, {"line": 8, "condition": "x > 1"}])
        );
        assert_eq!(
            runtime.breakpoints[&file].len(),
            3,
            "the list keeps the disabled one"
        );
        assert!(is_disabled(&runtime.breakpoints[&file][1]));
    }

    #[test]
    fn function_breakpoints_are_sent_deduplicated_and_cleared() {
        let (mut runtime, writer) = paused_runtime();
        runtime.add_function_breakpoint("  main ").unwrap();
        runtime.add_function_breakpoint("main").unwrap();
        runtime.add_function_breakpoint("   ").unwrap();
        runtime.add_function_breakpoint("helper").unwrap();
        let requests = writer.requests();
        assert_eq!(requests.len(), 2, "only the two new names were sent");
        assert_eq!(requests[1]["command"], "setFunctionBreakpoints");
        assert_eq!(
            requests[1]["arguments"]["breakpoints"],
            serde_json::json!([{"name": "main"}, {"name": "helper"}])
        );
        runtime.remove_function_breakpoints().unwrap();
        assert_eq!(
            writer.last()["arguments"]["breakpoints"],
            serde_json::json!([])
        );
        assert!(runtime.function_breakpoints.is_empty());
    }

    #[test]
    fn function_breakpoints_are_held_back_while_all_breakpoints_are_off() {
        let (mut runtime, writer) = paused_runtime();
        runtime.breakpoints_disabled = true;
        runtime.add_function_breakpoint("main").unwrap();
        assert_eq!(
            writer.last()["arguments"]["breakpoints"],
            serde_json::json!([])
        );
        assert_eq!(runtime.function_breakpoints, ["main"], "still remembered");
    }

    #[test]
    fn data_breakpoints_are_sent_deduplicated_and_respect_disabled_flag() {
        let (mut runtime, writer) = paused_runtime();
        runtime
            .add_data_breakpoint("0x1000", Some("write"), Some("x > 0"))
            .unwrap();
        runtime
            .add_data_breakpoint("0x1000", Some("readWrite"), None)
            .unwrap();
        assert_eq!(runtime.data_breakpoints.len(), 1);
        let req = writer.last();
        assert_eq!(req["command"], "setDataBreakpoints");
        assert_eq!(req["arguments"]["breakpoints"][0]["dataId"], "0x1000");
        assert_eq!(
            req["arguments"]["breakpoints"][0]["accessType"],
            "readWrite"
        );

        runtime.breakpoints_disabled = true;
        runtime.send_data_breakpoints().unwrap();
        assert_eq!(
            writer.last()["arguments"]["breakpoints"],
            serde_json::json!([])
        );

        runtime.remove_data_breakpoints().unwrap();
        assert!(runtime.data_breakpoints.is_empty());
    }

    #[test]
    fn setting_a_variable_sends_the_request_and_shows_the_new_value() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.set_variable("a", "42").unwrap();
        let request = writer.last();
        assert_eq!(request["command"], "setVariable");
        assert_eq!(request["arguments"]["variablesReference"], 5);
        assert_eq!(request["arguments"]["name"], "a");
        assert_eq!(request["arguments"]["value"], "42");
        let seq = request["seq"].as_i64().unwrap();
        respond(
            &mut runtime,
            seq,
            serde_json::json!({"value": "42", "type": "int"}),
        );
        let row = runtime
            .variable_rows()
            .into_iter()
            .find(|row| row.name == "a")
            .unwrap();
        assert_eq!(row.value, "42");
    }

    #[test]
    fn a_rejected_set_variable_reports_the_error_and_keeps_the_value() {
        let (mut runtime, writer) = paused_with_locals();
        runtime.set_variable("a", "oops").unwrap();
        let seq = writer.last()["seq"].as_i64().unwrap();
        respond_error(&mut runtime, seq, "not a number");
        assert_eq!(runtime.output.last().unwrap().text, "not a number");
        let row = runtime
            .variable_rows()
            .into_iter()
            .find(|row| row.name == "a")
            .unwrap();
        assert_eq!(row.value, "1");
    }

    #[test]
    fn setting_an_unknown_variable_is_refused_without_a_request() {
        let (mut runtime, writer) = paused_with_locals();
        let before = writer.requests().len();
        assert!(runtime.set_variable("nope", "1").is_err());
        assert_eq!(writer.requests().len(), before);
    }
}
