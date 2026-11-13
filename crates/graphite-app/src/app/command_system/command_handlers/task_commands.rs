//! Task commands: running, rerunning, restarting and configuring tasks.

use super::*;

/// Runs the default task of a group, or explains that there is none.
fn run_group(app: &mut App, group: &str, missing: &str) -> Result<(), CommandError> {
    let file = app.task_file().map_err(CommandError::Failed)?;
    let label = file
        .default_group(group)
        .map(|task| task.label.clone())
        .ok_or_else(|| CommandError::Disabled(app.t(missing).into()))?;
    app.run_task(&label).map_err(CommandError::Failed)
}

fn rerun_last_task(app: &mut App) -> Result<(), CommandError> {
    let label = app
        .task_last_label
        .clone()
        .ok_or_else(|| CommandError::Disabled(app.t("tasks.no_last_task").into()))?;
    app.run_task(&label).map_err(CommandError::Failed)
}

fn task_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_pick_task();
    Ok(())
}

fn task_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_show_running_tasks()
        .map_err(CommandError::Disabled)
}

fn task_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.terminal_restart_running_task()
        .map_err(CommandError::Disabled)
}

fn task_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.cancel_task();
    Ok(())
}

fn task_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    rerun_last_task(app)
}

fn task_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run_group(app, "test", "tasks.no_test_task")
}

fn task_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run_group(app, "clean", "tasks.no_clean_task")
}

fn task_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run_group(app, "rebuild", "tasks.no_rebuild_task")
}

fn task_8(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_task_configuration();
    Ok(())
}

fn task_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_task_configuration();
    Ok(())
}

fn task_10(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_task_configuration();
    Ok(())
}

fn task_11(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_task_configuration();
    Ok(())
}

fn task_12(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_task_log();
    Ok(())
}

fn task_13(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_task_problems();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 14] = [
        ("workbench.action.tasks.runTask", "Run task...", task_0),
        (
            "workbench.action.tasks.showTasks",
            "Show running tasks",
            task_1,
        ),
        (
            "workbench.action.tasks.restartTask",
            "Restart running task",
            task_2,
        ),
        ("workbench.action.tasks.terminate", "Terminate task", task_3),
        (
            "workbench.action.tasks.reRunTask",
            "Rerun last task",
            task_4,
        ),
        ("workbench.action.tasks.test", "Run test task", task_5),
        ("workbench.action.tasks.clean", "Run clean task", task_6),
        ("workbench.action.tasks.rebuild", "Run rebuild task", task_7),
        (
            "workbench.action.tasks.configureTaskRunner",
            "Configure task runner",
            task_8,
        ),
        (
            "workbench.action.tasks.openWorkspaceFileTasks",
            "Open workspace tasks",
            task_9,
        ),
        (
            "workbench.action.tasks.configureDefaultBuildTask",
            "Configure default build task",
            task_10,
        ),
        (
            "workbench.action.tasks.configureDefaultTestTask",
            "Configure default test task",
            task_11,
        ),
        ("workbench.action.tasks.showLog", "Show task log", task_12),
        (
            "workbench.action.tasks.toggleProblems",
            "Toggle problems in the panel",
            task_13,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Tasks", ALL, "true", None, handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::phase5::{TaskFile, WorkspaceTrust};
    use crate::test_support::TempDir;

    const TASKS: &str = r#"{"tasks":[
        {"label":"build one","type":"process","command":"echo","group":"build"},
        {"label":"unit","type":"process","command":"echo","group":{"kind":"test","isDefault":true}},
        {"label":"other test","type":"process","command":"echo","group":{"kind":"test"}},
        {"label":"wipe","type":"process","command":"echo","group":"clean"}
    ]}"#;

    fn app_with_tasks(source: Option<&str>, trusted: bool) -> (App, TempDir) {
        let dir = TempDir::new();
        if let Some(source) = source {
            std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
            std::fs::write(dir.path().join(".vscode/tasks.json"), source).unwrap();
        }
        let mut app = App::new_with_options(dir.path().into(), None, true);
        if trusted {
            let mut trust = WorkspaceTrust::open(dir.path().join("trust.json")).unwrap();
            trust.trust(app.tree.root()).unwrap();
            app.workspace_trust = Some(trust);
        }
        (app, dir)
    }

    #[test]
    fn a_group_picks_its_default_task_or_a_task_named_by_the_group() {
        let file = TaskFile::parse(TASKS).unwrap();
        assert_eq!(file.default_build().unwrap().label, "build one");
        assert_eq!(
            file.default_test().unwrap().label,
            "unit",
            "the default one, not the first"
        );
        assert_eq!(file.default_group("clean").unwrap().label, "wipe");
        assert!(file.default_group("rebuild").is_none());
    }

    #[test]
    fn a_missing_group_task_is_reported_as_unavailable() {
        let (mut app, _dir) = app_with_tasks(Some(TASKS), true);
        let Err(CommandError::Disabled(message)) =
            run_group(&mut app, "rebuild", "tasks.no_rebuild_task")
        else {
            panic!("expected a disabled command");
        };
        assert_eq!(message, app.t("tasks.no_rebuild_task"));
    }

    #[test]
    fn the_group_task_is_found_and_handed_to_the_runner() {
        // There is no event loop in tests, so the runner stops right after finding the task.
        let (mut app, _dir) = app_with_tasks(Some(TASKS), true);
        let Err(CommandError::Failed(message)) = run_group(&mut app, "test", "tasks.no_test_task")
        else {
            panic!("expected the runner to fail");
        };
        assert!(message.contains("event loop"), "{message}");
    }

    #[test]
    fn rerunning_needs_a_previous_task() {
        let (mut app, _dir) = app_with_tasks(Some(TASKS), true);
        assert!(matches!(
            rerun_last_task(&mut app),
            Err(CommandError::Disabled(_))
        ));
        app.task_last_label = Some("unit".into());
        let Err(CommandError::Failed(message)) = rerun_last_task(&mut app) else {
            panic!("expected the runner to fail");
        };
        assert!(
            message.contains("event loop"),
            "it looked the task up: {message}"
        );
        app.task_last_label = Some("gone".into());
        let Err(CommandError::Failed(message)) = rerun_last_task(&mut app) else {
            panic!("expected a failure");
        };
        assert!(message.contains("not found"), "{message}");
    }

    #[test]
    fn without_a_tasks_file_group_commands_fail_with_the_reason() {
        let (mut app, _dir) = app_with_tasks(None, true);
        assert!(matches!(
            run_group(&mut app, "test", "tasks.no_test_task"),
            Err(CommandError::Failed(_))
        ));
    }

    #[test]
    fn the_panel_commands_show_the_log_and_flip_problems() {
        let (mut app, _dir) = app_with_tasks(None, false);
        app.terminal_visible = false;
        app.run_ui_command("workbench.action.tasks.showLog");
        assert!(app.terminal_visible && !app.terminal_problems);
        app.run_ui_command("workbench.action.tasks.toggleProblems");
        assert!(app.terminal_problems);
        app.run_ui_command("workbench.action.tasks.toggleProblems");
        assert!(!app.terminal_problems);
    }

    #[test]
    fn the_configure_commands_open_the_tasks_file() {
        for id in [
            "workbench.action.tasks.configureTaskRunner",
            "workbench.action.tasks.openWorkspaceFileTasks",
            "workbench.action.tasks.configureDefaultTestTask",
        ] {
            let (mut app, dir) = app_with_tasks(None, false);
            app.run_ui_command(id);
            let opened = app.documents[app.active_document()]
                .buffer
                .path()
                .unwrap()
                .to_path_buf();
            // The open path may be the canonical form, so compare the last two components.
            assert!(
                opened.ends_with(Path::new(".vscode").join("tasks.json")),
                "{id}: {opened:?}"
            );
            assert!(dir.path().join(".vscode/tasks.json").is_file(), "{id}");
        }
    }

    #[test]
    fn the_task_ids_are_registered() {
        for id in [
            "workbench.action.tasks.runTask",
            "workbench.action.tasks.showTasks",
            "workbench.action.tasks.restartTask",
            "workbench.action.tasks.terminate",
            "workbench.action.tasks.reRunTask",
            "workbench.action.tasks.test",
            "workbench.action.tasks.clean",
            "workbench.action.tasks.rebuild",
        ] {
            assert!(
                crate::app::command_registry::registry().get(id).is_some(),
                "{id}"
            );
        }
    }
}
