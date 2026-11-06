use super::*;

pub(crate) fn accessibility_position_to_char(buffer: &Buffer, node: NodeId, index: usize) -> usize {
    let row = node.0.saturating_sub(1_000) as usize;
    let row = row.min(buffer.len_lines().saturating_sub(1));
    let text = buffer.line(row);
    buffer.line_start_char(row) + index.min(text.trim_end_matches(['\n', '\r']).chars().count())
}

pub(crate) fn terminal_panel_height(height: f32) -> f32 {
    (height * 0.32).clamp(150.0, 300.0)
}

pub(crate) fn utf16_column_to_chars(line: &str, units: u32) -> usize {
    let mut consumed = 0;
    let mut chars = 0;
    for ch in line.trim_end_matches(['\n', '\r']).chars() {
        let width = ch.len_utf16() as u32;
        if consumed + width > units {
            break;
        }
        consumed += width;
        chars += 1;
    }
    chars
}

pub(crate) fn spans_for(cache: &Option<HighlightCache>, line: usize) -> &[Span] {
    cache
        .as_ref()
        .and_then(|c| c.lines.get(line))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Draws one line, coloring runs of characters by their highlight class.
pub(crate) fn draw_code_line(
    r: &mut Renderer,
    line: &str,
    spans: &[Span],
    x: f32,
    y: f32,
    cw: f32,
    tab_width: usize,
) {
    let line = line.trim_end_matches(['\n', '\r']);
    let color_of = |class: Class| -> Color { rgb(Theme::class_color(class)) };

    let mut idx = 0;
    let mut col = 0usize;
    let mut run = String::new();
    let mut run_start_col = 0usize;
    let mut run_class = Class::Default;

    for (byte, ch) in line.char_indices() {
        while idx < spans.len() && spans[idx].range.end <= byte {
            idx += 1;
        }
        let class = match spans.get(idx) {
            Some(s) if s.range.start <= byte => s.class,
            _ => Class::Default,
        };
        if class != run_class && !run.is_empty() {
            r.draw_text(&run, x + run_start_col as f32 * cw, y, color_of(run_class));
            run.clear();
        }
        if run.is_empty() {
            run_start_col = col;
        }
        run_class = class;
        if ch == '\t' {
            run.push_str(&" ".repeat(tab_width));
            col += tab_width;
        } else {
            run.push(ch);
            col += 1;
        }
    }
    if !run.is_empty() {
        r.draw_text(&run, x + run_start_col as f32 * cw, y, color_of(run_class));
    }
}

/// Strips the line terminator and expands tabs for display.
pub(crate) fn display_line(line: &str, tab_width: usize) -> String {
    let mut out = String::with_capacity(line.len());
    for ch in line.trim_end_matches(['\n', '\r']).chars() {
        if ch == '\t' {
            out.push_str(&" ".repeat(tab_width));
        } else {
            out.push(ch);
        }
    }
    out
}
