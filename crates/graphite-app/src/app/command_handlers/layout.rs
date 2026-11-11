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
}

fn customize_layout(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    let Some(width) = app.gpu.as_ref().map(|gpu| gpu.renderer.size().0) else {
        return Ok(());
    };
    let char_width = app.gpu.as_ref().map_or(app.font_size * 0.6, |gpu| gpu.renderer.char_width());
    let anchor = app
        .titlebar_layout(width, char_width)
        .layout_controls
        .iter()
        .find(|(control, _)| *control == LayoutControl::Customize)
        .map_or(0.0, |(_, rect)| rect.0);
    app.run_layout_control(LayoutControl::Customize, anchor);
    Ok(())
}

fn toggle_primary_sidebar(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.primary_sidebar_visible = !app.view_preferences.primary_sidebar_visible;
    app.save_view_preferences_for_command();
    Ok(())
}

fn toggle_panel(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.panel_visible = !app.view_preferences.panel_visible;
    app.save_view_preferences_for_command();
    Ok(())
}

fn toggle_secondary_sidebar(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.view_preferences.secondary_sidebar_visible = !app.view_preferences.secondary_sidebar_visible;
    app.save_view_preferences_for_command();
    Ok(())
}
