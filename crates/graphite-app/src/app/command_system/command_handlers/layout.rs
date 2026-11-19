use super::*;
use crate::app::titlebar_layout_controls::LayoutControl;

pub(super) fn register(registry: &mut CommandRegistry) {
    add(
        registry,
        "graphite.layout.customize",
        "Customize Layout...",
        "Workbench",
        ALL,
        "true",
        None,
        customize_layout,
    );
    add(
        registry,
        "workbench.action.customizeLayout",
        "Customize layout",
        "View",
        ALL,
        "true",
        None,
        customize_layout,
    );
    add(
        registry,
        "graphite.layout.togglePrimarySidebar",
        "Toggle Primary Side Bar",
        "Workbench",
        ALL,
        "true",
        None,
        toggle_primary_sidebar,
    );
    add(
        registry,
        "graphite.layout.togglePanel",
        "Toggle Panel",
        "Workbench",
        ALL,
        "true",
        None,
        toggle_panel,
    );
    add(
        registry,
        "graphite.layout.toggleSecondarySidebar",
        "Toggle Secondary Side Bar",
        "Workbench",
        ALL,
        "true",
        None,
        toggle_secondary_sidebar,
    );
    add(
        registry,
        "workbench.action.increaseViewSize",
        "Increase current view size",
        "View",
        ALL,
        "true",
        None,
        increase_view_size,
    );
    add(
        registry,
        "workbench.action.decreaseViewSize",
        "Decrease current view size",
        "View",
        ALL,
        "true",
        None,
        decrease_view_size,
    );
    add(
        registry,
        "workbench.action.resetViewLocations",
        "Reset view locations",
        "View",
        ALL,
        "true",
        None,
        reset_view_locations,
    );
    add(
        registry,
        "notifications.togglePosition",
        "Toggle notifications position",
        "Notifications",
        ALL,
        "true",
        None,
        toggle_notifications_position,
    );
    add(
        registry,
        "workbench.action.setNotificationsPosition.bottomLeft",
        "Set Notifications Position: Bottom Left",
        "Notifications",
        ALL,
        "true",
        None,
        set_notifications_bottom_left,
    );
    add(
        registry,
        "workbench.action.setNotificationsPosition.bottomRight",
        "Set Notifications Position: Bottom Right",
        "Notifications",
        ALL,
        "true",
        None,
        set_notifications_bottom_right,
    );
    add(
        registry,
        "workbench.action.setNotificationsPosition.topRight",
        "Set Notifications Position: Top Right",
        "Notifications",
        ALL,
        "true",
        None,
        set_notifications_top_right,
    );
}

fn customize_layout(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    let Some(width) = app.gpu.as_ref().map(|gpu| gpu.renderer.size().0) else {
        return Ok(());
    };
    let char_width = app
        .gpu
        .as_ref()
        .map_or(app.font_size * 0.6, |gpu| gpu.renderer.char_width());
    let anchor = app
        .titlebar_layout(width, char_width)
        .layout_controls
        .iter()
        .find(|(control, _)| *control == LayoutControl::Customize)
        .map_or(0.0, |(_, rect)| rect.0);
    app.run_layout_control(LayoutControl::Customize, anchor);
    Ok(())
}

fn toggle_primary_sidebar(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.view_preferences.primary_sidebar_visible = !app.view_preferences.primary_sidebar_visible;
    app.save_view_preferences_for_command();
    Ok(())
}

fn toggle_panel(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.panel_visible = !app.view_preferences.panel_visible;
    app.save_view_preferences_for_command();
    Ok(())
}

fn toggle_secondary_sidebar(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_secondary_sidebar_visible(!app.view_preferences.secondary_sidebar_visible);
    app.save_view_preferences_for_command();
    Ok(())
}

fn adjust_view_size(app: &mut App, step: i16) {
    if app.terminal_focused {
        app.view_preferences.panel_height_delta =
            (app.view_preferences.panel_height_delta + step).clamp(-180, 800);
    } else {
        app.view_preferences.sidebar_width_delta =
            (app.view_preferences.sidebar_width_delta + step).clamp(-160, 320);
    }
    app.save_view_preferences_for_command();
}

fn increase_view_size(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    adjust_view_size(app, 24);
    Ok(())
}

fn decrease_view_size(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    adjust_view_size(app, -24);
    Ok(())
}

fn reset_view_locations(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.sidebar_width_delta = 0;
    app.view_preferences.panel_height_delta = 0;
    app.view_preferences.panel_position = crate::config::PanelPosition::Bottom;
    app.view_preferences.notifications_position = crate::config::NotificationPosition::BottomRight;
    app.save_view_preferences_for_command();
    Ok(())
}

fn toggle_notifications_position(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    use crate::config::NotificationPosition;
    app.view_preferences.notifications_position = match app.view_preferences.notifications_position
    {
        NotificationPosition::BottomRight => NotificationPosition::BottomLeft,
        NotificationPosition::BottomLeft => NotificationPosition::TopRight,
        NotificationPosition::TopRight => NotificationPosition::BottomRight,
    };
    app.save_view_preferences_for_command();
    Ok(())
}

fn set_notifications_position(app: &mut App, pos: crate::config::NotificationPosition) {
    app.view_preferences.notifications_position = pos;
    app.save_view_preferences_for_command();
}

fn set_notifications_bottom_left(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    set_notifications_position(app, crate::config::NotificationPosition::BottomLeft);
    Ok(())
}

fn set_notifications_bottom_right(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    set_notifications_position(app, crate::config::NotificationPosition::BottomRight);
    Ok(())
}

fn set_notifications_top_right(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    set_notifications_position(app, crate::config::NotificationPosition::TopRight);
    Ok(())
}
