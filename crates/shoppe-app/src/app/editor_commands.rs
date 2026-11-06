use super::*;

impl App {
    pub(crate) fn request_window_close(&mut self) {
        if self.tabs.iter().any(|tab| tab.buffer.is_dirty()) {
            self.close_window_confirm = true;
            self.status = "Unsaved documents: S save all, D discard, Esc cancel".into();
        } else {
            self.exit_requested = true;
        }
    }

    pub(crate) fn save_all_before_close(&mut self) {
        let previous = self.active_tab;
        for index in 0..self.tabs.len() {
            if !self.tabs[index].buffer.is_dirty() {
                continue;
            }
            self.active_tab = index;
            self.save();
            if self.tabs[index].buffer.is_dirty() {
                self.status = format!("Cannot close: {}", self.status);
                self.close_window_confirm = false;
                return;
            }
        }
        self.active_tab = previous;
        self.close_window_confirm = false;
        self.exit_requested = true;
    }

    pub(crate) fn accept_completion(&mut self) {
        let Some(item) = self.completion_items.get(self.completion_index).cloned() else {
            return;
        };
        let text = item.insert_text.unwrap_or_else(|| item.label.clone());
        let buffer = &mut self.tabs[self.active_tab].buffer;
        let chars: Vec<char> = buffer.text().chars().collect();
        let cursor = buffer.cursor().char_idx.min(chars.len());
        let mut start = cursor;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        buffer.set_selection(start, cursor);
        buffer.insert(&text);
        self.completion_items.clear();
        self.hover_text = None;
    }

    pub(crate) fn navigate_diagnostic(&mut self, direction: isize) {
        let diagnostics = self.tabs[self.active_tab].diagnostics.clone();
        if diagnostics.is_empty() {
            self.status = "No diagnostics in this file".into();
            return;
        }
        let (line, _) = self.tabs[self.active_tab].buffer.cursor_line_col();
        let next = if direction > 0 {
            diagnostics
                .iter()
                .find(|d| d.range.start.line as usize > line)
                .or_else(|| diagnostics.first())
        } else {
            diagnostics
                .iter()
                .rev()
                .find(|d| (d.range.start.line as usize) < line)
                .or_else(|| diagnostics.last())
        };
        if let Some(diagnostic) = next {
            let line = diagnostic.range.start.line as usize;
            let col = utf16_column_to_chars(
                &self.tabs[self.active_tab].buffer.line(line),
                diagnostic.range.start.character,
            );
            self.tabs[self.active_tab]
                .buffer
                .set_cursor_line_col(line, col);
            self.status = diagnostic.message.clone();
        }
    }

    pub(crate) fn close_active_tab(&mut self) {
        self.smart_selection_history.clear();
        self.smart_selection_guard = None;
        self.snapshot_editor_view();
        let document = self.active_tab;
        let last_view = self.editor_groups.view_count(document) == 1;
        if self.editor_groups.close_active(false) {
            if last_view {
                if let Some(path) = self.tabs[document].buffer.path().map(PathBuf::from) {
                    self.lsp_manager.close(&path);
                }
                self.tabs.remove(document);
                self.editor_groups.forget_document(document);
            }
            self.restore_editor_view();
        } else {
            if let Some(path) = self.tabs[document].buffer.path().map(PathBuf::from) {
                self.lsp_manager.close(&path);
            }
            self.tabs[document] = Tab {
                id: next_document_id(),
                buffer: Buffer::new(),
                scroll_line: 0,
                language: None,
                highlights: None,
                highlighter: IncrementalHighlighter::new(),
                diagnostics: Vec::new(),
                lsp_revision: None,
                indentation: shoppe_text::Indentation {
                    insert_spaces: self.insert_spaces,
                    tab_size: self.tab_size,
                },
            };
            self.snapshot_editor_view();
        }
        self.status = "Tab closed".into();
    }
}
