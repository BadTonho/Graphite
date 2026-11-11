use super::*;

impl App {
    pub(crate) fn ime_preedit(&mut self, text: String) {
        if self.plugin_owns_keyboard() {
            return;
        }
        self.editor_groups.active_view_mut().ime_composing = !text.is_empty();
        self.editor_groups.active_view_mut().ime_preedit = text;
    }

    pub(crate) fn ime_commit(&mut self, text: &str) {
        self.editor_groups.active_view_mut().ime_composing = false;
        self.editor_groups.active_view_mut().ime_preedit.clear();
        if self.plugin_commit_text(text) {
            return;
        }
        if self.terminal_focused {
            if let Some(terminal) = self.terminals.get(self.focused_terminal) {
                terminal.input(text.as_bytes().to_vec());
            }
        } else if let Some(state) = &mut self.quick_open {
            state.query.push_str(text);
            self.request_quick_open();
        } else if let Some(picker) = &mut self.go_picker {
            picker.query.push_str(text);
            picker.selected = 0;
        } else if let Some(search) = &mut self.local_search {
            if search.edit_replacement {
                search.replacement.push_str(text);
            } else {
                search.query.push_str(text);
            }
        } else if let Some(query) = &mut self.search_query {
            query.push_str(text);
            self.start_project_search();
        } else if let Some(query) = &mut self.snippet_query {
            query.push_str(text);
        } else {
            let before = self.editor_state();
            self.editor_mut().insert(text);
            if before != self.editor_state() {
                self.invalidate_lsp_requests();
            }
            self.sync_lsp_active();
        }
    }
}
