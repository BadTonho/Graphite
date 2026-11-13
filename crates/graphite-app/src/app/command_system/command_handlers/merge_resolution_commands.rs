//! Merge editor commands that resolve the whole file.

use super::*;

const OPEN: &str = "mergeEditorVisible";

fn accept_all_input1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_accept_all(|state| state.accept_all_current());
    Ok(())
}

fn accept_all_input2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_accept_all(|state| state.accept_all_incoming());
    Ok(())
}

fn accept_all_combination(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_accept_all(|state| state.accept_all_both());
    Ok(())
}

fn reset_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_reset_all();
    Ok(())
}

fn reset_close_choice(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_reset_close_choice();
    Ok(())
}

fn accept_merge(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_complete().map_err(CommandError::Failed)
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 6] = [
        (
            "merge.acceptAllInput1",
            "Accept All from Current",
            accept_all_input1,
        ),
        (
            "merge.acceptAllInput2",
            "Accept All from Incoming",
            accept_all_input2,
        ),
        (
            "mergeEditor.acceptAllCombination",
            "Accept All, Both Sides",
            accept_all_combination,
        ),
        (
            "mergeEditor.resetResultToBaseAndAutoMerge",
            "Reset the Result",
            reset_all,
        ),
        (
            "mergeEditor.resetCloseWithConflictsChoice",
            "Reset the Close Warning",
            reset_close_choice,
        ),
        ("mergeEditor.acceptMerge", "Complete Merge", accept_merge),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Merge", ALL, OPEN, None, handler);
    }
}
