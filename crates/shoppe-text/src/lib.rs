//! Text buffer backed by a rope, with cursor editing and file I/O.

use ropey::Rope;
use std::{
    io,
    path::{Path, PathBuf},
};

mod edit;
mod file_io;
mod indentation;
pub use indentation::Indentation;
mod lines;
mod multicursor;
mod navigation;
pub use multicursor::CursorMotion;
mod search;
pub use search::{SearchMatch, SearchOptions};

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

/// A replacement whose range is expressed in character offsets of the same revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextReplacement {
    pub range: std::ops::Range<usize>,
    pub text: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransactionError {
    #[error("document is read-only")]
    ReadOnly,
    #[error("replacement range is invalid or outside the document")]
    InvalidRange,
    #[error("replacement ranges overlap or contain duplicate cursors")]
    OverlappingRanges,
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
    before_extra: Vec<Selection>,
    after_extra: Vec<Selection>,
    after_selection: Option<Selection>,
}

#[derive(Debug, Default)]
pub struct Buffer {
    rope: Rope,
    path: Option<PathBuf>,
    cursor: Cursor,
    dirty: bool,
    revision: u64,
    selection: Option<Selection>,
    extra_selections: Vec<Selection>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    saved_text: Rope,
    last_change: Option<TextChange>,
    read_only: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self::default()
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

    /// A read-only buffer rejects all text mutations and history operations.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
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

    pub fn rope(&self) -> &Rope {
        &self.rope
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
        self.extra_selections.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        let path = std::env::temp_dir().join("graphite_text_roundtrip.txt");
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
        let path = std::env::temp_dir().join("graphite_text_saved_state.txt");
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

    #[test]
    fn large_unicode_file_roundtrip_and_saved_history() {
        let path = std::env::temp_dir().join(format!("graphite_large_{}.txt", std::process::id()));
        let content = "Olá 🌎\r\n".repeat(150_000);
        fs::write(&path, &content).unwrap();
        let mut b = Buffer::open(&path).unwrap();
        assert_eq!(b.rope().len_bytes(), content.len());
        b.set_cursor_line_col(149_999, 2);
        b.insert("🙂");
        b.undo();
        assert!(!b.is_dirty());
        b.redo();
        b.save().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), b.text());
        assert!(!b.is_dirty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_io_keeps_document_and_original_path() {
        let dir = std::env::temp_dir().join(format!("graphite_failed_io_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("original.txt");
        fs::write(&path, "original").unwrap();
        let mut b = Buffer::open(&path).unwrap();
        b.insert("é");
        let revision = b.revision();
        assert!(b.save_as(dir.join("missing").join("file")).is_err());
        assert_eq!(b.path(), Some(path.as_path()));
        assert_eq!(b.text(), "éoriginal");
        assert_eq!(b.revision(), revision);
        assert!(b.is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        assert!(Buffer::open(dir.join("missing-file")).is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
