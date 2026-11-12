//! Options and include/exclude filters of the project search panel.

use super::*;
use graphite_workspace::SearchOptions;

/// The text field of the project search panel that receives typed characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SearchFocus {
    /// The query, or the replacement when `search_editing_replacement` is set.
    #[default]
    Query,
    Include,
    Exclude,
}

/// Toggles and filter text of the project search panel.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectSearchFilters {
    pub(crate) regex: bool,
    pub(crate) case_sensitive: bool,
    pub(crate) whole_word: bool,
    /// Whether the include/exclude row is shown.
    pub(crate) details: bool,
    pub(crate) include: String,
    pub(crate) exclude: String,
    pub(crate) focus: SearchFocus,
}

impl ProjectSearchFilters {
    pub(crate) fn options(&self) -> SearchOptions {
        SearchOptions {
            regex: self.regex,
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            include: graphite_workspace::parse_glob_list(&self.include),
            exclude: graphite_workspace::parse_glob_list(&self.exclude),
        }
    }

    /// Text of the focused filter field, if a filter field has focus.
    pub(crate) fn focused_filter_mut(&mut self) -> Option<&mut String> {
        match self.focus {
            SearchFocus::Include => Some(&mut self.include),
            SearchFocus::Exclude => Some(&mut self.exclude),
            SearchFocus::Query => None,
        }
    }

    /// Option flags shown on the panel label (`Aa`, `ab`, `.*`) when they are on.
    pub(crate) fn flags_label(&self) -> String {
        [
            (self.case_sensitive, "Aa"),
            (self.whole_word, "ab"),
            (self.regex, ".*"),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, label)| label)
        .collect::<Vec<_>>()
        .join(" ")
    }
}

/// One stop of the Tab cycle through the panel's fields.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    Query,
    Replacement,
    Include,
    Exclude,
}

impl App {
    /// Tab moves focus through query, replacement (when shown), include and exclude (when shown).
    pub(crate) fn cycle_search_focus(&mut self) {
        let mut stops = vec![Stop::Query];
        if self.search_replacement.is_some() {
            stops.push(Stop::Replacement);
        }
        if self.search_filters.details {
            stops.extend([Stop::Include, Stop::Exclude]);
        }
        let current = match self.search_filters.focus {
            SearchFocus::Include => Stop::Include,
            SearchFocus::Exclude => Stop::Exclude,
            SearchFocus::Query if self.search_editing_replacement => Stop::Replacement,
            SearchFocus::Query => Stop::Query,
        };
        let position = stops.iter().position(|stop| *stop == current).unwrap_or(0);
        match stops[(position + 1) % stops.len()] {
            Stop::Query => {
                self.search_filters.focus = SearchFocus::Query;
                self.search_editing_replacement = false;
            }
            Stop::Replacement => {
                self.search_filters.focus = SearchFocus::Query;
                self.search_editing_replacement = true;
            }
            Stop::Include => self.search_filters.focus = SearchFocus::Include,
            Stop::Exclude => self.search_filters.focus = SearchFocus::Exclude,
        }
    }

    /// Routes typed text to the focused project search field.
    pub(crate) fn push_project_search_text(&mut self, text: &str) {
        if self.search_editing_replacement && self.search_filters.focus == SearchFocus::Query {
            if let Some(replacement) = &mut self.search_replacement {
                replacement.push_str(text);
            }
            return;
        }
        if let Some(field) = self.search_filters.focused_filter_mut() {
            field.push_str(text);
        } else if let Some(query) = &mut self.search_query {
            query.push_str(text);
        }
        self.start_project_search();
    }

    /// Removes the last character of the focused project search field.
    pub(crate) fn delete_project_search_left(&mut self) {
        if self.search_editing_replacement && self.search_filters.focus == SearchFocus::Query {
            if let Some(replacement) = &mut self.search_replacement {
                replacement.pop();
            }
            return;
        }
        if let Some(field) = self.search_filters.focused_filter_mut() {
            field.pop();
        } else if let Some(query) = &mut self.search_query {
            query.pop();
        }
        self.start_project_search();
    }

    /// Toggles one of the option flags and runs the search again.
    pub(crate) fn toggle_project_search_option(&mut self, command: &str) {
        let (label, on) = match command {
            "projectSearch.toggleRegex" => {
                self.search_filters.regex = !self.search_filters.regex;
                ("search.option.regex", self.search_filters.regex)
            }
            "projectSearch.toggleCase" => {
                self.search_filters.case_sensitive = !self.search_filters.case_sensitive;
                ("search.option.case", self.search_filters.case_sensitive)
            }
            "projectSearch.toggleWholeWord" => {
                self.search_filters.whole_word = !self.search_filters.whole_word;
                ("search.option.word", self.search_filters.whole_word)
            }
            _ => return,
        };
        let name = self.t(label).to_owned();
        let state = self
            .t(if on {
                "search.option.on"
            } else {
                "search.option.off"
            })
            .to_owned();
        self.status = self.tf(
            "search.option.changed",
            &[("option", &name), ("state", &state)],
        );
        self.start_project_search();
    }

    pub(crate) fn toggle_project_search_details(&mut self) {
        self.search_filters.details = !self.search_filters.details;
        if !self.search_filters.details {
            self.search_filters.focus = SearchFocus::Query;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_fields_are_parsed_into_search_options() {
        let filters = ProjectSearchFilters {
            regex: true,
            include: " src/**, *.rs ".into(),
            exclude: "target/**".into(),
            ..Default::default()
        };
        let options = filters.options();
        assert!(options.regex);
        assert!(!options.case_sensitive);
        assert_eq!(options.include, vec!["src/**", "*.rs"]);
        assert_eq!(options.exclude, vec!["target/**"]);
        assert_eq!(filters.flags_label(), ".*");
    }

    #[test]
    fn invalid_regex_is_reported_and_clears_stale_results() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.search_query = Some("(".into());
        app.search_filters.regex = true;
        app.status.clear();
        app.start_project_search();
        assert!(app.search_results.is_empty());
        assert!(!app.status.is_empty());
    }

    #[test]
    fn option_toggles_flip_their_flag_and_report_the_new_state() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.search_query = Some(String::new());
        app.toggle_project_search_option("projectSearch.toggleCase");
        assert!(app.search_filters.case_sensitive);
        assert!(app.status.contains(app.t("search.option.on")));
        app.toggle_project_search_option("projectSearch.toggleCase");
        assert!(!app.search_filters.case_sensitive);
    }

    #[test]
    fn tab_cycles_query_replacement_include_and_exclude() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.search_query = Some(String::new());
        app.search_replacement = Some(String::new());
        app.search_filters.details = true;

        app.cycle_search_focus();
        app.push_project_search_text("new");
        assert_eq!(app.search_replacement.as_deref(), Some("new"));

        app.cycle_search_focus();
        assert_eq!(app.search_filters.focus, SearchFocus::Include);
        app.push_project_search_text("src");
        assert_eq!(app.search_filters.include, "src");

        app.cycle_search_focus();
        assert_eq!(app.search_filters.focus, SearchFocus::Exclude);
        app.push_project_search_text("t");
        app.delete_project_search_left();
        assert_eq!(app.search_filters.exclude, "");

        app.cycle_search_focus();
        assert_eq!(app.search_filters.focus, SearchFocus::Query);
        assert!(!app.search_editing_replacement);
        app.push_project_search_text("fn");
        assert_eq!(app.search_query.as_deref(), Some("fn"));
    }
}
