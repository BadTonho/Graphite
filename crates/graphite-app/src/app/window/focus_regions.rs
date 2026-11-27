//! Keyboard focus between the main regions of the window (F6 and Shift+F6).

use super::*;

/// A region that can receive keyboard focus, in the order F6 visits them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusRegion {
    Editor,
    Terminal,
    Explorer,
    Chat,
    ActivityBar,
    StatusBar,
}

impl FocusRegion {
    fn label_key(self) -> &'static str {
        match self {
            FocusRegion::Editor => "focus.region.editor",
            FocusRegion::Terminal => "focus.region.terminal",
            FocusRegion::Explorer => "focus.region.explorer",
            FocusRegion::Chat => "focus.region.chat",
            FocusRegion::ActivityBar => "focus.region.activity_bar",
            FocusRegion::StatusBar => "focus.region.status_bar",
        }
    }
}

/// The region after `current` in `available` (or before it when `reverse`), wrapping around.
/// A `current` that is not available moves to the first region.
pub(crate) fn next_region(
    available: &[FocusRegion],
    current: FocusRegion,
    reverse: bool,
) -> FocusRegion {
    let Some(position) = available.iter().position(|region| *region == current) else {
        return available.first().copied().unwrap_or(FocusRegion::Editor);
    };
    let len = available.len();
    let next = if reverse {
        (position + len - 1) % len
    } else {
        (position + 1) % len
    };
    available[next]
}

impl App {
    /// Regions that are visible right now, in F6 order: editor, terminal, explorer.
    pub(crate) fn available_focus_regions(&self) -> Vec<FocusRegion> {
        if self.ai_mode.is_some() {
            let mut regions = vec![FocusRegion::Chat];
            if self.ai_file_group().is_some() {
                regions.push(FocusRegion::Editor);
            }
            if self.terminal_visible {
                regions.push(FocusRegion::Terminal);
            }
            return regions;
        }
        let mut regions = vec![FocusRegion::Editor];
        if self.terminal_visible && self.view_preferences.panel_visible {
            regions.push(FocusRegion::Terminal);
        }
        if self.view_preferences.primary_sidebar_visible {
            regions.push(FocusRegion::Explorer);
        }
        if self.chat.visible {
            regions.push(FocusRegion::Chat);
        }
        if self.activity_bar_preferences.position != crate::config::ActivityBarPosition::Hidden
            && !self.zen_mode
        {
            regions.push(FocusRegion::ActivityBar);
        }
        if self.view_preferences.status_bar_visible && !self.zen_mode {
            regions.push(FocusRegion::StatusBar);
        }
        regions
    }

    pub(crate) fn current_focus_region(&self) -> FocusRegion {
        if self.chat.focused && self.chat.visible {
            FocusRegion::Chat
        } else if self.terminal_focused {
            FocusRegion::Terminal
        } else if self.explorer_focused {
            FocusRegion::Explorer
        } else if self.activity_bar_focused {
            FocusRegion::ActivityBar
        } else if self.status_bar_focused {
            FocusRegion::StatusBar
        } else {
            FocusRegion::Editor
        }
    }

    /// Moves keyboard focus to the next (or previous) visible region and says where it went.
    pub(crate) fn cycle_focus_region(&mut self, reverse: bool) {
        let available = self.available_focus_regions();
        let region = next_region(&available, self.current_focus_region(), reverse);
        self.focus_region_and_announce(region);
    }

    pub(crate) fn focus_activity_bar(&mut self) {
        self.focus_region_and_announce(FocusRegion::ActivityBar);
    }

    pub(crate) fn focus_status_bar(&mut self) {
        self.focus_region_and_announce(FocusRegion::StatusBar);
    }

    fn focus_region_and_announce(&mut self, region: FocusRegion) {
        self.focus_region(region);
        self.status = self.tf(
            "focus.region.changed",
            &[("region", &self.t(region.label_key()))],
        );
    }

    pub(crate) fn focus_region(&mut self, region: FocusRegion) {
        self.terminal_focused = false;
        self.explorer_focused = false;
        self.chat.focused = false;
        self.activity_bar_focused = false;
        self.status_bar_focused = false;
        match region {
            FocusRegion::Editor => {}
            FocusRegion::Terminal => self.terminal_focused = true,
            FocusRegion::Explorer => self.explorer_focused = true,
            FocusRegion::Chat => self.chat.focused = true,
            FocusRegion::ActivityBar => self.activity_bar_focused = true,
            FocusRegion::StatusBar => self.status_bar_focused = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f6_visits_only_the_visible_regions_and_wraps_both_ways() {
        let all = [
            FocusRegion::Editor,
            FocusRegion::Terminal,
            FocusRegion::Explorer,
        ];
        assert_eq!(
            next_region(&all, FocusRegion::Editor, false),
            FocusRegion::Terminal
        );
        assert_eq!(
            next_region(&all, FocusRegion::Explorer, false),
            FocusRegion::Editor
        );
        assert_eq!(
            next_region(&all, FocusRegion::Editor, true),
            FocusRegion::Explorer
        );
        let editor_only = [FocusRegion::Editor];
        assert_eq!(
            next_region(&editor_only, FocusRegion::Editor, false),
            FocusRegion::Editor
        );
        // A region that was just hidden goes back to the first visible one.
        let no_terminal = [FocusRegion::Editor, FocusRegion::Explorer];
        assert_eq!(
            next_region(&no_terminal, FocusRegion::Terminal, false),
            FocusRegion::Editor
        );
    }

    #[test]
    fn cycling_updates_the_focus_flags_and_reports_the_region() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.terminal_visible = true;
        app.view_preferences.panel_visible = true;
        app.view_preferences.primary_sidebar_visible = true;
        app.activity_bar_preferences.position = crate::config::ActivityBarPosition::Hidden;
        app.view_preferences.status_bar_visible = false;

        assert_eq!(app.current_focus_region(), FocusRegion::Editor);
        app.cycle_focus_region(false);
        assert!(app.terminal_focused && !app.explorer_focused);
        assert!(app.status.contains(app.t("focus.region.terminal")));
        app.cycle_focus_region(false);
        assert!(app.explorer_focused && !app.terminal_focused);
        app.cycle_focus_region(false);
        assert_eq!(app.current_focus_region(), FocusRegion::Editor);
        app.cycle_focus_region(true);
        assert_eq!(app.current_focus_region(), FocusRegion::Explorer);
    }

    #[test]
    fn activity_bar_and_status_bar_receive_focus_when_visible() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.view_preferences.primary_sidebar_visible = false;
        app.view_preferences.panel_visible = false;
        app.activity_bar_preferences.position = crate::config::ActivityBarPosition::Default;
        app.view_preferences.status_bar_visible = true;

        app.run_ui_command("workbench.action.focusActivityBar");
        assert_eq!(app.current_focus_region(), FocusRegion::ActivityBar);
        assert!(app.activity_bar_focused);

        app.run_ui_command("workbench.action.focusStatusBar");
        assert_eq!(app.current_focus_region(), FocusRegion::StatusBar);
        assert!(app.status_bar_focused && !app.activity_bar_focused);
    }
}
