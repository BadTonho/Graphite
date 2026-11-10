//! Activity bar on the far left of the workbench.
//!
//! The bar owns a fixed-width column; the Explorer, Source Control and Plugins
//! panels start after it. Geometry is intentionally independent of the font
//! metrics so hit testing and drawing share the same slots.

use super::*;

pub(crate) const ACTIVITY_BAR_ITEM_HEIGHT: f32 = 48.0;
const ACTIVITY_BAR_TOP: f32 = 8.0;
const ACTIVITY_BAR_ICON_SIZE: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Activity {
    Explorer,
    Search,
    SourceControl,
    Plugins,
}

impl Activity {
    pub(crate) const ALL: [Activity; 4] = [
        Activity::Explorer,
        Activity::Search,
        Activity::SourceControl,
        Activity::Plugins,
    ];

    pub(crate) fn glyph(self) -> char {
        match self {
            Activity::Explorer => '\u{eaf0}',      // cod-files
            Activity::Search => '\u{ea6d}',        // cod-search
            Activity::SourceControl => '\u{ea68}', // cod-source_control
            Activity::Plugins => '\u{eae6}',       // cod-extensions
        }
    }

    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Activity::Explorer => "activity.explorer",
            Activity::Search => "activity.search",
            Activity::SourceControl => "activity.source_control",
            Activity::Plugins => "activity.plugins",
        }
    }
}

/// Top edge of the item at `index`, in physical pixels.
pub(crate) fn activity_top(index: usize) -> f32 {
    ACTIVITY_BAR_TOP + index as f32 * ACTIVITY_BAR_ITEM_HEIGHT
}

/// Returns the item under `y`, or `None` outside every slot.
pub(crate) fn activity_at(y: f32) -> Option<Activity> {
    if y < ACTIVITY_BAR_TOP {
        return None;
    }
    let index = ((y - ACTIVITY_BAR_TOP) / ACTIVITY_BAR_ITEM_HEIGHT) as usize;
    Activity::ALL.get(index).copied()
}

impl App {
    /// The view represented by the activity bar's current selection.
    pub(crate) fn active_activity(&self) -> Activity {
        if self.plugins.visible {
            Activity::Plugins
        } else if self.search_query.is_some() {
            Activity::Search
        } else if self.scm_visible {
            Activity::SourceControl
        } else {
            Activity::Explorer
        }
    }

    pub(crate) fn activate_activity(&mut self, activity: Activity) {
        match activity {
            Activity::Explorer => {
                self.close_plugins();
                self.scm_visible = false;
                self.close_project_search();
            }
            Activity::Search => {
                self.close_plugins();
                if self.search_query.is_some() {
                    self.close_project_search();
                } else {
                    self.scm_visible = false;
                    self.open_project_search();
                }
            }
            Activity::SourceControl => {
                self.close_plugins();
                if self.scm_visible {
                    self.scm_visible = false;
                } else {
                    self.close_project_search();
                    self.scm_visible = true;
                    self.refresh_scm_async();
                }
            }
            Activity::Plugins => {
                if !self.plugins.visible {
                    self.open_plugins();
                } else if self.plugins.focused {
                    self.close_plugins();
                } else {
                    self.focus_plugins();
                }
            }
        }
    }

    /// Handles a click inside the bar. Returns `false` when `y` misses an item.
    pub(crate) fn activity_bar_click(&mut self, y: f32) -> bool {
        let Some(activity) = activity_at(y) else {
            return false;
        };
        self.activate_activity(activity);
        true
    }

    pub(crate) fn draw_activity_bar(&self, renderer: &mut Renderer, height: f32) {
        renderer.draw_rect(
            0.0,
            0.0,
            ACTIVITY_BAR_WIDTH,
            height,
            rgb(Theme::color(Theme::ACTIVITY_BAR, 0xe8e8e8)),
        );
        let active = self.active_activity();
        let hovered = if (self.mouse.x as f32) < ACTIVITY_BAR_WIDTH {
            activity_at(self.mouse.y as f32)
        } else {
            None
        };
        for (index, activity) in Activity::ALL.iter().copied().enumerate() {
            let top = activity_top(index);
            if top + ACTIVITY_BAR_ITEM_HEIGHT > height {
                break;
            }
            if hovered == Some(activity) && activity != active {
                renderer.draw_rect(
                    0.0,
                    top,
                    ACTIVITY_BAR_WIDTH,
                    ACTIVITY_BAR_ITEM_HEIGHT,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            if activity == active {
                renderer.draw_rect(
                    0.0,
                    top,
                    2.0,
                    ACTIVITY_BAR_ITEM_HEIGHT,
                    rgb(Theme::color(Theme::STATUS, 0x005a9e)),
                );
            }
            let color = if activity == active {
                Theme::color(Theme::TEXT, 0x1e1e1e)
            } else {
                Theme::color(Theme::DIM, 0x616161)
            };
            renderer.draw_icon(
                activity.glyph(),
                (ACTIVITY_BAR_WIDTH - ACTIVITY_BAR_ICON_SIZE) * 0.5,
                top + (ACTIVITY_BAR_ITEM_HEIGHT - ACTIVITY_BAR_ICON_SIZE) * 0.5,
                ACTIVITY_BAR_ICON_SIZE,
                rgb(color),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_testing_maps_each_vertical_slot() {
        assert_eq!(activity_at(activity_top(0) + 1.0), Some(Activity::Explorer));
        assert_eq!(activity_at(activity_top(1) + 1.0), Some(Activity::Search));
        assert_eq!(
            activity_at(activity_top(2) + ACTIVITY_BAR_ITEM_HEIGHT - 1.0),
            Some(Activity::SourceControl)
        );
        assert_eq!(activity_at(activity_top(3) + 1.0), Some(Activity::Plugins));
        assert_eq!(activity_at(ACTIVITY_BAR_TOP - 1.0), None);
        assert_eq!(activity_at(activity_top(4)), None);
        assert_eq!(activity_at(-10.0), None);
    }

    #[test]
    fn active_activity_prefers_overlays_over_the_explorer() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        assert_eq!(app.active_activity(), Activity::Explorer);
        app.scm_visible = true;
        assert_eq!(app.active_activity(), Activity::SourceControl);
        app.search_query = Some(String::new());
        assert_eq!(app.active_activity(), Activity::Search);
        app.plugins.visible = true;
        assert_eq!(app.active_activity(), Activity::Plugins);
    }

    #[test]
    fn clicking_switches_views_and_toggles_search() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);

        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(app.scm_visible);
        assert_eq!(app.active_activity(), Activity::SourceControl);

        assert!(app.activity_bar_click(activity_top(1) + 1.0));
        assert!(!app.scm_visible);
        assert!(app.search_query.is_some());
        assert_eq!(app.active_activity(), Activity::Search);

        assert!(app.activity_bar_click(activity_top(1) + 1.0));
        assert!(app.search_query.is_none());
        assert_eq!(app.active_activity(), Activity::Explorer);

        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(app.scm_visible);
        assert!(app.activity_bar_click(activity_top(0) + 1.0));
        assert!(!app.scm_visible);
        assert!(!app.activity_bar_click(activity_top(4)));
    }

    #[test]
    fn plugins_item_toggles_focus_and_other_views_close_it() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.use_plugin_store(temp.path().join("plugins"));

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.plugins.visible);
        assert!(app.plugins.focused);
        assert_eq!(app.active_activity(), Activity::Plugins);

        app.plugins.focused = false;
        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.plugins.visible);
        assert!(app.plugins.focused);

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(!app.plugins.visible);
        assert!(!app.plugins.focused);
        assert_eq!(app.active_activity(), Activity::Explorer);

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(!app.plugins.visible);
        assert!(app.scm_visible);

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.activity_bar_click(activity_top(1) + 1.0));
        assert!(!app.plugins.visible);
        assert!(app.search_query.is_some());
    }

    #[test]
    fn activity_bar_adds_to_the_existing_explorer_width() {
        assert_eq!(SIDEBAR_WIDTH - ACTIVITY_BAR_WIDTH, 260.0);
    }

    #[test]
    fn every_activity_glyph_exists_in_the_embedded_font() {
        for activity in Activity::ALL {
            assert!(
                graphite_render::icons::AVAILABLE_GLYPHS.contains(&activity.glyph()),
                "missing glyph U+{:04X} in the embedded icon font",
                activity.glyph() as u32
            );
        }
    }
}
