//! Security and task configuration primitives for v0.5.
use crate::{app::theme::Theme, config, App, DARK_THEME};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TrustFile {
    trusted_workspaces: Vec<String>,
}

/// Workspace trust is opt-in, canonical-path based, and independent of plugin-version trust.
#[derive(Clone, Debug)]
pub struct WorkspaceTrust {
    path: PathBuf,
    trusted: Vec<String>,
}
impl WorkspaceTrust {
    pub fn restricted(path: PathBuf) -> Self {
        Self {
            path,
            trusted: Vec::new(),
        }
    }
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let trusted = match fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice::<TrustFile>(&bytes)
                    .map_err(|e| e.to_string())?
                    .trusted_workspaces
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self { path, trusted })
    }
    pub fn is_trusted(&self, workspace: &Path) -> bool {
        canonical_workspace(workspace)
            .ok()
            .is_some_and(|key| self.trusted.contains(&key))
    }
    pub fn trust(&mut self, workspace: &Path) -> Result<(), String> {
        let key = canonical_workspace(workspace)?;
        if !self.trusted.contains(&key) {
            let mut trusted = self.trusted.clone();
            trusted.push(key);
            self.persist(trusted)?;
        }
        Ok(())
    }
    pub fn revoke(&mut self, workspace: &Path) -> Result<(), String> {
        let key = canonical_workspace(workspace)?;
        let mut trusted = self.trusted.clone();
        trusted.retain(|item| item != &key);
        self.persist(trusted)
    }
    fn persist(&mut self, trusted: Vec<String>) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let value = TrustFile {
            trusted_workspaces: trusted,
        };
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        replace_profile_pointer(&tmp, &self.path).map_err(|e| e.to_string())?;
        self.trusted = value.trusted_workspaces;
        Ok(())
    }
}
fn canonical_workspace(workspace: &Path) -> Result<String, String> {
    let canonical = workspace
        .canonicalize()
        .map_err(|e| format!("Cannot resolve workspace path: {e}"))?;
    let key = canonical.to_str().ok_or("Workspace path is not UTF-8")?;
    #[cfg(windows)]
    let key = key.to_lowercase();
    Ok(key.to_owned())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileMetadata {
    pub name: String,
    #[serde(default)]
    pub created_from_legacy: bool,
}
#[derive(Clone, Debug)]
pub struct Profiles {
    root: PathBuf,
    active: String,
}
impl Profiles {
    pub fn open(config_root: &Path) -> Result<Self, String> {
        let root = config_root.join("profiles");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let active_path = root.join("active-profile.json");
        let configured = match fs::read(&active_path) {
            Ok(bytes) => serde_json::from_slice::<String>(&bytes).map_err(|e| e.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "Default".into(),
            Err(error) => return Err(error.to_string()),
        };
        validate_profile_name(&configured)?;
        let metadata = root.join("Default").join("profile.json");
        if !metadata.exists() {
            fs::create_dir_all(metadata.parent().unwrap()).map_err(|e| e.to_string())?;
            // Copy legacy files; old state is preserved and remains recoverable.
            for name in [
                "settings.json",
                "keybindings.json",
                "theme.json",
                "terminal.json",
                "snippets",
                "imported",
            ] {
                let source = config_root.join(name);
                if source.exists() {
                    copy_tree_no_links(&source, &metadata.parent().unwrap().join(name))?;
                }
            }
            fs::write(
                &metadata,
                serde_json::to_vec_pretty(&ProfileMetadata {
                    name: "Default".into(),
                    created_from_legacy: true,
                })
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        let active = if root.join(&configured).join("profile.json").is_file() {
            configured
        } else {
            "Default".into()
        };
        Ok(Self { root, active })
    }
    pub fn active(&self) -> &str {
        &self.active
    }
    pub fn path(&self, profile: &str) -> Result<PathBuf, String> {
        validate_profile_name(profile)?;
        let path = self.root.join(profile);
        if !path.join("profile.json").is_file() {
            return Err("Profile does not exist".into());
        }
        Ok(path)
    }
    pub fn list(&self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_dir()
                && entry.path().join("profile.json").is_file()
            {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        out.sort();
        Ok(out)
    }
    pub fn create(&self, name: &str) -> Result<(), String> {
        validate_profile_name(name)?;
        let dir = self.root.join(name);
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        fs::write(
            dir.join("profile.json"),
            serde_json::to_vec_pretty(&ProfileMetadata {
                name: name.into(),
                created_from_legacy: false,
            })
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn switch(&mut self, name: &str) -> Result<bool, String> {
        self.path(name)?;
        if self.active == name {
            return Ok(false);
        }
        let active_path = self.root.join("active-profile.json");
        let temp_path = self.root.join(".active-profile.json.tmp");
        fs::write(
            &temp_path,
            serde_json::to_vec(name).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Err(error) = replace_profile_pointer(&temp_path, &active_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error.to_string());
        }
        self.active = name.into();
        Ok(true)
    }
}

impl App {
    pub(crate) fn create_user_profile(&mut self, name: &str) -> Result<(), String> {
        self.profiles
            .as_ref()
            .ok_or_else(|| "Profiles are unavailable".to_owned())?
            .create(name)?;
        self.status = format!("Created profile: {name}");
        Ok(())
    }

    pub(crate) fn user_profiles(&self) -> Result<Vec<String>, String> {
        self.profiles
            .as_ref()
            .ok_or_else(|| "Profiles are unavailable".to_owned())?
            .list()
    }

    pub(crate) fn switch_user_profile(&mut self, name: &str) -> Result<(), String> {
        let Some(profiles) = self.profiles.as_mut() else {
            return Err("Profiles are unavailable".into());
        };
        if !profiles.switch(name)? {
            self.status = format!("Profile already active: {name}");
            return Ok(());
        }
        let profile_root = profiles.path(name)?;
        let translator = self.translator;
        let workspace = self.tree.root().to_path_buf();
        self.profile_root = profile_root;
        self.sidebar_width = config::load_sidebar_width(&self.profile_root);
        self.refresh_plugins_for_profile();
        if let Some(host) = self.extension_host.supervisor.as_mut() {
            host.shutdown();
        }
        self.clear_host_session();
        if let Ok(prefs) =
            config::editor_prefs_for_workspace_in(&self.profile_root, &workspace, &translator)
        {
            self.font_size = prefs.font_size;
            self.tab_size = prefs.tab_size;
            self.insert_spaces = prefs.insert_spaces;
            self.detect_indentation = prefs.detect_indentation;
            self.trim_trailing_whitespace = prefs.trim_trailing_whitespace;
            self.rulers = prefs.rulers;
            self.word_wrap = prefs.word_wrap;
            self.wrapping_indent = prefs.wrapping_indent;
            self.read_only_patterns = prefs.read_only_patterns;
        }
        if let Ok(colors) =
            config::imported_theme_colors_in(&self.profile_root, &workspace, &translator)
        {
            Theme::set_imported_colors(colors);
        }
        DARK_THEME.store(
            config::load_dark_theme_from(&self.profile_root),
            Ordering::Relaxed,
        );
        (self.snippets, self.snippet_load_warnings) =
            config::load_snippets_for_workspace_in(&self.profile_root, &workspace, &translator);
        if let Ok(settings) = config::terminal_settings_in(&self.profile_root, &translator) {
            self.terminal_profiles = settings.profiles;
            self.selected_terminal_profile = settings.default_profile;
        }
        let keybindings = self.profile_root.join("keybindings.json");
        if let Ok(source) = fs::read_to_string(keybindings) {
            if let Err(error) = self.keymap.reload(&source, &translator) {
                self.status = format!("Profile keybindings could not be loaded: {error}");
            }
        } else {
            self.keymap = crate::keybindings::Keymap::default();
        }
        self.status = format!("Switched to profile: {name}");
        Ok(())
    }
}
fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || name == "."
        || name == ".."
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-' || b == b'_')
    {
        return Err("Invalid profile name".into());
    }
    Ok(())
}
#[cfg(not(windows))]
pub(crate) fn replace_profile_pointer(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    fs::rename(from, to)
}
#[cfg(windows)]
pub(crate) fn replace_profile_pointer(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), 1 | 8) } != 0 {
        return Ok(());
    }
    let native_error = std::io::Error::last_os_error();
    // Some Windows filesystems and restricted test environments reject the
    // native replacement call. A same-volume copy is the portable fallback.
    fs::copy(from, to).map_err(|_| native_error)?;
    fs::remove_file(from)
}
fn copy_tree_no_links(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_file() {
        fs::copy(source, destination).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_tree_no_links(&entry.path(), &destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskFile {
    #[serde(default)]
    pub tasks: Vec<Task>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub options: TaskOptions,
    #[serde(default)]
    pub group: Option<TaskGroup>,
    #[serde(default)]
    pub problem_matcher: Vec<ProblemMatcher>,
    #[serde(rename = "type", default = "default_task_type")]
    pub task_type: String,
}
fn default_task_type() -> String {
    "shell".into()
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskOptions {
    pub cwd: Option<PathBuf>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum TaskGroup {
    Name(String),
    Detailed {
        kind: String,
        #[serde(default, rename = "isDefault")]
        is_default: bool,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ProblemMatcher {
    Name(String),
    Pattern(ProblemPattern),
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProblemPattern {
    pub regexp: String,
    pub file: Option<String>,
    pub line: Option<String>,
    pub column: Option<String>,
    pub message: Option<String>,
}
impl TaskFile {
    pub fn parse(source: &str) -> Result<Self, String> {
        let file: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        for task in &file.tasks {
            if task.label.trim().is_empty()
                || task.command.trim().is_empty()
                || !["shell", "process"].contains(&task.task_type.as_str())
            {
                return Err(
                    "Tasks require a label, command, and supported shell/process type".into(),
                );
            }
            for matcher in &task.problem_matcher {
                if let ProblemMatcher::Pattern(pattern) = matcher {
                    regex::Regex::new(&pattern.regexp)
                        .map_err(|e| format!("Invalid problem matcher: {e}"))?;
                }
            }
        }
        Ok(file)
    }
    pub fn default_build(&self) -> Option<&Task> {
        self.tasks.iter().find(|task| match &task.group {
            Some(TaskGroup::Name(name)) => name == "build",
            Some(TaskGroup::Detailed { kind, is_default }) => kind == "build" && *is_default,
            _ => false,
        })
    }
    pub fn get(&self, label: &str) -> Option<&Task> {
        self.tasks.iter().find(|task| task.label == label)
    }
    pub fn run(
        &self,
        label: &str,
        workspace: &Path,
        trusted: bool,
        cancelled: &AtomicBool,
        mut output: impl FnMut(&str),
    ) -> Result<i32, String> {
        self.run_with_process(label, workspace, trusted, cancelled, |_| {}, output)
    }

    pub fn run_with_process(
        &self,
        label: &str,
        workspace: &Path,
        trusted: bool,
        cancelled: &AtomicBool,
        on_process: impl FnOnce(u32),
        mut output: impl FnMut(&str),
    ) -> Result<i32, String> {
        if !trusted {
            return Err("Workspace is restricted; trust is required to run tasks".into());
        }
        let task = self
            .tasks
            .iter()
            .find(|task| task.label == label)
            .ok_or_else(|| "Task not found".to_owned())?;
        let cwd = task
            .options
            .cwd
            .as_ref()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    workspace.join(path)
                }
            })
            .unwrap_or_else(|| workspace.to_path_buf());
        let mut command = if task.task_type == "shell" {
            #[cfg(windows)]
            {
                let mut command = Command::new("cmd.exe");
                command
                    .args(["/D", "/S", "/C"])
                    .arg(shell_join(&task.command, &task.args));
                command
            }
            #[cfg(not(windows))]
            {
                let mut command = Command::new("sh");
                command.args(["-c", &shell_join(&task.command, &task.args)]);
                command
            }
        } else {
            let mut command = Command::new(&task.command);
            command.args(&task.args);
            command
        };
        let mut child = command
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        on_process(child.id());
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        spawn_task_reader(stdout, tx.clone());
        spawn_task_reader(stderr, tx.clone());
        drop(tx);
        loop {
            while let Ok(line) = rx.try_recv() {
                output(&line);
            }
            if cancelled.load(Ordering::Acquire) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Task cancelled".into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                while let Ok(line) = rx.try_recv() {
                    output(&line);
                }
                return Ok(status.code().unwrap_or(1));
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

fn shell_join(command: &str, args: &[String]) -> String {
    #[cfg(windows)]
    fn quote(arg: &str) -> String {
        format!("\"{}\"", arg.replace('"', "\\\""))
    }
    #[cfg(not(windows))]
    fn quote(arg: &str) -> String {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn spawn_task_reader<R: std::io::Read + Send + 'static>(
    stream: R,
    tx: std::sync::mpsc::Sender<String>,
) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stream)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDiagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
}
impl ProblemPattern {
    pub fn parse_line(&self, text: &str) -> Option<TaskDiagnostic> {
        let regex = regex::Regex::new(&self.regexp).ok()?;
        let captures = regex.captures(text)?;
        let get = |key: &Option<String>| {
            key.as_ref()
                .and_then(|name| captures.name(name))
                .map(|m| m.as_str())
        };
        Some(TaskDiagnostic {
            file: get(&self.file)?.into(),
            line: get(&self.line)?.parse().ok()?,
            column: get(&self.column).and_then(|s| s.parse().ok()).unwrap_or(1),
            message: get(&self.message)?.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn workspace_trust_distinguishes_paths_that_differ_in_case() {
        let temp = crate::test_support::TempDir::new();
        let trusted = temp.path().join("Workspace");
        let restricted = temp.path().join("workspace");
        fs::create_dir(&trusted).unwrap();
        fs::create_dir(&restricted).unwrap();
        let mut trust = WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
        trust.trust(&trusted).unwrap();
        assert!(trust.is_trusted(&trusted));
        assert!(!trust.is_trusted(&restricted));
    }
    use super::*;
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "graphite-phase5-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn workspace_trust_defaults_restricted_and_persists_per_canonical_path() {
        let dir = temp();
        let ws = dir.join("workspace");
        fs::create_dir(&ws).unwrap();
        let file = dir.join("trust.json");
        let mut trust = WorkspaceTrust::open(file.clone()).unwrap();
        assert!(!trust.is_trusted(&ws));
        trust.trust(&ws).unwrap();
        assert!(WorkspaceTrust::open(file.clone()).unwrap().is_trusted(&ws));
        trust.revoke(&ws).unwrap();
        assert!(!WorkspaceTrust::open(file).unwrap().is_trusted(&ws));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn profile_migration_copies_legacy_state_and_switches_isolated_directories() {
        let dir = temp();
        fs::write(dir.join("settings.json"), "legacy").unwrap();
        let mut profiles = Profiles::open(&dir).unwrap();
        let default = profiles.path("Default").unwrap();
        assert_eq!(
            fs::read_to_string(default.join("settings.json")).unwrap(),
            "legacy"
        );
        assert_eq!(
            fs::read_to_string(dir.join("settings.json")).unwrap(),
            "legacy"
        );
        profiles.create("Work").unwrap();
        assert!(profiles.switch("Work").unwrap());
        assert_eq!(profiles.active(), "Work");
        assert!(profiles.path("../escape").is_err());
        assert_eq!(Profiles::open(&dir).unwrap().active(), "Work");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn tasks_parse_default_build_match_diagnostics_and_require_workspace_trust() {
        let dir = temp();
        let tasks=TaskFile::parse(r#"{"tasks":[{"label":"compile","type":"process","command":"rustc","args":["main.rs"],"group":{"kind":"build","isDefault":true},"problemMatcher":[{"regexp":"^(?P<file>[^:]+):(?P<line>[0-9]+):(?P<column>[0-9]+): (?P<message>.+)$","file":"file","line":"line","column":"column","message":"message"}]}]}"#).unwrap();
        assert_eq!(tasks.default_build().unwrap().label, "compile");
        let ProblemMatcher::Pattern(pattern) = &tasks.tasks[0].problem_matcher[0] else {
            panic!()
        };
        assert_eq!(
            pattern.parse_line("src/main.rs:4:2: unexpected token"),
            Some(TaskDiagnostic {
                file: "src/main.rs".into(),
                line: 4,
                column: 2,
                message: "unexpected token".into()
            })
        );
        assert!(tasks
            .run("compile", &dir, false, &AtomicBool::new(false), |_| {})
            .unwrap_err()
            .contains("trust"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn trusted_process_task_streams_output_and_returns_exit_code() {
        let dir = temp();
        #[cfg(windows)]
        let source = r#"{"tasks":[{"label":"fixture","type":"process","command":"cmd.exe","args":["/C","echo task-output"]}]}"#;
        #[cfg(not(windows))]
        let source = r#"{"tasks":[{"label":"fixture","type":"process","command":"sh","args":["-c","echo task-output"]}]}"#;
        let tasks = TaskFile::parse(source).unwrap();
        let mut output = Vec::new();
        let code = tasks
            .run("fixture", &dir, true, &AtomicBool::new(false), |line| {
                output.push(line.to_owned())
            })
            .unwrap();
        assert_eq!(code, 0);
        assert!(output.iter().any(|line| line.contains("task-output")));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unsupported_or_malformed_tasks_fail_closed() {
        assert!(
            TaskFile::parse(r#"{"tasks":[{"label":"x","type":"npm","command":"npm"}]}"#).is_err()
        );
        assert!(TaskFile::parse(r#"{"tasks":[{"label":"x","command":"echo","problemMatcher":[{"regexp":"[","file":"f","line":"l","message":"m"}]}]}"#).is_err());
    }

    #[test]
    fn task_runner_observes_cancellation_and_stops_the_child() {
        let dir = temp();
        #[cfg(windows)]
        let source = r#"{"tasks":[{"label":"wait","type":"process","command":"ping.exe","args":["-t","127.0.0.1"]}]}"#;
        #[cfg(not(windows))]
        let source = r#"{"tasks":[{"label":"wait","type":"process","command":"sh","args":["-c","sleep 10"]}]}"#;
        let tasks = TaskFile::parse(source).unwrap();
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let root = dir.clone();
        let worker = std::thread::spawn(move || {
            let result = tasks.run("wait", &root, true, &flag, |_| {});
            tx.send(result).unwrap();
        });
        std::thread::sleep(Duration::from_millis(100));
        cancelled.store(true, Ordering::Release);
        assert!(rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap_err()
            .contains("cancelled"));
        worker.join().unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
