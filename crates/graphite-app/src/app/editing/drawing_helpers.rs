use super::*;

pub(crate) fn accessibility_position_to_char(buffer: &Buffer, row: usize, index: usize) -> usize {
    let row = row.min(buffer.len_lines().saturating_sub(1));
    let text = buffer.line(row);
    buffer.line_start_char(row)
        + index.min(super::accessibility::line_content(&text).chars().count())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn terminal_panel_height(height: f32) -> f32 {
    terminal_panel_height_with_delta(height, 0)
}

pub(crate) fn terminal_panel_height_with_delta(height: f32, delta: i16) -> f32 {
    let base = (height * 0.32).clamp(150.0, 300.0);
    (base + delta as f32).clamp(120.0, (height * 0.75).max(300.0))
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

/// Maps a bracket nesting level to its highlight class.
pub(crate) fn bracket_class(level: u8) -> Class {
    match level % 3 {
        0 => Class::BracketLevel1,
        1 => Class::BracketLevel2,
        _ => Class::BracketLevel3,
    }
}

/// Returns spans covering the whole line, with bracket overlays replacing syntax classes.
///
/// `overlays` pairs a character column within `line` with the class to paint there.
pub(crate) fn overlay_classes(
    line: &str,
    spans: &[Span],
    overlays: &[(usize, Class)],
) -> Vec<Span> {
    let mut classes = vec![Class::Default; line.len()];
    for span in spans {
        for byte in span.range.clone() {
            if let Some(slot) = classes.get_mut(byte) {
                *slot = span.class;
            }
        }
    }
    for &(column, class) in overlays {
        if let Some((byte, ch)) = line.char_indices().nth(column) {
            for slot in classes.iter_mut().skip(byte).take(ch.len_utf8()) {
                *slot = class;
            }
        }
    }
    let mut out = Vec::new();
    let mut start = 0;
    for index in 1..=classes.len() {
        if index == classes.len() || classes[index] != classes[start] {
            if start < index {
                out.push(Span {
                    range: start..index,
                    class: classes[start],
                });
            }
            start = index;
        }
    }
    out
}

/// `base` with the classes of `top` painted over it, as spans covering the whole line. Used for
/// semantic tokens over syntax highlighting.
pub(crate) fn overlay_spans(line: &str, base: &[Span], top: &[Span]) -> Vec<Span> {
    let mut classes = vec![Class::Default; line.len()];
    for span in base.iter().chain(top) {
        for byte in span.range.clone() {
            if let Some(slot) = classes.get_mut(byte) {
                *slot = span.class;
            }
        }
    }
    let mut out = Vec::new();
    let mut start = 0;
    for index in 1..=classes.len() {
        if index == classes.len() || classes[index] != classes[start] {
            if start < index {
                out.push(Span {
                    range: start..index,
                    class: classes[start],
                });
            }
            start = index;
        }
    }
    out
}

/// Display columns where indent guides are drawn for a line; blank lines have none.
pub(crate) fn indent_guide_columns(line: &str, tab_width: usize) -> Vec<usize> {
    if line.chars().all(|ch| ch.is_whitespace()) {
        return Vec::new();
    }
    let step = tab_width.max(1);
    let mut width = 0;
    for ch in line.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += step,
            _ => break,
        }
    }
    (0..width).step_by(step).collect()
}

#[cfg(test)]
mod guide_and_bracket_tests {
    use super::*;

    #[test]
    fn indent_guides_follow_each_indent_level_and_skip_blank_lines() {
        assert_eq!(indent_guide_columns("    let x = 1;", 4), vec![0]);
        assert_eq!(indent_guide_columns("        deep", 4), vec![0, 4]);
        assert_eq!(indent_guide_columns("\t\tcode", 4), vec![0, 4]);
        assert!(indent_guide_columns("    ", 4).is_empty());
        assert!(indent_guide_columns("plain", 4).is_empty());
    }

    #[test]
    fn overlays_replace_only_the_bracket_characters() {
        let spans = vec![Span {
            range: 0..5,
            class: Class::Keyword,
        }];
        let merged = overlay_classes(
            "fn(a)",
            &spans,
            &[(2, Class::BracketLevel1), (4, Class::BracketLevel1)],
        );
        assert_eq!(
            merged,
            vec![
                Span {
                    range: 0..2,
                    class: Class::Keyword
                },
                Span {
                    range: 2..3,
                    class: Class::BracketLevel1
                },
                Span {
                    range: 3..4,
                    class: Class::Keyword
                },
                Span {
                    range: 4..5,
                    class: Class::BracketLevel1
                },
            ]
        );
    }

    #[test]
    fn bracket_levels_map_onto_three_classes() {
        assert_eq!(bracket_class(0), Class::BracketLevel1);
        assert_eq!(bracket_class(1), Class::BracketLevel2);
        assert_eq!(bracket_class(2), Class::BracketLevel3);
        assert_eq!(bracket_class(3), Class::BracketLevel1);
    }

    #[test]
    fn terminal_panel_height_applies_delta_within_bounds() {
        assert_eq!(terminal_panel_height(800.0), 256.0);
        assert_eq!(terminal_panel_height_with_delta(800.0, 220), 476.0);
        assert_eq!(terminal_panel_height_with_delta(800.0, -200), 120.0);
    }
}
