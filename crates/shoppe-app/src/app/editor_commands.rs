use super::*;

impl App {
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
        if self.tabs.len() == 1 {
            if let Some(path) = self.tabs[0].buffer.path().map(PathBuf::from) {
                self.lsp_manager.close(&path);
            }
            self.tabs[0] = Tab {
                buffer: Buffer::new(),
                scroll_line: 0,
                language: None,
                highlights: None,
                highlighter: IncrementalHighlighter::new(),
                diagnostics: Vec::new(),
                lsp_revision: None,
            };
        } else {
            if let Some(path) = self.tabs[self.active_tab].buffer.path().map(PathBuf::from) {
                self.lsp_manager.close(&path);
            }
            self.tabs.remove(self.active_tab);
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }
        self.status = "Tab closed".into();
    }
}
