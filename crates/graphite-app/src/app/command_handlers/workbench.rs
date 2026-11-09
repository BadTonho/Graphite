use super::*;

pub(super) fn workbench_action_quick_open(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.context_keys.close_widget(WidgetKind::QuickOpen);
    app.start_quick_open();
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
    _app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        let dark = !DARK_THEME.load(Ordering::Relaxed);
        DARK_THEME.store(dark, Ordering::Relaxed);
        config::save_dark_theme(dark);
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
    {
        app.switch_recent_document(false);
    };
    Ok(())
}

pub(super) fn workbench_action_previous_editor(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.switch_recent_document(true);
    Ok(())
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
