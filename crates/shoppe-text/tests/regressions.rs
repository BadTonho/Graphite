use shoppe_text::Buffer;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("shoppe-text-test-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create test directory: {e}"),
            }
        }
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reverse_unicode_selection_restores_on_undo_and_new_edit_discards_redo() {
    let mut buffer = Buffer::new();
    buffer.insert("a🌎éz");
    buffer.set_selection(3, 1);
    assert_eq!(buffer.selected_text().as_deref(), Some("🌎é"));
    buffer.insert("🙂");
    assert_eq!(buffer.text(), "a🙂z");
    buffer.undo();
    assert_eq!(buffer.text(), "a🌎éz");
    assert_eq!(buffer.selected_text().as_deref(), Some("🌎é"));
    buffer.insert("ç");
    buffer.redo();
    assert_eq!(buffer.text(), "açz");
}

#[test]
fn save_and_reopen_preserve_unicode_crlf_and_history() {
    let temp = TempDir::new();
    let path = temp.path().join("doc.txt");
    fs::write(&path, "a🌎\r\nbé\r\n").unwrap();
    let mut buffer = Buffer::open(&path).unwrap();
    buffer.set_cursor_line_col(1, 2);
    buffer.insert("!");
    buffer.save().unwrap();
    assert!(!buffer.is_dirty());
    assert_eq!(Buffer::open(&path).unwrap().text(), "a🌎\r\nbé!\r\n");
    buffer.undo();
    assert!(buffer.is_dirty());
    buffer.redo();
    assert!(!buffer.is_dirty());
}

#[test]
fn invalid_utf8_and_failed_save_leave_existing_data_intact() {
    let temp = TempDir::new();
    let original = temp.path().join("original.txt");
    let invalid = temp.path().join("invalid.txt");
    fs::write(&invalid, [0xff, 0xfe]).unwrap();
    assert!(Buffer::open(&invalid).is_err());
    fs::write(&original, "original").unwrap();
    let mut buffer = Buffer::open(&original).unwrap();
    buffer.insert("🌎");
    assert!(buffer.save_as(temp.path()).is_err());
    assert_eq!(buffer.path(), Some(original.as_path()));
    assert_eq!(buffer.text(), "🌎original");
    assert!(buffer.is_dirty());
    assert_eq!(fs::read_to_string(original).unwrap(), "original");
}
