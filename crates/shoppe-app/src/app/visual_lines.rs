//! Mapping between document character offsets and wrapped visual rows.

use super::*;
use crate::config::WrapIndent;
use graphite_text::Buffer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct VisualSegment {
    pub start: usize,
    pub end: usize,
    pub indent: usize,
    pub width: usize,
}

fn cell_width(ch: char, tab_size: usize) -> usize {
    if ch == '\t' {
        tab_size.max(1)
    } else {
        1
    }
}

/// Split one logical line without modifying it. Character offsets exclude the line ending.
pub(super) fn wrap_line(
    line: &str,
    columns: usize,
    tab_size: usize,
    indent: WrapIndent,
) -> Vec<VisualSegment> {
    let text = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line);
    let chars: Vec<_> = text.chars().collect();
    let columns = columns.max(1);
    let leading: usize = chars
        .iter()
        .take_while(|ch| **ch == ' ' || **ch == '\t')
        .map(|ch| cell_width(*ch, tab_size))
        .sum();
    let continuation = match indent {
        WrapIndent::None => 0,
        WrapIndent::Same => leading,
        WrapIndent::Indent => leading.saturating_add(tab_size),
        WrapIndent::DeepIndent => leading.saturating_add(tab_size.saturating_mul(2)),
    }
    .min(columns / 2);
    if chars.is_empty() {
        return vec![VisualSegment {
            start: 0,
            end: 0,
            indent: 0,
            width: 0,
        }];
    }
    let mut segments = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let padding = if start == 0 { 0 } else { continuation };
        let capacity = columns.saturating_sub(padding).max(1);
        let mut end = start;
        let mut width = 0;
        let mut last_break = None;
        while end < chars.len() {
            let next = cell_width(chars[end], tab_size);
            if width + next > capacity && end > start {
                break;
            }
            width += next;
            end += 1;
            if chars[end - 1].is_whitespace() {
                last_break = Some((end, width));
            }
            if width >= capacity {
                break;
            }
        }
        if end < chars.len() {
            if let Some((at, at_width)) = last_break.filter(|(at, _)| *at > start) {
                end = at;
                width = at_width;
            }
        }
        segments.push(VisualSegment {
            start,
            end,
            indent: padding,
            width,
        });
        start = end;
    }
    segments
}

/// Map a document character column to a visual row and display column.
pub(super) fn position_to_visual(
    line: &str,
    segments: &[VisualSegment],
    position: usize,
    tab_size: usize,
) -> (usize, usize) {
    let text = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line);
    let chars: Vec<_> = text.chars().collect();
    let position = position.min(chars.len());
    let row = segments
        .iter()
        .position(|segment| position < segment.end)
        .unwrap_or(segments.len() - 1);
    let segment = &segments[row];
    let column = segment.indent
        + chars[segment.start..position.max(segment.start)]
            .iter()
            .map(|ch| cell_width(*ch, tab_size))
            .sum::<usize>();
    (row, column)
}

/// Map a visual row and display column to the nearest character boundary.
pub(super) fn visual_to_position(
    line: &str,
    segments: &[VisualSegment],
    row: usize,
    column: usize,
    tab_size: usize,
) -> usize {
    let text = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line);
    let chars: Vec<_> = text.chars().collect();
    let segment = &segments[row.min(segments.len() - 1)];
    let mut x = segment.indent;
    for (offset, ch) in chars[segment.start..segment.end].iter().enumerate() {
        let width = cell_width(*ch, tab_size);
        if column < x + (width + 1) / 2 {
            return segment.start + offset;
        }
        x += width;
    }
    segment.end
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct VisualRow {
    pub line: usize,
    pub subrow: usize,
    pub segment: VisualSegment,
}

/// Return only the requested visible rows, reading the rope one logical line at a time.
pub(super) fn viewport(
    buffer: &Buffer,
    first: usize,
    count: usize,
    columns: usize,
    tab_size: usize,
    indent: WrapIndent,
) -> Vec<VisualRow> {
    if count == 0 {
        return Vec::new();
    }
    let mut index = 0;
    let mut rows = Vec::with_capacity(count);
    for line in 0..buffer.len_lines() {
        let text = buffer.line(line);
        for (subrow, segment) in wrap_line(&text, columns, tab_size, indent)
            .into_iter()
            .enumerate()
        {
            if index >= first {
                rows.push(VisualRow {
                    line,
                    subrow,
                    segment,
                });
            }
            index += 1;
            if rows.len() == count {
                return rows;
            }
        }
    }
    rows
}

/// Map an absolute character offset to its row in the wrapped document.
pub(super) fn document_position(
    buffer: &Buffer,
    offset: usize,
    columns: usize,
    tab_size: usize,
    indent: WrapIndent,
) -> (usize, usize) {
    let offset = offset.min(buffer.len_chars());
    let line = buffer.rope().char_to_line(offset);
    let preceding = (0..line)
        .map(|index| wrap_line(&buffer.line(index), columns, tab_size, indent).len())
        .sum::<usize>();
    let text = buffer.line(line);
    let segments = wrap_line(&text, columns, tab_size, indent);
    let (subrow, column) = position_to_visual(
        &text,
        &segments,
        offset - buffer.line_start_char(line),
        tab_size,
    );
    (preceding + subrow, column)
}

/// Map a visual row and column to an absolute character offset in the rope.
pub(super) fn document_offset(
    buffer: &Buffer,
    row: usize,
    column: usize,
    columns: usize,
    tab_size: usize,
    indent: WrapIndent,
) -> usize {
    let mut first = 0;
    for line in 0..buffer.len_lines() {
        let text = buffer.line(line);
        let segments = wrap_line(&text, columns, tab_size, indent);
        if row < first + segments.len() || line + 1 == buffer.len_lines() {
            return buffer.line_start_char(line)
                + visual_to_position(
                    &text,
                    &segments,
                    row.saturating_sub(first),
                    column,
                    tab_size,
                );
        }
        first += segments.len();
    }
    buffer.len_chars()
}

impl App {
    pub(super) fn reconfigure_wrapping(&mut self, enabled: bool, indent: WrapIndent) {
        if self.word_wrap == enabled && self.wrapping_indent == indent {
            return;
        }

        let (width, height, lh, cw) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (width, height) = gpu.renderer.size();
                (
                    width,
                    height,
                    gpu.renderer.line_height(),
                    gpu.renderer.char_width(),
                )
            })
            .unwrap_or((1200.0, 760.0, 21.0, 9.0));
        let rects = self.group_rectangles(width, height, lh);
        let old_enabled = self.word_wrap;
        let old_indent = self.wrapping_indent;
        let tabs = &self.documents;
        self.editor_groups.for_each_view_mut(|group, view| {
            let Some(rect) = rects.iter().find(|rect| rect.id == group) else {
                return;
            };
            let Some(tab) = tabs.get(view.document) else {
                return;
            };
            let gutter = tab.buffer.len_lines().to_string().len().max(3) as f32 * cw + PAD * 2.0;
            let columns = (((rect.width - gutter).max(cw)) / cw) as usize;
            let top = if old_enabled {
                document_offset(
                    &tab.buffer,
                    view.scroll_line,
                    0,
                    columns,
                    tab.indentation.tab_size,
                    old_indent,
                )
            } else {
                tab.buffer
                    .line_start_char(view.scroll_line.min(tab.buffer.len_lines() - 1))
            };
            view.scroll_line = if enabled {
                document_position(&tab.buffer, top, columns, tab.indentation.tab_size, indent).0
            } else {
                tab.buffer.rope().char_to_line(top)
            };
        });
        self.word_wrap = enabled;
        self.wrapping_indent = indent;
    }

    pub(super) fn move_visual_selections_for_columns(
        &mut self,
        columns: usize,
        down: bool,
        extend: bool,
    ) {
        let document = self.active_document();
        let tab = &mut self.documents[document];
        let tab_size = tab.indentation.tab_size;
        let indent = self.wrapping_indent;
        let mut buffer = self.editor_mut();
        let last_row = document_position(&buffer, buffer.len_chars(), columns, tab_size, indent).0;
        let selections = buffer
            .selections()
            .into_iter()
            .map(|selection| {
                let (row, column) =
                    document_position(&buffer, selection.head, columns, tab_size, indent);
                let next_row = if down {
                    row.saturating_add(1).min(last_row)
                } else {
                    row.saturating_sub(1)
                };
                let head = if row == next_row {
                    selection.head
                } else {
                    document_offset(&buffer, next_row, column, columns, tab_size, indent)
                };
                graphite_text::Selection {
                    anchor: if extend { selection.anchor } else { head },
                    head,
                }
            })
            .collect();
        buffer.set_selections(selections);
    }

    pub(super) fn move_visual_selections(&mut self, down: bool, extend: bool) -> bool {
        let Some(gpu) = &self.gpu else { return false };
        let (width, height) = gpu.renderer.size();
        let cw = gpu.renderer.char_width();
        let rect = self.focused_group_rect(width, height, gpu.renderer.line_height());
        let columns = (((rect.width - self.gutter_width(cw)).max(cw)) / cw) as usize;
        self.move_visual_selections_for_columns(columns, down, extend);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraps_at_word_boundaries_with_continuation_indent_and_crlf() {
        let text = "  café word xyz\r\n";
        let rows = wrap_line(text, 9, 4, WrapIndent::Same);
        assert_eq!(
            rows,
            vec![
                VisualSegment {
                    start: 0,
                    end: 7,
                    indent: 0,
                    width: 7
                },
                VisualSegment {
                    start: 7,
                    end: 12,
                    indent: 2,
                    width: 5
                },
                VisualSegment {
                    start: 12,
                    end: 15,
                    indent: 2,
                    width: 3
                },
            ]
        );
        assert_eq!(position_to_visual(text, &rows, 7, 4), (1, 2));
        assert_eq!(visual_to_position(text, &rows, 1, 3, 4), 8);
    }
    #[test]
    fn handles_tabs_long_words_and_narrow_or_empty_lines() {
        let text = "\t🙂abcdef";
        let rows = wrap_line(text, 4, 4, WrapIndent::Indent);
        assert_eq!(
            rows[0],
            VisualSegment {
                start: 0,
                end: 1,
                indent: 0,
                width: 4
            }
        );
        assert_eq!(rows[1].indent, 2);
        assert_eq!(visual_to_position(text, &rows, 0, 1, 4), 0);
        assert_eq!(visual_to_position(text, &rows, 0, 3, 4), 1);
        for pos in 0..text.chars().count() {
            let (row, col) = position_to_visual(text, &rows, pos, 4);
            assert_eq!(visual_to_position(text, &rows, row, col, 4), pos);
        }
        assert_eq!(wrap_line("\n", 0, 4, WrapIndent::Same)[0].end, 0);
    }

    #[test]
    fn document_viewport_and_hit_testing_preserve_unicode_offsets() {
        let mut buffer_document = Buffer::new();
        let mut buffer_state = graphite_text::SelectionState::default();
        let mut buffer = buffer_document.edit(&mut buffer_state, graphite_text::ViewId(1));
        buffer.insert("hello world\r\n    é🙂 xyz\n");
        let rows = viewport(&buffer, 1, 3, 6, 4, WrapIndent::Same);
        assert_eq!(
            rows.iter()
                .map(|row| (row.line, row.subrow))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 0), (1, 1)]
        );
        for offset in [0, 1, 6, 8, 13, 14, 15, 16, buffer.len_chars()] {
            let (row, column) = document_position(&buffer, offset, 6, 4, WrapIndent::Same);
            let recovered = document_offset(&buffer, row, column, 6, 4, WrapIndent::Same);
            assert_eq!(recovered, offset, "offset {offset}");
        }
    }

    #[test]
    fn moving_multiple_selections_uses_visual_rows_without_editing_text() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("abcdefghi\nuvwxyz");
        app.test_editor_mut(0).set_selections(vec![
            graphite_text::Selection { anchor: 1, head: 1 },
            graphite_text::Selection {
                anchor: 11,
                head: 11,
            },
        ]);
        let revision = app.documents[app.document_at(0)].buffer.revision();
        app.move_visual_selections_for_columns(4, true, true);
        assert_eq!(
            app.test_editor(0).selections(),
            vec![
                graphite_text::Selection { anchor: 1, head: 5 },
                graphite_text::Selection {
                    anchor: 11,
                    head: 15
                },
            ]
        );
        assert_eq!(
            app.documents[app.document_at(0)].buffer.revision(),
            revision
        );
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "abcdefghi\nuvwxyz"
        );
    }

    #[test]
    fn changing_wrap_setting_preserves_the_top_logical_line_in_each_view() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.test_editor_mut(0)
            .insert(&format!("{}\nsecond\n", "x".repeat(200)));
        app.editor_groups.active_view_mut().scroll_line = 1;
        let other = app.split_editor_group(super::super::editor_groups::SplitDirection::Vertical);
        app.editor_groups.active_view_mut().scroll_line = 0;
        app.reconfigure_wrapping(true, WrapIndent::Same);
        assert!(app.focus_editor_group(0));
        assert!(app.editor_groups.active_view_mut().scroll_line > 1);
        app.reconfigure_wrapping(false, WrapIndent::Same);
        assert_eq!(app.editor_groups.active_view_mut().scroll_line, 1);
        assert!(app.focus_editor_group(other));
        assert_eq!(app.editor_groups.active_view_mut().scroll_line, 0);
    }
}
