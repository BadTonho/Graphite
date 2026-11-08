use super::*;

pub(crate) fn accessibility_position_to_char(buffer: &Buffer, row: usize, index: usize) -> usize {
    let row = row.min(buffer.len_lines().saturating_sub(1));
    let text = buffer.line(row);
    buffer.line_start_char(row)
        + index.min(super::accessibility::line_content(&text).chars().count())
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

pub(crate) fn draw_diagnostic_squiggle(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    width: f32,
    line_height: f32,
    color: u32,
) {
    if width <= 0.0 {
        return;
    }
    let mut offset = 0.0;
    while offset < width {
        let wave = if ((offset / 2.0) as usize) % 2 == 0 {
            0.0
        } else {
            1.0
        };
        renderer.draw_rect(
            x + offset,
            y + line_height - 2.0 + wave,
            2.0,
            1.0,
            rgb(color),
        );
        offset += 3.0;
    }
}

pub(crate) fn draw_line_diagnostic_squiggles(
    renderer: &mut Renderer,
    diagnostics: &[lsp_types::Diagnostic],
    schema_diagnostics: &[lsp_types::Diagnostic],
    text: &str,
    line: usize,
    segment_start: usize,
    segment_end: usize,
    x: f32,
    y: f32,
    char_width: f32,
    line_height: f32,
    tab_size: usize,
) {
    let line_text = text.trim_end_matches(['\n', '\r']);
    let line_chars = line_text.chars().count();
    let segment_start = segment_start.min(line_chars);
    let segment_end = segment_end.min(line_chars).max(segment_start);
    if segment_start == segment_end {
        return;
    }

    for diagnostic in diagnostics.iter().chain(schema_diagnostics) {
        if diagnostic.range.start.line as usize != line {
            continue;
        }
        let start = utf16_column_to_chars(line_text, diagnostic.range.start.character);
        let mut end = utf16_column_to_chars(line_text, diagnostic.range.end.character);
        if end <= start {
            end = (start + 1).min(line_chars);
        }
        let start = start.max(segment_start).min(segment_end);
        let end = end.max(segment_start).min(segment_end);
        if start >= end {
            continue;
        }

        let display_column = |prefix: &str| {
            prefix
                .chars()
                .map(|ch| if ch == '\t' { tab_size } else { 1 })
                .sum::<usize>()
        };
        let prefix = line_text
            .chars()
            .skip(segment_start)
            .take(start - segment_start)
            .collect::<String>();
        let marked = line_text
            .chars()
            .skip(start)
            .take(end - start)
            .collect::<String>();
        let x = x + display_column(&prefix) as f32 * char_width;
        let width = (display_column(&marked) as f32 * char_width).max(char_width);
        let color = match diagnostic.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => 0xf14c4c,
            Some(lsp_types::DiagnosticSeverity::WARNING) => 0xe2c08d,
            Some(lsp_types::DiagnosticSeverity::INFORMATION) => 0x3794ff,
            Some(lsp_types::DiagnosticSeverity::HINT) => 0x75beff,
            _ => 0xf14c4c,
        };
        draw_diagnostic_squiggle(renderer, x, y, width, line_height, color);
    }
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
