use super::*;
use ropey::Rope;
use std::sync::Arc;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

/// Stateful tree-sitter highlighting that reuses and edits the previous syntax tree.
pub struct IncrementalHighlighter {
    parser: Parser,
    language: Option<Language>,
    query: Option<Query>,
    /// The highlight class of each capture of `query`, by capture index.
    classes: Vec<Option<Class>>,
    tree: Option<Tree>,
    lines: Arc<Vec<Vec<Span>>>,
}
impl Default for IncrementalHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
impl IncrementalHighlighter {
    /// Syntax-node byte ranges from the innermost named node to the root.
    pub fn selection_ranges(&self, start: usize, end: usize) -> Vec<std::ops::Range<usize>> {
        let Some(tree) = &self.tree else {
            return Vec::new();
        };
        let root = tree.root_node();
        if start > root.end_byte() || end > root.end_byte() {
            return Vec::new();
        }
        let mut node = root.descendant_for_byte_range(start, end).unwrap_or(root);
        let mut result = Vec::new();
        loop {
            let range = node.byte_range();
            if node.is_named()
                && range.start <= start
                && range.end >= end
                && result.last() != Some(&range)
            {
                result.push(range);
            }
            let Some(parent) = node.parent() else { break };
            node = parent;
        }
        result
    }

    /// The declarations that enclose `line`, outermost first, from the current syntax tree.
    /// `source` must be the text last highlighted; `None` when nothing was highlighted yet.
    pub fn enclosing_scopes(&self, source: &Rope, line: usize) -> Option<Vec<ScopeHeader>> {
        let tree = self.tree.as_ref()?;
        Some(crate::enclosing_scopes_in(tree, line, |row| {
            source.get_line(row).map(|text| text.to_string())
        }))
    }

    /// Every declaration in source order, from the current syntax tree. `source` must be the
    /// text last highlighted; `None` when nothing was highlighted yet.
    pub fn declarations(&self, source: &Rope) -> Option<Vec<ScopeHeader>> {
        let tree = self.tree.as_ref()?;
        Some(crate::declarations_in(tree, |row| {
            source.get_line(row).map(|text| text.to_string())
        }))
    }

    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            language: None,
            query: None,
            classes: Vec::new(),
            tree: None,
            lines: Arc::default(),
        }
    }
    pub fn highlight(
        &mut self,
        language: Language,
        source: &str,
        edit: Option<TextEdit>,
    ) -> Arc<Vec<Vec<Span>>> {
        self.highlight_rope(language, &Rope::from_str(source), edit)
    }

    /// Parse and query rope chunks directly, without copying the whole document. `edit` is the
    /// one change since the last call, or `None` to highlight from scratch.
    pub fn highlight_rope(
        &mut self,
        language: Language,
        source: &Rope,
        edit: Option<TextEdit>,
    ) -> Arc<Vec<Vec<Span>>> {
        self.highlight_rope_edits(language, source, edit.as_ref().map(std::slice::from_ref))
    }

    /// Like [`Self::highlight_rope`] for any number of changes since the last call, oldest
    /// first. Only the lines the changes touch, and those whose syntax they changed, are queried
    /// again; the other lines keep their spans and only move. The spans are shared: a caller that
    /// drops its copy before the next call lets the update happen in place.
    pub fn highlight_rope_edits(
        &mut self,
        language: Language,
        source: &Rope,
        edits: Option<&[TextEdit]>,
    ) -> Arc<Vec<Vec<Span>>> {
        let ts_language = language.ts_language();
        if self.language != Some(language) {
            if self.parser.set_language(&ts_language).is_err() {
                return Arc::default();
            }
            self.query = Query::new(&ts_language, language.highlight_query()).ok();
            self.classes = self
                .query
                .as_ref()
                .map(|query| {
                    query
                        .capture_names()
                        .iter()
                        .map(|name| {
                            CAPTURES
                                .iter()
                                .find(|(capture, _)| capture == name)
                                .map(|(_, class)| *class)
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.language = Some(language);
            self.tree = None;
        }
        let line_count = source.len_lines();
        let lines = Arc::make_mut(&mut self.lines);
        // Rows to query again, in the coordinates of the new text.
        let mut dirty: Vec<std::ops::Range<usize>> = Vec::new();
        let old_tree = match (self.tree.take(), edits) {
            (Some(mut tree), Some(edits)) => {
                for edit in edits {
                    tree.edit(&InputEdit {
                        start_byte: edit.start_byte,
                        old_end_byte: edit.old_end_byte,
                        new_end_byte: edit.new_end_byte,
                        start_position: Point::new(edit.start_position.0, edit.start_position.1),
                        old_end_position: Point::new(
                            edit.old_end_position.0,
                            edit.old_end_position.1,
                        ),
                        new_end_position: Point::new(
                            edit.new_end_position.0,
                            edit.new_end_position.1,
                        ),
                    });
                    let start = edit.start_position.0;
                    let old_end = edit.old_end_position.0;
                    let new_end = edit.new_end_position.0;
                    if start < lines.len() {
                        let removed = old_end.min(lines.len() - 1) + 1;
                        lines.splice(start..removed, (start..=new_end).map(|_| Vec::new()));
                    }
                    // Earlier dirty rows after this change move with it.
                    let shift = new_end as isize - old_end as isize;
                    for range in &mut dirty {
                        if range.start > old_end {
                            range.start = (range.start as isize + shift) as usize;
                            range.end = (range.end as isize + shift) as usize;
                        } else if range.end > old_end {
                            range.end =
                                (range.end as isize + shift).max(range.start as isize) as usize;
                        }
                    }
                    dirty.push(start..new_end + 1);
                    // The old top-level items around the change, whose lines may no longer
                    // belong to them (a block comment that now ends earlier, for example).
                    dirty.extend(top_level_rows(&tree, edit.start_byte, edit.new_end_byte));
                }
                Some(tree)
            }
            _ => None,
        };
        if old_tree.is_none() || lines.len() != line_count {
            // From scratch: no earlier tree, unknown changes, or rows that no longer line up.
            lines.clear();
            lines.resize(line_count, Vec::new());
            dirty.clear();
            dirty.push(0..line_count);
        }
        let Some(tree) = self.parser.parse_with_options(
            &mut |byte, _| {
                if byte >= source.len_bytes() {
                    return &[][..];
                }
                let (chunk, start, _, _) = source.chunk_at_byte(byte);
                &chunk.as_bytes()[byte - start..]
            },
            old_tree.as_ref(),
            None,
        ) else {
            return Arc::default();
        };
        if let Some(old_tree) = &old_tree {
            // Nodes whose structure changed, such as everything after a new string quote.
            for range in old_tree.changed_ranges(&tree) {
                dirty.push(range.start_point.row..range.end_point.row + 1);
            }
            // The new top-level items around every changed line.
            let changed: Vec<_> = merge_ranges(std::mem::take(&mut dirty), line_count);
            for rows in &changed {
                let start = source.line_to_byte(rows.start);
                let end = source.line_to_byte(rows.end.min(line_count));
                dirty.extend(top_level_rows(&tree, start, end));
            }
            dirty.extend(changed);
            if tree.root_node().has_error() {
                // Error recovery can regroup text far from a change (an unclosed `/*` pairing with
                // a later `*/`) without reporting it as changed, so everything after the first
                // changed line is queried again, as a full pass would.
                if let Some(first) = dirty.iter().map(|rows| rows.start).min() {
                    dirty.push(first..line_count);
                }
            }
        }
        self.tree = Some(tree);
        let Some(query) = &self.query else {
            return self.lines.clone();
        };
        let tree = self.tree.as_ref().expect("parsed above");
        for rows in merge_ranges(dirty, line_count) {
            for row in rows.clone() {
                lines[row].clear();
            }
            let start_byte = source.line_to_byte(rows.start);
            let end_byte = source.line_to_byte(rows.end);
            let mut cursor = QueryCursor::new();
            cursor.set_byte_range(start_byte..end_byte.max(start_byte));
            let mut captures =
                cursor.captures(query, tree.root_node(), |node: tree_sitter::Node| {
                    source
                        .byte_slice(node.byte_range())
                        .chunks()
                        .map(str::as_bytes)
                });
            while let Some((query_match, capture_index)) = captures.next() {
                let capture = query_match.captures()[*capture_index];
                let Some(class) = self.classes.get(capture.index as usize).copied().flatten()
                else {
                    continue;
                };
                let start = capture.node.start_byte();
                let end = capture.node.end_byte();
                let first_row = source
                    .byte_to_line(start.min(source.len_bytes()))
                    .max(rows.start);
                let last_row = source
                    .byte_to_line(end.min(source.len_bytes()))
                    .min(rows.end - 1);
                let touched = lines
                    .iter_mut()
                    .enumerate()
                    .take(last_row + 1)
                    .skip(first_row);
                for (row, spans) in touched {
                    let line = source.line(row);
                    let line_start = source.line_to_byte(row);
                    let mut content_end = line_start + line.len_bytes();
                    if line.len_chars() > 0 && line.char(line.len_chars() - 1) == '\n' {
                        content_end -= 1;
                    }
                    if line.len_chars() > 1 && line.char(line.len_chars() - 2) == '\r' {
                        content_end -= 1;
                    }
                    let a = start.max(line_start);
                    let b = end.min(content_end);
                    if a < b {
                        spans.push(Span {
                            range: a - line_start..b - line_start,
                            class,
                        });
                    }
                }
            }
            for row in rows {
                lines[row].sort_by_key(|span| (span.range.start, span.range.end));
            }
        }
        self.lines.clone()
    }
}

/// The rows of the top-level items of `tree` that overlap `start..=end` (bytes).
fn top_level_rows(tree: &Tree, start: usize, end: usize) -> Option<std::ops::Range<usize>> {
    let root = tree.root_node();
    let mut node = root.first_child_for_byte(start).or_else(|| {
        // Past the last item: the last one may still have grown into the change.
        root.child(root.child_count().checked_sub(1)?)
    })?;
    // The item just before may end where the change begins.
    if let Some(previous) = node.prev_sibling() {
        if previous.end_byte() >= start {
            node = previous;
        }
    }
    // Text before the first item or after the last one belongs to no node (error recovery can
    // leave such text), so the first and last items reach the start and end of the file.
    let first = if node.prev_sibling().is_some() {
        node.start_position().row
    } else {
        0
    };
    let mut last = node.end_position().row;
    loop {
        let Some(next) = node.next_sibling() else {
            last = usize::MAX - 1;
            break;
        };
        if next.start_byte() > end {
            break;
        }
        last = next.end_position().row;
        node = next;
    }
    Some(first..last + 1)
}

/// `ranges` clipped to `0..limit`, sorted, with overlapping or touching ranges joined.
fn merge_ranges(
    mut ranges: Vec<std::ops::Range<usize>>,
    limit: usize,
) -> Vec<std::ops::Range<usize>> {
    ranges.retain_mut(|range| {
        range.end = range.end.min(limit);
        range.start < range.end
    });
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<std::ops::Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so a failure reproduces.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self, bound: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 33) as usize) % bound.max(1)
        }
    }

    /// Applies one random replacement to `rope` and returns the edit that describes it.
    fn random_edit(rope: &mut Rope, rng: &mut Lcg) -> TextEdit {
        const PIECES: &[&str] = &[
            "",
            "x",
            "\n",
            "\"",
            "/*",
            "*/",
            "fn f() {}\n",
            "let s = \"é🌎\";\n",
            "// c\n",
            "}",
            "{\n",
            "'a'",
            "\r\n",
        ];
        let len = rope.len_chars();
        let start = rng.next(len + 1);
        let end = (start + rng.next(8)).min(len);
        let inserted = PIECES[rng.next(PIECES.len())];
        let position = |rope: &Rope, char_index: usize| {
            let byte = rope.char_to_byte(char_index);
            let row = rope.byte_to_line(byte);
            (byte, (row, byte - rope.line_to_byte(row)))
        };
        let (start_byte, start_position) = position(rope, start);
        let (old_end_byte, old_end_position) = position(rope, end);
        rope.remove(start..end);
        rope.insert(start, inserted);
        let (new_end_byte, new_end_position) = position(rope, start + inserted.chars().count());
        TextEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        }
    }

    #[test]
    fn batches_of_random_edits_highlight_like_a_fresh_parse() {
        let mut rope = Rope::from_str(
            &"fn main() {\n    let value = \"text\"; // note\n    /* block */ call(1, 'c');\n}\n"
                .repeat(40),
        );
        let mut rng = Lcg(0x5eed);
        let mut incremental = IncrementalHighlighter::new();
        incremental.highlight_rope(Language::Rust, &rope, None);
        for round in 0..120 {
            let edits: Vec<_> = (0..1 + rng.next(4))
                .map(|_| random_edit(&mut rope, &mut rng))
                .collect();
            let actual = incremental.highlight_rope_edits(Language::Rust, &rope, Some(&edits));
            let expected =
                IncrementalHighlighter::new().highlight_rope(Language::Rust, &rope, None);
            assert_eq!(actual.len(), rope.len_lines(), "round {round}");
            assert!(
                *actual == *expected,
                "round {round}: incremental highlighting differs from a fresh parse"
            );
        }
    }

    #[test]
    fn edits_that_keep_the_code_valid_highlight_like_a_fresh_parse() {
        const ITEMS: &[&str] = &[
            "fn f() { let s = \"x\"; }\n",
            "/* a\n   multi-line\n   comment */\n",
            "// line comment\n",
            "struct S { field: u8 }\n",
            "const C: &str = \"é🌎\";\n",
            "\n",
        ];
        let mut rope = Rope::from_str(&ITEMS.concat().repeat(10));
        let mut rng = Lcg(0xbeef);
        let mut incremental = IncrementalHighlighter::new();
        incremental.highlight_rope(Language::Rust, &rope, None);
        for round in 0..150 {
            // Whole items are inserted at, or removed from, the start of a line.
            let lines = rope.len_lines() - 1;
            let row = rng.next(lines);
            let start = rope.line_to_char(row);
            let removing = rng.next(3) == 0 && lines > 20;
            let (end, inserted) = if removing {
                (rope.line_to_char(row + 1), "")
            } else {
                (start, ITEMS[rng.next(ITEMS.len())])
            };
            let removed = rope.slice(start..end).to_string();
            if removing && (removed.contains("/*") || removed.contains("*/")) {
                continue;
            }
            let start_byte = rope.char_to_byte(start);
            let old_end_byte = rope.char_to_byte(end);
            let old_end_row = rope.char_to_line(end);
            rope.remove(start..end);
            rope.insert(start, inserted);
            let new_end_byte = start_byte + inserted.len();
            let new_end_row = rope.byte_to_line(new_end_byte);
            // Both ends of the replaced text are at the start of a line.
            let edit = TextEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position: (row, 0),
                old_end_position: (old_end_row, 0),
                new_end_position: (new_end_row, new_end_byte - rope.line_to_byte(new_end_row)),
            };
            let actual = incremental.highlight_rope_edits(Language::Rust, &rope, Some(&[edit]));
            assert!(
                !incremental.tree.as_ref().unwrap().root_node().has_error(),
                "round {round}: the edit kept the code valid"
            );
            let expected =
                IncrementalHighlighter::new().highlight_rope(Language::Rust, &rope, None);
            assert!(
                *actual == *expected,
                "round {round}: differs from a fresh parse"
            );
        }
    }

    #[test]
    fn spans_are_updated_in_place_when_the_caller_drops_its_copy() {
        let mut rope = Rope::from_str("fn a() {}\n");
        let mut highlighter = IncrementalHighlighter::new();
        let first = highlighter.highlight_rope(Language::Rust, &rope, None);
        let address = Arc::as_ptr(&first);
        drop(first);
        rope.insert(0, "// x\n");
        let second = highlighter.highlight_rope(
            Language::Rust,
            &rope,
            Some(TextEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: 5,
                start_position: (0, 0),
                old_end_position: (0, 0),
                new_end_position: (1, 0),
            }),
        );
        assert_eq!(Arc::as_ptr(&second), address, "no copy of every line");
        assert_eq!(second.len(), 3);
    }

    #[test]
    fn overlapping_and_touching_row_ranges_are_merged() {
        assert_eq!(
            merge_ranges(vec![5..8, 0..2, 2..3, 7..12, 20..30], 25),
            vec![0..3, 5..12, 20..25]
        );
        assert!(merge_ranges(vec![4..4, 30..40], 10).is_empty());
    }
}
