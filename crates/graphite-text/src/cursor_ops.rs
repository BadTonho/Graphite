//! Multi-cursor commands: cursors down to the end of the text, rotating the primary cursor, and
//! moving the last selection to the next or previous match.

use super::*;

impl SelectionState {
    /// Sets the selections keeping the given order, so the first one stays the primary cursor.
    /// Unlike `set_selections` this neither sorts nor merges; callers pass disjoint selections.
    pub(super) fn set_ordered_selections(&mut self, mut selections: Vec<Selection>, len: usize) {
        if selections.is_empty() {
            return;
        }
        for selection in &mut selections {
            selection.anchor = selection.anchor.min(len);
            selection.head = selection.head.min(len);
        }
        let primary = selections.remove(0);
        self.cursor.char_idx = primary.head;
        self.selection = (primary.anchor != primary.head).then_some(primary);
        self.extra_selections = selections;
    }
}

impl Editor<'_> {
    /// Adds a cursor (or a copy of the selection's columns) on every line from the primary
    /// selection to the last line, or to the first when `bottom` is false. Lines that are too
    /// short get the cursor at their end.
    pub fn add_cursors_to_edge(&mut self, bottom: bool) {
        let primary = self.selections()[0];
        let start = primary.anchor.min(primary.head);
        let end = primary.anchor.max(primary.head);
        let start_line = self.rope.char_to_line(start);
        let start_column = start - self.rope.line_to_char(start_line);
        let end_column = end - self.rope.line_to_char(self.rope.char_to_line(end));
        let lines: Vec<usize> = if bottom {
            (start_line..self.len_lines()).collect()
        } else {
            (0..=start_line).rev().collect()
        };
        let selections = lines
            .into_iter()
            .map(|line| {
                let first = self.rope.line_to_char(line);
                let length = self.line_content_len(line);
                Selection {
                    anchor: first + start_column.min(length),
                    head: first + end_column.min(length),
                }
            })
            .collect();
        self.set_selections(selections);
    }

    /// Makes the next (or previous) selection the primary one; the others keep their positions.
    pub fn focus_cursor(&mut self, next: bool) {
        let mut selections = self.selections();
        if selections.len() < 2 {
            return;
        }
        if next {
            selections.rotate_left(1);
        } else {
            selections.rotate_right(1);
        }
        let len = self.len_chars();
        self.state.set_ordered_selections(selections, len);
    }

    /// Replaces the last selection (or the first, when going backwards) with the next
    /// occurrence of its text. Nothing changes when there is no other occurrence.
    pub fn move_selection_to_occurrence(&mut self, next: bool) {
        let before = self.selections();
        let moved = if next { before.last() } else { before.first() }.copied();
        if next {
            self.select_occurrences(false);
        } else {
            self.select_previous_occurrence();
        }
        let after = self.selections();
        let Some(moved) = moved.filter(|_| after.len() > before.len()) else {
            return;
        };
        let remaining = after
            .into_iter()
            .filter(|selection| *selection != moved)
            .collect();
        self.set_selections(remaining);
    }
}
