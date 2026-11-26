use super::*;
use ropey::Rope;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

/// Stateful tree-sitter highlighting that reuses and edits the previous syntax tree.
pub struct IncrementalHighlighter {
    parser: Parser,
    language: Option<Language>,
    query: Option<Query>,
    tree: Option<Tree>,
    lines: Vec<Vec<Span>>,
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
            tree: None,
            lines: Vec::new(),
        }
    }
    pub fn highlight(
        &mut self,
        language: Language,
        source: &str,
        edit: Option<TextEdit>,
    ) -> Vec<Vec<Span>> {
        self.highlight_rope(language, &Rope::from_str(source), edit)
    }

    /// Parse and query rope chunks directly, without copying the whole document.
    pub fn highlight_rope(
        &mut self,
        language: Language,
        source: &Rope,
        edit: Option<TextEdit>,
    ) -> Vec<Vec<Span>> {
        let ts_language = language.ts_language();
        if self.language != Some(language) {
            if self.parser.set_language(&ts_language).is_err() {
                return Vec::new();
            }
            self.query = Query::new(&ts_language, language.highlight_query()).ok();
            self.language = Some(language);
            self.tree = None;
        }
        let mut invalidate_from = 0usize;
        if let (Some(tree), Some(edit)) = (&mut self.tree, edit) {
            let root = tree.root_node();
            let mut node = root
                .descendant_for_byte_range(
                    edit.start_byte,
                    edit.old_end_byte.max(edit.start_byte.saturating_add(1)),
                )
                .unwrap_or(root);
            while let Some(parent) = node.parent() {
                if parent.parent().is_none() {
                    break;
                }
                if parent.is_named() {
                    node = parent
                } else {
                    break;
                }
            }
            invalidate_from = node.start_byte();
            tree.edit(&InputEdit {
                start_byte: edit.start_byte,
                old_end_byte: edit.old_end_byte,
                new_end_byte: edit.new_end_byte,
                start_position: Point::new(edit.start_position.0, edit.start_position.1),
                old_end_position: Point::new(edit.old_end_position.0, edit.old_end_position.1),
                new_end_position: Point::new(edit.new_end_position.0, edit.new_end_position.1),
            });
        } else if self.tree.is_some() {
            self.tree = None;
            self.lines.clear();
        }
        if self.tree.is_none() {
            invalidate_from = 0;
            self.lines.clear();
        }
        let base_row = source.byte_to_line(invalidate_from.min(source.len_bytes()));
        invalidate_from = source.line_to_byte(base_row);
        self.lines.truncate(base_row);
        self.lines.resize(source.len_lines(), Vec::new());
        for row in self.lines.iter_mut().skip(base_row) {
            row.clear();
        }
        let Some(tree) = self.parser.parse_with_options(
            &mut |byte, _| {
                if byte >= source.len_bytes() {
                    return &[][..];
                }
                let (chunk, start, _, _) = source.chunk_at_byte(byte);
                &chunk.as_bytes()[byte - start..]
            },
            self.tree.as_ref(),
            None,
        ) else {
            return Vec::new();
        };
        self.tree = Some(tree);
        let Some(query) = &self.query else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(invalidate_from..source.len_bytes());
        let mut captures = cursor.captures(
            query,
            self.tree.as_ref().unwrap().root_node(),
            |node: tree_sitter::Node| {
                source
                    .byte_slice(node.byte_range())
                    .chunks()
                    .map(str::as_bytes)
            },
        );
        while let Some((query_match, capture_index)) = captures.next() {
            let capture = query_match.captures()[*capture_index];
            let name = query
                .capture_names()
                .get(capture.index as usize)
                .copied()
                .unwrap_or("");
            let Some(class) = CAPTURES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c) else {
                continue;
            };
            let start = capture.node.start_byte();
            let end = capture.node.end_byte();
            let first_row = source.byte_to_line(start.min(source.len_bytes()));
            let last_row = source.byte_to_line(end.min(source.len_bytes()));
            for row in first_row..=last_row {
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
                if a < b && row >= base_row {
                    self.lines[row].push(Span {
                        range: a - line_start..b - line_start,
                        class,
                    });
                }
            }
        }
        for spans in &mut self.lines {
            spans.sort_by_key(|s| (s.range.start, s.range.end));
        }
        self.lines.clone()
    }
}
