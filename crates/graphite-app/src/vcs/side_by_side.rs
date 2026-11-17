//! Model of the two-file diff view: aligned rows for the side-by-side layout, with the words
//! that changed inside a modified line marked, and the same rows flattened for the inline
//! layout. Pure text logic, no drawing.
use similar::{DiffOp, TextDiff};
use std::ops::Range;

/// Unchanged lines kept around each change when unchanged regions are collapsed.
pub(crate) const CONTEXT: usize = 3;

/// One line on one side of a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    /// 1-based line number in that file.
    pub number: usize,
    pub text: String,
    /// Character ranges of `text` that differ from the paired line (empty when the whole line
    /// is new or gone, or when the line is unchanged).
    pub changed: Vec<Range<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowKind {
    Equal,
    /// A line that only exists on the left.
    Removed,
    /// A line that only exists on the right.
    Added,
    /// A left line replaced by a right line.
    Modified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub kind: RowKind,
    pub left: Option<Cell>,
    pub right: Option<Cell>,
}

/// What the view draws: a row, or a folded run of unchanged rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Row(Row),
    Folded(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DiffModel {
    pub rows: Vec<Row>,
    pub added: usize,
    pub removed: usize,
}

/// Tabs become spaces and carriage returns are dropped, so columns match what is drawn.
fn clean(line: &str) -> String {
    line.trim_end_matches(['\r', '\n']).replace('\t', "    ")
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(clean).collect()
}

/// Splits a line into words (letters, digits, `_`) and single other characters.
fn tokens(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut word_start: Option<usize> = None;
    for (index, ch) in line.char_indices() {
        let word = ch.is_alphanumeric() || ch == '_';
        match (word, word_start) {
            (true, None) => word_start = Some(index),
            (false, Some(start)) => {
                out.push(&line[start..index]);
                out.push(&line[index..index + ch.len_utf8()]);
                word_start = None;
            }
            (false, None) => out.push(&line[index..index + ch.len_utf8()]),
            (true, Some(_)) => {}
        }
    }
    if let Some(start) = word_start {
        out.push(&line[start..]);
    }
    out
}

/// Character offset where each token starts, plus the total length at the end.
fn offsets(tokens: &[&str]) -> Vec<usize> {
    let mut at = 0;
    let mut out = Vec::with_capacity(tokens.len() + 1);
    for token in tokens {
        out.push(at);
        at += token.chars().count();
    }
    out.push(at);
    out
}

/// Character ranges of `old` and `new` that are not shared, word by word.
fn changed_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let (old_tokens, new_tokens) = (tokens(old), tokens(new));
    let (old_at, new_at) = (offsets(&old_tokens), offsets(&new_tokens));
    let diff = TextDiff::from_slices(&old_tokens, &new_tokens);
    let (mut left, mut right): (Vec<Range<usize>>, Vec<Range<usize>>) = (Vec::new(), Vec::new());
    let mark = |ranges: &mut Vec<Range<usize>>, at: &[usize], index: usize, len: usize| {
        if len > 0 {
            push_range(ranges, at[index], at[index + len] - at[index]);
        }
    };
    for op in diff.ops() {
        match *op {
            DiffOp::Equal { .. } => {}
            DiffOp::Delete {
                old_index, old_len, ..
            } => mark(&mut left, &old_at, old_index, old_len),
            DiffOp::Insert {
                new_index, new_len, ..
            } => mark(&mut right, &new_at, new_index, new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                mark(&mut left, &old_at, old_index, old_len);
                mark(&mut right, &new_at, new_index, new_len);
            }
        }
    }
    (left, right)
}

fn push_range(ranges: &mut Vec<Range<usize>>, start: usize, len: usize) {
    match ranges.last_mut() {
        Some(last) if last.end == start => last.end = start + len,
        _ => ranges.push(start..start + len),
    }
}

impl DiffModel {
    pub(crate) fn build(left: &str, right: &str) -> Self {
        let old = lines_of(left);
        let new = lines_of(right);
        let old_refs: Vec<&str> = old.iter().map(String::as_str).collect();
        let new_refs: Vec<&str> = new.iter().map(String::as_str).collect();
        let diff = TextDiff::from_slices(&old_refs, &new_refs);
        let mut model = DiffModel::default();
        let cell = |lines: &[String], index: usize, changed: Vec<Range<usize>>| Cell {
            number: index + 1,
            text: lines[index].clone(),
            changed,
        };
        for op in diff.ops() {
            match *op {
                DiffOp::Equal {
                    old_index,
                    new_index,
                    len,
                } => {
                    for offset in 0..len {
                        model.rows.push(Row {
                            kind: RowKind::Equal,
                            left: Some(cell(&old, old_index + offset, Vec::new())),
                            right: Some(cell(&new, new_index + offset, Vec::new())),
                        });
                    }
                }
                DiffOp::Delete {
                    old_index, old_len, ..
                } => model.push_removed(&old, old_index, old_len),
                DiffOp::Insert {
                    new_index, new_len, ..
                } => model.push_added(&new, new_index, new_len),
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => {
                    let paired = old_len.min(new_len);
                    for offset in 0..paired {
                        let (left_changed, right_changed) =
                            changed_ranges(&old[old_index + offset], &new[new_index + offset]);
                        model.rows.push(Row {
                            kind: RowKind::Modified,
                            left: Some(cell(&old, old_index + offset, left_changed)),
                            right: Some(cell(&new, new_index + offset, right_changed)),
                        });
                        model.added += 1;
                        model.removed += 1;
                    }
                    model.push_removed(&old, old_index + paired, old_len - paired);
                    model.push_added(&new, new_index + paired, new_len - paired);
                }
            }
        }
        model
    }

    fn push_removed(&mut self, lines: &[String], start: usize, len: usize) {
        for (offset, line) in lines[start..start + len].iter().enumerate() {
            self.rows.push(Row {
                kind: RowKind::Removed,
                left: Some(Cell {
                    number: start + offset + 1,
                    text: line.clone(),
                    changed: Vec::new(),
                }),
                right: None,
            });
            self.removed += 1;
        }
    }

    fn push_added(&mut self, lines: &[String], start: usize, len: usize) {
        for (offset, line) in lines[start..start + len].iter().enumerate() {
            self.rows.push(Row {
                kind: RowKind::Added,
                left: None,
                right: Some(Cell {
                    number: start + offset + 1,
                    text: line.clone(),
                    changed: Vec::new(),
                }),
            });
            self.added += 1;
        }
    }

    pub(crate) fn has_changes(&self) -> bool {
        self.rows.iter().any(|row| row.kind != RowKind::Equal)
    }

    /// Indexes (in `rows`) where each group of consecutive changed rows starts.
    pub(crate) fn change_starts(&self) -> Vec<usize> {
        let mut starts = Vec::new();
        let mut inside = false;
        for (index, row) in self.rows.iter().enumerate() {
            let changed = row.kind != RowKind::Equal;
            if changed && !inside {
                starts.push(index);
            }
            inside = changed;
        }
        starts
    }

    /// The rows as drawn side by side. With `collapse`, long unchanged runs fold away, keeping
    /// [`CONTEXT`] lines next to each change.
    pub(crate) fn items(&self, collapse: bool) -> Vec<Item> {
        if !collapse {
            return self.rows.iter().cloned().map(Item::Row).collect();
        }
        let mut keep = vec![false; self.rows.len()];
        for (index, row) in self.rows.iter().enumerate() {
            if row.kind != RowKind::Equal {
                let from = index.saturating_sub(CONTEXT);
                let to = (index + CONTEXT + 1).min(self.rows.len());
                keep[from..to].iter_mut().for_each(|k| *k = true);
            }
        }
        let mut items = Vec::new();
        let mut hidden = 0;
        for (row, kept) in self.rows.iter().zip(keep) {
            if kept {
                if hidden > 0 {
                    items.push(Item::Folded(hidden));
                    hidden = 0;
                }
                items.push(Item::Row(row.clone()));
            } else {
                hidden += 1;
            }
        }
        if hidden > 0 {
            items.push(Item::Folded(hidden));
        }
        items
    }

    /// Index in the folded list of the item showing the row `row_index` of `rows`.
    pub(crate) fn item_index_of_row(&self, items: &[Item], row_index: usize) -> usize {
        let mut seen = 0usize;
        for (position, item) in items.iter().enumerate() {
            let width = match item {
                Item::Row(_) => 1,
                Item::Folded(count) => *count,
            };
            if row_index < seen + width {
                return position;
            }
            seen += width;
        }
        items.len().saturating_sub(1)
    }

    /// The rows for the inline layout: removed lines first, then the lines that replace them.
    pub(crate) fn inline_rows(&self, collapse: bool) -> Vec<InlineItem> {
        let mut out = Vec::new();
        for item in self.items(collapse) {
            match item {
                Item::Folded(count) => out.push(InlineItem::Folded(count)),
                Item::Row(row) => match row.kind {
                    RowKind::Equal => out.push(InlineItem::Line {
                        sign: ' ',
                        cell: row.right.expect("equal rows have both sides"),
                        old_number: row.left.map(|cell| cell.number),
                    }),
                    RowKind::Removed => out.push(InlineItem::Line {
                        sign: '-',
                        cell: row.left.expect("removed rows have a left side"),
                        old_number: None,
                    }),
                    RowKind::Added => out.push(InlineItem::Line {
                        sign: '+',
                        cell: row.right.expect("added rows have a right side"),
                        old_number: None,
                    }),
                    RowKind::Modified => {
                        out.push(InlineItem::Line {
                            sign: '-',
                            cell: row.left.expect("modified rows have both sides"),
                            old_number: None,
                        });
                        out.push(InlineItem::Line {
                            sign: '+',
                            cell: row.right.expect("modified rows have both sides"),
                            old_number: None,
                        });
                    }
                },
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InlineItem {
    Line {
        sign: char,
        cell: Cell,
        /// Old line number for unchanged lines (`cell.number` is the new one).
        old_number: Option<usize>,
    },
    Folded(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(model: &DiffModel) -> Vec<RowKind> {
        model.rows.iter().map(|row| row.kind).collect()
    }

    #[test]
    fn identical_texts_have_only_equal_rows_and_no_changes() {
        let model = DiffModel::build("a\nb\n", "a\nb\n");
        assert_eq!(kinds(&model), vec![RowKind::Equal, RowKind::Equal]);
        assert!(!model.has_changes());
        assert!(model.change_starts().is_empty());
    }

    #[test]
    fn an_added_line_has_only_a_right_side() {
        let model = DiffModel::build("a\nc\n", "a\nb\nc\n");
        assert_eq!(
            kinds(&model),
            vec![RowKind::Equal, RowKind::Added, RowKind::Equal]
        );
        let added = &model.rows[1];
        assert!(added.left.is_none());
        assert_eq!(added.right.as_ref().unwrap().number, 2);
        assert_eq!((model.added, model.removed), (1, 0));
    }

    #[test]
    fn a_removed_line_has_only_a_left_side() {
        let model = DiffModel::build("a\nb\nc\n", "a\nc\n");
        assert_eq!(model.rows[1].kind, RowKind::Removed);
        assert!(model.rows[1].right.is_none());
        assert_eq!(model.rows[2].right.as_ref().unwrap().number, 2);
        assert_eq!((model.added, model.removed), (0, 1));
    }

    #[test]
    fn a_replaced_line_pairs_both_sides_and_marks_the_changed_word() {
        let model = DiffModel::build("let value = 1;\n", "let total = 1;\n");
        let row = &model.rows[0];
        assert_eq!(row.kind, RowKind::Modified);
        let left = row.left.as_ref().unwrap();
        let right = row.right.as_ref().unwrap();
        let word = |cell: &Cell| -> Vec<String> {
            cell.changed
                .iter()
                .map(|r| cell.text.chars().skip(r.start).take(r.len()).collect())
                .collect()
        };
        assert_eq!(word(left), vec!["value".to_string()]);
        assert_eq!(word(right), vec!["total".to_string()]);
    }

    #[test]
    fn extra_replaced_lines_become_removed_or_added_rows() {
        let model = DiffModel::build("a\nb\nc\n", "x\n");
        assert_eq!(
            kinds(&model),
            vec![RowKind::Modified, RowKind::Removed, RowKind::Removed]
        );
        let model = DiffModel::build("x\n", "a\nb\nc\n");
        assert_eq!(
            kinds(&model),
            vec![RowKind::Modified, RowKind::Added, RowKind::Added]
        );
    }

    #[test]
    fn crlf_and_tabs_do_not_leak_into_the_text() {
        let model = DiffModel::build("a\r\n\tb\r\n", "a\r\n\tb\r\n");
        assert!(!model.has_changes());
        assert_eq!(model.rows[1].left.as_ref().unwrap().text, "    b");
    }

    #[test]
    fn an_empty_side_is_all_added_or_removed() {
        let all_added = DiffModel::build("", "a\nb\n");
        assert_eq!(kinds(&all_added), vec![RowKind::Added, RowKind::Added]);
        let all_removed = DiffModel::build("a\nb\n", "");
        assert_eq!(
            kinds(&all_removed),
            vec![RowKind::Removed, RowKind::Removed]
        );
        assert!(DiffModel::build("", "").rows.is_empty());
    }

    #[test]
    fn change_starts_mark_each_group_once() {
        let model = DiffModel::build("a\nb\nc\nd\ne\n", "a\nB\nc\nd\nE\nF\n");
        assert_eq!(model.change_starts(), vec![1, 4]);
    }

    #[test]
    fn collapsing_keeps_context_and_counts_the_hidden_rows() {
        let old: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let new = old.replace("line 10\n", "changed\n");
        let model = DiffModel::build(&old, &new);
        let items = model.items(true);
        assert!(matches!(items.first(), Some(Item::Folded(6))));
        assert!(matches!(items.last(), Some(Item::Folded(7))));
        // 3 before + the changed row + 3 after, between the two folds.
        assert_eq!(items.len(), 2 + 7);
        assert_eq!(model.items(false).len(), 20);
    }

    #[test]
    fn the_item_index_follows_folded_runs() {
        let old: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let new = old.replace("line 10\n", "changed\n");
        let model = DiffModel::build(&old, &new);
        let items = model.items(true);
        let start = model.change_starts()[0];
        let at = model.item_index_of_row(&items, start);
        assert!(matches!(&items[at], Item::Row(row) if row.kind == RowKind::Modified));
    }

    #[test]
    fn the_inline_layout_lists_removed_then_added_for_a_modified_row() {
        let model = DiffModel::build("a\nold\n", "a\nnew\n");
        let signs: Vec<char> = model
            .inline_rows(false)
            .iter()
            .map(|item| match item {
                InlineItem::Line { sign, .. } => *sign,
                InlineItem::Folded(_) => '~',
            })
            .collect();
        assert_eq!(signs, vec![' ', '-', '+']);
    }
}
