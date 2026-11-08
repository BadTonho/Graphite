use super::*;

pub(crate) struct CliWaiter {
    paths: Vec<PathBuf>,
    waiting_for_diff_close: bool,
    acknowledge: std::sync::mpsc::Sender<()>,
}

impl App {
    pub(crate) fn apply_cli_request(
        &mut self,
        request: crate::cli::CliRequest,
        acknowledge: Option<std::sync::mpsc::Sender<()>>,
    ) {
        if let crate::cli::CliAction::Open { paths, .. } = &request.action {
            let different = paths.iter().any(|path| {
                path.is_dir() && path.canonicalize().ok() != self.tree.root().canonicalize().ok()
            });
            if different && (self.documents.iter().any(|d| d.is_dirty()) || self.saving.pending()) {
                self.defer_workspace_request(request, acknowledge);
                return;
            }
        }
        self.apply_prepared_cli_request(request, acknowledge, HashMap::new());
    }

    pub(in crate::app) fn apply_prepared_cli_request(
        &mut self,
        request: crate::cli::CliRequest,
        acknowledge: Option<std::sync::mpsc::Sender<()>>,
        mut prepared: HashMap<PathBuf, Workspace>,
    ) {
        let mut paths = Vec::new();
        let mut waiting_for_diff_close = false;
        match request.action {
            crate::cli::CliAction::Open {
                paths: requested,
                location,
            } => {
                for path in requested {
                    if path.is_dir() || prepared.contains_key(&path) {
                        if path.canonicalize().ok() != self.tree.root().canonicalize().ok() {
                            if self.documents.iter().any(|tab| tab.is_dirty()) {
                                self.status = format!(
                                    "Cannot open {} while documents have unsaved changes",
                                    path.display()
                                );
                            } else {
                                match prepared
                                    .remove(&path)
                                    .map(Ok)
                                    .unwrap_or_else(|| Workspace::open(&path))
                                {
                                    Ok(workspace) => {
                                        self.lsp_manager.change_workspace();
                                        let documents: Vec<_> = self.documents.ids().collect();
                                        for document in &documents {
                                            let doc = &mut self.documents[*document];
                                            doc.lsp_revision = None;
                                            doc.diagnostics.clear();
                                            doc.inlay_hints.clear();
                                            doc.inlay_revision = None;
                                        }
                                        self.tree.replace_workspace(workspace);
                                        for document in documents {
                                            self.sync_lsp_document(document);
                                        }
                                        self.status = format!("Opened folder {}", path.display());
                                    }
                                    Err(error) => {
                                        self.status = format!("Cannot open folder: {error}")
                                    }
                                }
                            }
                        }
                    } else {
                        self.open_file(path.clone());
                        paths.push(path);
                    }
                }
                if let Some((path, line, column)) = location {
                    self.open_file(path.clone());
                    let target = self.documents.id_for_path(&path);
                    if let Some(document) = target {
                        let tab = &mut self.documents[document];
                        let line = line
                            .saturating_sub(1)
                            .min(tab.buffer.len_lines().saturating_sub(1));
                        self.activate_editor_document(document);
                        self.editor_mut()
                            .set_cursor_line_col(line, column.saturating_sub(1));
                        self.reveal_cursor = true;
                    }
                    paths.push(path);
                }
            }
            crate::cli::CliAction::Diff { left, right } => {
                waiting_for_diff_close = true;
                self.open_diff_paths(left, right);
            }
        }
        if request.wait {
            if let Some(acknowledge) = acknowledge {
                self.cli_waiters.push(CliWaiter {
                    paths,
                    waiting_for_diff_close,
                    acknowledge,
                });
                self.check_cli_waiters();
                return;
            }
        }
        if let Some(acknowledge) = acknowledge {
            let _ = acknowledge.send(());
        }
    }

    pub(crate) fn check_cli_waiters(&mut self) {
        let tabs = &self.documents;
        let diff_open = self.diff_view.is_some() || self.diff_loading;
        let mut pending = Vec::new();
        for waiter in self.cli_waiters.drain(..) {
            let document_open = waiter.paths.iter().any(|path| {
                tabs.iter().any(|tab| {
                    tab.buffer
                        .path()
                        .is_some_and(|open| crate::same_file_path(open, path))
                })
            });
            if document_open || (waiter.waiting_for_diff_close && diff_open) {
                pending.push(waiter);
            } else {
                let _ = waiter.acknowledge.send(());
            }
        }
        self.cli_waiters = pending;
    }
}
