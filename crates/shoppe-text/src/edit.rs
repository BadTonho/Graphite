use super::*;

impl Buffer {
    pub(super) fn replace(&mut self, start: usize, end: usize, text: &str) {
        let removed = self.rope.slice(start..end).to_string();
        if removed == text {
            return;
        }
        let before = self.cursor.char_idx;
        let before_selection = self.selection;
        self.record_change(start, end, text);
        self.rope.remove(start..end);
        self.rope.insert(start, text);
        let after = start + text.chars().count();
        self.cursor.char_idx = after;
        self.selection = None;
        self.undo.push(Edit {
            start,
            removed,
            inserted: text.to_owned(),
            before,
            after,
            before_selection,
        });
        self.redo.clear();
        self.dirty = true;
        self.revision += 1;
    }

    pub fn undo(&mut self) {
        if let Some(edit) = self.undo.pop() {
            let end = edit.start + edit.inserted.chars().count();
            self.record_change(edit.start, end, &edit.removed);
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.removed);
            self.cursor.char_idx = edit.before.min(self.rope.len_chars());
            self.selection = edit.before_selection;
            self.dirty = self.rope != self.saved_text;
            self.redo.push(edit);
            self.revision += 1;
        }
    }

    pub fn redo(&mut self) {
        if let Some(edit) = self.redo.pop() {
            let end = edit.start + edit.removed.chars().count();
            self.record_change(edit.start, end, &edit.inserted);
            self.rope.remove(edit.start..end);
            self.rope.insert(edit.start, &edit.inserted);
            self.cursor.char_idx = edit.after.min(self.rope.len_chars());
            self.selection = None;
            self.dirty = self.rope != self.saved_text;
            self.undo.push(edit);
            self.revision += 1;
        }
    }
}
