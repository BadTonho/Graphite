//! VSIX Task provider & execution bridge (`MainThreadTask` / `ExtHostTask`).
//!
//! Tracks `vscode.tasks.registerTaskProvider` providers, supports process/shell and
//! `CustomExecution` task runs inside trusted workspaces, streams output lines into Graphite's
//! task output buffer, and relays task lifecycle events back to the extension host.

use crate::process_util::HideConsole;
use super::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

/// Replies to VSIX task requests carry ids in `TASK_BASE..TASK_END`.
pub(super) const TASK_BASE: u64 = 12_000_000;
const TASK_END: u64 = 13_000_000;

pub(super) fn is_task_reply(id: u64) -> bool {
    (TASK_BASE..TASK_END).contains(&id)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VsixTaskProvider {
    pub handle: u64,
    pub task_type: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VsixTaskEntry {
    pub provider_handle: Option<u64>,
    pub id: String,
    pub name: String,
    pub task_type: String,
    pub source: String,
    pub raw: Value,
}

enum TaskWorkerEvent {
    Output(String),
    Finished(Option<i32>),
}

struct RunningTask {
    id: String,
    task: Value,
    cancel: Arc<AtomicBool>,
    events: Option<Receiver<TaskWorkerEvent>>,
}

#[derive(Default)]
pub(super) struct Tasks {
    /// Only the tests allocate call ids from this counter.
    #[cfg_attr(not(test), allow(dead_code))]
    next_id: u64,
    next_exec_seq: u64,
    pub(crate) providers: BTreeMap<u64, VsixTaskProvider>,
    pub(crate) discovered: Vec<VsixTaskEntry>,
    running: BTreeMap<String, RunningTask>,
    in_flight_provide: BTreeMap<u64, u64>,
}

impl Tasks {
    pub(super) fn busy(&self) -> bool {
        !self.running.is_empty() || !self.in_flight_provide.is_empty()
    }

    #[cfg(test)]
    fn next_call_id(&mut self) -> u64 {
        let id = TASK_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (TASK_END - TASK_BASE);
        id
    }

    fn next_exec_id(&mut self, prefix: &str) -> String {
        self.next_exec_seq += 1;
        format!("vsix-task:{prefix}:{}", self.next_exec_seq)
    }
}

fn task_identifier(task: &Value, seq: u64) -> String {
    if let Some(id) = task["_id"].as_str().or_else(|| task["id"].as_str()) {
        if !id.is_empty() {
            return id.to_owned();
        }
    }
    let kind = task["type"].as_str().unwrap_or("custom");
    let name = task["name"]
        .as_str()
        .or_else(|| task["_label"].as_str())
        .unwrap_or("task");
    format!("vsix-task:{kind}:{name}:{seq}")
}

fn resolve_task_cwd(raw_cwd: Option<&str>, workspace_root: &Path) -> Result<PathBuf> {
    let Some(cwd) = raw_cwd.filter(|s| !s.is_empty()) else {
        return Ok(workspace_root.to_path_buf());
    };
    let path = if cwd.starts_with("file:") {
        crate::lsp::uri_to_path(cwd).ok_or("Invalid task cwd URI")?
    } else {
        let p = PathBuf::from(cwd);
        if p.is_absolute() {
            p
        } else {
            workspace_root.join(p)
        }
    };
    let root_canon =
        std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    let cwd_canon = std::fs::canonicalize(&path).map_err(|e| format!("Invalid task cwd: {e}"))?;
    if !cwd_canon.starts_with(&root_canon) {
        return Err("Task cwd escapes the trusted workspace root".into());
    }
    Ok(cwd_canon)
}

fn build_task_command(task: &Value, workspace_root: &Path) -> Result<Option<Command>> {
    let command_obj = &task["command"];
    if command_obj.is_null() {
        return Ok(None);
    }
    if command_obj["isCustomExecution"].as_bool() == Some(true)
        || task["isCustomExecution"].as_bool() == Some(true)
    {
        return Ok(None);
    }

    let name = command_obj["name"]
        .as_str()
        .or_else(|| command_obj["runtime"].as_str())
        .or_else(|| command_obj["commandLine"].as_str())
        .filter(|s| !s.is_empty());
    let Some(program) = name else {
        return Ok(None);
    };

    let raw_cwd = command_obj["presentation"]["cwd"]
        .as_str()
        .or_else(|| task["command"]["options"]["cwd"].as_str());
    let cwd = resolve_task_cwd(raw_cwd, workspace_root)?;

    let is_shell = command_obj["runtime"].as_str() == Some("shell")
        || command_obj["commandLine"].as_str().is_some();

    let mut cmd = if is_shell {
        let args: Vec<String> = command_obj["args"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v["value"].as_str().map(str::to_owned))
            })
            .collect();
        let full_line = if let Some(line) = command_obj["commandLine"].as_str() {
            line.to_owned()
        } else if args.is_empty() {
            program.to_owned()
        } else {
            format!("{program} {}", args.join(" "))
        };
        #[cfg(windows)]
        let shell = {
            let mut c = Command::new("cmd");
            c.args(["/C", &full_line]);
            c
        };
        #[cfg(not(windows))]
        let shell = {
            let mut c = Command::new("sh");
            c.args(["-c", &full_line]);
            c
        };
        shell
    } else {
        let mut process = Command::new(program);
        if let Some(args) = command_obj["args"].as_array() {
            for arg in args {
                if let Some(text) = arg.as_str().or_else(|| arg["value"].as_str()) {
                    process.arg(text);
                }
            }
        }
        process
    };

    cmd.hide_console();
    cmd.current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(env_map) = command_obj["options"]["env"].as_object() {
        for (k, v) in env_map {
            if let Some(val) = v.as_str() {
                cmd.env(k, val);
            }
        }
    }
    Ok(Some(cmd))
}

impl App {
    pub(super) fn handle_vsix_task_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        match method {
            "$registerTaskSystem" | "$registerSupportedExecutions" => Ok(Value::Null),
            "$registerTaskProvider" => {
                let handle = args
                    .first()
                    .and_then(Value::as_u64)
                    .ok_or("Missing task provider handle")?;
                let task_type = args
                    .get(1)
                    .and_then(Value::as_str)
                    .unwrap_or("custom")
                    .to_owned();
                runtime
                    .tasks
                    .providers
                    .insert(handle, VsixTaskProvider { handle, task_type });
                Ok(Value::Null)
            }
            "$unregister" => {
                if let Some(handle) = args.first().and_then(Value::as_u64) {
                    runtime.tasks.providers.remove(&handle);
                    runtime
                        .tasks
                        .discovered
                        .retain(|entry| entry.provider_handle != Some(handle));
                }
                Ok(Value::Null)
            }
            "$tryCreateTaskId" => {
                let task = args.first().unwrap_or(&Value::Null);
                let seq = runtime.tasks.next_exec_seq + 1;
                Ok(Value::String(task_identifier(task, seq)))
            }
            "$executeTask" => {
                let task = args.first().cloned().unwrap_or(Value::Null);
                self.start_vsix_task_execution(runtime, task)
            }
            "$terminateTask" => {
                let id = args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("Missing task execution id")?;
                if let Some(running) = runtime.tasks.running.remove(id) {
                    running.cancel.store(true, Ordering::Release);
                    let _ = runtime.session.send(json!({
                        "type": "call",
                        "actor": "ExtHostTask",
                        "method": "$onDidEndTaskProcess",
                        "args": [id, Value::Null],
                    }));
                    let _ = runtime.session.send(json!({
                        "type": "call",
                        "actor": "ExtHostTask",
                        "method": "$onDidEndTask",
                        "args": [{ "id": id, "task": running.task }],
                    }));
                }
                Ok(Value::Null)
            }
            "$customExecutionComplete" => {
                let id = args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("Missing custom execution task id")?
                    .to_owned();
                let exit_code = args.get(1).and_then(Value::as_i64);
                let task = runtime
                    .tasks
                    .running
                    .remove(&id)
                    .map(|r| r.task)
                    .unwrap_or(Value::Null);
                let _ = runtime.session.send(json!({
                    "type": "call",
                    "actor": "ExtHostTask",
                    "method": "$onDidEndTaskProcess",
                    "args": [id, exit_code],
                }));
                let _ = runtime.session.send(json!({
                    "type": "call",
                    "actor": "ExtHostTask",
                    "method": "$onDidEndTask",
                    "args": [{ "id": id, "task": task }],
                }));
                Ok(Value::Null)
            }
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadTask", method);
                Err(format!("Graphite does not support MainThreadTask.{method}"))
            }
        }
    }

    pub(super) fn start_vsix_task_execution(
        &mut self,
        runtime: &mut Runtime,
        task: Value,
    ) -> Result<Value> {
        if !self.vsix_workspace_trusted() {
            return Err("Workspace is restricted. Trust it before executing VSIX tasks.".into());
        }
        let prefix = task["type"].as_str().unwrap_or("custom");
        let exec_id = runtime.tasks.next_exec_id(prefix);
        let label = task["name"]
            .as_str()
            .or_else(|| task["_label"].as_str())
            .unwrap_or(&exec_id)
            .to_owned();
        let execution_dto = json!({
            "id": exec_id,
            "task": task,
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let events = if let Some(mut cmd) = build_task_command(&task, self.tree.root())? {
            let (tx, rx) = mpsc::channel();
            let worker_cancel = cancel.clone();
            std::thread::Builder::new()
                .name("graphite-vsix-task".into())
                .spawn(move || match cmd.spawn() {
                    Ok(mut child) => {
                        if let Some(stdout) = child.stdout.take() {
                            let tx_out = tx.clone();
                            std::thread::spawn(move || {
                                for line in BufReader::new(stdout)
                                    .lines()
                                    .map_while(std::result::Result::ok)
                                {
                                    let _ = tx_out.send(TaskWorkerEvent::Output(line));
                                }
                            });
                        }
                        if let Some(stderr) = child.stderr.take() {
                            let tx_err = tx.clone();
                            std::thread::spawn(move || {
                                for line in BufReader::new(stderr)
                                    .lines()
                                    .map_while(std::result::Result::ok)
                                {
                                    let _ = tx_err.send(TaskWorkerEvent::Output(line));
                                }
                            });
                        }
                        loop {
                            if worker_cancel.load(Ordering::Acquire) {
                                let _ = child.kill();
                                let _ = child.wait();
                                let _ = tx.send(TaskWorkerEvent::Finished(None));
                                break;
                            }
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    let _ = tx.send(TaskWorkerEvent::Finished(status.code()));
                                    break;
                                }
                                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                                Err(_) => {
                                    let _ = tx.send(TaskWorkerEvent::Finished(Some(1)));
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(TaskWorkerEvent::Output(format!(
                            "Failed to start task: {error}"
                        )));
                        let _ = tx.send(TaskWorkerEvent::Finished(Some(1)));
                    }
                })
                .map_err(|e| e.to_string())?;
            Some(rx)
        } else {
            None
        };

        runtime.tasks.running.insert(
            exec_id.clone(),
            RunningTask {
                id: exec_id.clone(),
                task: execution_dto["task"].clone(),
                cancel,
                events,
            },
        );
        self.status = format!("Running VSIX task: {label}");
        self.task_last_label = Some(label);
        self.task_output.clear();
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostTask",
            "method": "$onDidStartTask",
            "args": [execution_dto, -1, -1],
        }));
        Ok(execution_dto)
    }

    #[cfg(test)]
    pub(crate) fn request_vsix_tasks(&mut self) -> Result<()> {
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            return Err("No active VSIX runtime".into());
        };
        let providers: Vec<(u64, String)> = runtime
            .tasks
            .providers
            .iter()
            .map(|(h, p)| (*h, p.task_type.clone()))
            .collect();
        for (handle, task_type) in providers {
            let call_id = runtime.tasks.next_call_id();
            runtime.tasks.in_flight_provide.insert(call_id, handle);
            runtime.session.send(json!({
                "type": "call",
                "id": call_id,
                "actor": "ExtHostTask",
                "method": "$provideTasks",
                "args": [handle, { &task_type: true }],
                "cancellable": true,
            }))?;
        }
        Ok(())
    }

    pub(super) fn finish_vsix_task(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some(handle) = runtime.tasks.in_flight_provide.remove(&id) else {
            return;
        };
        if let Some(error) = event["error"].as_str() {
            self.status = error.to_owned();
            return;
        }
        runtime
            .tasks
            .discovered
            .retain(|entry| entry.provider_handle != Some(handle));
        if let Some(tasks) = event["result"]["tasks"]
            .as_array()
            .or_else(|| event["result"].as_array())
        {
            for (idx, raw) in tasks.iter().enumerate() {
                let task_type = raw["type"].as_str().unwrap_or("custom").to_owned();
                let name = raw["name"]
                    .as_str()
                    .or_else(|| raw["_label"].as_str())
                    .unwrap_or("Task")
                    .to_owned();
                let source = raw["_source"]["label"]
                    .as_str()
                    .unwrap_or(&task_type)
                    .to_owned();
                let id = task_identifier(raw, idx as u64 + 1);
                runtime.tasks.discovered.push(VsixTaskEntry {
                    provider_handle: Some(handle),
                    id,
                    name,
                    task_type,
                    source,
                    raw: raw.clone(),
                });
            }
        }
    }

    pub(super) fn poll_vsix_tasks(&mut self, runtime: &mut Runtime) {
        let ids: Vec<String> = runtime.tasks.running.keys().cloned().collect();
        for id in ids {
            let mut finished = None;
            if let Some(running) = runtime.tasks.running.get(&id) {
                if let Some(rx) = running.events.as_ref() {
                    while let Ok(evt) = rx.try_recv() {
                        match evt {
                            TaskWorkerEvent::Output(line) => self.task_output.push(line),
                            TaskWorkerEvent::Finished(code) => finished = Some(code),
                        }
                    }
                }
            }
            if let Some(code) = finished {
                if let Some(running) = runtime.tasks.running.remove(&id) {
                    let _ = runtime.session.send(json!({
                        "type": "call",
                        "actor": "ExtHostTask",
                        "method": "$onDidEndTaskProcess",
                        "args": [running.id, code],
                    }));
                    let _ = runtime.session.send(json!({
                        "type": "call",
                        "actor": "ExtHostTask",
                        "method": "$onDidEndTask",
                        "args": [{ "id": running.id, "task": running.task }],
                    }));
                    self.status = match code {
                        Some(0) => "VSIX task finished".into(),
                        Some(c) => format!("VSIX task exited with code {c}"),
                        None => "VSIX task cancelled".into(),
                    };
                }
            }
        }
    }
}
