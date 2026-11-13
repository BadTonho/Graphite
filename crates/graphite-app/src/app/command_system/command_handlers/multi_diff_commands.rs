//! Multi-file diff commands.

use super::*;
use crate::scm_changes::ChangeSet;

const OPEN: &str = "multiDiffVisible";

fn open_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_multi_diff(ChangeSet::All);
    Ok(())
}

fn open_staged(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_multi_diff(ChangeSet::Staged);
    Ok(())
}

fn open_unstaged(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_multi_diff(ChangeSet::Unstaged);
    Ok(())
}

fn close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_multi_diff();
    Ok(())
}

fn toggle_file(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_toggle_current_file();
    Ok(())
}

fn collapse_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_collapse(true);
    Ok(())
}

fn expand_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_collapse(false);
    Ok(())
}

fn focus_file_list(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_toggle_list();
    Ok(())
}

fn go_to_file(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_go_to_file();
    Ok(())
}

fn next_change(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_step(true);
    Ok(())
}

fn previous_change(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.multi_diff_step(false);
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let openers: [(&'static str, &'static str, Handler); 3] = [
        ("graphite.scm.openAllChanges", "Open All Changes", open_all),
        (
            "graphite.scm.openAllStagedChanges",
            "Open All Staged Changes",
            open_staged,
        ),
        (
            "graphite.scm.openAllUnstagedChanges",
            "Open All Unstaged Changes",
            open_unstaged,
        ),
    ];
    for (id, title, handler) in openers {
        add(registry, id, title, "Multi-Diff", ALL, "true", None, handler);
    }
    let commands: [(&'static str, &'static str, Handler); 8] = [
        ("graphite.multiDiff.close", "Close Multi-File Diff", close),
        (
            "graphite.multiDiff.toggleFile",
            "Collapse or Expand the Selected File",
            toggle_file,
        ),
        (
            "multiDiffEditor.collapseAll",
            "Collapse All Files",
            collapse_all,
        ),
        ("multiDiffEditor.expandAll", "Expand All Files", expand_all),
        (
            "multiDiffEditor.focusFileList",
            "Focus the File List",
            focus_file_list,
        ),
        ("multiDiffEditor.goToFile", "Open the File", go_to_file),
        (
            "multiDiffEditor.goToNextChange",
            "Go to Next Change",
            next_change,
        ),
        (
            "multiDiffEditor.goToPreviousChange",
            "Go to Previous Change",
            previous_change,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Multi-Diff", ALL, OPEN, None, handler);
    }
}
