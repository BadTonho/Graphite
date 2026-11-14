//! Merge editor block actions as commands.

use super::*;

const OPEN: &str = "mergeEditorVisible";

fn accept_current(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(true, |state, block| state.accept_current(block));
    Ok(())
}

fn accept_incoming(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(true, |state, block| state.accept_incoming(block));
    Ok(())
}

fn accept_both(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(true, |state, block| state.accept_both(block, false));
    Ok(())
}

fn accept_both_incoming_first(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(true, |state, block| state.accept_both(block, true));
    Ok(())
}

fn ignore_conflict(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(true, |state, block| state.ignore(block));
    Ok(())
}

fn revert_conflict(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(false, |state, block| state.reset(block));
    Ok(())
}

fn toggle_input1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(false, |state, block| state.toggle_current(block));
    Ok(())
}

fn toggle_input2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_edit(false, |state, block| state.toggle_incoming(block));
    Ok(())
}

fn toggle_between_inputs(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_toggle_focused_input();
    Ok(())
}

fn compare_input1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_compare_with_base(0);
    Ok(())
}

fn compare_input2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_compare_with_base(1);
    Ok(())
}

fn open_base(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_open_base();
    Ok(())
}

fn open_result(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.merge_open_result();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 13] = [
        (
            "graphite.merge.acceptCurrent",
            "Accept Current",
            accept_current,
        ),
        (
            "graphite.merge.acceptIncoming",
            "Accept Incoming",
            accept_incoming,
        ),
        (
            "graphite.merge.acceptBoth",
            "Accept Both, Current First",
            accept_both,
        ),
        (
            "graphite.merge.acceptBothIncomingFirst",
            "Accept Both, Incoming First",
            accept_both_incoming_first,
        ),
        ("graphite.merge.ignore", "Ignore Conflict", ignore_conflict),
        (
            "graphite.merge.revert",
            "Revert Conflict to Unresolved",
            revert_conflict,
        ),
        (
            "merge.toggleActiveConflictInput1",
            "Toggle Current for the Active Conflict",
            toggle_input1,
        ),
        (
            "merge.toggleActiveConflictInput2",
            "Toggle Incoming for the Active Conflict",
            toggle_input2,
        ),
        (
            "mergeEditor.toggleBetweenInputs",
            "Toggle Between Inputs",
            toggle_between_inputs,
        ),
        (
            "mergeEditor.compareInput1WithBase",
            "Compare Current with Base",
            compare_input1,
        ),
        (
            "mergeEditor.compareInput2WithBase",
            "Compare Incoming with Base",
            compare_input2,
        ),
        ("merge.openBaseEditor", "Open Base", open_base),
        ("merge.openResult", "Open Result in the Editor", open_result),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Merge", ALL, OPEN, None, handler);
    }
}
