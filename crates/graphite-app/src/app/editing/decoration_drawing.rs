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

#[cfg(test)]
/// Pixel span `[left, right)` of a mark on a line, or `None` when it is empty.
fn span(
    mark: &crate::app::plugins::DecorationMark,
    text: &str,
    tab_size: usize,
    text_x: f32,
    cw: f32,
    area_right: f32,
) -> Option<(f32, f32)> {
    let line_chars = text.trim_end_matches(['\r', '\n']).chars().count();
    span_in(mark, text, tab_size, text_x, cw, area_right, 0..line_chars)
}

/// Like [`span`] for the part of the line in the characters `segment`, which starts at `text_x`
/// (a row of a wrapped line). Returns `None` when the mark does not reach into the segment.
fn span_in(
    mark: &crate::app::plugins::DecorationMark,
    text: &str,
    tab_size: usize,
    text_x: f32,
    cw: f32,
    area_right: f32,
    segment: std::ops::Range<usize>,
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
    let start = chars_at(mark.start, 0).clamp(segment.start, segment.end);
    let end = chars_at(mark.end, line_chars).clamp(segment.start, segment.end);
    let columns = |from: usize, to: usize| {
        let part: String = text.chars().skip(from).take(to - from).collect();
        display_line(&part, tab_size).chars().count()
    };
    let left = columns(segment.start, start);
    let right = left + columns(start, end);
    (end > start).then_some((text_x + left as f32 * cw, text_x + right as f32 * cw))
}

impl App {
    /// The URI extensions know the document at `path` by.
    pub(crate) fn decoration_uri(&self, path: &std::path::Path) -> Option<String> {
        crate::app::plugins::vsix::virtual_fs::uri_for_path(path)
            .map_or_else(|| graphite_core_api::DocumentUri::from_path(path), Ok)
            .ok()
            .map(|uri| uri.as_str().to_owned())
    }

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
        let line_chars = text.trim_end_matches(['\r', '\n']).chars().count();
        // Unwrapped lines show their lenses in a row above, so the end of the line keeps its own.
        let lens_above = !self.word_wrap;
        self.draw_marks(
            r,
            layer,
            uri,
            line,
            text,
            tab_size,
            geometry,
            0..line_chars,
            lens_above,
        );
    }

    /// Draws one layer of the decorations on the characters `segment` of a line: all of it, or
    /// one row of a wrapped line. The `after` text goes after the last row of the line.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_segment_decorations(
        &self,
        r: &mut Renderer,
        layer: DecorationLayer,
        uri: &str,
        line: usize,
        text: &str,
        tab_size: usize,
        geometry: (f32, f32, f32, f32, f32),
        segment: std::ops::Range<usize>,
    ) {
        self.draw_marks(
            r, layer, uri, line, text, tab_size, geometry, segment, false,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_marks(
        &self,
        r: &mut Renderer,
        layer: DecorationLayer,
        uri: &str,
        line: usize,
        text: &str,
        tab_size: usize,
        geometry: (f32, f32, f32, f32, f32),
        segment: std::ops::Range<usize>,
        skip_lens: bool,
    ) {
        let (text_x, y, cw, lh, area_right) = geometry;
        let line_chars = text.trim_end_matches(['\r', '\n']).chars().count();
        let dark = crate::app::theme::DARK_THEME.load(Ordering::Relaxed);
        for mark in self.vsix_line_decorations(uri, line, dark) {
            if skip_lens && mark.lens {
                continue;
            }
            let span = span_in(
                &mark,
                text,
                tab_size,
                text_x,
                cw,
                area_right,
                segment.clone(),
            );
            match layer {
                DecorationLayer::Under => {
                    if let (Some(color), Some((left, right))) = (mark.look.background, span) {
                        r.draw_rect(left, y, right - left, lh, color);
                    }
                }
                DecorationLayer::Over => {
                    if let (Some(color), Some((left, right))) = (mark.look.underline, span) {
                        r.draw_rect(left, y + lh - 2.0, right - left, 1.0, color);
                    }
                    if let (Some(color), Some((left, right))) = (mark.look.border, span) {
                        let width = right - left;
                        r.draw_rect(left, y, width, 1.0, color);
                        r.draw_rect(left, y + lh - 1.0, width, 1.0, color);
                        r.draw_rect(left, y, 1.0, lh, color);
                        r.draw_rect(right - 1.0, y, 1.0, lh, color);
                    }
                    if let Some(before) = &mark.before {
                        // Drawn over the text at the start of the range, like an inlay hint.
                        let start = mark.start.map_or(0, |column| {
                            utf16_column_to_chars(text, column).min(line_chars)
                        });
                        if segment.contains(&start)
                            || (start == line_chars && segment.end == line_chars)
                        {
                            let prefix: String = text
                                .chars()
                                .skip(segment.start)
                                .take(start - segment.start)
                                .collect();
                            let shown = display_line(&prefix, tab_size).chars().count();
                            let color = before
                                .color
                                .unwrap_or_else(|| rgb(Theme::color(Theme::DIM, 0x858585)));
                            r.draw_text(&before.text, text_x + shown as f32 * cw, y, color);
                        }
                    }
                    if let Some(after) = &mark.after {
                        if segment.end >= line_chars {
                            let tail: String = text.chars().skip(segment.start).collect();
                            let shown = display_line(tail.trim_end_matches(['\r', '\n']), tab_size)
                                .chars()
                                .count();
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

    /// Draws the gutter icons of the decorations on a line: small images at the gutter's left.
    pub(crate) fn draw_gutter_icons(
        &self,
        r: &mut Renderer,
        uri: &str,
        line: usize,
        x: f32,
        y: f32,
        lh: f32,
    ) {
        let dark = crate::app::theme::DARK_THEME.load(Ordering::Relaxed);
        for mark in self.vsix_line_decorations(uri, line, dark) {
            let Some(path) = &mark.gutter_icon else {
                continue;
            };
            let image = self
                .decoration_icons
                .borrow_mut()
                .entry(path.clone())
                .or_insert_with(|| crate::icon_theme::load_file_image(path))
                .clone();
            if let Some(image) = image {
                let size = (lh * 0.85).round();
                r.draw_image(&image, x + 1.0, y + (lh - size) * 0.5, size, size);
            }
        }
    }

    /// Draws breakpoint markers, the active paused debug line, and Test Explorer status
    /// decorators in the editor gutter for `path` at 0-based `line`.
    pub(crate) fn draw_debug_and_test_gutter(
        &self,
        r: &mut Renderer,
        path: Option<&std::path::Path>,
        line: usize,
        x: f32,
        y: f32,
        lh: f32,
    ) {
        let Some(path) = path else {
            return;
        };
        if let Some(color) = breakpoint_gutter_color(
            self.debug.breakpoints.get(path).map(Vec::as_slice),
            line,
            self.debug.breakpoints_disabled,
        ) {
            let size = (lh * 0.45).clamp(6.0, 9.0);
            r.draw_rect(x + 3.0, y + (lh - size) * 0.5, size, size, rgb(color));
        }
        if matches!(self.debug.state, crate::debugger::DebugState::Paused { .. }) {
            let active_frame = self.debug.frames.get(self.debug.selected_frame);
            if active_frame.is_some_and(|frame| {
                frame.source.as_deref() == Some(path) && frame.line as usize == line
            }) {
                r.draw_rect(x + 1.0, y + 2.0, 4.0, (lh - 4.0).max(4.0), rgb(0xfacc15));
            }
        }
        if let Some(color) = test_gutter_color(&self.test_explorer, path, line) {
            let size = (lh * 0.42).clamp(5.0, 8.0);
            r.draw_rect(x + 11.0, y + (lh - size) * 0.5, size, size, rgb(color));
        }
    }

    /// Draws the overview ruler: a strip at the right edge of the editor with a mark for each
    /// decoration that asks for one, placed by its line within the whole document.
    pub(crate) fn draw_overview_ruler(
        &self,
        r: &mut Renderer,
        uri: &str,
        total_lines: usize,
        right: f32,
        top: f32,
        height: f32,
    ) {
        let dark = crate::app::theme::DARK_THEME.load(Ordering::Relaxed);
        let marks = self.vsix_overview_marks(uri, dark);
        if marks.is_empty() || total_lines == 0 || height <= 0.0 {
            return;
        }
        let mark_height = (height / total_lines as f32).clamp(2.0, 6.0);
        for (line, color) in marks {
            let y = top + (line.min(total_lines - 1) as f32 / total_lines as f32) * height;
            r.draw_rect(
                right - 6.0,
                y.min(top + height - mark_height),
                6.0,
                mark_height,
                color,
            );
        }
    }

    /// What decorations ask to show on hover at the cursor of the active document.
    pub(crate) fn decoration_hover_at_cursor(&self) -> Option<String> {
        let document = self.active_document();
        let uri = self.decoration_uri(self.documents[document].buffer.path()?)?;
        let (line, column) = self.editor().cursor_line_col();
        let text = self.editor().line(line);
        let utf16: usize = text.chars().take(column).map(char::len_utf16).sum();
        let found = self.vsix_decoration_hover(&uri, line as u32, utf16 as u32);
        (!found.is_empty()).then(|| found.join("\n\n"))
    }
}

/// Selects the gutter color for a breakpoint on 0-based `line`, if any exists.
pub(crate) fn breakpoint_gutter_color(
    points: Option<&[serde_json::Value]>,
    line: usize,
    all_disabled: bool,
) -> Option<u32> {
    let target_line = line as u64 + 1;
    let point = points?
        .iter()
        .find(|p| p["line"].as_u64() == Some(target_line))?;
    let enabled = point["enabled"].as_bool().unwrap_or(true) && !all_disabled;
    if !enabled {
        return Some(0x6b7280);
    }
    if point["logMessage"].as_str().is_some_and(|s| !s.is_empty()) {
        return Some(0xa855f7);
    }
    if point["condition"].as_str().is_some_and(|s| !s.is_empty())
        || point["hitCondition"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        return Some(0xd97706);
    }
    Some(0xe51400)
}

/// Selects the gutter color for the highest-priority Test Explorer item at `path` and 0-based `line`.
pub(crate) fn test_gutter_color(
    explorer: &crate::test_explorer::TestExplorer,
    path: &std::path::Path,
    line: usize,
) -> Option<u32> {
    fn visit(
        items: &[crate::test_explorer::TestItem],
        path: &std::path::Path,
        line: u32,
        best: &mut Option<(u8, u32)>,
    ) {
        use crate::test_explorer::TestState;
        for item in items {
            if item.uri.as_deref() == Some(path) && item.line == Some(line) {
                let ranked = match &item.state {
                    TestState::Failed { .. } | TestState::Errored(_) => (6, 0xef4444),
                    TestState::Running => (5, 0x3b82f6),
                    TestState::Queued => (4, 0xf59e0b),
                    TestState::Passed => (3, 0x22c55e),
                    TestState::Skipped => (2, 0x6b7280),
                    TestState::Discovered => (1, 0x9ca3af),
                };
                if best.is_none_or(|(rank, _)| ranked.0 > rank) {
                    *best = Some(ranked);
                }
            }
            visit(&item.children, path, line, best);
        }
    }
    let mut best = None;
    for controller in explorer.controllers.values() {
        visit(&controller.roots, path, line as u32, &mut best);
    }
    best.map(|(_, color)| color)
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
            ..Default::default()
        }
    }

    #[test]
    fn spans_in_a_wrapped_row_are_relative_to_the_row() {
        // A 20 character line wrapped into rows of 8; the second row starts at character 8.
        let text = "0123456789abcdefghij
";
        let row = |start: Option<u32>, end: Option<u32>| {
            span_in(&mark(start, end, false), text, 4, 50.0, 10.0, 900.0, 8..16)
        };
        // Inside the row: columns count from the start of the row.
        assert_eq!(row(Some(10), Some(12)), Some((70.0, 90.0)));
        // A range that crosses the row boundaries is cut to the row.
        assert_eq!(row(Some(5), Some(30)), Some((50.0, 130.0)));
        assert_eq!(row(Some(12), Some(30)), Some((90.0, 130.0)));
        // Ranges outside the row draw nothing there.
        assert_eq!(row(Some(0), Some(8)), None);
        assert_eq!(row(Some(16), Some(20)), None);
        // A whole-line mark spans every row.
        assert_eq!(
            span_in(&mark(None, None, true), text, 4, 50.0, 10.0, 900.0, 8..16),
            Some((50.0, 900.0))
        );
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

    #[test]
    fn breakpoint_and_test_gutter_colors_reflect_state() {
        let points = vec![
            serde_json::json!({ "line": 2 }),
            serde_json::json!({ "line": 4, "condition": "x > 1" }),
            serde_json::json!({ "line": 6, "logMessage": "hit" }),
            serde_json::json!({ "line": 8, "enabled": false }),
        ];
        assert_eq!(
            breakpoint_gutter_color(Some(&points), 1, false),
            Some(0xe51400)
        );
        assert_eq!(
            breakpoint_gutter_color(Some(&points), 3, false),
            Some(0xd97706)
        );
        assert_eq!(
            breakpoint_gutter_color(Some(&points), 5, false),
            Some(0xa855f7)
        );
        assert_eq!(
            breakpoint_gutter_color(Some(&points), 7, false),
            Some(0x6b7280)
        );
        assert_eq!(
            breakpoint_gutter_color(Some(&points), 1, true),
            Some(0x6b7280)
        );
        assert_eq!(breakpoint_gutter_color(Some(&points), 0, false), None);

        let mut explorer = crate::test_explorer::TestExplorer::default();
        explorer
            .register_controller("c".into(), "Tests".into())
            .unwrap();
        let file = std::path::PathBuf::from("src/lib.rs");
        explorer
            .replace_roots(
                "c",
                vec![crate::test_explorer::TestItem {
                    id: "t1".into(),
                    label: "t1".into(),
                    uri: Some(file.clone()),
                    line: Some(10),
                    state: crate::test_explorer::TestState::Passed,
                    output: Vec::new(),
                    children: vec![crate::test_explorer::TestItem {
                        id: "t2".into(),
                        label: "t2".into(),
                        uri: Some(file.clone()),
                        line: Some(10),
                        state: crate::test_explorer::TestState::Failed {
                            message: "boom".into(),
                            expected: None,
                            actual: None,
                        },
                        output: Vec::new(),
                        children: Vec::new(),
                    }],
                }],
            )
            .unwrap();
        // Failed takes precedence over Passed on the same line.
        assert_eq!(test_gutter_color(&explorer, &file, 10), Some(0xef4444));
        assert_eq!(test_gutter_color(&explorer, &file, 11), None);
    }
}
