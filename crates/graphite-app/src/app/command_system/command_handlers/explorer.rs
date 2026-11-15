use super::*;

pub(super) fn register(registry: &mut CommandRegistry) {
    register_history(registry);
    add(
        registry,
        "explorer.rename",
        "Rename",
        "Explorer",
        VIEW,
        "explorerHasSelection",
        None,
        explorer_rename,
    );
    add(
        registry,
        "explorer.delete",
        "Delete",
        "Explorer",
        VIEW,
        "explorerHasSelection",
        None,
        explorer_delete,
    );
    add(
        registry,
        "filesExplorer.cut",
        "Cut",
        "Explorer",
        VIEW,
        "explorerHasSelection",
        None,
        files_explorer_cut,
    );
    add(
        registry,
        "filesExplorer.copy",
        "Copy",
        "Explorer",
        VIEW,
        "explorerHasSelection",
        None,
        files_explorer_copy,
    );
    add(
        registry,
        "filesExplorer.paste",
        "Paste",
        "Explorer",
        VIEW,
        "explorerFocused",
        None,
        files_explorer_paste,
    );
    add(
        registry,
        "explorer.openInNewWindow",
        "Open in New Window",
        "Explorer",
        VIEW,
        "true",
        None,
        explorer_open_in_new_window,
    );
}

pub(super) fn register_history(registry: &mut CommandRegistry) {
    add(
        registry,
        "file.localHistory",
        "Open local history",
        "File",
        VIEW,
        "documentHasPath",
        None,
        file_local_history,
    );
}

fn file_local_history(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.open_local_history();
    Ok(())
}

fn explorer_rename(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_rename();
    Ok(())
}

fn explorer_delete(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_delete();
    Ok(())
}

fn files_explorer_cut(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_cut();
    Ok(())
}

fn files_explorer_copy(app: &mut App, _invocation: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_copy_entry();
    Ok(())
}

fn files_explorer_paste(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.explorer_paste().map_err(CommandError::Failed)
}

fn explorer_open_in_new_window(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.explorer_open_in_new_window()
        .map_err(CommandError::Failed)
}
