//! Selection and caret commands: the selection anchor, moving selected text sideways, bracket
//! navigation and removal, swapping letters, and replacing a value with its next or previous one.

use super::*;
use std::ops::Range;

/// Words that cycle through each other, like `true` and `false`.
const VALUE_SETS: [&[&str]; 4] = [
    &["true", "false"],
    &["True", "False"],
    &[
        "Private",
        "Public",
        "Friend",
        "ReadOnly",
        "Partial",
        "Protected",
        "WriteOnly",
    ],
    &["public", "protected", "private"],
];

/// The value that replaces `text` when stepping up (next) or down (previous): a number moves by
/// one (a fractional part is kept, with trailing zeros dropped), a word from a value set moves to
/// its neighbor in the set.
pub fn next_value(text: &str, up: bool) -> Option<String> {
    if let Some(number) = step_number(text, up) {
        return Some(number);
    }
    VALUE_SETS.iter().find_map(|set| {
        let index = set.iter().position(|value| *value == text)?;
        let next = if up {
            (index + 1) % set.len()
        } else {
            (index + set.len() - 1) % set.len()
        };
        Some(set[next].to_owned())
    })
}

fn step_number(text: &str, up: bool) -> Option<String> {
    let value: f64 = text.parse().ok()?;
    if !value.is_finite() || text.starts_with('+') || text.ends_with('.') {
        return None;
    }
    if value == 0.0 && !up {
        return None;
    }
    let decimals = text.rfind('.').map_or(0, |dot| text.len() - dot - 1);
    let precision = 10f64.powi(decimals as i32);
    let stepped = (value * precision).floor() + if up { precision } else { -precision };
    Some((stepped / precision).to_string())
}

impl Editor<'_> {
    /// Remembers the cursor position. The mark follows the text as it is edited.
    pub fn set_selection_anchor(&mut self) {
        self.state.mark = Some(self.state.cursor.char_idx);
    }

    pub fn selection_anchor(&self) -> Option<usize> {
        self.state.mark
    }

    pub fn clear_selection_anchor(&mut self) {
        self.state.mark = None;
    }

    /// Moves the cursor to the anchor. Returns `false` when no anchor is set.
    pub fn go_to_selection_anchor(&mut self) -> bool {
        let Some(mark) = self.state.mark else {
            return false;
        };
        self.set_selection(mark, mark);
        true
    }

    /// Selects from the anchor to the cursor and drops the anchor. Returns `false` when no
    /// anchor is set.
    pub fn select_from_selection_anchor(&mut self) -> bool {
        let Some(mark) = self.state.mark else {
            return false;
        };
        let head = self.state.cursor.char_idx;
        self.set_selection(mark, head);
        self.state.mark = None;
        true
    }

    /// The bracket pair a selection starting at `offset` refers to: the pair of a bracket at or
    /// before the offset, else the pair around it, else the pair of the next bracket.
    fn bracket_pair_near(&self, offset: usize, allow_next: bool) -> Option<(usize, usize)> {
        if let Some((a, b)) = matching_bracket(&self.rope, offset) {
            return Some((a.min(b), a.max(b)));
        }
        if let Some(pair) = enclosing_brackets(&self.rope, offset) {
            return Some(pair);
        }
        if !allow_next {
            return None;
        }
        let next = next_bracket(&self.rope, offset)?;
        matching_bracket(&self.rope, next).map(|(a, b)| (a.min(b), a.max(b)))
    }

    /// Moves each cursor to the bracket that pairs with the one it is on, else to the closing
    /// bracket around it, else to the next bracket.
    pub fn jump_to_bracket(&mut self) {
        let selections = self
            .selections()
            .into_iter()
            .map(|selection| {
                let at = selection.anchor.min(selection.head);
                let target = if let Some((_, other)) = matching_bracket(&self.rope, at) {
                    Some(other)
                } else if let Some((_, close)) = enclosing_brackets(&self.rope, at) {
                    Some(close)
                } else {
                    next_bracket(&self.rope, at)
                };
                let target = target.unwrap_or(at);
                Selection {
                    anchor: target,
                    head: target,
                }
            })
            .collect();
        self.set_selections(selections);
    }

    /// Selects each bracket pair, with the brackets or only what is inside them. Backwards when
    /// the cursor is on the closing bracket.
    pub fn select_to_bracket(&mut self, include_brackets: bool) {
        let mut selections = Vec::new();
        for selection in self.selections() {
            let at = selection.anchor.min(selection.head);
            let Some((open, close)) = self.bracket_pair_near(at, true) else {
                continue;
            };
            let (from, to) = if include_brackets {
                (open, close + 1)
            } else {
                (open + 1, close)
            };
            let on_close = (close..=close + 1).contains(&at);
            selections.push(if on_close {
                Selection {
                    anchor: to,
                    head: from,
                }
            } else {
                Selection {
                    anchor: from,
                    head: to,
                }
            });
        }
        if !selections.is_empty() {
            self.set_selections(selections);
        }
    }

    /// Deletes the pair of brackets at or around each cursor.
    pub fn remove_brackets(&mut self) {
        if self.read_only {
            return;
        }
        let mut pairs: Vec<(usize, usize)> = self
            .selections()
            .into_iter()
            .filter_map(|selection| self.bracket_pair_near(selection.head, false))
            .collect();
        pairs.sort();
        pairs.dedup();
        let mut edits = Vec::new();
        for (open, close) in pairs {
            edits.push(TextReplacement {
                range: open..open + 1,
                text: String::new(),
            });
            edits.push(TextReplacement {
                range: close..close + 1,
                text: String::new(),
            });
        }
        edits.sort_by_key(|edit| edit.range.start);
        edits.dedup_by_key(|edit| edit.range.start);
        let _ = self.apply_preserving_selections(&edits);
    }

    /// Moves each selected piece of text one character left or right along its line, the
    /// character it jumps over ending up on the other side.
    pub fn move_selected_text(&mut self, left: bool) {
        if self.read_only {
            return;
        }
        let selections = self.selections();
        let mut moves: Vec<(usize, TextReplacement)> = Vec::new();
        for (index, selection) in selections.iter().enumerate() {
            let start = selection.anchor.min(selection.head);
            let end = selection.anchor.max(selection.head);
            if start == end {
                continue;
            }
            let line = self.rope.char_to_line(start);
            if self.rope.char_to_line(end) != line {
                continue;
            }
            let line_start = self.rope.line_to_char(line);
            let line_end = line_start + self.line_content_len(line);
            let selected = self.rope.slice(start..end).to_string();
            if left {
                if start == line_start {
                    continue;
                }
                let jumped = self.rope.char(start - 1);
                moves.push((
                    index,
                    TextReplacement {
                        range: start - 1..end,
                        text: format!("{selected}{jumped}"),
                    },
                ));
            } else {
                if end >= line_end {
                    continue;
                }
                let jumped = self.rope.char(end);
                moves.push((
                    index,
                    TextReplacement {
                        range: start..end + 1,
                        text: format!("{jumped}{selected}"),
                    },
                ));
            }
        }
        if moves.is_empty() {
            return;
        }
        let edits: Vec<_> = moves.iter().map(|(_, edit)| edit.clone()).collect();
        let revision = self.revision();
        let Ok(ends) = self.apply_transaction(&edits) else {
            return;
        };
        if self.revision() == revision {
            return;
        }
        let mut after = self.selections();
        for ((index, edit), end) in moves.iter().zip(ends) {
            let length = edit.text.chars().count() - 1;
            let start = end - length - 1;
            if let Some(slot) = after.get_mut(*index) {
                *slot = if left {
                    Selection {
                        anchor: start,
                        head: start + length,
                    }
                } else {
                    Selection {
                        anchor: start + 1,
                        head: end,
                    }
                };
            }
        }
        self.set_selections(after);
        self.finish_selections();
    }

    /// Swaps the letters before and after each cursor; at the end of a line, the last two
    /// letters. Does nothing across a line break.
    pub fn transpose_letters(&mut self) {
        if self.read_only {
            return;
        }
        let mut edits: Vec<TextReplacement> = Vec::new();
        for selection in self.selections() {
            if selection.anchor != selection.head {
                continue;
            }
            let offset = selection.head;
            let line = self.rope.char_to_line(offset);
            let line_start = self.rope.line_to_char(line);
            let line_end = line_start + self.line_content_len(line);
            let (begin, end) = if offset == line_end {
                if offset < line_start + 2 {
                    continue;
                }
                (offset - 2, offset)
            } else {
                if offset == line_start {
                    continue;
                }
                (offset - 1, offset + 1)
            };
            let (left, right) = (self.rope.char(begin), self.rope.char(end - 1));
            edits.push(TextReplacement {
                range: begin..end,
                text: [right, left].iter().collect(),
            });
        }
        edits.sort_by_key(|edit| edit.range.start);
        edits.dedup_by(|later, earlier| later.range.start < earlier.range.end);
        let _ = self.apply_cursor_edits(&edits);
    }

    /// Replaces each selection (or the word or number at a cursor) with its next value when
    /// `up`, or its previous one. The replaced text stays selected.
    pub fn replace_with_next_value(&mut self, up: bool) {
        if self.read_only {
            return;
        }
        let mut edits: Vec<TextReplacement> = Vec::new();
        for selection in self.selections() {
            let start = selection.anchor.min(selection.head);
            let end = selection.anchor.max(selection.head);
            if self.rope.char_to_line(start) != self.rope.char_to_line(end) {
                continue;
            }
            let candidate = |range: Range<usize>| {
                let text = self.rope.slice(range.clone()).to_string();
                next_value(&text, up).map(|text| TextReplacement { range, text })
            };
            let edit = (start < end)
                .then(|| candidate(start..end))
                .flatten()
                .or_else(|| self.value_range_at(start).and_then(candidate));
            edits.extend(edit);
        }
        edits.sort_by_key(|edit| edit.range.start);
        edits.dedup_by(|later, earlier| later.range.start < earlier.range.end);
        if edits.is_empty() {
            return;
        }
        let revision = self.revision();
        let Ok(ends) = self.apply_transaction(&edits) else {
            return;
        };
        if self.revision() == revision {
            return;
        }
        let selections = edits
            .iter()
            .zip(ends)
            .map(|(edit, end)| Selection {
                anchor: end - edit.text.chars().count(),
                head: end,
            })
            .collect();
        self.set_selections(selections);
        self.finish_selections();
    }

    /// The word or decimal number around `offset`.
    fn value_range_at(&self, offset: usize) -> Option<Range<usize>> {
        let char_at = |index: usize| (index < self.len_chars()).then(|| self.rope.char(index));
        let part = |index: usize| match char_at(index) {
            Some(c) if c.is_alphanumeric() || c == '_' => true,
            Some('.') => {
                let before = index
                    .checked_sub(1)
                    .and_then(char_at)
                    .is_some_and(|c| c.is_ascii_digit());
                before && char_at(index + 1).is_some_and(|c| c.is_ascii_digit())
            }
            _ => false,
        };
        let mut start = offset;
        while start > 0 && part(start - 1) {
            start -= 1;
        }
        let mut end = offset;
        while part(end) {
            end += 1;
        }
        (start < end).then_some(start..end)
    }
}
