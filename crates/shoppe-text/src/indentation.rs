use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indentation {
    pub insert_spaces: bool,
    pub tab_size: usize,
}

impl Buffer {
    /// Detect a prevailing indentation style from non-empty indented lines.
    pub fn detect_indentation(&self) -> Option<Indentation> {
        let mut tabs = 0usize;
        let mut spaces = Vec::new();
        for line in self.rope.lines().take(1000) {
            if line.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
                continue;
            }
            let mut prefix = 0usize;
            let mut first = None;
            for ch in line.chars() {
                if ch != ' ' && ch != '\t' {
                    break;
                }
                first.get_or_insert(ch);
                if ch != first.unwrap() {
                    first = None;
                    break;
                }
                prefix += 1;
            }
            match first {
                Some('\t') => tabs += 1,
                Some(' ') if prefix >= 2 => spaces.push(prefix),
                _ => {}
            }
        }
        if tabs == 0 && spaces.is_empty() {
            return None;
        }
        if tabs > spaces.len() {
            return Some(Indentation {
                insert_spaces: false,
                tab_size: 4,
            });
        }
        let gcd = |mut a: usize, mut b: usize| {
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        };
        let size = spaces.into_iter().reduce(gcd).unwrap_or(4).clamp(2, 8);
        Some(Indentation {
            insert_spaces: true,
            tab_size: size,
        })
    }

    /// Convert leading indentation only, grouping all changed lines into one undo step.
    pub fn convert_indentation(&mut self, target: Indentation) -> Result<usize, TransactionError> {
        if self.is_read_only() {
            return Err(TransactionError::ReadOnly);
        }
        let width = target.tab_size.clamp(1, 16);
        let mut edits = Vec::new();
        for (index, line) in self.rope.lines().enumerate() {
            let mut columns = 0usize;
            let mut count = 0usize;
            for ch in line.chars() {
                match ch {
                    ' ' => columns += 1,
                    '\t' => columns += width - columns % width,
                    _ => break,
                }
                count += 1;
            }
            if count == 0 {
                continue;
            }
            let replacement = if target.insert_spaces {
                " ".repeat(columns)
            } else {
                format!(
                    "{}{}",
                    "\t".repeat(columns / width),
                    " ".repeat(columns % width)
                )
            };
            if line.slice(..count).to_string() != replacement {
                let start = self.rope.line_to_char(index);
                edits.push(TextReplacement {
                    range: start..start + count,
                    text: replacement,
                });
            }
        }
        let count = edits.len();
        self.apply_preserving_selections(&edits)?;
        Ok(count)
    }

    /// Remove spaces and tabs at line ends without altering line terminators.
    pub fn trim_trailing_whitespace(&mut self) -> Result<usize, TransactionError> {
        if self.is_read_only() {
            return Err(TransactionError::ReadOnly);
        }
        let mut edits = Vec::new();
        for (index, line) in self.rope.lines().enumerate() {
            let mut end = line.len_chars();
            if end > 0 && line.char(end - 1) == '\n' {
                end -= 1;
            }
            if end > 0 && line.char(end - 1) == '\r' {
                end -= 1;
            }
            let mut start = end;
            while start > 0 && matches!(line.char(start - 1), ' ' | '\t') {
                start -= 1;
            }
            if start < end {
                let base = self.rope.line_to_char(index);
                edits.push(TextReplacement {
                    range: base + start..base + end,
                    text: String::new(),
                });
            }
        }
        let count = edits.len();
        self.apply_preserving_selections(&edits)?;
        Ok(count)
    }
}
