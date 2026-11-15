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
