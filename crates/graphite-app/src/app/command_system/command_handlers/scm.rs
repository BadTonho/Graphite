use super::*;

pub(super) fn graphite_scm_stage(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.perform_scm_action(crate::app::scm_ui::ScmAction::Stage);
    Ok(())
}

pub(super) fn graphite_scm_unstage(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.perform_scm_action(crate::app::scm_ui::ScmAction::Unstage);
    Ok(())
}

pub(super) fn graphite_scm_stage_line(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.perform_scm_action(crate::app::scm_ui::ScmAction::StageCursorLine)
    };
    Ok(())
}

pub(super) fn graphite_scm_revert_line(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.perform_scm_action(crate::app::scm_ui::ScmAction::RevertCursorLine)
    };
    Ok(())
}

pub(super) fn graphite_scm_revert_hunk(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.perform_scm_action(crate::app::scm_ui::ScmAction::RevertFirstHunk)
    };
    Ok(())
}

pub(super) fn graphite_scm_commit_prompt(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.scm_commit_prompt = true;
        app.scm_commit_message.clear();
        app.status = app.t("status.commit_prompt").into();
    };
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    register_git_commands(registry);
    add(
        registry,
        "graphite.scm.stage",
        "Stage",
        "Scm",
        ALL,
        "true",
        None,
        graphite_scm_stage,
    );
    add(
        registry,
        "graphite.scm.unstage",
        "Unstage",
        "Scm",
        ALL,
        "true",
        None,
        graphite_scm_unstage,
    );
    add(
        registry,
        "graphite.scm.stageLine",
        "Stage line",
        "Scm",
        VIEW,
        "documentHasPath",
        None,
        graphite_scm_stage_line,
    );
    add(
        registry,
        "graphite.scm.revertLine",
        "Revert line",
        "Scm",
        VIEW,
        "documentHasPath && editorWritable",
        None,
        graphite_scm_revert_line,
    );
    add(
        registry,
        "graphite.scm.revertHunk",
        "Revert hunk",
        "Scm",
        VIEW,
        "documentHasPath && editorWritable",
        None,
        graphite_scm_revert_hunk,
    );
    add(
        registry,
        "graphite.scm.commitPrompt",
        "Commit prompt",
        "Scm",
        ALL,
        "true",
        None,
        graphite_scm_commit_prompt,
    );
}

fn scm_pull(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_scm_remote(crate::app::scm_remote_ui::RemoteOp::Pull);
    Ok(())
}

fn scm_push(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_scm_remote(crate::app::scm_remote_ui::RemoteOp::Push);
    Ok(())
}

fn scm_fetch(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_scm_remote(crate::app::scm_remote_ui::RemoteOp::Fetch);
    Ok(())
}

fn scm_sync(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_scm_remote(crate::app::scm_remote_ui::RemoteOp::Sync);
    Ok(())
}

fn scm_stash(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_scm_remote(crate::app::scm_remote_ui::RemoteOp::Stash);
    Ok(())
}

fn scm_view_history(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_scm_history();
    Ok(())
}

fn scm_create_pull_request(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_pull_request();
    Ok(())
}

fn scm_more_actions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_scm_more_menu();
    Ok(())
}

fn scm_amend(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.amend_last_commit();
    Ok(())
}

fn scm_create_branch(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_create_branch_prompt();
    Ok(())
}

fn scm_merge(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_branch_picker_for(crate::app::scm_branch_ui::BranchPickerAction::Merge);
    Ok(())
}

fn scm_rebase(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_branch_picker_for(crate::app::scm_branch_ui::BranchPickerAction::Rebase);
    Ok(())
}

fn scm_abort_operation(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.abort_git_operation();
    Ok(())
}

fn scm_continue_operation(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.continue_git_operation();
    Ok(())
}

/// The `Git:` commands of the palette (and of the SCM view's "…" button).
pub(super) fn register_git_commands(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 19] = [
        ("graphite.scm.pull", "Git: Pull", scm_pull),
        ("graphite.scm.push", "Git: Push", scm_push),
        ("graphite.scm.fetch", "Git: Fetch", scm_fetch),
        ("graphite.scm.sync", "Git: Sync", scm_sync),
        ("graphite.scm.stash", "Git: Stash", scm_stash),
        (
            "graphite.scm.viewHistory",
            "Git: View History",
            scm_view_history,
        ),
        (
            "graphite.scm.createPullRequest",
            "Git: Create Pull Request",
            scm_create_pull_request,
        ),
        (
            "graphite.scm.moreActions",
            "Git: More Actions...",
            scm_more_actions,
        ),
        ("graphite.scm.amend", "Git: Amend Last Commit", scm_amend),
        (
            "graphite.scm.createBranch",
            "Git: Create Branch...",
            scm_create_branch,
        ),
        ("graphite.scm.merge", "Git: Merge Branch...", scm_merge),
        (
            "graphite.scm.rebase",
            "Git: Rebase onto Branch...",
            scm_rebase,
        ),
        (
            "graphite.scm.interactiveRebase",
            "Git: Interactive Rebase onto Branch...",
            scm_interactive_rebase,
        ),
        (
            "graphite.scm.runInteractiveRebase",
            "Git: Run Interactive Rebase",
            scm_run_interactive_rebase,
        ),
        (
            "graphite.scm.cancelInteractiveRebase",
            "Git: Cancel Interactive Rebase",
            scm_cancel_interactive_rebase,
        ),
        (
            "graphite.scm.deleteBranch",
            "Git: Delete Branch...",
            scm_delete_branch,
        ),
        (
            "graphite.scm.deleteBranchForce",
            "Git: Delete Branch (Force)...",
            scm_delete_branch_force,
        ),
        (
            "graphite.scm.abortOperation",
            "Git: Abort Merge or Rebase",
            scm_abort_operation,
        ),
        (
            "graphite.scm.continueOperation",
            "Git: Continue Merge or Rebase",
            scm_continue_operation,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Scm", ALL, "true", None, handler);
    }
}

fn scm_delete_branch(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_branch_picker_for(crate::app::scm_branch_ui::BranchPickerAction::Delete);
    Ok(())
}

fn scm_delete_branch_force(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_branch_picker_for(crate::app::scm_branch_ui::BranchPickerAction::DeleteForce);
    Ok(())
}

fn scm_interactive_rebase(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_branch_picker_for(crate::app::scm_branch_ui::BranchPickerAction::RebaseInteractive);
    Ok(())
}

fn scm_run_interactive_rebase(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_interactive_rebase();
    Ok(())
}

fn scm_cancel_interactive_rebase(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cancel_interactive_rebase();
    Ok(())
}
