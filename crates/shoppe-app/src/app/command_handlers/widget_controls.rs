//! Semantic widget actions share the command entry point; text input stays local.
use super::*;

pub(super) fn register(registry: &mut CommandRegistry) {
    registry
        .register(CommandDescriptor {
            id: "shoppe.problems.open",
            title: "Open diagnostic",
            category: "Workbench",
            targets: ALL,
            enablement: "true",
            menu_when: None,
            arguments: ArgumentKind::Index,
            widget: None,
            handler: open_diagnostic,
        })
        .unwrap();
    for (id, title, kind, condition, handler) in [
        (
            "shoppe.branch.close",
            "Close branch picker",
            WidgetKind::Branch,
            "branchPickerVisible",
            branch_close as Handler,
        ),
        (
            "shoppe.branch.next",
            "Select next branch",
            WidgetKind::Branch,
            "widgetHasItems",
            branch_next as Handler,
        ),
        (
            "shoppe.branch.previous",
            "Select previous branch",
            WidgetKind::Branch,
            "widgetHasItems",
            branch_previous as Handler,
        ),
        (
            "shoppe.conflict.close",
            "Close conflict",
            WidgetKind::Conflict,
            "conflictVisible",
            conflict_close as Handler,
        ),
        (
            "shoppe.commit.cancel",
            "Cancel commit",
            WidgetKind::Commit,
            "commitPromptVisible",
            commit_cancel as Handler,
        ),
        (
            "shoppe.commit.deleteLeft",
            "Delete commit message character",
            WidgetKind::Commit,
            "commitPromptVisible",
            commit_delete as Handler,
        ),
        (
            "shoppe.scm.promptCommit",
            "Enter commit message",
            WidgetKind::Scm,
            "scmVisible",
            commit_prompt as Handler,
        ),
        (
            "shoppe.scm.next",
            "Select next changed file",
            WidgetKind::Scm,
            "widgetHasItems",
            scm_next as Handler,
        ),
        (
            "shoppe.scm.previous",
            "Select previous changed file",
            WidgetKind::Scm,
            "widgetHasItems",
            scm_previous as Handler,
        ),
        (
            "shoppe.scm.open",
            "Open selected changed file",
            WidgetKind::Scm,
            "widgetHasItems",
            scm_open as Handler,
        ),
        (
            "shoppe.scm.close",
            "Close source control",
            WidgetKind::Scm,
            "scmVisible",
            scm_close as Handler,
        ),
    ] {
        add(
            registry,
            id,
            title,
            "Widgets",
            WIDGET,
            condition,
            Some(kind),
            handler,
        );
    }
    registry
        .register(CommandDescriptor {
            id: "shoppe.scm.select",
            title: "Select changed file",
            category: "Widgets",
            targets: WIDGET,
            enablement: "widgetHasItems",
            menu_when: None,
            arguments: ArgumentKind::Index,
            widget: Some(WidgetKind::Scm),
            handler: scm_select,
        })
        .unwrap();
}

fn open_diagnostic(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let row = invocation.request.arguments["index"].as_u64().unwrap() as usize;
    let (document, diagnostic) = app.diagnostic_entries()[row].clone();
    app.remember_diagnostic(document, &diagnostic);
    app.push_navigation_location();
    app.activate_editor_document(document);
    let line = (diagnostic.range.start.line as usize)
        .min(app.documents[document].buffer.len_lines().saturating_sub(1));
    let col = super::super::drawing_helpers::utf16_column_to_chars(
        &app.documents[document].buffer.line(line),
        diagnostic.range.start.character,
    );
    app.editor_mut().set_cursor_line_col(line, col);
    app.reveal_cursor = true;
    app.push_navigation_location();
    app.request_quick_fixes_for(diagnostic);
    Ok(())
}
fn branch_close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.branch_picker = false;
    Ok(())
}
fn branch_next(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.branch_selected =
        (app.branch_selected + 1).min(app.scm_snapshot.branches.len().saturating_sub(1));
    Ok(())
}
fn branch_previous(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.branch_selected = app.branch_selected.saturating_sub(1);
    Ok(())
}
fn conflict_close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.conflict_view = None;
    Ok(())
}
fn commit_cancel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_commit_prompt = false;
    app.status = "Commit cancelled".into();
    Ok(())
}
fn commit_delete(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_commit_message.pop();
    Ok(())
}
fn commit_prompt(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_commit_prompt = true;
    app.scm_commit_message.clear();
    Ok(())
}
fn scm_next(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_selected = (app.scm_selected + 1).min(app.scm_snapshot.files.len().saturating_sub(1));
    Ok(())
}
fn scm_previous(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_selected = app.scm_selected.saturating_sub(1);
    Ok(())
}
fn scm_open(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(path) = app.scm_selected_path() {
        app.open_file(path);
    }
    Ok(())
}
fn scm_close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_visible = false;
    Ok(())
}
fn scm_select(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.scm_selected = invocation.request.arguments["index"].as_u64().unwrap() as usize;
    Ok(())
}
