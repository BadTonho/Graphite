use super::*;

impl App {
    pub(crate) fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
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
            match event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.completion_index =
                        (self.completion_index + 1) % self.completion_items.len();
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.completion_index = self
                        .completion_index
                        .checked_sub(1)
                        .unwrap_or(self.completion_items.len() - 1);
                    return;
                }
                Key::Named(NamedKey::Escape) => {
                    self.completion_items.clear();
                    return;
                }
                Key::Named(NamedKey::Enter | NamedKey::Tab) => {
                    self.accept_completion();
                    return;
                }
                _ => {}
            }
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
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => self.snippet_query = None,
                Key::Named(NamedKey::Backspace) => {
                    if let Some(q) = &mut self.snippet_query {
                        q.pop();
                    }
                }
                Key::Named(NamedKey::Enter) => {
                    let q = self.snippet_query.as_deref().unwrap_or("");
                    if let Some(snippet) = self
                        .snippets
                        .iter()
                        .find(|s| s.prefix.starts_with(q) || s.prefix == q)
                    {
                        self.tabs[self.active_tab].buffer.insert(&snippet.body);
                    }
                    self.snippet_query = None;
                }
                _ => {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            if let Some(q) = &mut self.snippet_query {
                                q.push_str(text);
                            }
                        }
                    }
                }
            }
            return;
        }
        if self.word_wrap && self.search_query.is_none() && !ctrl && !self.modifiers.alt_key() {
            let down = match event.logical_key {
                Key::Named(NamedKey::ArrowUp) => Some(false),
                Key::Named(NamedKey::ArrowDown) => Some(true),
                _ => None,
            };
            if let Some(down) = down {
                if self.move_visual_selections(down, shift) {
                    return;
                }
            }
        }
        if self.search_query.is_none()
            && !ctrl
            && !self.modifiers.alt_key()
            && self.tabs[self.active_tab].buffer.selections().len() > 1
        {
            let motion = match event.logical_key {
                Key::Named(NamedKey::ArrowLeft) => Some(shoppe_text::CursorMotion::Left),
                Key::Named(NamedKey::ArrowRight) => Some(shoppe_text::CursorMotion::Right),
                Key::Named(NamedKey::ArrowUp) => Some(shoppe_text::CursorMotion::Up),
                Key::Named(NamedKey::ArrowDown) => Some(shoppe_text::CursorMotion::Down),
                Key::Named(NamedKey::Home) => Some(shoppe_text::CursorMotion::Home),
                Key::Named(NamedKey::End) => Some(shoppe_text::CursorMotion::End),
                _ => None,
            };
            if let Some(motion) = motion {
                self.tabs[self.active_tab]
                    .buffer
                    .move_selections(motion, shift);
                return;
            }
        }
        let before = self.tabs[self.active_tab].buffer.cursor().char_idx;
        let mut moved = false;
        if self.search_query.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.search_query = None;
                    self.search_generation = self.search_generation.wrapping_add(1);
                    self.search_cancel
                        .store(self.search_generation, Ordering::Relaxed);
                    self.status = "Search closed".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    if let Some(q) = &mut self.search_query {
                        q.pop();
                    }
                    self.start_project_search();
                }
                Key::Named(NamedKey::ArrowDown) if !self.search_results.is_empty() => {
                    self.search_index = (self.search_index + 1) % self.search_results.len()
                }
                Key::Named(NamedKey::ArrowUp) if !self.search_results.is_empty() => {
                    self.search_index = self
                        .search_index
                        .checked_sub(1)
                        .unwrap_or(self.search_results.len() - 1)
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(hit) = self.search_results.get(self.search_index).cloned() {
                        self.open_file(hit.path);
                        let b = &mut self.tabs[self.active_tab].buffer;
                        let col = hit.text.get(..hit.start).unwrap_or("").chars().count();
                        b.set_cursor_line_col(hit.line, col);
                        self.search_query = None;
                        self.search_generation = self.search_generation.wrapping_add(1);
                        self.search_cancel
                            .store(self.search_generation, Ordering::Relaxed);
                    } else {
                        self.status = "No matches".into();
                    }
                }
                _ => {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            if let Some(q) = &mut self.search_query {
                                q.push_str(text);
                            }
                            self.start_project_search();
                        }
                    }
                }
            }
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Backspace) => self.tabs[self.active_tab].buffer.backspace(),
            Key::Named(NamedKey::Delete) => self.tabs[self.active_tab].buffer.delete(),
            Key::Named(NamedKey::Enter) => self.tabs[self.active_tab].buffer.insert("\n"),
            Key::Named(NamedKey::Tab) if !ctrl && !self.modifiers.alt_key() => {
                let indentation = self.tabs[self.active_tab].indentation;
                let value = if indentation.insert_spaces {
                    " ".repeat(indentation.tab_size)
                } else {
                    "\t".into()
                };
                self.tabs[self.active_tab].buffer.insert(&value);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.tabs[self.active_tab].buffer.move_left();
                moved = true;
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.tabs[self.active_tab].buffer.move_right();
                moved = true;
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.tabs[self.active_tab].buffer.move_vertical(-1);
                moved = true;
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.tabs[self.active_tab].buffer.move_vertical(1);
                moved = true;
            }
            Key::Named(NamedKey::Home) => {
                self.tabs[self.active_tab].buffer.move_home();
                moved = true;
            }
            Key::Named(NamedKey::End) => {
                self.tabs[self.active_tab].buffer.move_end();
                moved = true;
            }
            _ if !ctrl => {
                if !self.ime_composing {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            self.tabs[self.active_tab].buffer.insert(text);
                        }
                    }
                }
            }
            _ => {}
        }
        if moved {
            let after = self.tabs[self.active_tab].buffer.cursor().char_idx;
            if shift {
                let anchor = self.tabs[self.active_tab]
                    .buffer
                    .selection()
                    .map(|s| s.anchor)
                    .unwrap_or(before);
                self.tabs[self.active_tab]
                    .buffer
                    .set_selection(anchor, after);
            }
        }
    }
}
