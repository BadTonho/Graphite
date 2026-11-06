use super::*;

impl App {
    pub(crate) fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        if ctrl
            && (matches!(&event.logical_key, Key::Character(c) if c == "`")
                || matches!(event.physical_key, PhysicalKey::Code(KeyCode::Backquote)))
        {
            self.toggle_terminal();
            return;
        }
        if ctrl
            && shift
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("m"))
        {
            self.terminal_visible = true;
            self.terminal_problems = true;
            self.terminal_focused = false;
            self.resize_terminal();
            return;
        }
        if self.terminal_focused {
            self.handle_terminal_key(event);
            return;
        }
        if self.lsp_chord {
            self.lsp_chord = false;
            if ctrl
                && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("i"))
            {
                self.request_lsp("hover");
                return;
            }
        }
        if ctrl
            && (matches!(&event.logical_key, Key::Character(c) if c == " ")
                || matches!(event.logical_key, Key::Named(NamedKey::Space)))
        {
            self.completion_items.clear();
            self.request_lsp("completion");
            return;
        }
        if matches!(event.logical_key, Key::Named(NamedKey::F12)) {
            self.request_lsp("definition");
            return;
        }
        if ctrl && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("k")) {
            self.lsp_chord = true;
            self.status = "Press Ctrl+I for hover information".into();
            return;
        }
        if matches!(event.logical_key, Key::Named(NamedKey::F8)) {
            self.navigate_diagnostic(if shift { -1 } else { 1 });
            return;
        }
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
        if ctrl
            && self.modifiers.alt_key()
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("t"))
        {
            let dark = !DARK_THEME.load(Ordering::Relaxed);
            DARK_THEME.store(dark, Ordering::Relaxed);
            config::save_dark_theme(dark);
            return;
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
        let before = self.tabs[self.active_tab].buffer.cursor().char_idx;
        let mut moved = false;
        if self.search_query.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.search_query = None;
                    self.status = "Search closed".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    if let Some(q) = &mut self.search_query {
                        q.pop();
                    }
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
                    } else {
                        self.status = "No matches".into();
                    }
                }
                _ => {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            if let Some(q) = &mut self.search_query {
                                q.push_str(text);
                                self.search_results = self.tree.search(q);
                            }
                        }
                    }
                }
            }
            return;
        }
        match &event.logical_key {
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("i") => {
                let report = config::import_vscode();
                DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
                Theme::set_imported_colors(config::imported_theme_colors());
                let prefs = config::editor_prefs();
                self.font_size = prefs.font_size;
                self.tab_size = prefs.tab_size;
                self.insert_spaces = prefs.insert_spaces;
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer
                        .set_font_size(self.font_size * gpu.window.scale_factor() as f32);
                }
                self.snippets = config::load_snippets();
                self.status = if report.warnings.is_empty() {
                    format!("Imported {} VS Code items", report.imported)
                } else {
                    format!(
                        "Imported {} items; {} warning(s): {}",
                        report.imported,
                        report.warnings.len(),
                        report.warnings[0]
                    )
                };
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("p") => {
                self.snippets = config::load_snippets();
                self.snippet_query = Some(String::new());
                self.status = "Snippet: type prefix, Enter inserts, Esc closes".into();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("f") => {
                self.search_query = Some(String::new());
                self.search_results.clear();
                self.search_index = 0;
                self.status = "Search: type query; Up/Down select, Enter opens result".into();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("s") && self.close_confirm => {
                self.save();
                self.close_confirm = false;
                self.close_active_tab();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("d") && self.close_confirm => {
                self.close_confirm = false;
                self.close_active_tab();
            }
            Key::Named(NamedKey::Tab) if ctrl => {
                self.active_tab = (self.active_tab + 1) % self.tabs.len()
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("s") => self.save(),
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("z") => {
                self.tabs[self.active_tab].buffer.undo()
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("y") => {
                self.tabs[self.active_tab].buffer.redo()
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("a") => {
                let len = self.tabs[self.active_tab].buffer.len_chars();
                self.tabs[self.active_tab].buffer.set_selection(0, len);
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("w") => {
                if self.tabs[self.active_tab].buffer.is_dirty() {
                    self.close_confirm = true;
                    self.status = "Unsaved tab: S save, D discard, Esc cancel".into();
                } else {
                    self.close_active_tab();
                }
            }
            Key::Named(NamedKey::Escape) if self.close_confirm => {
                self.close_confirm = false;
                self.status = "Close cancelled".into();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("c") => {
                if let Some(text) = self.tabs[self.active_tab].buffer.selected_text() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(text);
                    }
                }
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("x") => {
                if let Some(text) = self.tabs[self.active_tab].buffer.selected_text() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if clipboard.set_text(text).is_ok() {
                            self.tabs[self.active_tab].buffer.insert("");
                        }
                    }
                }
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("v") => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.tabs[self.active_tab].buffer.insert(&text);
                    }
                }
            }
            Key::Named(NamedKey::Backspace) => self.tabs[self.active_tab].buffer.backspace(),
            Key::Named(NamedKey::Delete) => self.tabs[self.active_tab].buffer.delete(),
            Key::Named(NamedKey::Enter) => self.tabs[self.active_tab].buffer.insert("\n"),
            Key::Named(NamedKey::Tab) => {
                let value = if self.insert_spaces {
                    " ".repeat(self.tab_size)
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
