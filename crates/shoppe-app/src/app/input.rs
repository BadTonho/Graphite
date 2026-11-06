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
        if ctrl
            && !shift
            && !self.modifiers.alt_key()
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("p"))
        {
            self.start_quick_open();
            return;
        }
        if self.quick_open.is_some() {
            self.handle_quick_open(event);
            return;
        }
        if ctrl
            && !shift
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("f") || c.eq_ignore_ascii_case("h"))
        {
            let replace =
                matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("h"));
            self.open_local_search(replace);
            return;
        }
        if self.local_search.is_some() {
            self.handle_local_search(event);
            return;
        }
        if self.modifiers.alt_key() && shift && !ctrl {
            match event.logical_key {
                Key::Named(NamedKey::ArrowRight) => {
                    self.smart_select(true);
                    return;
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    self.smart_select(false);
                    return;
                }
                _ => {}
            }
        }
        self.smart_selection_history.clear();
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
        if ctrl
            && !shift
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("k"))
        {
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
        if self.search_query.is_none() && self.modifiers.alt_key() && !ctrl {
            match event.logical_key {
                Key::Named(NamedKey::ArrowUp) if shift => {
                    self.tabs[self.active_tab].buffer.duplicate_line(false);
                    return;
                }
                Key::Named(NamedKey::ArrowDown) if shift => {
                    self.tabs[self.active_tab].buffer.duplicate_line(true);
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.tabs[self.active_tab].buffer.move_line(-1);
                    return;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.tabs[self.active_tab].buffer.move_line(1);
                    return;
                }
                _ => {}
            }
        }
        if self.search_query.is_none()
            && ctrl
            && shift
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("k"))
        {
            self.tabs[self.active_tab].buffer.delete_line();
            return;
        }
        if self.search_query.is_none() && ctrl && shift && self.modifiers.alt_key() {
            match &event.logical_key {
                Key::Character(c) if c.eq_ignore_ascii_case("i") => {
                    let tab = &mut self.tabs[self.active_tab];
                    let mut target = tab.indentation;
                    target.insert_spaces = !target.insert_spaces;
                    match tab.buffer.convert_indentation(target) {
                        Ok(_) => {
                            tab.indentation = target;
                            self.status = if target.insert_spaces {
                                "Indentation converted to spaces"
                            } else {
                                "Indentation converted to tabs"
                            }
                            .into();
                        }
                        Err(error) => self.status = error.to_string(),
                    }
                    return;
                }
                Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                    self.tabs[self.active_tab].buffer.sort_lines();
                    return;
                }
                Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                    self.tabs[self.active_tab].buffer.deduplicate_lines();
                    return;
                }
                _ => {}
            }
        }
        if self.search_query.is_none() && ctrl && !self.modifiers.alt_key() {
            if matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("d"))
                && !shift
            {
                self.tabs[self.active_tab].buffer.select_occurrences(false);
                return;
            }
            if matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("l"))
                && shift
            {
                self.tabs[self.active_tab].buffer.select_occurrences(true);
                return;
            }
        }
        if self.search_query.is_none()
            && !ctrl
            && !self.modifiers.alt_key()
            && self.tabs[self.active_tab].buffer.selections().len() > 1
        {
            let motion = match event.logical_key {
                Key::Named(NamedKey::ArrowLeft) => Some(graphite_text::CursorMotion::Left),
                Key::Named(NamedKey::ArrowRight) => Some(graphite_text::CursorMotion::Right),
                Key::Named(NamedKey::ArrowUp) => Some(graphite_text::CursorMotion::Up),
                Key::Named(NamedKey::ArrowDown) => Some(graphite_text::CursorMotion::Down),
                Key::Named(NamedKey::Home) => Some(graphite_text::CursorMotion::Home),
                Key::Named(NamedKey::End) => Some(graphite_text::CursorMotion::End),
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
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("i") => {
                let report = config::import_vscode();
                let settings = config::editor_prefs_for_workspace(self.tree.root());
                let theme = config::imported_theme_colors(self.tree.root());
                let settings_error = match (settings, theme) {
                    (Ok(prefs), Ok(colors)) => {
                        DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
                        Theme::set_imported_colors(colors);
                        self.font_size = prefs.font_size;
                        self.tab_size = prefs.tab_size;
                        self.insert_spaces = prefs.insert_spaces;
                        self.read_only_patterns = prefs.read_only_patterns;
                        self.detect_indentation = prefs.detect_indentation;
                        self.trim_trailing_whitespace = prefs.trim_trailing_whitespace;
                        self.rulers = prefs.rulers;
                        None
                    }
                    (Err(error), _) | (_, Err(error)) => Some(error),
                };
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer
                        .set_font_size(self.font_size * gpu.window.scale_factor() as f32);
                }
                self.snippets = config::load_snippets();
                self.status = if let Some(error) = settings_error {
                    format!("Settings unchanged: {error}")
                } else if report.warnings.is_empty() {
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
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("f") => {
                self.search_query = Some(String::new());
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_cancel
                    .store(self.search_generation, Ordering::Relaxed);
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
                let selected = self.tabs[self.active_tab].buffer.selected_texts();
                if !selected.is_empty() {
                    let text = selected.join("\n");
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(text);
                    }
                }
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("x") => {
                let selected = self.tabs[self.active_tab].buffer.selected_texts();
                if !selected.is_empty() {
                    let text = selected.join("\n");
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
                        self.tabs[self.active_tab].buffer.paste(&text);
                    }
                }
            }
            Key::Named(NamedKey::Backspace) => self.tabs[self.active_tab].buffer.backspace(),
            Key::Named(NamedKey::Delete) => self.tabs[self.active_tab].buffer.delete(),
            Key::Named(NamedKey::Enter) => self.tabs[self.active_tab].buffer.insert("\n"),
            Key::Named(NamedKey::Tab) => {
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
