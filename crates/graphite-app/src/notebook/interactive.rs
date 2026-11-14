//! The interactive window and the REPL: a notebook bound to a kernel with an input box under the
//! cells. Whatever is submitted becomes a cell that runs, so the history is a notebook that can
//! be saved.

use super::model::CellKind;
use super::session::Session;
use graphite_text::{Buffer, SelectionState, ViewId};
use std::path::PathBuf;

/// Lines of the input box before it scrolls.
pub(crate) const MAX_INPUT_LINES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InteractiveKind {
    /// Enter adds a line; Shift+Enter or Ctrl+Enter runs.
    Window,
    /// Enter runs; Shift+Enter adds a line.
    Repl,
}

pub(crate) struct Interactive {
    pub kind: InteractiveKind,
    pub input: Buffer,
    pub input_selection: SelectionState,
    /// What was submitted before, oldest first.
    pub history: Vec<String>,
    /// The history entry shown in the input box, when one is.
    history_index: Option<usize>,
    /// What was typed before the history was entered, to come back to.
    draft: String,
    /// Keys go to the input box rather than to the cells.
    pub input_focused: bool,
    /// The language of the code sent from an editor, to choose a kernel.
    pub language_hint: Option<String>,
    /// The first cell is the empty one every notebook has, not yet used for anything.
    pub(super) placeholder: bool,
}

impl Interactive {
    fn new(kind: InteractiveKind) -> Self {
        Self {
            kind,
            input: Buffer::from_text(PathBuf::from("input"), ""),
            input_selection: SelectionState::default(),
            history: Vec::new(),
            history_index: None,
            draft: String::new(),
            input_focused: true,
            language_hint: None,
            placeholder: true,
        }
    }

    pub(crate) fn input_text(&self) -> String {
        self.input.text()
    }

    /// Lines the input box shows.
    pub(crate) fn input_lines(&self) -> usize {
        self.input.len_lines().clamp(1, MAX_INPUT_LINES)
    }

    fn set_input(&mut self, text: &str) {
        let end = self.input.len_chars();
        let mut editor = self.input.edit(&mut self.input_selection, ViewId(0));
        editor.set_selection(0, end);
        editor.insert(text);
        let end = editor.len_chars();
        editor.set_selection(end, end);
    }
}

impl Session {
    /// An empty interactive window or REPL.
    pub(crate) fn new_interactive(kind: InteractiveKind) -> Self {
        let mut session = Session::from_notebook(super::model::Notebook::new());
        session.interactive = Some(Interactive::new(kind));
        session
    }

    pub(crate) fn interactive(&self) -> Option<&Interactive> {
        self.interactive.as_ref()
    }

    /// Tells the window which language the code sent to it is in.
    pub(crate) fn set_language_hint(&mut self, language: Option<String>) {
        if let Some(state) = self.interactive.as_mut() {
            state.language_hint = language;
        }
    }

    /// Whether `index` is the unused first cell, which is neither drawn nor counted.
    pub(crate) fn is_hidden_cell(&self, index: usize) -> bool {
        index == 0
            && self
                .interactive
                .as_ref()
                .is_some_and(|state| state.placeholder)
    }

    /// Runs `action` on the input box.
    pub(crate) fn edit_input<R>(
        &mut self,
        action: impl FnOnce(&mut graphite_text::Editor<'_>) -> R,
    ) -> Option<R> {
        let state = self.interactive.as_mut()?;
        state.history_index = None;
        let mut editor = state.input.edit(&mut state.input_selection, ViewId(0));
        Some(action(&mut editor))
    }

    pub(crate) fn focus_input(&mut self, focused: bool) {
        if let Some(state) = self.interactive.as_mut() {
            state.input_focused = focused;
        }
    }

    pub(crate) fn clear_input(&mut self) {
        if let Some(state) = self.interactive.as_mut() {
            state.history_index = None;
            state.draft.clear();
            state.set_input("");
        }
    }

    /// Replaces the input with `text`.
    pub(crate) fn set_input_text(&mut self, text: &str) {
        if let Some(state) = self.interactive.as_mut() {
            state.history_index = None;
            state.set_input(text);
        }
    }

    /// Takes the input as a new cell at the end and empties the box. Returns the index of the
    /// cell, or `None` when the input is blank.
    pub(crate) fn submit_input(&mut self) -> Option<usize> {
        let state = self.interactive.as_mut()?;
        let code = state.input_text();
        if code.trim().is_empty() {
            return None;
        }
        if state.history.last() != Some(&code) {
            state.history.push(code.clone());
        }
        state.history_index = None;
        state.draft.clear();
        state.set_input("");
        let reuse = std::mem::replace(&mut state.placeholder, false);
        let index = if reuse {
            // The unused first cell takes the code instead of adding one next to it.
            self.cells[0] = super::session::CellState::new(
                self.cells[0].uid,
                super::model::Cell::new(CellKind::Code, &code),
            );
            0
        } else {
            let end = self.cells.len();
            self.insert_cell(end, CellKind::Code, &code)
        };
        self.set_focus(index, false);
        Some(index)
    }

    /// Shows the previous entry of the history in the input box. Returns whether it changed.
    pub(crate) fn history_previous(&mut self) -> bool {
        let Some(state) = self.interactive.as_mut() else {
            return false;
        };
        if state.history.is_empty() {
            return false;
        }
        let index = match state.history_index {
            None => {
                state.draft = state.input_text();
                state.history.len() - 1
            }
            Some(0) => return false,
            Some(index) => index - 1,
        };
        let text = state.history[index].clone();
        state.set_input(&text);
        state.history_index = Some(index);
        true
    }

    /// Shows the next entry of the history, and after the last one what was being typed.
    pub(crate) fn history_next(&mut self) -> bool {
        let Some(state) = self.interactive.as_mut() else {
            return false;
        };
        let Some(index) = state.history_index else {
            return false;
        };
        if index + 1 < state.history.len() {
            let text = state.history[index + 1].clone();
            state.set_input(&text);
            state.history_index = Some(index + 1);
        } else {
            let draft = std::mem::take(&mut state.draft);
            state.set_input(&draft);
            state.history_index = None;
        }
        true
    }

    /// Whether the caret is on the first (`up`) or last line of the input.
    pub(crate) fn input_caret_at_edge(&self, up: bool) -> bool {
        let Some(state) = self.interactive.as_ref() else {
            return false;
        };
        let (line, _) = state.input.view(&state.input_selection).cursor_line_col();
        if up {
            line == 0
        } else {
            line + 1 >= state.input.len_lines()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> Session {
        Session::new_interactive(InteractiveKind::Window)
    }

    fn type_input(session: &mut Session, text: &str) {
        session.edit_input(|editor| editor.insert(text));
    }

    fn input(session: &Session) -> String {
        session.interactive().unwrap().input_text()
    }

    fn sources(session: &Session) -> Vec<String> {
        session.cells.iter().map(|state| state.text()).collect()
    }

    #[test]
    fn a_new_window_has_a_focused_empty_input_and_a_hidden_first_cell() {
        let session = window();
        assert!(session.interactive().unwrap().input_focused);
        assert_eq!(input(&session), "");
        assert!(session.is_hidden_cell(0));
        assert_eq!(session.cells.len(), 1);
        // An ordinary notebook has no such thing.
        let notebook = Session::from_notebook(super::super::model::Notebook::new());
        assert!(notebook.interactive().is_none());
        assert!(!notebook.is_hidden_cell(0));
    }

    #[test]
    fn submitting_turns_the_input_into_a_cell_and_the_first_one_reuses_the_hidden_cell() {
        let mut session = window();
        type_input(&mut session, "x = 1");
        let uid = session.cells[0].uid;
        assert_eq!(session.submit_input(), Some(0));
        assert_eq!(sources(&session), ["x = 1"]);
        assert_eq!(session.cells[0].uid, uid);
        assert!(!session.is_hidden_cell(0));
        assert_eq!(input(&session), "");
        type_input(&mut session, "y = 2\nz = 3");
        assert_eq!(session.submit_input(), Some(1));
        assert_eq!(sources(&session), ["x = 1", "y = 2\nz = 3"]);
        assert_eq!(session.focus, 1);
        assert_eq!(
            session.interactive().unwrap().history,
            ["x = 1", "y = 2\nz = 3"]
        );
    }

    #[test]
    fn a_blank_input_submits_nothing() {
        let mut session = window();
        assert_eq!(session.submit_input(), None);
        type_input(&mut session, "  \n ");
        assert_eq!(session.submit_input(), None);
        assert!(session.is_hidden_cell(0));
        assert_eq!(session.interactive().unwrap().history.len(), 0);
    }

    #[test]
    fn the_history_walks_back_and_forward_and_returns_to_the_draft() {
        let mut session = window();
        for code in ["one", "two", "three"] {
            type_input(&mut session, code);
            session.submit_input();
        }
        type_input(&mut session, "draft");
        assert!(session.history_previous());
        assert_eq!(input(&session), "three");
        assert!(session.history_previous());
        assert!(session.history_previous());
        assert_eq!(input(&session), "one");
        // The oldest entry is the end of the walk.
        assert!(!session.history_previous());
        assert!(session.history_next());
        assert_eq!(input(&session), "two");
        assert!(session.history_next());
        assert!(session.history_next());
        assert_eq!(input(&session), "draft");
        assert!(!session.history_next());
    }

    #[test]
    fn repeating_the_last_entry_does_not_grow_the_history_and_editing_leaves_it() {
        let mut session = window();
        type_input(&mut session, "same");
        session.submit_input();
        type_input(&mut session, "same");
        session.submit_input();
        assert_eq!(session.interactive().unwrap().history, ["same"]);
        assert!(session.history_previous());
        // Typing while an entry is shown starts from it and ends the walk.
        type_input(&mut session, "!");
        assert!(!session.history_next());
        assert!(session.history_previous());
        assert_eq!(input(&session), "same");
        assert!(!session.history_previous() || input(&session) == "same");
    }

    #[test]
    fn without_history_the_walk_does_nothing() {
        let mut session = window();
        assert!(!session.history_previous());
        assert!(!session.history_next());
        let mut notebook = Session::from_notebook(super::super::model::Notebook::new());
        assert!(!notebook.history_previous());
        assert_eq!(notebook.submit_input(), None);
        assert!(notebook.edit_input(|_| ()).is_none());
    }

    #[test]
    fn clearing_and_setting_the_input() {
        let mut session = window();
        type_input(&mut session, "abc");
        session.clear_input();
        assert_eq!(input(&session), "");
        session.set_input_text("from an editor\nsecond line");
        assert_eq!(input(&session), "from an editor\nsecond line");
        assert_eq!(session.interactive().unwrap().input_lines(), 2);
        // Typing continues at the end.
        type_input(&mut session, "!");
        assert!(input(&session).ends_with("second line!"));
    }

    #[test]
    fn the_input_box_grows_up_to_a_limit() {
        let mut session = window();
        assert_eq!(session.interactive().unwrap().input_lines(), 1);
        session.set_input_text(&"x\n".repeat(30));
        assert_eq!(
            session.interactive().unwrap().input_lines(),
            MAX_INPUT_LINES
        );
    }

    #[test]
    fn the_caret_edge_tells_when_arrows_should_walk_the_history() {
        let mut session = window();
        session.set_input_text("a\nb\nc");
        // The caret is at the end, on the last line.
        assert!(session.input_caret_at_edge(false));
        assert!(!session.input_caret_at_edge(true));
        session.edit_input(|editor| editor.set_cursor_line_col(0, 0));
        assert!(session.input_caret_at_edge(true));
        assert!(!session.input_caret_at_edge(false));
    }

    #[test]
    fn focus_moves_between_the_input_and_the_cells() {
        let mut session = window();
        session.focus_input(false);
        assert!(!session.interactive().unwrap().input_focused);
        session.focus_input(true);
        assert!(session.interactive().unwrap().input_focused);
    }

    #[test]
    fn the_history_is_a_notebook_that_exports_with_the_code_and_no_placeholder_left() {
        let mut session = window();
        type_input(&mut session, "1 + 1");
        session.submit_input();
        let notebook = session.export();
        assert_eq!(notebook.cells.len(), 1);
        assert_eq!(notebook.cells[0].source, "1 + 1");
    }
}
