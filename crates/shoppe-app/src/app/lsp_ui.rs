use super::*;

impl App {
    pub(crate) fn register_lsp_active(&mut self) {
        self.sync_schema_active();
        let Some(path) = self.tabs[self.active_tab].buffer.path().map(PathBuf::from) else {
            return;
        };
        if !self.lsp_manager.supports_path(&path) {
            return;
        }
        let version = self.tabs[self.active_tab].buffer.revision() as i32;
        let text = self.tabs[self.active_tab].buffer.text();
        self.lsp_manager.open(
            &path,
            self.tree.root(),
            text,
            version,
            self.lsp_sender.clone(),
        );
        self.tabs[self.active_tab].lsp_revision = Some(version as u64);
        self.request_inlay_hints();
    }

    pub(crate) fn sync_lsp_active(&mut self) {
        self.sync_schema_active();
        let tab = &mut self.tabs[self.active_tab];
        let Some(path) = tab.buffer.path().map(PathBuf::from) else {
            return;
        };
        let revision = tab.buffer.revision();
        if tab.lsp_revision == Some(revision) {
            return;
        }
        if tab.lsp_revision.is_none() {
            self.register_lsp_active();
            return;
        }
        self.lsp_manager
            .change(&path, tab.buffer.text(), revision as i32);
        tab.lsp_revision = Some(revision);
        self.request_inlay_hints();
    }

    pub(crate) fn process_lsp_events(&mut self) {
        let events = self
            .lsp_queue
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for event in events {
            match event {
                LspUiEvent::Status(status) => self.status = status,
                LspUiEvent::Diagnostics {
                    uri,
                    version,
                    diagnostics,
                } => {
                    if let Some(path) = lsp::uri_to_path(&uri) {
                        if let Some(index) = self.tabs.iter().position(|t| {
                            t.buffer
                                .path()
                                .is_some_and(|open_path| same_file_path(open_path, &path))
                        }) {
                            let tab = &mut self.tabs[index];
                            if !lsp::diagnostics_are_current(tab.lsp_revision, version) {
                                continue;
                            }
                            tab.diagnostics = diagnostics;
                            if index == self.active_tab {
                                self.status = format!(
                                    "{} diagnostic(s)",
                                    tab.diagnostics.len() + tab.schema_diagnostics.len()
                                );
                            }
                        }
                    }
                }
                LspUiEvent::Completion { request_id, items }
                    if request_id == self.lsp_request_id =>
                {
                    self.completion_items = items;
                    self.completion_index = 0;
                }
                LspUiEvent::Definition {
                    request_id,
                    location,
                } if request_id == self.lsp_request_id => {
                    if let Some(location) = location {
                        if let Some(path) = lsp::uri_to_path(location.uri.as_str()) {
                            self.open_file(path);
                            let line = location.range.start.line as usize;
                            let col = utf16_column_to_chars(
                                &self.tabs[self.active_tab].buffer.line(line),
                                location.range.start.character,
                            );
                            self.tabs[self.active_tab]
                                .buffer
                                .set_cursor_line_col(line, col);
                            self.push_navigation_location();
                            self.status = "Definition opened".into();
                        }
                    } else {
                        self.status = "No definition found".into();
                    }
                }
                LspUiEvent::Hover { request_id, text } if request_id == self.lsp_request_id => {
                    self.hover_text = text;
                }
                LspUiEvent::InlayHints { request_id, hints }
                    if request_id == self.inlay_request_id =>
                {
                    let tab = &mut self.tabs[self.active_tab];
                    tab.inlay_hints = hints;
                    tab.inlay_revision = Some(tab.buffer.revision());
                }
                LspUiEvent::CodeActions {
                    request_id,
                    actions,
                    error,
                } if request_id == self.lsp_request_id => {
                    self.quick_fix_actions = actions;
                    self.quick_fix_index = 0;
                    self.quick_fix_visible = true;
                    self.status = if let Some(error) = error {
                        self.quick_fix_visible = false;
                        format!("Quick fix request failed: {error}")
                    } else if self.quick_fix_actions.is_empty() {
                        "No quick fixes available".into()
                    } else {
                        "Quick Fix: Up/Down to choose, Enter to apply, Esc to close".into()
                    };
                }
                LspUiEvent::JsonDiagnostics {
                    uri,
                    revision,
                    diagnostics,
                } => {
                    if let Some(path) = lsp::uri_to_path(&uri) {
                        if let Some(tab) = self.tabs.iter_mut().find(|tab| {
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
                    if request_id == self.lsp_request_id =>
                {
                    self.completion_items = items;
                    self.completion_index = 0;
                    if self.completion_items.is_empty() {
                        self.status = "No schema completions available".into();
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn active_lsp_position(&self) -> Option<(PathBuf, lsp_types::Position)> {
        let tab = &self.tabs[self.active_tab];
        let path = tab.buffer.path()?.to_path_buf();
        let char_index = tab.buffer.cursor().char_idx;
        Some((path, lsp::position_at_char(&tab.buffer.text(), char_index)))
    }

    pub(crate) fn editor_state(&self) -> (usize, usize, u64, Vec<graphite_text::Selection>) {
        let tab = &self.tabs[self.active_tab];
        (
            self.active_tab,
            tab.buffer.cursor().char_idx,
            tab.buffer.revision(),
            tab.buffer.selections(),
        )
    }

    pub(crate) fn invalidate_lsp_requests(&mut self) {
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.inlay_request_id = self.inlay_request_id.wrapping_add(1);
        self.completion_items.clear();
        self.hover_text = None;
    }

    pub(crate) fn request_inlay_hints(&mut self) {
        let tab = &self.tabs[self.active_tab];
        let Some(path) = tab.buffer.path().map(PathBuf::from) else {
            return;
        };
        if !self.lsp_manager.supports_path(&path) {
            return;
        }
        let text = tab.buffer.text();
        let end = lsp::position_at_char(&text, text.chars().count());
        self.inlay_request_id = self.inlay_request_id.wrapping_add(1);
        self.tabs[self.active_tab].inlay_revision = None;
        self.lsp_manager.request_inlay_hints(
            &path,
            lsp_types::Position::new(0, 0),
            end,
            self.inlay_request_id,
        );
    }

    fn sync_schema_active(&mut self) {
        let index = self.active_tab;
        let tab = &self.tabs[index];
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
        self.tabs[index].schema_revision = Some(revision);
    }

    pub(crate) fn request_quick_fixes(&mut self) {
        let Some(diagnostic) = self.tabs[self.active_tab]
            .diagnostics
            .iter()
            .find(|diagnostic| {
                let tab = &self.tabs[self.active_tab];
                let (line, col) = tab.buffer.cursor_line_col();
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
            self.status = "Place the cursor on a diagnostic to see quick fixes".into();
            return;
        };
        self.request_quick_fixes_for(diagnostic);
    }

    pub(crate) fn request_quick_fixes_for(&mut self, diagnostic: lsp_types::Diagnostic) {
        let Some(path) = self.tabs[self.active_tab].buffer.path().map(PathBuf::from) else {
            self.quick_fix_visible = false;
            self.status = "Save this file to use quick fixes".into();
            return;
        };
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.quick_fix_actions.clear();
        self.quick_fix_index = 0;
        self.quick_fix_visible = true;
        self.status = "Loading quick fixes…".into();
        self.lsp_manager.request_code_actions(
            &path,
            diagnostic.range,
            vec![diagnostic],
            self.lsp_request_id,
        );
    }

    pub(crate) fn request_lsp(&mut self, kind: &str) {
        let Some((path, position)) = self.active_lsp_position() else {
            self.status = "Save this file to use language features".into();
            return;
        };
        if kind == "completion" && self.tabs[self.active_tab].language == Some(Language::Json) {
            self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
            if let Some(manager) = &self.json_schema_manager {
                manager.complete(
                    path,
                    self.tabs[self.active_tab].buffer.text(),
                    position,
                    self.lsp_request_id,
                );
                return;
            }
        }
        if !self.lsp_manager.supports_path(&path) {
            self.status = "No language server configured for this file type".into();
            return;
        }
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        match kind {
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
            _ => {}
        }
    }
}
