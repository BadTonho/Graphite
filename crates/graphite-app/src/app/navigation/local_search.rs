use super::*;
use graphite_text::SearchOptions;
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
        let buffer = self.editor();
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
        self.status = self.t("search.prompt").into();
    }

    pub(crate) fn local_search_next(&mut self, search: &mut LocalSearch, backwards: bool) {
        let translator = self.translator;
        let mut buffer = self.editor_mut();
        match buffer.find_matches(&search.query, search.options, search.scope.clone()) {
            Ok(matches) if matches.is_empty() => {
                search.message = translator.text("search.no_matches").into();
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
            Err(error) => {
                search.message = translator.format("search.invalid_regex", &[("error", &error)]);
            }
        }
    }

    pub(crate) fn local_replace_all(&mut self, search: &mut LocalSearch) {
        let translator = self.translator;
        let mut buffer = self.editor_mut();
        if buffer.is_read_only() {
            search.message = translator.text("search.read_only").into();
            return;
        }
        match buffer.replace_matches(
            &search.query,
            &search.replacement,
            search.options,
            search.scope.clone(),
        ) {
            Ok(n) => {
                search.message = translator.format("search.replaced_many", &[("count", &n)]);
                search.scope = None;
            }
            Err(error) => {
                search.message = translator.format("search.invalid_regex", &[("error", &error)]);
            }
        }
    }

    pub(crate) fn local_replace_selected(&mut self, search: &mut LocalSearch) {
        let translator = self.translator;
        let mut buffer = self.editor_mut();
        if buffer.is_read_only() {
            search.message = translator.text("search.read_only").into();
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
                search.message = translator.text("search.replaced_one").into();
            }
            Ok(false) => search.message = translator.text("search.select_match").into(),
            Err(error) => {
                search.message = translator.format("search.invalid_regex", &[("error", &error)]);
            }
        }
    }

    pub(crate) fn handle_local_search(&mut self, event: &winit::event::KeyEvent) {
        let Some(mut search) = self.local_search.take() else {
            return;
        };
        let ctrl = self.modifiers.control_key();
        let alt = self.modifiers.alt_key();
        if !self.editor_groups.active_view_mut().ime_composing && !ctrl && !alt {
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
        self.local_search = Some(search);
    }

    pub(crate) fn execute_find_command(&mut self, command: &str) {
        let Some(mut search) = self.local_search.take() else {
            return;
        };
        match command {
            "closeFindWidget" => {
                self.status = self.t("search.closed").into();
                return;
            }
            "find.toggleField" if search.replace => {
                search.edit_replacement = !search.edit_replacement
            }
            "editor.action.nextMatchFindAction" => self.local_search_next(&mut search, false),
            "editor.action.previousMatchFindAction" => self.local_search_next(&mut search, true),
            "editor.action.replaceAll" if search.replace => self.local_replace_all(&mut search),
            "editor.action.replaceOne" if search.replace => {
                self.local_replace_selected(&mut search)
            }
            "find.deleteLeft" => {
                if search.edit_replacement {
                    search.replacement.pop();
                } else {
                    search.query.pop();
                }
            }
            "find.toggleRegex" => search.options.regex = !search.options.regex,
            "find.toggleCase" => search.options.case_sensitive = !search.options.case_sensitive,
            "find.toggleWholeWord" => search.options.whole_word = !search.options.whole_word,
            "find.togglePreserveCase" => {
                search.options.preserve_case = !search.options.preserve_case
            }
            "find.toggleSelection" => {
                search.scope = if search.scope.is_some() {
                    None
                } else {
                    self.editor()
                        .selection()
                        .map(|s| s.anchor.min(s.head)..s.anchor.max(s.head))
                };
            }
            _ => {}
        }
        self.local_search = Some(search);
    }
}
