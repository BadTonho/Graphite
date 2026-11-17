//! Update lifecycle and redacted Issue Reporter / Troubleshooting commands.

use super::*;

type Entry = (&'static str, &'static str, &'static str, Handler);

fn check_for_updates(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.check_for_updates();
    Ok(())
}

fn download_update(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.download_update();
    Ok(())
}

fn install_update(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.install_update();
    Ok(())
}

fn restart_to_update(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.restart_to_update();
    Ok(())
}

fn cancel_update(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cancel_update();
    Ok(())
}

fn show_update_info(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_update_info();
    Ok(())
}

fn show_release_notes(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_update_release_notes();
    Ok(())
}

fn open_issue_reporter(app: &mut App, inv: &CommandInvocation) -> Result<(), CommandError> {
    let summary = inv
        .request
        .arguments
        .as_str()
        .or_else(|| inv.request.arguments.get("text").and_then(|v| v.as_str()))
        .filter(|s| !s.trim().is_empty());
    app.open_issue_reporter(summary);
    Ok(())
}

fn troubleshoot_start(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.start_troubleshooting();
    Ok(())
}

fn troubleshoot_stop(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.stop_troubleshooting();
    Ok(())
}

fn report_chat_issue(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_issue_reporter(Some("Chat / Agent bug report"));
    Ok(())
}

pub(crate) fn register(registry: &mut CommandRegistry) {
    let entries: &[Entry] = &[
        (
            "update.check",
            "Check for Updates",
            "Update",
            check_for_updates,
        ),
        (
            "update.checkForUpdate",
            "Check for Update",
            "Update",
            check_for_updates,
        ),
        (
            "update.checking",
            "Checking for Updates",
            "Update",
            show_update_info,
        ),
        (
            "update.downloadNow",
            "Download Update Now",
            "Update",
            download_update,
        ),
        (
            "update.downloadUpdate",
            "Download Update",
            "Update",
            download_update,
        ),
        (
            "update.downloading",
            "Show Download Status",
            "Update",
            show_update_info,
        ),
        ("update.install", "Install Update", "Update", install_update),
        (
            "update.installUpdate",
            "Install Update Package",
            "Update",
            install_update,
        ),
        (
            "update.updating",
            "Show Update Installation Status",
            "Update",
            show_update_info,
        ),
        ("update.restart", "Restart", "Update", restart_to_update),
        (
            "update.restartToUpdate",
            "Restart to Update",
            "Update",
            restart_to_update,
        ),
        (
            "update.cancelling",
            "Cancel Update",
            "Update",
            cancel_update,
        ),
        (
            "update.showUpdateInfo",
            "Show Update Info",
            "Update",
            show_update_info,
        ),
        (
            "update.showUpdateReleaseNotes",
            "Show Release Notes",
            "Update",
            show_release_notes,
        ),
        (
            "workbench.action.openIssueReporter",
            "Open Issue Reporter",
            "Help",
            open_issue_reporter,
        ),
        (
            "workbench.action.troubleshootIssue.start",
            "Start Troubleshooting",
            "Help",
            troubleshoot_start,
        ),
        (
            "workbench.action.troubleshootIssue.stop",
            "Stop Troubleshooting",
            "Help",
            troubleshoot_stop,
        ),
        (
            "workbench.action.chat.reportIssueForBug",
            "Report Issue for Chat",
            "Chat",
            report_chat_issue,
        ),
    ];

    for (id, title, category, handler) in entries {
        add(registry, id, title, category, ALL, "true", None, *handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_and_issue_reporter_commands_redact_secrets_and_track_troubleshooting() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);

        app.run_ui_command("update.showUpdateInfo");
        assert!(app.editor().text().contains("Graphite Update Info"));

        app.run_ui_command("update.showUpdateReleaseNotes");
        assert!(app.editor().text().contains("Release Notes"));

        app.run_ui_command("workbench.action.troubleshootIssue.start");
        assert!(app.troubleshooting.active);

        let draft = app.open_issue_reporter(Some(
            "Failed auth with ghp_1234567890abcdefghij and sk-secret1234567890abcdef",
        ));
        assert!(!draft.body_markdown.contains("ghp_123456"));
        assert!(!draft.body_markdown.contains("sk-secret123456"));
        assert!(draft.body_markdown.contains("[REDACTED]"));
        assert!(draft
            .github_url
            .starts_with("https://github.com/BadTonho/Graphite/issues/new?"));

        app.run_ui_command("workbench.action.troubleshootIssue.stop");
        assert!(!app.troubleshooting.active);
    }
}
