use super::*;

impl App {
    pub(super) fn sync_lsp_document(&mut self, document: DocumentId) {
        if document == self.active_document() {
            self.sync_schema_active();
        }
        let Ok(snapshot) = super::services::snapshot_document(document, &self.documents[document])
        else {
            return;
        };
        let Ok(path) = snapshot.uri.to_path() else {
            return;
        };
        if !self.lsp_manager.supports_path(&path) {
            return;
        }
        let revision = snapshot.version.unwrap();
        let opened = self.documents[document].lsp_revision;
        if opened == Some(revision) {
            return;
        }
        use graphite_core_api::WorkspaceService;
        let Ok(workspace) = self.workspace() else {
            return;
        };
        if super::services::lsp::publish_document(
            &mut self.lsp_manager,
            &snapshot,
            &workspace,
            self.lsp_sender.clone(),
            opened.is_some(),
            self.translator,
        )
        .is_ok()
        {
            self.documents[document].lsp_revision = Some(revision);
            if document == self.active_document() {
                self.request_inlay_hints();
            }
        }
    }
    pub(crate) fn register_lsp_active(&mut self) {
        self.sync_lsp_document(self.active_document());
    }
    pub(crate) fn sync_lsp_active(&mut self) {
        self.sync_lsp_document(self.active_document());
    }

    pub(crate) fn process_lsp_events(&mut self) {
        let translator = self.translator;
        let replaced: Vec<_> = self
            .command_runtime
            .lsp_jobs
            .values()
            .filter(|job| {
                job.instance.is_some()
                    && job.instance != self.lsp_manager.server_instance_for(&job.path)
            })
            .map(|job| job.execution.id)
            .collect();
        for id in replaced {
            self.finish_command(
                id,
                Err(super::command_execution::CommandError::Failed(
                    translator.text("lsp.instance_changed").into(),
                )),
            );
        }

        let events = self
            .lsp_queue
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for event in events {
            let event = match event {
                LspUiEvent::Instance {
                    extension,
                    instance,
                    event,
                } => {
                    if !self.lsp_manager.current_instance(&extension, instance) {
                        continue;
                    }
                    *event
                }
                event => event,
            };
            if let LspUiEvent::ServerStopped { extension } = &event {
                self.lsp_manager.retire_instance(extension);
                for (_, document) in self.documents.iter_mut() {
                    if document.buffer.path().is_some_and(|path| {
                        self.lsp_manager.server_extension_for(path).as_ref() == Some(extension)
                    }) {
                        document.lsp_revision = None;
                        document.inlay_hints.clear();
                        document.inlay_revision = None;
                        document.diagnostics.clear();
                    }
                }
                let source_matches = |kind| {
                    self.context_keys
                        .widget_id(kind)
                        .and_then(|id| self.context_keys.widget(id))
                        .and_then(|scope| scope.parent)
                        .and_then(|view| self.editor_groups.view(view))
                        .and_then(|view| self.documents[view.document].buffer.path())
                        .is_some_and(|path| {
                            self.lsp_manager.server_extension_for(path).as_ref() == Some(extension)
                        })
                };
                if source_matches(super::context_keys::WidgetKind::QuickFix) {
                    self.quick_fix_visible = false;
                    self.quick_fix_actions.clear();
                }
                if source_matches(super::context_keys::WidgetKind::Suggest) {
                    self.completion_items.clear();
                }

                let executions: Vec<_> = self
                    .command_runtime
                    .lsp_jobs
                    .values()
                    .filter(|job| {
                        self.lsp_manager.server_extension_for(&job.path).as_ref() == Some(extension)
                    })
                    .map(|job| job.execution.id)
                    .collect();
                for execution in executions {
                    self.finish_command(
                        execution,
                        Err(super::command_execution::CommandError::Failed(
                            translator.text("lsp.server_stopped").into(),
                        )),
                    );
                }
                continue;
            }
            let request = match &event {
                LspUiEvent::Completion { request_id, .. }
                | LspUiEvent::Definition { request_id, .. }
                | LspUiEvent::Hover { request_id, .. }
                | LspUiEvent::JsonCompletion { request_id, .. }
                | LspUiEvent::CodeActions { request_id, .. }
                | LspUiEvent::RequestFailed { request_id, .. }
                | LspUiEvent::RequestRejected { request_id, .. }
                | LspUiEvent::ServerCommandCompleted { request_id, .. }
                | LspUiEvent::Query { request_id, .. } => Some(*request_id),
                _ => None,
            };
            if let Some(request) = request {
                if let Some(job) = self.command_runtime.lsp_jobs.remove(&request) {
                    let result = match &event {
                        LspUiEvent::RequestRejected { error, .. } => Err(match error {
                            graphite_core_api::ServiceError::Timeout => {
                                super::command_execution::CommandError::Timeout
                            }
                            graphite_core_api::ServiceError::Busy => {
                                super::command_execution::CommandError::Busy
                            }
                            _ => super::command_execution::CommandError::Failed(error.to_string()),
                        }),
                        LspUiEvent::RequestFailed { message, .. } => Err(
                            super::command_execution::CommandError::Failed(message.clone()),
                        ),
                        LspUiEvent::CodeActions {
                            error: Some(message),
                            ..
                        } => Err(super::command_execution::CommandError::Failed(
                            message.clone(),
                        )),
                        LspUiEvent::ServerCommandCompleted { result, .. } => result
                            .clone()
                            .map_err(super::command_execution::CommandError::Failed),
                        LspUiEvent::Completion { items, .. }
                        | LspUiEvent::JsonCompletion { items, .. } => {
                            Ok(serde_json::to_value(items).unwrap())
                        }
                        LspUiEvent::Hover { text, .. } => Ok(serde_json::to_value(text).unwrap()),
                        LspUiEvent::Definition { location, .. } => {
                            Ok(serde_json::to_value(location).unwrap())
                        }
                        LspUiEvent::Query { result, .. } => Ok(result.clone()),
                        LspUiEvent::CodeActions { actions, .. } => {
                            Ok(serde_json::to_value(actions).unwrap())
                        }
                        _ => Ok(serde_json::Value::Null),
                    };
                    self.accept_command_event(
                        job.execution.id,
                        Some(Box::new(UserEvent::LspCommandEvent(Box::new(event)))),
                        result,
                    );
                    continue;
                }
                // Every user query has an execution. Missing correlation means
                // it finished or was cancelled; late/duplicate replies are inert.
                continue;
            }
            self.apply_lsp_ui_event(event, false);
        }
    }
    pub(crate) fn apply_lsp_ui_event(&mut self, event: LspUiEvent, tracked: bool) {
        let translator = self.translator;
        match event {
            LspUiEvent::ServiceRequest {
                id,
                method,
                params,
                reply,
                active,
            } => {
                if !active.swap(false, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                let result = if method == "window/showMessageRequest" {
                    use graphite_core_api::WindowService;
                    self.show_message(
                        params
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .into(),
                    )
                    .map(|_| serde_json::Value::Null)
                } else {
                    super::services::lsp::workspace_request(self, &method, &params)
                };
                let response = match result {
                    Ok(value) => lsp_server::Response::new_ok(id, value),
                    Err(error) => lsp_server::Response::new_err(id, -32603, error.to_string()),
                };
                let _ = reply.send(response);
            }
            LspUiEvent::ServerStopped { .. } => {}
            LspUiEvent::RequestFailed { message, .. } => self.status = message,
            LspUiEvent::ServerCommandCompleted { result, .. } => {
                self.status = result
                    .map(|_| translator.text("lsp.command.completed").to_string())
                    .unwrap_or_else(|e| translator.format("lsp.command.failed", &[("error", &e)]));
            }
            LspUiEvent::Status(status) => {
                use graphite_core_api::WindowService;
                let _ = self.show_message(status);
            }
            LspUiEvent::ApplyEdit {
                id,
                edit,
                reply,
                active,
            } => {
                if !active.swap(false, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                let result = self.apply_workspace_edit(edit);
                let payload = match result {
                    Ok(()) => serde_json::json!({ "applied": true }),
                    Err(error) => {
                        serde_json::json!({ "applied": false, "failureReason": error })
                    }
                };
                let _ = reply.send(lsp_server::Response {
                    id,
                    result: Some(payload),
                    error: None,
                });
            }
            LspUiEvent::Diagnostics {
                uri,
                version,
                diagnostics,
            } => {
                if let Some(path) = lsp::uri_to_path(&uri) {
                    let index = self.documents.id_for_path(&path);
                    let active = self.command_document();
                    if let Some(index) = index {
                        let tab = &mut self.documents[index];
                        if !lsp::diagnostics_are_current(tab.lsp_revision, version) {
                            return;
                        }
                        tab.diagnostics = diagnostics;
                        if index == active {
                            let count = tab.diagnostics.len() + tab.schema_diagnostics.len();
                            self.status =
                                translator.format("lsp.diagnostics.count", &[("count", &count)]);
                        }
                    }
                }
            }
            LspUiEvent::Completion { request_id, items }
                if (tracked || request_id == self.lsp_request_id) =>
            {
                self.completion_items = items;
                self.completion_index = 0;
            }
            LspUiEvent::Definition {
                request_id,
                location,
            } if (tracked || request_id == self.lsp_request_id) => {
                if self.lsp_peek_request == Some(request_id) {
                    self.lsp_peek_request = None;
                    self.show_lsp_peek(location);
                } else if let Some(location) = location {
                    if let Some(path) = lsp::uri_to_path(location.uri.as_str()) {
                        self.open_file(path);
                        let line = location.range.start.line as usize;
                        let col = utf16_column_to_chars(
                            &self.editor().line(line),
                            location.range.start.character,
                        );
                        self.editor_mut().set_cursor_line_col(line, col);
                        self.push_navigation_location();
                        self.status = translator.text("lsp.definition.opened").into();
                    }
                } else {
                    self.status = translator.text("lsp.definition.none").into();
                }
            }
            LspUiEvent::Query { kind, result, .. } => {
                self.handle_lsp_query(kind, result);
            }
            LspUiEvent::Hover { request_id, text }
                if (tracked || request_id == self.lsp_request_id) =>
            {
                self.hover_text = text;
            }
            LspUiEvent::InlayHints { request_id, hints }
                if (tracked || request_id == self.inlay_request_id) =>
            {
                let document = self.command_document();
                let tab = &mut self.documents[document];
                tab.inlay_hints = hints;
                tab.inlay_revision = Some(tab.buffer.revision());
            }
            LspUiEvent::CodeActions {
                request_id,
                actions,
                error,
            } if (tracked || request_id == self.lsp_request_id) => {
                self.quick_fix_actions = actions;
                self.quick_fix_index = 0;
                self.quick_fix_visible = true;
                self.status = if let Some(error) = error {
                    self.quick_fix_visible = false;
                    translator.format("lsp.quick_fix.request_failed", &[("error", &error)])
                } else if self.quick_fix_actions.is_empty() {
                    translator.text("lsp.quick_fix.none").into()
                } else {
                    translator.text("lsp.quick_fix.prompt").into()
                };
            }
            LspUiEvent::JsonDiagnostics {
                uri,
                revision,
                diagnostics,
            } => {
                if let Some(path) = lsp::uri_to_path(&uri) {
                    if let Some((_, tab)) = self.documents.iter_mut().find(|(_, tab)| {
                        tab.buffer
                            .path()
                            .is_some_and(|open| same_file_path(open, &path))
                    }) {
                        if tab.buffer.revision() == revision {
                            tab.schema_revision = Some(revision);
                            tab.schema_diagnostics = diagnostics;
                        }
                    }
                }
            }
            LspUiEvent::JsonCompletion { request_id, items }
                if (tracked || request_id == self.lsp_request_id) =>
            {
                self.completion_items = items;
                self.completion_index = 0;
                if self.completion_items.is_empty() {
                    self.status = translator.text("lsp.completion.none").into();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn active_lsp_position(&self) -> Option<(PathBuf, lsp_types::Position)> {
        let snapshot = super::services::snapshot_document(
            self.command_document(),
            &self.documents[self.command_document()],
        )
        .ok()?;
        let path = snapshot.uri.to_path().ok()?;
        let char_index = self.editor().cursor().char_idx;
        Some((path, lsp::position_at_char(&snapshot.text, char_index)))
    }

    pub(crate) fn editor_state(&self) -> (DocumentId, usize, u64, Vec<graphite_text::Selection>) {
        let tab = &self.documents[self.active_document()];
        (
            self.active_document(),
            self.editor().cursor().char_idx,
            tab.buffer.revision(),
            self.editor().selections(),
        )
    }

    pub(crate) fn invalidate_lsp_requests(&mut self) {
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.inlay_request_id = self.inlay_request_id.wrapping_add(1);
        self.completion_items.clear();
        self.hover_text = None;
    }

    pub(crate) fn request_inlay_hints(&mut self) {
        let tab = &self.documents[self.active_document()];
        let Some(path) = tab.buffer.path().map(PathBuf::from) else {
            return;
        };
        if !self.lsp_manager.supports_path(&path) {
            return;
        }
        let text = tab.buffer.text();
        let end = lsp::position_at_char(&text, text.chars().count());
        self.inlay_request_id = self.inlay_request_id.wrapping_add(1);
        let document = self.active_document();
        self.documents[document].inlay_revision = None;
        self.lsp_manager.request_inlay_hints(
            &path,
            lsp_types::Position::new(0, 0),
            end,
            self.inlay_request_id,
        );
    }

    fn sync_schema_active(&mut self) {
        let index = self.active_document();
        let tab = &self.documents[index];
        let Some(path) = tab.buffer.path().map(PathBuf::from) else {
            return;
        };
        let revision = tab.buffer.revision();
        if tab.schema_revision == Some(revision) {
            return;
        }
        let text = tab.buffer.text();
        let Some(manager) = &self.json_schema_manager else {
            return;
        };
        manager.validate(path, text, revision);
        self.documents[index].schema_revision = Some(revision);
    }

    pub(crate) fn request_quick_fixes(&mut self) {
        let Some(diagnostic) = self.documents[self.command_document()]
            .diagnostics
            .iter()
            .find(|diagnostic| {
                let tab = &self.documents[self.command_document()];
                let (line, col) = self.editor().cursor_line_col();
                let start_line = (diagnostic.range.start.line as usize)
                    .min(tab.buffer.len_lines().saturating_sub(1));
                let end_line = (diagnostic.range.end.line as usize)
                    .min(tab.buffer.len_lines().saturating_sub(1));
                let start = utf16_column_to_chars(
                    &tab.buffer.line(start_line),
                    diagnostic.range.start.character,
                );
                let end = utf16_column_to_chars(
                    &tab.buffer.line(end_line),
                    diagnostic.range.end.character,
                );
                let start_pos = tab.buffer.line_start_char(start_line) + start;
                let end_pos = tab.buffer.line_start_char(end_line) + end;
                let cursor = tab.buffer.line_start_char(line) + col;
                cursor >= start_pos && cursor <= end_pos
            })
            .cloned()
        else {
            self.quick_fix_visible = false;
            self.status = self.t("lsp.quick_fix.place_cursor").into();
            return;
        };
        self.request_quick_fixes_for(diagnostic);
    }

    pub(crate) fn request_quick_fixes_for(&mut self, diagnostic: lsp_types::Diagnostic) {
        let Some(path) = self.documents[self.command_document()]
            .buffer
            .path()
            .map(PathBuf::from)
        else {
            self.quick_fix_visible = false;
            self.status = self.t("lsp.quick_fix.save_file").into();
            return;
        };
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.context_keys
            .close_widget(super::context_keys::WidgetKind::QuickFix);
        self.quick_fix_actions.clear();
        self.quick_fix_index = 0;
        self.quick_fix_visible = true;
        self.status = self.t("lsp.quick_fix.loading").into();
        self.track_lsp_command(self.lsp_request_id, &path);
        let sent = self.lsp_manager.request_code_actions(
            &path,
            diagnostic.range,
            vec![diagnostic],
            self.lsp_request_id,
        );
        if !sent {
            self.fail_current_command(super::command_execution::CommandError::Failed(
                self.t("lsp.unavailable").into(),
            ));
        }
    }

    pub(crate) fn request_lsp(&mut self, kind: &str) {
        let Some((path, position)) = self.active_lsp_position() else {
            self.status = self.t("lsp.save_for_features").into();
            return;
        };
        if kind == "completion"
            && self.documents[self.command_document()].language == Some(Language::Json)
        {
            self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
            if self.json_schema_manager.is_some() {
                self.track_lsp_command(self.lsp_request_id, &path);
                self.json_schema_manager.as_ref().unwrap().complete(
                    path,
                    self.documents[self.command_document()].buffer.text(),
                    position,
                    self.lsp_request_id,
                );
                return;
            }
        }
        if !self.lsp_manager.supports_path(&path) {
            self.status = self.t("lsp.not_configured").into();
            self.fail_current_command(super::command_execution::CommandError::Failed(
                self.status.clone(),
            ));
            return;
        }
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.track_lsp_command(self.lsp_request_id, &path);
        let sent = match kind {
            "completion" => {
                self.lsp_manager
                    .request_completion(&path, position, self.lsp_request_id)
            }
            "definition" => {
                self.lsp_manager
                    .request_definition(&path, position, self.lsp_request_id)
            }
            "hover" => self
                .lsp_manager
                .request_hover(&path, position, self.lsp_request_id),
            _ => false,
        };
        if !sent {
            self.fail_current_command(super::command_execution::CommandError::Failed(
                self.t("lsp.unavailable").into(),
            ));
        }
    }
}
