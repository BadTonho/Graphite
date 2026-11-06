//! Text buffer backed by a rope, with cursor editing and file I/O.

use ropey::Rope;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Cursor position expressed in character offsets.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub char_idx: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct TextChange {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_row: usize,
    pub start_column: usize,
    pub old_end_row: usize,
    pub old_end_column: usize,
    pub new_end_row: usize,
    pub new_end_column: usize,
}

#[derive(Debug, Clone)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    before: usize,
    after: usize,
    before_selection: Option<Selection>,
}

#[derive(Debug, Default)]
pub struct Buffer {
    rope: Rope,
    path: Option<PathBuf>,
    cursor: Cursor,
    dirty: bool,
    revision: u64,
    selection: Option<Selection>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    saved_text: String,
    last_change: Option<TextChange>,
}

impl Buffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, BufferError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        Ok(Self {
            rope: Rope::from_str(&text),
            path: Some(path.to_path_buf()),
            cursor: Cursor::default(),
            dirty: false,
            revision: 0,
            selection: None,
            undo: Vec::new(),
            redo: Vec::new(),
            saved_text: text,
            last_change: None,
        })
    }

    pub fn save(&mut self) -> Result<(), BufferError> {
        if let Some(path) = &self.path {
            fs::write(path, self.rope.to_string())?;
            self.saved_text = self.rope.to_string();
            self.dirty = false;
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), BufferError> {
        self.path = Some(path.as_ref().to_path_buf());
        self.save()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Increments on every edit; use it to invalidate caches derived from the text.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn last_change(&self) -> Option<TextChange> {
        self.last_change
    }

    fn record_change(&mut self, start: usize, end: usize, inserted: &str) {
        let start_byte = self.rope.char_to_byte(start);
        let old_end_byte = self.rope.char_to_byte(end);
        let start_row = self.rope.char_to_line(start);
        let old_end_row = self.rope.char_to_line(end);
        let start_column = start_byte - self.rope.line_to_byte(start_row);
        let old_end_column = old_end_byte - self.rope.line_to_byte(old_end_row);
        let newlines = inserted.bytes().filter(|b| *b == b'\n').count();
        let (new_end_row, new_end_column) = if newlines == 0 {
            (start_row, start_column + inserted.len())
        } else {
            (
                start_row + newlines,
                inserted.len() - inserted.rfind('\n').unwrap() - 1,
            )
        };
        self.last_change = Some(TextChange {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + inserted.len(),
            start_row,
            start_column,
            old_end_row,
            old_end_column,
            new_end_row,
            new_end_column,
        });
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line(&self, idx: usize) -> String {
        self.rope.line(idx).to_string()
    }
    pub fn line_start_char(&self, idx: usize) -> usize {
        self.rope.line_to_char(idx)
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection.map(|s| {
            self.rope
                .slice(s.anchor.min(s.head)..s.anchor.max(s.head))
                .to_string()
        })
    }

    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        let len = self.rope.len_chars();
        let anchor = anchor.min(len);
        let head = head.min(len);
        self.selection = (anchor != head).then_some(Selection { anchor, head });
        self.cursor.char_idx = head;
    }

    fn replace(&mut self, start: usize, end: usize, text: &str) {
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
            self.dirty = self.rope.to_string() != self.saved_text;
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
            self.dirty = self.rope.to_string() != self.saved_text;
            self.undo.push(edit);
            self.revision += 1;
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_backspace() {
        let mut b = Buffer::new();
        b.insert("héllo");
        assert_eq!(b.text(), "héllo");
        b.backspace();
        assert_eq!(b.text(), "héll");
        assert!(b.is_dirty());
    }

    #[test]
    fn vertical_movement_clamps_column() {
        let mut b = Buffer::new();
        b.insert("abcdef\nab\nabcd");
        b.cursor.char_idx = 5; // line 0, col 5
        b.move_vertical(1);
        assert_eq!(b.cursor_line_col(), (1, 2));
        b.move_vertical(1);
        assert_eq!(b.cursor_line_col(), (2, 2));
    }

    #[test]
    fn save_roundtrip() {
        let path = std::env::temp_dir().join("shoppe_text_roundtrip.txt");
        let mut b = Buffer::new();
        b.insert("olá\nmundo");
        b.save_as(&path).unwrap();
        let loaded = Buffer::open(&path).unwrap();
        assert_eq!(loaded.text(), "olá\nmundo");
        assert!(!loaded.is_dirty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn selection_replace_and_unicode_undo_redo() {
        let mut b = Buffer::new();
        b.insert("Olá, mundo");
        b.set_selection(5, 10);
        assert_eq!(b.selected_text().as_deref(), Some("mundo"));
        b.insert("🌎");
        assert_eq!(b.text(), "Olá, 🌎");
        b.undo();
        assert_eq!(b.text(), "Olá, mundo");
        b.redo();
        assert_eq!(b.text(), "Olá, 🌎");
    }

    #[test]
    fn undo_to_saved_text_clears_dirty_state() {
        let path = std::env::temp_dir().join("shoppe_text_saved_state.txt");
        let mut b = Buffer::new();
        b.insert("saved");
        b.save_as(&path).unwrap();
        b.insert(" change");
        assert!(b.is_dirty());
        b.undo();
        assert!(!b.is_dirty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn edit_delta_tracks_utf8_bytes_and_lines() {
        let mut b = Buffer::new();
        b.insert("a\nb");
        let first = b.last_change().unwrap();
        assert_eq!(
            (first.new_end_byte, first.new_end_row, first.new_end_column),
            (3, 1, 1)
        );
        b.set_cursor_line_col(1, 1);
        b.insert("🌎");
        let edit = b.last_change().unwrap();
        assert_eq!(
            (edit.start_byte, edit.old_end_byte, edit.new_end_byte),
            (3, 3, 7)
        );
        assert_eq!(
            (
                edit.start_row,
                edit.start_column,
                edit.new_end_row,
                edit.new_end_column
            ),
            (1, 1, 1, 5)
        );
    }
}
