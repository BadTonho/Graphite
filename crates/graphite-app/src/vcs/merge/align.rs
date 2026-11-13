//! Aligned rows for the merge editor panes. Every pane shows the same row window, so one scroll
//! offset keeps current, incoming, base and result in step even though a conflict has a
//! different number of lines on each side.

use super::parse::Segment;
use super::resolve::{MergeState, Resolution};
use similar::{ChangeTag, TextDiff};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pane {
    Current = 0,
    Incoming = 1,
    Base = 2,
    Result = 3,
}

impl Pane {
    pub(crate) const ALL: [Pane; 4] = [Pane::Current, Pane::Incoming, Pane::Base, Pane::Result];
}

/// One line of one pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    /// 1-based line number inside that pane's own text.
    pub line: usize,
    pub text: String,
    /// The line differs from the other side of the same conflict.
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    /// The conflict this row belongs to, `None` for plain text shared by every pane.
    pub block: Option<usize>,
    pub first_of_block: bool,
    pub resolution: Resolution,
    cells: [Option<Cell>; 4],
}

impl Row {
    pub(crate) fn cell(&self, pane: Pane) -> Option<&Cell> {
        self.cells[pane as usize].as_ref()
    }
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n')
        .map(|line| {
            line.strip_suffix("\r\n")
                .or_else(|| line.strip_suffix('\n'))
                .unwrap_or(line)
        })
        .collect()
}

/// For each line of `current` and `incoming`, whether the other side lacks it.
fn changed_lines(current: &str, incoming: &str) -> (Vec<bool>, Vec<bool>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for change in TextDiff::from_lines(current, incoming).iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                left.push(false);
                right.push(false);
            }
            ChangeTag::Delete => left.push(true),
            ChangeTag::Insert => right.push(true),
        }
    }
    (left, right)
}

/// Builds the rows of every pane. Plain text appears in all panes; a conflict takes as many rows
/// as its longest pane and shorter panes are left empty there.
pub(crate) fn aligned_rows(state: &MergeState) -> Vec<Row> {
    let mut rows = Vec::new();
    // Next line number to hand out in each pane.
    let mut next = [1usize; 4];
    let mut block_index = 0;
    for segment in &state.parsed().segments {
        match segment {
            Segment::Text(text) => {
                for line in lines_of(text) {
                    let mut cells: [Option<Cell>; 4] = [None, None, None, None];
                    for pane in Pane::ALL {
                        cells[pane as usize] = Some(Cell {
                            line: next[pane as usize],
                            text: line.to_owned(),
                            changed: false,
                        });
                        next[pane as usize] += 1;
                    }
                    rows.push(Row {
                        block: None,
                        first_of_block: false,
                        resolution: Resolution::Unresolved,
                        cells,
                    });
                }
            }
            Segment::Conflict(block) => {
                let resolution = state.resolution(block_index).unwrap_or_default();
                let (current_changed, incoming_changed) =
                    changed_lines(&block.current, &block.incoming);
                let result_text = state
                    .resolved_text(block_index)
                    .unwrap_or_else(|| block.base.clone().unwrap_or_default());
                let columns: [(Vec<&str>, Vec<bool>); 4] = [
                    (lines_of(&block.current), current_changed),
                    (lines_of(&block.incoming), incoming_changed),
                    (lines_of(block.base.as_deref().unwrap_or("")), Vec::new()),
                    (lines_of(&result_text), Vec::new()),
                ];
                let height = columns
                    .iter()
                    .map(|(lines, _)| lines.len())
                    .max()
                    .unwrap_or(0)
                    .max(1);
                for offset in 0..height {
                    let mut cells: [Option<Cell>; 4] = [None, None, None, None];
                    for pane in Pane::ALL {
                        let (lines, changed) = &columns[pane as usize];
                        if let Some(line) = lines.get(offset) {
                            cells[pane as usize] = Some(Cell {
                                line: next[pane as usize],
                                text: (*line).to_owned(),
                                changed: changed.get(offset).copied().unwrap_or(false),
                            });
                            next[pane as usize] += 1;
                        }
                    }
                    rows.push(Row {
                        block: Some(block_index),
                        first_of_block: offset == 0,
                        resolution,
                        cells,
                    });
                }
                block_index += 1;
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(row: &Row, pane: Pane) -> Option<&str> {
        row.cell(pane).map(|cell| cell.text.as_str())
    }

    #[test]
    fn plain_text_is_identical_in_every_pane_with_own_line_numbers() {
        let rows = aligned_rows(&MergeState::new("a\nb\n"));
        assert_eq!(rows.len(), 2);
        for pane in Pane::ALL {
            assert_eq!(text_of(&rows[1], pane), Some("b"));
            assert_eq!(rows[1].cell(pane).unwrap().line, 2);
        }
        assert_eq!(rows[0].block, None);
    }

    #[test]
    fn shorter_panes_are_padded_and_line_numbers_follow_each_pane() {
        let state = MergeState::new("a\n<<<<<<< x\n1\n2\n3\n=======\nz\n>>>>>>> y\nb\n");
        let rows = aligned_rows(&state);
        // a, three conflict rows, b.
        assert_eq!(rows.len(), 5);
        assert!(rows[1].first_of_block && !rows[2].first_of_block);
        assert_eq!(text_of(&rows[3], Pane::Current), Some("3"));
        assert_eq!(text_of(&rows[3], Pane::Incoming), None);
        assert_eq!(text_of(&rows[3], Pane::Base), None);
        assert_eq!(rows[4].cell(Pane::Current).unwrap().line, 5);
        assert_eq!(rows[4].cell(Pane::Incoming).unwrap().line, 3);
        // The result starts empty (no base), so its next line is still 2.
        assert_eq!(rows[4].cell(Pane::Result).unwrap().line, 2);
    }

    #[test]
    fn differences_between_the_two_sides_are_marked() {
        let state = MergeState::new("<<<<<<< x\nsame\nours\n=======\nsame\ntheirs\n>>>>>>> y\n");
        let rows = aligned_rows(&state);
        assert!(!rows[0].cell(Pane::Current).unwrap().changed);
        assert!(rows[1].cell(Pane::Current).unwrap().changed);
        assert!(!rows[0].cell(Pane::Incoming).unwrap().changed);
        assert!(rows[1].cell(Pane::Incoming).unwrap().changed);
    }

    #[test]
    fn result_pane_shows_the_resolution_or_the_base_until_decided() {
        let mut state = MergeState::new("<<<<<<< x\nc\n||||||| o\nb\n=======\ni\n>>>>>>> y\n");
        assert_eq!(text_of(&aligned_rows(&state)[0], Pane::Result), Some("b"));
        assert!(!aligned_rows(&state)[0].resolution.is_handled());
        state.accept_incoming(0);
        let rows = aligned_rows(&state);
        assert_eq!(text_of(&rows[0], Pane::Result), Some("i"));
        assert!(rows[0].resolution.is_handled());
        assert_eq!(text_of(&rows[0], Pane::Base), Some("b"));
    }

    #[test]
    fn an_empty_conflict_still_takes_one_row() {
        let rows = aligned_rows(&MergeState::new("<<<<<<< x\n=======\n>>>>>>> y\n"));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].first_of_block);
        assert_eq!(text_of(&rows[0], Pane::Current), None);
    }

    #[test]
    fn crlf_is_not_part_of_the_displayed_text() {
        let rows = aligned_rows(&MergeState::new("a\r\nb\r\n"));
        assert_eq!(text_of(&rows[0], Pane::Current), Some("a"));
    }
}
