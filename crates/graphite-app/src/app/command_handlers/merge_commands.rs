//! Merge editor commands.

use super::*;
use crate::app::merge_editor::{BasePlacement, MergeLayout};

const OPEN: &str = "mergeEditorVisible";

fn open_editor(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(path) = app.documents[app.active_document()]
        .buffer
        .path()
        .map(Path::to_path_buf)
    else {
        return Err(CommandError::Failed(app.t("merge.error.no_file").into()));
    };
    app.open_merge_editor(path).map_err(CommandError::Failed)
}

fn close_editor(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_merge_editor();
    Ok(())
}

fn column_layout(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_set_layout(MergeLayout::Column);
    Ok(())
}

fn mixed_layout(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_set_layout(MergeLayout::Mixed);
    Ok(())
}

fn show_base(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_toggle_base(BasePlacement::Top);
    Ok(())
}

fn show_base_top(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_toggle_base(BasePlacement::Top);
    Ok(())
}

fn show_base_center(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_toggle_base(BasePlacement::Center);
    Ok(())
}

fn toggle_unchanged(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_toggle_unchanged();
    Ok(())
}

fn next_conflict(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_goto(true);
    Ok(())
}

fn previous_conflict(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_goto(false);
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    add(
        registry,
        "graphite.merge.open",
        "Open Merge Editor",
        "Merge",
        ALL,
        "true",
        None,
        open_editor,
    );
    let commands: [(&'static str, &'static str, Handler); 9] = [
        ("graphite.merge.close", "Close Merge Editor", close_editor),
        ("merge.columnLayout", "Column Layout", column_layout),
        ("merge.mixedLayout", "Mixed Layout", mixed_layout),
        ("merge.showBase", "Show Base", show_base),
        ("merge.showBaseTop", "Show Base at the Top", show_base_top),
        (
            "merge.showBaseCenter",
            "Show Base in the Center",
            show_base_center,
        ),
        (
            "merge.showNonConflictingChanges",
            "Show Non-Conflicting Changes",
            toggle_unchanged,
        ),
        (
            "merge.goToNextUnhandledConflict",
            "Go to Next Unhandled Conflict",
            next_conflict,
        ),
        (
            "merge.goToPreviousUnhandledConflict",
            "Go to Previous Unhandled Conflict",
            previous_conflict,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Merge", ALL, OPEN, None, handler);
    }
}
