//! Runs the assistant's `run_command` calls with bounded output, a time limit, and cancellation.

use crate::tools::CommandRunner;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The most output, in bytes, returned to the model. Each stream is also capped at this size
/// while it is read, so a noisy command cannot fill memory.
pub const MAX_COMMAND_OUTPUT_BYTES: usize = 24_000;
/// How long a command may run before it is stopped.
pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
/// How long to wait, after the command ends, for output still held by processes it started.
const DRAIN_GRACE: Duration = Duration::from_millis(500);
const TRUNCATED: &str = "\n...[truncated]";

/// Runs commands through the system shell: `cmd /C` on Windows, `sh -c` elsewhere.
pub struct SystemCommandRunner {
    cancel: Option<Arc<AtomicBool>>,
    timeout: Duration,
}

impl Default for SystemCommandRunner {
    fn default() -> Self {
        Self {
            cancel: None,
            timeout: DEFAULT_COMMAND_TIMEOUT,
        }
    }
}

impl SystemCommandRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops a running command as soon as `cancel` is set.
    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Started,
    Denied,
    Exited(i32),
    TimedOut,
    Cancelled,
    Failed,
    Unknown,
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, cwd: &Path, command: &str) -> Result<String, String> {
        self.run_with_status(cwd, command).0
    }

    fn run_with_status(
        &self,
        cwd: &Path,
        command: &str,
    ) -> (Result<String, String>, CommandStatus) {
        if self.cancelled() {
            return (
                Err("Command was cancelled".into()),
                CommandStatus::Cancelled,
            );
        }
        let mut child = match shell(command)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return (
                    Err(format!("Failed to launch command: {error}")),
                    CommandStatus::Failed,
                )
            }
        };
        let stdout = Capture::start(child.stdout.take());
        let stderr = Capture::start(child.stderr.take());

        let deadline = Instant::now() + self.timeout;
        let ended = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    break status.code().ok_or_else(|| {
                        (
                            "Command ended without an exit code".into(),
                            CommandStatus::Failed,
                        )
                    })
                }
                Ok(None) => {}
                Err(error) => {
                    kill_tree(&mut child);
                    break Err((
                        format!("Could not wait for the command: {error}"),
                        CommandStatus::Failed,
                    ));
                }
            }
            if self.cancelled() {
                kill_tree(&mut child);
                break Err(("Command was cancelled".into(), CommandStatus::Cancelled));
            }
            if Instant::now() >= deadline {
                kill_tree(&mut child);
                break Err((
                    format!(
                        "Command timed out after {} s and was stopped",
                        self.timeout.as_secs_f32()
                    ),
                    CommandStatus::TimedOut,
                ));
            }
            thread::sleep(POLL_INTERVAL);
        };

        let drain_until = Instant::now() + DRAIN_GRACE;
        let (out, out_cut) = stdout.finish(drain_until);
        let (err, err_cut) = stderr.finish(drain_until);
        let code = match &ended {
            Ok(code) => *code,
            Err(_) => -1,
        };
        let report = format_output(code, &out, &err, out_cut || err_cut);
        match ended {
            Ok(0) => (Ok(report), CommandStatus::Exited(0)),
            Ok(code) => (Err(report), CommandStatus::Exited(code)),
            Err((reason, status)) => (Err(format!("{reason}\n{report}")), status),
        }
    }
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut shell = Command::new("cmd");
    // `cmd` parses its own command line, so the command is passed as written.
    shell.arg("/C").raw_arg(command);
    // No console window of its own: the output is read through the pipes.
    shell.creation_flags(0x0800_0000);
    shell
}

#[cfg(not(windows))]
fn shell(command: &str) -> Command {
    use std::os::unix::process::CommandExt;
    let mut shell = Command::new("sh");
    shell.args(["-c", command]);
    // A process group of its own, so stopping the command also stops what it started.
    shell.process_group(0);
    shell
}

/// Ends the command and the processes it started.
fn kill_tree(child: &mut Child) {
    let pid = child.id().to_string();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Reads one output stream on its own thread, keeping at most [`MAX_COMMAND_OUTPUT_BYTES`].
struct Capture {
    state: Arc<Mutex<Captured>>,
    reader: Option<thread::JoinHandle<()>>,
}

#[derive(Default)]
struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Capture {
    fn start(pipe: Option<impl Read + Send + 'static>) -> Self {
        let state = Arc::new(Mutex::new(Captured::default()));
        let reader = pipe.map(|pipe| {
            let state = state.clone();
            thread::spawn(move || read_capped(pipe, &state))
        });
        Self { state, reader }
    }

    /// The captured text. Waits until `until` for the stream to close; a stream still held
    /// open by a leftover process returns what has arrived so far.
    fn finish(mut self, until: Instant) -> (String, bool) {
        if let Some(reader) = self.reader.take() {
            while !reader.is_finished() && Instant::now() < until {
                thread::sleep(POLL_INTERVAL);
            }
            if reader.is_finished() {
                let _ = reader.join();
            }
        }
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            String::from_utf8_lossy(&state.bytes).into_owned(),
            state.truncated,
        )
    }
}

fn read_capped(mut pipe: impl Read, state: &Mutex<Captured>) {
    let mut buffer = [0u8; 8192];
    loop {
        let read = match pipe.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let room = MAX_COMMAND_OUTPUT_BYTES.saturating_sub(state.bytes.len());
        if read > room {
            state.truncated = true;
        }
        // Reading continues past the limit, so the command never blocks on a full pipe.
        state.bytes.extend_from_slice(&buffer[..read.min(room)]);
    }
}

/// The report returned to the model, at most [`MAX_COMMAND_OUTPUT_BYTES`] plus a marker.
fn format_output(code: i32, stdout: &str, stderr: &str, truncated: bool) -> String {
    let mut report = format!("exit code: {code}\n");
    if !stdout.is_empty() {
        report.push_str("stdout:\n");
        report.push_str(stdout);
    }
    if !stderr.is_empty() {
        report.push_str("stderr:\n");
        report.push_str(stderr);
    }
    let cut = truncate_at_char_boundary(&mut report, MAX_COMMAND_OUTPUT_BYTES);
    if truncated || cut {
        report.push_str(TRUNCATED);
    }
    report
}

/// Shortens `text` to at most `max` bytes without splitting a character. Returns whether it cut.
pub(crate) fn truncate_at_char_boundary(text: &mut String, max: usize) -> bool {
    if text.len() <= max {
        return false;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn long_multibyte_output_is_cut_on_a_character_boundary() {
        // Two-byte characters with the limit falling inside one of them.
        let stdout = "é".repeat(MAX_COMMAND_OUTPUT_BYTES);
        let report = format_output(0, &stdout, "", false);

        assert!(report.ends_with(TRUNCATED));
        assert!(report.len() <= MAX_COMMAND_OUTPUT_BYTES + TRUNCATED.len());
        assert!(!report.contains('\u{FFFD}'));
    }

    #[test]
    fn reading_keeps_at_most_the_limit_and_marks_the_rest() {
        let state = Mutex::new(Captured::default());
        let input = vec![b'x'; MAX_COMMAND_OUTPUT_BYTES * 4];

        read_capped(std::io::Cursor::new(input), &state);

        let state = state.into_inner().unwrap();
        assert_eq!(state.bytes.len(), MAX_COMMAND_OUTPUT_BYTES);
        assert!(state.truncated);
    }

    #[test]
    fn short_output_is_returned_whole() {
        let report = format_output(1, "out\n", "err\n", false);
        assert_eq!(report, "exit code: 1\nstdout:\nout\nstderr:\nerr\n");
    }

    #[test]
    fn successful_command_reports_its_output() {
        let dir = workspace();
        let (output, status) =
            SystemCommandRunner::new().run_with_status(dir.path(), "echo graphite-ok");
        assert_eq!(status, CommandStatus::Exited(0));
        let output = output.unwrap();
        assert!(output.starts_with("exit code: 0"), "{output}");
        assert!(output.contains("graphite-ok"), "{output}");
    }

    #[test]
    fn failing_command_is_an_error_with_its_exit_code() {
        let dir = workspace();
        let (output, status) = SystemCommandRunner::new().run_with_status(dir.path(), "exit 3");
        assert_eq!(status, CommandStatus::Exited(3));
        let output = output.unwrap_err();
        assert!(output.starts_with("exit code: 3"), "{output}");
    }

    /// A command that runs far longer than any test waits.
    fn long_command() -> &'static str {
        if cfg!(windows) {
            "ping -n 60 127.0.0.1 >nul"
        } else {
            "sleep 60"
        }
    }

    #[test]
    fn command_that_runs_too_long_is_stopped() {
        let dir = workspace();
        let started = Instant::now();
        let (result, status) = SystemCommandRunner::new()
            .with_timeout(Duration::from_millis(300))
            .run_with_status(dir.path(), long_command());
        assert_eq!(status, CommandStatus::TimedOut);
        let error = result.unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn cancelling_stops_a_running_command() {
        let dir = workspace();
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        let stopper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            trigger.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();

        let (result, status) = SystemCommandRunner::new()
            .with_cancel(cancel)
            .run_with_status(dir.path(), long_command());
        assert_eq!(status, CommandStatus::Cancelled);
        let error = result.unwrap_err();

        stopper.join().unwrap();
        assert!(error.contains("cancelled"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(20));
    }
}
