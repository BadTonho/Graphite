//! Nonblocking barriers for destructive decisions and workspace continuations.
use super::*;
pub(super) struct Expected {
    document: DocumentId,
    revision: u64,
    encoding: Encoding,
    ending: LineEnding,
}
pub(super) struct Barrier {
    expected: Vec<Expected>,
    action: DiscardAction,
}
pub(in crate::app) enum DiscardAction {
    Close {
        execution: u64,
        view: graphite_text::ViewId,
    },
    Window {
        execution: u64,
    },
    Workspace,
    Revert {
        document: DocumentId,
        execution: Option<u64>,
    },
}
pub(super) struct WorkspaceTransition {
    pub request: Option<crate::cli::CliRequest>,
    pub menu_roots: Option<(Vec<PathBuf>, Option<PathBuf>)>,
    pub acknowledge: Option<mpsc::Sender<()>>,
    prepared: HashMap<PathBuf, Workspace>,
}
impl App {
    pub(crate) fn defer_workspace_request(
        &mut self,
        request: crate::cli::CliRequest,
        acknowledge: Option<mpsc::Sender<()>>,
    ) {
        if self.saving.workspace.is_some() || self.close_window_confirm {
            self.status = self.t("workspace.decision_pending").into();
            if let Some(acknowledge) = acknowledge {
                let _ = acknowledge.send(());
            }
            return;
        }
        let mut prepared = HashMap::new();
        if let crate::cli::CliAction::Open { paths, .. } = &request.action {
            for path in paths.iter().filter(|path| path.is_dir()) {
                match Workspace::open(path) {
                    Ok(workspace) => {
                        prepared.insert(path.clone(), workspace);
                    }
                    Err(error) => {
                        self.status = self.tf("workspace.cannot_open", &[("error", &error)]);
                        if let Some(acknowledge) = acknowledge {
                            let _ = acknowledge.send(());
                        }
                        return;
                    }
                }
            }
        }
        self.saving.workspace = Some(WorkspaceTransition {
            request: Some(request),
            menu_roots: None,
            acknowledge,
            prepared,
        });
        self.close_window_confirm = true;
        self.status = self.t("workspace.switch_prompt").into();
    }
    pub(crate) fn defer_workspace_roots(
        &mut self,
        roots: Vec<PathBuf>,
        workspace_file: Option<PathBuf>,
    ) {
        if self.saving.workspace.is_some() || self.close_window_confirm {
            self.status = self.t("workspace.decision_pending").into();
            return;
        }
        self.saving.workspace = Some(WorkspaceTransition {
            request: None,
            menu_roots: Some((roots, workspace_file)),
            acknowledge: None,
            prepared: HashMap::new(),
        });
        self.close_window_confirm = true;
        self.status = self.t("workspace.switch_prompt").into();
    }
    pub(crate) fn workspace_decision_pending(&self) -> bool {
        self.saving.workspace.is_some()
    }
    pub(crate) fn resolve_workspace_save(&mut self, save: bool, cancel: bool) {
        self.close_window_confirm = false;
        if cancel {
            self.abort_workspace_transition(self.t("workspace.switch_cancelled"));
        } else if save {
            let documents = self
                .documents
                .iter()
                .filter(|d| d.is_dirty() || self.saving.document_pending(d.id))
                .map(|d| d.id)
                .collect();
            self.save_batch(documents, AfterSave::Workspace);
        } else {
            let documents = self.documents.ids().collect();
            self.defer_discard(documents, DiscardAction::Workspace);
        }
    }
    pub(super) fn abort_workspace_transition(&mut self, message: &str) {
        self.status = message.into();
        if let Some(transition) = self.saving.workspace.take() {
            if let Some(acknowledge) = transition.acknowledge {
                let _ = acknowledge.send(());
            }
        }
    }
    pub(super) fn complete_workspace_transition(&mut self) {
        if let Some(transition) = self.saving.workspace.take() {
            if let Some(request) = transition.request {
                self.apply_prepared_cli_request(
                    request,
                    transition.acknowledge,
                    transition.prepared,
                );
            } else if let Some((roots, workspace_file)) = transition.menu_roots {
                self.replace_workspace_roots(roots, workspace_file);
            }
        }
    }
    pub(in crate::app) fn defer_discard(
        &mut self,
        documents: Vec<DocumentId>,
        action: DiscardAction,
    ) {
        let expected = documents
            .iter()
            .filter_map(|id| {
                self.documents.get(*id).map(|doc| Expected {
                    document: *id,
                    revision: doc.buffer.revision(),
                    encoding: doc.encoding,
                    ending: doc.line_ending,
                })
            })
            .collect();
        for document in documents {
            if let Some(active) = self.saving.active.get(&document) {
                active.gate.cancel();
            }
            if let Some(queued) = self.saving.queued.remove(&document) {
                for request in queued {
                    self.saving
                        .completed
                        .insert(request.id, Err(self.t("save.cancelled_by_discard").into()));
                }
            }
        }
        self.saving.barriers.push(Barrier { expected, action });
        self.poll_save_barriers();
    }
    pub(super) fn poll_save_barriers(&mut self) {
        let barriers = std::mem::take(&mut self.saving.barriers);
        for barrier in barriers {
            if barrier
                .expected
                .iter()
                .any(|e| self.saving.document_pending(e.document))
            {
                self.saving.barriers.push(barrier);
                continue;
            }
            let new_dirty = matches!(
                &barrier.action,
                DiscardAction::Window { .. } | DiscardAction::Workspace
            ) && self.documents.iter().any(|d| {
                (d.is_dirty() || d.buffer.revision() > 0)
                    && !barrier.expected.iter().any(|e| e.document == d.id)
            });
            let unchanged = !new_dirty
                && barrier.expected.iter().all(|e| {
                    self.documents.get(e.document).is_some_and(|doc| {
                        doc.buffer.revision() == e.revision
                            && doc.encoding == e.encoding
                            && doc.line_ending == e.ending
                    })
                });
            let result = if unchanged {
                Ok(serde_json::Value::Null)
            } else {
                Err(CommandError::Failed(
                    self.t("workspace.changed_waiting").into(),
                ))
            };
            match barrier.action {
                DiscardAction::Close { execution, view } => {
                    if self.command_runtime.pending.contains_key(&execution)
                        && result.is_ok()
                        && self.editor_groups.view(view).is_some()
                    {
                        let previous = self.command_runtime.view.replace(view);
                        self.close_active_tab();
                        self.command_runtime.view = previous;
                    }
                    self.finish_command(execution, result);
                }
                DiscardAction::Window { execution } => {
                    if self.command_runtime.pending.contains_key(&execution) && result.is_ok() {
                        self.exit_requested = true;
                    }
                    self.finish_command(execution, result);
                }
                DiscardAction::Workspace => {
                    if let Err(error) = result {
                        self.abort_workspace_transition(&error.to_string());
                        continue;
                    }
                    if let Err(error) = self.discard_workspace_documents() {
                        self.abort_workspace_transition(&error);
                    } else {
                        self.complete_workspace_transition();
                    }
                }
                DiscardAction::Revert {
                    document,
                    execution,
                } => {
                    if let Err(error) = result {
                        self.status = error.to_string();
                    } else {
                        self.reload_document_from_disk(document);
                    }
                    self.pending_revert_confirmation = None;
                    self.close_window_confirm = false;
                    if let Some(execution) = execution {
                        self.finish_command(execution, Ok(serde_json::Value::Null));
                    }
                }
            }
        }
    }
    fn discard_workspace_documents(&mut self) -> Result<(), String> {
        if self.saving.workspace.is_none() {
            return Err(self.t("workspace.no_transition").into());
        }
        // Read every file before discarding any buffer; a failed read leaves all text intact.
        let mut reloads = Vec::new();
        let mut untitled = Vec::new();
        for doc in self.documents.iter().filter(|doc| doc.is_dirty()) {
            if let Some(path) = doc.buffer.path() {
                reloads.push((
                    path.to_path_buf(),
                    Ok(crate::file_format::read(path, &self.translator)
                        .map_err(|e| e.to_string())?),
                ));
            } else {
                untitled.push(doc.id);
            }
        }
        self.reload_after_branch_checkout(reloads);
        for document in untitled {
            self.documents[document].buffer.reload_text("");
            self.documents[document].saved_format = (
                self.documents[document].encoding,
                self.documents[document].line_ending,
            );
            let mut views = Vec::new();
            self.editor_groups.for_each_view_mut(|_, view| {
                if view.document == document {
                    views.push(view.id);
                }
            });
            for view in views {
                let previous = self.command_runtime.view.replace(view);
                self.close_active_tab();
                self.command_runtime.view = previous;
            }
        }
        Ok(())
    }
}
