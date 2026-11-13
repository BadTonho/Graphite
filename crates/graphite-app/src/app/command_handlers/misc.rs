//! Smaller workbench commands: the activity bar location, breadcrumbs, new windows, clearing the
//! editor history, reverting and closing, and duplicating a group.

use super::*;
use crate::app::clipboard::{ClipboardAccess, SystemClipboard};
use crate::app::editor_groups::SplitDirection;
use crate::config::ActivityBarPosition;

fn activity_bar(app: &mut App, position: ActivityBarPosition) -> Result<(), CommandError> {
    app.set_activity_bar_position(position);
    Ok(())
}

fn set_breadcrumbs(app: &mut App, visible: bool) -> Result<(), CommandError> {
    app.view_preferences.breadcrumbs_visible = visible;
    app.save_view_preferences_for_command();
    Ok(())
}

fn copy_breadcrumbs_path(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(path) = app.documents[app.command_document()]
        .buffer
        .path()
        .map(|path| path.to_string_lossy().into_owned())
    else {
        return Err(CommandError::Disabled(app.t("misc.no_path").into()));
    };
    if SystemClipboard.set_text(&path).is_err() {
        return Err(CommandError::Failed(app.t("misc.clipboard_failed").into()));
    }
    app.status = app.t("misc.path_copied").into();
    Ok(())
}

fn new_window(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.spawn_window(None);
    Ok(())
}

/// Forgets the recently used editors, the navigation history and the reopen stack.
fn clear_editor_history(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.recent_documents.clear();
    app.navigation_history.clear();
    app.navigation_history_index = None;
    app.closed_tabs.clear();
    app.status = app.t("misc.history_cleared").into();
    Ok(())
}

/// Reloads the file from disk, then closes its tab, unless reverting needs a confirmation first.
fn revert_and_close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.revert_active_file();
    if app.pending_revert_confirmation.is_none() {
        app.close_active_tab();
    }
    Ok(())
}

/// Splits a new group on one side holding a copy of every tab, with the same tab active.
fn duplicate_group(app: &mut App, split: SplitDirection, before: bool) -> Result<(), CommandError> {
    let source = app.editor_groups.focused();
    let active_document = app.command_view_ref().document;
    let views = app.editor_groups.group_views(source);
    if before {
        app.split_editor_group_before(split);
    } else {
        app.split_editor_group(split);
    }
    let target = app.editor_groups.focused();
    for view in views {
        app.editor_groups.transfer_view(view, target, true);
    }
    let tabs = app.editor_groups.group_views(target);
    if let Some(index) = tabs.iter().position(|view| {
        app.editor_groups
            .view(*view)
            .is_some_and(|view| view.document == active_document)
    }) {
        app.editor_groups.select_tab(index);
    }
    Ok(())
}

macro_rules! command {
    ($name:ident, $body:expr) => {
        fn $name(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
            let run: fn(&mut App) -> Result<(), CommandError> = $body;
            run(app)
        }
    };
}

command!(bar_default, |app| activity_bar(
    app,
    ActivityBarPosition::Default
));
command!(bar_top, |app| activity_bar(app, ActivityBarPosition::Top));
command!(bar_bottom, |app| activity_bar(
    app,
    ActivityBarPosition::Bottom
));
command!(bar_hidden, |app| activity_bar(
    app,
    ActivityBarPosition::Hidden
));
command!(breadcrumbs_toggle, |app| {
    let visible = !app.view_preferences.breadcrumbs_visible;
    set_breadcrumbs(app, visible)
});
command!(breadcrumbs_on, |app| set_breadcrumbs(app, true));
command!(duplicate_left, |app| duplicate_group(
    app,
    SplitDirection::Vertical,
    true
));
command!(duplicate_right, |app| duplicate_group(
    app,
    SplitDirection::Vertical,
    false
));
command!(duplicate_up, |app| duplicate_group(
    app,
    SplitDirection::Horizontal,
    true
));
command!(duplicate_down, |app| duplicate_group(
    app,
    SplitDirection::Horizontal,
    false
));

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, &'static str, Handler); 14] = [
        (
            "workbench.action.activityBarLocation.default",
            "Move activity bar to the side",
            "View",
            bar_default,
        ),
        (
            "workbench.action.activityBarLocation.top",
            "Move activity bar to the top",
            "View",
            bar_top,
        ),
        (
            "workbench.action.activityBarLocation.bottom",
            "Move activity bar to the bottom",
            "View",
            bar_bottom,
        ),
        (
            "workbench.action.activityBarLocation.hide",
            "Hide activity bar",
            "View",
            bar_hidden,
        ),
        (
            "breadcrumbs.toggle",
            "Toggle breadcrumbs",
            "View",
            breadcrumbs_toggle,
        ),
        (
            "breadcrumbs.toggleToOn",
            "Show breadcrumbs",
            "View",
            breadcrumbs_on,
        ),
        (
            "breadcrumbs.copyPath",
            "Copy breadcrumbs path",
            "Editor",
            copy_breadcrumbs_path,
        ),
        (
            "workbench.action.newWindow",
            "New window",
            "File",
            new_window,
        ),
        (
            "workbench.action.clearEditorHistory",
            "Clear editor history",
            "Workbench",
            clear_editor_history,
        ),
        (
            "workbench.action.revertAndCloseActiveEditor",
            "Revert and close editor",
            "File",
            revert_and_close,
        ),
        (
            "workbench.action.duplicateActiveEditorGroupLeft",
            "Duplicate editor group to the left",
            "Workbench",
            duplicate_left,
        ),
        (
            "workbench.action.duplicateActiveEditorGroupRight",
            "Duplicate editor group to the right",
            "Workbench",
            duplicate_right,
        ),
        (
            "workbench.action.duplicateActiveEditorGroupUp",
            "Duplicate editor group up",
            "Workbench",
            duplicate_up,
        ),
        (
            "workbench.action.duplicateActiveEditorGroupDown",
            "Duplicate editor group down",
            "Workbench",
            duplicate_down,
        ),
    ];
    for (id, title, category, handler) in commands {
        add(registry, id, title, category, ALL, "true", None, handler);
    }
    add(
        registry,
        "workbench.action.clearEditorHistoryWithoutConfirm",
        "Clear editor history without confirmation",
        "Workbench",
        ALL,
        "true",
        None,
        clear_editor_history,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn app_with_files(names: &[&str]) -> (App, TempDir) {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        for name in names {
            std::fs::write(dir.path().join(name), name).unwrap();
            app.open_file(dir.path().join(name));
        }
        (app, dir)
    }

    #[test]
    fn the_activity_bar_moves_and_hides() {
        let (mut app, _dir) = app_with_files(&[]);
        for (command, position) in [
            (
                "workbench.action.activityBarLocation.top",
                ActivityBarPosition::Top,
            ),
            (
                "workbench.action.activityBarLocation.bottom",
                ActivityBarPosition::Bottom,
            ),
            (
                "workbench.action.activityBarLocation.hide",
                ActivityBarPosition::Hidden,
            ),
            (
                "workbench.action.activityBarLocation.default",
                ActivityBarPosition::Default,
            ),
        ] {
            app.run_ui_command(command);
            assert_eq!(app.activity_bar_preferences.position, position, "{command}");
        }
    }

    #[test]
    fn breadcrumbs_toggle_and_turn_on() {
        let (mut app, _dir) = app_with_files(&[]);
        let initial = app.view_preferences.breadcrumbs_visible;
        app.run_ui_command("breadcrumbs.toggle");
        assert_eq!(app.view_preferences.breadcrumbs_visible, !initial);
        app.run_ui_command("breadcrumbs.toggleToOn");
        assert!(app.view_preferences.breadcrumbs_visible);
        app.run_ui_command("breadcrumbs.toggleToOn");
        assert!(app.view_preferences.breadcrumbs_visible, "on stays on");
    }

    #[test]
    fn clearing_the_history_forgets_recent_editors_navigation_and_closed_tabs() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt"]);
        app.push_navigation_location();
        assert!(!app.recent_documents.is_empty());
        app.run_ui_command("workbench.action.clearEditorHistory");
        assert!(app.recent_documents.is_empty());
        assert!(app.navigation_history.is_empty());
        assert!(app.closed_tabs.is_empty());
        app.run_ui_command("workbench.action.clearEditorHistoryWithoutConfirm");
    }

    #[test]
    fn reverting_and_closing_reloads_a_clean_file_and_closes_its_tab() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt"]);
        let tabs = app
            .editor_groups
            .group_views(app.editor_groups.focused())
            .len();
        app.run_ui_command("workbench.action.revertAndCloseActiveEditor");
        let after = app
            .editor_groups
            .group_views(app.editor_groups.focused())
            .len();
        assert_eq!(after, tabs - 1);
    }

    #[test]
    fn reverting_a_file_with_unsaved_changes_asks_first_and_keeps_the_tab() {
        let (mut app, _dir) = app_with_files(&["a.txt"]);
        app.editor_mut().insert("changed");
        app.run_ui_command("workbench.action.revertAndCloseActiveEditor");
        assert!(app.pending_revert_confirmation.is_some());
        assert_eq!(
            app.editor_groups
                .group_views(app.editor_groups.focused())
                .len(),
            1
        );
    }

    #[test]
    fn duplicating_a_group_copies_every_tab_and_keeps_the_active_one() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt", "c.txt"]);
        app.run_ui_command("workbench.action.firstEditorInGroup");
        let source = app.editor_groups.focused();
        app.run_ui_command("workbench.action.duplicateActiveEditorGroupRight");
        let groups = app.editor_groups.ordered_group_ids();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], source);
        let copy = app.editor_groups.focused();
        assert_ne!(copy, source);
        assert_eq!(app.editor_groups.group_views(copy).len(), 3);
        assert_eq!(app.editor_groups.group_views(source).len(), 3);
        let active = app.command_view_ref().document;
        let first = app
            .editor_groups
            .view(app.editor_groups.group_views(source)[0])
            .unwrap()
            .document;
        assert_eq!(active, first, "the copy shows the same tab as the original");
    }

    #[test]
    fn new_window_and_copy_path_are_registered() {
        for id in [
            "workbench.action.newWindow",
            "breadcrumbs.copyPath",
            "workbench.action.duplicateActiveEditorGroupLeft",
            "workbench.action.duplicateActiveEditorGroupUp",
            "workbench.action.duplicateActiveEditorGroupDown",
        ] {
            assert!(
                crate::app::command_registry::registry().get(id).is_some(),
                "{id}"
            );
        }
    }
}
