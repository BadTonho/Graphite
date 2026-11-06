use super::*;

impl App {
    pub(crate) fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        let ctrl = self.modifiers.control_key();
        if self.close_window_confirm {
            match &event.logical_key {
                Key::Character(c) if c.eq_ignore_ascii_case("s") => self.save_all_before_close(),
                Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                    self.close_window_confirm = false;
                    self.exit_requested = true;
                }
                Key::Named(NamedKey::Escape) => {
                    self.close_window_confirm = false;
                    self.status = "Close cancelled".into();
                }
                _ => {}
            }
            return;
        }
        if !self.close_confirm && !self.ime_composing && self.handle_custom_keybinding(event) {
            return;
        }
        if self.terminal_focused {
            self.handle_terminal_key(event);
            return;
        }
        if self.quick_open.is_some() {
            self.handle_quick_open(event);
            return;
        }
        if self.local_search.is_some() {
            self.handle_local_search(event);
            return;
        }
        self.smart_selection_history.clear();
        if !self.completion_items.is_empty() {
            self.completion_items.clear();
        }
        if self.close_confirm {
            let key = event.text.as_deref().unwrap_or("").to_ascii_lowercase();
            if key == "d" {
                self.close_confirm = false;
                self.close_active_tab();
                return;
            }
            if key == "s" {
                self.save();
                self.close_confirm = false;
                if !self.tabs[self.active_tab].buffer.is_dirty() {
                    self.close_active_tab();
                }
                return;
            }
            if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                self.close_confirm = false;
                self.status = "Close cancelled".into();
            }
            return;
        }
        if self.snippet_query.is_some() {
            if !ctrl && !self.modifiers.alt_key() {
                if let Some(text) = event.text.as_ref().filter(|_| !self.ime_composing) {
                    if !text.chars().any(char::is_control) {
                        if let Some(q) = &mut self.snippet_query {
                            q.push_str(text);
                        }
                    }
                }
            }
            return;
        }
        if self.search_query.is_some() {
            if !self.ime_composing && !ctrl && !self.modifiers.alt_key() {
                if let Some(text) = &event.text {
                    if !text.chars().any(char::is_control) {
                        if let Some(q) = &mut self.search_query {
                            q.push_str(text);
                        }
                        self.start_project_search();
                    }
                }
            }
            return;
        }
        if !ctrl && !self.ime_composing {
            if let Some(text) = &event.text {
                if !text.chars().any(char::is_control) {
                    self.tabs[self.active_tab].buffer.insert(text);
                }
            }
        }
    }
}
