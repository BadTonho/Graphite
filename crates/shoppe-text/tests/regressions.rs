use shoppe_text::{
    Buffer, CursorMotion, Indentation, SearchOptions, Selection, TextReplacement, TransactionError,
};
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
fn line_end_and_vertical_navigation_stop_before_crlf() {
    let mut b = Buffer::new();
    b.insert("ab\r\n🌎\r\n");
    b.set_cursor_line_col(0, usize::MAX);
    assert_eq!(b.cursor_line_col(), (0, 2));
    b.move_vertical(1);
    assert_eq!(b.cursor_line_col(), (1, 1));
    b.insert("!");
    assert_eq!(b.text(), "ab\r\n🌎!\r\n");
}

#[test]
fn crlf_navigation_and_deletion_keep_line_endings_intact() {
    let mut b = Buffer::new();
    b.insert("a\r\nb\r\nc");
    b.set_cursor_line_col(1, 0);
    b.move_left();
    assert_eq!(b.cursor_line_col(), (0, 1));
    b.move_right();
    assert_eq!(b.cursor_line_col(), (1, 0));
    b.backspace();
    assert_eq!(b.text(), "ab\r\nc");
    b.undo();
    b.set_cursor_line_col(0, 1);
    b.delete();
    assert_eq!(b.text(), "ab\r\nc");
    b.undo();
    b.set_selections(vec![
        Selection { anchor: 0, head: 0 },
        Selection { anchor: 3, head: 3 },
    ]);
    b.backspace();
    assert_eq!(b.text(), "ab\r\nc");
    assert_eq!(
        b.selections(),
        vec![
            Selection { anchor: 0, head: 0 },
            Selection { anchor: 1, head: 1 }
        ]
    );
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

#[test]
fn line_actions_preserve_unicode_crlf_and_undo() {
    let mut b = Buffer::new();
    b.insert("α\r\n🌎\r\nlast");
    b.set_cursor_line_col(1, 1);
    b.move_line(-1);
    assert_eq!(b.text(), "🌎\r\nα\r\nlast");
    assert_eq!(b.cursor_line_col(), (0, 1));
    b.undo();
    assert_eq!(b.text(), "α\r\n🌎\r\nlast");
    b.set_cursor_line_col(2, 2);
    b.duplicate_line(true);
    assert_eq!(b.text(), "α\r\n🌎\r\nlast\r\nlast");
    b.undo();
    assert_eq!(b.text(), "α\r\n🌎\r\nlast");
    b.set_cursor_line_col(2, 0);
    b.delete_line();
    assert_eq!(b.text(), "α\r\n🌎");
    b.undo();
    assert_eq!(b.text(), "α\r\n🌎\r\nlast");
}

#[test]
fn readonly_blocks_mutations_and_history_without_losing_history() {
    let mut b = Buffer::new();
    b.insert("a🌎");
    let revision = b.revision();
    b.set_read_only(true);
    b.insert("x");
    b.backspace();
    b.delete();
    b.undo();
    b.move_line(1);
    b.duplicate_line(true);
    b.delete_line();
    assert_eq!(b.text(), "a🌎");
    assert_eq!(b.revision(), revision);
    assert!(b.save().is_err());
    b.set_read_only(false);
    b.undo();
    assert_eq!(b.text(), "");
}

#[test]
fn file_permissions_enable_read_only_on_open() {
    let temp = TempDir::new();
    let path = temp.path().join("protected.txt");
    fs::write(&path, "original").unwrap();
    let original = fs::metadata(&path).unwrap().permissions();
    let mut protected = original.clone();
    protected.set_readonly(true);
    fs::set_permissions(&path, protected).unwrap();
    let mut b = Buffer::open(&path).unwrap();
    assert!(b.is_read_only());
    b.insert("changed");
    assert_eq!(b.text(), "original");
    assert!(b.save().is_err());
    fs::set_permissions(&path, original).unwrap();
}

#[test]
fn multiple_unicode_edits_are_one_revision_and_one_undo() {
    let mut b = Buffer::new();
    b.insert("a🌎b\nabc");
    let before = b.revision();
    let cursors = b
        .apply_transaction(&[
            TextReplacement {
                range: 5..6,
                text: "é".into(),
            },
            TextReplacement {
                range: 1..2,
                text: "🚀!".into(),
            },
        ])
        .unwrap();
    assert_eq!(b.text(), "a🚀!b\naéc");
    assert_eq!(cursors, [7, 3]);
    assert_eq!(b.revision(), before + 1);
    b.undo();
    assert_eq!(b.text(), "a🌎b\nabc");
    b.redo();
    assert_eq!(b.text(), "a🚀!b\naéc");
}

#[test]
fn multiple_cursors_edit_and_history_restore_all_positions() {
    let mut b = Buffer::new();
    b.insert("a🌎b\na🌎b");
    b.set_selections(vec![
        Selection { anchor: 1, head: 2 },
        Selection { anchor: 5, head: 6 },
    ]);
    let before = b.revision();
    b.insert("é");
    assert_eq!(b.text(), "aéb\naéb");
    assert_eq!(b.revision(), before + 1);
    assert_eq!(
        b.selections(),
        vec![
            Selection { anchor: 2, head: 2 },
            Selection { anchor: 6, head: 6 }
        ]
    );
    b.undo();
    assert_eq!(b.text(), "a🌎b\na🌎b");
    assert_eq!(b.selected_texts(), ["🌎", "🌎"]);
    b.redo();
    assert_eq!(b.text(), "aéb\naéb");
    b.move_selections(CursorMotion::Left, false);
    b.delete();
    assert_eq!(b.text(), "ab\nab");
    b.undo();
    assert_eq!(b.text(), "aéb\naéb");
}

#[test]
fn indentation_conversion_and_trim_preserve_crlf_and_undo() {
    let mut b = Buffer::new();
    b.insert("fn x() {  \r\n    α();\t\r\n        β();  \r\n}\r\n");
    let original = b.text();
    assert_eq!(
        b.detect_indentation(),
        Some(Indentation {
            insert_spaces: true,
            tab_size: 4
        })
    );
    assert_eq!(
        b.convert_indentation(Indentation {
            insert_spaces: false,
            tab_size: 4
        })
        .unwrap(),
        2
    );
    assert!(b.text().contains("\tα();"));
    b.undo();
    assert_eq!(b.text(), original);
    assert_eq!(b.trim_trailing_whitespace().unwrap(), 3);
    assert_eq!(b.text(), "fn x() {\r\n    α();\r\n        β();\r\n}\r\n");
    b.undo();
    assert_eq!(b.text(), original);
    b.set_read_only(true);
    assert_eq!(
        b.trim_trailing_whitespace(),
        Err(TransactionError::ReadOnly)
    );
}

#[test]
fn trimming_keeps_multiple_cursors_at_their_text_positions() {
    let mut b = Buffer::new();
    b.insert("a  \nb  \n");
    b.set_selections(vec![
        Selection { anchor: 1, head: 1 },
        Selection { anchor: 5, head: 5 },
    ]);
    b.trim_trailing_whitespace().unwrap();
    assert_eq!(b.text(), "a\nb\n");
    assert_eq!(
        b.selections(),
        vec![
            Selection { anchor: 1, head: 1 },
            Selection { anchor: 3, head: 3 }
        ]
    );
    b.undo();
    assert_eq!(
        b.selections(),
        vec![
            Selection { anchor: 1, head: 1 },
            Selection { anchor: 5, head: 5 }
        ]
    );
    b.redo();
    assert_eq!(
        b.selections(),
        vec![
            Selection { anchor: 1, head: 1 },
            Selection { anchor: 3, head: 3 }
        ]
    );
}

#[test]
fn next_and_all_occurrences_create_unicode_safe_selections() {
    let mut b = Buffer::new();
    b.insert("école école 🌎 école");
    b.set_cursor_line_col(0, 2);
    b.select_occurrences(false);
    assert_eq!(b.selected_texts(), ["école"]);
    b.select_occurrences(false);
    assert_eq!(b.selected_texts(), ["école", "école"]);
    b.select_occurrences(true);
    assert_eq!(b.selected_texts(), ["école", "école", "école"]);
    b.insert("X");
    assert_eq!(b.text(), "X X 🌎 X");
    b.undo();
    assert_eq!(b.selected_texts(), ["école", "école", "école"]);
}

#[test]
fn multi_cursor_paste_distributes_lines_and_undoes_as_one_edit() {
    let mut b = Buffer::new();
    b.insert("a\nb");
    b.set_selections(vec![
        Selection { anchor: 1, head: 1 },
        Selection { anchor: 3, head: 3 },
    ]);
    b.paste("🌎\r\né");
    assert_eq!(b.text(), "a🌎\nbé");
    b.undo();
    assert_eq!(b.text(), "a\nb");
    b.paste("single");
    assert_eq!(b.text(), "asingle\nbsingle");
}

#[test]
fn invalid_or_overlapping_transactions_are_atomic() {
    let mut b = Buffer::new();
    b.insert("abcd");
    let before = b.revision();
    let edits = [
        TextReplacement {
            range: 1..3,
            text: "x".into(),
        },
        TextReplacement {
            range: 2..4,
            text: "y".into(),
        },
    ];
    assert_eq!(
        b.apply_transaction(&edits),
        Err(TransactionError::OverlappingRanges)
    );
    assert_eq!(
        b.apply_transaction(&[TextReplacement {
            range: 5..6,
            text: "x".into()
        }]),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(b.text(), "abcd");
    assert_eq!(b.revision(), before);
    b.set_read_only(true);
    assert_eq!(b.apply_transaction(&edits), Err(TransactionError::ReadOnly));
}

#[test]
fn replace_selected_match_checks_context_captures_and_whole_word() {
    let mut b = Buffer::new();
    b.insert("cat cater CAT");
    let options = SearchOptions {
        regex: true,
        whole_word: true,
        preserve_case: true,
        ..Default::default()
    };
    b.set_selection(4, 7);
    assert!(!b
        .replace_selected_match("(cat)", "${1}fish", options, None)
        .unwrap());
    assert_eq!(b.text(), "cat cater CAT");
    b.set_selection(10, 13);
    assert!(b
        .replace_selected_match("(cat)", "${1}fish", options, None)
        .unwrap());
    assert_eq!(b.text(), "cat cater CATFISH");
    b.undo();
    assert_eq!(b.text(), "cat cater CAT");
}

#[test]
fn search_respects_unicode_whole_words_selection_and_regex_errors() {
    let mut b = Buffer::new();
    b.insert("Olá olá olá_mundo 🌎 olá");
    let options = SearchOptions {
        whole_word: true,
        ..Default::default()
    };
    let matches = b.find_matches("olá", options, None).unwrap();
    assert_eq!(
        matches.iter().map(|m| m.range.clone()).collect::<Vec<_>>(),
        vec![0..3, 4..7, 20..23]
    );
    assert_eq!(b.find_matches("olá", options, Some(4..7)).unwrap().len(), 1);
    assert!(b
        .find_matches(
            "(",
            SearchOptions {
                regex: true,
                ..Default::default()
            },
            None
        )
        .is_err());
}

#[test]
fn replace_all_supports_captures_case_and_single_undo() {
    let mut b = Buffer::new();
    b.insert("CAT cat Cat CATS");
    let options = SearchOptions {
        regex: true,
        whole_word: true,
        preserve_case: true,
        ..Default::default()
    };
    assert_eq!(
        b.replace_matches("(cat)", "${1}fish", options, None)
            .unwrap(),
        3
    );
    assert_eq!(b.text(), "CATFISH catfish Catfish CATS");
    b.undo();
    assert_eq!(b.text(), "CAT cat Cat CATS");
}

#[test]
fn selected_lines_sort_and_deduplicate_with_single_undo() {
    let mut b = Buffer::new();
    b.insert("outside\r\nzé\r\na\r\nzé\r\nend");
    b.set_selection(9, 19);
    b.deduplicate_lines();
    assert_eq!(b.text(), "outside\r\nzé\r\na\r\nend");
    b.undo();
    assert_eq!(b.text(), "outside\r\nzé\r\na\r\nzé\r\nend");
    b.sort_lines();
    assert_eq!(b.text(), "outside\r\na\r\nzé\r\nzé\r\nend");
    b.undo();
    assert_eq!(b.text(), "outside\r\nzé\r\na\r\nzé\r\nend");
}
