//! Commands that open the settings and keyboard shortcut files.

use super::*;

fn prefs_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn prefs_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn prefs_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn prefs_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn prefs_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn prefs_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_workspace_settings_file();
    Ok(())
}

fn prefs_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_workspace_settings_file();
    Ok(())
}

fn prefs_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_workspace_settings_file();
    Ok(())
}

fn prefs_8(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_workspace_settings_file();
    Ok(())
}

fn prefs_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("keybindings.json");
    Ok(())
}

fn prefs_10(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_default_keybindings_view();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 11] = [
        (
            "workbench.action.openGlobalSettings",
            "Open user settings",
            prefs_0,
        ),
        (
            "workbench.action.openSettingsJson",
            "Open user settings (JSON)",
            prefs_1,
        ),
        (
            "workbench.action.openApplicationSettingsJson",
            "Open application settings (JSON)",
            prefs_2,
        ),
        ("workbench.action.openSettings2", "Open settings", prefs_3),
        (
            "workbench.action.openAccessibilitySettings",
            "Open accessibility settings",
            prefs_4,
        ),
        (
            "workbench.action.openWorkspaceSettings",
            "Open workspace settings",
            prefs_5,
        ),
        (
            "workbench.action.openWorkspaceSettingsFile",
            "Open workspace settings (JSON)",
            prefs_6,
        ),
        (
            "workbench.action.openFolderSettings",
            "Open folder settings",
            prefs_7,
        ),
        (
            "workbench.action.openFolderSettingsFile",
            "Open folder settings (JSON)",
            prefs_8,
        ),
        (
            "workbench.action.openGlobalKeybindingsFile",
            "Open keyboard shortcuts (JSON)",
            prefs_9,
        ),
        (
            "workbench.action.openDefaultKeybindingsFile",
            "Open default keyboard shortcuts (JSON)",
            prefs_10,
        ),
    ];
    for (id, title, handler) in commands {
        add(
            registry,
            id,
            title,
            "Preferences",
            ALL,
            "true",
            None,
            handler,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn app() -> (App, TempDir) {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");
        std::fs::create_dir_all(&app.profile_root).unwrap();
        (app, dir)
    }

    fn open_path(app: &App) -> PathBuf {
        app.documents[app.active_document()]
            .buffer
            .path()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn the_user_settings_commands_open_and_create_the_profile_file() {
        let (mut app, _dir) = app();
        for id in [
            "workbench.action.openGlobalSettings",
            "workbench.action.openSettingsJson",
            "workbench.action.openApplicationSettingsJson",
            "workbench.action.openSettings2",
            "workbench.action.openAccessibilitySettings",
        ] {
            app.run_ui_command(id);
            assert_eq!(
                open_path(&app),
                app.profile_root.join("settings.json"),
                "{id}"
            );
        }
        assert!(app.profile_root.join("settings.json").is_file());
    }

    #[test]
    fn the_workspace_and_folder_commands_open_the_workspace_settings_file() {
        let (mut app, dir) = app();
        for id in [
            "workbench.action.openWorkspaceSettings",
            "workbench.action.openWorkspaceSettingsFile",
            "workbench.action.openFolderSettings",
            "workbench.action.openFolderSettingsFile",
        ] {
            app.run_ui_command(id);
            let opened = open_path(&app);
            assert!(
                opened.ends_with(Path::new(".vscode").join("settings.json")),
                "{id}: {opened:?}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".vscode/settings.json")).unwrap(),
            "{}\n"
        );
    }

    #[test]
    fn an_existing_workspace_settings_file_is_not_overwritten() {
        let (mut app, dir) = app();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(dir.path().join(".vscode/settings.json"), "{\"a\":1}").unwrap();
        app.run_ui_command("workbench.action.openWorkspaceSettings");
        assert_eq!(app.editor().text(), "{\"a\":1}");
    }

    #[test]
    fn the_keyboard_shortcut_commands_open_the_user_file_and_a_read_only_default_list() {
        let (mut app, _dir) = app();
        app.run_ui_command("workbench.action.openGlobalKeybindingsFile");
        assert_eq!(open_path(&app), app.profile_root.join("keybindings.json"));

        app.run_ui_command("workbench.action.openDefaultKeybindingsFile");
        assert!(app.documents[app.active_document()].buffer.is_read_only());
        let text = app.editor().text();
        assert!(text.contains("workbench.action.showCommands"));
        assert!(text.contains("\"key\": \"ctrl+shift+p\""));
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(parsed.as_array().is_some_and(|list| list.len() > 50));
        // Opening it again reuses the same view.
        let views = app.documents.len();
        app.run_ui_command("workbench.action.openDefaultKeybindingsFile");
        assert_eq!(app.documents.len(), views);
    }
}
