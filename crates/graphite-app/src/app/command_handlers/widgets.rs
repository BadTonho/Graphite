use super::*;

pub(super) fn suggest_close(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.completion_items.clear();
    Ok(())
}

pub(super) fn suggest_accept(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.accept_completion();
    Ok(())
}

pub(super) fn suggest_previous(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        if !app.completion_items.is_empty() {
            app.completion_index = app
                .completion_index
                .checked_sub(1)
                .unwrap_or(app.completion_items.len() - 1);
        }
    };
    Ok(())
}

pub(super) fn suggest_next(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        if !app.completion_items.is_empty() {
            app.completion_index = (app.completion_index + 1) % app.completion_items.len();
        }
    };
    Ok(())
}

pub(super) fn quick_fix_close(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.quick_fix_visible = false;
    Ok(())
}

pub(super) fn quick_fix_previous(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        if !app.quick_fix_actions.is_empty() {
            app.quick_fix_index = app
                .quick_fix_index
                .checked_sub(1)
                .unwrap_or(app.quick_fix_actions.len() - 1);
        }
    };
    Ok(())
}

pub(super) fn quick_fix_next(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        if !app.quick_fix_actions.is_empty() {
            app.quick_fix_index = (app.quick_fix_index + 1) % app.quick_fix_actions.len();
        }
    };
    Ok(())
}

pub(super) fn quick_fix_apply(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.apply_quick_fix();
    Ok(())
}

pub(super) fn snippet_close(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.snippet_query = None;
    Ok(())
}

pub(super) fn snippet_delete_left(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        if let Some(query) = &mut app.snippet_query {
            query.pop();
        }
    };
    Ok(())
}

pub(super) fn snippet_accept(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    {
        if let Some(query) = app.snippet_query.take() {
            if let Some(snippet) = app
                .snippets
                .iter()
                .find(|s| s.prefix.starts_with(&query) && app.snippet_scope_matches(s))
            {
                let body = snippet.body.clone();
                app.editor_mut().insert(&body);
            }
        }
    };
    Ok(())
}

pub(super) fn quick_open_close(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let command = invocation.request.id.as_str();
    app.execute_quick_open_command(command);
    Ok(())
}

pub(super) fn project_search_close(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let command = invocation.request.id.as_str();
    app.execute_project_search_command(command);
    Ok(())
}

pub(super) fn close_find_widget(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let command = invocation.request.id.as_str();
    app.execute_find_command(command);
    if let Some(search) = &app.local_search {
        if matches!(
            command,
            "editor.action.nextMatchFindAction"
                | "editor.action.previousMatchFindAction"
                | "editor.action.replaceAll"
                | "editor.action.replaceOne"
        ) && search.message.starts_with("Invalid regex:")
        {
            return Err(CommandError::Failed(search.message.clone()));
        }
    }
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    add(
        registry,
        "suggest.close",
        "Close",
        "Widgets",
        WIDGET,
        "suggestWidgetVisible",
        Some(WidgetKind::Suggest),
        suggest_close,
    );
    add(
        registry,
        "suggest.accept",
        "Accept",
        "Widgets",
        WIDGET,
        "suggestWidgetVisible && editorWritable",
        Some(WidgetKind::Suggest),
        suggest_accept,
    );
    add(
        registry,
        "suggest.previous",
        "Previous",
        "Widgets",
        WIDGET,
        "suggestWidgetVisible",
        Some(WidgetKind::Suggest),
        suggest_previous,
    );
    add(
        registry,
        "suggest.next",
        "Next",
        "Widgets",
        WIDGET,
        "suggestWidgetVisible",
        Some(WidgetKind::Suggest),
        suggest_next,
    );
    add(
        registry,
        "quickFix.close",
        "Close",
        "Widgets",
        WIDGET,
        "quickFixVisible",
        Some(WidgetKind::QuickFix),
        quick_fix_close,
    );
    add(
        registry,
        "quickFix.previous",
        "Previous",
        "Widgets",
        WIDGET,
        "quickFixVisible && quickFixHasItems",
        Some(WidgetKind::QuickFix),
        quick_fix_previous,
    );
    add(
        registry,
        "quickFix.next",
        "Next",
        "Widgets",
        WIDGET,
        "quickFixVisible && quickFixHasItems",
        Some(WidgetKind::QuickFix),
        quick_fix_next,
    );
    add(
        registry,
        "quickFix.apply",
        "Apply",
        "Widgets",
        WIDGET,
        "quickFixVisible && quickFixHasItems",
        Some(WidgetKind::QuickFix),
        quick_fix_apply,
    );
    add(
        registry,
        "snippet.close",
        "Close",
        "Widgets",
        WIDGET,
        "snippetVisible",
        Some(WidgetKind::Snippet),
        snippet_close,
    );
    add(
        registry,
        "snippet.deleteLeft",
        "Delete left",
        "Widgets",
        WIDGET,
        "snippetVisible",
        Some(WidgetKind::Snippet),
        snippet_delete_left,
    );
    add(
        registry,
        "snippet.accept",
        "Accept",
        "Widgets",
        WIDGET,
        "snippetVisible && editorWritable",
        Some(WidgetKind::Snippet),
        snippet_accept,
    );
    add(
        registry,
        "quickOpen.close",
        "Close",
        "Widgets",
        WIDGET,
        "quickOpenVisible",
        Some(WidgetKind::QuickOpen),
        quick_open_close,
    );
    add(
        registry,
        "quickOpen.accept",
        "Accept",
        "Widgets",
        WIDGET,
        "quickOpenVisible && quickOpenHasItems",
        Some(WidgetKind::QuickOpen),
        quick_open_close,
    );
    add(
        registry,
        "quickOpen.previous",
        "Previous",
        "Widgets",
        WIDGET,
        "quickOpenVisible && quickOpenHasItems",
        Some(WidgetKind::QuickOpen),
        quick_open_close,
    );
    add(
        registry,
        "quickOpen.next",
        "Next",
        "Widgets",
        WIDGET,
        "quickOpenVisible && quickOpenHasItems",
        Some(WidgetKind::QuickOpen),
        quick_open_close,
    );
    add(
        registry,
        "quickOpen.deleteLeft",
        "Delete left",
        "Widgets",
        WIDGET,
        "quickOpenVisible",
        Some(WidgetKind::QuickOpen),
        quick_open_close,
    );
    add(
        registry,
        "projectSearch.close",
        "Close",
        "Widgets",
        WIDGET,
        "projectSearchVisible",
        Some(WidgetKind::ProjectSearch),
        project_search_close,
    );
    add(
        registry,
        "projectSearch.accept",
        "Accept",
        "Widgets",
        WIDGET,
        "projectSearchVisible && searchHasItems",
        Some(WidgetKind::ProjectSearch),
        project_search_close,
    );
    add(
        registry,
        "projectSearch.previous",
        "Previous",
        "Widgets",
        WIDGET,
        "projectSearchVisible && searchHasItems",
        Some(WidgetKind::ProjectSearch),
        project_search_close,
    );
    add(
        registry,
        "projectSearch.next",
        "Next",
        "Widgets",
        WIDGET,
        "projectSearchVisible && searchHasItems",
        Some(WidgetKind::ProjectSearch),
        project_search_close,
    );
    add(
        registry,
        "projectSearch.deleteLeft",
        "Delete left",
        "Widgets",
        WIDGET,
        "projectSearchVisible",
        Some(WidgetKind::ProjectSearch),
        project_search_close,
    );
    add(
        registry,
        "closeFindWidget",
        "Close find widget",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.toggleField",
        "Toggle field",
        "Widgets",
        WIDGET,
        "findWidgetVisible && findReplace",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "editor.action.nextMatchFindAction",
        "Next match find action",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "editor.action.previousMatchFindAction",
        "Previous match find action",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "editor.action.replaceAll",
        "Replace all",
        "Widgets",
        WIDGET,
        "findWidgetVisible && editorWritable && findReplace",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "editor.action.replaceOne",
        "Replace one",
        "Widgets",
        WIDGET,
        "findWidgetVisible && editorWritable && findReplace",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.deleteLeft",
        "Delete left",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.toggleRegex",
        "Toggle regex",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.toggleCase",
        "Toggle case",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.toggleWholeWord",
        "Toggle whole word",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.togglePreserveCase",
        "Toggle preserve case",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
    add(
        registry,
        "find.toggleSelection",
        "Toggle selection",
        "Widgets",
        WIDGET,
        "findWidgetVisible",
        Some(WidgetKind::Find),
        close_find_widget,
    );
}
