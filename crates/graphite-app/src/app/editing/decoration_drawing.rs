//! Draws the text decorations that extensions set on the document (see
//! `plugins/vsix/decorations.rs`): backgrounds under the text, borders and `after` text above it.

use super::drawing_helpers::{display_line, utf16_column_to_chars};
use super::*;
use std::sync::atomic::Ordering;

/// Which layer of the line to draw.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecorationLayer {
    /// Backgrounds, drawn before the line's text.
    Under,
    /// Borders and the text after the line, drawn over the text.
    Over,
}

/// Pixel span `[left, right)` of a mark on a line, or `None` when it is empty.
fn span(
    mark: &crate::app::plugins::DecorationMark,
    text: &str,
    tab_size: usize,
    text_x: f32,
    cw: f32,
    area_right: f32,
) -> Option<(f32, f32)> {
    if mark.whole_line {
        return (area_right > text_x).then_some((text_x, area_right));
    }
    let line_chars = text.trim_end_matches(['\r', '\n']).chars().count();
    let chars_at = |column: Option<u32>, default: usize| {
        column.map_or(default, |column| {
            utf16_column_to_chars(text, column).min(line_chars)
        })
    };
    let columns = |chars: usize| {
        let prefix: String = text.chars().take(chars).collect();
        display_line(&prefix, tab_size).chars().count()
    };
    let start = columns(chars_at(mark.start, 0));
    let end = columns(chars_at(mark.end, line_chars));
    (end > start).then(|| (text_x + start as f32 * cw, text_x + end as f32 * cw))
}

impl App {
    /// Draws one layer of the extension decorations on a line of the document with this URI.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_line_decorations(
        &self,
        r: &mut Renderer,
        layer: DecorationLayer,
        uri: &str,
        line: usize,
        text: &str,
        tab_size: usize,
        geometry: (f32, f32, f32, f32, f32),
    ) {
        let (text_x, y, cw, lh, area_right) = geometry;
        let dark = crate::app::theme::DARK_THEME.load(Ordering::Relaxed);
        for mark in self.vsix_line_decorations(uri, line, dark) {
            match layer {
                DecorationLayer::Under => {
                    if let (Some(color), Some((left, right))) = (
                        mark.look.background,
                        span(&mark, text, tab_size, text_x, cw, area_right),
                    ) {
                        r.draw_rect(left, y, right - left, lh, color);
                    }
                }
                DecorationLayer::Over => {
                    if let (Some(color), Some((left, right))) = (
                        mark.look.underline,
                        span(&mark, text, tab_size, text_x, cw, area_right),
                    ) {
                        r.draw_rect(left, y + lh - 2.0, right - left, 1.0, color);
                    }
                    if let (Some(color), Some((left, right))) = (
                        mark.look.border,
                        span(&mark, text, tab_size, text_x, cw, area_right),
                    ) {
                        let width = right - left;
                        r.draw_rect(left, y, width, 1.0, color);
                        r.draw_rect(left, y + lh - 1.0, width, 1.0, color);
                        r.draw_rect(left, y, 1.0, lh, color);
                        r.draw_rect(right - 1.0, y, 1.0, lh, color);
                    }
                    if let Some(after) = &mark.after {
                        let shown = display_line(text, tab_size).chars().count();
                        let color = after
                            .color
                            .unwrap_or_else(|| rgb(Theme::color(Theme::DIM, 0x858585)));
                        r.draw_text(&after.text, text_x + (shown + 1) as f32 * cw, y, color);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::plugins::DecorationMark;

    fn mark(start: Option<u32>, end: Option<u32>, whole_line: bool) -> DecorationMark {
        DecorationMark {
            look: Default::default(),
            whole_line,
            start,
            end,
            after: None,
        }
    }

    #[test]
    fn spans_follow_character_columns_and_clamp_to_the_line() {
        // `cw` is 10 px and text starts at x = 100.
        let text = "hello world\n";
        assert_eq!(
            span(&mark(Some(2), Some(5), false), text, 4, 100.0, 10.0, 900.0),
            Some((120.0, 150.0))
        );
        assert_eq!(
            span(&mark(None, None, false), text, 4, 100.0, 10.0, 900.0),
            Some((100.0, 210.0))
        );
        // An end column past the line clamps to the line length.
        assert_eq!(
            span(&mark(Some(6), Some(99), false), text, 4, 100.0, 10.0, 900.0),
            Some((160.0, 210.0))
        );
        // Empty ranges draw nothing, a whole line spans the text area.
        assert_eq!(
            span(&mark(Some(3), Some(3), false), text, 4, 100.0, 10.0, 900.0),
            None
        );
        assert_eq!(
            span(&mark(Some(3), Some(3), true), text, 4, 100.0, 10.0, 900.0),
            Some((100.0, 900.0))
        );
    }

    #[test]
    fn utf16_columns_map_to_characters_and_tabs_widen_the_span() {
        // The emoji is two UTF-16 units but one character, one display column.
        assert_eq!(
            span(&mark(Some(2), Some(3), false), "a🌎b", 4, 0.0, 10.0, 500.0),
            Some((10.0, 20.0))
        );
        // A tab counts as `tab_size` display columns.
        assert_eq!(
            span(&mark(Some(0), Some(1), false), "\tx", 4, 0.0, 10.0, 500.0),
            Some((0.0, 40.0))
        );
    }
}
