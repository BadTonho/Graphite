use super::*;

impl Buffer {
    fn transform_selected_lines(&mut self, deduplicate: bool) {
        if self.read_only {
            return;
        }
        let (start_char, end_char) = self
            .selection
            .map(|s| (s.anchor.min(s.head), s.anchor.max(s.head)))
            .unwrap_or((self.cursor.char_idx, self.cursor.char_idx));
        let first = self.rope.char_to_line(start_char);
        let last_offset = if end_char > start_char {
            end_char - 1
        } else {
            end_char
        };
        let last = self.rope.char_to_line(last_offset);
        let start = self.rope.line_to_char(first);
        let end = if last + 1 < self.len_lines() {
            self.rope.line_to_char(last + 1)
        } else {
            self.len_chars()
        };
        let lines: Vec<String> = (first..=last)
            .map(|i| self.rope.line(i).to_string())
            .collect();
        let mut values: Vec<String> = lines
            .iter()
            .map(|s| {
                s.strip_suffix("\r\n")
                    .or_else(|| s.strip_suffix('\n'))
                    .unwrap_or(s)
                    .to_owned()
            })
            .collect();
        if deduplicate {
            let mut seen = std::collections::HashSet::new();
            values.retain(|line| seen.insert(line.clone()));
        } else {
            values.sort();
        }
        let ending = if lines.iter().any(|s| s.ends_with("\r\n")) {
            "\r\n"
        } else {
            "\n"
        };
        let trailing = lines.last().is_some_and(|line| line.ends_with('\n'));
        let mut replacement = values.join(ending);
        if trailing && !values.is_empty() {
            replacement.push_str(ending);
        }
        let replacement_len = replacement.chars().count();
        self.replace(start, end, &replacement);
        self.set_selection(start, start + replacement_len);
    }

    /// Sort selected lines alphabetically, or the current line if there is no selection.
    pub fn sort_lines(&mut self) {
        self.transform_selected_lines(false);
    }

    /// Remove duplicate selected lines while retaining their first occurrence.
    pub fn deduplicate_lines(&mut self) {
        self.transform_selected_lines(true);
    }

    /// Move the current line by one position, preserving the original line endings.
    pub fn move_line(&mut self, direction: isize) {
        if self.read_only || direction == 0 {
            return;
        }
        let (line, col) = self.cursor_line_col();
        let other = if direction < 0 {
            line.checked_sub(1)
        } else {
            line.checked_add(1).filter(|n| *n < self.len_lines())
        };
        let Some(other) = other else {
            return;
        };
        // The trailing empty Rope line represents a final line ending, not a movable line.
        if self.rope.line(other).len_chars() == 0 {
            return;
        }
        let first = line.min(other);
        let second = line.max(other);
        let a = self.rope.line(first).to_string();
        let b = self.rope.line(second).to_string();
        let ending = if a.ends_with("\r\n") { "\r\n" } else { "\n" };
        let content_a = a
            .strip_suffix("\r\n")
            .or_else(|| a.strip_suffix('\n'))
            .unwrap_or(&a);
        let content_b = b
            .strip_suffix("\r\n")
            .or_else(|| b.strip_suffix('\n'))
            .unwrap_or(&b);
        let trailing = if b.ends_with("\r\n") {
            "\r\n"
        } else if b.ends_with('\n') {
            "\n"
        } else {
            ""
        };
        let replacement = format!("{content_b}{ending}{content_a}{trailing}");
        let start = self.rope.line_to_char(first);
        let end = start + a.chars().count() + b.chars().count();
        self.replace(start, end, &replacement);
        self.set_cursor_line_col(other, col);
    }

    /// Duplicate the current line above or below it.
    pub fn duplicate_line(&mut self, below: bool) {
        if self.read_only {
            return;
        }
        let (line, col) = self.cursor_line_col();
        let text = self.rope.line(line).to_string();
        let ending = if text.ends_with("\r\n")
            || (!text.ends_with('\n')
                && line > 0
                && self.rope.line(line - 1).to_string().ends_with("\r\n"))
        {
            "\r\n"
        } else {
            "\n"
        };
        let (at, inserted) = if text.ends_with('\n') {
            (self.rope.line_to_char(line), text)
        } else {
            (self.rope.line_to_char(line), format!("{text}{ending}"))
        };
        self.replace(at, at, &inserted);
        self.set_cursor_line_col(line + usize::from(below), col);
    }

    /// Delete the current logical line, including a line ending when possible.
    pub fn delete_line(&mut self) {
        if self.read_only {
            return;
        }
        let (line, _) = self.cursor_line_col();
        let mut start = self.rope.line_to_char(line);
        let end = start + self.rope.line(line).len_chars();
        if line > 0
            && end == self.rope.len_chars()
            && !self.rope.line(line).to_string().ends_with('\n')
        {
            start -= 1;
            if start > 0 && self.rope.char(start) == '\n' && self.rope.char(start - 1) == '\r' {
                start -= 1;
            }
        }
        self.replace(start, end, "");
    }
}
