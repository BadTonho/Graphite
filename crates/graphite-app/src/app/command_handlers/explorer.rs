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
