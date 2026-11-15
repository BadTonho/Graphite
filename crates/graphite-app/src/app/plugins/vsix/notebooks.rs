//! Notebook serializers and controllers (`MainThreadNotebook`, `MainThreadNotebookKernels`).
//!
//! * A **serializer** lets an extension define a notebook file format. Graphite reads the file
//!   through the serializer into a notebook, shows it in its own notebook editor, and writes it
//!   back through the serializer on save. The editor works on a mirror (`<profile>/notebook-mirror`)
//!   that holds the notebook as `.ipynb`; saving the mirror writes the real file.
//! * A **controller** is a kernel an extension provides. It appears in the kernel picker and runs
//!   cells like a Jupyter kernel does: Graphite gives the notebook a kernel handle whose other end
//!   is [`Adapter`], which turns execute requests into `$executeCells` calls and the execution's
//!   updates back into kernel events, so outputs, run states and interrupts use the existing
//!   notebook machinery.

use super::notebook_data::{notebook_from_dto, notebook_to_dto, output_from_dto, output_to_dto};
use super::*;
use crate::notebook::kernel::{
    decode, Command, KernelEvent, KernelHandle, KernelSpec, KernelState,
};
use base64::Engine;
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

/// Replies to notebook calls carry ids in `NB_BASE..NB_END`.
pub(super) const NB_BASE: u64 = 18_000_000;
const NB_END: u64 = 19_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// The notebook type of `.ipynb` files.
pub(crate) const JUPYTER_TYPE: &str = "jupyter-notebook";
/// Prefix of the name of a kernel spec that stands for an extension's controller.
pub(crate) const SPEC_PREFIX: &str = "vsix:";

pub(super) fn is_notebook_reply(id: u64) -> bool {
    (NB_BASE..NB_END).contains(&id)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Controller {
    pub handle: u64,
    pub id: String,
    pub notebook_type: String,
    pub label: String,
    pub detail: Option<String>,
    pub languages: Vec<String>,
}

impl Controller {
    fn update(&mut self, data: &Value) {
        if let Some(label) = data["label"].as_str() {
            self.label = label.to_owned();
        }
        if data.get("detail").is_some() {
            self.detail = data["detail"].as_str().map(str::to_owned);
        }
        if let Some(languages) = data["supportedLanguages"].as_array() {
            self.languages = languages
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
    }

    fn spec(&self) -> KernelSpec {
        KernelSpec {
            name: format!("{SPEC_PREFIX}{}", self.handle),
            display_name: match &self.detail {
                Some(detail) if !detail.is_empty() => format!("{} ({detail})", self.label),
                _ => self.label.clone(),
            },
            language: self.languages.first().cloned().unwrap_or_default(),
            argv: Vec::new(),
            env: Default::default(),
            interrupt_mode: "message".into(),
        }
    }
}

/// One cell being run by a controller.
#[derive(Debug)]
struct Run {
    cell: u64,
    /// The handle of the execution the extension created for the cell.
    execution: Option<u64>,
    count: u64,
    /// The outputs shown so far: id, items, metadata.
    outputs: Vec<(String, Vec<Value>, Value)>,
}

/// The extension end of a notebook's kernel handle.
struct Adapter {
    key: u64,
    controller: u64,
    document: DocumentId,
    notebook: String,
    commands: Receiver<Command>,
    events: Sender<KernelEvent>,
    /// Execute requests whose cell the session has not recorded yet.
    waiting: Vec<String>,
    runs: BTreeMap<String, Run>,
    counter: u64,
}

impl Adapter {
    fn emit(&self, event: KernelEvent) -> bool {
        self.events.send(event).is_ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Op {
    /// `$executeCells` for a request of an adapter.
    Execute {
        adapter: u64,
        request: String,
    },
    /// `$dataToNotebook`: the file at `path` is being read.
    Read {
        path: PathBuf,
        view_type: String,
    },
    /// `$notebookToData`: the mirror's notebook is being written to `original`.
    Write {
        original: PathBuf,
    },
    Other(&'static str),
}

#[derive(Default)]
pub(super) struct Notebooks {
    next_id: u64,
    next_adapter: u64,
    serializers: BTreeMap<u64, String>,
    controllers: BTreeMap<u64, Controller>,
    adapters: Vec<Adapter>,
    pending: BTreeMap<u64, (Op, Instant)>,
    /// Notebooks the extension host knows, by URI, with the cell documents it was given.
    synced: BTreeMap<String, Vec<String>>,
    /// Files waiting for their serializer to register: (path, notebook type).
    waiting_opens: Vec<(PathBuf, String)>,
    /// Mirror files of serialized notebooks, with the file they stand for.
    bindings: BTreeMap<PathBuf, (PathBuf, String)>,
}

impl Notebooks {
    pub(super) fn busy(&self) -> bool {
        !self.pending.is_empty() || !self.adapters.is_empty()
    }

    fn next_id(&mut self) -> u64 {
        let id = NB_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (NB_END - NB_BASE);
        id
    }

    fn serializer_for(&self, view_type: &str) -> Option<u64> {
        self.serializers
            .iter()
            .find(|(_, registered)| registered.as_str() == view_type)
            .map(|(handle, _)| *handle)
    }

    pub(crate) fn controllers(&self) -> impl Iterator<Item = &Controller> {
        self.controllers.values()
    }

    #[cfg(test)]
    pub(super) fn binding(&self, mirror: &Path) -> Option<&(PathBuf, String)> {
        self.bindings.get(mirror)
    }
}

fn uri_argument(uri: &str) -> Value {
    json!({ "$graphiteUri": uri })
}

fn updates_of(data: &Value) -> Vec<Value> {
    let list = if data["value"].is_array() {
        &data["value"]
    } else {
        data
    };
    list.as_array().cloned().unwrap_or_default()
}

/// A stable name for the mirror of `original` inside the mirror folder.
fn mirror_path(profile: &Path, original: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    original.hash(&mut hasher);
    let name = original
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "notebook".into());
    profile
        .join("notebook-mirror")
        .join(format!("{:016x}", hasher.finish()))
        .join(format!("{name}.ipynb"))
}

impl App {
    pub(super) fn handle_vsix_notebook_request(
        &mut self,
        runtime: &mut Runtime,
        actor: &str,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let handle = args.first().and_then(Value::as_u64);
        let text = |index: usize| args.get(index).and_then(Value::as_str);
        match (actor, method) {
            ("MainThreadNotebook", "$registerNotebookSerializer") => {
                let view_type = text(2).ok_or("Invalid notebook type")?.to_owned();
                runtime
                    .notebooks
                    .serializers
                    .insert(handle.ok_or("Invalid serializer handle")?, view_type);
                let waiting = std::mem::take(&mut runtime.notebooks.waiting_opens);
                for (path, view_type) in waiting {
                    self.start_notebook_read(runtime, &path, &view_type);
                }
                Ok(Value::Null)
            }
            ("MainThreadNotebook", "$unregisterNotebookSerializer") => {
                if let Some(handle) = handle {
                    runtime.notebooks.serializers.remove(&handle);
                }
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$addKernel") => {
                let data = args.get(1).unwrap_or(&Value::Null);
                let handle = handle.ok_or("Invalid controller handle")?;
                let mut controller = Controller {
                    handle,
                    id: data["id"].as_str().unwrap_or_default().to_owned(),
                    notebook_type: data["notebookType"].as_str().unwrap_or_default().to_owned(),
                    label: String::new(),
                    detail: None,
                    languages: Vec::new(),
                };
                controller.update(data);
                runtime.notebooks.controllers.insert(handle, controller);
                self.kernel_specs = None;
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$updateKernel") => {
                if let Some(controller) =
                    handle.and_then(|h| runtime.notebooks.controllers.get_mut(&h))
                {
                    controller.update(args.get(1).unwrap_or(&Value::Null));
                }
                self.kernel_specs = None;
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$removeKernel") => {
                if let Some(handle) = handle {
                    runtime.notebooks.controllers.remove(&handle);
                    let gone: Vec<usize> = runtime
                        .notebooks
                        .adapters
                        .iter()
                        .enumerate()
                        .filter(|(_, adapter)| adapter.controller == handle)
                        .map(|(index, _)| index)
                        .collect();
                    for index in gone.into_iter().rev() {
                        let adapter = runtime.notebooks.adapters.remove(index);
                        adapter.emit(KernelEvent::Exited {
                            details: "The notebook controller was removed".into(),
                        });
                    }
                }
                self.kernel_specs = None;
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$createExecution") => {
                let execution = handle.ok_or("Invalid execution handle")?;
                let cell = args.get(3).and_then(Value::as_u64);
                let notebook = text(2).unwrap_or_default();
                for adapter in &mut runtime.notebooks.adapters {
                    if adapter.notebook != notebook {
                        continue;
                    }
                    let request = adapter
                        .runs
                        .iter()
                        .find(|(_, run)| Some(run.cell) == cell && run.execution.is_none())
                        .map(|(request, _)| request.clone());
                    if let Some(request) = request {
                        adapter.counter += 1;
                        let count = adapter.counter;
                        if let Some(run) = adapter.runs.get_mut(&request) {
                            run.execution = Some(execution);
                            run.count = count;
                        }
                        adapter.emit(KernelEvent::ExecuteInput {
                            parent: request,
                            count,
                        });
                    }
                }
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$updateExecution") => {
                let execution = handle.ok_or("Invalid execution handle")?;
                for update in updates_of(args.get(1).unwrap_or(&Value::Null)) {
                    apply_execution_update(&mut runtime.notebooks.adapters, execution, &update);
                }
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$completeExecution") => {
                let execution = handle.ok_or("Invalid execution handle")?;
                let data = args.get(1).unwrap_or(&Value::Null);
                let data = if data["value"].is_object() {
                    &data["value"]
                } else {
                    data
                };
                let ok = data["lastRunSuccess"].as_bool().unwrap_or(true);
                for adapter in &mut runtime.notebooks.adapters {
                    let request = adapter
                        .runs
                        .iter()
                        .find(|(_, run)| run.execution == Some(execution))
                        .map(|(request, _)| request.clone());
                    if let Some(request) = request {
                        finish_run(adapter, &request, ok);
                    }
                }
                Ok(Value::Null)
            }
            ("MainThreadNotebookKernels", "$postMessage") => Ok(json!(false)),
            ("MainThreadNotebookKernels", _) => Ok(Value::Null),
            ("MainThreadNotebookDocuments", "$tryOpenNotebook")
            | ("MainThreadNotebookEditors", "$tryShowNotebookDocument") => {
                let uri = text(0).ok_or("Invalid notebook URI")?;
                let path = graphite_core_api::DocumentUri::parse(uri)
                    .map_err(|error| error.to_string())?
                    .to_path()
                    .map_err(|error| error.to_string())?;
                self.open_file(path);
                Ok(if method == "$tryOpenNotebook" {
                    json!(uri)
                } else {
                    json!("graphite-notebook")
                })
            }
            ("MainThreadNotebookDocuments", "$trySaveNotebook") => Ok(json!(false)),
            ("MainThreadNotebook", _) => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, actor, method);
                Err(format!("Graphite does not support {actor}.{method}"))
            }
        }
    }

    fn notebook_call(
        &mut self,
        runtime: &mut Runtime,
        actor: &str,
        method: &str,
        args: Vec<Value>,
        serializable: &[usize],
        op: Op,
    ) -> Result<u64> {
        let id = runtime.notebooks.next_id();
        runtime
            .notebooks
            .pending
            .insert(id, (op, Instant::now() + REQUEST_TIMEOUT));
        let sent = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": actor,
            "method": method,
            "args": args,
            "serializable": serializable,
            "cancellable": method != "$executeCells",
            "bytes": method == "$notebookToData",
        }));
        if let Err(error) = sent {
            runtime.notebooks.pending.remove(&id);
            return Err(error);
        }
        Ok(id)
    }

    /// The controllers that can run a notebook of `notebook_type`, as kernel specs.
    pub(crate) fn vsix_notebook_kernel_specs(&self) -> Vec<KernelSpec> {
        let Some(runtime) = self.plugins.vsix.as_ref() else {
            return Vec::new();
        };
        let document = self.active_document();
        let notebook_type = self.documents[document]
            .buffer
            .path()
            .and_then(|path| runtime.notebooks.bindings.get(path))
            .map_or(JUPYTER_TYPE, |(_, view_type)| view_type.as_str());
        runtime
            .notebooks
            .controllers()
            .filter(|controller| {
                controller.notebook_type == notebook_type || controller.notebook_type == "*"
            })
            .map(Controller::spec)
            .collect()
    }

    /// Gives the active notebook a kernel that is the controller `spec` stands for.
    pub(crate) fn start_extension_kernel(
        &mut self,
        spec: KernelSpec,
    ) -> std::result::Result<(), String> {
        let handle: u64 = spec
            .name
            .strip_prefix(SPEC_PREFIX)
            .and_then(|handle| handle.parse().ok())
            .ok_or("Invalid notebook controller")?;
        let document = self.active_document();
        let notebook = self.documents[document]
            .buffer
            .path()
            .and_then(super::editors::resource_of)
            .ok_or("Save the notebook before running it with an extension kernel")?;
        let runtime = self
            .plugins
            .vsix
            .as_mut()
            .ok_or("The extension host is not running")?;
        if !runtime.notebooks.controllers.contains_key(&handle) {
            return Err("The notebook controller is no longer registered".into());
        }
        let (command_tx, command_rx) = channel();
        let (event_tx, event_rx) = channel();
        let kernel = KernelHandle::from_parts(command_tx, event_rx, "", false);
        runtime.notebooks.next_adapter += 1;
        let adapter = Adapter {
            key: runtime.notebooks.next_adapter,
            controller: handle,
            document,
            notebook,
            commands: command_rx,
            events: event_tx,
            waiting: Vec::new(),
            runs: BTreeMap::new(),
            counter: 0,
        };
        adapter.emit(KernelEvent::Status {
            state: KernelState::Idle,
            parent: None,
        });
        runtime.notebooks.adapters.push(adapter);
        let Some(session) = self.documents[document].notebook.as_deref_mut() else {
            return Ok(());
        };
        session.attach_kernel(kernel, spec);
        Ok(())
    }

    /// Moves the notebook kernels forward: starts the runs that were requested, passes interrupts
    /// on and drops the kernels whose notebook let go of them.
    pub(super) fn drive_vsix_notebooks(&mut self, runtime: &mut Runtime) {
        let now = Instant::now();
        runtime
            .notebooks
            .pending
            .retain(|_, (_, deadline)| *deadline > now);
        let mut changed = false;
        let mut index = 0;
        while index < runtime.notebooks.adapters.len() {
            let mut gone = false;
            let mut incoming = Vec::new();
            loop {
                match runtime.notebooks.adapters[index].commands.try_recv() {
                    Ok(command) => incoming.push(command),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        gone = true;
                        break;
                    }
                }
            }
            for command in incoming {
                match command {
                    Command::Send { frames, .. } => {
                        if let Ok(message) = decode(b"", &frames) {
                            changed |= self.notebook_request(runtime, index, &message);
                        }
                    }
                    Command::Kill | Command::KillAfter(_) => gone = true,
                    Command::Signal => {}
                }
            }
            if !gone {
                self.start_waiting_runs(runtime, index);
            }
            if gone {
                let adapter = runtime.notebooks.adapters.remove(index);
                self.cancel_adapter_runs(runtime, &adapter);
            } else {
                index += 1;
            }
        }
        if changed || !runtime.notebooks.adapters.is_empty() {
            self.poll_kernels();
        }
    }

    fn notebook_request(
        &mut self,
        runtime: &mut Runtime,
        index: usize,
        message: &crate::notebook::kernel::Message,
    ) -> bool {
        match message.msg_type() {
            "execute_request" => {
                runtime.notebooks.adapters[index]
                    .waiting
                    .push(message.id().to_owned());
                true
            }
            "interrupt_request" => {
                let adapter = &runtime.notebooks.adapters[index];
                let cells: Vec<u64> = adapter.runs.values().map(|run| run.cell).collect();
                let (controller, notebook) = (adapter.controller, adapter.notebook.clone());
                if !cells.is_empty() {
                    let _ = runtime.session.send(json!({
                        "type": "call",
                        "actor": "ExtHostNotebookKernels",
                        "method": "$cancelCells",
                        "args": [controller, uri_argument(&notebook), cells],
                    }));
                }
                false
            }
            "kernel_info_request" => {
                let language = runtime
                    .notebooks
                    .controllers
                    .get(&runtime.notebooks.adapters[index].controller)
                    .and_then(|controller| controller.languages.first().cloned());
                runtime.notebooks.adapters[index].emit(KernelEvent::Info {
                    language,
                    banner: None,
                });
                true
            }
            _ => false,
        }
    }

    /// Starts the execute requests whose cell the notebook has recorded.
    fn start_waiting_runs(&mut self, runtime: &mut Runtime, index: usize) {
        let waiting = std::mem::take(&mut runtime.notebooks.adapters[index].waiting);
        let document = runtime.notebooks.adapters[index].document;
        let mut still_waiting = Vec::new();
        for request in waiting {
            let cell = self.documents[document]
                .notebook
                .as_deref()
                .and_then(|session| session.request_cell(&request));
            let Some(cell) = cell else {
                still_waiting.push(request);
                continue;
            };
            self.sync_notebook_with_host(runtime, index);
            let adapter = &mut runtime.notebooks.adapters[index];
            adapter.runs.insert(
                request.clone(),
                Run {
                    cell,
                    execution: None,
                    count: 0,
                    outputs: Vec::new(),
                },
            );
            adapter.emit(KernelEvent::Status {
                state: KernelState::Busy,
                parent: Some(request.clone()),
            });
            let (controller, notebook, key) =
                (adapter.controller, adapter.notebook.clone(), adapter.key);
            let sent = self.notebook_call(
                runtime,
                "ExtHostNotebookKernels",
                "$executeCells",
                vec![json!(controller), uri_argument(&notebook), json!([cell])],
                &[],
                Op::Execute {
                    adapter: key,
                    request: request.clone(),
                },
            );
            if let Err(error) = sent {
                self.status = error;
                if let Some(adapter) = runtime.notebooks.adapters.get_mut(index) {
                    finish_run(adapter, &request, false);
                }
            }
        }
        if let Some(adapter) = runtime.notebooks.adapters.get_mut(index) {
            adapter.waiting.extend(still_waiting);
        }
    }

    /// An adapter was dropped: its runs end.
    fn cancel_adapter_runs(&mut self, runtime: &mut Runtime, adapter: &Adapter) {
        if adapter.runs.is_empty() {
            return;
        }
        let cells: Vec<u64> = adapter.runs.values().map(|run| run.cell).collect();
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostNotebookKernels",
            "method": "$cancelCells",
            "args": [adapter.controller, uri_argument(&adapter.notebook), cells],
        }));
    }

    /// Gives the extension host the notebook as it is now, with a text document for each cell.
    fn sync_notebook_with_host(&mut self, runtime: &mut Runtime, index: usize) {
        let adapter = &runtime.notebooks.adapters[index];
        let (document, uri, controller) = (
            adapter.document,
            adapter.notebook.clone(),
            adapter.controller,
        );
        let Some(session) = self.documents[document].notebook.as_deref() else {
            return;
        };
        let view_type = runtime
            .notebooks
            .controllers
            .get(&controller)
            .map(|controller| controller.notebook_type.clone())
            .unwrap_or_else(|| JUPYTER_TYPE.into());
        let notebook = session.export();
        let default_language = notebook_to_dto(&notebook)["cells"][0]["language"]
            .as_str()
            .unwrap_or("plaintext")
            .to_owned();
        let base = uri.split('#').next().unwrap_or(&uri).to_owned();
        let mut cell_documents = Vec::new();
        let mut cells = Vec::new();
        let mut cell_uris = Vec::new();
        for state in &session.cells {
            let cell_uri = format!(
                "vscode-notebook-cell:{}#ch{:07}",
                base.trim_start_matches("file://"),
                state.uid
            );
            let code = state.cell.kind == crate::notebook::CellKind::Code;
            let language = if code {
                default_language.clone()
            } else {
                "markdown".into()
            };
            let source = state.text();
            cell_documents.push(json!({
                "uri": uri_argument(&cell_uri),
                "versionId": 1,
                "languageId": language,
                "lines": source.split('\n').collect::<Vec<_>>(),
                "EOL": "\n",
                "isDirty": false,
                "encoding": "utf8",
            }));
            cells.push(json!({
                "handle": state.uid,
                "uri": uri_argument(&cell_uri),
                "eol": "\n",
                "source": source.split_inclusive('\n').collect::<Vec<_>>(),
                "language": language,
                "cellKind": if code { 2 } else { 1 },
                "outputs": state.cell.outputs.iter().enumerate()
                    .map(|(n, output)| output_to_dto(output, &format!("{}-{n}", state.uid)))
                    .collect::<Vec<_>>(),
                "metadata": state.cell.metadata,
            }));
            cell_uris.push(uri_argument(&cell_uri));
        }
        let previous = runtime.notebooks.synced.remove(&uri).unwrap_or_default();
        let removed_notebook = !previous.is_empty();
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostDocumentsAndEditors",
            "method": "$acceptDocumentsAndEditorsDelta",
            "args": [{
                "removedDocuments": previous.iter().map(|uri| uri_argument(uri)).collect::<Vec<_>>(),
                "addedDocuments": cell_documents,
            }],
        }));
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostNotebook",
            "method": "$acceptDocumentAndEditorsDelta",
            "serializable": [0],
            "args": [{
                "removedDocuments": if removed_notebook { vec![uri_argument(&uri)] } else { Vec::new() },
                "addedDocuments": [{
                    "uri": uri_argument(&uri),
                    "versionId": 1,
                    "cells": cells,
                    "viewType": view_type,
                    "metadata": notebook.metadata,
                }],
            }],
        }));
        runtime.notebooks.synced.insert(
            uri,
            cell_uris
                .iter()
                .filter_map(|value| value["$graphiteUri"].as_str().map(str::to_owned))
                .collect(),
        );
    }

    pub(super) fn finish_vsix_notebook(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some((op, _)) = runtime.notebooks.pending.remove(&id) else {
            return;
        };
        let error = event["error"].as_str().map(str::to_owned);
        match op {
            Op::Execute { adapter, request } => {
                if let Some(adapter) = runtime
                    .notebooks
                    .adapters
                    .iter_mut()
                    .find(|a| a.key == adapter)
                {
                    // A run the extension never completed ends with its handler.
                    if adapter.runs.contains_key(&request) {
                        if let Some(error) = &error {
                            adapter.emit(KernelEvent::Output {
                                parent: request.clone(),
                                output: json!({"output_type": "error", "ename": "Error", "evalue": error, "traceback": []}),
                            });
                        }
                        finish_run(adapter, &request, error.is_none());
                    }
                }
                self.poll_kernels();
            }
            Op::Read { path, view_type } => match error {
                Some(error) => {
                    self.status = format!("{}: {error}", path.display());
                    self.open_file_as_text(path);
                }
                None => self.open_serialized_notebook_result(
                    runtime,
                    &path,
                    &view_type,
                    &event["result"],
                ),
            },
            Op::Write { original } => match error {
                Some(error) => self.status = format!("{}: {error}", original.display()),
                None => {
                    let bytes = event["result"]["$bytes"].as_str().and_then(|text| {
                        base64::engine::general_purpose::STANDARD.decode(text).ok()
                    });
                    match bytes {
                        Some(bytes) => match std::fs::write(&original, bytes) {
                            Ok(()) => {
                                self.status = format!(
                                    "{} {}",
                                    self.t("custom_editor.saved"),
                                    original.display()
                                )
                            }
                            Err(error) => self.status = format!("{}: {error}", original.display()),
                        },
                        None => {
                            self.status =
                                format!("{}: the serializer returned no data", original.display())
                        }
                    }
                }
            },
            Op::Other(what) => {
                if let Some(error) = error {
                    self.status = format!("{what}: {error}");
                }
            }
        }
    }

    /// The notebook type of the serializer an extension offers for `path` by default.
    pub(crate) fn default_notebook_type(&self, path: &Path) -> Option<String> {
        if self.plugins.vsix_polling
            || !self.vsix_workspace_trusted()
            || path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ipynb"))
            || self.custom_editor_text_paths.contains(path)
        {
            return None;
        }
        self.plugins
            .installed
            .iter()
            .filter(|record| super::eligible(record))
            .filter_map(|record| match &record.metadata {
                Metadata::Vsix(vsix) => Some(&vsix.notebooks),
                _ => None,
            })
            .flatten()
            .find(|notebook| notebook.is_default() && notebook.matches(path))
            .map(|notebook| notebook.view_type.clone())
    }

    /// Opens `path` through its notebook serializer. Returns false when that is not possible and
    /// the file should open as text.
    pub(crate) fn open_serialized_notebook(&mut self, path: &Path, view_type: &str) -> bool {
        self.trigger_vsix_activation_event(&format!("onNotebook:{view_type}"));
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return false;
        };
        if runtime.notebooks.serializer_for(view_type).is_some() {
            self.start_notebook_read(&mut runtime, path, view_type);
        } else {
            runtime
                .notebooks
                .waiting_opens
                .push((path.to_path_buf(), view_type.to_owned()));
        }
        self.plugins.vsix = Some(runtime);
        self.status = self.tf(
            "notebook.opening_with_extension",
            &[("name", &path.display().to_string())],
        );
        true
    }

    fn start_notebook_read(&mut self, runtime: &mut Runtime, path: &Path, view_type: &str) {
        let Some(handle) = runtime.notebooks.serializer_for(view_type) else {
            return;
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.status = format!("{}: {error}", path.display());
                return;
            }
        };
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let sent = self.notebook_call(
            runtime,
            "ExtHostNotebook",
            "$dataToNotebook",
            vec![json!(handle), json!({ "$graphiteBytes": encoded })],
            &[],
            Op::Read {
                path: path.to_path_buf(),
                view_type: view_type.to_owned(),
            },
        );
        if let Err(error) = sent {
            self.status = error;
        }
    }

    /// The serializer returned the notebook: it opens in the notebook editor through its mirror.
    fn open_serialized_notebook_result(
        &mut self,
        runtime: &mut Runtime,
        original: &Path,
        view_type: &str,
        result: &Value,
    ) {
        let notebook = notebook_from_dto(result);
        let mirror = mirror_path(&self.profile_root, original);
        let written = mirror
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&mirror, notebook.to_json()));
        if let Err(error) = written {
            self.status = format!("{}: {error}", mirror.display());
            return;
        }
        runtime.notebooks.bindings.insert(
            mirror.clone(),
            (original.to_path_buf(), view_type.to_owned()),
        );
        self.open_file_as_text(mirror);
    }

    /// A mirror of a serialized notebook was saved: writes the real file through the serializer.
    pub(crate) fn upload_serialized_notebook(&mut self, mirror: &Path) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        if let Some((original, view_type)) = runtime.notebooks.bindings.get(mirror).cloned() {
            self.write_serialized_notebook(&mut runtime, mirror, &original, &view_type);
        }
        self.plugins.vsix = Some(runtime);
    }

    fn write_serialized_notebook(
        &mut self,
        runtime: &mut Runtime,
        mirror: &Path,
        original: &Path,
        view_type: &str,
    ) {
        let Some(handle) = runtime.notebooks.serializer_for(view_type) else {
            self.status = self.t("custom_editor.unavailable").to_owned();
            return;
        };
        let Some(id) = self.documents.id_for_path(mirror) else {
            return;
        };
        let Some(session) = self.documents[id].notebook.as_deref() else {
            return;
        };
        let dto = notebook_to_dto(&session.export());
        let sent = self.notebook_call(
            runtime,
            "ExtHostNotebook",
            "$notebookToData",
            vec![json!(handle), dto],
            &[1],
            Op::Write {
                original: original.to_path_buf(),
            },
        );
        if let Err(error) = sent {
            self.status = error;
        }
    }

    /// The file a notebook mirror stands for.
    #[allow(dead_code)]
    pub(crate) fn serialized_notebook_origin(&self, mirror: &Path) -> Option<PathBuf> {
        self.plugins
            .vsix
            .as_ref()
            .and_then(|runtime| runtime.notebooks.bindings.get(mirror))
            .map(|(original, _)| original.clone())
    }
}

/// Applies one `ICellExecuteUpdateDto` of an execution to the run it belongs to.
fn apply_execution_update(adapters: &mut [Adapter], execution: u64, update: &Value) {
    for adapter in adapters {
        let Some(request) = adapter
            .runs
            .iter()
            .find(|(_, run)| run.execution == Some(execution))
            .map(|(request, _)| request.clone())
        else {
            continue;
        };
        match update["editType"].as_u64() {
            // Output
            Some(1) => {
                let append = update["append"].as_bool().unwrap_or(false);
                let Some(run) = adapter.runs.get_mut(&request) else {
                    return;
                };
                if !append {
                    run.outputs.clear();
                    adapter.emit(KernelEvent::ClearOutput {
                        parent: request.clone(),
                        wait: false,
                    });
                }
                for output in update["outputs"].as_array().into_iter().flatten() {
                    let (id, notebook_output) = output_from_dto(output, None);
                    let items = output["items"].as_array().cloned().unwrap_or_default();
                    if let Some(run) = adapter.runs.get_mut(&request) {
                        run.outputs.push((id, items, output["metadata"].clone()));
                    }
                    adapter.emit(KernelEvent::Output {
                        parent: request.clone(),
                        output: notebook_output,
                    });
                }
            }
            // OutputItems
            Some(2) => {
                let id = update["outputId"].as_str().unwrap_or_default();
                let append = update["append"].as_bool().unwrap_or(false);
                let new_items = update["items"].as_array().cloned().unwrap_or_default();
                let Some(run) = adapter.runs.get_mut(&request) else {
                    return;
                };
                let Some((_, items, metadata)) =
                    run.outputs.iter_mut().find(|(output, ..)| output == id)
                else {
                    return;
                };
                if append {
                    items.extend(new_items);
                } else {
                    *items = new_items;
                }
                // The notebook replaces only display outputs by id, so the outputs of the run are
                // shown again, in order, with the changed one updated.
                let outputs: Vec<Value> = run
                    .outputs
                    .iter()
                    .map(|(id, items, metadata)| {
                        output_from_dto(
                            &json!({"outputId": id, "items": items, "metadata": metadata}),
                            None,
                        )
                        .1
                    })
                    .collect();
                adapter.emit(KernelEvent::ClearOutput {
                    parent: request.clone(),
                    wait: false,
                });
                for output in outputs {
                    adapter.emit(KernelEvent::Output {
                        parent: request.clone(),
                        output,
                    });
                }
            }
            // ExecutionState
            Some(3) => {
                if let Some(order) = update["executionOrder"].as_u64() {
                    if let Some(run) = adapter.runs.get_mut(&request) {
                        run.count = order;
                    }
                    adapter.counter = adapter.counter.max(order);
                    adapter.emit(KernelEvent::ExecuteInput {
                        parent: request.clone(),
                        count: order,
                    });
                }
            }
            _ => {}
        }
        return;
    }
}

/// Ends the run of `request`: the reply and the idle status the notebook waits for.
fn finish_run(adapter: &mut Adapter, request: &str, ok: bool) {
    let Some(run) = adapter.runs.remove(request) else {
        return;
    };
    adapter.emit(KernelEvent::Reply {
        parent: request.to_owned(),
        ok,
        count: (run.count > 0).then_some(run.count),
    });
    adapter.emit(KernelEvent::Status {
        state: KernelState::Idle,
        parent: Some(request.to_owned()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> (Adapter, Receiver<KernelEvent>) {
        let (_command_tx, command_rx) = channel();
        let (event_tx, event_rx) = channel();
        (
            Adapter {
                key: 1,
                controller: 3,
                document: DocumentId(0),
                notebook: "file:///a.ipynb".into(),
                commands: command_rx,
                events: event_tx,
                waiting: Vec::new(),
                runs: BTreeMap::new(),
                counter: 0,
            },
            event_rx,
        )
    }

    fn run(adapter: &mut Adapter, request: &str, execution: u64) {
        adapter.runs.insert(
            request.into(),
            Run {
                cell: 7,
                execution: Some(execution),
                count: 0,
                outputs: Vec::new(),
            },
        );
    }

    fn item(mime: &str, text: &str) -> Value {
        json!({"mime": mime, "valueBytes": {"$bytes": base64::engine::general_purpose::STANDARD.encode(text)}})
    }

    #[test]
    fn output_updates_become_kernel_events_and_changed_outputs_are_shown_again() {
        let (mut adapters, events) = adapter();
        run(&mut adapters, "req", 9);
        let mut list = [adapters];
        apply_execution_update(&mut list, 9, &json!({"editType": 3, "executionOrder": 4}));
        apply_execution_update(
            &mut list,
            9,
            &json!({"editType": 1, "append": false,
            "outputs": [{"outputId": "o1", "items": [item("application/vnd.code.notebook.stdout", "a")]}]}),
        );
        apply_execution_update(
            &mut list,
            9,
            &json!({"editType": 2, "outputId": "o1", "append": true,
            "items": [item("application/vnd.code.notebook.stdout", "b")]}),
        );
        // Updates of an unknown execution or output change nothing.
        apply_execution_update(&mut list, 10, &json!({"editType": 1, "outputs": []}));
        apply_execution_update(
            &mut list,
            9,
            &json!({"editType": 2, "outputId": "nope", "items": []}),
        );
        let sent: Vec<KernelEvent> = events.try_iter().collect();
        assert_eq!(sent.len(), 5, "{sent:?}");
        assert_eq!(
            sent[0],
            KernelEvent::ExecuteInput {
                parent: "req".into(),
                count: 4
            }
        );
        assert!(matches!(
            sent[1],
            KernelEvent::ClearOutput { wait: false, .. }
        ));
        let KernelEvent::Output { output, .. } = &sent[2] else {
            panic!("{:?}", sent[2])
        };
        assert_eq!(output["text"], "a");
        // Changing an output shows the outputs of the run again with the new content.
        assert!(matches!(
            sent[3],
            KernelEvent::ClearOutput { wait: false, .. }
        ));
        let KernelEvent::Output { output, .. } = &sent[4] else {
            panic!("{:?}", sent[4])
        };
        assert_eq!(output["text"], "ab", "appended items extend the output");
    }

    #[test]
    fn finishing_a_run_sends_the_reply_then_idle_and_only_once() {
        let (mut adapters, events) = adapter();
        run(&mut adapters, "req", 9);
        adapters.runs.get_mut("req").unwrap().count = 2;
        finish_run(&mut adapters, "req", false);
        finish_run(&mut adapters, "req", true);
        let sent: Vec<KernelEvent> = events.try_iter().collect();
        assert_eq!(
            sent,
            [
                KernelEvent::Reply {
                    parent: "req".into(),
                    ok: false,
                    count: Some(2)
                },
                KernelEvent::Status {
                    state: KernelState::Idle,
                    parent: Some("req".into())
                },
            ]
        );
    }

    #[test]
    fn controllers_appear_as_kernel_specs_and_update_in_place() {
        let mut controller = Controller {
            handle: 5,
            id: "ext.kernel".into(),
            notebook_type: JUPYTER_TYPE.into(),
            label: "Sample".into(),
            detail: None,
            languages: Vec::new(),
        };
        controller.update(&json!({"label": "Sample Kernel", "detail": "3.12", "supportedLanguages": ["python", "r"]}));
        let spec = controller.spec();
        assert_eq!(spec.name, "vsix:5");
        assert_eq!(spec.display_name, "Sample Kernel (3.12)");
        assert_eq!(spec.language, "python");
        controller.update(&json!({"detail": null}));
        assert_eq!(controller.spec().display_name, "Sample Kernel");
    }

    #[test]
    fn mirrors_are_stable_per_file_and_keep_the_file_name() {
        let profile = std::env::temp_dir().join("graphite-nb-mirror-test");
        let a = mirror_path(&profile, Path::new("/w/one/data.sbook"));
        assert_eq!(a, mirror_path(&profile, Path::new("/w/one/data.sbook")));
        assert_ne!(a, mirror_path(&profile, Path::new("/w/two/data.sbook")));
        assert!(a.starts_with(profile.join("notebook-mirror")));
        assert_eq!(a.file_name().unwrap(), "data.sbook.ipynb");
    }

    #[test]
    fn execution_updates_accept_the_wrapped_and_the_plain_form() {
        assert_eq!(
            updates_of(&json!({"value": [{"editType": 3}], "buffers": []})).len(),
            1
        );
        assert_eq!(
            updates_of(&json!([{"editType": 3}, {"editType": 1}])).len(),
            2
        );
        assert!(updates_of(&Value::Null).is_empty());
    }
}
