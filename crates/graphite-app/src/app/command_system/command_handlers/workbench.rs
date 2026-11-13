use super::*;

pub(super) fn activity_bar_default(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_activity_bar_position(crate::config::ActivityBarPosition::Default);
    Ok(())
}

pub(super) fn activity_bar_top(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.set_activity_bar_position(crate::config::ActivityBarPosition::Top);
    Ok(())
}

pub(super) fn activity_bar_bottom(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_activity_bar_position(crate::config::ActivityBarPosition::Bottom);
    Ok(())
}

pub(super) fn activity_bar_hidden(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.set_activity_bar_position(crate::config::ActivityBarPosition::Hidden);
    Ok(())
}

pub(super) fn activity_bar_toggle_size(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    let size = if app.activity_bar_preferences.size == crate::config::ActivityBarSize::Default {
        crate::config::ActivityBarSize::Compact
    } else {
        crate::config::ActivityBarSize::Default
    };
    app.set_activity_bar_size(size);
    Ok(())
}

pub(super) fn activity_bar_move_sidebar(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_primary_sidebar_side();
    Ok(())
}

pub(super) fn workbench_action_toggle_menu_bar(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.view_preferences.menu_bar_visible = !app.view_preferences.menu_bar_visible;
    app.save_view_preferences_for_command();
    Ok(())
}

pub(super) fn workbench_action_toggle_primary_sidebar(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.view_preferences.primary_sidebar_visible = !app.view_preferences.primary_sidebar_visible;
    app.save_view_preferences_for_command();
    Ok(())
}

pub(super) fn workbench_action_quick_open(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.context_keys.close_widget(WidgetKind::QuickOpen);
    app.start_quick_open();
    Ok(())
}

pub(super) fn workbench_action_show_commands(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.open_command_palette();
    Ok(())
}

pub(super) fn workbench_action_tasks_build(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let file = app.task_file().map_err(CommandError::Failed)?;
    let task = file
        .default_build()
        .ok_or_else(|| CommandError::Disabled("No default build task is configured".into()))?;
    app.run_task(&task.label).map_err(CommandError::Failed)
}

pub(super) fn workbench_action_tasks_cancel(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.cancel_task();
    Ok(())
}

pub(super) fn graphite_security_trust_workspace(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let workspace = app.tree.root().to_path_buf();
    let Some(trust) = app.workspace_trust.as_mut() else {
        return Err(CommandError::Failed(
            "Workspace trust storage is unavailable".into(),
        ));
    };
    trust.trust(&workspace).map_err(CommandError::Failed)?;
    app.start_workspace_trusted_plugins();
    app.status = "Workspace trusted. Plugin-specific trust is still required.".into();
    Ok(())
}

pub(super) fn graphite_security_revoke_workspace_trust(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let workspace = app.tree.root().to_path_buf();
    let Some(trust) = app.workspace_trust.as_mut() else {
        return Err(CommandError::Failed(
            "Workspace trust storage is unavailable".into(),
        ));
    };
    trust.revoke(&workspace).map_err(CommandError::Failed)?;
    app.cancel_task();
    let _ = app.debug.stop();
    app.test_explorer.active_run = None;
    app.debug_panel_visible = false;
    app.tests_visible = false;
    app.close_webview();
    app.stop_workspace_plugins();
    if let Some(host) = app.extension_host.supervisor.as_mut() {
        host.shutdown();
    }
    app.clear_host_session();
    app.status = "Workspace trust revoked; tasks and extension hosts stopped.".into();
    Ok(())
}

pub(super) fn workbench_view_scm(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_scm();
    Ok(())
}

pub(super) fn workbench_action_toggle_zen_mode(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_zen_mode();
    Ok(())
}

pub(super) fn workbench_action_toggle_centered_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_centered_editor();
    Ok(())
}

pub(super) fn workbench_action_toggle_outline(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_outline();
    Ok(())
}

pub(super) fn workbench_action_show_process_explorer(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_process_explorer();
    Ok(())
}

pub(super) fn workbench_action_toggle_screencast_mode(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_screencast_mode();
    Ok(())
}

pub(super) fn graphite_action_diff_active_file(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.open_active_file_diff();
    };
    Ok(())
}

pub(super) fn graphite_action_close_diff_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.diff_view = None;
    Ok(())
}

pub(super) fn workbench_action_split_editor_right(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.split_editor_group(crate::app::editor_groups::SplitDirection::Vertical);
    };
    Ok(())
}

pub(super) fn workbench_action_split_editor_down(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.split_editor_group(crate::app::editor_groups::SplitDirection::Horizontal);
    };
    Ok(())
}

pub(super) fn workbench_action_focus_next_group(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.focus_next_editor_group();
    Ok(())
}

pub(super) fn workbench_action_find_in_files(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.open_project_search();
    Ok(())
}

pub(super) fn graphite_action_toggle_theme(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        let dark = !DARK_THEME.load(Ordering::Relaxed);
        DARK_THEME.store(dark, Ordering::Relaxed);
        config::save_dark_theme_in(&app.profile_root, dark);
    };
    Ok(())
}

pub(super) fn workbench_actions_view_problems(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        app.terminal_visible = true;
        app.terminal_problems = true;
        app.terminal_focused = false;
        app.resize_terminal();
    };
    Ok(())
}

pub(super) fn workbench_action_next_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.switch_go_editor(false, false, false);
    Ok(())
}

pub(super) fn workbench_action_previous_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.switch_go_editor(true, false, false);
    Ok(())
}

fn goto_line(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_picker = Some(crate::app::go_menu::GoPicker {
        kind: crate::app::go_menu::GoPickerKind::LineColumn,
        query: String::new(),
        items: Vec::new(),
        selected: 0,
        loading: false,
        generation: 0,
        message: None,
    });
    Ok(())
}
fn focus_group_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_number(1);
    Ok(())
}
fn focus_group_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_number(2);
    Ok(())
}
fn focus_group_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_number(3);
    Ok(())
}
fn focus_group_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_number(4);
    Ok(())
}
fn focus_group_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_number(5);
    Ok(())
}
fn focus_next_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_step(false);
    Ok(())
}
fn focus_previous_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_group_step(true);
    Ok(())
}
fn focus_left_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_adjacent_group(crate::app::go_menu::GoSubcommand::GroupLeft);
    Ok(())
}
fn focus_right_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_adjacent_group(crate::app::go_menu::GoSubcommand::GroupRight);
    Ok(())
}
fn focus_above_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_adjacent_group(crate::app::go_menu::GoSubcommand::GroupAbove);
    Ok(())
}
fn focus_below_group(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.focus_adjacent_group(crate::app::go_menu::GoSubcommand::GroupBelow);
    Ok(())
}
fn unsupported_go(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    Err(CommandError::Disabled(app.t("go.unsupported").into()))
}

pub(super) fn workbench_action_reopen_closed_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.reopen_closed_tab();
    Ok(())
}

pub(super) fn workbench_action_navigate_back(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.navigate_history(-1);
    Ok(())
}

pub(super) fn workbench_action_navigate_forward(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.navigate_history(1);
    Ok(())
}

pub(super) fn actions_find(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.context_keys.close_widget(WidgetKind::Find);
    app.open_local_search(false);
    Ok(())
}

pub(super) fn workbench_action_files_save(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let job = app.command_job(false).expect("save command ticket");
    app.save_batch(
        vec![app.command_document()],
        super::super::saving::AfterSave::Command(job),
    );
    Ok(())
}

pub(super) fn workbench_action_terminal_toggle_terminal(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.toggle_terminal();
    Ok(())
}

pub(super) fn workbench_action_close_active_editor(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if app.saving.transition_pending()
        || app.command_runtime.pending_close.is_some()
        || app.command_runtime.pending_window_close.is_some()
    {
        return Err(CommandError::Busy);
    }
    {
        if app.active_view_needs_confirmation() {
            app.close_confirm = true;
            app.command_runtime.pending_close =
                Some((invocation.execution, app.command_view_ref().id));
            if let Some(pending) = app.command_runtime.pending.get_mut(&invocation.execution) {
                pending.deferred = true;
            }
            app.status = app.t("status.unsaved_tab_prompt").into();
        } else if app.saving.document_pending(app.command_document()) {
            app.command_job(false);
            app.save_batch(
                vec![app.command_document()],
                super::super::saving::AfterSave::Close {
                    execution: invocation.execution,
                    view: app.command_view_ref().id,
                },
            );
        } else {
            app.close_active_tab();
        }
    };
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    for (id, title, handler) in [
        (
            "graphite.profiles.create",
            "Create Profile",
            graphite_profile_create as Handler,
        ),
        (
            "graphite.profiles.switch",
            "Switch Profile",
            graphite_profile_switch as Handler,
        ),
    ] {
        registry
            .register(CommandDescriptor {
                id,
                title,
                category: "Profiles",
                targets: ALL,
                enablement: "true",
                menu_when: None,
                widget: None,
                arguments: ArgumentKind::Text,
                handler,
            })
            .expect("unique profile command");
    }
    add(
        registry,
        "graphite.profiles.list",
        "List Profiles",
        "Profiles",
        ALL,
        "true",
        None,
        graphite_profile_list,
    );

    add(
        registry,
        "workbench.action.files.saveAll",
        "Save all",
        "File",
        ALL,
        "true",
        None,
        |app, _| {
            let job = app.command_job(false).expect("save all ticket");
            let documents = app
                .documents
                .iter()
                .filter(|d| d.is_dirty() || app.saving.document_pending(d.id))
                .map(|d| d.id)
                .collect();
            app.save_batch(documents, super::super::saving::AfterSave::Command(job));
            Ok(())
        },
    );

    add(
        registry,
        "graphite.security.revokeWorkspaceTrust",
        "Revoke Workspace Trust",
        "Security",
        ALL,
        "true",
        None,
        graphite_security_revoke_workspace_trust,
    );
    add(
        registry,
        "graphite.security.trustWorkspace",
        "Trust This Workspace",
        "Security",
        ALL,
        "true",
        None,
        graphite_security_trust_workspace,
    );
    add(
        registry,
        "workbench.action.showCommands",
        "Show All Commands",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_show_commands,
    );
    add(
        registry,
        "workbench.action.tasks.build",
        "Run Build Task",
        "Tasks",
        ALL,
        "true",
        None,
        workbench_action_tasks_build,
    );
    add(
        registry,
        "workbench.action.tasks.cancel",
        "Terminate Task",
        "Tasks",
        ALL,
        "true",
        None,
        workbench_action_tasks_cancel,
    );
    add(
        registry,
        "workbench.action.quickOpen",
        "Quick open",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_quick_open,
    );
    add(
        registry,
        "workbench.action.showAllSymbols",
        "Go to Symbol in Workspace",
        "Go",
        VIEW,
        "true",
        None,
        |app, _| {
            app.open_go_picker(crate::app::go_menu::GoPickerKind::WorkspaceSymbols);
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.gotoSymbol",
        "Go to Symbol in Editor",
        "Go",
        VIEW,
        "true",
        None,
        |app, _| {
            if !app.request_vsix_language(crate::app::plugins::VsixLanguageFeature::DocumentSymbols)
            {
                app.open_go_picker(crate::app::go_menu::GoPickerKind::EditorSymbols);
            }
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.gotoLine",
        "Go to Line/Column",
        "Go",
        VIEW,
        "documentAvailable",
        None,
        goto_line,
    );
    add(
        registry,
        "workbench.action.nextUsedEditor",
        "Next Used Editor",
        "Go",
        ALL,
        "true",
        None,
        |app, _| {
            app.switch_go_editor(false, false, true);
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.previousUsedEditor",
        "Previous Used Editor",
        "Go",
        ALL,
        "true",
        None,
        |app, _| {
            app.switch_go_editor(true, false, true);
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.nextEditorInGroup",
        "Next Editor in Group",
        "Go",
        ALL,
        "true",
        None,
        |app, _| {
            app.switch_go_editor(false, true, false);
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.previousEditorInGroup",
        "Previous Editor in Group",
        "Go",
        ALL,
        "true",
        None,
        |app, _| {
            app.switch_go_editor(true, true, false);
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.nextUsedEditorInGroup",
        "Next Used Editor in Group",
        "Go",
        ALL,
        "true",
        None,
        |app, _| {
            app.switch_go_editor(false, true, true);
            Ok(())
        },
    );
    add(
        registry,
        "workbench.action.previousUsedEditorInGroup",
        "Previous Used Editor in Group",
        "Go",
        ALL,
        "true",
        None,
        |app, _| {
            app.switch_go_editor(true, true, true);
            Ok(())
        },
    );
    let group_commands = [
        (
            "workbench.action.focusFirstGroup",
            "Group 1",
            focus_group_1 as Handler,
        ),
        (
            "workbench.action.focusSecondGroup",
            "Group 2",
            focus_group_2 as Handler,
        ),
        (
            "workbench.action.focusThirdGroup",
            "Group 3",
            focus_group_3 as Handler,
        ),
        (
            "workbench.action.focusFourthGroup",
            "Group 4",
            focus_group_4 as Handler,
        ),
        (
            "workbench.action.focusFifthGroup",
            "Group 5",
            focus_group_5 as Handler,
        ),
    ];
    for (id, title, handler) in group_commands {
        add(registry, id, title, "Go", ALL, "true", None, handler);
    }
    add(
        registry,
        "workbench.action.focusPreviousGroup",
        "Previous Group",
        "Go",
        ALL,
        "true",
        None,
        focus_previous_group,
    );
    add(
        registry,
        "workbench.action.focusLeftGroup",
        "Group Left",
        "Go",
        ALL,
        "true",
        None,
        focus_left_group,
    );
    add(
        registry,
        "workbench.action.focusRightGroup",
        "Group Right",
        "Go",
        ALL,
        "true",
        None,
        focus_right_group,
    );
    add(
        registry,
        "workbench.action.focusAboveGroup",
        "Group Above",
        "Go",
        ALL,
        "true",
        None,
        focus_above_group,
    );
    add(
        registry,
        "workbench.action.focusBelowGroup",
        "Group Below",
        "Go",
        ALL,
        "true",
        None,
        focus_below_group,
    );
    for (id, title) in [
        (
            "workbench.action.navigateToLastEditLocation",
            "Last Edit Location",
        ),
    ] {
        add(
            registry,
            id,
            title,
            "Go",
            VIEW,
            "false",
            None,
            unsupported_go,
        );
    }
    add(
        registry,
        "workbench.view.scm",
        "Scm",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_view_scm,
    );
    add(
        registry,
        "workbench.action.toggleMenuBar",
        "Toggle Menu Bar",
        "View",
        ALL,
        "true",
        None,
        workbench_action_toggle_menu_bar,
    );
    add(
        registry,
        "workbench.action.togglePrimarySideBar",
        "Toggle Primary Side Bar",
        "View",
        ALL,
        "true",
        None,
        workbench_action_toggle_primary_sidebar,
    );
    add(
        registry,
        "workbench.action.toggleZenMode",
        "Toggle zen mode",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_toggle_zen_mode,
    );
    add(
        registry,
        "workbench.action.toggleCenteredEditor",
        "Toggle centered editor",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_toggle_centered_editor,
    );
    add(
        registry,
        "workbench.action.toggleOutline",
        "Toggle outline",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_toggle_outline,
    );
    add(
        registry,
        "workbench.action.showProcessExplorer",
        "Show process explorer",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_show_process_explorer,
    );
    add(
        registry,
        "graphite.help.welcome",
        "Welcome",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_help_modal(super::super::help_menu::HelpPage::Welcome);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.editorPlayground",
        "Editor Playground",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_help_modal(super::super::help_menu::HelpPage::Playground);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.openWalkthrough",
        "Open Walkthrough",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_help_modal(super::super::help_menu::HelpPage::Walkthrough);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.releaseNotes",
        "Show Release Notes",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_help_modal(super::super::help_menu::HelpPage::ReleaseNotes);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.keyboardShortcuts",
        "Keyboard Shortcuts Reference",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_help_modal(super::super::help_menu::HelpPage::KeyboardShortcuts);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.developerTools",
        "Toggle Developer Tools",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            if app.help_modal.as_ref().is_some_and(|modal| {
                modal.page == super::super::help_menu::HelpPage::DeveloperTools
            }) {
                app.close_help_modal();
            } else {
                app.open_help_modal(super::super::help_menu::HelpPage::DeveloperTools);
            }
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.about",
        "About",
        "Help",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_help_modal(super::super::help_menu::HelpPage::About);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.help.restartToUpdate",
        "Restart to Update",
        "Help",
        ALL,
        "false",
        None,
        |_, _| Ok(()),
    );
    add(
        registry,
        "workbench.action.toggleScreencastMode",
        "Toggle screencast mode",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_toggle_screencast_mode,
    );
    add(
        registry,
        "graphite.action.diffActiveFile",
        "Diff active file",
        "Workbench",
        VIEW,
        "documentHasPath && !diffLoading",
        None,
        graphite_action_diff_active_file,
    );
    add(
        registry,
        "graphite.action.closeDiffEditor",
        "Close diff editor",
        "Workbench",
        ALL,
        "true",
        None,
        graphite_action_close_diff_editor,
    );
    add(
        registry,
        "workbench.action.splitEditorRight",
        "Split editor right",
        "Workbench",
        VIEW,
        "true",
        None,
        workbench_action_split_editor_right,
    );
    add(
        registry,
        "workbench.action.splitEditorDown",
        "Split editor down",
        "Workbench",
        VIEW,
        "true",
        None,
        workbench_action_split_editor_down,
    );
    add(
        registry,
        "workbench.action.focusNextGroup",
        "Focus next group",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_focus_next_group,
    );
    add(
        registry,
        "workbench.action.findInFiles",
        "Find in files",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_find_in_files,
    );
    add(
        registry,
        "workbench.action.replaceInFiles",
        "Replace in files",
        "Workbench",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_project_search_mode(true);
            Ok(())
        },
    );
    add(
        registry,
        "graphite.action.toggleTheme",
        "Toggle theme",
        "Workbench",
        ALL,
        "true",
        None,
        graphite_action_toggle_theme,
    );
    add(
        registry,
        "workbench.actions.view.problems",
        "Problems",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_actions_view_problems,
    );
    add(
        registry,
        "workbench.action.nextEditor",
        "Next editor",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_next_editor,
    );
    add(
        registry,
        "workbench.action.previousEditor",
        "Previous editor",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_previous_editor,
    );
    add(
        registry,
        "workbench.action.reopenClosedEditor",
        "Reopen closed editor",
        "Workbench",
        ALL,
        "hasClosedTabs",
        None,
        workbench_action_reopen_closed_editor,
    );
    add(
        registry,
        "workbench.action.navigateBack",
        "Navigate back",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_navigate_back,
    );
    add(
        registry,
        "workbench.action.navigateForward",
        "Navigate forward",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_navigate_forward,
    );
    add(
        registry,
        "actions.find",
        "Find",
        "Workbench",
        VIEW,
        "true",
        None,
        actions_find,
    );
    add(
        registry,
        "workbench.action.files.save",
        "Save",
        "Workbench",
        VIEW,
        "documentHasPath && editorWritable",
        None,
        workbench_action_files_save,
    );
    add(
        registry,
        "workbench.action.terminal.toggleTerminal",
        "Toggle terminal",
        "Workbench",
        ALL,
        "true",
        None,
        workbench_action_terminal_toggle_terminal,
    );
    add(
        registry,
        "workbench.action.closeActiveEditor",
        "Close active editor",
        "Workbench",
        VIEW,
        "documentAvailable && !closeConfirmation",
        None,
        workbench_action_close_active_editor,
    );
}

fn graphite_profile_create(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let name =
        invocation.request.arguments["text"]
            .as_str()
            .ok_or(CommandError::InvalidArguments(
                "Profile name required".into(),
            ))?;
    app.create_user_profile(name).map_err(CommandError::Failed)
}

fn graphite_profile_switch(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let name =
        invocation.request.arguments["text"]
            .as_str()
            .ok_or(CommandError::InvalidArguments(
                "Profile name required".into(),
            ))?;
    app.switch_user_profile(name).map_err(CommandError::Failed)
}

fn graphite_profile_list(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let profiles = app.user_profiles().map_err(CommandError::Failed)?;
    app.status = format!("Profiles: {}", profiles.join(", "));
    Ok(())
}
