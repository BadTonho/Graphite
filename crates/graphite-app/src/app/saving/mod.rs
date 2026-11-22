//! Per-document save coordination. Only the event loop reconciles worker results.
use super::command_execution::{CommandError, CommandJob};
use super::*;
use crate::file_format::{Encoding, LineEnding};
use std::sync::{mpsc, Arc};
mod coordinator;
mod lifecycle;
mod participants;
mod unsaved_modal;
mod writer;
pub(super) use lifecycle::DiscardAction;
#[cfg(test)]
mod interruption_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;

pub(super) struct Snapshot {
    pub base: String,
    pub text: String,
    pub revision: u64,
    pub path: PathBuf,
    pub encoding: Encoding,
    pub ending: LineEnding,
    #[cfg(test)]
    control: Option<Arc<writer::WriteControl>>,
}
struct Request {
    id: u64,
    origin: Option<graphite_text::ViewId>,
    owner: Option<u64>,
    revision: u64,
    format: (Encoding, LineEnding),
    reason: graphite_core_api::SaveReason,
}
struct Active {
    requests: Vec<Request>,
    snapshot: Snapshot,
    _lease: crate::document_store::DocumentLease,
    gate: writer::CommitGate,
    writing: bool,
    participants: participants::Participants,
    warnings: Vec<String>,
    reason: graphite_core_api::SaveReason,
}
pub(super) enum AfterSave {
    Command(CommandJob),
    Service {
        job: CommandJob,
        reason: graphite_core_api::SaveReason,
    },
    Workspace,
    Close {
        execution: u64,
        view: graphite_text::ViewId,
    },
    Window {
        execution: Option<u64>,
    },
    OpenEditorWindow {
        path: PathBuf,
        move_editor: bool,
    },
    Revert {
        document: DocumentId,
        execution: Option<u64>,
    },
    /// Commit once every listed document is on disk.
    Commit(crate::scm_commit::CommitOptions),
}
struct Batch {
    documents: Vec<DocumentId>,
    expected: HashMap<DocumentId, (u64, (Encoding, LineEnding))>,
    requests: Vec<u64>,
    action: AfterSave,
}
pub(crate) struct SaveRuntime {
    next: u64,
    invocation: u64,
    active: HashMap<DocumentId, Active>,
    queued: HashMap<DocumentId, Vec<Request>>,
    completed: HashMap<u64, Result<(), String>>,
    changed: std::collections::HashSet<u64>,
    outcomes: HashMap<u64, graphite_core_api::SaveOutcome>,
    reconciled: HashMap<u64, (DocumentId, u64, (Encoding, LineEnding))>,
    batches: Vec<Batch>,
    barriers: Vec<lifecycle::Barrier>,
    workspace: Option<lifecycle::WorkspaceTransition>,
    sender: mpsc::Sender<writer::Written>,
    receiver: mpsc::Receiver<writer::Written>,
}
impl Default for SaveRuntime {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            next: 0,
            invocation: 0,
            active: HashMap::new(),
            queued: HashMap::new(),
            completed: HashMap::new(),
            changed: Default::default(),
            outcomes: Default::default(),
            reconciled: Default::default(),
            batches: Vec::new(),
            barriers: Vec::new(),
            workspace: None,
            sender,
            receiver,
        }
    }
}
impl SaveRuntime {
    pub(crate) fn pending(&self) -> bool {
        !self.active.is_empty() || !self.batches.is_empty() || !self.barriers.is_empty()
    }
    pub(crate) fn transition_pending(&self) -> bool {
        self.workspace.is_some()
            || !self.barriers.is_empty()
            || self.batches.iter().any(|b| {
                matches!(
                    b.action,
                    AfterSave::Close { .. }
                        | AfterSave::Window { .. }
                        | AfterSave::Workspace
                        | AfterSave::OpenEditorWindow { .. }
                )
            })
    }
    pub(crate) fn document_pending(&self, document: DocumentId) -> bool {
        self.active.contains_key(&document)
    }
}
impl App {
    pub(crate) fn save_document(&mut self, document: DocumentId) -> u64 {
        self.record_local_history(document);
        self.saving.next += 1;
        let id = self.saving.next;
        let mut origin =
            (self.command_document() == document).then_some(self.command_view_ref().id);
        if origin.is_none() {
            self.editor_groups.for_each_view_mut(|_, view| {
                if origin.is_none() && view.document == document {
                    origin = Some(view.id);
                }
            });
        }
        let state = self
            .documents
            .get(document)
            .map(|d| (d.buffer.revision(), (d.encoding, d.line_ending)));
        let (revision, format) = state.unwrap_or((0, (Encoding::Utf8, LineEnding::Lf)));
        let request = Request {
            id,
            origin,
            owner: None,
            revision,
            format,
            reason: graphite_core_api::SaveReason::Manual,
        };
        if self.saving.active.contains_key(&document) {
            self.saving
                .queued
                .entry(document)
                .or_default()
                .push(request);
        } else {
            self.start_save(document, vec![request]);
        }
        id
    }
    fn start_save(&mut self, document: DocumentId, requests: Vec<Request>) {
        let Some(doc) = self.documents.get(document) else {
            for request in requests {
                self.saving
                    .completed
                    .insert(request.id, Err(self.t("save.document_closed").into()));
            }
            return;
        };
        let error = if doc.buffer.is_read_only() {
            Some(self.t("save.read_only"))
        } else if doc.buffer.path().is_none() {
            Some(self.t("save.no_file"))
        } else {
            None
        };
        if let Some(error) = error {
            let label = doc
                .buffer
                .path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| {
                    graphite_core_api::DocumentUri::untitled(document)
                        .as_str()
                        .to_owned()
                });
            let error = format!("{label}: {error}");
            self.status = self.tf("save.failed", &[("error", &error)]);
            for request in requests {
                self.saving.completed.insert(request.id, Err(error.clone()));
            }
            return;
        }
        let snapshot = Snapshot {
            base: doc.buffer.text(),
            text: doc.buffer.text(),
            revision: doc.buffer.revision(),
            path: doc.buffer.path().unwrap().into(),
            encoding: doc.encoding,
            ending: doc.line_ending,
            #[cfg(test)]
            control: None,
        };
        let participants = self.save_participants(document);
        let reason = requests
            .first()
            .map(|request| request.reason)
            .unwrap_or(graphite_core_api::SaveReason::Manual);
        self.saving.active.insert(
            document,
            Active {
                requests,
                snapshot,
                _lease: self.documents.lease(document).unwrap(),
                gate: Default::default(),
                writing: false,
                participants,
                warnings: Vec::new(),
                reason,
            },
        );
        self.status = self.t("save.saving").into();
    }
    pub(super) fn save_batch(&mut self, documents: Vec<DocumentId>, action: AfterSave) {
        let expected = match &action {
            AfterSave::Window { .. } | AfterSave::Workspace => self
                .documents
                .iter()
                .map(|d| (d.id, (d.buffer.revision(), (d.encoding, d.line_ending))))
                .collect(),
            AfterSave::Revert { document, .. } => self
                .documents
                .get(*document)
                .map(|d| {
                    HashMap::from([(
                        *document,
                        (d.buffer.revision(), (d.encoding, d.line_ending)),
                    )])
                })
                .unwrap_or_default(),
            AfterSave::Close { .. } | AfterSave::OpenEditorWindow { .. } | AfterSave::Commit(_) => {
                documents
                    .iter()
                    .filter_map(|id| {
                        self.documents
                            .get(*id)
                            .map(|d| (*id, (d.buffer.revision(), (d.encoding, d.line_ending))))
                    })
                    .collect()
            }
            _ => HashMap::new(),
        };
        let reason = match &action {
            AfterSave::Workspace => graphite_core_api::SaveReason::Workspace,
            AfterSave::Command(job) => {
                if self
                    .command_runtime
                    .pending
                    .get(&job.id)
                    .is_some_and(|p| p.command == "workbench.action.files.saveAll")
                {
                    graphite_core_api::SaveReason::SaveAll
                } else {
                    graphite_core_api::SaveReason::Manual
                }
            }
            AfterSave::Service { reason, .. } => *reason,
            AfterSave::Commit(_) => graphite_core_api::SaveReason::Manual,
            _ => graphite_core_api::SaveReason::Close,
        };
        let owner = match &action {
            AfterSave::Command(job) | AfterSave::Service { job, .. } => Some(job.id),
            AfterSave::Close { execution, .. } => Some(*execution),
            AfterSave::Window { execution } | AfterSave::Revert { execution, .. } => *execution,
            AfterSave::Workspace | AfterSave::OpenEditorWindow { .. } | AfterSave::Commit(_) => {
                None
            }
        };
        let requests: Vec<_> = documents
            .iter()
            .copied()
            .map(|document| self.save_document(document))
            .collect();
        for active in self.saving.active.values_mut() {
            for request in &mut active.requests {
                if requests.contains(&request.id) {
                    request.owner = owner;
                    request.reason = reason;
                    active.reason = reason;
                }
            }
        }
        for queued in self.saving.queued.values_mut() {
            for request in queued {
                if requests.contains(&request.id) {
                    request.owner = owner;
                    request.reason = reason;
                }
            }
        }
        self.saving.batches.push(Batch {
            documents,
            expected,
            requests,
            action,
        });
    }
}
