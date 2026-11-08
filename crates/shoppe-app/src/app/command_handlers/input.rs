use super::*;
pub(super) fn insert_text(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    invocation.cancellation.check()?;
    app.editor_mut()
        .insert(invocation.request.arguments["text"].as_str().unwrap());
    Ok(())
}
pub(super) fn set_value(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let mut editor = app.editor_mut();
    let len = editor.len_chars();
    editor.set_selection(0, len);
    editor.insert(invocation.request.arguments["text"].as_str().unwrap());
    Ok(())
}
pub(super) fn set_selection(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().set_selection(
        invocation.request.arguments["anchor"].as_u64().unwrap() as usize,
        invocation.request.arguments["head"].as_u64().unwrap() as usize,
    );
    Ok(())
}
pub(super) fn set_cursor(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.editor_mut().set_cursor_line_col(
        invocation.request.arguments["line"].as_u64().unwrap() as usize,
        invocation.request.arguments["column"].as_u64().unwrap() as usize,
    );
    Ok(())
}
