//! Commands for snippets and the output panel.

use super::*;

fn open_snippets(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_snippet_configuration();
    Ok(())
}

/// Inserts the text given as the argument at every cursor, as a snippet body.
fn insert_snippet(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let body = invocation.request.arguments["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if body.is_empty() {
        return Err(CommandError::Failed(app.t("snippets.empty").into()));
    }
    app.editor_mut().insert(&body);
    Ok(())
}

/// Shows the output of tasks in the panel, or hides the panel when it already shows it.
fn toggle_output(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if app.terminal_visible && !app.terminal_problems {
        app.terminal_visible = false;
    } else {
        app.terminal_visible = true;
        app.terminal_problems = false;
    }
    app.resize_terminal();
    Ok(())
}

fn show_output(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_visible = true;
    app.terminal_problems = false;
    app.resize_terminal();
    Ok(())
}

fn show_logs(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_log_picker();
    Ok(())
}

fn clear_output(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.task_output.clear();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, &'static str, Handler); 6] = [
        ("workbench.action.showLogs", "Show logs", "View", show_logs),
        (
            "workbench.action.openLogFile",
            "Open log file",
            "View",
            show_logs,
        ),
        (
            "workbench.action.openSnippets",
            "Configure snippets",
            "Preferences",
            open_snippets,
        ),
        (
            "workbench.action.output.toggleOutput",
            "Toggle output",
            "View",
            toggle_output,
        ),
        (
            "workbench.action.showOutputChannels",
            "Show output",
            "View",
            show_output,
        ),
        (
            "workbench.output.action.clearOutput",
            "Clear output",
            "View",
            clear_output,
        ),
    ];
    for (id, title, category, handler) in commands {
        add(registry, id, title, category, ALL, "true", None, handler);
    }
    registry
        .register(CommandDescriptor {
            id: "editor.action.insertSnippet",
            title: "Insert snippet",
            category: "Editor",
            targets: VIEW,
            enablement: "editorWritable",
            widget: None,
            menu_when: None,
            arguments: ArgumentKind::Text,
            handler: insert_snippet,
        })
        .expect("unique built-in command");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use serde_json::json;

    fn app() -> (App, TempDir) {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");
        std::fs::create_dir_all(&app.profile_root).unwrap();
        (app, dir)
    }

    fn run_with_text(
        app: &mut App,
        id: &str,
        text: &str,
    ) -> crate::app::command_execution::CommandResult {
        app.execute_command(graphite_core_api::CommandRequest {
            id: id.into(),
            arguments: json!({"text": text}),
            target: app.ui_target_for_command(id),
        })
        .try_result()
        .expect("finishes at once")
    }

    #[test]
    fn insert_snippet_puts_the_text_at_the_cursor_and_refuses_empty_text() {
        let (mut app, _dir) = app();
        app.editor_mut().insert("ab");
        app.editor_mut().set_selection(1, 1);
        run_with_text(&mut app, "editor.action.insertSnippet", "SNIP").unwrap();
        assert_eq!(app.editor().text(), "aSNIPb");
        assert!(run_with_text(&mut app, "editor.action.insertSnippet", "").is_err());
        assert_eq!(app.editor().text(), "aSNIPb");
    }

    #[test]
    fn the_log_commands_list_the_profile_log_files() {
        let (mut app, _dir) = app();
        app.run_ui_command("workbench.action.showLogs");
        assert_eq!(app.status, app.t("logs.none"));
        assert!(app.go_picker.is_none());
        std::fs::create_dir_all(app.profile_root.join("code-oss")).unwrap();
        std::fs::write(app.profile_root.join("code-oss/unsupported-api.log"), "x").unwrap();
        std::fs::write(app.profile_root.join("requested-commands.log"), "y").unwrap();
        std::fs::write(app.profile_root.join("notes.txt"), "z").unwrap();
        app.run_ui_command("workbench.action.openLogFile");
        let mut names: Vec<_> = app
            .go_picker
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.name.clone())
            .collect();
        names.sort();
        assert_eq!(names, ["requested-commands.log", "unsupported-api.log"]);
    }

    #[test]
    fn configure_snippets_opens_the_snippets_file() {
        let (mut app, _dir) = app();
        app.run_ui_command("workbench.action.openSnippets");
        let path = app.documents[app.active_document()]
            .buffer
            .path()
            .unwrap()
            .to_path_buf();
        assert!(path.ends_with("snippets.json"), "{path:?}");
    }

    #[test]
    fn the_output_commands_show_and_toggle_the_task_output() {
        let (mut app, _dir) = app();
        app.terminal_visible = false;
        app.run_ui_command("workbench.action.showOutputChannels");
        assert!(app.terminal_visible && !app.terminal_problems);
        app.run_ui_command("workbench.action.output.toggleOutput");
        assert!(!app.terminal_visible);
        app.terminal_problems = true;
        app.terminal_visible = true;
        app.run_ui_command("workbench.action.output.toggleOutput");
        assert!(
            app.terminal_visible && !app.terminal_problems,
            "problems switch to output"
        );
    }
}
