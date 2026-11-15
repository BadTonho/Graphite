use super::*;

impl Editor<'_> {
    /// Insert line breaks using each cursor's local line-ending convention.
    pub fn insert_newline(&mut self) {
        if self.read_only {
            return;
        }
        let edits: Vec<_> = self
            .selections()
            .into_iter()
            .map(|selection| {
                let start = selection.anchor.min(selection.head);
                let end = selection.anchor.max(selection.head);
                let line = self.rope.char_to_line(start);
                let ending = (0..=line)
                    .rev()
                    .find_map(|index| {
                        let text = self.rope.line(index);
                        let len = text.len_chars();
                        if len > 0 && text.char(len - 1) == '\n' {
                            Some(if len > 1 && text.char(len - 2) == '\r' {
                                "\r\n"
                            } else {
                                "\n"
                            })
                        } else {
                            None
                        }
                    })
                    .unwrap_or("\n");
                TextReplacement {
                    range: start..end,
                    text: ending.into(),
                }
            })
            .collect();
        self.apply_cursor_replacements(&edits);
    }

    /// Cursor as (line, column) in characters.
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let line = self.rope.char_to_line(self.state.cursor.char_idx);
        (
            line,
            self.state.cursor.char_idx - self.rope.line_to_char(line),
        )
    }

    pub fn insert(&mut self, text: &str) {
        if !self.state.extra_selections.is_empty() {
            self.replace_selections(text, false, false);
            return;
        }
        let (start, end) = self
            .state
            .selection
            .map(|s| (s.anchor.min(s.head), s.anchor.max(s.head)))
            .unwrap_or((self.state.cursor.char_idx, self.state.cursor.char_idx));
        self.replace(start, end, text);
    }

    /// Inserts typed `text` in overtype (overwrite) mode: replaces any active selection,
    /// or overwrites up to `text.chars().count()` characters on the current line before
    /// the line terminator for each caret.
    pub fn insert_overtype(&mut self, text: &str) {
        if self.read_only {
            return;
        }
        let count = text.chars().count();
        if count == 0 {
            return;
        }
        let selections = self.selections();
        let len = self.len_chars();
        let mut ranges: Vec<std::ops::Range<usize>> = selections
            .into_iter()
            .map(|s| {
                let start = s.anchor.min(s.head);
                let mut end = s.anchor.max(s.head);
                if start == end {
                    let mut remaining = count;
                    while remaining > 0 && end < len {
                        let ch = self.rope.char(end);
                        if ch == '\n' || ch == '\r' {
                            break;
                        }
                        end += 1;
                        remaining -= 1;
                    }
                }
                start..end
            })
            .collect();
        ranges.sort_by_key(|r| (r.start, r.end));
        let mut merged: Vec<std::ops::Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut() {
                if range.start < last.end || (range.start == last.start && range.end == last.end) {
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

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self) {
        if !self.state.extra_selections.is_empty() {
            self.replace_selections("", true, false);
            return;
        }
        if let Some(s) = self.state.selection {
            self.replace(s.anchor.min(s.head), s.anchor.max(s.head), "");
            return;
        }
        let i = self.state.cursor.char_idx;
        if i > 0 {
            let mut start = i - 1;
            if start > 0 && self.rope.char(start) == '\n' && self.rope.char(start - 1) == '\r' {
                start -= 1;
            }
            self.replace(start, i, "");
        }
    }

    /// Deletes the character after the cursor.
    pub fn delete(&mut self) {
        if !self.state.extra_selections.is_empty() {
            self.replace_selections("", false, true);
            return;
        }
        if let Some(s) = self.state.selection {
            self.replace(s.anchor.min(s.head), s.anchor.max(s.head), "");
            return;
        }
        let i = self.state.cursor.char_idx;
        if i < self.rope.len_chars() {
            let mut end = i + 1;
            if self.rope.char(i) == '\r'
                && end < self.rope.len_chars()
                && self.rope.char(end) == '\n'
            {
                end += 1;
            }
            self.replace(i, end, "");
        }
    }

    /// Places the cursor at (line, column), clamping to valid positions.
    pub fn set_cursor_line_col(&mut self, line: usize, col: usize) {
        self.state.selection = None;
        self.state.extra_selections.clear();
        let line = line.min(self.rope.len_lines() - 1);
        let max_col = self.line_content_len(line);
        self.state.cursor.char_idx = self.rope.line_to_char(line) + col.min(max_col);
    }

    pub fn move_home(&mut self) {
        self.state.selection = None;
        let (line, _) = self.cursor_line_col();
        self.state.cursor.char_idx = self.rope.line_to_char(line);
    }

    pub fn move_end(&mut self) {
        self.state.selection = None;
        let (line, _) = self.cursor_line_col();
        self.set_cursor_line_col(line, usize::MAX);
    }

    pub fn move_left(&mut self) {
        self.state.selection = None;
        self.state.cursor.char_idx = self.state.cursor.char_idx.saturating_sub(1);
        if self.state.cursor.char_idx > 0
            && self.rope.char(self.state.cursor.char_idx) == '\n'
            && self.rope.char(self.state.cursor.char_idx - 1) == '\r'
        {
            self.state.cursor.char_idx -= 1;
        }
    }

    pub fn move_right(&mut self) {
        self.state.selection = None;
        self.state.cursor.char_idx = (self.state.cursor.char_idx + 1).min(self.rope.len_chars());
        if self.state.cursor.char_idx < self.rope.len_chars()
            && self.rope.char(self.state.cursor.char_idx) == '\n'
            && self.state.cursor.char_idx > 0
            && self.rope.char(self.state.cursor.char_idx - 1) == '\r'
        {
            self.state.cursor.char_idx += 1;
        }
    }

    pub fn move_vertical(&mut self, delta: isize) {
        self.state.selection = None;
        let (line, col) = self.cursor_line_col();
        let target = (line as isize + delta).clamp(0, self.rope.len_lines() as isize - 1) as usize;
        let max_col = self.line_content_len(target);
        self.state.cursor.char_idx = self.rope.line_to_char(target) + col.min(max_col);
    }
}
