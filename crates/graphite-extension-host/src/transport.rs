use graphite_core_api::{protocol::*, ServiceError, ServiceResult};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread,
};
type Transport = (
    Child,
    SyncSender<Vec<u8>>,
    Receiver<Message>,
    Arc<Mutex<Option<ServiceError>>>,
);
pub(super) fn spawn(
    path: &Path,
    args: &[std::ffi::OsString],
    wake: Arc<dyn Fn() + Send + Sync>,
    log_path: Option<PathBuf>,
) -> ServiceResult<Transport> {
    let mut command = Command::new(path);
    command.args(args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| ServiceError::Io(e.to_string()))?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let (tx, rx) = sync_channel::<Vec<u8>>(MAX_PENDING);
    let (events, incoming) = sync_channel(MAX_PENDING);
    let fault = Arc::new(Mutex::new(None));
    let read_fault = fault.clone();
    let read_wake = wake.clone();
    thread::spawn(move || {
        let error = loop {
            match read_message(&mut stdout) {
                Ok(Some(message)) => {
                    if events.try_send(message).is_err() {
                        break ServiceError::Busy;
                    }
                    read_wake();
                }
                Ok(None) => break ServiceError::HostStopped,
                Err(error) => break error,
            }
        };
        if let Ok(mut fault) = read_fault.lock() {
            if fault.is_none() {
                *fault = Some(error);
            }
        }
        read_wake();
    });
    let write_fault = fault.clone();
    thread::spawn(move || {
        while let Ok(bytes) = rx.recv() {
            if let Err(error) = stdin.write_all(&bytes).and_then(|_| stdin.flush()) {
                if let Ok(mut fault) = write_fault.lock() {
                    if fault.is_none() {
                        *fault = Some(ServiceError::Io(error.to_string()));
                    }
                }
                wake();
                break;
            }
        }
    });
    thread::spawn(move || {
        let mut log_file = log_path.and_then(|path| {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        });
        let mut bytes = [0; 4096];
        while let Ok(n) = stderr.read(&mut bytes) {
            if n == 0 {
                break;
            }
            if let Some(file) = log_file.as_mut() {
                let _ = file.write_all(&bytes[..n]);
                let _ = file.flush();
            } else {
                let _ = std::io::stderr().write_all(&bytes[..n]);
            }
        }
    });
    Ok((child, tx, incoming, fault))
}
