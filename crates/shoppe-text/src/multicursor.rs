use super::*;

#[derive(Debug, Clone, Copy)]
pub enum CursorMotion {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

impl Buffer {
    /// Complete the word at each cursor, or replace each selected range, as one edit.
    pub fn complete_word(&mut self, text: &str) {
        if self.read_only {
            return;
        }
        let mut ranges: Vec<_> = self
            .selections()
            .into_iter()
            .map(|selection| {
                let mut start = selection.anchor.min(selection.head);
                let end = selection.anchor.max(selection.head);
                if start == end {
                    while start > 0 && is_word(self.rope.char(start - 1)) {
                        start -= 1;
                    }
                }
                start..end
            })
            .collect();
        ranges.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<std::ops::Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut() {
                if range.start < last.end || range.start == last.start {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        let edits: Vec<_> = merged
            .into_iter()
            .map(|range| TextReplacement {
                range,
                text: text.into(),
            })
            .collect();
        self.apply_cursor_replacements(&edits);
    }

    /// Apply edits while keeping cursor and selection positions attached to the text.
    pub(super) fn apply_preserving_selections(
        &mut self,
        edits: &[TextReplacement],
    ) -> Result<(), TransactionError> {
        let before = self.selections();
        let mut ordered: Vec<_> = edits.iter().collect();
        ordered.sort_by_key(|edit| edit.range.start);
        let map_offset = |offset: usize| {
            let mut delta = 0isize;
            for edit in &ordered {
                let inserted = edit.text.chars().count();
                if offset < edit.range.start {
                    break;
                }
                if offset <= edit.range.end {
                    return edit.range.start.saturating_add_signed(delta)
                        + (offset - edit.range.start).min(inserted);
                }
                delta += inserted as isize - (edit.range.end - edit.range.start) as isize;
            }
            offset.saturating_add_signed(delta)
        };
        let after: Vec<_> = before
            .into_iter()
            .map(|s| Selection {
                anchor: map_offset(s.anchor),
                head: map_offset(s.head),
            })
            .collect();
        let revision = self.revision();
        self.apply_transaction(edits)?;
        if self.revision() != revision {
            self.set_selections(after);
            let cursor = self.cursor.char_idx;
            let selection = self.selection;
            let extra = self.extra_selections.clone();
            if let Some(edit) = self.undo.last_mut() {
                edit.after = cursor;
                edit.after_selection = selection;
                edit.after_extra = extra;
            }
        }
        Ok(())
    }

    /// Select the current word and then its next occurrence, or all occurrences.
    pub fn select_occurrences(&mut self, all: bool) {
        let mut selections = self.selections();
        let primary = selections[0];
        let (start, end) = if primary.anchor != primary.head {
            (
                primary.anchor.min(primary.head),
                primary.anchor.max(primary.head),
            )
        } else {
            let mut start = primary.head;
            let mut end = primary.head;
            while start > 0 && is_word(self.rope.char(start - 1)) {
                start -= 1;
            }
            while end < self.len_chars() && is_word(self.rope.char(end)) {
                end += 1;
            }
            if start == end {
                return;
            }
            selections[0] = Selection {
                anchor: start,
                head: end,
            };
            if !all {
                self.set_selections(selections);
                return;
            }
            (start, end)
        };
        let needle = self.rope.slice(start..end).to_string();
        if needle.is_empty() {
            return;
        }
        let text = self.rope.to_string();
        let existing: Vec<_> = selections
            .iter()
            .map(|s| s.anchor.min(s.head)..s.anchor.max(s.head))
            .collect();
        let mut candidates: Vec<_> = text
            .match_indices(&needle)
            .map(|(byte, value)| {
                let a = self.rope.byte_to_char(byte);
                let b = self.rope.byte_to_char(byte + value.len());
                a..b
            })
            .filter(|candidate| {
                !existing
                    .iter()
                    .any(|r| candidate.start < r.end && r.start < candidate.end)
            })
            .collect();
        if !all {
            let end = existing.iter().map(|r| r.end).max().unwrap_or(0);
            if let Some(pos) = candidates.iter().position(|r| r.start >= end) {
                candidates.rotate_left(pos);
            }
            candidates.truncate(1);
        }
        selections.extend(candidates.into_iter().map(|range| Selection {
            anchor: range.start,
            head: range.end,
        }));
        self.set_selections(selections);
    }
    pub fn selections(&self) -> Vec<Selection> {
        let mut result = vec![self.selection.unwrap_or(Selection {
            anchor: self.cursor.char_idx,
            head: self.cursor.char_idx,
        })];
        result.extend_from_slice(&self.extra_selections);
        result
    }

    /// Set all selections, clamping invalid offsets and merging overlaps.
    pub fn set_selections(&mut self, mut selections: Vec<Selection>) {
        if selections.is_empty() {
            selections.push(Selection {
                anchor: self.cursor.char_idx,
                head: self.cursor.char_idx,
            });
        }
        let len = self.len_chars();
        for s in &mut selections {
            s.anchor = s.anchor.min(len);
            s.head = s.head.min(len);
        }
        selections.sort_by_key(|s| (s.anchor.min(s.head), s.anchor.max(s.head)));
        let mut merged: Vec<Selection> = Vec::new();
        for selection in selections {
            let start = selection.anchor.min(selection.head);
            let end = selection.anchor.max(selection.head);
            if let Some(last) = merged.last_mut() {
                let a = last.anchor.min(last.head);
                let b = last.anchor.max(last.head);
                if start < b || (start == a && end == b) || (start == b && start == end) {
                    *last = Selection {
                        anchor: a,
                        head: b.max(end),
                    };
                    continue;
                }
            }
            merged.push(selection);
        }
        let primary = merged.remove(0);
        self.cursor.char_idx = primary.head;
        self.selection = (primary.anchor != primary.head).then_some(primary);
        self.extra_selections = merged;
    }

    pub fn add_cursor(&mut self, offset: usize) {
        let mut selections = self.selections();
        selections.push(Selection {
            anchor: offset,
            head: offset,
        });
        self.set_selections(selections);
    }

    pub fn selected_texts(&self) -> Vec<String> {
        self.selections()
            .into_iter()
            .filter(|s| s.anchor != s.head)
            .map(|s| {
                self.rope
                    .slice(s.anchor.min(s.head)..s.anchor.max(s.head))
                    .to_string()
            })
            .collect()
    }

    /// Paste one clipboard line per cursor when the line count matches; otherwise paste all text at each cursor.
    pub fn paste(&mut self, text: &str) {
        let selections = self.selections();
        if selections.len() == 1 {
            self.insert(text);
            return;
        }
        let lines: Vec<_> = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect();
        let distribute = lines.len() == selections.len();
        let edits: Vec<_> = selections
            .iter()
            .enumerate()
            .map(|(index, selection)| TextReplacement {
                range: selection.anchor.min(selection.head)..selection.anchor.max(selection.head),
                text: if distribute { lines[index] } else { text }.to_owned(),
            })
            .collect();
        self.apply_cursor_replacements(&edits);
    }

    /// Apply replacements as one history entry and keep the resulting cursors.
    pub(super) fn apply_cursor_replacements(&mut self, edits: &[TextReplacement]) {
        let revision = self.revision();
        if let Ok(cursors) = self.apply_transaction(edits) {
            if self.revision() == revision {
                return;
            }
            self.set_selections(
                cursors
                    .into_iter()
                    .map(|head| Selection { anchor: head, head })
                    .collect(),
            );
            let after = self.cursor.char_idx;
            let selection = self.selection;
            let extra = self.extra_selections.clone();
            if let Some(edit) = self.undo.last_mut() {
                edit.after = after;
                edit.after_selection = selection;
                edit.after_extra = extra;
            }
        }
    }

    pub(super) fn replace_selections(&mut self, text: &str, backward: bool, forward: bool) {
        if self.read_only {
            return;
        }
        let mut ranges: Vec<_> = self
            .selections()
            .into_iter()
            .map(|s| {
                let mut start = s.anchor.min(s.head);
                let mut end = s.anchor.max(s.head);
                if start == end && backward {
                    start = start.saturating_sub(1);
                    if start > 0
                        && self.rope.char(start) == '\n'
                        && self.rope.char(start - 1) == '\r'
                    {
                        start -= 1;
                    }
                }
                if start == end && forward {
                    end = (end + 1).min(self.len_chars());
                    if start < self.len_chars()
                        && self.rope.char(start) == '\r'
                        && end < self.len_chars()
                        && self.rope.char(end) == '\n'
                    {
                        end += 1;
                    }
                }
                start..end
            })
            .collect();
        ranges.sort_by_key(|r| (r.start, r.end));
        let mut merged: Vec<std::ops::Range<usize>> = Vec::new();
        for r in ranges {
            if let Some(last) = merged.last_mut() {
                if r.start < last.end || r.start == last.start {
                    last.end = last.end.max(r.end);
                    continue;
                }
            }
            merged.push(r);
        }
        let edits: Vec<_> = merged
            .into_iter()
            .map(|range| TextReplacement {
                range,
                text: text.into(),
            })
            .collect();
        self.apply_cursor_replacements(&edits);
    }

    pub fn move_selections(&mut self, motion: CursorMotion, extend: bool) {
        let mut selections = self.selections();
        for s in &mut selections {
            let head = s.head;
            let line = self.rope.char_to_line(head);
            let col = head - self.rope.line_to_char(line);
            let next = match motion {
                CursorMotion::Left => {
                    if !extend && s.anchor != s.head {
                        s.anchor.min(s.head)
                    } else {
                        let mut next = head.saturating_sub(1);
                        if next > 0
                            && self.rope.char(next) == '\n'
                            && self.rope.char(next - 1) == '\r'
                        {
                            next -= 1;
                        }
                        next
                    }
                }
                CursorMotion::Right => {
                    if !extend && s.anchor != s.head {
                        s.anchor.max(s.head)
                    } else {
                        let mut next = (head + 1).min(self.len_chars());
                        if next < self.len_chars()
                            && next > 0
                            && self.rope.char(next) == '\n'
                            && self.rope.char(next - 1) == '\r'
                        {
                            next += 1;
                        }
                        next
                    }
                }
                CursorMotion::Home => self.rope.line_to_char(line),
                CursorMotion::End => self.rope.line_to_char(line) + self.line_content_len(line),
                CursorMotion::Up | CursorMotion::Down => {
                    let target = if matches!(motion, CursorMotion::Up) {
                        line.saturating_sub(1)
                    } else {
                        (line + 1).min(self.len_lines() - 1)
                    };
                    self.rope.line_to_char(target) + col.min(self.line_content_len(target))
                }
            };
            if !extend {
                s.anchor = next;
            }
            s.head = next;
        }
        self.set_selections(selections);
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
