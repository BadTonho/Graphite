//! Private subprocess channel to the versioned Code OSS protocol adapter.
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const COMMIT: &str = "c4453f8ea430795f941c1b2ff6c9321135a74a02";
const NODE_VERSION: &[u8] = b"v24.22.1";
const MAX_MESSAGE: usize = 32 * 1024 * 1024;
const QUEUE: usize = 128;

pub struct Session {
    outgoing: SyncSender<Value>,
    incoming: Receiver<Result<Value, String>>,
    stopped: Arc<AtomicBool>,
}

impl Session {
    pub fn bundled_runtime() -> Option<PathBuf> {
        let root = std::env::current_exe().ok()?.parent()?.join("code-oss");
        root.join("commit").is_file().then_some(root)
    }

    pub fn start(runtime: PathBuf, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (outgoing, commands) = mpsc::sync_channel(QUEUE);
        let (events, incoming) = mpsc::sync_channel(QUEUE);
        let wake = Arc::new(wake);
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stop = stopped.clone();
        thread::spawn(move || {
            if let Err(error) = run(&runtime, commands, &events, &wake, &worker_stop) {
                let _ = events.try_send(Err(error));
                wake();
            }
        });
        Self {
            outgoing,
            incoming,
            stopped,
        }
    }

    pub fn send(&self, message: Value) -> Result<(), String> {
        self.outgoing
            .try_send(message)
            .map_err(|error| error.to_string())
    }

    pub fn receive(&self) -> Option<Result<Value, String>> {
        match self.incoming.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Code OSS disconnected".into())),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
    }
}

fn node(path: &Path) -> Command {
    let mut command = Command::new(path);
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy().to_ascii_uppercase();
        if name.starts_with("NODE_")
            || name.starts_with("VSCODE_")
            || name.starts_with("ELECTRON_")
            || matches!(name.as_str(), "LD_PRELOAD" | "LD_LIBRARY_PATH")
        {
            command.env_remove(key);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn run(
    runtime: &Path,
    commands: Receiver<Value>,
    events: &SyncSender<Result<Value, String>>,
    wake: &Arc<impl Fn() + Send + Sync + 'static>,
    stopped: &AtomicBool,
) -> Result<(), String> {
    if stopped.load(Ordering::Acquire) {
        return Ok(());
    }
    if std::fs::read_to_string(runtime.join("commit"))
        .ok()
        .as_deref()
        .map(str::trim)
        != Some(COMMIT)
    {
        return Err("Invalid Code OSS runtime revision".into());
    }
    let executable = runtime.join(if cfg!(windows) { "node.exe" } else { "node" });
    let version = node(&executable)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    if !version.status.success() || version.stdout.trim_ascii() != NODE_VERSION {
        return Err("The bundled Code OSS runtime requires Node.js 24.22.1".into());
    }
    if stopped.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut child = node(&executable)
        .arg(runtime.join("bridge.mjs"))
        .current_dir(runtime)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let tx = events.clone();
    let wake_reader = wake.clone();
    thread::spawn(move || {
        let mut input = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            let result = (&mut input)
                .take((MAX_MESSAGE + 1) as u64)
                .read_until(b'\n', &mut line);
            let message = match result {
                Ok(0) => break,
                Ok(_) if line.len() <= MAX_MESSAGE && line.ends_with(b"\n") => {
                    serde_json::from_slice(&line).map_err(|error| error.to_string())
                }
                Ok(_) => Err("Invalid Code OSS frame length".into()),
                Err(error) => Err(error.to_string()),
            };
            let failed = message.is_err();
            if tx.try_send(message).is_err() {
                break;
            }
            wake_reader();
            if failed {
                break;
            }
        }
    });
    let result = loop {
        if stopped.load(Ordering::Acquire) {
            break Ok(());
        }
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(message) => {
                if stopped.load(Ordering::Acquire) {
                    break Ok(());
                }
                let mut bytes = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
                if bytes.len() >= MAX_MESSAGE {
                    break Err("Code OSS frame exceeds limit".into());
                }
                bytes.push(b'\n');
                if let Err(error) = stdin.write_all(&bytes) {
                    break Err(error.to_string());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => match child.try_wait() {
                Ok(Some(status)) => break Err(format!("Code OSS bridge exited ({status})")),
                Ok(None) => {}
                Err(error) => break Err(error.to_string()),
            },
        }
    };
    drop(stdin);
    finish(&mut child);
    result
}

fn finish(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
}
