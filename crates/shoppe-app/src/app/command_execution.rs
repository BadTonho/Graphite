//! Tickets and cooperative cancellation never block the owning event loop.
use super::command_registry::{registry, ArgumentKind, CommandTarget};
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CommandRequest {
    pub id: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub target: CommandTarget,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub(crate) enum CommandError {
    UnknownCommand(String),
    InvalidArguments(String),
    InvalidTarget(String),
    TargetClosed,
    Disabled(String),
    Cancelled,
    Failed(String),
    StaleDocument,
}
impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCommand(id) => write!(f, "Unknown command: {id}"),
            Self::InvalidArguments(message)
            | Self::InvalidTarget(message)
            | Self::Failed(message) => f.write_str(message),
            Self::Disabled(message) => write!(f, "Command disabled: {message}"),
            Self::TargetClosed => f.write_str("Command target closed"),
            Self::Cancelled => f.write_str("Command cancelled"),
            Self::StaleDocument => f.write_str("Document changed while command was pending"),
        }
    }
}
pub(crate) type CommandResult = Result<Value, CommandError>;
#[derive(Clone, Debug, Default)]
pub(crate) struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), CommandError> {
        if self.is_cancelled() {
            Err(CommandError::Cancelled)
        } else {
            Ok(())
        }
    }
}
// Exposed to programmatic consumers, including cancellation without UI input.
#[allow(dead_code)]
pub(crate) struct CommandTicket {
    pub id: u64,
    pub cancellation: CancellationToken,
    receiver: Receiver<CommandResult>,
}
impl CommandTicket {
    pub fn try_result(&self) -> Option<CommandResult> {
        self.receiver.try_recv().ok()
    }
}
pub(crate) struct CommandInvocation {
    pub request: CommandRequest,
    pub execution: u64,
    pub cancellation: CancellationToken,
}
pub(crate) struct PendingCommand {
    pub target: CommandTarget,
    pub command: String,
    pub workspace: PathBuf,
    pub revision: Option<(DocumentId, u64)>,
    pub workspace_revisions: Option<Vec<(DocumentId, u64)>>,
    pub cancellation: CancellationToken,
    pub deferred: bool,
    pub sender: Sender<CommandResult>,
}
#[derive(Clone, Debug)]
pub(crate) struct CommandJob {
    pub id: u64,
    pub cancellation: CancellationToken,
}
pub(crate) fn deliver_command_event(
    proxy: &winit::event_loop::EventLoopProxy<UserEvent>,
    job: Option<&CommandJob>,
    event: UserEvent,
    result: CommandResult,
) {
    let event = if let Some(job) = job {
        UserEvent::CommandCompleted {
            execution: job.id,
            event: Some(Box::new(event)),
            result,
        }
    } else {
        event
    };
    let _ = proxy.send_event(event);
}
#[derive(Default)]
pub(crate) struct CommandRuntime {
    pub next: u64,
    pub pending: HashMap<u64, PendingCommand>,
    pub current: Option<u64>,
    pub view: Option<graphite_text::ViewId>,
    pub lsp_jobs: HashMap<u64, (CommandJob, PathBuf)>,
    pub pending_close: Option<(u64, graphite_text::ViewId)>,
    pub pending_window_close: Option<u64>,
}

impl App {
    pub(super) fn track_lsp_command(&mut self, request_id: u64, path: &Path) {
        if let Some(job) = self.command_job(true) {
            self.command_runtime
                .lsp_jobs
                .insert(request_id, (job, path.to_path_buf()));
        }
    }
    pub(super) fn command_job(&mut self, version_sensitive: bool) -> Option<CommandJob> {
        let id = self.command_runtime.current?;
        let revision = self
            .command_runtime
            .view
            .and_then(|id| self.editor_groups.view(id))
            .map(|view| {
                (
                    view.document,
                    self.documents[view.document].buffer.revision(),
                )
            });
        let workspace_revisions = self
            .documents
            .iter()
            .map(|doc| (doc.id, doc.buffer.revision()))
            .collect();
        let pending = self.command_runtime.pending.get_mut(&id)?;
        pending.deferred = true;
        if pending.command == "shoppe.scm.checkout" {
            pending.workspace_revisions = Some(workspace_revisions);
        }
        if matches!(
            pending.command.as_str(),
            "quickFix.apply" | "shoppe.scm.checkout" | "shoppe.conflict.resolve"
        ) {
            // Consuming a popup hands the continuation to its owning view/workspace.
            pending.target = self
                .command_runtime
                .view
                .map(|id| CommandTarget::View(id.0))
                .unwrap_or(CommandTarget::Global);
        }
        pending.revision = if version_sensitive { revision } else { None };
        Some(CommandJob {
            id,
            cancellation: pending.cancellation.clone(),
        })
    }
    pub(super) fn fail_current_command(&mut self, error: CommandError) {
        self.status = error.to_string();
        if let Some(id) = self.command_runtime.current {
            self.finish_command(id, Err(error));
        }
    }
    pub(crate) fn accept_command_event(
        &mut self,
        execution: u64,
        event: Option<Box<UserEvent>>,
        result: CommandResult,
    ) {
        self.refresh_command_context();
        self.reap_commands();
        let Some(pending) = self.command_runtime.pending.get(&execution) else {
            return;
        };
        if pending
            .workspace_revisions
            .as_ref()
            .is_some_and(|expected| {
                expected.len() != self.documents.len()
                    || expected.iter().any(|(id, revision)| {
                        self.documents
                            .get(*id)
                            .is_none_or(|doc| doc.buffer.revision() != *revision)
                    })
            })
        {
            self.finish_command(execution, Err(CommandError::StaleDocument));
            return;
        }
        if pending.revision.is_some_and(|(id, revision)| {
            self.documents
                .get(id)
                .is_none_or(|doc| doc.buffer.revision() != revision)
        }) {
            self.finish_command(execution, Err(CommandError::StaleDocument));
            return;
        }
        if pending.workspace != self.tree.root() {
            self.finish_command(execution, Err(CommandError::TargetClosed));
            return;
        }
        let target = pending.target;
        if result.is_ok() {
            let view = self.command_target_view(target).ok().flatten();
            let previous = std::mem::replace(&mut self.command_runtime.view, view);
            if let Some(event) = event {
                self.handle_user_event(*event);
            }
            self.refresh_command_context();
            self.command_runtime.view = previous;
        }
        self.finish_command(execution, result);
        self.refresh_command_context();
    }
    pub(crate) fn execute_command(&mut self, request: CommandRequest) -> CommandTicket {
        self.execute_command_with_cancellation(request, CancellationToken::default())
    }
    pub(crate) fn execute_command_with_cancellation(
        &mut self,
        request: CommandRequest,
        cancellation: CancellationToken,
    ) -> CommandTicket {
        self.refresh_command_context();
        self.command_runtime.next += 1;
        let id = self.command_runtime.next;
        let (sender, receiver) = mpsc::channel();
        let ticket = CommandTicket {
            id,
            cancellation: cancellation.clone(),
            receiver,
        };
        let validate = || {
            cancellation.check()?;
            let entry = registry()
                .get(&request.id)
                .ok_or_else(|| CommandError::UnknownCommand(request.id.clone()))?;
            if !entry.targets.contains(&request.target.kind()) {
                return Err(CommandError::InvalidTarget(request.id.clone()));
            }
            let view = self.command_target_view(request.target)?;
            if let Some(kind) = entry.widget {
                if !matches!(request.target, CommandTarget::Widget(id) if self.context_keys.widget(id).is_some_and(|scope| scope.kind == kind))
                {
                    return Err(CommandError::InvalidTarget(
                        "Command requires its own widget instance".into(),
                    ));
                }
            }
            validate_arguments(entry.arguments, &request.arguments)?;
            if matches!(entry.arguments, ArgumentKind::Index) {
                let index = request.arguments["index"].as_u64().unwrap();
                let valid = match request.id.as_str() {
                    "shoppe.problems.open" => index < self.diagnostic_entries().len() as u64,
                    "shoppe.terminal.select" => index < self.terminals.len() as u64,
                    "shoppe.scm.select" => index < self.scm_snapshot.files.len() as u64,
                    "shoppe.conflict.resolve" => {
                        index <= 3
                            && self.conflict_view.as_ref().is_some_and(|conflict| {
                                index == 3 || conflict.sides[index as usize].is_some()
                            })
                    }
                    _ => usize::try_from(index).is_ok(),
                };
                if !valid {
                    return Err(CommandError::InvalidArguments("Index out of range".into()));
                }
            }
            let context = self.context_keys.resolve_target(request.target);
            if !registry().enabled(&request.id, &context) {
                return Err(CommandError::Disabled(request.id.clone()));
            }
            Ok((entry.clone(), view))
        };
        let (entry, view) = match validate() {
            Ok(value) => value,
            Err(error) => {
                let _ = sender.send(Err(error));
                return ticket;
            }
        };
        let revision = view.and_then(|id| self.editor_groups.view(id)).map(|view| {
            (
                view.document,
                self.documents[view.document].buffer.revision(),
            )
        });
        self.command_runtime.pending.insert(
            id,
            PendingCommand {
                target: request.target,
                command: request.id.clone(),
                workspace: self.tree.root().to_path_buf(),
                revision,
                workspace_revisions: None,
                cancellation: cancellation.clone(),
                deferred: false,
                sender,
            },
        );
        let previous_execution = self.command_runtime.current.replace(id);
        let previous_view = std::mem::replace(&mut self.command_runtime.view, view);
        if !request.id.starts_with("suggest.") && request.id != "editor.action.triggerSuggest" {
            self.completion_items.clear();
        }
        let invocation = CommandInvocation {
            request,
            execution: id,
            cancellation,
        };
        let result = (entry.handler)(self, &invocation);
        self.refresh_command_context();
        self.command_runtime.view = previous_view;
        self.command_runtime.current = previous_execution;
        if let Err(error) = result {
            self.finish_command(id, Err(error));
        } else if self
            .command_runtime
            .pending
            .get(&id)
            .is_some_and(|pending| !pending.deferred)
        {
            self.finish_command(id, Ok(Value::Null));
        }
        self.refresh_command_context();
        self.reap_commands();
        ticket
    }
    pub(crate) fn finish_command(&mut self, id: u64, result: CommandResult) {
        if self
            .command_runtime
            .pending_close
            .is_some_and(|(execution, _)| execution == id)
        {
            self.command_runtime.pending_close = None;
            self.close_confirm = false;
        }
        if self.command_runtime.pending_window_close == Some(id) {
            self.command_runtime.pending_window_close = None;
            self.close_window_confirm = false;
        }

        let lsp: Vec<_> = self
            .command_runtime
            .lsp_jobs
            .iter()
            .filter(|(_, (job, _))| job.id == id)
            .map(|(request, (_, path))| (*request, path.clone()))
            .collect();
        for (request, path) in lsp {
            self.lsp_manager.cancel_request(&path, request);
            self.command_runtime.lsp_jobs.remove(&request);
        }
        if let Some(pending) = self.command_runtime.pending.remove(&id) {
            if pending.command == "shoppe.action.diffActiveFile" {
                self.diff_loading = false;
            }
            let result = if pending.cancellation.is_cancelled() && result.is_ok() {
                Err(CommandError::Cancelled)
            } else {
                result
            };
            if let Err(error) = &result {
                self.status = error.to_string();
            }
            let _ = pending.sender.send(result);
        }
    }
    #[allow(dead_code)] // ID-based API complements cancelling a ticket token.
    pub(crate) fn cancel_command(&mut self, id: u64) {
        if let Some(pending) = self.command_runtime.pending.get(&id) {
            pending.cancellation.cancel();
        }
        self.finish_command(id, Err(CommandError::Cancelled));
    }
    pub(crate) fn run_view_command_args(
        &mut self,
        id: &str,
        view: graphite_text::ViewId,
        arguments: Value,
    ) {
        let ticket = self.execute_command(CommandRequest {
            id: id.into(),
            arguments,
            target: CommandTarget::View(view.0),
        });
        if let Some(Err(error)) = ticket.try_result() {
            self.status = error.to_string();
        }
    }
    pub(crate) fn run_ui_command_args(&mut self, id: &str, arguments: Value) {
        self.refresh_command_context();
        let target = self.ui_target_for_command(id);
        let ticket = self.execute_command(CommandRequest {
            id: id.into(),
            arguments,
            target,
        });
        if let Some(Err(error)) = ticket.try_result() {
            self.status = error.to_string();
        }
    }
    pub(crate) fn run_ui_command(&mut self, id: &str) {
        self.refresh_command_context();
        let target = self.ui_target_for_command(id);
        let ticket = self.execute_command(CommandRequest {
            id: id.into(),
            arguments: Value::Null,
            target,
        });
        if let Some(Err(error)) = ticket.try_result() {
            self.status = error.to_string();
        }
    }
    pub(crate) fn reap_commands(&mut self) {
        let dead: Vec<_> = self
            .command_runtime
            .pending
            .iter()
            .filter_map(|(id, pending)| {
                if pending.cancellation.is_cancelled() {
                    Some((*id, CommandError::Cancelled))
                } else if pending.workspace != self.tree.root()
                    || self.command_target_view(pending.target).is_err()
                {
                    Some((*id, CommandError::TargetClosed))
                } else {
                    None
                }
            })
            .collect();
        for (id, error) in dead {
            if let Some(pending) = self.command_runtime.pending.get(&id) {
                pending.cancellation.cancel();
            }
            self.finish_command(id, Err(error));
        }
    }
}
fn validate_arguments(kind: ArgumentKind, value: &Value) -> Result<(), CommandError> {
    let valid = match kind {
        ArgumentKind::None => value.is_null(),
        ArgumentKind::Text => value
            .as_object()
            .is_some_and(|o| o.len() == 1 && o.get("text").is_some_and(Value::is_string)),
        ArgumentKind::Index => value
            .as_object()
            .is_some_and(|o| o.len() == 1 && o.get("index").and_then(Value::as_u64).is_some()),
        ArgumentKind::Selection => value.as_object().is_some_and(|o| {
            o.len() == 2
                && o.get("anchor").and_then(Value::as_u64).is_some()
                && o.get("head").and_then(Value::as_u64).is_some()
        }),
        ArgumentKind::Position => value.as_object().is_some_and(|o| {
            o.len() == 2
                && o.get("line").and_then(Value::as_u64).is_some()
                && o.get("column").and_then(Value::as_u64).is_some()
        }),
    };
    if valid {
        Ok(())
    } else {
        Err(CommandError::InvalidArguments(
            "Invalid command arguments".into(),
        ))
    }
}
