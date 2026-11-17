//! User Data Sync command handlers.

use super::*;
use crate::app::user_data_sync::state::ConflictChoice;

type Entry = (&'static str, &'static str, &'static str, Handler);

fn turn_on(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.turn_on_sync();
    Ok(())
}

fn turn_off(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.turn_off_sync();
    Ok(())
}

fn cancel_turn_on(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cancel_turn_on_sync();
    Ok(())
}

fn sync_now(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.sync_now();
    Ok(())
}

fn signin(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.start_sync_sign_in();
    Ok(())
}

fn accept_merges(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.accept_sync_merges(ConflictChoice::KeepMerged);
    Ok(())
}

fn open_backups_folder(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_sync_backups_folder();
    Ok(())
}

fn show_synced_data(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_synced_data();
    Ok(())
}

fn reset_data(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.reset_sync_data();
    Ok(())
}

fn open_settings(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn manage_sync(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_synced_data();
    Ok(())
}

fn sync_help(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.new_untitled_document();
    app.editor_mut().insert(
        "# User Data Sync\n\n\
         Graphite synchronizes `settings.json`, `keybindings.json`, snippets, profiles, \
         and installed plugin lists using a private GitHub Gist (`graphite-sync.json`).\n\n\
         - Local backups are saved before each merge under `<profile>/sync-backups/`.\n\
         - Key-level conflicts in JSON settings are flagged for review (`Accept Merges`).\n",
    );
    app.status = "Opened User Data Sync documentation".into();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let entries: &[Entry] = &[
        (
            "workbench.actions.syncData.reset",
            "Reset Synced Data",
            "User Data Sync",
            reset_data,
        ),
        (
            "workbench.userData.actions.cancelTurnOn",
            "Cancel Turning on Settings Sync",
            "User Data Sync",
            cancel_turn_on,
        ),
        (
            "workbench.userData.actions.openSyncBackupsFolder",
            "Open Local Backups Folder",
            "User Data Sync",
            open_backups_folder,
        ),
        (
            "workbench.userData.actions.signin",
            "Sign in to Sync Data",
            "User Data Sync",
            signin,
        ),
        (
            "workbench.userData.actions.turningOn",
            "Turning On Settings Sync",
            "User Data Sync",
            turn_on,
        ),
        (
            "workbench.userDataSync.actions.acceptMerges",
            "Accept Pending Sync Merges",
            "User Data Sync",
            accept_merges,
        ),
        (
            "workbench.userDataSync.actions.help",
            "Show User Data Sync Help",
            "User Data Sync",
            sync_help,
        ),
        (
            "workbench.userDataSync.actions.manage",
            "Manage User Data Sync",
            "User Data Sync",
            manage_sync,
        ),
        (
            "workbench.userDataSync.actions.settings",
            "Configure Synced Settings",
            "User Data Sync",
            open_settings,
        ),
        (
            "workbench.userDataSync.actions.showSyncedData",
            "Show Synced Data",
            "User Data Sync",
            show_synced_data,
        ),
        (
            "workbench.userDataSync.actions.syncNow",
            "Sync User Data Now",
            "User Data Sync",
            sync_now,
        ),
        (
            "workbench.userDataSync.actions.turnOff",
            "Turn Off Settings Sync",
            "User Data Sync",
            turn_off,
        ),
        (
            "workbench.userDataSync.actions.turnOn",
            "Turn On Settings Sync",
            "User Data Sync",
            turn_on,
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
    fn sync_commands_toggle_backups_show_data_and_reset_work() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");

        app.run_ui_command("workbench.userData.actions.openSyncBackupsFolder");
        assert!(app.profile_root.join("sync-backups").is_dir());

        app.run_ui_command("workbench.userDataSync.actions.showSyncedData");
        assert!(app.profile_root.join("user-data-sync.json").is_file());

        app.run_ui_command("workbench.userDataSync.actions.help");
        assert!(app.editor().text().contains("User Data Sync"));

        app.sync.persisted.enabled = true;
        app.run_ui_command("workbench.userDataSync.actions.turnOff");
        assert!(!app.sync.persisted.enabled);

        app.run_ui_command("workbench.actions.syncData.reset");
        assert!(app.sync.persisted.remote_id.is_none());
    }
}
