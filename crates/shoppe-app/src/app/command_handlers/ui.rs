use super::*;
use serde_json::Value;

pub(super) fn register(registry: &mut CommandRegistry) {
    for (id, title, widget, enablement, handler) in [
        (
            "shoppe.close.save",
            "Save and close",
            Some(WidgetKind::Close),
            "closeConfirmation",
            close_save as Handler,
        ),
        (
            "shoppe.close.discard",
            "Discard and close",
            Some(WidgetKind::Close),
            "closeConfirmation",
            close_discard as Handler,
        ),
        (
            "shoppe.close.cancel",
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
            "shoppe.window.saveAndClose",
            "Save all and close window",
            Some(WidgetKind::WindowClose),
            "windowCloseConfirmation",
            window_save as Handler,
        ),
        (
            "shoppe.window.discardAndClose",
            "Discard and close window",
            Some(WidgetKind::WindowClose),
            "windowCloseConfirmation",
            window_discard as Handler,
        ),
        (
            "shoppe.window.cancelClose",
            "Cancel window close",
            Some(WidgetKind::WindowClose),
            "windowCloseConfirmation",
            window_cancel as Handler,
        ),
        (
            "shoppe.scm.commit",
            "Commit changes",
            None,
            "true",
            commit as Handler,
        ),
        (
            "shoppe.scm.stageHunk",
            "Stage first hunk",
            None,
            "true",
            stage_hunk as Handler,
        ),
        (
            "shoppe.scm.openConflict",
            "Open conflict",
            None,
            "true",
            conflict as Handler,
        ),
        (
            "shoppe.scm.openBranchPicker",
            "Select branch",
            None,
            "true",
            branch_picker as Handler,
        ),
        (
            "shoppe.scm.checkout",
            "Checkout branch",
            Some(WidgetKind::Branch),
            "branchPickerVisible && widgetHasItems",
            checkout as Handler,
        ),
        (
            "shoppe.diff.toggleLayout",
            "Toggle diff layout",
            Some(WidgetKind::Diff),
            "diffVisible",
            diff_layout as Handler,
        ),
        (
            "shoppe.terminal.new",
            "New terminal",
            None,
            "true",
            terminal_new as Handler,
        ),
        (
            "shoppe.terminal.close",
            "Close terminal",
            None,
            "true",
            terminal_close as Handler,
        ),
        (
            "shoppe.terminal.split",
            "Split terminal",
            None,
            "true",
            terminal_split as Handler,
        ),
        (
            "shoppe.terminal.nextProfile",
            "Next terminal profile",
            None,
            "true",
            terminal_profile as Handler,
        ),
        (
            "shoppe.terminal.scrollLeft",
            "Scroll terminal tabs left",
            None,
            "true",
            terminal_left as Handler,
        ),
        (
            "shoppe.terminal.scrollRight",
            "Scroll terminal tabs right",
            None,
            "true",
            terminal_right as Handler,
        ),
        (
            "shoppe.editor.toggleLineEnding",
            "Toggle line endings",
            None,
            "editorWritable",
            line_ending as Handler,
        ),
        (
            "shoppe.editor.nextEncoding",
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
            } else if id.starts_with("shoppe.editor.") {
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
            "shoppe.terminal.select",
            "Select terminal",
            ArgumentKind::Index,
            select_terminal as Handler,
        ),
        (
            "shoppe.conflict.resolve",
            "Resolve conflict",
            ArgumentKind::Index,
            resolve_conflict as Handler,
        ),
        (
            "shoppe.editor.addCursor",
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
                targets: if id == "shoppe.editor.addCursor" {
                    VIEW
                } else {
                    ALL
                },
                enablement: if id == "shoppe.editor.addCursor" {
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
        "shoppe.view.focus",
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
            app.save_document(app.command_document());
        }
        if save && app.documents[app.command_document()].is_dirty() {
            Err(CommandError::Failed(app.status.clone()))
        } else {
            app.close_active_tab();
            Ok(Value::Null)
        }
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
    app.request_window_close();
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
    let execution = app
        .command_runtime
        .pending_window_close
        .take()
        .ok_or(CommandError::TargetClosed)?;
    let result = if cancel {
        app.close_window_confirm = false;
        Err(CommandError::Cancelled)
    } else if save {
        app.save_all_before_close();
        if app.exit_requested {
            Ok(Value::Null)
        } else {
            Err(CommandError::Failed(app.status.clone()))
        }
    } else {
        app.close_window_confirm = false;
        app.exit_requested = true;
        Ok(Value::Null)
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
    app.perform_scm_action(crate::app::scm_ui::ScmAction::Commit);
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
    app.checkout_selected_branch();
    Ok(())
}
fn diff_layout(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(diff) = &mut app.diff_view {
        diff.inline = !diff.inline;
    }
    Ok(())
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
            "Terminal index out of range".into(),
        ));
    }
    app.select_terminal(index);
    Ok(())
}
fn resolve_conflict(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let index = invocation.request.arguments["index"].as_u64().unwrap() as usize;
    if index > 3 || app.conflict_view.is_none() {
        return Err(CommandError::InvalidArguments(
            "Conflict choice out of range".into(),
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
