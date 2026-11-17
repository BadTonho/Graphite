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
/// line does not open a region.
/// Fold regions an extension provided: start line to last line, 0-based and inclusive.
pub(crate) type Provided = std::collections::BTreeMap<usize, usize>;

/// Inclusive last line of the fold region starting at `start`. With `provided` regions (from an
/// extension) only those count; otherwise regions come from the indentation.
pub(crate) fn region_end(
    buffer: &Buffer,
    start: usize,
    tab_size: usize,
    provided: Option<&Provided>,
) -> Option<usize> {
    if let Some(provided) = provided {
        let last = buffer.len_lines().saturating_sub(1);
        return provided
            .get(&start)
            .map(|end| (*end).min(last))
            .filter(|end| *end > start);
    }
    if start >= buffer.len_lines() {
        return None;
    }
    let marker_end = marker_region_end(buffer, start);
    let Some(base) = indent_of(&buffer.line(start), tab_size) else {
        return marker_end;
    };
    let mut end = None;
    for line in start + 1..buffer.len_lines() {
        match indent_of(&buffer.line(line), tab_size) {
            None => continue,
            Some(indent) if indent <= base => break,
            Some(_) => end = Some(line),
        }
    }
    end.max(marker_end)
}

/// Innermost region that contains `line`, as `(start, end)` with both inclusive.
pub(crate) fn innermost_region(
    buffer: &Buffer,
    line: usize,
    tab_size: usize,
    provided: Option<&Provided>,
) -> Option<(usize, usize)> {
    let line = line.min(buffer.len_lines().saturating_sub(1));
    (0..=line).rev().find_map(|start| {
        let end = region_end(buffer, start, tab_size, provided)?;
        (end >= line).then_some((start, end))
    })
}

/// Hidden line ranges `(first, last)` inclusive for the folded start lines.
/// A fold hides the lines after its start line; its start line stays visible.
pub(crate) fn hidden_ranges(
    buffer: &Buffer,
    folded: &[usize],
    tab_size: usize,
    provided: Option<&Provided>,
) -> Vec<(usize, usize)> {
    folded
        .iter()
        .filter_map(|&start| {
            let end = region_end(buffer, start, tab_size, provided)?;
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

/// Whether a line opens or closes a marker region (`// #region`, `# region`, `<!-- #region`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Marker {
    Start,
    End,
}

/// Reads a region marker from a line of text. The marker word must follow a comment leader.
pub(crate) fn marker(line: &str) -> Option<Marker> {
    let text = line.trim();
    let text = ["//", "/*", "<!--", "--", "#", ";", "%", "'"]
        .iter()
        .find_map(|leader| text.strip_prefix(leader))?;
    let text = text.trim_start();
    let text = text.strip_prefix('#').unwrap_or(text);
    let word_end = text
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(text.len());
    match text[..word_end].to_ascii_lowercase().as_str() {
        "region" => Some(Marker::Start),
        "endregion" => Some(Marker::End),
        _ => None,
    }
}

/// Inclusive last line of the marker region that starts on `start`, if that line opens one.
fn marker_region_end(buffer: &Buffer, start: usize) -> Option<usize> {
    if marker(&buffer.line(start)) != Some(Marker::Start) {
        return None;
    }
    let mut depth = 1usize;
    for line in start + 1..buffer.len_lines() {
        match marker(&buffer.line(line)) {
            Some(Marker::Start) => depth += 1,
            Some(Marker::End) => {
                depth -= 1;
                if depth == 0 {
                    return Some(line);
                }
            }
            None => {}
        }
    }
    None
}

/// Every marker region as `(start, end)`, matching each end with the nearest open start.
fn marker_spans(buffer: &Buffer) -> Vec<(usize, usize)> {
    let mut open = Vec::new();
    let mut spans = Vec::new();
    for line in 0..buffer.len_lines() {
        match marker(&buffer.line(line)) {
            Some(Marker::Start) => open.push(line),
            Some(Marker::End) => {
                if let Some(start) = open.pop() {
                    spans.push((start, line));
                }
            }
            None => {}
        }
    }
    spans
}

/// Every indentation region as `(start, end)`, found in one pass over the lines.
fn indentation_spans(buffer: &Buffer, tab_size: usize) -> Vec<(usize, usize)> {
    fn close(
        stack: &mut Vec<(usize, usize)>,
        spans: &mut Vec<(usize, usize)>,
        previous: Option<usize>,
        indent: Option<usize>,
    ) {
        while let Some(&(base, start)) = stack.last() {
            if indent.is_some_and(|indent| indent > base) {
                break;
            }
            stack.pop();
            if let Some(end) = previous.filter(|end| *end > start) {
                spans.push((start, end));
            }
        }
    }
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut spans = Vec::new();
    let mut previous: Option<usize> = None;
    for line in 0..buffer.len_lines() {
        let Some(indent) = indent_of(&buffer.line(line), tab_size) else {
            continue;
        };
        close(&mut stack, &mut spans, previous, Some(indent));
        stack.push((indent, line));
        previous = Some(line);
    }
    close(&mut stack, &mut spans, previous, None);
    spans
}

/// A fold region and its place in the nesting tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub start: usize,
    pub end: usize,
    /// Index of the smallest region that contains this one.
    pub parent: Option<usize>,
    /// 1 for a top-level region, 2 for one inside it, and so on.
    pub level: usize,
}

impl Region {
    pub(crate) fn contains_line(&self, line: usize) -> bool {
        self.start <= line && line <= self.end
    }

    /// Whether this region lies inside `other` (or is the same region).
    pub(crate) fn within(&self, other: &Region) -> bool {
        other.start <= self.start && self.end <= other.end
    }
}

/// Every fold region of the document, each one listed before the regions inside it. Extension
/// regions replace the computed ones; otherwise indentation and marker regions are merged, with
/// the larger one kept when both start on the same line.
pub(crate) fn regions(
    buffer: &Buffer,
    tab_size: usize,
    provided: Option<&Provided>,
) -> Vec<Region> {
    let last = buffer.len_lines().saturating_sub(1);
    let mut spans: Vec<(usize, usize)> = match provided {
        Some(provided) => provided
            .iter()
            .map(|(start, end)| (*start, (*end).min(last)))
            .filter(|(start, end)| end > start)
            .collect(),
        None => {
            let mut spans = indentation_spans(buffer, tab_size);
            spans.extend(marker_spans(buffer));
            spans
        }
    };
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    spans.dedup_by_key(|span| span.0);
    let mut regions: Vec<Region> = Vec::with_capacity(spans.len());
    let mut stack: Vec<usize> = Vec::new();
    for (start, end) in spans {
        while stack.last().is_some_and(|&top| regions[top].end < end) {
            stack.pop();
        }
        let parent = stack.last().copied();
        regions.push(Region {
            start,
            end,
            parent,
            level: stack.len() + 1,
        });
        stack.push(regions.len() - 1);
    }
    regions
}

/// The innermost region containing `line`.
pub(crate) fn region_at_line(regions: &[Region], line: usize) -> Option<usize> {
    regions
        .iter()
        .enumerate()
        .rev()
        .find(|(_, region)| region.contains_line(line))
        .map(|(index, _)| index)
}

/// The regions inside `regions[index]`, at any depth.
pub(crate) fn regions_inside(regions: &[Region], index: usize) -> impl Iterator<Item = usize> + '_ {
    let end = regions[index].end;
    (index + 1..regions.len()).take_while(move |&i| regions[i].start < end)
}

/// Start line of the previous fold: the previous sibling when `line` starts a region, otherwise
/// the last region that starts above `line`.
pub(crate) fn previous_fold_line(regions: &[Region], line: usize) -> Option<usize> {
    if let Some(index) = region_at_line(regions, line).filter(|&i| regions[i].start == line) {
        let parent = regions[index].parent;
        let floor = parent.map(|p| regions[p].start);
        for candidate in (0..index).rev() {
            let region = &regions[candidate];
            if floor.is_some_and(|floor| region.start <= floor) {
                return None;
            }
            if region.parent == parent {
                return Some(region.start);
            }
        }
        return None;
    }
    regions
        .iter()
        .rev()
        .find(|region| region.start < line)
        .map(|region| region.start)
}

/// Start line of the next fold: the next sibling when `line` starts a region, otherwise the
/// first region that starts below `line`.
pub(crate) fn next_fold_line(regions: &[Region], line: usize) -> Option<usize> {
    if let Some(index) = region_at_line(regions, line).filter(|&i| regions[i].start == line) {
        let parent = regions[index].parent;
        let ceiling = match parent {
            Some(p) => regions[p].end,
            None => regions.last()?.end,
        };
        for region in &regions[index + 1..] {
            if region.start >= ceiling {
                return None;
            }
            if region.parent == parent {
                return Some(region.start);
            }
        }
        return None;
    }
    regions
        .iter()
        .find(|region| region.start > line)
        .map(|region| region.start)
}

/// Start line of the parent fold: the start of the region containing `line`, or when `line`
/// already starts it, the start of the region around that one.
pub(crate) fn parent_fold_line(regions: &[Region], line: usize) -> Option<usize> {
    let index = region_at_line(regions, line)?;
    let region = &regions[index];
    if line != region.start {
        return Some(region.start);
    }
    region.parent.map(|parent| regions[parent].start)
}

/// The first run of at least two `import`-style lines, as `(first, last)`.
pub(crate) fn import_block(buffer: &Buffer) -> Option<(usize, usize)> {
    fn is_import(line: &str) -> bool {
        let text = line.trim_start();
        [
            "import ",
            "use ",
            "using ",
            "#include",
            "extern crate ",
            "require ",
        ]
        .iter()
        .any(|prefix| text.starts_with(prefix))
            || (text.starts_with("from ") && text.contains(" import "))
    }
    let mut run: Option<(usize, usize)> = None;
    for line in 0..buffer.len_lines() {
        let text = buffer.line(line);
        if is_import(&text) {
            run = Some(match run {
                Some((first, _)) => (first, line),
                None => (line, line),
            });
        } else if let Some((first, last)) = run {
            if text.trim().is_empty() {
                continue;
            }
            return (last > first).then_some((first, last));
        }
    }
    run.filter(|(first, last)| last > first)
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
        assert_eq!(region_end(&b, 0, 4, None), Some(4));
        assert_eq!(region_end(&b, 2, 4, None), Some(3));
    }

    #[test]
    fn lines_without_deeper_indentation_open_no_region() {
        let b = buffer(SAMPLE);
        assert_eq!(region_end(&b, 1, 4, None), None);
        assert_eq!(region_end(&b, 5, 4, None), None);
        assert_eq!(region_end(&b, 99, 4, None), None);
    }

    #[test]
    fn provided_regions_replace_the_indentation_rules() {
        let b = buffer("a\nb\nc\nd\n    e\nf\n");
        let provided: Provided = [(0, 2), (3, 99), (1, 1)].into_iter().collect();
        assert_eq!(region_end(&b, 0, 4, Some(&provided)), Some(2));
        // An end past the document is clamped to its last line.
        assert_eq!(region_end(&b, 3, 4, Some(&provided)), Some(6));
        // A region that does not span two lines is not a region.
        assert_eq!(region_end(&b, 1, 4, Some(&provided)), None);
        // The indented line that indentation would fold is ignored once regions are provided.
        assert_eq!(region_end(&b, 3, 4, None), Some(4));
        assert_eq!(region_end(&b, 4, 4, Some(&provided)), None);
        assert_eq!(innermost_region(&b, 1, 4, Some(&provided)), Some((0, 2)));
        assert_eq!(
            hidden_ranges(&b, &[0, 3], 4, Some(&provided)),
            vec![(1, 2), (4, 6)]
        );
    }

    #[test]
    fn blank_lines_inside_a_region_do_not_end_it_and_trailing_blanks_are_excluded() {
        let b = buffer("def f():\n    x = 1\n\n    y = 2\n\nz = 3\n");
        assert_eq!(region_end(&b, 0, 4, None), Some(3));
    }

    #[test]
    fn tabs_count_as_tab_size_columns() {
        let b = buffer("if x:\n\tpass\n    pass\n");
        // With a tab size of 4, the tab-indented body line measures 4 columns, deeper
        // than the base line at 0, so both body lines belong to the region.
        assert_eq!(region_end(&b, 0, 4, None), Some(2));
    }

    #[test]
    fn innermost_region_prefers_the_nearest_enclosing_block() {
        let b = buffer(SAMPLE);
        assert_eq!(innermost_region(&b, 3, 4, None), Some((2, 3)));
        assert_eq!(innermost_region(&b, 1, 4, None), Some((0, 4)));
        assert_eq!(innermost_region(&b, 5, 4, None), None);
    }

    #[test]
    fn hidden_ranges_skip_folds_that_do_not_open_a_region() {
        let b = buffer(SAMPLE);
        assert_eq!(hidden_ranges(&b, &[0, 1], 4, None), vec![(1, 4)]);
    }

    #[test]
    fn visible_lines_skip_folded_bodies_and_keep_the_start_line() {
        let b = buffer(SAMPLE);
        let hidden = hidden_ranges(&b, &[2], 4, None);
        // Line 6 is the empty line after the sample's final newline.
        assert_eq!(visible_lines(&b, &hidden, 0, 10), vec![0, 1, 2, 4, 5, 6]);
    }

    #[test]
    fn visible_lines_start_from_the_fold_owner_when_scroll_lands_inside_a_fold() {
        let b = buffer(SAMPLE);
        let hidden = hidden_ranges(&b, &[0], 4, None);
        assert_eq!(visible_owner(&hidden, 3), 0);
        assert_eq!(visible_lines(&b, &hidden, 3, 3), vec![0, 5, 6]);
    }

    #[test]
    fn nested_folds_hide_through_the_outermost_fold() {
        let b = buffer(SAMPLE);
        let hidden = hidden_ranges(&b, &[0, 2], 4, None);
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
