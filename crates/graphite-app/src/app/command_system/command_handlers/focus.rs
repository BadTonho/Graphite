use super::*;
use crate::app::theme::HIGH_CONTRAST;
use std::sync::atomic::Ordering;

pub(super) fn register(registry: &mut CommandRegistry) {
    add(
        registry,
        "workbench.action.focusNextRegion",
        "Focus next region",
        "Workbench",
        ALL,
        "true",
        None,
        focus_next_region,
    );
    add(
        registry,
        "workbench.action.focusPreviousRegion",
        "Focus previous region",
        "Workbench",
        ALL,
        "true",
        None,
        focus_previous_region,
    );
    add(
        registry,
        "workbench.action.focusActivityBar",
        "Focus activity bar",
        "Workbench",
        ALL,
        "true",
        None,
        focus_activity_bar,
    );
    add(
        registry,
        "workbench.action.focusStatusBar",
        "Focus status bar",
        "Workbench",
        ALL,
        "true",
        None,
        focus_status_bar,
    );
    add(
        registry,
        "workbench.action.toggleHighContrast",
        "Toggle high contrast",
        "Workbench",
        ALL,
        "true",
        None,
        toggle_high_contrast,
    );
    add(
        registry,
        "workbench.action.generateColorTheme",
        "Generate color theme from current settings",
        "Preferences",
        ALL,
        "true",
        None,
        generate_color_theme,
    );
}

fn focus_next_region(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.cycle_focus_region(false);
    Ok(())
}

fn focus_previous_region(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.cycle_focus_region(true);
    Ok(())
}

fn focus_activity_bar(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_activity_bar();
    Ok(())
}

fn focus_status_bar(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_status_bar();
    Ok(())
}

fn toggle_high_contrast(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let enabled = !HIGH_CONTRAST.load(Ordering::Relaxed);
    HIGH_CONTRAST.store(enabled, Ordering::Relaxed);
    crate::config::save_high_contrast_in(&app.profile_root, enabled);
    app.tree.refresh_icons();
    let state = app
        .t(if enabled { "common.on" } else { "common.off" })
        .to_owned();
    app.status = app.tf("theme.high_contrast.changed", &[("state", &state)]);
    Ok(())
}

fn generate_color_theme(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    use crate::app::theme::Theme;
    let hex = |c: u32| format!("#{c:06x}");
    let theme = serde_json::json!({
        "name": "Generated Color Theme",
        "colors": {
            "editor.background": hex(Theme::color(Theme::BG, 0xffffff)),
            "editor.foreground": hex(Theme::color(Theme::TEXT, 0x333333)),
            "sideBar.background": hex(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            "activityBar.background": hex(Theme::color(Theme::ACTIVITY_BAR, 0x2c2c2c)),
            "statusBar.background": hex(Theme::color(Theme::STATUS, 0x007acc)),
            "list.activeSelectionBackground": hex(Theme::color(Theme::ACTIVE_ROW, 0xe4e6f1)),
            "editorCursor.foreground": hex(Theme::color(Theme::CURSOR, 0x000000)),
        }
    });
    let json = serde_json::to_string_pretty(&theme).unwrap_or_default();
    app.new_untitled_document();
    app.editor_mut().insert(&json);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn generate_color_theme_opens_untitled_json_theme() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let initial_docs = app.documents.len();
        app.run_ui_command("workbench.action.generateColorTheme");
        assert_eq!(app.documents.len(), initial_docs + 1);
        let text = app.documents[app.command_document()].buffer.text();
        assert!(text.contains("\"editor.background\""));
    }
}
