use super::editor_groups::{GroupRect, View};
use super::visual_lines;
use super::*;

fn clipped_spans(spans: &[Span], byte_start: usize, byte_end: usize) -> Vec<Span> {
    spans
        .iter()
        .filter_map(|span| {
            let start = span.range.start.max(byte_start);
            let end = span.range.end.min(byte_end);
            (start < end).then(|| Span {
                range: start - byte_start..end - byte_start,
                class: span.class,
            })
        })
        .collect()
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_wrapped_view(
        &self,
        r: &mut Renderer,
        rect: GroupRect,
        view: &View,
        text_x: f32,
        gutter: f32,
        visible: usize,
        lh: f32,
        cw: f32,
    ) {
        let Some(index) = self.resolve_document(view.document) else {
            return;
        };
        let tab = &self.documents[index];
        let buffer = &tab.buffer;
        let tab_size = tab.indentation.tab_size;
        let columns = (((rect.x + rect.width - text_x).max(cw)) / cw) as usize;
        let decoration_uri = buffer.path().and_then(|path| self.decoration_uri(path));
        let rows = visual_lines::viewport(
            buffer,
            view.scroll_line,
            visible,
            columns,
            tab_size,
            self.wrapping_indent,
        );
        let primary = view
            .selection_state
            .selections()
            .first()
            .copied()
            .unwrap_or_default();
        let cur_line = buffer
            .rope()
            .char_to_line(primary.head.min(buffer.len_chars()));
        let highlight_current_line = primary.anchor == primary.head;
        let chrome = super::drawing_helpers::editor_chrome();
        for (row_index, row) in rows.iter().enumerate() {
            let y = rect.y + lh * 1.5 + row_index as f32 * lh;
            if highlight_current_line && row.line == cur_line {
                r.draw_rect(
                    text_x,
                    y,
                    (rect.x + rect.width - text_x).max(0.0),
                    lh,
                    rgb(chrome.current_line),
                );
            }
            let text = buffer.line(row.line);
            let start_char = buffer.line_start_char(row.line);
            let segment = &row.segment;
            let byte_start = text
                .char_indices()
                .nth(segment.start)
                .map_or(text.len(), |(i, _)| i);
            let byte_end = text
                .char_indices()
                .nth(segment.end)
                .map_or(text.len(), |(i, _)| i);
            let slice = &text[byte_start..byte_end];
            let row_x = text_x + segment.indent as f32 * cw;
            let row_right = rect.x + rect.width;
            if let Some(uri) = &decoration_uri {
                if row.subrow == 0 {
                    self.draw_gutter_icons(r, uri, row.line, rect.x, y, lh);
                }
                self.draw_segment_decorations(
                    r,
                    super::decoration_drawing::DecorationLayer::Under,
                    uri,
                    row.line,
                    &text,
                    tab_size,
                    (row_x, y, cw, lh, row_right),
                    segment.start..segment.end,
                );
            }
            for selection in &view.selection_state.selections() {
                let start = selection
                    .anchor
                    .min(selection.head)
                    .max(start_char + segment.start);
                let end = selection
                    .anchor
                    .max(selection.head)
                    .min(start_char + segment.end);
                if start < end {
                    let prefix: String = text
                        .chars()
                        .skip(segment.start)
                        .take(start - start_char - segment.start)
                        .collect();
                    let selected: String = text
                        .chars()
                        .skip(start - start_char)
                        .take(end - start)
                        .collect();
                    let left = display_line(&prefix, tab_size).chars().count();
                    let width = display_line(&selected, tab_size).chars().count();
                    r.draw_rect(
                        row_x + left as f32 * cw,
                        y,
                        width as f32 * cw,
                        lh,
                        rgb(0x264f78),
                    );
                }
            }
            if row.subrow == 0 {
                self.draw_debug_and_test_gutter(r, buffer.path(), row.line, rect.x, y, lh);
                if tab
                    .diagnostics
                    .iter()
                    .chain(&tab.schema_diagnostics)
                    .any(|d| d.range.start.line as usize == row.line)
                {
                    r.draw_rect(rect.x + 2.0, y + lh * 0.3, 5.0, 5.0, rgb(0xf14c4c));
                }
                r.draw_text(
                    &format!(
                        "{:>width$}",
                        row.line + 1,
                        width = ((gutter - PAD * 2.0) / cw) as usize
                    ),
                    rect.x + PAD,
                    y,
                    rgb(Theme::color(Theme::DIM, 0x616161)),
                );
            }
            let spans = clipped_spans(spans_for(&tab.highlights, row.line), byte_start, byte_end);
            draw_code_line(r, slice, &spans, row_x, y, cw, tab_size);
            if let Some(uri) = &decoration_uri {
                self.draw_segment_decorations(
                    r,
                    super::decoration_drawing::DecorationLayer::Over,
                    uri,
                    row.line,
                    &text,
                    tab_size,
                    (row_x, y, cw, lh, row_right),
                    segment.start..segment.end,
                );
            }
            super::drawing_helpers::draw_line_diagnostic_squiggles(
                r,
                &tab.diagnostics,
                &tab.schema_diagnostics,
                &text,
                row.line,
                segment.start,
                segment.end,
                row_x,
                y,
                cw,
                lh,
                tab_size,
            );
        }
        if let Some(uri) = &decoration_uri {
            self.draw_overview_ruler(
                r,
                uri,
                buffer.len_lines(),
                rect.x + rect.width,
                rect.y,
                rect.height,
            );
        }
        for selection in &view.selection_state.selections() {
            let (row, column) = visual_lines::document_position(
                buffer,
                selection.head,
                columns,
                tab_size,
                self.wrapping_indent,
            );
            if row < view.scroll_line || row >= view.scroll_line + visible {
                continue;
            }
            let x = text_x + column as f32 * cw;
            let y = rect.y + lh * 1.5 + (row - view.scroll_line) as f32 * lh;
            r.draw_rect(x, y, 2.0, lh, rgb(Theme::color(Theme::CURSOR, 0x202020)));
            if rect.id == self.editor_groups.focused()
                && view.selection_state.selections().first() == Some(selection)
                && !view.ime_preedit.is_empty()
                && self.local_search.is_none()
                && self.quick_open.is_none()
                && self.search_query.is_none()
                && self.snippet_query.is_none()
                && !self.terminal_focused
            {
                r.draw_text(
                    &view.ime_preedit,
                    x,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
                r.draw_rect(
                    x,
                    y + lh - 2.0,
                    view.ime_preedit.chars().count() as f32 * cw,
                    1.0,
                    rgb(Theme::color(Theme::CURSOR, 0x202020)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn syntax_spans_are_clipped_to_visual_segment_byte_boundaries() {
        let spans = vec![
            Span {
                range: 0..3,
                class: Class::Keyword,
            },
            Span {
                range: 4..12,
                class: Class::String,
            },
            Span {
                range: 13..16,
                class: Class::Comment,
            },
        ];
        let clipped = clipped_spans(&spans, 7, 14);
        assert_eq!(clipped.len(), 2);
        assert_eq!(clipped[0].range, 0..5);
        assert_eq!(clipped[1].range, 6..7);
    }
}
