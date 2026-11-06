use super::*;
use shoppe_text::SearchOptions;
use std::ops::Range;

#[derive(Default)]
pub(crate) struct LocalSearch {
    pub query: String,
    pub replacement: String,
    pub replace: bool,
    pub edit_replacement: bool,
    pub options: SearchOptions,
    pub scope: Option<Range<usize>>,
    pub index: usize,
    pub message: String,
}

impl App {
    pub(crate) fn open_local_search(&mut self, replace: bool) {
        let buffer = &self.tabs[self.active_tab].buffer;
        self.local_search = Some(LocalSearch {
            query: buffer
                .selected_text()
                .filter(|s| !s.contains('\n'))
                .unwrap_or_default(),
            replace,
            ..Default::default()
        });
        self.search_query = None;
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_cancel
            .store(self.search_generation, Ordering::Relaxed);
        self.status =
            "Find: Enter next; Alt+R regex, Alt+C case, Alt+W word, Alt+S selection".into();
    }

    pub(crate) fn local_search_next(&mut self, search: &mut LocalSearch, backwards: bool) {
        let buffer = &mut self.tabs[self.active_tab].buffer;
        match buffer.find_matches(&search.query, search.options, search.scope.clone()) {
            Ok(matches) if matches.is_empty() => {
                search.message = "No matches".into();
            }
            Ok(matches) => {
                let current = buffer
                    .selection()
                    .map(|s| (s.anchor.min(s.head), s.anchor.max(s.head)));
                let selected = current.and_then(|(a, b)| {
                    matches
                        .iter()
                        .position(|m| m.range.start == a && m.range.end == b)
                });
                let index = match selected {
                    Some(i) if backwards => (i + matches.len() - 1) % matches.len(),
                    Some(i) => (i + 1) % matches.len(),
                    None => matches
                        .iter()
                        .position(|m| m.range.start >= buffer.cursor().char_idx)
                        .unwrap_or(0),
                };
                let range = matches[index].range.clone();
                buffer.set_selection(range.start, range.end);
                search.index = index;
                search.message = format!("{} / {}", index + 1, matches.len());
            }
            Err(error) => search.message = format!("Invalid regex: {error}"),
        }
    }

    pub(crate) fn local_replace_all(&mut self, search: &mut LocalSearch) {
        let buffer = &mut self.tabs[self.active_tab].buffer;
        if buffer.is_read_only() {
            search.message = "Read-only document".into();
            return;
        }
        match buffer.replace_matches(
            &search.query,
            &search.replacement,
            search.options,
            search.scope.clone(),
        ) {
            Ok(n) => {
                search.message = format!("Replaced {n} matches");
                search.scope = None;
            }
            Err(error) => search.message = format!("Invalid regex: {error}"),
        }
    }

    pub(crate) fn local_replace_selected(&mut self, search: &mut LocalSearch) {
        let buffer = &mut self.tabs[self.active_tab].buffer;
        if buffer.is_read_only() {
            search.message = "Read-only document".into();
            return;
        }
        let before = buffer.len_chars();
        match buffer.replace_selected_match(
            &search.query,
            &search.replacement,
            search.options,
            search.scope.clone(),
        ) {
            Ok(true) => {
                if let Some(scope) = &mut search.scope {
                    scope.end = scope
                        .end
                        .saturating_add_signed(buffer.len_chars() as isize - before as isize);
                }
                search.message = "Replaced one match".into();
            }
            Ok(false) => search.message = "Select a match first".into(),
            Err(error) => search.message = format!("Invalid regex: {error}"),
        }
    }

    pub(crate) fn handle_local_search(&mut self, event: &winit::event::KeyEvent) {
        let Some(mut search) = self.local_search.take() else {
            return;
        };
        let ctrl = self.modifiers.control_key();
        let alt = self.modifiers.alt_key();
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.status = "Find closed".into();
                return;
            }
            Key::Named(NamedKey::Tab) if search.replace => {
                search.edit_replacement = !search.edit_replacement
            }
            Key::Named(NamedKey::Enter) if ctrl && search.replace => {
                self.local_replace_all(&mut search)
            }
            Key::Named(NamedKey::Enter) if alt && search.replace => {
                self.local_replace_selected(&mut search)
            }
            Key::Named(NamedKey::Enter) => {
                self.local_search_next(&mut search, self.modifiers.shift_key())
            }
            Key::Named(NamedKey::Backspace) => {
                if search.edit_replacement {
                    search.replacement.pop();
                } else {
                    search.query.pop();
                }
            }
            Key::Character(c) if alt && c.eq_ignore_ascii_case("r") => {
                search.options.regex = !search.options.regex
            }
            Key::Character(c) if alt && c.eq_ignore_ascii_case("c") => {
                search.options.case_sensitive = !search.options.case_sensitive
            }
            Key::Character(c) if alt && c.eq_ignore_ascii_case("w") => {
                search.options.whole_word = !search.options.whole_word
            }
            Key::Character(c) if alt && c.eq_ignore_ascii_case("p") => {
                search.options.preserve_case = !search.options.preserve_case
            }
            Key::Character(c) if alt && c.eq_ignore_ascii_case("s") => {
                search.scope = if search.scope.is_some() {
                    None
                } else {
                    self.tabs[self.active_tab]
                        .buffer
                        .selection()
                        .map(|s| s.anchor.min(s.head)..s.anchor.max(s.head))
                };
            }
            _ if !ctrl && !alt => {
                if let Some(text) = &event.text {
                    if !text.chars().any(char::is_control) {
                        if search.edit_replacement {
                            search.replacement.push_str(text);
                        } else {
                            search.query.push_str(text);
                        }
                    }
                }
            }
            _ => {}
        }
        self.local_search = Some(search);
    }
}
