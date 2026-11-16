//! Commands for the parts of the window: closing the side bars and the panel, moving the focus
//! between parts, and stepping through the side bar views.

use super::*;
use crate::app::focus_regions::FocusRegion;

fn close_side_bar(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.primary_sidebar_visible = false;
    app.save_view_preferences_for_command();
    Ok(())
}

fn close_panel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.panel_visible = false;
    app.save_view_preferences_for_command();
    Ok(())
}

fn close_auxiliary_bar(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.secondary_sidebar_visible = false;
    app.save_view_preferences_for_command();
    Ok(())
}

/// Shows the side bar if it is hidden and moves the keyboard focus into it.
fn focus_side_bar(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if !app.view_preferences.primary_sidebar_visible {
        app.view_preferences.primary_sidebar_visible = true;
        app.save_view_preferences_for_command();
    }
    app.focus_region(FocusRegion::Explorer);
    Ok(())
}

/// Shows the panel with the terminal and moves the keyboard focus into it.
fn focus_panel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if !app.view_preferences.panel_visible {
        app.view_preferences.panel_visible = true;
        app.save_view_preferences_for_command();
    }
    app.terminal_visible = true;
    app.focus_region(FocusRegion::Terminal);
    Ok(())
}

fn focus_next_part(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cycle_focus_region(false);
    Ok(())
}

fn focus_previous_part(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cycle_focus_region(true);
    Ok(())
}

fn next_side_bar_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cycle_side_bar_view(false);
    Ok(())
}

fn previous_side_bar_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cycle_side_bar_view(true);
    Ok(())
}

fn toggle_full_screen(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_full_screen();
    Ok(())
}

fn show_about(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_help_modal(crate::app::help_menu::HelpPage::About);
    Ok(())
}

fn toggle_maximized_panel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if !app.view_preferences.panel_visible {
        app.view_preferences.panel_visible = true;
    }
    app.view_preferences.panel_height_delta = if app.view_preferences.panel_height_delta > 0 {
        0
    } else {
        220
    };
    app.save_view_preferences_for_command();
    app.resize_terminal();
    Ok(())
}

fn cycle_panel_view(app: &mut App, reverse: bool) {
    app.view_preferences.panel_visible = true;
    app.terminal_visible = true;
    // Order: 0 = Problems, 1 = Output, 2 = DebugConsole, 3 = Terminal, 4 = Ports
    let current = if app.terminal_problems {
        0
    } else if app.debug_panel_visible {
        2
    } else {
        3
    };
    let next = if reverse {
        (current + 4) % 5
    } else {
        (current + 1) % 5
    };
    app.terminal_problems = false;
    app.terminal_focused = false;
    app.debug_panel_visible = false;
    match next {
        0 => app.terminal_problems = true,
        1 => app.run_ui_command("workbench.action.showOutputChannels"),
        2 => app.debug_panel_visible = true,
        3 => app.terminal_focused = true,
        _ => app.run_ui_command("graphite.ports.open"),
    }
    app.save_view_preferences_for_command();
}

fn next_panel_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    cycle_panel_view(app, false);
    Ok(())
}

fn previous_panel_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    cycle_panel_view(app, true);
    Ok(())
}

fn cycle_auxiliary_bar_view(app: &mut App) {
    use crate::app::chat::state::ChatSurface;
    app.view_preferences.secondary_sidebar_visible = true;
    let in_chat = app.chat.visible && app.chat.surface == ChatSurface::SideBar;
    if in_chat {
        app.chat.visible = false;
        app.chat.focused = false;
        app.outline_visible = true;
    } else {
        app.outline_visible = false;
        app.open_chat_sidebar();
    }
    app.save_view_preferences_for_command();
}

fn next_auxiliary_bar_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    cycle_auxiliary_bar_view(app);
    Ok(())
}

fn change_eol(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_ui_command("graphite.editor.toggleLineEnding");
    Ok(())
}

fn change_encoding(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_ui_command("graphite.editor.nextEncoding");
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, &'static str, Handler); 19] = [
        (
            "workbench.action.closeSidebar",
            "Close primary side bar",
            "View",
            close_side_bar,
        ),
        (
            "workbench.action.closePanel",
            "Close panel",
            "View",
            close_panel,
        ),
        (
            "workbench.action.closeAuxiliaryBar",
            "Close secondary side bar",
            "View",
            close_auxiliary_bar,
        ),
        (
            "workbench.action.focusSideBar",
            "Focus into primary side bar",
            "View",
            focus_side_bar,
        ),
        (
            "workbench.action.focusPanel",
            "Focus into panel",
            "View",
            focus_panel,
        ),
        (
            "workbench.action.focusNextPart",
            "Focus next part",
            "View",
            focus_next_part,
        ),
        (
            "workbench.action.focusPreviousPart",
            "Focus previous part",
            "View",
            focus_previous_part,
        ),
        (
            "workbench.action.nextSideBarView",
            "Next primary side bar view",
            "View",
            next_side_bar_view,
        ),
        (
            "workbench.action.previousSideBarView",
            "Previous primary side bar view",
            "View",
            previous_side_bar_view,
        ),
        (
            "workbench.action.toggleFullScreen",
            "Toggle full screen",
            "View",
            toggle_full_screen,
        ),
        (
            "workbench.action.toggleCenteredLayout",
            "Toggle centered layout",
            "View",
            super::workbench::workbench_action_toggle_centered_editor,
        ),
        (
            "workbench.action.showAboutDialog",
            "About",
            "Help",
            show_about,
        ),
        (
            "workbench.action.toggleMaximizedPanel",
            "Toggle Maximized Panel",
            "View",
            toggle_maximized_panel,
        ),
        (
            "workbench.action.nextPanelView",
            "Next Panel View",
            "View",
            next_panel_view,
        ),
        (
            "workbench.action.previousPanelView",
            "Previous Panel View",
            "View",
            previous_panel_view,
        ),
        (
            "workbench.action.nextAuxiliaryBarView",
            "Next Secondary Side Bar View",
            "View",
            next_auxiliary_bar_view,
        ),
        (
            "workbench.action.previousAuxiliaryBarView",
            "Previous Secondary Side Bar View",
            "View",
            next_auxiliary_bar_view,
        ),
        (
            "workbench.action.editor.changeEOL",
            "Change End of Line Sequence",
            "File",
            change_eol,
        ),
        (
            "workbench.action.editor.changeEncoding",
            "Change File Encoding",
            "File",
            change_encoding,
        ),
    ];
    for (id, title, category, handler) in commands {
        add(registry, id, title, category, ALL, "true", None, handler);
    }
    // The same navigation as Navigate Back and Navigate Forward, under their other ids.
    for (id, title, handler) in [
        (
            "workbench.action.navigateBackInNavigationLocations",
            "Navigate back in navigation locations",
            super::workbench::workbench_action_navigate_back as Handler,
        ),
        (
            "workbench.action.navigatePreviousInNavigationLocations",
            "Navigate to the previous navigation location",
            super::workbench::workbench_action_navigate_back as Handler,
        ),
        (
            "workbench.action.navigateForwardInNavigationLocations",
            "Navigate forward in navigation locations",
            super::workbench::workbench_action_navigate_forward as Handler,
        ),
    ] {
        add(registry, id, title, "Go", ALL, "true", None, handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::activity_bar::Activity;
    use crate::test_support::TempDir;

    fn app() -> (App, TempDir) {
        let dir = TempDir::new();
        (App::new_with_options(dir.path().into(), None, true), dir)
    }

    #[test]
    fn the_close_commands_hide_their_parts() {
        let (mut app, _dir) = app();
        app.run_ui_command("workbench.action.closeSidebar");
        assert!(!app.view_preferences.primary_sidebar_visible);
        app.run_ui_command("workbench.action.closePanel");
        assert!(!app.view_preferences.panel_visible);
        app.run_ui_command("workbench.action.closeAuxiliaryBar");
        assert!(!app.view_preferences.secondary_sidebar_visible);
    }

    #[test]
    fn focusing_a_hidden_part_shows_it_first() {
        let (mut app, _dir) = app();
        app.view_preferences.primary_sidebar_visible = false;
        app.run_ui_command("workbench.action.focusSideBar");
        assert!(app.view_preferences.primary_sidebar_visible);
        assert!(app.explorer_focused);

        app.view_preferences.panel_visible = false;
        app.terminal_visible = false;
        app.run_ui_command("workbench.action.focusPanel");
        assert!(app.view_preferences.panel_visible && app.terminal_visible);
        assert!(app.terminal_focused && !app.explorer_focused);
    }

    #[test]
    fn next_and_previous_part_cycle_the_focus() {
        let (mut app, _dir) = app();
        app.terminal_visible = true;
        app.view_preferences.panel_visible = true;
        app.view_preferences.primary_sidebar_visible = true;
        app.focus_region(FocusRegion::Editor);
        app.run_ui_command("workbench.action.focusNextPart");
        assert_eq!(app.current_focus_region(), FocusRegion::Terminal);
        app.run_ui_command("workbench.action.focusNextPart");
        assert_eq!(app.current_focus_region(), FocusRegion::Explorer);
        app.run_ui_command("workbench.action.focusPreviousPart");
        assert_eq!(app.current_focus_region(), FocusRegion::Terminal);
    }

    #[test]
    fn side_bar_views_step_in_order_and_wrap() {
        let (mut app, _dir) = app();
        assert_eq!(app.active_activity(), Activity::Explorer);
        app.run_ui_command("workbench.action.nextSideBarView");
        assert_eq!(app.active_activity(), Activity::Search);
        app.run_ui_command("workbench.action.previousSideBarView");
        assert_eq!(app.active_activity(), Activity::Explorer);
        app.run_ui_command("workbench.action.previousSideBarView");
        assert_eq!(app.active_activity(), Activity::Plugins);
        app.run_ui_command("workbench.action.nextSideBarView");
        assert_eq!(app.active_activity(), Activity::Explorer);
    }

    #[test]
    fn stepping_shows_a_hidden_side_bar_and_skips_hidden_views() {
        let (mut app, _dir) = app();
        app.view_preferences.primary_sidebar_visible = false;
        app.activity_bar_preferences
            .hidden_items
            .insert("search".to_owned());
        app.run_ui_command("workbench.action.nextSideBarView");
        assert!(app.view_preferences.primary_sidebar_visible);
        assert_eq!(app.active_activity(), Activity::SourceControl);
    }

    #[test]
    fn the_navigation_ids_and_about_are_registered() {
        for id in [
            "workbench.action.navigateBackInNavigationLocations",
            "workbench.action.navigatePreviousInNavigationLocations",
            "workbench.action.navigateForwardInNavigationLocations",
            "workbench.action.toggleCenteredLayout",
            "workbench.action.toggleFullScreen",
            "workbench.action.showAboutDialog",
            "workbench.action.toggleMaximizedPanel",
            "workbench.action.nextPanelView",
            "workbench.action.previousPanelView",
            "workbench.action.nextAuxiliaryBarView",
            "workbench.action.previousAuxiliaryBarView",
            "workbench.action.editor.changeEOL",
            "workbench.action.editor.changeEncoding",
            "workbench.action.setNotificationsPosition.bottomLeft",
            "workbench.action.setNotificationsPosition.bottomRight",
            "workbench.action.setNotificationsPosition.topRight",
        ] {
            assert!(
                crate::app::command_registry::registry().get(id).is_some(),
                "{id}"
            );
        }
    }

    #[test]
    fn toggle_maximized_panel_and_cycle_views_work() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.view_preferences.panel_height_delta = 0;

        app.run_ui_command("workbench.action.toggleMaximizedPanel");
        assert!(app.view_preferences.panel_visible);
        assert!(app.view_preferences.panel_height_delta > 0);

        app.run_ui_command("workbench.action.toggleMaximizedPanel");
        assert_eq!(app.view_preferences.panel_height_delta, 0);

        app.terminal_problems = true;
        app.run_ui_command("workbench.action.previousPanelView");
        assert!(!app.terminal_problems);

        app.view_preferences.secondary_sidebar_visible = false;
        app.outline_visible = false;
        app.run_ui_command("workbench.action.nextAuxiliaryBarView");
        assert!(app.view_preferences.secondary_sidebar_visible);
        assert!(app.chat.visible);

        app.run_ui_command("workbench.action.previousAuxiliaryBarView");
        assert!(app.outline_visible);

        app.run_ui_command("workbench.action.setNotificationsPosition.bottomLeft");
        assert_eq!(
            app.view_preferences.notifications_position,
            crate::config::NotificationPosition::BottomLeft
        );
        app.run_ui_command("workbench.action.setNotificationsPosition.topRight");
        assert_eq!(
            app.view_preferences.notifications_position,
            crate::config::NotificationPosition::TopRight
        );
        app.run_ui_command("workbench.action.setNotificationsPosition.bottomRight");
        assert_eq!(
            app.view_preferences.notifications_position,
            crate::config::NotificationPosition::BottomRight
        );

        let before_eol = app.documents[app.active_document()].line_ending;
        app.run_ui_command("workbench.action.editor.changeEOL");
        assert_ne!(app.documents[app.active_document()].line_ending, before_eol);

        let before_enc = app.documents[app.active_document()].encoding;
        app.run_ui_command("workbench.action.editor.changeEncoding");
        assert_ne!(app.documents[app.active_document()].encoding, before_enc);
    }
}
