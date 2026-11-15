//! Source control and change navigation commands.

use super::*;

fn change_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_change(true);
    Ok(())
}

fn change_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_change(false);
    Ok(())
}

fn change_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_change(true);
    Ok(())
}

fn change_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_change(false);
    Ok(())
}

fn change_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_change(true);
    Ok(())
}

fn change_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_change(false);
    Ok(())
}

fn change_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_accept_input();
    Ok(())
}

fn change_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_clear_input();
    Ok(())
}

fn change_8(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_repository_in_terminal();
    Ok(())
}

fn change_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_repository_in_terminal();
    Ok(())
}

fn revert_selected_ranges(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.perform_scm_action(crate::app::scm_ui::ScmAction::RevertCursorLine);
    Ok(())
}

fn focus_resource_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_ui_command("workbench.view.scm");
    app.explorer_focused = true;
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, &'static str, Handler); 12] = [
        (
            "workbench.action.editor.nextChange",
            "Go to next change",
            "Go",
            change_0,
        ),
        (
            "workbench.action.editor.previousChange",
            "Go to previous change",
            "Go",
            change_1,
        ),
        ("workbench.action.nextChange", "Next change", "Go", change_2),
        (
            "workbench.action.previousChange",
            "Previous change",
            "Go",
            change_3,
        ),
        (
            "editor.action.dirtydiff.next",
            "Go to next change",
            "Editor",
            change_4,
        ),
        (
            "editor.action.dirtydiff.previous",
            "Go to previous change",
            "Editor",
            change_5,
        ),
        ("scm.acceptInput", "Commit staged changes", "Scm", change_6),
        ("scm.clearInput", "Clear commit message", "Scm", change_7),
        (
            "scm.openInTerminal",
            "Open repository in terminal",
            "Scm",
            change_8,
        ),
        (
            "scm.openInIntegratedTerminal",
            "Open repository in integrated terminal",
            "Scm",
            change_9,
        ),
        (
            "git.revertSelectedRanges",
            "Revert selected ranges",
            "Scm",
            revert_selected_ranges,
        ),
        (
            "scm.focusResourceGroup",
            "Focus source control resource group",
            "Scm",
            focus_resource_group,
        ),
    ];
    for (id, title, category, handler) in commands {
        add(registry, id, title, category, ALL, "true", None, handler);
    }
}
