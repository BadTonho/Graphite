//! Commands for the quick open widgets and the recently used editor lists.

use super::*;

fn quick_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_editor_history_picker(false, false);
    Ok(())
}

fn quick_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_editor_history_picker(false, false);
    Ok(())
}

fn quick_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_editor_history_picker(false, true);
    Ok(())
}

fn quick_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_editor_history_picker(true, false);
    Ok(())
}

fn quick_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_editor_history_picker(true, true);
    Ok(())
}

fn quick_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.accept_quick_widget();
    Ok(())
}

fn quick_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.accept_quick_widget_permanently();
    Ok(())
}

fn quick_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_quick_widgets();
    Ok(())
}

fn quick_8(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_quick_widgets();
    Ok(())
}

fn quick_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.start_quick_open();
    Ok(())
}

fn quick_10(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.switch_go_editor(false, false, true);
    Ok(())
}

fn quick_11(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.switch_go_editor(true, false, true);
    Ok(())
}

fn quick_12(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.switch_go_editor(false, true, true);
    Ok(())
}

fn quick_13(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.switch_go_editor(true, true, true);
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 14] = [
        (
            "workbench.action.quickOpenPreviousRecentlyUsedEditor",
            "Quick open previous editor from history",
            quick_0,
        ),
        (
            "workbench.action.quickOpenPreviousEditor",
            "Quick open previous editor",
            quick_1,
        ),
        (
            "workbench.action.quickOpenLeastRecentlyUsedEditor",
            "Quick open least recently used editor",
            quick_2,
        ),
        (
            "workbench.action.quickOpenPreviousRecentlyUsedEditorInGroup",
            "Quick open previous editor from history in group",
            quick_3,
        ),
        (
            "workbench.action.quickOpenLeastRecentlyUsedEditorInGroup",
            "Quick open least recently used editor in group",
            quick_4,
        ),
        (
            "workbench.action.acceptSelectedQuickOpenItem",
            "Accept the selected quick open item",
            quick_5,
        ),
        (
            "workbench.action.alternativeAcceptSelectedQuickOpenItem",
            "Open the selected quick open item in a permanent tab",
            quick_6,
        ),
        (
            "workbench.action.closeQuickOpen",
            "Close quick open",
            quick_7,
        ),
        (
            "workbench.action.focusQuickOpen",
            "Focus quick open",
            quick_8,
        ),
        (
            "workbench.action.quickOpenWithModes",
            "Quick open with modes",
            quick_9,
        ),
        (
            "workbench.action.openNextRecentlyUsedEditor",
            "Open next recently used editor",
            quick_10,
        ),
        (
            "workbench.action.openPreviousRecentlyUsedEditor",
            "Open previous recently used editor",
            quick_11,
        ),
        (
            "workbench.action.openNextRecentlyUsedEditorInGroup",
            "Open next recently used editor in group",
            quick_12,
        ),
        (
            "workbench.action.openPreviousRecentlyUsedEditorInGroup",
            "Open previous recently used editor in group",
            quick_13,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Go", ALL, "true", None, handler);
    }
}
