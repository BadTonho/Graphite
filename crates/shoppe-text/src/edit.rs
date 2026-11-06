use super::*;

impl Buffer {
    /// Apply disjoint replacements against one revision as a single undoable edit.
    /// Returns the resulting cursor position for each replacement in input order.
    pub fn apply_transaction(
        &mut self,
        replacements: &[TextReplacement],
    ) -> Result<Vec<usize>, TransactionError> {
        if self.read_only {
            return Err(TransactionError::ReadOnly);
        }
        let mut ordered: Vec<_> = replacements.iter().enumerate().collect();
        for (_, edit) in &ordered {
            if edit.range.start > edit.range.end || edit.range.end > self.len_chars() {
                return Err(TransactionError::InvalidRange);
            }
        }
        ordered.sort_by_key(|(_, edit)| (edit.range.start, edit.range.end));
        for pair in ordered.windows(2) {
            let a = &pair[0].1.range;
            let b = &pair[1].1.range;
            if a.end > b.start || a.start == b.start {
                return Err(TransactionError::OverlappingRanges);
            }
        }
        if ordered.is_empty() {
            return Ok(Vec::new());
        }
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
        self.replace(start, end, &result);
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
        let before = self.cursor.char_idx;
        let before_selection = self.selection;
        let before_extra = self.extra_selections.clone();
        self.record_change(start, end, text);
        self.rope.remove(start..end);
        self.rope.insert(start, text);
        let after = start + text.chars().count();
        self.cursor.char_idx = after;
        self.selection = None;
        self.extra_selections.clear();
        self.undo.push(Edit {
            start,
            removed,
            inserted: text.to_owned(),
            before,
            after,
            before_selection,
            before_extra,
            after_extra: Vec::new(),
            after_selection: None,
        });
        self.redo.clear();
        self.dirty = true;
        self.revision += 1;
    }

    pub fn undo(&mut self) {
        if self.read_only {
            return;
        }
        if let Some(edit) = self.undo.pop() {
            let end = edit.start + edit.inserted.chars().count();
            self.record_change(edit.start, end, &edit.removed);
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.removed);
            self.cursor.char_idx = edit.before.min(self.rope.len_chars());
            self.selection = edit.before_selection;
            self.extra_selections = edit.before_extra.clone();
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
            let end = edit.start + edit.removed.chars().count();
            self.record_change(edit.start, end, &edit.inserted);
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.inserted);
            self.cursor.char_idx = edit.after.min(self.rope.len_chars());
            self.selection = edit.after_selection;
            self.extra_selections = edit.after_extra.clone();
            self.dirty = self.rope != self.saved_text;
            self.undo.push(edit);
            self.revision += 1;
        }
    }
}
