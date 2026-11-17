use super::*;

pub(super) fn register(registry: &mut CommandRegistry) {
    for (id, title, widget, enablement, handler) in [
        (
            "graphite.close.save",
            "Save and close",
            Some(WidgetKind::Close),
            "closeConfirmation",
            close_save as Handler,
        ),
        (
            "graphite.close.discard",
            "Discard and close",
            Some(WidgetKind::Close),
            "closeConfirmation",
            close_discard as Handler,
        ),
        (
            "graphite.close.cancel",
            "Cancel close",
            Some(WidgetKind::Close),
            "closeConfirmation",
            close_cancel as Handler,
        ),
        (
            "workbench.action.quit",
            "Close window",
            None,
            "true",
            quit as Handler,
        ),
        (
            "graphite.window.saveAndClose",
            "Save all and close window",
            Some(WidgetKind::WindowClose),
            "windowCloseConfirmation",
            window_save as Handler,
        ),
        (
            "graphite.window.discardAndClose",
            "Discard and close window",
            Some(WidgetKind::WindowClose),
            "windowCloseConfirmation",
            window_discard as Handler,
        ),
        (
            "graphite.window.cancelClose",
            "Cancel window close",
            Some(WidgetKind::WindowClose),
            "windowCloseConfirmation",
            window_cancel as Handler,
        ),
        (
            "graphite.scm.commit",
            "Commit changes",
            None,
            "true",
            commit as Handler,
        ),
        (
            "graphite.scm.stageHunk",
            "Stage first hunk",
            None,
            "true",
            stage_hunk as Handler,
        ),
        (
            "graphite.scm.openConflict",
            "Open conflict",
            None,
            "true",
            conflict as Handler,
        ),
        (
            "graphite.scm.openBranchPicker",
            "Select branch",
            None,
            "true",
            branch_picker as Handler,
        ),
        (
            "graphite.scm.checkout",
            "Checkout branch",
            Some(WidgetKind::Branch),
            "branchPickerVisible && widgetHasItems",
            checkout as Handler,
        ),
        (
            "graphite.diff.toggleLayout",
            "Toggle diff layout",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_layout as Handler,
        ),
        (
            "diffEditor.switchSide",
            "Switch sides in diff editor",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_switch_side as Handler,
        ),
        (
            "diffEditor.collapseAllUnchangedRegions",
            "Collapse all unchanged regions",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_collapse_unchanged as Handler,
        ),
        (
            "diffEditor.showAllUnchangedRegions",
            "Show all unchanged regions",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_show_unchanged as Handler,
        ),
        (
            "workbench.action.compareEditor.nextChange",
            "Go to next change in diff editor",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_next_change as Handler,
        ),
        (
            "workbench.action.compareEditor.previousChange",
            "Go to previous change in diff editor",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_previous_change as Handler,
        ),
        (
            "graphite.terminal.new",
            "New terminal",
            None,
            "true",
            terminal_new as Handler,
        ),
        (
            "graphite.terminal.close",
            "Close terminal",
            None,
            "true",
            terminal_close as Handler,
        ),
        (
            "graphite.terminal.split",
            "Split terminal",
            None,
            "true",
            terminal_split as Handler,
        ),
        (
            "graphite.terminal.runActiveFile",
            "Run active file in terminal",
            None,
            "true",
            run_active_file_command as Handler,
        ),
        (
            "graphite.terminal.runSelectedText",
            "Run selected text in terminal",
            None,
            "true",
            run_selected_text_command as Handler,
        ),
        (
            "graphite.tasks.pick",
            "Run task",
            None,
            "true",
            pick_task_command as Handler,
        ),
        (
            "graphite.tasks.showRunning",
            "Show running tasks",
            None,
            "true",
            show_running_tasks_command as Handler,
        ),
        (
            "graphite.tasks.restartRunning",
            "Restart running task",
            None,
            "true",
            restart_running_task_command as Handler,
        ),
        (
            "graphite.tasks.configure",
            "Configure tasks",
            None,
            "true",
            configure_tasks_command as Handler,
        ),
        (
            "graphite.tasks.configureDefaultBuild",
            "Configure default build task",
            None,
            "true",
            configure_default_build_task_command as Handler,
        ),
        (
            "graphite.terminal.nextProfile",
            "Next terminal profile",
            None,
            "true",
            terminal_profile as Handler,
        ),
        (
            "graphite.terminal.previousCommand",
            "Go to previous command",
            None,
            "true",
            terminal_previous_command as Handler,
        ),
        (
            "graphite.terminal.nextCommand",
            "Go to next command",
            None,
            "true",
            terminal_next_command as Handler,
        ),
        (
            "graphite.terminal.clear",
            "Clear terminal",
            None,
            "true",
            terminal_clear as Handler,
        ),
        (
            "graphite.terminal.scrollLeft",
            "Scroll terminal tabs left",
            None,
            "true",
            terminal_left as Handler,
        ),
        (
            "graphite.terminal.scrollRight",
            "Scroll terminal tabs right",
            None,
            "true",
            terminal_right as Handler,
        ),
        (
            "graphite.editor.toggleLineEnding",
            "Toggle line endings",
            None,
            "editorWritable",
            line_ending as Handler,
        ),
        (
            "graphite.editor.nextEncoding",
            "Next encoding",
            None,
            "editorWritable",
            encoding as Handler,
        ),
    ] {
        add(
            registry,
            id,
            title,
            "Workbench",
            if widget.is_some() {
                WIDGET
            } else if id.starts_with("graphite.editor.") {
                VIEW
            } else {
                ALL
            },
            enablement,
            widget,
            handler,
        );
    }
    for (id, title, arguments, handler) in [
        (
            "graphite.terminal.select",
            "Select terminal",
            ArgumentKind::Index,
            select_terminal as Handler,
        ),
        (
            "graphite.conflict.resolve",
            "Resolve conflict",
            ArgumentKind::Index,
            resolve_conflict as Handler,
        ),
        (
            "graphite.editor.addCursor",
            "Add cursor",
            ArgumentKind::Index,
            add_cursor as Handler,
        ),
    ] {
        registry
            .register(CommandDescriptor {
                id,
                title,
                category: "Workbench",
                targets: if id == "graphite.editor.addCursor" {
                    VIEW
                } else {
                    ALL
                },
                enablement: if id == "graphite.editor.addCursor" {
                    "documentAvailable"
                } else {
                    "true"
                },
                menu_when: None,
                arguments,
                widget: None,
                handler,
            })
            .unwrap();
    }
    add(
        registry,
        "graphite.view.focus",
        "Focus editor view",
        "Editor",
        VIEW,
        "documentAvailable",
        None,
        focus_view,
    );
}

fn resolve_close(app: &mut App, save: bool, cancel: bool) -> Result<(), CommandError> {
    let (execution, view) = app
        .command_runtime
        .pending_close
        .take()
        .ok_or(CommandError::TargetClosed)?;
    let previous = app.command_runtime.view.replace(view);
    app.close_confirm = false;
    let result = if cancel {
        Err(CommandError::Cancelled)
    } else if app.editor_groups.view(view).is_none() {
        Err(CommandError::TargetClosed)
    } else {
        if save {
            let document = app.command_document();
            app.save_batch(
                vec![document],
                super::super::saving::AfterSave::Close { execution, view },
            );
            app.command_runtime.view = previous;
            return Ok(());
        }
        let document = app.command_document();
        app.defer_discard(
            vec![document],
            super::super::saving::DiscardAction::Close { execution, view },
        );
        app.command_runtime.view = previous;
        return Ok(());
    };
    app.command_runtime.view = previous;
    app.finish_command(execution, result);
    Ok(())
}
fn close_save(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    resolve_close(app, true, false)
}
fn close_discard(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    resolve_close(app, false, false)
}
fn close_cancel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    resolve_close(app, false, true)
}
fn quit(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    if app.saving.transition_pending()
        || app.command_runtime.pending_window_close.is_some()
        || app.command_runtime.pending_close.is_some()
    {
        return Err(CommandError::Busy);
    }
    app.request_window_close();
    if !app.close_window_confirm && app.saving.pending() {
        app.exit_requested = false;
        app.command_job(false);
        let documents = app
            .documents
            .iter()
            .filter(|d| app.saving.document_pending(d.id))
            .map(|d| d.id)
            .collect();
        app.save_batch(
            documents,
            super::super::saving::AfterSave::Window {
                execution: Some(invocation.execution),
            },
        );
    }
    if app.close_window_confirm {
        app.command_runtime.pending_window_close = Some(invocation.execution);
        app.command_runtime
            .pending
            .get_mut(&invocation.execution)
            .unwrap()
            .deferred = true;
    }
    Ok(())
}
fn resolve_window(app: &mut App, save: bool, cancel: bool) -> Result<(), CommandError> {
    if app.pending_commit_save.is_some() {
        app.resolve_commit_save(save, cancel);
        return Ok(());
    }
    if let Some(document) = app.pending_revert_confirmation.take() {
        app.close_window_confirm = false;
        let execution = app.command_runtime.pending_window_close.take();
        if cancel {
            if let Some(execution) = execution {
                app.finish_command(execution, Err(CommandError::Cancelled));
            }
        } else if save {
            app.save_batch(
                vec![document],
                super::super::saving::AfterSave::Revert {
                    document,
                    execution,
                },
            );
        } else {
            app.defer_discard(
                vec![document],
                super::super::saving::DiscardAction::Revert {
                    document,
                    execution,
                },
            );
        }
        return Ok(());
    }
    if app.workspace_decision_pending() {
        app.resolve_workspace_save(save, cancel);
        return Ok(());
    }
    let execution = app
        .command_runtime
        .pending_window_close
        .take()
        .ok_or(CommandError::TargetClosed)?;
    let result = if cancel {
        app.close_window_confirm = false;
        Err(CommandError::Cancelled)
    } else if save {
        app.close_window_confirm = false;
        let documents = app
            .documents
            .iter()
            .filter(|d| d.is_dirty() || app.saving.document_pending(d.id))
            .map(|d| d.id)
            .collect();
        app.save_batch(
            documents,
            super::super::saving::AfterSave::Window {
                execution: Some(execution),
            },
        );
        return Ok(());
    } else {
        app.close_window_confirm = false;
        let documents = app.documents.ids().collect();
        app.defer_discard(
            documents,
            super::super::saving::DiscardAction::Window { execution },
        );
        return Ok(());
    };
    app.finish_command(execution, result);
    Ok(())
}
fn window_save(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    resolve_window(app, true, false)
}
fn window_discard(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    resolve_window(app, false, false)
}
fn window_cancel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    resolve_window(app, false, true)
}
fn commit(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_commit_prompt = false;
    app.perform_scm_action(crate::app::scm_ui::ScmAction::Commit(
        crate::scm_commit::CommitOptions::PLAIN,
    ));
    Ok(())
}
fn stage_hunk(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.perform_scm_action(crate::app::scm_ui::ScmAction::StageHunk(0));
    Ok(())
}
fn conflict(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_selected_conflict();
    Ok(())
}
fn branch_picker(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_branch_picker();
    Ok(())
}
fn checkout(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.confirm_branch_picker();
    Ok(())
}
fn diff_layout(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(diff) = &mut app.diff_view {
        diff.inline = !diff.inline;
    }
    Ok(())
}
fn diff_switch_side(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(diff) = &mut app.diff_view {
        std::mem::swap(&mut diff.left_name, &mut diff.right_name);
        std::mem::swap(&mut diff.left, &mut diff.right);
    }
    Ok(())
}
fn diff_collapse_unchanged(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(diff) = &mut app.diff_view {
        diff.collapse_unchanged = true;
    }
    Ok(())
}
fn diff_show_unchanged(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(diff) = &mut app.diff_view {
        diff.collapse_unchanged = false;
    }
    Ok(())
}
fn diff_next_change(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    diff_step_change(app, true);
    Ok(())
}
fn diff_previous_change(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    diff_step_change(app, false);
    Ok(())
}
fn diff_step_change(app: &mut App, forward: bool) {
    let Some(diff) = &mut app.diff_view else {
        return;
    };
    let mut changed_lines = Vec::new();
    let mut line_index = 0usize;
    let mut in_change = false;
    for change in similar::TextDiff::from_lines(&diff.left, &diff.right).iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Equal => in_change = false,
            similar::ChangeTag::Delete | similar::ChangeTag::Insert => {
                if !in_change {
                    changed_lines.push(line_index);
                    in_change = true;
                }
            }
        }
        line_index += 1;
    }
    if changed_lines.is_empty() {
        return;
    }
    let current = diff.selected_change;
    let next = if forward {
        changed_lines
            .iter()
            .copied()
            .find(|&l| l > current)
            .unwrap_or(changed_lines[0])
    } else {
        changed_lines
            .iter()
            .rev()
            .copied()
            .find(|&l| l < current)
            .unwrap_or(*changed_lines.last().unwrap())
    };
    diff.selected_change = next;
}
fn terminal_new(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.create_terminal_tab();
    Ok(())
}
fn terminal_close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_focused_terminal();
    Ok(())
}
fn terminal_split(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_terminal_split();
    Ok(())
}
fn terminal_profile(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cycle_terminal_profile();
    Ok(())
}
fn run_active_file_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_run_active_file().map_err(CommandError::Failed)
}
fn run_selected_text_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_run_selected_text()
        .map_err(CommandError::Failed)
}
fn pick_task_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_pick_task();
    Ok(())
}
fn show_running_tasks_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_show_running_tasks()
        .map_err(CommandError::Failed)
}
fn restart_running_task_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_restart_running_task()
        .map_err(CommandError::Failed)
}
fn configure_tasks_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_task_configuration();
    Ok(())
}
fn configure_default_build_task_command(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    app.open_task_configuration();
    Ok(())
}
fn terminal_previous_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.jump_terminal_prompt(false);
    Ok(())
}
fn terminal_next_command(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.jump_terminal_prompt(true);
    Ok(())
}
fn terminal_clear(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.clear_focused_terminal();
    Ok(())
}
fn terminal_left(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scroll_terminal_tabs(-1);
    Ok(())
}
fn terminal_right(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scroll_terminal_tabs(1);
    Ok(())
}
fn select_terminal(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let index = invocation.request.arguments["index"].as_u64().unwrap() as usize;
    if index >= app.terminals.len() {
        return Err(CommandError::InvalidArguments(
            app.t("error.terminal_index").into(),
        ));
    }
    app.select_terminal(index);
    Ok(())
}
fn resolve_conflict(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let index = invocation.request.arguments["index"].as_u64().unwrap() as usize;
    if index > 3 || app.conflict_view.is_none() {
        return Err(CommandError::InvalidArguments(
            app.t("error.conflict_choice").into(),
        ));
    }
    app.resolve_conflict_choice(index);
    Ok(())
}
fn add_cursor(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.editor_mut()
        .add_cursor(invocation.request.arguments["index"].as_u64().unwrap() as usize);
    Ok(())
}
fn line_ending(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let id = app.command_document();
    app.documents[id].line_ending = app.documents[id].line_ending.toggle();
    Ok(())
}
fn encoding(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let id = app.command_document();
    app.documents[id].encoding = app.documents[id].encoding.next();
    Ok(())
}
fn focus_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let id = app.command_view_ref().id;
    let group = app
        .editor_groups
        .group_for_view(id)
        .ok_or(CommandError::TargetClosed)?;
    let index = app
        .editor_groups
        .views(group)
        .unwrap()
        .0
        .iter()
        .position(|view| view.id == id)
        .unwrap();
    app.focus_editor_group(group);
    app.editor_groups.select_tab(index);
    Ok(())
}
