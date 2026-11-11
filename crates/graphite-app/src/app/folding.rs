//! Indentation-based code folding.
//!
//! A line starts a fold region when the next non-blank line is indented deeper than it.
//! The region runs through every following line that is blank or indented deeper, and
//! ends at its last non-blank line. Regions are computed on demand from the buffer, so
//! nothing is cached per document.

use graphite_text::Buffer;

/// Display width of a line's leading whitespace, or `None` for blank lines.
fn indent_of(line: &str, tab_size: usize) -> Option<usize> {
    let mut width = 0;
    for ch in line.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += tab_size.max(1),
            '\r' | '\n' => {}
            _ if ch.is_whitespace() => {}
            _ => return Some(width),
        }
    }
    None
}

/// Inclusive last line of the fold region starting at `start`, or `None` when the
/// line does not open a region.
pub(crate) fn region_end(buffer: &Buffer, start: usize, tab_size: usize) -> Option<usize> {
    if start >= buffer.len_lines() {
        return None;
    }
    let base = indent_of(&buffer.line(start), tab_size)?;
    let mut end = None;
    for line in start + 1..buffer.len_lines() {
        match indent_of(&buffer.line(line), tab_size) {
            None => continue,
            Some(indent) if indent <= base => break,
            Some(_) => end = Some(line),
        }
    }
    end
}

/// Innermost region that contains `line`, as `(start, end)` with both inclusive.
pub(crate) fn innermost_region(
    buffer: &Buffer,
    line: usize,
    tab_size: usize,
) -> Option<(usize, usize)> {
    let line = line.min(buffer.len_lines().saturating_sub(1));
    (0..=line).rev().find_map(|start| {
        let end = region_end(buffer, start, tab_size)?;
        (end >= line).then_some((start, end))
    })
}

/// Hidden line ranges `(first, last)` inclusive for the folded start lines.
/// A fold hides the lines after its start line; its start line stays visible.
pub(crate) fn hidden_ranges(
    buffer: &Buffer,
    folded: &[usize],
    tab_size: usize,
) -> Vec<(usize, usize)> {
    folded
        .iter()
        .filter_map(|&start| {
            let end = region_end(buffer, start, tab_size)?;
            Some((start + 1, end))
        })
        .collect()
}

/// The visible line that owns `line`: if `line` is hidden by a fold, the start of the
/// outermost fold hiding it; otherwise `line` itself.
pub(crate) fn visible_owner(hidden: &[(usize, usize)], line: usize) -> usize {
    hidden
        .iter()
        .filter(|(first, last)| (*first..=*last).contains(&line))
        .map(|(first, _)| first - 1)
        .min()
        .unwrap_or(line)
}

/// The next `count` visible lines starting at `from`, skipping hidden lines.
pub(crate) fn visible_lines(
    buffer: &Buffer,
    hidden: &[(usize, usize)],
    from: usize,
    count: usize,
) -> Vec<usize> {
    let total = buffer.len_lines();
    let mut lines = Vec::with_capacity(count.min(total));
    let mut line = visible_owner(hidden, from);
    while lines.len() < count && line < total {
        lines.push(line);
        line += 1;
        while let Some((_, last)) = hidden
            .iter()
            .find(|(first, last)| (*first..=*last).contains(&line))
        {
            line = last + 1;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(text: &str) -> Buffer {
        Buffer::from_text("sample.rs", text)
    }

    const SAMPLE: &str =
        "fn main() {\n    let a = 1;\n    if a > 0 {\n        println!(\"x\");\n    }\n}\n";

    #[test]
    fn region_spans_the_block_body_and_ends_before_the_dedent() {
        let b = buffer(SAMPLE);
        assert_eq!(region_end(&b, 0, 4), Some(4));
        assert_eq!(region_end(&b, 2, 4), Some(3));
    }

    #[test]
    fn lines_without_deeper_indentation_open_no_region() {
        let b = buffer(SAMPLE);
        assert_eq!(region_end(&b, 1, 4), None);
        assert_eq!(region_end(&b, 5, 4), None);
        assert_eq!(region_end(&b, 99, 4), None);
    }

    #[test]
    fn blank_lines_inside_a_region_do_not_end_it_and_trailing_blanks_are_excluded() {
        let b = buffer("def f():\n    x = 1\n\n    y = 2\n\nz = 3\n");
        assert_eq!(region_end(&b, 0, 4), Some(3));
    }

    #[test]
    fn tabs_count_as_tab_size_columns() {
        let b = buffer("if x:\n\tpass\n    pass\n");
        // With a tab size of 4, the tab-indented body line measures 4 columns, deeper
        // than the base line at 0, so both body lines belong to the region.
        assert_eq!(region_end(&b, 0, 4), Some(2));
    }

    #[test]
    fn innermost_region_prefers_the_nearest_enclosing_block() {
        let b = buffer(SAMPLE);
        assert_eq!(innermost_region(&b, 3, 4), Some((2, 3)));
        assert_eq!(innermost_region(&b, 1, 4), Some((0, 4)));
        assert_eq!(innermost_region(&b, 5, 4), None);
    }

    #[test]
    fn hidden_ranges_skip_folds_that_do_not_open_a_region() {
        let b = buffer(SAMPLE);
        assert_eq!(hidden_ranges(&b, &[0, 1], 4), vec![(1, 4)]);
    }

    #[test]
    fn visible_lines_skip_folded_bodies_and_keep_the_start_line() {
        let b = buffer(SAMPLE);
        let hidden = hidden_ranges(&b, &[2], 4);
        // Line 6 is the empty line after the sample's final newline.
        assert_eq!(visible_lines(&b, &hidden, 0, 10), vec![0, 1, 2, 4, 5, 6]);
    }

    #[test]
    fn visible_lines_start_from_the_fold_owner_when_scroll_lands_inside_a_fold() {
        let b = buffer(SAMPLE);
        let hidden = hidden_ranges(&b, &[0], 4);
        assert_eq!(visible_owner(&hidden, 3), 0);
        assert_eq!(visible_lines(&b, &hidden, 3, 3), vec![0, 5, 6]);
    }

    #[test]
    fn nested_folds_hide_through_the_outermost_fold() {
        let b = buffer(SAMPLE);
        let hidden = hidden_ranges(&b, &[0, 2], 4);
        assert_eq!(visible_owner(&hidden, 3), 0);
        assert_eq!(visible_lines(&b, &hidden, 0, 10), vec![0, 5, 6]);
    }

    #[test]
    fn visible_lines_stop_at_the_end_of_the_document() {
        let b = buffer(SAMPLE);
        let hidden: Vec<(usize, usize)> = Vec::new();
        assert_eq!(visible_lines(&b, &hidden, 4, 10), vec![4, 5, 6]);
    }
}
