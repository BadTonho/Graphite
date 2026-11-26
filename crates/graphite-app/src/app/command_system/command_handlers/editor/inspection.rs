//! Commands that inspect the editor: TextMate scopes, Code Lens, and accessibility help.

use super::*;

pub(super) fn show_requested_commands(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.show_requested_vsix_commands();
    Ok(())
}

pub(super) fn run_code_lens(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    if app.run_code_lens_on_cursor_line() {
        Ok(())
    } else {
        Err(CommandError::Disabled(
            "There is no code lens on the cursor line".into(),
        ))
    }
}

pub(super) fn inspect_tm_scopes(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let doc_idx = app.command_document();
    let (line, col) = app.editor().cursor_line_col();
    let lang = app.documents[doc_idx]
        .language
        .map(|l| format!("{l:?}"))
        .unwrap_or_else(|| "PlainText".into());
    let full_text = app.documents[doc_idx].buffer.text();
    let line_text = full_text.lines().nth(line).unwrap_or("");
    let token: String = line_text
        .chars()
        .skip(col.saturating_sub(1))
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect();
    app.status = format!(
        "Token inspector: language={lang}, line={}, col={}, token={}",
        line + 1,
        col + 1,
        if token.is_empty() { "<none>" } else { &token }
    );
    Ok(())
}

pub(super) fn toggle_screen_reader_mode(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.screen_reader_mode = !app.screen_reader_mode;
    let state = app
        .t(if app.screen_reader_mode {
            "common.on"
        } else {
            "common.off"
        })
        .to_owned();
    app.status = format!("Screen reader accessibility mode: {state}");
    Ok(())
}

pub(super) fn show_accessibility_help(
    app: &mut App,
    _invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    app.status = "Accessibility: F6 cycles regions, Ctrl+Shift+P opens commands, Alt+F1 toggles screen reader mode".into();
    Ok(())
}

#[cfg(test)]
mod inspection_tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn inspect_tokens_and_accessibility_commands_work() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.editor_mut().insert("let answer = 42;");
        app.run_ui_command("editor.action.inspectTMScopes");
        assert!(app.status.contains("Token inspector:"));

        assert!(!app.screen_reader_mode);
        app.run_ui_command("editor.action.toggleScreenReaderAccessibilityMode");
        assert!(app.screen_reader_mode);
        app.run_ui_command("workbench.action.showAccessibilityHelp");
        assert!(app.status.contains("F6"));
    }
}
