//! Explorer and file commands: creating files and folders, refreshing, and opening entries.

use super::*;

fn files_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_new_entry(false);
    Ok(())
}

fn files_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_new_entry(true);
    Ok(())
}

fn files_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.refresh_explorer();
    Ok(())
}

fn files_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_collapse_all();
    Ok(())
}

fn files_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_open_preserving_focus();
    Ok(())
}

fn files_5(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let _ = invocation;
    super::misc::copy_path_of_active_file(app)
}

fn files_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.new_untitled_document();
    Ok(())
}

fn files_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.new_untitled_document();
    Ok(())
}

fn files_8(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let _ = invocation;
    super::groups::close_all_groups(app, invocation)
}

fn files_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.editor_groups.flip_layout();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, &'static str, Handler); 10] = [
        (
            "workbench.files.action.createFileFromExplorer",
            "New file...",
            "Explorer",
            files_0,
        ),
        (
            "workbench.files.action.createFolderFromExplorer",
            "New folder...",
            "Explorer",
            files_1,
        ),
        (
            "workbench.files.action.refreshFilesExplorer",
            "Refresh explorer",
            "Explorer",
            files_2,
        ),
        (
            "workbench.files.action.collapseExplorerFolders",
            "Collapse folders in explorer",
            "Explorer",
            files_3,
        ),
        (
            "filesExplorer.openFilePreserveFocus",
            "Open file and keep focus in explorer",
            "Explorer",
            files_4,
        ),
        (
            "workbench.action.files.copyPathOfActiveFile",
            "Copy path of active file",
            "File",
            files_5,
        ),
        (
            "openEditors.newUntitledFile",
            "New untitled text file",
            "File",
            files_6,
        ),
        (
            "workbench.action.files.newUntitledFileFromNewTabButton",
            "New untitled text file from the tab bar",
            "File",
            files_7,
        ),
        ("openEditors.closeAll", "Close all editors", "File", files_8),
        (
            "workbench.action.toggleEditorGroupLayout",
            "Toggle vertical/horizontal editor layout",
            "View",
            files_9,
        ),
    ];
    for (id, title, category, handler) in commands {
        add(registry, id, title, category, ALL, "true", None, handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use winit::keyboard::{Key, NamedKey};

    fn app() -> (App, TempDir) {
        let dir = TempDir::new();
        std::fs::create_dir(dir.path().join("folder")).unwrap();
        std::fs::write(dir.path().join("existing.txt"), "x").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.tree.refresh();
        (app, dir)
    }

    fn submit(app: &mut App, name: &str) {
        app.explorer_prompt.as_mut().expect("a prompt").value = name.into();
        assert!(app.handle_explorer_prompt_key(&Key::Named(NamedKey::Enter), None));
    }

    #[test]
    fn a_new_file_is_created_in_the_selected_folder_and_opened() {
        let (mut app, dir) = app();
        app.explorer_select(dir.path().join("folder"));
        app.run_ui_command("workbench.files.action.createFileFromExplorer");
        submit(&mut app, "sub/made.txt");
        let made = dir.path().join("folder/sub/made.txt");
        assert!(made.is_file());
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(made.as_path())
        );
        assert_eq!(app.explorer_selected.as_deref(), Some(made.as_path()));
        assert!(app.explorer_prompt.is_none());
    }

    #[test]
    fn a_new_file_next_to_the_selected_file_or_in_the_root() {
        let (mut app, dir) = app();
        app.explorer_select(dir.path().join("existing.txt"));
        app.run_ui_command("workbench.files.action.createFileFromExplorer");
        submit(&mut app, "beside.txt");
        assert!(dir.path().join("beside.txt").is_file());

        app.explorer_selected = None;
        app.run_ui_command("workbench.files.action.createFileFromExplorer");
        submit(&mut app, "root.txt");
        assert!(dir.path().join("root.txt").is_file());
    }

    #[test]
    fn a_new_folder_is_created_without_opening_anything() {
        let (mut app, dir) = app();
        let before = app.active_document();
        app.run_ui_command("workbench.files.action.createFolderFromExplorer");
        submit(&mut app, "a/b");
        assert!(dir.path().join("a/b").is_dir());
        assert_eq!(app.active_document(), before);
    }

    #[test]
    fn invalid_and_duplicate_names_create_nothing() {
        let (mut app, dir) = app();
        for name in ["../outside.txt", "", "existing.txt"] {
            app.run_ui_command("workbench.files.action.createFileFromExplorer");
            submit(&mut app, name);
        }
        assert!(!dir.path().parent().unwrap().join("outside.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("existing.txt")).unwrap(),
            "x"
        );
        assert!(app.status.contains("exists"), "{}", app.status);
    }

    #[test]
    fn refresh_shows_files_created_outside_and_collapse_folds_every_folder() {
        let (mut app, dir) = app();
        std::fs::write(dir.path().join("late.txt"), "").unwrap();
        assert!(!app.tree.rows.iter().any(|row| row.entry.name == "late.txt"));
        app.run_ui_command("workbench.files.action.refreshFilesExplorer");
        assert!(app.tree.rows.iter().any(|row| row.entry.name == "late.txt"));

        app.explorer_select(dir.path().join("folder"));
        app.tree.set_expanded(&dir.path().join("folder"), true);
        app.run_ui_command("workbench.files.action.collapseExplorerFolders");
        assert!(app.tree.rows.iter().all(|row| !row.expanded));
    }

    #[test]
    fn opening_a_file_can_keep_the_focus_in_the_explorer() {
        let (mut app, dir) = app();
        app.explorer_select(dir.path().join("existing.txt"));
        app.run_ui_command("filesExplorer.openFilePreserveFocus");
        assert_eq!(
            app.documents[app.active_document()]
                .buffer
                .path()
                .map(|p| p.to_path_buf()),
            Some(dir.path().join("existing.txt"))
        );
        assert!(app.explorer_focused);
    }

    #[test]
    fn untitled_files_and_close_all_work_from_the_open_editors_commands() {
        let (mut app, dir) = app();
        app.open_file(dir.path().join("existing.txt"));
        let documents = app.documents.len();
        app.run_ui_command("openEditors.newUntitledFile");
        assert_eq!(app.documents.len(), documents + 1);
        app.run_ui_command("workbench.action.files.newUntitledFileFromNewTabButton");
        assert_eq!(app.documents.len(), documents + 2);
        app.run_ui_command("openEditors.closeAll");
        assert_eq!(
            app.editor_groups
                .group_views(app.editor_groups.focused())
                .len(),
            1
        );
    }

    #[test]
    fn the_layout_toggle_and_copy_path_are_registered() {
        for id in [
            "workbench.action.toggleEditorGroupLayout",
            "workbench.action.files.copyPathOfActiveFile",
        ] {
            assert!(
                crate::app::command_registry::registry().get(id).is_some(),
                "{id}"
            );
        }
    }
}
