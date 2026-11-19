//! The draggable dividers between the workbench blocks: the primary side bar, the editor column
//! with the panel below it, and the secondary side bar.
//!
//! A divider lights up while the pointer is over it or while it is being dragged, and a double
//! click puts the block back to its default size.

use super::*;
use std::time::{Duration, Instant};

/// Two presses on the same divider within this time are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sash {
    /// Between the primary side bar and the editor column.
    Primary,
    /// Between the editor column and the secondary side bar.
    Secondary,
    /// Between the editor groups and the panel below them.
    Panel,
}

/// Whether a second press at `now` completes a double click that began with `previous`.
pub(crate) fn is_double_click(previous: Option<(Sash, Instant)>, sash: Sash, now: Instant) -> bool {
    previous.is_some_and(|(last, at)| last == sash && now.duration_since(at) <= DOUBLE_CLICK)
}

impl App {
    /// The divider under the pointer, if any. The secondary side bar's edge wins over the primary
    /// one's, and the panel's top edge is checked last.
    pub(crate) fn sash_at(&self, x: f32, y: f32) -> Option<Sash> {
        if self.zen_mode {
            return None;
        }
        if self.is_over_secondary_resize_handle(x, y) {
            return Some(Sash::Secondary);
        }
        if self.view_preferences.primary_sidebar_visible
            && y >= self.activity_bar_vertical_offset()
            && (x - self.sidebar_width_px()).abs() <= 4.0
        {
            return Some(Sash::Primary);
        }
        self.is_over_terminal_resize_handle(x, y)
            .then_some(Sash::Panel)
    }

    /// The divider to light up: the one being dragged, else the one under the pointer.
    pub(crate) fn active_sash(&self) -> Option<Sash> {
        if self.secondary_resize_dragging {
            Some(Sash::Secondary)
        } else if self.sidebar_resize_dragging {
            Some(Sash::Primary)
        } else if self.panel_resize_dragging {
            Some(Sash::Panel)
        } else {
            self.sash_at(self.mouse.x as f32, self.mouse.y as f32)
        }
    }

    /// Notes a press on `sash`; true when it completes a double click.
    pub(crate) fn sash_double_click(&mut self, sash: Sash, now: Instant) -> bool {
        if is_double_click(self.last_sash_press, sash, now) {
            self.last_sash_press = None;
            true
        } else {
            self.last_sash_press = Some((sash, now));
            false
        }
    }

    /// Puts the block next to `sash` back to its default size.
    pub(crate) fn reset_sash(&mut self, sash: Sash) {
        match sash {
            Sash::Primary => {
                self.sidebar_priority_primary = true;
                self.sidebar_width = config::DEFAULT_SIDEBAR_WIDTH;
                if self.profiles.is_some() {
                    let _ = config::save_sidebar_width(&self.profile_root, self.sidebar_width);
                }
            }
            Sash::Secondary => {
                self.sidebar_priority_primary = false;
                self.secondary_sidebar_user_width = None;
                if self.profiles.is_some() {
                    config::clear_secondary_sidebar_width(&self.profile_root);
                }
            }
            Sash::Panel => {
                self.view_preferences.panel_height_delta = 0;
                self.save_view_preferences_for_command();
            }
        }
    }

    /// Lights up the active divider with a thin line in the accent color, over the blocks.
    pub(crate) fn draw_sash_highlight(&self, r: &mut Renderer, w: f32, h: f32, lh: f32) {
        let Some(sash) = self.active_sash() else {
            return;
        };
        let color = rgb(Theme::color(Theme::STATUS, 0x005fb8));
        let top = self.activity_bar_vertical_offset();
        let height = (h - lh - top).max(0.0);
        match sash {
            Sash::Primary => r.draw_rect(self.sidebar_width_px() - 1.0, top, 2.0, height, color),
            Sash::Secondary => {
                r.draw_rect(self.secondary_separator_x(w) - 1.0, top, 2.0, height, color)
            }
            Sash::Panel => {
                let panel = self.panel_bounds(w, h, lh);
                r.draw_rect(panel.x, panel.y - 1.0, panel.width, 2.0, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, crate::test_support::TempDir) {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.sidebar_window_width_px = 1400.0;
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        (app, dir)
    }

    #[test]
    fn a_double_click_needs_the_same_divider_in_time() {
        let start = Instant::now();
        let soon = start + Duration::from_millis(250);
        let late = start + Duration::from_millis(900);
        assert!(!is_double_click(None, Sash::Primary, soon));
        assert!(is_double_click(
            Some((Sash::Primary, start)),
            Sash::Primary,
            soon
        ));
        assert!(!is_double_click(
            Some((Sash::Primary, start)),
            Sash::Secondary,
            soon
        ));
        assert!(!is_double_click(
            Some((Sash::Primary, start)),
            Sash::Primary,
            late
        ));

        let (mut app, _dir) = app();
        assert!(!app.sash_double_click(Sash::Panel, start));
        assert!(app.sash_double_click(Sash::Panel, soon));
        // The double click is used up: the next press starts a new one.
        assert!(!app.sash_double_click(Sash::Panel, soon));
    }

    #[test]
    fn each_divider_is_found_at_its_own_edge() {
        let (mut app, _dir) = app();
        let y = app.activity_bar_vertical_offset() + 100.0;
        let primary_x = app.sidebar_width_px();
        let secondary_x = app.secondary_separator_x(1400.0);
        assert_eq!(app.sash_at(primary_x + 2.0, y), Some(Sash::Primary));
        assert_eq!(app.sash_at(secondary_x - 2.0, y), Some(Sash::Secondary));
        assert_eq!(app.sash_at((primary_x + secondary_x) / 2.0, y), None);
        app.zen_mode = true;
        assert_eq!(app.sash_at(primary_x, y), None);
        app.zen_mode = false;
        app.view_preferences.primary_sidebar_visible = false;
        assert_eq!(app.sash_at(app.sidebar_width_px() + 1.0, y), None);
    }

    #[test]
    fn a_dragged_divider_stays_lit_wherever_the_pointer_goes() {
        let (mut app, _dir) = app();
        app.mouse.x = 600.0;
        app.mouse.y = 300.0;
        assert_eq!(app.active_sash(), None);
        app.secondary_resize_dragging = true;
        assert_eq!(app.active_sash(), Some(Sash::Secondary));
        app.secondary_resize_dragging = false;
        app.sidebar_resize_dragging = true;
        assert_eq!(app.active_sash(), Some(Sash::Primary));
        app.sidebar_resize_dragging = false;
        app.panel_resize_dragging = true;
        assert_eq!(app.active_sash(), Some(Sash::Panel));
    }

    #[test]
    fn resetting_a_divider_restores_the_default_size_of_its_block() {
        let (mut app, _dir) = app();
        app.sidebar_width = 480.0;
        app.reset_sash(Sash::Primary);
        assert_eq!(app.sidebar_width, config::DEFAULT_SIDEBAR_WIDTH);

        app.resize_secondary_sidebar_from_pointer(1400.0 - 600.0);
        assert_eq!(app.secondary_sidebar_width(), 600.0);
        app.reset_sash(Sash::Secondary);
        assert_eq!(app.secondary_sidebar_user_width, None);
        assert_eq!(app.secondary_sidebar_width(), 320.0);

        app.view_preferences.panel_height_delta = 90;
        app.reset_sash(Sash::Panel);
        assert_eq!(app.view_preferences.panel_height_delta, 0);
    }
}
