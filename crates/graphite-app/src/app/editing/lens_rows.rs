//! Code lenses above their line. A line that has a code lens gets a row of its own above it in
//! the editor body, holding the lens text; the rows after it move down by one. With word wrap
//! on, lenses stay at the end of their line.

use super::*;

/// A row of the editor body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodyRow {
    /// The lenses of a line, drawn above it.
    Lens(usize),
    Line(usize),
}

impl BodyRow {
    /// The document line the row belongs to.
    pub(crate) fn line(self) -> usize {
        match self {
            Self::Lens(line) | Self::Line(line) => line,
        }
    }
}

/// The rows for `lines`, in order, cut to `capacity` rows. A lens row is dropped with its line
/// when the line does not fit.
pub(crate) fn body_rows(
    lines: &[usize],
    capacity: usize,
    has_lens: impl Fn(usize) -> bool,
) -> Vec<BodyRow> {
    let mut rows = Vec::with_capacity(lines.len().min(capacity) + 1);
    for &line in lines {
        let needed = 1 + usize::from(has_lens(line));
        if rows.len() + needed > capacity {
            break;
        }
        if needed == 2 {
            rows.push(BodyRow::Lens(line));
        }
        rows.push(BodyRow::Line(line));
    }
    rows
}

impl App {
    /// The code lens text of `line` of the active document, when lenses are drawn above lines.
    pub(crate) fn lens_text_above(&self, uri: &str, line: usize) -> Option<String> {
        if self.word_wrap {
            return None;
        }
        self.code_lens_text(uri, line)
    }

    /// The URI of the active document for lens lookups.
    fn lens_uri(&self) -> Option<String> {
        self.plugins.vsix.as_ref()?;
        self.decoration_uri(self.documents[self.active_document()].buffer.path()?)
    }

    /// The rows of the viewport of the active document: `lines` with their lens rows.
    pub(crate) fn body_rows_for_active(&self, lines: &[usize], capacity: usize) -> Vec<BodyRow> {
        let uri = self.lens_uri();
        body_rows(lines, capacity, |line| {
            uri.as_deref()
                .is_some_and(|uri| self.lens_text_above(uri, line).is_some())
        })
    }

    /// How many lens rows the first `visible` rows of the active view hold.
    pub(crate) fn lens_rows_in_view(&self, visible: usize) -> usize {
        let Some(uri) = self.lens_uri() else {
            return 0;
        };
        let document = self.active_document();
        let tab = &self.documents[document];
        let view = self.editor_groups.active_view();
        let hidden = super::folding::hidden_ranges(
            &tab.buffer,
            super::fold_commands::effective_folds(view, tab.buffer.revision()),
            tab.indentation.tab_size,
            super::fold_commands::region_source(view, tab),
        );
        super::folding::visible_lines(&tab.buffer, &hidden, view.scroll_line, visible)
            .into_iter()
            .filter(|line| self.lens_text_above(&uri, *line).is_some())
            .count()
    }

    /// How many lens rows lie between the first visible line of the active view and `line`
    /// (including the one above `line`).
    pub(crate) fn lens_rows_before(&self, line: usize) -> usize {
        let Some(uri) = self.lens_uri() else {
            return 0;
        };
        let first = self.editor_groups.active_view().scroll_line;
        (first..=line)
            .take(2_000)
            .filter(|candidate| self.lens_text_above(&uri, *candidate).is_some())
            .count()
    }

    /// The line whose lens row is the `row`-th row of the active view, if it is a lens row.
    pub(crate) fn editor_row_lens_line(&self, row: usize) -> Option<usize> {
        match self.editor_body_row(row)? {
            BodyRow::Lens(line) => Some(line),
            BodyRow::Line(_) => None,
        }
    }

    /// The `row`-th row of the active view's body.
    pub(crate) fn editor_body_row(&self, row: usize) -> Option<BodyRow> {
        let document = self.active_document();
        let tab = &self.documents[document];
        let view = self.editor_groups.active_view();
        let hidden = super::folding::hidden_ranges(
            &tab.buffer,
            super::fold_commands::effective_folds(view, tab.buffer.revision()),
            tab.indentation.tab_size,
            super::fold_commands::region_source(view, tab),
        );
        // Each line takes at least one row, so `row + 1` lines are enough to reach the row.
        let lines = super::folding::visible_lines(&tab.buffer, &hidden, view.scroll_line, row + 1);
        // Lens rows take room too: twice the rows is room for every line with its lens.
        self.body_rows_for_active(&lines, 2 * (row + 1)).get(row).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_with_a_lens_get_a_row_above_them() {
        let rows = body_rows(&[3, 4, 5], 10, |line| line == 4);
        assert_eq!(
            rows,
            [BodyRow::Line(3), BodyRow::Lens(4), BodyRow::Line(4), BodyRow::Line(5)]
        );
        assert_eq!(rows[1].line(), 4);
        assert_eq!(body_rows(&[0, 1], 10, |_| false), [BodyRow::Line(0), BodyRow::Line(1)]);
        assert!(body_rows(&[], 10, |_| true).is_empty());
    }

    #[test]
    fn rows_are_cut_to_the_capacity_without_separating_a_lens_from_its_line() {
        let all = |_| true;
        assert_eq!(body_rows(&[1, 2], 3, all), [BodyRow::Lens(1), BodyRow::Line(1)], "the lens row of 2 would be alone");
        assert_eq!(body_rows(&[1, 2], 4, all).len(), 4);
        assert_eq!(body_rows(&[1, 2], 1, all), [], "not even one line with its lens fits");
        assert_eq!(body_rows(&[1, 2, 3], 2, |_| false), [BodyRow::Line(1), BodyRow::Line(2)]);
        assert!(body_rows(&[1], 0, |_| false).is_empty());
    }
}
