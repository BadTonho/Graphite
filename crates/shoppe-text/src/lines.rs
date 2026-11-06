use super::*;
use std::ops::Range;

/// A text position associated with a logical line in the original document.
#[derive(Clone, Copy)]
struct LinePosition {
    line: usize,
    column: usize,
}

impl Buffer {
    /// Convert all selected ranges to disjoint, contiguous line blocks.
    fn selected_line_blocks(&self) -> Vec<Range<usize>> {
        let mut blocks: Vec<_> = self
            .selections()
            .into_iter()
            .map(|selection| {
                let start = selection.anchor.min(selection.head);
                let end = selection.anchor.max(selection.head);
                let first = self.rope.char_to_line(start);
                let last = self
                    .rope
                    .char_to_line(if end > start { end - 1 } else { end });
                first..last + 1
            })
            .collect();
        blocks.sort_by_key(|block| block.start);
        let mut merged: Vec<Range<usize>> = Vec::new();
        for block in blocks {
            if let Some(previous) = merged.last_mut() {
                if block.start <= previous.end {
                    previous.end = previous.end.max(block.end);
                    continue;
                }
            }
            merged.push(block);
        }
        merged
    }

    fn line_block_range(&self, block: &Range<usize>) -> Range<usize> {
        self.rope.line_to_char(block.start)..if block.end < self.len_lines() {
            self.rope.line_to_char(block.end)
        } else {
            self.len_chars()
        }
    }

    fn position_for_selection_endpoint(
        &self,
        offset: usize,
        upper: bool,
        nonempty: bool,
    ) -> LinePosition {
        let mut line = self.rope.char_to_line(offset);
        let mut column = offset - self.rope.line_to_char(line);
        // A selection ending at a line start includes the preceding terminator,
        // so keep that endpoint attached to the preceding selected line.
        if upper && nonempty && column == 0 && line > 0 {
            line -= 1;
            column = self.rope.line(line).len_chars();
        }
        LinePosition { line, column }
    }

    fn store_after_selections(&mut self, selections: Vec<Selection>) {
        self.set_selections(selections);
        let cursor = self.cursor.char_idx;
        let selection = self.selection;
        let extra = self.extra_selections.clone();
        if let Some(edit) = self.undo.last_mut() {
            edit.after = cursor;
            edit.after_selection = selection;
            edit.after_extra = extra;
        }
    }

    /// Move each selected line block by one line as one undoable transaction.
    pub fn move_line(&mut self, direction: isize) {
        if self.read_only || direction == 0 {
            return;
        }
        let blocks = self.selected_line_blocks();
        let mut edits = Vec::new();
        let mut moves = Vec::new();
        for block in &blocks {
            let segment = if direction < 0 {
                if block.start == 0 {
                    continue;
                }
                block.start - 1..block.end
            } else {
                if block.end >= self.len_lines() {
                    continue;
                }
                block.start..block.end + 1
            };
            let mut contents = Vec::new();
            let mut endings = Vec::new();
            for line in segment.clone() {
                let value = self.rope.line(line).to_string();
                let (content, ending) = split_line_ending(&value);
                contents.push(content.to_owned());
                endings.push(ending.to_owned());
            }
            if direction < 0 {
                contents.rotate_left(1);
            } else {
                contents.rotate_right(1);
            }
            let text: String = contents
                .into_iter()
                .zip(endings)
                .map(|(content, ending)| format!("{content}{ending}"))
                .collect();
            edits.push(TextReplacement {
                range: self.line_block_range(&segment),
                text,
            });
            moves.push((block.clone(), if direction < 0 { -1isize } else { 1isize }));
        }
        if edits.is_empty() {
            return;
        }
        let positions: Vec<_> = self
            .selections()
            .into_iter()
            .map(|selection| {
                let upper = selection.anchor.max(selection.head);
                let nonempty = selection.anchor != selection.head;
                (
                    self.position_for_selection_endpoint(
                        selection.anchor,
                        selection.anchor == upper,
                        nonempty,
                    ),
                    self.position_for_selection_endpoint(
                        selection.head,
                        selection.head == upper,
                        nonempty,
                    ),
                )
            })
            .collect();
        let revision = self.revision();
        if self.apply_transaction(&edits).is_err() || self.revision() == revision {
            return;
        }
        let map = |position: LinePosition| {
            let line = moves
                .iter()
                .find(|(block, _)| block.contains(&position.line))
                .map(|(_, delta)| position.line.saturating_add_signed(*delta))
                .unwrap_or(position.line);
            self.rope.line_to_char(line) + position.column.min(self.rope.line(line).len_chars())
        };
        let selections = positions
            .into_iter()
            .map(|(anchor, head)| Selection {
                anchor: map(anchor),
                head: map(head),
            })
            .collect();
        self.store_after_selections(selections);
    }

    /// Duplicate each selected line block above or below in one undoable transaction.
    pub fn duplicate_line(&mut self, below: bool) {
        if self.read_only {
            return;
        }
        let blocks = self.selected_line_blocks();
        let selections = self.selections();
        let mut edits = Vec::new();
        let mut details = Vec::new();
        for block in &blocks {
            let range = self.line_block_range(block);
            let original = self.rope.slice(range.clone()).to_string();
            let last_line = self.rope.line(block.end - 1).to_string();
            let ending = if last_line.ends_with("\r\n")
                || (!last_line.ends_with('\n')
                    && block.start > 0
                    && self
                        .rope
                        .line(block.start - 1)
                        .to_string()
                        .ends_with("\r\n"))
            {
                "\r\n"
            } else {
                "\n"
            };
            let text = if original.ends_with('\n') {
                original.clone()
            } else if below {
                format!("{ending}{original}")
            } else {
                format!("{original}{ending}")
            };
            let inserted_len = text.chars().count();
            let original_len = original.chars().count();
            let at = if below { range.end } else { range.start };
            edits.push(TextReplacement {
                range: at..at,
                text,
            });
            details.push((range, inserted_len, original_len));
        }
        let revision = self.revision();
        let Ok(cursors) = self.apply_transaction(&edits) else {
            return;
        };
        if self.revision() == revision {
            return;
        }
        let mapped = selections
            .into_iter()
            .map(|selection| {
                let selected_start = selection.anchor.min(selection.head);
                let index = details
                    .iter()
                    .position(|(range, _, _)| {
                        selected_start >= range.start && selected_start <= range.end
                    })
                    .unwrap_or(0);
                let (range, inserted_len, original_len) = &details[index];
                let copy_start = cursors[index] - if below { *original_len } else { *inserted_len };
                Selection {
                    anchor: copy_start + selection.anchor.saturating_sub(range.start),
                    head: copy_start + selection.head.saturating_sub(range.start),
                }
            })
            .collect();
        self.store_after_selections(mapped);
    }

    /// Delete all selected line blocks as one undoable transaction.
    pub fn delete_line(&mut self) {
        if self.read_only {
            return;
        }
        let blocks = self.selected_line_blocks();
        let mut ranges: Vec<Range<usize>> = blocks
            .iter()
            .map(|block| {
                let mut range = self.line_block_range(block);
                if range.end == self.len_chars()
                    && range.start > 0
                    && !self.rope.slice(range.clone()).to_string().ends_with('\n')
                {
                    range.start -= 1;
                    if range.start > 0
                        && self.rope.char(range.start) == '\n'
                        && self.rope.char(range.start - 1) == '\r'
                    {
                        range.start -= 1;
                    }
                }
                range
            })
            .collect();
        ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if let Some(previous) = merged.last_mut() {
                if range.start <= previous.end {
                    previous.end = previous.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        let edits: Vec<_> = merged
            .into_iter()
            .map(|range| TextReplacement {
                range,
                text: String::new(),
            })
            .collect();
        let _ = self.apply_preserving_selections(&edits);
    }

    /// Sort lines independently in each selected block.
    pub fn sort_lines(&mut self) {
        self.transform_selected_lines(false);
    }

    /// Remove duplicate lines independently in each selected block, keeping the first.
    pub fn deduplicate_lines(&mut self) {
        self.transform_selected_lines(true);
    }

    fn transform_selected_lines(&mut self, deduplicate: bool) {
        if self.read_only {
            return;
        }
        let mut edits = Vec::new();
        for block in self.selected_line_blocks() {
            let mut contents = Vec::new();
            let mut endings = Vec::new();
            for line in block.clone() {
                let value = self.rope.line(line).to_string();
                let (content, ending) = split_line_ending(&value);
                contents.push(content.to_owned());
                endings.push(ending.to_owned());
            }
            if deduplicate {
                let mut seen = std::collections::HashSet::new();
                contents.retain(|line| seen.insert(line.clone()));
            } else {
                contents.sort();
            }
            let trailing = endings.last().cloned().unwrap_or_default();
            let mut text = String::new();
            let count = contents.len();
            for (index, content) in contents.into_iter().enumerate() {
                text.push_str(&content);
                if index + 1 == count {
                    text.push_str(&trailing);
                } else {
                    text.push_str(&endings[index]);
                }
            }
            edits.push(TextReplacement {
                range: self.line_block_range(&block),
                text,
            });
        }
        let _ = self.apply_preserving_selections(&edits);
    }
}

fn split_line_ending(value: &str) -> (&str, &str) {
    if let Some(content) = value.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = value.strip_suffix('\n') {
        (content, "\n")
    } else {
        (value, "")
    }
}
