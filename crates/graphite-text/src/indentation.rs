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
            if line.slice(..count) != replacement.as_str() {
                let start = self.rope.line_to_char(index);
                edits.push(TextReplacement {
                    range: start..start + count,
                    text: replacement,
                });
            }
        }
        let count = edits.len();
        self.apply_transaction(&edits)?;
        Ok(count)
    }

    /// Remove spaces and tabs at line ends without altering line terminators.
    pub fn trim_trailing_whitespace(&mut self) -> Result<usize, TransactionError> {
        if self.is_read_only() {
            return Err(TransactionError::ReadOnly);
        }
        let edits = self.trailing_whitespace_edits();
        let count = edits.len();
        self.apply_transaction(&edits)?;
        Ok(count)
    }

    pub(super) fn trailing_whitespace_edits(&self) -> Vec<TextReplacement> {
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
        edits
    }

    /// Reindents either `lines` (`start..=end`, 0-based) or the entire buffer using `style`
    /// and bracket/colon indentation rules, grouping all changes into one undo step.
    pub fn reindent_lines(
        &mut self,
        style: Indentation,
        lines: Option<std::ops::RangeInclusive<usize>>,
        colon_indents: bool,
    ) -> Result<usize, TransactionError> {
        if self.is_read_only() {
            return Err(TransactionError::ReadOnly);
        }
        let total_lines = self.rope.len_lines();
        if total_lines == 0 {
            return Ok(0);
        }
        let target_range = match lines {
            Some(range) => {
                let start = (*range.start()).min(total_lines - 1);
                let end = (*range.end()).min(total_lines - 1);
                start..=end
            }
            None => 0..=(total_lines - 1),
        };
        let unit = if style.insert_spaces {
            " ".repeat(style.tab_size.clamp(1, 16))
        } else {
            "\t".to_string()
        };

        let mut depth = 0usize;
        let mut edits = Vec::new();
        for index in 0..=*target_range.end() {
            let line_slice = self.rope.line(index);
            let raw = line_slice.to_string();
            let content = raw.trim_end_matches(['\r', '\n']);
            let ws_chars = content
                .chars()
                .take_while(|c| matches!(c, ' ' | '\t'))
                .count();
            let trimmed: String = content.chars().skip(ws_chars).collect();

            if trimmed.is_empty() {
                if target_range.contains(&index) && ws_chars > 0 {
                    let base = self.rope.line_to_char(index);
                    edits.push(TextReplacement {
                        range: base..base + ws_chars,
                        text: String::new(),
                    });
                }
                continue;
            }

            let (leading_closers, net_delta, ends_with_colon) =
                scan_indent_tokens(&trimmed, colon_indents);
            let line_depth = depth.saturating_sub(leading_closers);

            if target_range.contains(&index) {
                let desired = unit.repeat(line_depth);
                let current: String = content.chars().take(ws_chars).collect();
                if current != desired {
                    let base = self.rope.line_to_char(index);
                    edits.push(TextReplacement {
                        range: base..base + ws_chars,
                        text: desired,
                    });
                }
            }

            let next_depth = (depth as isize + net_delta).max(0) as usize;
            depth = if ends_with_colon && net_delta == 0 {
                next_depth + 1
            } else {
                next_depth
            };
        }

        let count = edits.len();
        self.apply_transaction(&edits)?;
        Ok(count)
    }
}

/// Counts leading closing brackets, net bracket delta, and trailing `:` outside strings/comments.
fn scan_indent_tokens(trimmed: &str, colon_indents: bool) -> (usize, isize, bool) {
    let mut leading_closers = 0usize;
    let mut seen_non_closer = false;
    let mut net_delta = 0isize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let mut last_code_char = None;
    let mut chars = trimmed.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            break;
        }
        if ch == '#' && colon_indents {
            break;
        }
        if ch == '"' || ch == '\'' {
            seen_non_closer = true;
            in_string = Some(ch);
            last_code_char = Some(ch);
            continue;
        }
        if !ch.is_whitespace() {
            last_code_char = Some(ch);
        }
        match ch {
            '{' | '(' | '[' => {
                seen_non_closer = true;
                net_delta += 1;
            }
            '}' | ')' | ']' => {
                if !seen_non_closer {
                    leading_closers += 1;
                }
                net_delta -= 1;
            }
            _ => {
                if !ch.is_whitespace() {
                    seen_non_closer = true;
                }
            }
        }
    }

    (
        leading_closers,
        net_delta,
        colon_indents && last_code_char == Some(':'),
    )
}
