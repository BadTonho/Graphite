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
