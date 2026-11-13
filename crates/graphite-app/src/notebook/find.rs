//! Searching the text of the cells of a notebook.

use super::session::Session;
use graphite_text::ViewId;

/// Lowercase, one character for one, so that positions in the result are positions in the text.
fn fold(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

/// Start positions of `needle` in `haystack`.
fn matches(haystack: &[char], needle: &[char]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    (0..=haystack.len() - needle.len())
        .filter(|&start| haystack[start..start + needle.len()] == *needle)
        .collect()
}

/// A line of some cell that holds the search text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FoundLine {
    pub cell: usize,
    pub line: usize,
    pub text: String,
}

impl Session {
    /// Every line of every cell that holds `query`, ignoring case, in document order. At most
    /// `limit` are returned.
    pub(crate) fn find_lines(&self, query: &str, limit: usize) -> Vec<FoundLine> {
        let needle = fold(query);
        let mut found = Vec::new();
        if needle.is_empty() {
            return found;
        }
        for (cell, state) in self.cells.iter().enumerate() {
            for (line, text) in state.text().lines().enumerate() {
                if !matches(&fold(text), &needle).is_empty() {
                    found.push(FoundLine {
                        cell,
                        line,
                        text: text.to_owned(),
                    });
                    if found.len() >= limit {
                        return found;
                    }
                }
            }
        }
        found
    }

    /// Every non-empty line of every cell, in order, up to `limit`; what the search list shows.
    pub(crate) fn all_lines(&self, limit: usize) -> Vec<FoundLine> {
        let mut lines = Vec::new();
        for (cell, state) in self.cells.iter().enumerate() {
            for (line, text) in state.text().lines().enumerate() {
                if text.trim().is_empty() {
                    continue;
                }
                lines.push(FoundLine {
                    cell,
                    line,
                    text: text.to_owned(),
                });
                if lines.len() >= limit {
                    return lines;
                }
            }
        }
        lines
    }

    /// Selects the first match of `query` on `line` of `cell` and starts editing there.
    pub(crate) fn goto_match(&mut self, cell: usize, line: usize, query: &str) -> bool {
        let Some(state) = self.cells.get(cell) else {
            return false;
        };
        let text = state.text();
        let line_start: usize = text.lines().take(line).map(|text| text.chars().count() + 1).sum();
        let content = text.lines().nth(line).unwrap_or("");
        let needle = fold(query);
        let column = matches(&fold(content), &needle).first().copied().unwrap_or(0);
        let length = if needle.is_empty() { 0 } else { needle.len() };
        self.select_range(cell, line_start + column, line_start + column + length);
        true
    }

    /// Focuses `cell` in edit mode with the characters from `start` to `end` selected, opening
    /// any folded section that hid it.
    fn select_range(&mut self, cell: usize, start: usize, end: usize) {
        self.reveal(cell);
        self.set_focus(cell, false);
        self.enter_edit();
        let state = &mut self.cells[cell];
        let mut editor = state.buffer.edit(&mut state.selection, ViewId(state.uid));
        editor.set_selection(start, end);
    }

    /// Moves to the next (or previous) match of the last search, wrapping around the notebook.
    /// Returns whether there was one.
    pub(crate) fn find_step(&mut self, forward: bool) -> bool {
        let Some(query) = self.find_query.clone().filter(|query| !query.is_empty()) else {
            return false;
        };
        let needle = fold(&query);
        let count = self.cells.len();
        let (sel_start, sel_end) = {
            let state = &self.cells[self.focus];
            let view = state.buffer.view(&state.selection);
            match view.selection() {
                Some(selection) => (
                    selection.anchor.min(selection.head),
                    selection.anchor.max(selection.head),
                ),
                None => {
                    let cursor = view.cursor().char_idx;
                    (cursor, cursor)
                }
            }
        };
        // The focused cell is looked at twice: first beyond the caret, and last (after going all
        // the way around) from its other side.
        for step in 0..=count {
            let index = if forward {
                (self.focus + step) % count
            } else {
                (self.focus + count - step % count) % count
            };
            let positions = matches(&fold(&self.cells[index].text()), &needle);
            let wanted = if step == 0 {
                if forward {
                    positions.iter().copied().find(|&start| start >= sel_end)
                } else {
                    positions.iter().copied().rev().find(|&start| start + needle.len() <= sel_start)
                }
            } else if forward {
                positions.first().copied()
            } else {
                positions.last().copied()
            };
            if let Some(start) = wanted {
                self.select_range(index, start, start + needle.len());
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{Cell, CellKind, Notebook};
    use super::*;

    fn session(sources: &[&str]) -> Session {
        let mut notebook = Notebook::new();
        notebook.cells = sources
            .iter()
            .map(|source| Cell::new(CellKind::Code, source))
            .collect();
        Session::from_notebook(notebook)
    }

    fn selection(session: &Session) -> (usize, usize, usize) {
        let state = &session.cells[session.focus];
        let selection = state.buffer.view(&state.selection).selection().unwrap();
        (session.focus, selection.anchor, selection.head)
    }

    #[test]
    fn lines_with_the_text_are_listed_in_order_ignoring_case() {
        let session = session(&["Alpha\nbeta", "no match", "xx ALPHA yy\nalpha"]);
        let found = session.find_lines("alpha", 10);
        assert_eq!(
            found.iter().map(|line| (line.cell, line.line)).collect::<Vec<_>>(),
            [(0, 0), (2, 0), (2, 1)]
        );
        assert_eq!(found[1].text, "xx ALPHA yy");
        assert_eq!(session.find_lines("alpha", 2).len(), 2);
        assert!(session.find_lines("", 10).is_empty());
        assert!(session.find_lines("zzz", 10).is_empty());
    }


    #[test]
    fn the_search_list_has_every_non_empty_line_up_to_the_limit() {
        let session = session(&["a\n\n  \nb", "c"]);
        let lines = session.all_lines(10);
        assert_eq!(
            lines.iter().map(|line| (line.cell, line.line, line.text.as_str())).collect::<Vec<_>>(),
            [(0, 0, "a"), (0, 3, "b"), (1, 0, "c")]
        );
        assert_eq!(session.all_lines(2).len(), 2);
    }
    #[test]
    fn going_to_a_match_selects_it_and_starts_editing() {
        let mut session = session(&["a", "one\ntwo needle here"]);
        assert!(session.goto_match(1, 1, "NEEDLE"));
        // "one\n" is four characters, "two " four more.
        assert_eq!(selection(&session), (1, 8, 14));
        assert_eq!(session.mode, super::super::session::Mode::Edit);
        assert!(!session.goto_match(9, 0, "x"));
    }

    #[test]
    fn stepping_forward_and_back_visits_every_match_and_wraps() {
        let mut session = session(&["x foo x foo", "none", "foo"]);
        session.find_query = Some("FOO".into());
        assert!(session.find_step(true));
        assert_eq!(selection(&session), (0, 2, 5));
        assert!(session.find_step(true));
        assert_eq!(selection(&session), (0, 8, 11));
        assert!(session.find_step(true));
        assert_eq!(selection(&session), (2, 0, 3));
        // After the last match the search wraps to the first.
        assert!(session.find_step(true));
        assert_eq!(selection(&session), (0, 2, 5));
        assert!(session.find_step(false));
        assert_eq!(selection(&session), (2, 0, 3));
        assert!(session.find_step(false));
        assert_eq!(selection(&session), (0, 8, 11));
        assert!(session.find_step(false));
        assert_eq!(selection(&session), (0, 2, 5));
    }

    #[test]
    fn a_single_match_finds_itself_again_after_going_around() {
        let mut session = session(&["only foo here", "b"]);
        session.find_query = Some("foo".into());
        assert!(session.find_step(true));
        assert!(session.find_step(true));
        assert_eq!(selection(&session), (0, 5, 8));
        assert!(session.find_step(false));
        assert_eq!(selection(&session), (0, 5, 8));
    }

    #[test]
    fn nothing_to_find_without_a_query_or_a_match() {
        let mut session = session(&["abc"]);
        assert!(!session.find_step(true));
        session.find_query = Some(String::new());
        assert!(!session.find_step(true));
        session.find_query = Some("zzz".into());
        assert!(!session.find_step(false));
    }

    #[test]
    fn a_match_inside_a_folded_section_opens_it() {
        let mut notebook = Notebook::new();
        notebook.cells = vec![
            Cell::new(CellKind::Markdown, "# Head"),
            Cell::new(CellKind::Code, "hidden needle"),
        ];
        let mut session = Session::from_notebook(notebook);
        session.fold_all();
        assert!(session.hidden_cells()[1]);
        session.find_query = Some("needle".into());
        assert!(session.find_step(true));
        assert_eq!(session.focus, 1);
        assert!(!session.hidden_cells()[1]);
    }

    #[test]
    fn positions_are_characters_not_bytes() {
        let mut session = session(&["olá 🌎 alvo"]);
        session.find_query = Some("alvo".into());
        assert!(session.find_step(true));
        assert_eq!(selection(&session), (0, 6, 10));
    }
}
