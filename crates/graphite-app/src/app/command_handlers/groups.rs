//! Commands that move editors between groups and along the tab strip, create and join groups,
//! and close editors in bulk.

use super::*;
use crate::app::editor_groups::SplitDirection;
use crate::app::go_menu::GoSubcommand;

#[derive(Clone, Copy)]
enum Target {
    Next,
    Previous,
    First,
    Last,
}

fn target_group(app: &App, target: Target) -> Option<u64> {
    let groups = app.editor_groups.ordered_group_ids();
    let current = groups
        .iter()
        .position(|id| *id == app.editor_groups.focused())?;
    let index = match target {
        Target::Next => (current + 1) % groups.len(),
        Target::Previous => (current + groups.len() - 1) % groups.len(),
        Target::First => 0,
        Target::Last => groups.len() - 1,
    };
    (groups[index] != groups[current]).then_some(groups[index])
}

fn send_to_group(app: &mut App, target: Target, copy: bool) -> Result<(), CommandError> {
    let Some(group) = target_group(app, target) else {
        return Err(CommandError::Disabled(app.t("groups.no_target").into()));
    };
    let view = app.command_view_ref().id;
    app.editor_groups.transfer_view(view, group, copy);
    Ok(())
}

/// Copies the editor into the group next to the focused one, or splits a new one when there is
/// none in that direction.
fn send_to_adjacent(
    app: &mut App,
    direction: GoSubcommand,
    split: SplitDirection,
    before: bool,
) -> Result<(), CommandError> {
    match app.adjacent_group(direction) {
        Some(group) => {
            let view = app.command_view_ref().id;
            app.editor_groups.transfer_view(view, group, true);
        }
        None if before => {
            app.split_editor_group_before(split);
        }
        None => {
            app.split_editor_group(split);
        }
    }
    Ok(())
}

/// Moves the active tab to the index `place` picks from the current index and the tab count.
fn move_tab(app: &mut App, place: fn(usize, usize) -> usize) -> Result<(), CommandError> {
    let view = app.command_view_ref().id;
    let Some(group) = app.editor_groups.group_for_view(view) else {
        return Ok(());
    };
    let count = app.editor_groups.group_views(group).len();
    let index = app.editor_groups.active_index(group).unwrap_or(0);
    app.editor_groups
        .move_view_to_index(view, place(index, count));
    Ok(())
}

/// Closes the given views, leaving alone any whose only copy has unsaved changes.
fn close_views(app: &mut App, views: Vec<graphite_text::ViewId>) {
    for view in views {
        let dirty = app
            .editor_groups
            .view(view)
            .and_then(|view| app.documents.get(view.document))
            .is_some_and(|document| document.is_dirty());
        app.editor_groups.close_view(view, dirty);
    }
}

fn close_to_the_left(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let group = app.editor_groups.focused();
    let index = app.editor_groups.active_index(group).unwrap_or(0);
    let views = app.editor_groups.group_views(group)[..index].to_vec();
    close_views(app, views);
    Ok(())
}

fn close_in_other_groups(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let focused = app.editor_groups.focused();
    let views = app
        .editor_groups
        .ordered_group_ids()
        .into_iter()
        .filter(|group| *group != focused)
        .flat_map(|group| app.editor_groups.group_views(group))
        .collect();
    close_views(app, views);
    Ok(())
}

fn close_all_groups(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let views = app.editor_groups.ordered_view_ids();
    close_views(app, views);
    Ok(())
}

fn close_in_all_groups(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let document = app.command_view_ref().document;
    let views = app
        .editor_groups
        .all_views()
        .filter(|view| view.document == document)
        .map(|view| view.id)
        .collect();
    close_views(app, views);
    Ok(())
}

fn join_two_groups(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(next) = target_group(app, Target::Next) else {
        return Err(CommandError::Disabled(app.t("groups.no_target").into()));
    };
    let focused = app.editor_groups.focused();
    app.editor_groups.join_groups(focused, next);
    Ok(())
}

fn join_all_groups(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let focused = app.editor_groups.focused();
    for group in app.editor_groups.ordered_group_ids() {
        app.editor_groups.join_groups(group, focused);
    }
    Ok(())
}

fn focus_group(app: &mut App, target: Target) -> Result<(), CommandError> {
    let groups = app.editor_groups.ordered_group_ids();
    let id = match target {
        Target::First => groups.first(),
        Target::Last => groups.last(),
        _ => None,
    };
    if let Some(&id) = id {
        app.focus_editor_group(id);
    }
    Ok(())
}

macro_rules! handler {
    ($name:ident, $body:expr) => {
        fn $name(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
            let run: fn(&mut App) -> Result<(), CommandError> = $body;
            run(app)
        }
    };
}

handler!(move_to_next, |app| send_to_group(app, Target::Next, false));
handler!(move_to_previous, |app| send_to_group(
    app,
    Target::Previous,
    false
));
handler!(move_to_first, |app| send_to_group(
    app,
    Target::First,
    false
));
handler!(move_to_last, |app| send_to_group(app, Target::Last, false));
handler!(split_to_next, |app| send_to_group(app, Target::Next, true));
handler!(split_to_previous, |app| send_to_group(
    app,
    Target::Previous,
    true
));
handler!(split_to_first, |app| send_to_group(
    app,
    Target::First,
    true
));
handler!(split_to_last, |app| send_to_group(app, Target::Last, true));
handler!(split_to_left, |app| send_to_adjacent(
    app,
    GoSubcommand::GroupLeft,
    SplitDirection::Vertical,
    true
));
handler!(split_to_right, |app| send_to_adjacent(
    app,
    GoSubcommand::GroupRight,
    SplitDirection::Vertical,
    false
));
handler!(split_to_above, |app| send_to_adjacent(
    app,
    GoSubcommand::GroupAbove,
    SplitDirection::Horizontal,
    true
));
handler!(split_to_below, |app| send_to_adjacent(
    app,
    GoSubcommand::GroupBelow,
    SplitDirection::Horizontal,
    false
));
handler!(new_group_left, |app| {
    app.split_editor_group_before(SplitDirection::Vertical);
    Ok(())
});
handler!(new_group_right, |app| {
    app.split_editor_group(SplitDirection::Vertical);
    Ok(())
});
handler!(new_group_above, |app| {
    app.split_editor_group_before(SplitDirection::Horizontal);
    Ok(())
});
handler!(new_group_below, |app| {
    app.split_editor_group(SplitDirection::Horizontal);
    Ok(())
});
handler!(tab_left, |app| move_tab(app, |index, _| index
    .saturating_sub(1)));
handler!(tab_right, |app| move_tab(app, |index, count| (index + 1)
    .min(count - 1)));
handler!(tab_to_start, |app| move_tab(app, |_, _| 0));
handler!(tab_to_end, |app| move_tab(app, |_, count| count - 1));
handler!(first_in_group, |app| {
    app.editor_groups.select_tab(0);
    Ok(())
});
handler!(last_in_group, |app| {
    let group = app.editor_groups.focused();
    let count = app.editor_groups.group_views(group).len();
    app.editor_groups.select_tab(count - 1);
    Ok(())
});
handler!(focus_active_group, |app| {
    let focused = app.editor_groups.focused();
    app.focus_editor_group(focused);
    Ok(())
});
handler!(focus_first_group, |app| focus_group(app, Target::First));
handler!(focus_last_group, |app| focus_group(app, Target::Last));

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 31] = [
        (
            "workbench.action.moveEditorToNextGroup",
            "Move editor into next group",
            move_to_next,
        ),
        (
            "workbench.action.moveEditorToPreviousGroup",
            "Move editor into previous group",
            move_to_previous,
        ),
        (
            "workbench.action.moveEditorToFirstGroup",
            "Move editor into first group",
            move_to_first,
        ),
        (
            "workbench.action.moveEditorToLastGroup",
            "Move editor into last group",
            move_to_last,
        ),
        (
            "workbench.action.splitEditorToNextGroup",
            "Split editor into next group",
            split_to_next,
        ),
        (
            "workbench.action.splitEditorToPreviousGroup",
            "Split editor into previous group",
            split_to_previous,
        ),
        (
            "workbench.action.splitEditorToFirstGroup",
            "Split editor into first group",
            split_to_first,
        ),
        (
            "workbench.action.splitEditorToLastGroup",
            "Split editor into last group",
            split_to_last,
        ),
        (
            "workbench.action.splitEditorToLeftGroup",
            "Split editor into left group",
            split_to_left,
        ),
        (
            "workbench.action.splitEditorToRightGroup",
            "Split editor into right group",
            split_to_right,
        ),
        (
            "workbench.action.splitEditorToAboveGroup",
            "Split editor into group above",
            split_to_above,
        ),
        (
            "workbench.action.splitEditorToBelowGroup",
            "Split editor into group below",
            split_to_below,
        ),
        (
            "workbench.action.newGroupLeft",
            "New editor group to the left",
            new_group_left,
        ),
        (
            "workbench.action.newGroupRight",
            "New editor group to the right",
            new_group_right,
        ),
        (
            "workbench.action.newGroupAbove",
            "New editor group above",
            new_group_above,
        ),
        (
            "workbench.action.newGroupBelow",
            "New editor group below",
            new_group_below,
        ),
        (
            "workbench.action.moveEditorLeftInGroup",
            "Move editor left",
            tab_left,
        ),
        (
            "workbench.action.moveEditorRightInGroup",
            "Move editor right",
            tab_right,
        ),
        (
            "workbench.action.moveEditorToStart",
            "Move editor to the start of the group",
            tab_to_start,
        ),
        (
            "workbench.action.moveEditorToEnd",
            "Move editor to the end of the group",
            tab_to_end,
        ),
        (
            "workbench.action.firstEditorInGroup",
            "Open first editor in group",
            first_in_group,
        ),
        (
            "workbench.action.lastEditorInGroup",
            "Open last editor in group",
            last_in_group,
        ),
        (
            "workbench.action.closeEditorsToTheLeft",
            "Close editors to the left",
            close_to_the_left,
        ),
        (
            "workbench.action.closeEditorsInOtherGroups",
            "Close editors in other groups",
            close_in_other_groups,
        ),
        (
            "workbench.action.closeAllGroups",
            "Close all editor groups",
            close_all_groups,
        ),
        (
            "workbench.action.closeEditorInAllGroups",
            "Close editor in all groups",
            close_in_all_groups,
        ),
        (
            "workbench.action.joinTwoGroups",
            "Join editor group with next",
            join_two_groups,
        ),
        (
            "workbench.action.joinAllGroups",
            "Join all editor groups",
            join_all_groups,
        ),
        (
            "workbench.action.focusActiveEditorGroup",
            "Focus active editor group",
            focus_active_group,
        ),
        (
            "workbench.action.focusFirstEditorGroup",
            "Focus first editor group",
            focus_first_group,
        ),
        (
            "workbench.action.focusLastEditorGroup",
            "Focus last editor group",
            focus_last_group,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Workbench", ALL, "true", None, handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    /// An app with files `a.txt` to `c.txt` open in one group, `c.txt` active.
    fn app_with_three_tabs() -> (App, TempDir) {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        for name in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(dir.path().join(name), name).unwrap();
            app.open_file(dir.path().join(name));
        }
        (app, dir)
    }

    fn file_name(app: &App, document: DocumentId) -> String {
        app.documents[document]
            .buffer
            .path()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn tab_names(app: &App, group: u64) -> Vec<String> {
        app.editor_groups
            .group_views(group)
            .into_iter()
            .map(|view| file_name(app, app.editor_groups.view(view).unwrap().document))
            .collect()
    }

    fn focused_tabs(app: &App) -> Vec<String> {
        tab_names(app, app.editor_groups.focused())
    }

    fn active_name(app: &App) -> String {
        file_name(app, app.command_view_ref().document)
    }

    #[test]
    fn tabs_move_along_the_strip_and_the_active_tab_follows() {
        let (mut app, _dir) = app_with_three_tabs();
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt", "c.txt"]);
        app.run_ui_command("workbench.action.moveEditorLeftInGroup");
        assert_eq!(focused_tabs(&app), ["a.txt", "c.txt", "b.txt"]);
        assert_eq!(active_name(&app), "c.txt");
        app.run_ui_command("workbench.action.moveEditorToStart");
        assert_eq!(focused_tabs(&app), ["c.txt", "a.txt", "b.txt"]);
        app.run_ui_command("workbench.action.moveEditorLeftInGroup");
        assert_eq!(focused_tabs(&app)[0], "c.txt", "already first");
        app.run_ui_command("workbench.action.moveEditorRightInGroup");
        assert_eq!(focused_tabs(&app), ["a.txt", "c.txt", "b.txt"]);
        app.run_ui_command("workbench.action.moveEditorToEnd");
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt", "c.txt"]);
        app.run_ui_command("workbench.action.moveEditorRightInGroup");
        assert_eq!(focused_tabs(&app)[2], "c.txt", "already last");
    }

    #[test]
    fn first_and_last_editor_in_group_activate_the_ends() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.firstEditorInGroup");
        assert_eq!(active_name(&app), "a.txt");
        app.run_ui_command("workbench.action.lastEditorInGroup");
        assert_eq!(active_name(&app), "c.txt");
    }

    #[test]
    fn an_editor_moves_to_the_next_and_previous_group() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.newGroupRight");
        let groups = app.editor_groups.ordered_group_ids();
        assert_eq!(groups.len(), 2);
        // The new group holds a copy of c.txt. Send the active tab of the first group over.
        app.focus_editor_group(groups[0]);
        app.run_ui_command("workbench.action.moveEditorToNextGroup");
        assert_eq!(app.editor_groups.focused(), groups[1]);
        assert_eq!(tab_names(&app, groups[0]), ["a.txt", "b.txt"]);
        assert_eq!(
            tab_names(&app, groups[1]),
            ["c.txt"],
            "the group already had c.txt"
        );
        app.run_ui_command("workbench.action.moveEditorToPreviousGroup");
        assert_eq!(app.editor_groups.ordered_group_ids(), vec![groups[0]]);
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt", "c.txt"]);
    }

    #[test]
    fn splitting_into_another_group_copies_the_editor() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.newGroupRight");
        let groups = app.editor_groups.ordered_group_ids();
        app.focus_editor_group(groups[0]);
        app.run_ui_command("workbench.action.firstEditorInGroup");
        app.run_ui_command("workbench.action.splitEditorToLastGroup");
        assert_eq!(tab_names(&app, groups[0]), ["a.txt", "b.txt", "c.txt"]);
        assert_eq!(tab_names(&app, groups[1]), ["c.txt", "a.txt"]);
        assert_eq!(app.editor_groups.focused(), groups[1]);
    }

    #[test]
    fn moving_to_another_group_is_refused_with_a_single_group() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.moveEditorToNextGroup");
        assert_eq!(app.editor_groups.group_count(), 1);
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt", "c.txt"]);
    }

    #[test]
    fn new_groups_open_on_each_side_with_a_copy_of_the_editor() {
        for (command, before) in [
            ("workbench.action.newGroupLeft", true),
            ("workbench.action.newGroupRight", false),
            ("workbench.action.newGroupAbove", true),
            ("workbench.action.newGroupBelow", false),
        ] {
            let (mut app, _dir) = app_with_three_tabs();
            let original = app.editor_groups.focused();
            app.run_ui_command(command);
            let groups = app.editor_groups.ordered_group_ids();
            assert_eq!(groups.len(), 2, "{command}");
            assert_eq!(groups[usize::from(before)], original, "{command}");
            assert_ne!(app.editor_groups.focused(), original, "{command}");
        }
    }

    #[test]
    fn groups_join_into_one() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.newGroupRight");
        app.run_ui_command("workbench.action.newGroupBelow");
        assert_eq!(app.editor_groups.group_count(), 3);
        app.run_ui_command("workbench.action.joinTwoGroups");
        assert_eq!(app.editor_groups.group_count(), 2);
        app.run_ui_command("workbench.action.joinAllGroups");
        assert_eq!(app.editor_groups.group_count(), 1);
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt", "c.txt"]);
    }

    #[test]
    fn closing_editors_in_bulk_keeps_what_has_unsaved_changes() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.closeEditorsToTheLeft");
        assert_eq!(focused_tabs(&app), ["c.txt"]);

        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.firstEditorInGroup");
        app.editor_mut().insert("x");
        app.run_ui_command("workbench.action.lastEditorInGroup");
        app.run_ui_command("workbench.action.closeAllGroups");
        // a.txt has unsaved changes and stays; the rest closed.
        assert_eq!(focused_tabs(&app), ["a.txt"]);
    }

    #[test]
    fn closing_in_other_groups_and_in_all_groups() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.newGroupRight");
        let groups = app.editor_groups.ordered_group_ids();
        app.focus_editor_group(groups[0]);
        app.run_ui_command("workbench.action.closeEditorsInOtherGroups");
        assert_eq!(app.editor_groups.group_count(), 1);
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt", "c.txt"]);

        app.run_ui_command("workbench.action.closeEditorInAllGroups");
        assert_eq!(focused_tabs(&app), ["a.txt", "b.txt"]);
    }

    #[test]
    fn focus_commands_pick_the_first_and_last_group() {
        let (mut app, _dir) = app_with_three_tabs();
        app.run_ui_command("workbench.action.newGroupRight");
        let groups = app.editor_groups.ordered_group_ids();
        app.run_ui_command("workbench.action.focusFirstEditorGroup");
        assert_eq!(app.editor_groups.focused(), groups[0]);
        app.run_ui_command("workbench.action.focusLastEditorGroup");
        assert_eq!(app.editor_groups.focused(), groups[1]);
        app.run_ui_command("workbench.action.focusActiveEditorGroup");
        assert_eq!(app.editor_groups.focused(), groups[1]);
    }
}
