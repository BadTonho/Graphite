//! Folding the sections of a notebook, and moving between the cells that stay visible.

use super::sections::{enclosing_heading, heading_level, section_body, section_end};
use super::session::Session;

/// The heading of the section that holds the section of `heading`.
fn parent_heading(session: &Session, heading: usize) -> Option<usize> {
    let level = heading_level(session, heading)?;
    (0..heading)
        .rev()
        .find(|&before| heading_level(session, before).is_some_and(|other| other < level))
}

impl Session {
    /// Which cells are not drawn: the unused first cell of an interactive window, and the cells
    /// inside a folded section.
    pub(crate) fn hidden_cells(&self) -> Vec<bool> {
        let mut hidden = vec![false; self.cells.len()];
        let mut index = 0;
        while index < self.cells.len() {
            if self.is_hidden_cell(index) {
                hidden[index] = true;
            }
            if self.folded.contains(&self.cells[index].uid) && heading_level(self, index).is_some() {
                let end = section_end(self, index);
                hidden[index + 1..end].iter_mut().for_each(|slot| *slot = true);
                index = end;
                continue;
            }
            index += 1;
        }
        hidden
    }

    pub(crate) fn is_folded(&self, index: usize) -> bool {
        self.cells
            .get(index)
            .is_some_and(|state| self.folded.contains(&state.uid))
            && heading_level(self, index).is_some()
    }

    /// How many cells a folded heading hides.
    pub(crate) fn folded_count(&self, index: usize) -> usize {
        if self.is_folded(index) {
            section_end(self, index) - index - 1
        } else {
            0
        }
    }

    /// Folds the section the focused cell belongs to and focuses its heading. Returns whether
    /// there was a section to fold.
    pub(crate) fn fold_at_focus(&mut self) -> bool {
        let Some(heading) = enclosing_heading(self, self.focus) else {
            return false;
        };
        if section_end(self, heading) <= heading + 1 {
            // Nothing under it to hide.
            return false;
        }
        self.folded.insert(self.cells[heading].uid);
        self.set_focus(heading, false);
        true
    }

    /// Unfolds the folded section that holds the focused cell. Returns whether one was folded.
    pub(crate) fn unfold_at_focus(&mut self) -> bool {
        let mut candidate = enclosing_heading(self, self.focus);
        while let Some(heading) = candidate {
            if self.folded.remove(&self.cells[heading].uid) {
                self.set_focus(heading, false);
                return true;
            }
            candidate = parent_heading(self, heading);
        }
        false
    }

    pub(crate) fn fold_all(&mut self) {
        let uids: Vec<u64> = (0..self.cells.len())
            .filter(|&index| heading_level(self, index).is_some() && section_end(self, index) > index + 1)
            .map(|index| self.cells[index].uid)
            .collect();
        self.folded.extend(uids);
        // The focus must not stay inside what was hidden.
        self.move_focus_out_of_hidden();
    }

    pub(crate) fn unfold_all(&mut self) {
        self.folded.clear();
    }

    /// Unfolds every section that hides cell `index`.
    pub(crate) fn reveal(&mut self, index: usize) {
        let hiding: Vec<u64> = (0..index)
            .filter(|&heading| self.is_folded(heading) && section_end(self, heading) > index)
            .map(|heading| self.cells[heading].uid)
            .collect();
        for uid in hiding {
            self.folded.remove(&uid);
        }
    }

    /// When the focus is on a hidden cell, moves it to the heading that hides it.
    pub(crate) fn move_focus_out_of_hidden(&mut self) {
        let hidden = self.hidden_cells();
        if !hidden.get(self.focus).copied().unwrap_or(false) {
            return;
        }
        let mut target = self.focus;
        while target > 0 && hidden[target] {
            target -= 1;
        }
        self.set_focus(target, false);
    }

    /// The cells of the section of the focused cell, for running it.
    pub(crate) fn focused_section(&self) -> Option<std::ops::Range<usize>> {
        section_body(self, self.focus)
    }

    /// Moves the focus by `delta` visible cells, skipping the ones that are folded away.
    pub(crate) fn focus_visible_by(&mut self, delta: isize, extend: bool) {
        let hidden = self.hidden_cells();
        let step = delta.signum();
        let mut target = self.focus;
        let mut remaining = delta.abs();
        while remaining > 0 {
            let next = target as isize + step;
            if next < 0 || next >= self.cells.len() as isize {
                break;
            }
            target = next as usize;
            if !hidden[target] {
                remaining -= 1;
            }
        }
        // Running off the end onto hidden cells goes back to the last one that is shown.
        while hidden[target] && target != self.focus {
            target = (target as isize - step) as usize;
        }
        self.set_focus(target, extend);
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{Cell, CellKind, Notebook};
    use super::*;
    use CellKind::{Code, Markdown};

    fn session() -> Session {
        let mut notebook = Notebook::new();
        notebook.cells = [
            ("# One", Markdown),
            ("a", Code),
            ("## One A", Markdown),
            ("b", Code),
            ("# Two", Markdown),
            ("c", Code),
            ("tail", Code),
        ]
        .iter()
        .map(|(source, kind)| Cell::new(*kind, source))
        .collect();
        Session::from_notebook(notebook)
    }

    fn visible(session: &Session) -> Vec<usize> {
        session
            .hidden_cells()
            .iter()
            .enumerate()
            .filter(|(_, hidden)| !**hidden)
            .map(|(index, _)| index)
            .collect()
    }

    #[test]
    fn folding_hides_the_section_below_the_heading_but_not_the_heading() {
        let mut session = session();
        session.set_focus(3, false);
        // The focused code cell belongs to "## One A", which is folded and focused.
        assert!(session.fold_at_focus());
        assert_eq!(session.focus, 2);
        assert_eq!(visible(&session), [0, 1, 2, 4, 5, 6]);
        assert!(session.is_folded(2));
        assert_eq!(session.folded_count(2), 1);
        // The outer heading folds everything up to the next "# ".
        session.set_focus(0, false);
        assert!(session.fold_at_focus());
        assert_eq!(visible(&session), [0, 4, 5, 6]);
        assert_eq!(session.folded_count(0), 3);
    }

    #[test]
    fn unfolding_opens_the_nearest_folded_section_around_the_focus() {
        let mut session = session();
        session.fold_all();
        assert_eq!(visible(&session), [0, 4]);
        session.set_focus(4, false);
        assert!(session.unfold_at_focus());
        assert_eq!(visible(&session), [0, 4, 5, 6]);
        assert!(!session.unfold_at_focus());
        session.set_focus(0, false);
        assert!(session.unfold_at_focus());
        // "## One A" was folded too and stays so.
        assert_eq!(visible(&session), [0, 1, 2, 4, 5, 6]);
        session.unfold_all();
        assert_eq!(visible(&session), [0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn a_heading_with_nothing_under_it_cannot_fold_and_a_text_before_any_heading_neither() {
        let mut notebook = Notebook::new();
        notebook.cells = vec![
            Cell::new(Code, "before"),
            Cell::new(Markdown, "# Alone"),
        ];
        let mut session = Session::from_notebook(notebook);
        session.set_focus(1, false);
        assert!(!session.fold_at_focus());
        session.set_focus(0, false);
        assert!(!session.fold_at_focus());
        assert!(!session.unfold_at_focus());
        assert!(visible(&session).len() == 2);
    }

    #[test]
    fn fold_all_moves_a_focus_that_would_be_hidden_to_its_heading() {
        let mut session = session();
        session.set_focus(3, false);
        session.fold_all();
        assert_eq!(session.focus, 0);
        assert!(!session.hidden_cells()[session.focus]);
    }

    #[test]
    fn moving_the_focus_skips_hidden_cells_and_stops_at_the_ends() {
        let mut session = session();
        session.set_focus(0, false);
        session.fold_at_focus();
        session.focus_visible_by(1, false);
        assert_eq!(session.focus, 4);
        session.focus_visible_by(-1, false);
        assert_eq!(session.focus, 0);
        session.focus_visible_by(2, false);
        assert_eq!(session.focus, 5);
        session.focus_visible_by(10, false);
        assert_eq!(session.focus, 6);
        session.focus_visible_by(-10, false);
        assert_eq!(session.focus, 0);
        // A fold at the very end leaves the last shown cell as the end.
        let mut session = super::tests::session();
        session.set_focus(4, false);
        session.fold_at_focus();
        session.focus_visible_by(5, false);
        assert_eq!(session.focus, 4);
    }


    #[test]
    fn revealing_a_cell_opens_every_fold_above_it() {
        let mut session = session();
        session.fold_all();
        assert_eq!(visible(&session), [0, 4]);
        session.reveal(3);
        assert!(visible(&session).contains(&3));
        assert!(visible(&session).contains(&2));
        // Cells that were not inside the opened sections stay as they were.
        assert!(session.is_folded(4));
    }
    #[test]
    fn the_section_of_the_focus_is_what_runs_with_a_section_command() {
        let mut session = session();
        session.set_focus(3, false);
        assert_eq!(session.focused_section(), Some(3..4));
        session.set_focus(0, false);
        assert_eq!(session.focused_section(), Some(1..4));
    }
}
