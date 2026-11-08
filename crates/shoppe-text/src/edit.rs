use super::*;

impl Buffer {
    /// Validate a transaction without changing text, selections, or history.
    pub fn validate_transaction(
        &self,
        replacements: &[TextReplacement],
    ) -> Result<(), TransactionError> {
        if self.read_only {
            return Err(TransactionError::ReadOnly);
        }
        let mut ordered: Vec<_> = replacements.iter().collect();
        for edit in &ordered {
            if edit.range.start > edit.range.end || edit.range.end > self.len_chars() {
                return Err(TransactionError::InvalidRange);
            }
        }
        ordered.sort_by_key(|edit| (edit.range.start, edit.range.end));
        for pair in ordered.windows(2) {
            let a = &pair[0].range;
            let b = &pair[1].range;
            if a.end > b.start || a.start == b.start {
                return Err(TransactionError::OverlappingRanges);
            }
        }
        Ok(())
    }

    /// Apply disjoint replacements against one revision as a single undoable edit.
    /// Returns the resulting cursor position for each replacement in input order.
    pub fn apply_transaction(
        &mut self,
        replacements: &[TextReplacement],
    ) -> Result<Vec<usize>, TransactionError> {
        self.validate_transaction(replacements)?;
        let mut ordered: Vec<_> = replacements.iter().enumerate().collect();
        ordered.sort_by_key(|(_, edit)| (edit.range.start, edit.range.end));
        if ordered.is_empty() {
            return Ok(Vec::new());
        }
        let position_edits: Vec<_> = ordered
            .iter()
            .map(|(_, edit)| PositionEdit {
                start: edit.range.start,
                end: edit.range.end,
                inserted_chars: edit.text.chars().count(),
            })
            .collect();
        let start = ordered.first().unwrap().1.range.start;
        let end = ordered.last().unwrap().1.range.end;
        let mut result = String::new();
        let mut previous = start;
        let mut cursors = vec![0; ordered.len()];
        let mut output_chars = 0usize;
        for (index, edit) in ordered {
            let unchanged = self.rope.slice(previous..edit.range.start);
            output_chars += unchanged.len_chars();
            for chunk in unchanged.chunks() {
                result.push_str(chunk);
            }
            result.push_str(&edit.text);
            output_chars += edit.text.chars().count();
            cursors[index] = start + output_chars;
            previous = edit.range.end;
        }
        for chunk in self.rope.slice(previous..end).chunks() {
            result.push_str(chunk);
        }
        let revision = self.revision;
        self.replace(start, end, &result);
        if self.revision != revision {
            self.last_position_edits = position_edits.clone();
            self.undo
                .last_mut()
                .expect("replacement recorded in history")
                .position_edits = position_edits;
        }
        Ok(cursors)
    }

    pub(super) fn replace(&mut self, start: usize, end: usize, text: &str) {
        if self.read_only {
            return;
        }
        let removed = self.rope.slice(start..end).to_string();
        if removed == text {
            return;
        }
        self.record_change(start, end, text);
        self.rope.remove(start..end);
        self.rope.insert(start, text);
        let position_edits = vec![PositionEdit {
            start,
            end,
            inserted_chars: text.chars().count(),
        }];
        self.last_position_edits = position_edits.clone();
        self.undo.push(Edit {
            start,
            removed,
            inserted: text.to_owned(),
            selections: None,
            position_edits,
        });
        self.redo.clear();
        self.dirty = self.rope != self.saved_text;
        self.revision += 1;
    }

    pub fn undo(&mut self) {
        if self.read_only {
            return;
        }
        if let Some(edit) = self.undo.pop() {
            self.last_position_edits = inverse_position_edits(&edit.position_edits);
            let end = edit.start + edit.inserted.chars().count();
            self.record_change(edit.start, end, &edit.removed);
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.removed);
            self.dirty = self.rope != self.saved_text;
            self.redo.push(edit);
            self.revision += 1;
        }
    }

    pub fn redo(&mut self) {
        if self.read_only {
            return;
        }
        if let Some(edit) = self.redo.pop() {
            self.last_position_edits = edit.position_edits.clone();
            let end = edit.start + edit.removed.chars().count();
            self.record_change(edit.start, end, &edit.inserted);
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.inserted);
            self.dirty = self.rope != self.saved_text;
            self.undo.push(edit);
            self.revision += 1;
        }
    }
}

fn inverse_position_edits(changes: &[PositionEdit]) -> Vec<PositionEdit> {
    let mut delta = 0isize;
    changes
        .iter()
        .map(|change| {
            let start = change.start.saturating_add_signed(delta);
            let inverse = PositionEdit {
                start,
                end: start + change.inserted_chars,
                inserted_chars: change.end - change.start,
            };
            delta += change.inserted_chars as isize - (change.end - change.start) as isize;
            inverse
        })
        .collect()
}
