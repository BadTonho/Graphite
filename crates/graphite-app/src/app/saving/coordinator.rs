//! Worker reconciliation and completion of save consumers.
use super::*;
impl App {
    pub(crate) fn poll_saves(&mut self, now: std::time::Instant) {
        self.reap_commands();
        for active in self.saving.active.values_mut() {
            active.requests.retain(|r| {
                r.owner
                    .is_none_or(|id| self.command_runtime.pending.contains_key(&id))
            });
            if active.requests.is_empty() {
                active.gate.cancel();
            }
        }
        for queued in self.saving.queued.values_mut() {
            queued.retain(|r| {
                r.owner
                    .is_none_or(|id| self.command_runtime.pending.contains_key(&id))
            });
        }
        self.poll_save_participants(now);
        while let Ok(written) = self.saving.receiver.try_recv() {
            let Some(active) = self.saving.active.remove(&written.document) else {
                continue;
            };
            let document = written.document;
            let result = written
                .result
                .map_err(|error| format!("{}: {error}", active.snapshot.path.display()));
            if result.is_ok() {
                let snapshot = &active.snapshot;
                let doc = &self.documents[document];
                for request in &active.requests {
                    if request.revision != snapshot.revision
                        || request.format != (snapshot.encoding, snapshot.ending)
                        || doc.buffer.revision() != snapshot.revision
                        || (doc.encoding, doc.line_ending) != (snapshot.encoding, snapshot.ending)
                    {
                        self.saving.changed.insert(request.id);
                    }
                }
                let unchanged = doc.buffer.revision() == snapshot.revision
                    && doc.encoding == snapshot.encoding
                    && doc.line_ending == snapshot.ending
                    && !doc.buffer.is_read_only();
                if unchanged && snapshot.text != snapshot.base {
                    let edits = participants::text_diff(&snapshot.base, &snapshot.text);
                    let origin = active.requests.iter().filter_map(|r| r.origin).find(|id| {
                        self.editor_groups
                            .view(*id)
                            .is_some_and(|v| v.document == document)
                    });
                    if let Some(origin) = origin {
                        let previous = self.command_runtime.view.replace(origin);
                        self.editor_mut()
                            .apply_transaction(&edits)
                            .expect("validated save preparation");
                        self.command_runtime.view = previous;
                    } else {
                        let revision = self.documents[document].buffer.revision();
                        self.documents[document]
                            .buffer
                            .apply_transaction(&edits)
                            .expect("validated save preparation");
                        self.remap_document_views(document, revision);
                    }
                    // Already queued consumers accepted the source before this save's
                    // own preparation committed. That transaction is not new typing.
                    let revision = self.documents[document].buffer.revision();
                    for batch in &mut self.saving.batches {
                        if let Some((expected, format)) = batch.expected.get_mut(&document) {
                            if *expected == snapshot.revision
                                && *format == (snapshot.encoding, snapshot.ending)
                            {
                                *expected = revision;
                            }
                        }
                    }
                    if let Some(queued) = self.saving.queued.get_mut(&document) {
                        for request in queued {
                            if request.revision == snapshot.revision
                                && request.format == (snapshot.encoding, snapshot.ending)
                            {
                                request.revision = revision;
                            }
                        }
                    }
                }
                let doc = &mut self.documents[document];
                doc.buffer.mark_saved_snapshot(&snapshot.text);
                doc.saved_format = (snapshot.encoding, snapshot.ending);
                self.sync_lsp_document(document);
                self.lsp_manager.save(&snapshot.path, snapshot.text.clone());
                self.stage_saved_conflict(&snapshot.path);
                self.refresh_scm_async();
                self.status = if active.warnings.is_empty() {
                    self.t("save.saved").into()
                } else {
                    self.tf(
                        "save.saved_with_warnings",
                        &[("warnings", &active.warnings.join("; "))],
                    )
                };
            } else {
                self.status = self.tf("save.failed", &[("error", result.as_ref().unwrap_err())]);
            }
            for request in active.requests {
                if result.is_ok() {
                    let doc = &self.documents[document];
                    self.saving.reconciled.insert(
                        request.id,
                        (
                            document,
                            doc.buffer.revision(),
                            (doc.encoding, doc.line_ending),
                        ),
                    );
                    self.saving.outcomes.insert(
                        request.id,
                        graphite_core_api::SaveOutcome {
                            document,
                            version: active.snapshot.revision,
                            dirty: self.documents[document].is_dirty(),
                            warnings: active.warnings.clone(),
                        },
                    );
                }
                self.saving.completed.insert(request.id, result.clone());
            }
            if let Some(queued) = self
                .saving
                .queued
                .remove(&document)
                .filter(|queued| !queued.is_empty())
            {
                self.start_save(document, queued);
            }
        }
        let batches = std::mem::take(&mut self.saving.batches);
        for batch in batches {
            let owner = match &batch.action {
                AfterSave::Command(job) | AfterSave::Service { job, .. } => Some(job.id),
                AfterSave::Close { execution, .. } => Some(*execution),
                AfterSave::Window { execution } => *execution,
                AfterSave::Workspace => None,
            };
            if owner.is_some_and(|id| !self.command_runtime.pending.contains_key(&id)) {
                continue;
            }
            let lifecycle = !matches!(
                &batch.action,
                AfterSave::Command(_) | AfterSave::Service { .. }
            );
            let global_transition = matches!(
                &batch.action,
                AfterSave::Window { .. } | AfterSave::Workspace
            );
            let waiting_for_write = if global_transition {
                !self.saving.active.is_empty()
            } else {
                lifecycle
                    && batch
                        .documents
                        .iter()
                        .any(|document| self.saving.document_pending(*document))
            };
            if waiting_for_write
                || !batch
                    .requests
                    .iter()
                    .all(|id| self.saving.completed.contains_key(id))
            {
                self.saving.batches.push(batch);
                continue;
            }
            let errors: Vec<_> = batch
                .requests
                .iter()
                .filter_map(|id| self.saving.completed.remove(id).unwrap().err())
                .collect();
            let mut result = if errors.is_empty() {
                Ok(serde_json::Value::Null)
            } else {
                Err(CommandError::Failed(errors.join("; ")))
            };
            let guard_changed = lifecycle
                && (batch.expected.iter().any(|(id, (revision, format))| {
                    self.documents.get(*id).is_none_or(|d| {
                        d.buffer.revision() != *revision || (d.encoding, d.line_ending) != *format
                    })
                }) || (global_transition
                    && self.documents.iter().any(|d| {
                        !batch.expected.contains_key(&d.id)
                            && (d.is_dirty() || d.buffer.revision() > 0)
                    })));
            let changed = guard_changed
                || batch.requests.iter().any(|id| {
                    self.saving.changed.contains(id)
                        || self.saving.reconciled.get(id).is_some_and(
                            |(document, revision, format)| {
                                self.documents.get(*document).is_none_or(|d| {
                                    d.buffer.revision() != *revision
                                        || (d.encoding, d.line_ending) != *format
                                })
                            },
                        )
                });
            if changed
                && !matches!(
                    &batch.action,
                    AfterSave::Command(_) | AfterSave::Service { .. }
                )
            {
                result = Err(CommandError::Failed(
                    self.t("save.changed_kept_open").into(),
                ));
            }
            match batch.action {
                AfterSave::Workspace => {
                    if result.is_ok() && !self.documents.iter().any(|d| d.is_dirty()) {
                        self.complete_workspace_transition();
                    } else {
                        self.abort_workspace_transition(self.t("workspace.switch_failed"));
                    }
                }
                AfterSave::Command(job) => {
                    self.finish_command(job.id, result);
                }
                AfterSave::Service { job, .. } => {
                    if result.is_ok() {
                        result = serde_json::to_value(self.saving.outcomes.get(&batch.requests[0]))
                            .map_err(|e| CommandError::Failed(e.to_string()));
                    }
                    self.finish_command(job.id, result);
                }
                AfterSave::Close { execution, view } => {
                    if !self.command_runtime.pending.contains_key(&execution) {
                        continue;
                    }
                    if let Some(target) = self.editor_groups.view(view) {
                        if result.is_ok() && self.documents[target.document].is_dirty() {
                            result = Err(CommandError::Failed(
                                self.t("save.document_changed_kept_open").into(),
                            ));
                        }
                        if result.is_ok() {
                            let previous = self.command_runtime.view.replace(view);
                            self.close_active_tab();
                            self.command_runtime.view = previous;
                        }
                    } else {
                        result = Err(CommandError::TargetClosed);
                    }
                    self.finish_command(execution, result);
                }
                AfterSave::Window { execution } => {
                    if result.is_ok() && self.documents.iter().any(|d| d.is_dirty()) {
                        result = Err(CommandError::Failed(
                            self.t("save.changed_kept_open").into(),
                        ));
                    }
                    if execution.is_none_or(|id| self.command_runtime.pending.contains_key(&id)) {
                        self.exit_requested = result.is_ok();
                    }
                    if let Err(error) = &result {
                        self.status = self.tf("save.cannot_close", &[("error", error)]);
                    }
                    self.close_window_confirm = false;
                    if let Some(execution) = execution {
                        self.finish_command(execution, result);
                    }
                }
            }
        }
        self.poll_save_barriers();
        // Unobserved native requests have no consumer ticket.
        let observed: std::collections::HashSet<_> = self
            .saving
            .batches
            .iter()
            .flat_map(|b| b.requests.iter().copied())
            .collect();
        self.saving.completed.retain(|id, _| observed.contains(id));
        self.saving.changed.retain(|id| observed.contains(id));
        self.saving.outcomes.retain(|id, _| observed.contains(id));
        self.saving.reconciled.retain(|id, _| observed.contains(id));
    }
}
