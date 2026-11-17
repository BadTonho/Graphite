//! Line and text commands: reverse and sort lines, duplicate a selection, indent and outdent,
//! transpose characters and change case.

use super::lines::split_line_ending;
use super::*;
use crate::case::CaseTransform;
use std::ops::Range;

impl Editor<'_> {
    /// The line blocks a whole-text command works on. A lone cursor or single-line selection
    /// stands for the whole document; otherwise it is the selected blocks. A trailing empty
    /// line (after the last line break) is not part of any block.
    fn blocks_or_whole_document(&self) -> Vec<Range<usize>> {
        let selections = self.selections();
        let mut blocks = match selections.as_slice() {
            [only] if self.rope.char_to_line(only.anchor) == self.rope.char_to_line(only.head) => {
                let whole_document = 0..self.len_lines();
                vec![whole_document]
            }
            _ => self.selected_line_blocks(),
        };
        let total = self.len_lines();
        for block in &mut blocks {
            if block.end == total && block.len() > 1 && self.rope.line(total - 1).len_chars() == 0 {
                block.end -= 1;
            }
        }
        blocks
    }

    /// The text of the lines in `block` and the separators between them.
    fn block_contents(&self, block: &Range<usize>) -> (Vec<String>, Vec<String>) {
        let mut contents = Vec::new();
        let mut separators = Vec::new();
        for line in block.clone() {
            let value = self.rope.line(line).to_string();
            let (content, ending) = split_line_ending(&value);
            contents.push(content.to_owned());
            if line + 1 < block.end {
                separators.push(ending.to_owned());
            }
        }
        (contents, separators)
    }

    /// The characters of `block` from its first line up to the end of its last line's text.
    fn block_text_range(&self, block: &Range<usize>) -> Range<usize> {
        let last = block.end - 1;
        self.rope.line_to_char(block.start)
            ..self.rope.line_to_char(last) + self.line_content_len(last)
    }

    /// Writes `contents` back into `block`, keeping each line separator where it was.
    fn rewrite_block(
        &self,
        block: &Range<usize>,
        contents: &[String],
        separators: &[String],
    ) -> TextReplacement {
        let mut text = String::new();
        for (index, content) in contents.iter().enumerate() {
            text.push_str(content);
            if let Some(separator) = separators.get(index) {
                text.push_str(separator);
            }
        }
        TextReplacement {
            range: self.block_text_range(block),
            text,
        }
    }

    /// Sorts the lines of each selected block (the whole document for a lone cursor).
    pub fn sort_lines_ordered(&mut self, descending: bool) {
        if self.read_only {
            return;
        }
        let mut edits = Vec::new();
        for block in self.blocks_or_whole_document() {
            if block.len() < 2 {
                continue;
            }
            let (mut contents, separators) = self.block_contents(&block);
            let before = contents.clone();
            contents.sort();
            if descending {
                contents.reverse();
            }
            if contents != before {
                edits.push(self.rewrite_block(&block, &contents, &separators));
            }
        }
        let _ = self.apply_preserving_selections(&edits);
    }

    /// Reverses the order of the lines of each selected block (the whole document for a lone
    /// cursor). Each cursor keeps its column and follows its line to the new position.
    pub fn reverse_lines(&mut self) {
        if self.read_only {
            return;
        }
        let blocks: Vec<_> = self
            .blocks_or_whole_document()
            .into_iter()
            .filter(|block| block.len() > 1)
            .collect();
        if blocks.is_empty() {
            return;
        }
        let mut edits = Vec::new();
        for block in &blocks {
            let (mut contents, separators) = self.block_contents(block);
            contents.reverse();
            edits.push(self.rewrite_block(block, &contents, &separators));
        }
        let positions: Vec<_> = self
            .selections()
            .into_iter()
            .map(|s| (self.line_column(s.anchor), self.line_column(s.head)))
            .collect();
        let revision = self.revision();
        if self.apply_transaction(&edits).is_err() || self.revision() == revision {
            return;
        }
        let place = |(line, column): (usize, usize)| {
            let line = blocks
                .iter()
                .find(|block| block.contains(&line))
                .map_or(line, |block| block.start + block.end - 1 - line);
            self.rope.line_to_char(line) + column.min(self.line_content_len(line))
        };
        let selections = positions
            .into_iter()
            .map(|(anchor, head)| Selection {
                anchor: place(anchor),
                head: place(head),
            })
            .collect();
        self.set_selections(selections);
        self.finish_selections();
    }

    fn line_column(&self, offset: usize) -> (usize, usize) {
        let line = self.rope.char_to_line(offset);
        (line, offset - self.rope.line_to_char(line))
    }

    /// Shifts the indentation of each selected line to the next (or previous) indent stop.
    pub fn indent_lines(&mut self, indentation: Indentation) {
        self.shift_lines(indentation, false);
    }

    pub fn outdent_lines(&mut self, indentation: Indentation) {
        self.shift_lines(indentation, true);
    }

    fn shift_lines(&mut self, indentation: Indentation, unshift: bool) {
        if self.read_only {
            return;
        }
        let size = indentation.tab_size.max(1);
        let mut edits = Vec::new();
        let mut blank_lines = Vec::new();
        for block in self.selected_line_blocks() {
            let single = block.len() == 1;
            for line in block {
                let start = self.rope.line_to_char(line);
                let length = self.line_content_len(line);
                let content: Vec<char> = self.rope.slice(start..start + length).chars().collect();
                let lead = content
                    .iter()
                    .take_while(|c| matches!(c, ' ' | '\t'))
                    .count();
                if unshift && lead == 0 {
                    continue;
                }
                if !unshift && length == 0 && !single {
                    continue;
                }
                let width = content[..lead].iter().fold(0usize, |width, c| {
                    if *c == '\t' {
                        width + size - width % size
                    } else {
                        width + 1
                    }
                });
                let target = if unshift {
                    width.saturating_sub(1) / size * size
                } else {
                    (width / size + 1) * size
                };
                let replacement = if indentation.insert_spaces {
                    " ".repeat(target)
                } else {
                    "\t".repeat(target / size)
                };
                if !unshift && lead == length {
                    blank_lines.push(line);
                }
                if replacement.chars().eq(content[..lead].iter().copied()) {
                    continue;
                }
                edits.push(TextReplacement {
                    range: start..start + lead,
                    text: replacement,
                });
            }
        }
        if edits.is_empty() || self.apply_preserving_selections(&edits).is_err() {
            return;
        }
        // A cursor on a blank line ends up after the new indentation.
        let mut selections = self.selections();
        let mut moved = false;
        for selection in &mut selections {
            let (line, _) = self.line_column(selection.head);
            if selection.anchor == selection.head && blank_lines.contains(&line) {
                selection.anchor = self.rope.line_to_char(line) + self.line_content_len(line);
                selection.head = selection.anchor;
                moved = true;
            }
        }
        if moved {
            self.set_selections(selections);
            self.finish_selections();
        }
    }

    /// Swaps the characters around each cursor. At the end of a line the last character swaps
    /// with the line break, which moves it to the start of the next line.
    pub fn transpose_characters(&mut self) {
        if self.read_only {
            return;
        }
        let total = self.len_lines();
        let mut edits = Vec::new();
        for selection in self.selections() {
            if selection.anchor != selection.head {
                continue;
            }
            let offset = selection.head;
            let (line, column) = self.line_column(offset);
            let length = self.line_content_len(line);
            if column >= length {
                if line + 1 >= total || column == 0 {
                    continue;
                }
                let end = self.rope.line_to_char(line + 1);
                let text = format!(
                    "{}{}",
                    self.rope.slice(offset..end),
                    self.rope.char(offset - 1)
                );
                edits.push(TextReplacement {
                    range: offset - 1..end,
                    text,
                });
            } else if column > 0 {
                let text: String = [self.rope.char(offset), self.rope.char(offset - 1)]
                    .iter()
                    .collect();
                edits.push(TextReplacement {
                    range: offset - 1..offset + 1,
                    text,
                });
            }
        }
        let _ = self.apply_cursor_edits(&edits);
    }

    /// Applies edits and puts each cursor that produced one at the end of its edit.
    pub(super) fn apply_cursor_edits(
        &mut self,
        edits: &[TextReplacement],
    ) -> Result<(), TransactionError> {
        if edits.is_empty() {
            return Ok(());
        }
        let revision = self.revision();
        let ends = self.apply_transaction(edits)?;
        if self.revision() == revision {
            return Ok(());
        }
        let mut selections: Vec<Selection> = self
            .selections()
            .into_iter()
            .filter(|s| s.anchor != s.head)
            .collect();
        selections.extend(ends.into_iter().map(|end| Selection {
            anchor: end,
            head: end,
        }));
        self.set_selections(selections);
        self.finish_selections();
        Ok(())
    }

    /// Changes the case of each selection, or of the word at each cursor.
    pub fn transform_case(&mut self, transform: CaseTransform) {
        if self.read_only {
            return;
        }
        let mut edits: Vec<TextReplacement> = Vec::new();
        for selection in self.selections() {
            let start = selection.anchor.min(selection.head);
            let end = selection.anchor.max(selection.head);
            let range = if start == end {
                match self.word_range_at(start) {
                    Some(range) => range,
                    None => continue,
                }
            } else {
                start..end
            };
            let text = self.rope.slice(range.clone()).to_string();
            let changed = transform.apply(&text);
            if changed != text {
                edits.push(TextReplacement {
                    range,
                    text: changed,
                });
            }
        }
        edits.sort_by_key(|edit| edit.range.start);
        edits.dedup_by(|later, earlier| later.range.start < earlier.range.end);
        let _ = self.apply_preserving_selections(&edits);
    }

    /// The word the offset is in or touches, made of letters, digits and underscores.
    fn word_range_at(&self, offset: usize) -> Option<Range<usize>> {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = offset;
        while start > 0 && is_word(self.rope.char(start - 1)) {
            start -= 1;
        }
        let mut end = offset;
        while end < self.len_chars() && is_word(self.rope.char(end)) {
            end += 1;
        }
        (start < end).then_some(start..end)
    }
}
