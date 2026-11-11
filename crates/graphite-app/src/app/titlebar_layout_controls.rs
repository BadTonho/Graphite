//! Layout buttons in the title bar, left of the window controls: customize the layout, and show
//! or hide the primary side bar, the panel, and the secondary side bar.

use super::*;

/// Width of one layout button, before scaling.
const BUTTON_WIDTH: f32 = 30.0;
/// Pixel rectangle: x, y, width, height.
pub(crate) type Rect = (f32, f32, f32, f32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayoutControl {
    Customize,
    PrimarySidebar,
    Panel,
    SecondarySidebar,
}

/// Left to right, the order the buttons appear in.
pub(crate) const LAYOUT_CONTROLS: [LayoutControl; 4] = [
    LayoutControl::Customize,
    LayoutControl::PrimarySidebar,
    LayoutControl::Panel,
    LayoutControl::SecondarySidebar,
];

impl LayoutControl {
    /// Command that the button runs; `Customize` opens the layout menu instead.
    pub(crate) fn command(self) -> &'static str {
        match self {
            LayoutControl::Customize => "graphite.layout.customize",
            LayoutControl::PrimarySidebar => "graphite.layout.togglePrimarySidebar",
            LayoutControl::Panel => "graphite.layout.togglePanel",
            LayoutControl::SecondarySidebar => "graphite.layout.toggleSecondarySidebar",
        }
    }

    /// Translation key of the tooltip, which also names the shortcut.
    fn tooltip_key(self) -> &'static str {
        match self {
            LayoutControl::Customize => "titlebar.layout.customize",
            LayoutControl::PrimarySidebar => "titlebar.layout.primary_sidebar",
            LayoutControl::Panel => "titlebar.layout.panel",
            LayoutControl::SecondarySidebar => "titlebar.layout.secondary_sidebar",
        }
    }
}

/// Rectangles of the four buttons, laid out immediately left of the window controls.
pub(crate) fn layout_control_rects(
    controls_left: f32,
    title_height: f32,
    scale: f32,
) -> [(LayoutControl, Rect); 4] {
    let width = BUTTON_WIDTH * scale;
    let first_x = controls_left - width * LAYOUT_CONTROLS.len() as f32 - 2.0 * scale;
    LAYOUT_CONTROLS.map(|control| {
        let index = LAYOUT_CONTROLS.iter().position(|item| *item == control).unwrap() as f32;
        (control, (first_x + index * width, 0.0, width, title_height))
    })
}

fn contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

/// The layout button under a title bar point, if any.
pub(crate) fn layout_control_at(
    rects: &[(LayoutControl, Rect); 4],
    x: f32,
    y: f32,
) -> Option<LayoutControl> {
    rects
        .iter()
        .find(|(_, rect)| contains(*rect, x, y))
        .map(|(control, _)| *control)
}

impl App {
    /// Whether the button's panel is visible, for drawing its state.
    fn layout_control_active(&self, control: LayoutControl) -> bool {
        match control {
            LayoutControl::Customize => false,
            LayoutControl::PrimarySidebar => self.view_preferences.primary_sidebar_visible,
            LayoutControl::Panel => self.view_preferences.panel_visible,
            LayoutControl::SecondarySidebar => self.view_preferences.secondary_sidebar_visible,
        }
    }

    /// Runs a button's action from the title bar.
    pub(crate) fn run_layout_control(&mut self, control: LayoutControl, anchor_x: f32) {
        if control == LayoutControl::Customize {
            let Some((width, height)) = self.gpu.as_ref().map(|gpu| gpu.renderer.size()) else {
                return;
            };
            self.open_view_menu(anchor_x, self.top_menu_height(), width, height);
        } else {
            self.run_ui_command(control.command());
        }
    }

    /// Draws the four buttons, their state, and the tooltip of the hovered one.
    pub(crate) fn draw_layout_controls(&self, renderer: &mut Renderer, rects: &[(LayoutControl, Rect); 4], title_height: f32) {
        let (x, y) = (self.mouse.x as f32, self.mouse.y as f32);
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let hovered_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let mut hovered = None;
        for (control, rect) in rects {
            let is_hovered = contains(*rect, x, y);
            if is_hovered {
                hovered = Some((*control, *rect));
            }
            renderer.draw_rect(
                rect.0,
                rect.1,
                rect.2,
                rect.3,
                if is_hovered { hovered_bg } else { background },
            );
            draw_icon(renderer, *control, *rect, self.layout_control_active(*control), foreground, background);
        }
        if let Some((control, rect)) = hovered {
            self.draw_layout_tooltip(renderer, control, rect, title_height);
        }
    }

    fn draw_layout_tooltip(&self, renderer: &mut Renderer, control: LayoutControl, rect: Rect, title_height: f32) {
        let text = self.t(control.tooltip_key());
        let char_width = renderer.char_width();
        let width = text.chars().count() as f32 * char_width + 12.0;
        let (window_width, _) = renderer.size();
        let x = (rect.0 + rect.2 * 0.5 - width * 0.5).clamp(4.0, (window_width - width - 4.0).max(4.0));
        let y = title_height + 4.0;
        let height = renderer.line_height() + 8.0;
        renderer.draw_rect(x, y, width, height, rgb(Theme::color(Theme::DIM, 0x454545)));
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, rgb(Theme::color(Theme::SIDEBAR, 0x252526)));
        renderer.draw_text(text, x + 6.0, y + 4.0, rgb(Theme::color(Theme::TEXT, 0xd4d4d4)));
    }
}

/// Icon of one button: an outlined window with the active part filled, or a 2x2 grid for the
/// customize button. Drawn with rectangles so it does not depend on an icon font.
fn draw_icon(renderer: &mut Renderer, control: LayoutControl, rect: Rect, active: bool, foreground: Color, background: Color) {
    let (x, y, width, height) = rect;
    let cx = x + width * 0.5;
    let cy = y + height * 0.5;
    if control == LayoutControl::Customize {
        for (dx, dy) in [(-4.5, -4.5), (1.0, -4.5), (-4.5, 1.0), (1.0, 1.0)] {
            draw_outline(renderer, cx + dx, cy + dy, 3.5, 3.5, foreground);
        }
        return;
    }
    let (w, h) = (14.0, 11.0);
    let (ox, oy) = (cx - w * 0.5, cy - h * 0.5);
    draw_outline(renderer, ox, oy, w, h, foreground);
    let fill = if active { foreground } else { background };
    match control {
        LayoutControl::PrimarySidebar => renderer.draw_rect(ox + 1.0, oy + 1.0, 4.0, h - 2.0, fill),
        LayoutControl::SecondarySidebar => renderer.draw_rect(ox + w - 5.0, oy + 1.0, 4.0, h - 2.0, fill),
        LayoutControl::Panel => renderer.draw_rect(ox + 1.0, oy + h - 4.0, w - 2.0, 3.0, fill),
        LayoutControl::Customize => {}
    }
}

fn draw_outline(renderer: &mut Renderer, x: f32, y: f32, width: f32, height: f32, color: Color) {
    renderer.draw_rect(x, y, width, 1.0, color);
    renderer.draw_rect(x, y + height - 1.0, width, 1.0, color);
    renderer.draw_rect(x, y, 1.0, height, color);
    renderer.draw_rect(x + width - 1.0, y, 1.0, height, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_sit_left_of_the_window_controls_in_order() {
        let rects = layout_control_rects(900.0, 30.0, 1.0);
        let controls: Vec<LayoutControl> = rects.iter().map(|(control, _)| *control).collect();
        assert_eq!(controls, LAYOUT_CONTROLS.to_vec());
        let last = rects[3].1;
        assert!(last.0 + last.2 <= 900.0);
        for pair in rects.windows(2) {
            assert!((pair[1].1 .0 - (pair[0].1 .0 + pair[0].1 .2)).abs() < 0.01);
        }
    }

    #[test]
    fn a_click_finds_the_button_under_it_and_misses_the_gaps_outside() {
        let rects = layout_control_rects(900.0, 30.0, 1.0);
        let panel = rects[2].1;
        assert_eq!(layout_control_at(&rects, panel.0 + 2.0, 5.0), Some(LayoutControl::Panel));
        assert_eq!(layout_control_at(&rects, 10.0, 5.0), None);
    }

    #[test]
    fn every_button_names_its_command_and_tooltip() {
        assert_eq!(LayoutControl::Panel.command(), "graphite.layout.togglePanel");
        assert_eq!(LayoutControl::Customize.tooltip_key(), "titlebar.layout.customize");
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn each_toggle_command_flips_its_panel_and_keeps_the_others() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let panel = app.view_preferences.panel_visible;
        let primary = app.view_preferences.primary_sidebar_visible;
        let secondary = app.view_preferences.secondary_sidebar_visible;

        app.run_ui_command("graphite.layout.togglePanel");
        assert_eq!(app.view_preferences.panel_visible, !panel);
        assert_eq!(app.view_preferences.primary_sidebar_visible, primary);

        app.run_ui_command("graphite.layout.togglePrimarySidebar");
        assert_eq!(app.view_preferences.primary_sidebar_visible, !primary);

        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        assert_eq!(app.view_preferences.secondary_sidebar_visible, !secondary);
        assert_eq!(app.layout_control_active(LayoutControl::Panel), !panel);
    }
}
