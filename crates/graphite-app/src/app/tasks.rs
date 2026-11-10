use super::*;
use crate::app::phase5::{ProblemMatcher, TaskFile};

impl App {
    pub(crate) fn task_file(&self) -> Result<TaskFile, String> {
        let path = self.tree.root().join(".vscode/tasks.json");
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
        TaskFile::parse(&source)
    }

    pub(crate) fn run_task(&mut self, label: &str) -> Result<(), String> {
        if self.task_cancel.is_some() {
            return Err("A task is already running".into());
        }
        let trusted = self
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()));
        if !trusted {
            return Err("Workspace is restricted; trust it before running tasks".into());
        }
        let file = self.task_file()?;
        let task = file
            .get(label)
            .cloned()
            .ok_or_else(|| "Task not found".to_owned())?;
        let Some(proxy) = self.event_proxy.clone() else {
            return Err("Task event loop is unavailable".into());
        };
        let root = self.tree.root().to_path_buf();
        let diagnostic_base = task
            .options
            .cwd
            .as_ref()
            .map(|cwd| {
                if cwd.is_absolute() {
                    cwd.clone()
                } else {
                    root.join(cwd)
                }
            })
            .unwrap_or_else(|| root.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let task_file = TaskFile {
            tasks: vec![task.clone()],
        };
        let matchers = task.problem_matcher.clone();
        self.task_cancel = Some(cancel);
        self.task_output.clear();
        self.task_diagnostics.clear();
        self.terminal_status = None;
        self.terminal_visible = true;
        self.terminal_problems = false;
        self.terminal_focused = false;
        self.status = format!("Running task: {}", task.label);
        std::thread::Builder::new()
            .name("graphite-task".into())
            .spawn(move || {
                let mut diagnostics = Vec::new();
                let process_proxy = proxy.clone();
                let result = task_file.run_with_process(
                    &task.label,
                    &root,
                    trusted,
                    &worker_cancel,
                    move |pid| {
                        let _ = process_proxy.send_event(UserEvent::TaskProcessStarted(pid));
                    },
                    |line| {
                        let _ = proxy.send_event(UserEvent::TaskOutput(line.to_owned()));
                        for matcher in &matchers {
                            if let ProblemMatcher::Pattern(pattern) = matcher {
                                if let Some(diagnostic) = pattern.parse_line(line) {
                                    let mut diagnostic = diagnostic;
                                    let path = PathBuf::from(&diagnostic.file);
                                    if !path.is_absolute() {
                                        diagnostic.file =
                                            diagnostic_base.join(path).display().to_string();
                                    }
                                    diagnostics.push(diagnostic);
                                }
                            }
                        }
                    },
                );
                let _ = proxy.send_event(UserEvent::TaskFinished {
                    result,
                    diagnostics,
                });
            })
            .map_err(|error| {
                self.task_cancel = None;
                format!("Cannot start task worker: {error}")
            })?;
        Ok(())
    }

    pub(crate) fn cancel_task(&mut self) {
        if let Some(cancel) = &self.task_cancel {
            cancel.store(true, Ordering::Release);
            self.status = "Stopping task…".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_palette_entries_are_disabled_until_workspace_trust_is_granted() {
        let dir = crate::test_support::TempDir::new();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(
            dir.path().join(".vscode/tasks.json"),
            r#"{"tasks":[{"label":"Build fixture","type":"process","command":"echo","args":["ok"]}]}"#,
        )
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_palette();
        let task = app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|item| item.id == "graphite.tasks.run:Build fixture")
            .unwrap();
        assert!(!task.enabled);

        let mut trust =
            crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap();
        trust.trust(app.tree.root()).unwrap();
        app.workspace_trust = Some(trust);
        app.open_command_palette();
        assert!(app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|item| item.id == "graphite.tasks.run:Build fixture" && item.enabled));
    }

    #[test]
    fn task_runner_refuses_to_run_without_workspace_trust() {
        let dir = crate::test_support::TempDir::new();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(
            dir.path().join(".vscode/tasks.json"),
            r#"{"tasks":[{"label":"Build fixture","type":"process","command":"echo"}]}"#,
        )
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(app
            .run_task("Build fixture")
            .unwrap_err()
            .contains("restricted"));
    }

    #[test]
    fn task_matcher_diagnostics_join_the_problems_collection_for_open_files() {
        let dir = crate::test_support::TempDir::new();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let file = dir.path().join("src/main.rs");
        std::fs::write(&file, "first\nsecond\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file);
        app.task_diagnostics
            .push(crate::app::phase5::TaskDiagnostic {
                file: "src/main.rs".into(),
                line: 2,
                column: 3,
                message: "fixture error".into(),
            });
        let diagnostics = app.diagnostic_entries();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].1.message, "fixture error");
        assert_eq!(diagnostics[0].1.source.as_deref(), Some("task"));
        assert_eq!(diagnostics[0].1.range.start.line, 1);
        assert_eq!(diagnostics[0].1.range.start.character, 2);
    }
}
