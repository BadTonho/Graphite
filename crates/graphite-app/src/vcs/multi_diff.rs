//! Model of the multi-file diff: the changes of several files in one scrollable list, each file
//! collapsible, with navigation from change to change across files. Pure text logic.

use similar::{ChangeTag, TextDiff};

/// Unchanged lines shown around each change.
const CONTEXT: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiffLine {
    pub kind: LineKind,
    /// 1-based line numbers in the old and new text; `None` on the side the line is not in.
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Line(DiffLine),
    /// Unchanged lines left out between two groups of changes.
    Gap(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileStatus {
    Added,
    Deleted,
    Modified,
}

/// One file to compare. A missing side means the file does not exist there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileInput {
    pub path: String,
    pub old: Option<String>,
    pub new: Option<String>,
    /// The file cannot be shown as text.
    pub binary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub binary: bool,
    pub pieces: Vec<Piece>,
    pub added: usize,
    pub removed: usize,
    pub collapsed: bool,
}

impl FileDiff {
    pub(crate) fn build(input: FileInput) -> Self {
        let status = match (&input.old, &input.new) {
            (None, Some(_)) => FileStatus::Added,
            (Some(_), None) => FileStatus::Deleted,
            _ => FileStatus::Modified,
        };
        let mut file = FileDiff {
            path: input.path,
            status,
            binary: input.binary,
            pieces: Vec::new(),
            added: 0,
            removed: 0,
            collapsed: false,
        };
        if input.binary {
            return file;
        }
        let old = input.old.unwrap_or_default();
        let new = input.new.unwrap_or_default();
        let diff = TextDiff::from_lines(&old, &new);
        let total_old = diff.old_slices().len();
        let groups = diff.grouped_ops(CONTEXT);
        let mut covered = 0;
        for group in &groups {
            let start = group.first().map_or(0, |op| op.old_range().start);
            if start > covered {
                file.pieces.push(Piece::Gap(start - covered));
            }
            for op in group {
                for change in diff.iter_changes(op) {
                    let kind = match change.tag() {
                        ChangeTag::Equal => LineKind::Context,
                        ChangeTag::Insert => {
                            file.added += 1;
                            LineKind::Added
                        }
                        ChangeTag::Delete => {
                            file.removed += 1;
                            LineKind::Removed
                        }
                    };
                    let text = change
                        .value()
                        .trim_end_matches('\n')
                        .trim_end_matches('\r')
                        .to_owned();
                    file.pieces.push(Piece::Line(DiffLine {
                        kind,
                        old_line: change.old_index().map(|index| index + 1),
                        new_line: change.new_index().map(|index| index + 1),
                        text,
                    }));
                }
            }
            covered = group.last().map_or(covered, |op| op.old_range().end);
        }
        if !groups.is_empty() && total_old > covered {
            file.pieces.push(Piece::Gap(total_old - covered));
        }
        file
    }

    pub(crate) fn has_changes(&self) -> bool {
        self.binary || self.added + self.removed > 0
    }
}

/// A row of the list: a file header or one piece of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowRef {
    Header(usize),
    Piece(usize, usize),
}

/// Where a change starts: a file and, for a text file, the piece inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChangeRef {
    pub file: usize,
    pub piece: Option<usize>,
}

pub(crate) struct MultiDiff {
    pub title: String,
    pub files: Vec<FileDiff>,
    /// First visible row.
    pub scroll: usize,
    /// The change navigation stands on, as an index into [`MultiDiff::changes`].
    pub current: Option<usize>,
    /// The file list has the keyboard.
    pub list_focus: bool,
    pub selected_file: usize,
    rows: Vec<RowRef>,
    changes: Vec<ChangeRef>,
}

impl MultiDiff {
    pub(crate) fn new(title: String, inputs: Vec<FileInput>) -> Self {
        let mut diff = Self {
            title,
            files: inputs.into_iter().map(FileDiff::build).collect(),
            scroll: 0,
            current: None,
            list_focus: false,
            selected_file: 0,
            rows: Vec::new(),
            changes: Vec::new(),
        };
        diff.rebuild();
        diff
    }

    pub(crate) fn rows(&self) -> &[RowRef] {
        &self.rows
    }

    pub(crate) fn changes(&self) -> &[ChangeRef] {
        &self.changes
    }

    pub(crate) fn totals(&self) -> (usize, usize) {
        self.files.iter().fold((0, 0), |(added, removed), file| {
            (added + file.added, removed + file.removed)
        })
    }

    /// Recomputes rows and the list of changes after a file was collapsed or expanded.
    fn rebuild(&mut self) {
        self.rows.clear();
        self.changes.clear();
        for (index, file) in self.files.iter().enumerate() {
            self.rows.push(RowRef::Header(index));
            if file.binary {
                self.changes.push(ChangeRef {
                    file: index,
                    piece: None,
                });
                continue;
            }
            let mut in_run = false;
            for (piece_index, piece) in file.pieces.iter().enumerate() {
                if !file.collapsed {
                    self.rows.push(RowRef::Piece(index, piece_index));
                }
                let changed = matches!(
                    piece,
                    Piece::Line(DiffLine {
                        kind: LineKind::Added | LineKind::Removed,
                        ..
                    })
                );
                if changed && !in_run {
                    self.changes.push(ChangeRef {
                        file: index,
                        piece: Some(piece_index),
                    });
                }
                in_run = changed;
            }
        }
        self.scroll = self.scroll.min(self.rows.len().saturating_sub(1));
        self.selected_file = self.selected_file.min(self.files.len().saturating_sub(1));
    }

    /// Row where a file or piece is drawn, `None` when its file is collapsed.
    pub(crate) fn row_of(&self, target: RowRef) -> Option<usize> {
        self.rows.iter().position(|row| *row == target)
    }

    pub(crate) fn toggle_file(&mut self, file: usize) {
        if let Some(entry) = self.files.get_mut(file) {
            entry.collapsed = !entry.collapsed;
            self.rebuild();
        }
    }

    pub(crate) fn collapse_all(&mut self) {
        self.files.iter_mut().for_each(|file| file.collapsed = true);
        self.rebuild();
    }

    pub(crate) fn expand_all(&mut self) {
        self.files
            .iter_mut()
            .for_each(|file| file.collapsed = false);
        self.rebuild();
    }

    fn show_change(&mut self, index: usize) {
        let change = self.changes[index];
        if self.files[change.file].collapsed {
            self.files[change.file].collapsed = false;
            self.rebuild();
        }
        self.current = Some(index);
        self.selected_file = change.file;
        let target = match change.piece {
            Some(piece) => RowRef::Piece(change.file, piece),
            None => RowRef::Header(change.file),
        };
        if let Some(row) = self.row_of(target) {
            // Keep the file header visible above the change when it is close.
            self.scroll = row.saturating_sub(CONTEXT);
        }
    }

    /// Moves to the next change, across files, wrapping around. `false` when there is none.
    pub(crate) fn next_change(&mut self) -> bool {
        if self.changes.is_empty() {
            return false;
        }
        let next = self
            .current
            .map_or(0, |index| (index + 1) % self.changes.len());
        self.show_change(next);
        true
    }

    pub(crate) fn previous_change(&mut self) -> bool {
        if self.changes.is_empty() {
            return false;
        }
        let count = self.changes.len();
        let previous = self
            .current
            .map_or(count - 1, |index| (index + count - 1) % count);
        self.show_change(previous);
        true
    }

    /// The file and line (in the new text) the current change refers to.
    pub(crate) fn current_location(&self) -> Option<(usize, usize)> {
        let change = self.changes.get(self.current?)?;
        let file = &self.files[change.file];
        let line = change.piece.and_then(|piece| {
            file.pieces[piece..].iter().find_map(|piece| match piece {
                Piece::Line(line) => line.new_line,
                Piece::Gap(_) => None,
            })
        });
        // A removal has no new line; the line after it is the closest place in the new text.
        let line = line.or_else(|| {
            change.piece.and_then(|piece| {
                file.pieces[..piece]
                    .iter()
                    .rev()
                    .find_map(|piece| match piece {
                        Piece::Line(line) => line.new_line,
                        Piece::Gap(_) => None,
                    })
            })
        });
        Some((change.file, line.unwrap_or(1)))
    }

    pub(crate) fn scroll_by(&mut self, delta: isize, window: usize) {
        let max = self.rows.len().saturating_sub(window.min(self.rows.len()));
        self.scroll = (self.scroll as isize + delta).clamp(0, max as isize) as usize;
    }

    /// Moves the file list selection and scrolls to that file.
    pub(crate) fn select_file(&mut self, delta: isize) {
        if self.files.is_empty() {
            return;
        }
        let last = self.files.len() as isize - 1;
        self.selected_file = (self.selected_file as isize + delta).clamp(0, last) as usize;
        if let Some(row) = self.row_of(RowRef::Header(self.selected_file)) {
            self.scroll = row;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(path: &str, old: Option<&str>, new: Option<&str>) -> FileInput {
        FileInput {
            path: path.into(),
            old: old.map(Into::into),
            new: new.map(Into::into),
            binary: false,
        }
    }

    fn lines(file: &FileDiff) -> Vec<(LineKind, String)> {
        file.pieces
            .iter()
            .filter_map(|piece| match piece {
                Piece::Line(line) => Some((line.kind, line.text.clone())),
                Piece::Gap(_) => None,
            })
            .collect()
    }

    #[test]
    fn a_modified_file_shows_changes_with_numbers_and_counts() {
        let file = FileDiff::build(input(
            "a.txt",
            Some("one\ntwo\nthree\n"),
            Some("one\n2\nthree\n"),
        ));
        assert_eq!(file.status, FileStatus::Modified);
        assert_eq!((file.added, file.removed), (1, 1));
        assert_eq!(
            lines(&file),
            vec![
                (LineKind::Context, "one".into()),
                (LineKind::Removed, "two".into()),
                (LineKind::Added, "2".into()),
                (LineKind::Context, "three".into()),
            ]
        );
        let Piece::Line(removed) = &file.pieces[1] else {
            panic!("expected a line");
        };
        assert_eq!((removed.old_line, removed.new_line), (Some(2), None));
        let Piece::Line(added) = &file.pieces[2] else {
            panic!("expected a line");
        };
        assert_eq!((added.old_line, added.new_line), (None, Some(2)));
    }

    #[test]
    fn unchanged_stretches_become_gaps_and_context_is_three_lines() {
        let old: String = (1..=20).map(|n| format!("l{n}\n")).collect();
        let new = old.replace("l2\n", "L2\n").replace("l19\n", "L19\n");
        let file = FileDiff::build(input("a", Some(&old), Some(&new)));
        let gaps: Vec<usize> = file
            .pieces
            .iter()
            .filter_map(|piece| match piece {
                Piece::Gap(count) => Some(*count),
                Piece::Line(_) => None,
            })
            .collect();
        // Lines 6 to 15 are left out between the two changes.
        assert_eq!(gaps, vec![10]);
        assert_eq!(lines(&file).len(), 12);
    }

    #[test]
    fn added_deleted_and_binary_files_are_told_apart() {
        let added = FileDiff::build(input("n", None, Some("x\ny\n")));
        assert_eq!(added.status, FileStatus::Added);
        assert_eq!((added.added, added.removed), (2, 0));
        let deleted = FileDiff::build(input("d", Some("x\n"), None));
        assert_eq!(deleted.status, FileStatus::Deleted);
        assert_eq!((deleted.added, deleted.removed), (0, 1));
        let binary = FileDiff::build(FileInput {
            path: "b.png".into(),
            old: None,
            new: None,
            binary: true,
        });
        assert!(binary.pieces.is_empty() && binary.has_changes());
        let same = FileDiff::build(input("s", Some("a\n"), Some("a\n")));
        assert!(!same.has_changes() && same.pieces.is_empty());
    }

    #[test]
    fn crlf_and_missing_final_newline_are_handled() {
        let file = FileDiff::build(input("c", Some("a\r\nb"), Some("a\r\nc")));
        assert_eq!(
            lines(&file),
            vec![
                (LineKind::Context, "a".into()),
                (LineKind::Removed, "b".into()),
                (LineKind::Added, "c".into()),
            ]
        );
    }

    fn three_files() -> MultiDiff {
        MultiDiff::new(
            "changes".into(),
            vec![
                input("a.txt", Some("1\n2\n3\n"), Some("1\nX\n3\n")),
                input("same.txt", Some("s\n"), Some("s\n")),
                input("b.txt", None, Some("new\n")),
            ],
        )
    }

    #[test]
    fn rows_list_headers_and_pieces_and_collapsing_hides_pieces() {
        let mut diff = three_files();
        // a: header + 4 lines; same: header; b: header + 1 line.
        assert_eq!(diff.rows().len(), 5 + 1 + 2);
        assert_eq!(diff.totals(), (2, 1));
        diff.toggle_file(0);
        assert_eq!(diff.rows().len(), 1 + 1 + 2);
        diff.collapse_all();
        assert_eq!(diff.rows().len(), 3);
        diff.expand_all();
        assert_eq!(diff.rows().len(), 8);
        diff.toggle_file(9);
        assert_eq!(diff.rows().len(), 8);
    }

    #[test]
    fn changes_are_runs_of_changed_lines_and_navigation_crosses_files_and_wraps() {
        let mut diff = three_files();
        // One run in a.txt, one in b.txt; the unchanged file has none.
        assert_eq!(diff.changes().len(), 2);
        assert!(diff.next_change());
        assert_eq!(diff.current, Some(0));
        assert_eq!(diff.selected_file, 0);
        assert!(diff.next_change());
        assert_eq!(diff.current, Some(1));
        assert_eq!(diff.selected_file, 2);
        assert!(diff.next_change());
        assert_eq!(diff.current, Some(0));
        assert!(diff.previous_change());
        assert_eq!(diff.current, Some(1));
        assert!(diff.previous_change());
        assert_eq!(diff.current, Some(0));
    }

    #[test]
    fn navigating_to_a_change_in_a_collapsed_file_expands_it() {
        let mut diff = three_files();
        diff.collapse_all();
        // Collapsed files keep their changes in the navigation list.
        assert_eq!(diff.changes().len(), 2);
        assert!(diff.next_change());
        assert!(!diff.files[0].collapsed);
        assert!(diff.files[2].collapsed);
        let row = diff.row_of(RowRef::Piece(0, 1)).unwrap();
        assert_eq!(diff.scroll, row.saturating_sub(3));
    }

    #[test]
    fn a_change_run_is_one_stop_and_binary_files_stop_on_their_header() {
        let mut diff = MultiDiff::new(
            "t".into(),
            vec![
                input("a", Some("1\n2\n3\n4\n"), Some("1\nA\nB\n4\n")),
                FileInput {
                    path: "img.png".into(),
                    old: None,
                    new: None,
                    binary: true,
                },
            ],
        );
        assert_eq!(diff.changes().len(), 2);
        assert!(diff.next_change() && diff.next_change());
        assert_eq!(diff.changes()[1].piece, None);
        assert_eq!(
            diff.scroll,
            diff.row_of(RowRef::Header(1)).unwrap().saturating_sub(3)
        );
    }

    #[test]
    fn without_changes_navigation_reports_nothing() {
        let mut diff = MultiDiff::new("t".into(), vec![input("s", Some("a\n"), Some("a\n"))]);
        assert!(!diff.next_change());
        assert!(!diff.previous_change());
        assert_eq!(diff.current_location(), None);
        let mut empty = MultiDiff::new("t".into(), Vec::new());
        assert!(!empty.next_change());
        empty.select_file(1);
        empty.collapse_all();
    }

    #[test]
    fn current_location_points_at_the_new_line() {
        let mut diff = three_files();
        diff.next_change();
        assert_eq!(diff.current_location(), Some((0, 2)));
        diff.next_change();
        assert_eq!(diff.current_location(), Some((2, 1)));
        // A pure removal points at the nearest line of the new text.
        let mut removal = MultiDiff::new(
            "t".into(),
            vec![input("r", Some("a\nb\nc\n"), Some("a\nc\n"))],
        );
        removal.next_change();
        assert_eq!(removal.current_location(), Some((0, 2)));
        let mut deleted = MultiDiff::new("t".into(), vec![input("d", Some("a\n"), None)]);
        deleted.next_change();
        assert_eq!(deleted.current_location(), Some((0, 1)));
    }

    #[test]
    fn selecting_files_and_scrolling_are_clamped() {
        let mut diff = three_files();
        diff.select_file(1);
        assert_eq!(diff.selected_file, 1);
        assert_eq!(diff.scroll, 5);
        diff.select_file(10);
        assert_eq!(diff.selected_file, 2);
        diff.select_file(-10);
        assert_eq!(diff.selected_file, 0);
        diff.scroll_by(100, 3);
        assert_eq!(diff.scroll, 5);
        diff.scroll_by(-100, 3);
        assert_eq!(diff.scroll, 0);
        diff.scroll_by(100, 100);
        assert_eq!(diff.scroll, 0);
    }
}
