//! Drawing and hit testing of the two-file diff view.
//!
//! The view covers only the editor area, so the menu bar, activity bar, sidebar and status
//! bar stay visible and clickable. Its header carries a layout toggle and a close button.
use super::*;

type Rect = (f32, f32, f32, f32);

/// What a click on the diff header landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffHeaderHit {
    Close,
    ToggleLayout,
    None,
}

fn inside(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

/// Header strip height of the diff view.
pub(crate) fn diff_header_height(line_height: f32) -> f32 {
    line_height * 1.5
}

/// Close and layout button rectangles for a diff area.
pub(crate) fn diff_header_buttons(
    area: Rect,
    line_height: f32,
    char_width: f32,
    layout_label_chars: usize,
) -> (Rect, Rect) {
    let header = diff_header_height(line_height);
    let close = (area.0 + area.2 - header, area.1, header, header);
    let layout_w = layout_label_chars as f32 * char_width + 16.0;
    let layout = (close.0 - layout_w, area.1, layout_w, header);
    (close, layout)
}

pub(crate) fn diff_header_hit(
    area: Rect,
    line_height: f32,
    char_width: f32,
    layout_label_chars: usize,
    x: f32,
    y: f32,
) -> DiffHeaderHit {
    let (close, layout) = diff_header_buttons(area, line_height, char_width, layout_label_chars);
    if inside(close, x, y) {
        DiffHeaderHit::Close
    } else if inside(layout, x, y) {
        DiffHeaderHit::ToggleLayout
    } else {
        DiffHeaderHit::None
    }
}

impl App {
    /// The editor area the diff view occupies: right of the activity bar and sidebar, below the
    /// menu bar and above the status bar.
    pub(crate) fn diff_area(&self, width: f32, height: f32, line_height: f32) -> Rect {
        let top = self.top_menu_height();
        let bottom = if self.view_preferences.status_bar_visible && !self.zen_mode {
            (height - line_height).max(top)
        } else {
            height
        };
        let (mut left, mut right) = (self.activity_bar_width_px(), width);
        if self.view_preferences.primary_sidebar_visible && !self.zen_mode {
            let (sidebar_left, sidebar_width) = self.primary_sidebar_bounds(width);
            if self.activity_bar_preferences.primary_sidebar_right {
                right = sidebar_left;
            } else {
                left = sidebar_left + sidebar_width;
            }
        }
        (left, top, (right - left).max(0.0), (bottom - top).max(0.0))
    }

    pub(crate) fn diff_layout_label(&self) -> &str {
        self.t("scm.diff.toggle_layout")
    }

    pub(crate) fn close_diff_view(&mut self) {
        self.run_ui_command("graphite.action.closeDiffEditor");
        self.status = self.t("diff.closed").into();
        self.check_cli_waiters();
    }

    /// Handles a click while the diff view is open. Returns true when the click was consumed;
    /// a click outside the view returns false so the rest of the window still works.
    pub(crate) fn click_diff_view(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
        line_height: f32,
        char_width: f32,
    ) -> bool {
        if self.diff_view.is_none() {
            return false;
        }
        let area = self.diff_area(width, height, line_height);
        if inside(area, mx, my) {
            let label_chars = self.diff_layout_label().chars().count();
            match diff_header_hit(area, line_height, char_width, label_chars, mx, my) {
                DiffHeaderHit::Close => self.close_diff_view(),
                DiffHeaderHit::ToggleLayout => self.run_ui_command("graphite.diff.toggleLayout"),
                DiffHeaderHit::None => {}
            }
            return true;
        }
        // Using the sidebar means leaving the comparison; the click is then handled normally.
        if self.primary_sidebar_contains(mx, my, width, height, line_height) {
            self.close_diff_view();
        }
        false
    }

    pub(crate) fn draw_diff_view(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
    ) {
        let Some(diff) = &self.diff_view else {
            return;
        };
        let cw = renderer.char_width();
        let (x, y, w, h) = self.diff_area(width, height, line_height);
        let bg = rgb(Theme::color(Theme::BG, 0xffffff));
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        let header_bg = rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3));
        let button_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let header = diff_header_height(line_height);

        renderer.draw_rect(x, y, w, h, bg);
        renderer.draw_rect(x, y, w, header, header_bg);

        let label = self.diff_layout_label();
        let (close, layout) =
            diff_header_buttons((x, y, w, h), line_height, cw, label.chars().count());
        for rect in [close, layout] {
            renderer.draw_rect(
                rect.0 + 1.0,
                rect.1 + 3.0,
                rect.2 - 2.0,
                rect.3 - 6.0,
                button_bg,
            );
        }
        renderer.draw_text(
            label,
            layout.0 + 8.0,
            layout.1 + (header - line_height) * 0.5,
            text,
        );
        renderer.draw_text(
            "×",
            close.0 + (close.2 - cw) * 0.5,
            close.1 + (header - line_height) * 0.5,
            text,
        );

        let title = self.tf("scm.diff.escape_hint", &[("title", &diff.title)]);
        let title_room = (layout.0 - x - PAD * 2.0).max(0.0);
        renderer.draw_text(
            &Self::help_menu_visible_label(&title, cw, title_room),
            x + PAD,
            y + (header - line_height) * 0.5,
            text,
        );

        let content_top = y + header;
        let pane_width = w / 2.0;
        let chars_per_pane = ((pane_width - PAD * 2.0) / cw).max(0.0) as usize;
        let rows = ((h - header) / line_height) as usize;
        renderer.draw_rect(
            x + pane_width - 0.5,
            content_top,
            1.0,
            h - header,
            header_bg,
        );
        renderer.draw_text(
            &Self::help_menu_visible_label(&diff.left_name, cw, pane_width - PAD * 2.0),
            x + PAD,
            content_top,
            dim,
        );
        renderer.draw_text(
            &Self::help_menu_visible_label(&diff.right_name, cw, pane_width - PAD * 2.0),
            x + pane_width + PAD,
            content_top,
            dim,
        );

        if diff.left == diff.right {
            renderer.draw_text(
                self.t("scm.diff.no_changes"),
                x + PAD,
                content_top + line_height * 2.0,
                dim,
            );
            return;
        }
        let clip = |line: &str, chars: usize| line.chars().take(chars).collect::<String>();
        if diff.inline {
            for (index, line) in crate::scm::diff_text(&diff.left, &diff.right)
                .lines()
                .take(rows)
                .enumerate()
            {
                let color = if line.starts_with('+') {
                    0x1a7f37
                } else if line.starts_with('-') {
                    0xcf222e
                } else {
                    0x1e1e1e
                };
                renderer.draw_text(
                    &clip(line, ((w - PAD * 2.0) / cw).max(0.0) as usize),
                    x + PAD,
                    content_top + line_height * (index as f32 + 1.2),
                    rgb(color),
                );
            }
        } else {
            for (side, origin) in [(&diff.left, x + PAD), (&diff.right, x + pane_width + PAD)] {
                for (index, line) in side.lines().take(rows).enumerate() {
                    renderer.draw_text(
                        &clip(line, chars_per_pane),
                        origin,
                        content_top + line_height * (index as f32 + 1.2),
                        text,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::scm_ui::DiffView;
    use crate::test_support::TempDir;

    fn view(left: &str, right: &str) -> DiffView {
        DiffView {
            title: "t".into(),
            left_name: "a".into(),
            right_name: "b".into(),
            left: left.into(),
            right: right.into(),
            inline: false,
            collapse_unchanged: false,
            selected_change: 0,
        }
    }

    #[test]
    fn header_hits_are_close_layout_or_nothing() {
        let area = (100.0, 30.0, 800.0, 500.0);
        let (close, layout) = diff_header_buttons(area, 20.0, 8.0, 10);
        assert!(close.0 + close.2 <= area.0 + area.2 && layout.0 + layout.2 <= close.0);
        let at = |r: Rect| diff_header_hit(area, 20.0, 8.0, 10, r.0 + 1.0, r.1 + 1.0);
        assert_eq!(at(close), DiffHeaderHit::Close);
        assert_eq!(at(layout), DiffHeaderHit::ToggleLayout);
        assert_eq!(
            diff_header_hit(area, 20.0, 8.0, 10, area.0 + 5.0, area.1 + 5.0),
            DiffHeaderHit::None
        );
    }

    #[test]
    fn the_area_leaves_the_menu_bar_activity_bar_and_status_bar_free() {
        let dir = TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (x, y, w, h) = app.diff_area(1200.0, 800.0, 20.0);
        assert!(x >= app.activity_bar_width_px());
        assert!(y >= app.top_menu_height());
        assert!(x + w <= 1200.0 && y + h <= 800.0 - 20.0 + 0.01);
    }

    #[test]
    fn clicks_inside_are_consumed_and_the_close_button_closes_the_view() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.diff_view = Some(view("a", "b"));
        let area = app.diff_area(1200.0, 800.0, 20.0);
        let (close, _) =
            diff_header_buttons(area, 20.0, 8.0, app.diff_layout_label().chars().count());
        assert!(app.click_diff_view(area.0 + 40.0, area.1 + 200.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(app.diff_view.is_some());
        assert!(app.click_diff_view(close.0 + 2.0, close.1 + 2.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(app.diff_view.is_none());
    }

    #[test]
    fn clicks_outside_the_area_are_not_consumed() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.diff_view = Some(view("a", "b"));
        // The menu bar (top) and the status bar (bottom) belong to the rest of the window.
        assert!(!app.click_diff_view(300.0, 2.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(!app.click_diff_view(300.0, 795.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(app.diff_view.is_some());
    }

    #[test]
    fn without_a_diff_nothing_is_consumed() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(!app.click_diff_view(300.0, 300.0, 1200.0, 800.0, 20.0, 8.0));
    }
}
