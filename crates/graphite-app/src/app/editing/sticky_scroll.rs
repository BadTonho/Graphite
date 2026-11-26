//! Sticky scroll: the scope headers pinned above the text, and keyboard focus on them.

use super::editor_groups::ViewId;
use super::*;

/// The pinned header that has keyboard focus. It is valid only for the view and scroll position
/// it was set at; scrolling or switching views drops it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StickyFocus {
    pub view: ViewId,
    pub scroll_line: usize,
    pub index: usize,
}

/// Most headers pinned at once.
pub(crate) const MAX_STICKY_HEADERS: usize = 5;

impl App {
    /// The headers pinned above `scroll_line` of `document`, outermost first. Empty when sticky
    /// scroll is off or the language has no scopes.
    pub(crate) fn sticky_headers_for(
        &mut self,
        document: DocumentId,
        scroll_line: usize,
    ) -> Vec<graphite_syntax::ScopeHeader> {
        if !self.view_preferences.sticky_scroll {
            return Vec::new();
        }
        let revision = self.documents[document].buffer.revision();
        let cached = self.documents[document]
            .sticky_headers_cache
            .as_ref()
            .filter(|(cached_revision, cached_line, _)| {
                *cached_revision == revision && *cached_line == scroll_line
            })
            .map(|(_, _, headers)| headers.clone());
        let headers = cached.unwrap_or_else(|| {
            // Reuses the syntax tree the highlighter keeps instead of parsing the file again.
            self.refresh_document_highlights(document);
            let tab = &self.documents[document];
            let headers = tab
                .language
                .and_then(|_| {
                    tab.highlighter
                        .enclosing_scopes(tab.buffer.rope(), scroll_line)
                })
                .unwrap_or_default();
            self.documents[document].sticky_headers_cache =
                Some((revision, scroll_line, headers.clone()));
            headers
        });
        let mut headers: Vec<_> = headers
            .into_iter()
            .filter(|header| header.line < scroll_line)
            .collect();
        if headers.len() > MAX_STICKY_HEADERS {
            headers.drain(..headers.len() - MAX_STICKY_HEADERS);
        }
        headers
    }

    /// The focused header index for the view `view` drawn at `scroll_line`, if it is still valid.
    pub(crate) fn sticky_focus_index(&self, view: ViewId, scroll_line: usize) -> Option<usize> {
        self.sticky_focus
            .filter(|focus| focus.view == view && focus.scroll_line == scroll_line)
            .map(|focus| focus.index)
    }

    /// Headers of the active view as they are pinned right now.
    fn active_sticky_headers(&mut self) -> (ViewId, usize, Vec<graphite_syntax::ScopeHeader>) {
        let view = self.editor_groups.active_view();
        let (id, scroll_line, document) = (view.id, view.scroll_line, view.document);
        let document = self.resolve_document(document).unwrap_or(document);
        let headers = self.sticky_headers_for(document, scroll_line);
        (id, scroll_line, headers)
    }

    /// Puts keyboard focus on the innermost pinned header.
    pub(crate) fn focus_sticky_scroll(&mut self) {
        let (view, scroll_line, headers) = self.active_sticky_headers();
        if headers.is_empty() {
            self.sticky_focus = None;
            self.status = self.t("sticky_scroll.none").into();
            return;
        }
        self.sticky_focus = Some(StickyFocus {
            view,
            scroll_line,
            index: headers.len() - 1,
        });
    }

    /// Moves the focus to the next (`forward`) or previous pinned header, stopping at the ends.
    pub(crate) fn select_sticky_scroll_line(&mut self, forward: bool) {
        let (view, scroll_line, headers) = self.active_sticky_headers();
        let Some(current) = self.sticky_focus_index(view, scroll_line) else {
            self.status = self.t("sticky_scroll.not_focused").into();
            return;
        };
        let last = headers.len().saturating_sub(1);
        let index = if forward {
            (current + 1).min(last)
        } else {
            current.saturating_sub(1)
        };
        self.sticky_focus = Some(StickyFocus {
            view,
            scroll_line,
            index,
        });
    }

    /// Moves the cursor to the focused header and returns focus to the text.
    pub(crate) fn go_to_focused_sticky_scroll_line(&mut self) {
        let (view, scroll_line, headers) = self.active_sticky_headers();
        let Some(index) = self.sticky_focus_index(view, scroll_line) else {
            self.status = self.t("sticky_scroll.not_focused").into();
            return;
        };
        self.sticky_focus = None;
        let Some(header) = headers.get(index) else {
            return;
        };
        let line = header.line;
        self.editor_mut().set_cursor_line_col(line, 0);
        self.push_navigation_location();
        self.reveal_cursor = true;
    }

    /// Returns focus from the pinned headers to the text without moving the cursor.
    pub(crate) fn focus_editor_text(&mut self) {
        self.sticky_focus = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str =
        "struct Store;\nimpl Store {\n    fn load(&self) {\n        let x = 1;\n        let y = 2;\n    }\n}\n";

    fn app_scrolled_into_a_method(scroll_line: usize) -> (App, crate::test_support::TempDir) {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("store.rs");
        std::fs::write(&path, SOURCE).unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.view_preferences.sticky_scroll = true;
        app.editor_groups.active_view_mut().scroll_line = scroll_line;
        (app, dir)
    }

    fn focused(app: &App) -> Option<usize> {
        let view = app.editor_groups.active_view();
        app.sticky_focus_index(view.id, view.scroll_line)
    }

    #[test]
    fn pinned_headers_follow_edits_through_the_reused_syntax_tree() {
        let (mut app, _dir) = app_scrolled_into_a_method(3);
        let document = app.active_document();
        let before: Vec<_> = app
            .sticky_headers_for(document, 3)
            .into_iter()
            .map(|header| header.line)
            .collect();
        assert!(!before.is_empty());

        // Two new lines at the top move every declaration down by two.
        app.editor_mut().set_selection(0, 0);
        app.editor_mut().insert("// one\n// two\n");
        let after: Vec<_> = app
            .sticky_headers_for(document, 5)
            .into_iter()
            .map(|header| header.line)
            .collect();

        assert_eq!(
            after,
            before.iter().map(|line| line + 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn focus_starts_on_the_innermost_header_and_moves_within_the_ends() {
        let (mut app, _dir) = app_scrolled_into_a_method(3);
        assert_eq!(focused(&app), None);
        app.run_ui_command("editor.action.focusStickyScroll");
        assert_eq!(focused(&app), Some(1));
        app.run_ui_command("editor.action.selectNextStickyScrollLine");
        assert_eq!(focused(&app), Some(1));
        app.run_ui_command("editor.action.selectPreviousStickyScrollLine");
        assert_eq!(focused(&app), Some(0));
        app.run_ui_command("editor.action.selectPreviousStickyScrollLine");
        assert_eq!(focused(&app), Some(0));
        app.run_ui_command("editor.action.selectNextStickyScrollLine");
        assert_eq!(focused(&app), Some(1));
    }

    #[test]
    fn going_to_the_focused_line_moves_the_cursor_and_drops_the_focus() {
        let (mut app, _dir) = app_scrolled_into_a_method(3);
        app.run_ui_command("editor.action.focusStickyScroll");
        app.run_ui_command("editor.action.selectPreviousStickyScrollLine");
        app.run_ui_command("editor.action.goToFocusedStickyScrollLine");
        assert_eq!(app.editor().cursor_line_col(), (1, 0));
        assert_eq!(focused(&app), None);
        assert!(app.reveal_cursor);
    }

    #[test]
    fn the_focus_is_dropped_when_the_view_scrolls_or_on_select_editor() {
        let (mut app, _dir) = app_scrolled_into_a_method(3);
        app.run_ui_command("editor.action.focusStickyScroll");
        app.editor_groups.active_view_mut().scroll_line = 4;
        assert_eq!(focused(&app), None);
        app.editor_groups.active_view_mut().scroll_line = 3;
        app.run_ui_command("editor.action.focusStickyScroll");
        app.run_ui_command("editor.action.selectEditor");
        assert_eq!(focused(&app), None);
        assert_eq!(app.editor().cursor_line_col(), (0, 0));
    }

    #[test]
    fn without_pinned_headers_nothing_takes_focus_and_the_status_says_so() {
        let (mut app, _dir) = app_scrolled_into_a_method(0);
        app.run_ui_command("editor.action.focusStickyScroll");
        assert_eq!(focused(&app), None);
        assert_eq!(app.status, app.t("sticky_scroll.none"));

        let (mut app, _dir) = app_scrolled_into_a_method(3);
        app.view_preferences.sticky_scroll = false;
        app.run_ui_command("editor.action.focusStickyScroll");
        assert_eq!(focused(&app), None);
    }

    #[test]
    fn moving_or_going_without_focus_only_reports_it() {
        let (mut app, _dir) = app_scrolled_into_a_method(3);
        for id in [
            "editor.action.selectNextStickyScrollLine",
            "editor.action.goToFocusedStickyScrollLine",
        ] {
            app.status.clear();
            app.run_ui_command(id);
            assert_eq!(app.status, app.t("sticky_scroll.not_focused"), "{id}");
        }
        assert_eq!(app.editor().cursor_line_col(), (0, 0));
    }
}
