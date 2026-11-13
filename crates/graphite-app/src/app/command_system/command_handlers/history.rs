//! Local history commands: pick, restore, compare, create and delete saved versions.

use super::*;
use crate::app::local_history::HistoryAction;

fn history_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_local_history();
    Ok(())
}

fn history_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_history_picker(HistoryAction::Restore);
    Ok(())
}

fn history_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_history_picker(HistoryAction::Compare);
    Ok(())
}

fn history_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.compare_with_previous_version();
    Ok(())
}

fn history_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.create_history_entry();
    Ok(())
}

fn history_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_history_picker(HistoryAction::Delete);
    Ok(())
}

fn history_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.delete_all_history();
    Ok(())
}

fn history_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.restore_open_history_version();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 8] = [
        (
            "workbench.action.localHistory.open",
            "Find local history entry to open",
            history_0,
        ),
        (
            "workbench.action.localHistory.restoreViaPicker",
            "Find local history entry to restore",
            history_1,
        ),
        (
            "workbench.action.localHistory.compareWithFile",
            "Compare local history entry with the file",
            history_2,
        ),
        (
            "workbench.action.localHistory.compareWithPrevious",
            "Compare file with the latest local history entry",
            history_3,
        ),
        (
            "workbench.action.localHistory.create",
            "Create local history entry",
            history_4,
        ),
        (
            "workbench.action.localHistory.delete",
            "Delete local history entry",
            history_5,
        ),
        (
            "workbench.action.localHistory.deleteAll",
            "Delete all local history of the file",
            history_6,
        ),
        (
            "workbench.action.localHistory.restoreViaEditor",
            "Restore the open local history entry",
            history_7,
        ),
    ];
    for (id, title, handler) in commands {
        add(
            registry,
            id,
            title,
            "File",
            ALL,
            "documentHasPath",
            None,
            handler,
        );
    }
}
