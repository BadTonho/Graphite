use super::*;

impl Buffer {
    /// Cursor as (line, column) in characters.
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let line = self.rope.char_to_line(self.cursor.char_idx);
        (line, self.cursor.char_idx - self.rope.line_to_char(line))
    }

    pub fn insert(&mut self, text: &str) {
        let (start, end) = self
            .selection
            .map(|s| (s.anchor.min(s.head), s.anchor.max(s.head)))
            .unwrap_or((self.cursor.char_idx, self.cursor.char_idx));
        self.replace(start, end, text);
    }

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self) {
        if let Some(s) = self.selection {
            self.replace(s.anchor.min(s.head), s.anchor.max(s.head), "");
            return;
        }
        let i = self.cursor.char_idx;
        if i > 0 {
            self.replace(i - 1, i, "");
        }
    }

    /// Deletes the character after the cursor.
    pub fn delete(&mut self) {
        if let Some(s) = self.selection {
            self.replace(s.anchor.min(s.head), s.anchor.max(s.head), "");
            return;
        }
        let i = self.cursor.char_idx;
        if i < self.rope.len_chars() {
            self.replace(i, i + 1, "");
        }
    }

    /// Places the cursor at (line, column), clamping to valid positions.
    pub fn set_cursor_line_col(&mut self, line: usize, col: usize) {
        self.selection = None;
        let line = line.min(self.rope.len_lines() - 1);
        let line_len = self.rope.line(line).len_chars();
        let max_col = if line + 1 == self.rope.len_lines() {
            line_len
        } else {
            line_len.saturating_sub(1)
        };
        self.cursor.char_idx = self.rope.line_to_char(line) + col.min(max_col);
    }

    pub fn move_home(&mut self) {
        self.selection = None;
        let (line, _) = self.cursor_line_col();
        self.cursor.char_idx = self.rope.line_to_char(line);
    }

    pub fn move_end(&mut self) {
        self.selection = None;
        let (line, _) = self.cursor_line_col();
        self.set_cursor_line_col(line, usize::MAX);
    }

    pub fn move_left(&mut self) {
        self.selection = None;
        self.cursor.char_idx = self.cursor.char_idx.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.selection = None;
        self.cursor.char_idx = (self.cursor.char_idx + 1).min(self.rope.len_chars());
    }

    pub fn move_vertical(&mut self, delta: isize) {
        self.selection = None;
        let (line, col) = self.cursor_line_col();
        let target = (line as isize + delta).clamp(0, self.rope.len_lines() as isize - 1) as usize;
        let line_len = self.rope.line(target).len_chars();
        let max_col = if target + 1 == self.rope.len_lines() {
            line_len
        } else {
            line_len.saturating_sub(1)
        };
        self.cursor.char_idx = self.rope.line_to_char(target) + col.min(max_col);
    }
}
