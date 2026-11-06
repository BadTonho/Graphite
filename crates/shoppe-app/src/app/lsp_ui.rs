use super::*;

impl App {
    pub(crate) fn register_lsp_active(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        let Some(path) = tab.buffer.path().map(PathBuf::from) else {
            return;
        };
        if !self.lsp_manager.supports_path(&path) {
            return;
        }
        let version = tab.buffer.revision() as i32;
        self.lsp_manager.open(
            &path,
            self.tree.root(),
            tab.buffer.text(),
            version,
            self.lsp_sender.clone(),
        );
        tab.lsp_revision = Some(tab.buffer.revision());
    }

    pub(crate) fn sync_lsp_active(&mut self) {
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
                                self.status = format!("{} diagnostic(s)", tab.diagnostics.len());
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
                            self.status = "Definition opened".into();
                        }
                    } else {
                        self.status = "No definition found".into();
                    }
                }
                LspUiEvent::Hover { request_id, text } if request_id == self.lsp_request_id => {
                    self.hover_text = text;
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

    pub(crate) fn editor_state(&self) -> (usize, usize, u64, Option<shoppe_text::Selection>) {
        let tab = &self.tabs[self.active_tab];
        (
            self.active_tab,
            tab.buffer.cursor().char_idx,
            tab.buffer.revision(),
            tab.buffer.selection(),
        )
    }

    pub(crate) fn invalidate_lsp_requests(&mut self) {
        self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
        self.completion_items.clear();
        self.hover_text = None;
    }

    pub(crate) fn request_lsp(&mut self, kind: &str) {
        let Some((path, position)) = self.active_lsp_position() else {
            self.status = "Save this file to use language features".into();
            return;
        };
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
