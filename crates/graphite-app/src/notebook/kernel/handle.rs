//! A running kernel: the process, its sockets and the threads that talk to it.

use super::connection::Connection;
use super::interrupt::Interrupt;
use super::protocol::{decode, encode, event_from, KernelEvent, KernelState, Message};
use super::spec::KernelSpec;
use super::zmtp::Socket;
use serde_json::json;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a starting kernel gets to open its sockets.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// How much of the error output of a kernel that failed is kept.
const KEPT_ERROR_OUTPUT: usize = 4000;

/// Called from the kernel's threads whenever there are events to collect.
pub(crate) type Notify = Arc<dyn Fn() + Send + Sync>;

pub(crate) enum Command {
    Send {
        control: bool,
        frames: Vec<Vec<u8>>,
    },
    /// End the kernel process if it is still there after the given time.
    KillAfter(Duration),
    /// Send SIGINT to the kernel process (Unix kernels that interrupt with a signal).
    Signal,
    Kill,
}

/// The application's end of a kernel.
pub(crate) struct KernelHandle {
    commands: Sender<Command>,
    events: Receiver<KernelEvent>,
    key: Vec<u8>,
    session: String,
    interrupt_by_signal: bool,
}

impl KernelHandle {
    /// A handle over channels, for the process-backed kernel and for tests.
    pub(crate) fn from_parts(
        commands: Sender<Command>,
        events: Receiver<KernelEvent>,
        key: &str,
        interrupt_by_signal: bool,
    ) -> Self {
        Self {
            commands,
            events,
            key: key.as_bytes().to_vec(),
            session: super::protocol::random_id(),
            interrupt_by_signal,
        }
    }

    fn send(&self, control: bool, message: Message) -> String {
        let id = message.id().to_owned();
        let frames = encode(&self.key, &[], &message);
        let _ = self.commands.send(Command::Send { control, frames });
        id
    }

    /// Asks the kernel to run `code`. The returned id marks the events that belong to it.
    pub(crate) fn execute(&self, code: &str) -> String {
        self.send(
            false,
            Message::request(
                "execute_request",
                &self.session,
                json!({
                    "code": code,
                    "silent": false,
                    "store_history": true,
                    "user_expressions": {},
                    "allow_stdin": false,
                    "stop_on_error": true,
                }),
            ),
        )
    }

    pub(crate) fn request_info(&self) {
        self.send(
            false,
            Message::request("kernel_info_request", &self.session, json!({})),
        );
    }

    /// Interrupts what is running, by message and, for kernels that ask for it, by signal.
    pub(crate) fn interrupt(&self) {
        self.send(
            true,
            Message::request("interrupt_request", &self.session, json!({})),
        );
        if self.interrupt_by_signal {
            let _ = self.commands.send(Command::Signal);
        }
    }

    /// Asks the kernel to exit, and ends it if it has not after a short grace period.
    pub(crate) fn shutdown(&self) {
        self.send(
            true,
            Message::request("shutdown_request", &self.session, json!({"restart": false})),
        );
        let _ = self
            .commands
            .send(Command::KillAfter(Duration::from_secs(3)));
    }

    /// Ends the kernel process without asking.
    pub(crate) fn kill(&self) {
        let _ = self.commands.send(Command::Kill);
    }

    /// The events that arrived since the last call.
    pub(crate) fn drain(&self) -> Vec<KernelEvent> {
        self.events.try_iter().collect()
    }
}

impl Drop for KernelHandle {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Kill);
    }
}

fn keep_tail(buffer: &mut String, text: &str) {
    buffer.push_str(text);
    if buffer.len() > KEPT_ERROR_OUTPUT {
        let mut cut = buffer.len() - KEPT_ERROR_OUTPUT;
        while !buffer.is_char_boundary(cut) {
            cut += 1;
        }
        buffer.drain(..cut);
    }
}

/// Starts the kernel of `spec` in `directory`.
pub(crate) fn launch(
    spec: &KernelSpec,
    directory: &Path,
    notify: Notify,
) -> Result<KernelHandle, String> {
    let connection = Connection::allocate().map_err(|error| error.to_string())?;
    let file = connection
        .write(&spec.name)
        .map_err(|error| error.to_string())?;
    let argv: Vec<String> = spec
        .argv
        .iter()
        .map(|part| part.replace("{connection_file}", &file.to_string_lossy()))
        .collect();
    let mut command = std::process::Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(directory)
        .envs(&spec.env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let interrupt = if spec.interrupt_mode == "signal" {
        Interrupt::create()
    } else {
        None
    };
    if let Some((name, value)) = interrupt.as_ref().and_then(Interrupt::environment) {
        command.env(name, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window for the kernel.
        command.creation_flags(0x0800_0000);
    }
    let child = command.spawn().map_err(|error| {
        let _ = std::fs::remove_file(&file);
        format!("{}: {error}", argv[0])
    })?;
    Ok(attach(
        connection,
        interrupt,
        Some(child),
        Some(file),
        notify,
    ))
}

/// Connects to a kernel listening on the ports of `connection`. With a `child` the kernel is a
/// process this handle owns; its exit is reported, and it is ended when the handle is dropped.
pub(crate) fn attach(
    connection: Connection,
    interrupt: Option<Interrupt>,
    child: Option<Child>,
    cleanup: Option<PathBuf>,
    notify: Notify,
) -> KernelHandle {
    let (command_tx, command_rx) = channel();
    let (event_tx, event_rx) = channel();
    let handle =
        KernelHandle::from_parts(command_tx, event_rx, &connection.key, interrupt.is_some());
    let session = handle.session.clone();
    std::thread::Builder::new()
        .name("graphite-kernel".into())
        .spawn(move || {
            supervise(
                connection, session, command_rx, event_tx, child, interrupt, cleanup, notify,
            )
        })
        .ok();
    handle
}

fn connect(
    address: &str,
    socket_type: &str,
    identity: Option<&[u8]>,
    child: &mut Option<Child>,
    deadline: Instant,
) -> Result<Socket, String> {
    loop {
        match Socket::connect(address, socket_type, identity) {
            Ok(socket) => return Ok(socket),
            Err(error) => {
                if let Some(status) = child
                    .as_mut()
                    .and_then(|child| child.try_wait().ok().flatten())
                {
                    return Err(format!(
                        "the kernel exited ({status}) before it could be reached"
                    ));
                }
                if Instant::now() > deadline {
                    return Err(format!("the kernel did not answer in time: {error}"));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Reads a socket until it closes, turning messages into events.
fn read_events(
    mut socket: Socket,
    key: Vec<u8>,
    events: Sender<KernelEvent>,
    notify: Notify,
    forward: bool,
) {
    while let Ok(frames) = socket.recv() {
        if !forward {
            continue;
        }
        let Ok(message) = decode(&key, &frames) else {
            continue;
        };
        if let Some(event) = event_from(&message) {
            if events.send(event).is_err() {
                return;
            }
            notify();
        }
    }
}

fn supervise(
    connection: Connection,
    session: String,
    commands: Receiver<Command>,
    events: Sender<KernelEvent>,
    mut child: Option<Child>,
    interrupt: Option<Interrupt>,
    cleanup: Option<PathBuf>,
    notify: Notify,
) {
    let stderr = Arc::new(Mutex::new(String::new()));
    if let Some(pipe) = child.as_mut().and_then(|child| child.stderr.take()) {
        let stderr = stderr.clone();
        std::thread::spawn(move || {
            let mut pipe = pipe;
            let mut chunk = [0u8; 1024];
            while let Ok(count) = pipe.read(&mut chunk) {
                if count == 0 {
                    break;
                }
                if let Ok(mut kept) = stderr.lock() {
                    keep_tail(&mut kept, &String::from_utf8_lossy(&chunk[..count]));
                }
            }
        });
    }
    let finish = |child: &mut Option<Child>, details: String| {
        if let Some(child) = child.as_mut() {
            super::interrupt::kill_tree(child);
        }
        if let Some(path) = &cleanup {
            let _ = std::fs::remove_file(path);
        }
        let _ = events.send(KernelEvent::Exited { details });
        notify();
    };
    let _ = events.send(KernelEvent::Status {
        state: KernelState::Starting,
        parent: None,
    });
    notify();
    let deadline = Instant::now() + START_TIMEOUT;
    let sockets = (|| {
        let shell = connect(
            &connection.address(connection.shell_port),
            "DEALER",
            Some(session.as_bytes()),
            &mut child,
            deadline,
        )?;
        let iopub = connect(
            &connection.address(connection.iopub_port),
            "SUB",
            None,
            &mut child,
            deadline,
        )?;
        let control = connect(
            &connection.address(connection.control_port),
            "DEALER",
            Some(session.as_bytes()),
            &mut child,
            deadline,
        )?;
        Ok::<_, String>((shell, iopub, control))
    })();
    let (mut shell, mut iopub, mut control) = match sockets {
        Ok(sockets) => sockets,
        Err(error) => {
            let output = stderr.lock().map(|text| text.clone()).unwrap_or_default();
            let details = if output.trim().is_empty() {
                error
            } else {
                format!("{error}\n{output}")
            };
            finish(&mut child, details);
            return;
        }
    };
    let key = connection.key.as_bytes().to_vec();
    if iopub.subscribe_all().is_err() {
        finish(
            &mut child,
            "could not subscribe to the kernel output".into(),
        );
        return;
    }
    let readers: Vec<(Socket, bool)> = [(&shell, true), (&iopub, true), (&control, false)]
        .into_iter()
        .filter_map(|(socket, forward)| socket.try_clone().ok().map(|clone| (clone, forward)))
        .collect();
    for (socket, forward) in readers {
        let (key, events, notify) = (key.clone(), events.clone(), notify.clone());
        std::thread::spawn(move || read_events(socket, key, events, notify, forward));
    }
    let mut kill_at: Option<Instant> = None;
    // Ask who we are talking to; the answer reports the language.
    let info = Message::request("kernel_info_request", &session, json!({}));
    let _ = shell.send(&encode(&key, &[], &info));
    loop {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Send {
                control: on_control,
                frames,
            }) => {
                let socket = if on_control { &mut control } else { &mut shell };
                let _ = socket.send(&frames);
            }
            Ok(Command::KillAfter(grace)) => kill_at = Some(Instant::now() + grace),
            Ok(Command::Signal) => {
                if let (Some(interrupt), Some(child)) = (interrupt.as_ref(), child.as_ref()) {
                    interrupt.signal(child.id());
                }
            }
            Ok(Command::Kill) | Err(RecvTimeoutError::Disconnected) => {
                shell.shutdown();
                iopub.shutdown();
                control.shutdown();
                finish(&mut child, String::new());
                return;
            }
            Err(RecvTimeoutError::Timeout) => {
                if kill_at.is_some_and(|at| Instant::now() >= at) {
                    shell.shutdown();
                    iopub.shutdown();
                    control.shutdown();
                    finish(&mut child, String::new());
                    return;
                }
                if let Some(status) = child
                    .as_mut()
                    .and_then(|child| child.try_wait().ok().flatten())
                {
                    shell.shutdown();
                    iopub.shutdown();
                    control.shutdown();
                    let output = stderr.lock().map(|text| text.clone()).unwrap_or_default();
                    finish(
                        &mut child,
                        format!("the kernel exited ({status})\n{output}"),
                    );
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A kernel that speaks the real protocol over real sockets, written in Rust for tests.

    use super::*;
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    fn reply(parent: &Message, msg_type: &str, content: serde_json::Value) -> Message {
        Message::reply_to(parent, msg_type, content)
    }

    /// Starts the fake kernel on the ports of `connection`. It runs `print`-like code: the code
    /// `raise` produces an error, `sleep` waits for an interrupt, anything else prints it.
    pub(crate) fn start(connection: &Connection) -> JoinHandle<Vec<String>> {
        let key = connection.key.as_bytes().to_vec();
        let listeners = [
            connection.shell_port,
            connection.iopub_port,
            connection.control_port,
        ]
        .map(|port| TcpListener::bind(connection.address(port)).unwrap());
        std::thread::spawn(move || {
            let mut shell = Socket::accept(&listeners[0], "ROUTER").unwrap();
            let mut iopub = Socket::accept(&listeners[1], "PUB").unwrap();
            let mut control = Socket::accept(&listeners[2], "ROUTER").unwrap();
            // The subscription arrives first.
            let _ = iopub.recv();
            let interrupted = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = interrupted.clone();
            let control_key = key.clone();
            let control_thread = std::thread::spawn(move || {
                let mut log = Vec::new();
                while let Ok(frames) = control.recv() {
                    let Ok(message) = decode(&control_key, &frames) else {
                        continue;
                    };
                    log.push(message.msg_type().to_owned());
                    if message.msg_type() == "interrupt_request" {
                        flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    let kind = message.msg_type().replace("_request", "_reply");
                    let _ = control.send(&encode(
                        &control_key,
                        &[],
                        &reply(&message, &kind, json!({"status": "ok"})),
                    ));
                    if message.msg_type() == "shutdown_request" {
                        break;
                    }
                }
                log
            });
            let mut count = 0u64;
            let mut seen = Vec::new();
            while let Ok(frames) = shell.recv() {
                let Ok(request) = decode(&key, &frames) else {
                    continue;
                };
                seen.push(request.msg_type().to_owned());
                let publish = |iopub: &mut Socket, msg_type: &str, content: serde_json::Value| {
                    let message = reply(&request, msg_type, content);
                    let mut frames = encode(&key, &[msg_type.as_bytes().to_vec()], &message);
                    frames.rotate_left(0);
                    let _ = iopub.send(&frames);
                };
                match request.msg_type() {
                    "kernel_info_request" => {
                        let _ = shell.send(&encode(
                            &key,
                            &[],
                            &reply(
                                &request,
                                "kernel_info_reply",
                                json!({"status": "ok", "language_info": {"name": "fake"}, "banner": "fake kernel"}),
                            ),
                        ));
                    }
                    "execute_request" => {
                        let code = request.content["code"].as_str().unwrap_or("").to_owned();
                        count += 1;
                        publish(&mut iopub, "status", json!({"execution_state": "busy"}));
                        publish(
                            &mut iopub,
                            "execute_input",
                            json!({"code": code, "execution_count": count}),
                        );
                        let mut ok = true;
                        match code.as_str() {
                            "raise" => {
                                ok = false;
                                publish(
                                    &mut iopub,
                                    "error",
                                    json!({"ename": "Boom", "evalue": "raised", "traceback": ["Traceback", "Boom: raised"]}),
                                );
                            }
                            "sleep" => {
                                for _ in 0..200 {
                                    if interrupted.swap(false, std::sync::atomic::Ordering::SeqCst)
                                    {
                                        ok = false;
                                        publish(
                                            &mut iopub,
                                            "error",
                                            json!({"ename": "KeyboardInterrupt", "evalue": "", "traceback": ["KeyboardInterrupt"]}),
                                        );
                                        break;
                                    }
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                            }
                            other => {
                                publish(
                                    &mut iopub,
                                    "stream",
                                    json!({"name": "stdout", "text": format!("{other}\n")}),
                                );
                                publish(
                                    &mut iopub,
                                    "execute_result",
                                    json!({"execution_count": count, "data": {"text/plain": format!("'{other}'")}, "metadata": {}}),
                                );
                            }
                        }
                        publish(&mut iopub, "status", json!({"execution_state": "idle"}));
                        let _ = shell.send(&encode(
                            &key,
                            &[],
                            &reply(
                                &request,
                                "execute_reply",
                                json!({"status": if ok { "ok" } else { "error" }, "execution_count": count}),
                            ),
                        ));
                    }
                    _ => {}
                }
            }
            let mut log = control_thread.join().unwrap_or_default();
            log.extend(seen);
            log
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for(handle: &KernelHandle, done: impl Fn(&[KernelEvent]) -> bool) -> Vec<KernelEvent> {
        let mut all = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            all.extend(handle.drain());
            if done(&all) {
                return all;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for kernel events: {all:?}");
    }

    /// Ports are chosen by binding and releasing; this keeps parallel tests from choosing the
    /// same ones between choosing and binding.
    static PORTS: Mutex<()> = Mutex::new(());

    fn connected() -> (KernelHandle, std::thread::JoinHandle<Vec<String>>) {
        let guard = PORTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let connection = Connection::allocate().unwrap();
        let kernel = fake::start(&connection);
        drop(guard);
        let handle = attach(connection, None, None, None, Arc::new(|| {}));
        (handle, kernel)
    }

    fn finished(events: &[KernelEvent], id: &str) -> bool {
        events
            .iter()
            .any(|event| matches!(event, KernelEvent::Reply { parent, .. } if parent == id))
    }

    #[test]
    fn executing_code_reports_input_outputs_status_and_the_reply() {
        let (handle, _kernel) = connected();
        let id = handle.execute("hello");
        // The reply comes on the shell channel and the outputs on IOPub, in either order; the
        // idle status is the last thing IOPub says about the request.
        let events = wait_for(&handle, |events| {
            finished(events, &id)
                && events.iter().any(|event| matches!(
                    event,
                    KernelEvent::Status { state: KernelState::Idle, parent: Some(parent) } if parent == &id
                ))
        });
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Info { language: Some(language), .. } if language == "fake"
        )));
        assert!(matches!(
            events[0],
            KernelEvent::Status {
                state: KernelState::Starting,
                ..
            }
        ));
        let mine: Vec<_> = events
            .iter()
            .filter(|event| match event {
                KernelEvent::Output { parent, .. } | KernelEvent::ExecuteInput { parent, .. } => {
                    parent == &id
                }
                _ => false,
            })
            .collect();
        assert!(matches!(
            mine[0],
            KernelEvent::ExecuteInput { count: 1, .. }
        ));
        let KernelEvent::Output { output, .. } = mine[1] else {
            panic!("expected a stream")
        };
        assert_eq!(output["text"], "hello\n");
        let KernelEvent::Output { output, .. } = mine[2] else {
            panic!("expected a result")
        };
        assert_eq!(output["data"]["text/plain"], "'hello'");
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Reply {
                ok: true,
                count: Some(1),
                ..
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Status { state: KernelState::Busy, parent: Some(parent) } if parent == &id
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Status { state: KernelState::Idle, parent: Some(parent) } if parent == &id
        )));
    }

    #[test]
    fn an_error_in_the_code_arrives_as_an_error_output_and_a_failed_reply() {
        let (handle, _kernel) = connected();
        let id = handle.execute("raise");
        // The reply (shell channel) and the error output (IOPub) can arrive in either order,
        // so wait for both instead of stopping at the first one.
        let events = wait_for(&handle, |events| {
            finished(events, &id)
                && events.iter().any(|event| {
                    matches!(
                        event,
                        KernelEvent::Output { output, .. } if output["ename"] == "Boom"
                    )
                })
        });
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Output { output, .. } if output["ename"] == "Boom"
        )));
        assert!(events
            .iter()
            .any(|event| matches!(event, KernelEvent::Reply { ok: false, .. })));
    }

    #[test]
    fn requests_run_in_the_order_they_were_sent_with_increasing_counts() {
        let (handle, _kernel) = connected();
        let first = handle.execute("a");
        let second = handle.execute("b");
        let events = wait_for(&handle, |events| finished(events, &second));
        let counts: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                KernelEvent::Reply { parent, count, .. } => Some((parent.clone(), *count)),
                _ => None,
            })
            .collect();
        assert_eq!(counts, vec![(first, Some(1)), (second, Some(2))]);
    }

    #[test]
    fn an_interrupt_stops_running_code() {
        let (handle, _kernel) = connected();
        let id = handle.execute("sleep");
        wait_for(&handle, |events| {
            events
                .iter()
                .any(|event| matches!(event, KernelEvent::ExecuteInput { .. }))
        });
        handle.interrupt();
        // The reply and the KeyboardInterrupt output travel on different channels, so the
        // output can arrive after the reply; wait for both.
        let events = wait_for(&handle, |events| {
            finished(events, &id)
                && events.iter().any(|event| {
                    matches!(
                        event,
                        KernelEvent::Output { output, .. } if output["ename"] == "KeyboardInterrupt"
                    )
                })
        });
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Output { output, .. } if output["ename"] == "KeyboardInterrupt"
        )));
    }

    #[test]
    fn shutting_down_closes_the_kernel_and_the_handle_is_dropped_cleanly() {
        let (handle, kernel) = connected();
        let id = handle.execute("x");
        wait_for(&handle, |events| finished(events, &id));
        handle.shutdown();
        drop(handle);
        let log = kernel.join().unwrap();
        assert!(log.contains(&"shutdown_request".to_owned()));
        assert!(log.contains(&"execute_request".to_owned()));
        assert!(log.contains(&"kernel_info_request".to_owned()));
    }

    #[test]
    fn a_kernel_that_never_listens_is_reported_as_gone() {
        // Nothing listens on these ports and the process exits at once.
        let connection = Connection::allocate().unwrap();
        let child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
            .args(if cfg!(windows) {
                ["/C", "exit 3"]
            } else {
                ["-c", "exit 3"]
            })
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let handle = attach(connection, None, Some(child), None, Arc::new(|| {}));
        let events = wait_for(&handle, |events| {
            events
                .iter()
                .any(|event| matches!(event, KernelEvent::Exited { .. }))
        });
        let Some(KernelEvent::Exited { details }) = events.last() else {
            panic!("expected the exit event last");
        };
        assert!(details.contains("exited"), "{details}");
    }

    #[test]
    fn launching_a_missing_program_is_an_error_with_its_name() {
        let spec = KernelSpec {
            name: "missing".into(),
            display_name: "Missing".into(),
            language: "x".into(),
            argv: vec![
                "graphite-no-such-kernel-program".into(),
                "{connection_file}".into(),
            ],
            env: Default::default(),
            interrupt_mode: "signal".into(),
        };
        let error = launch(&spec, &std::env::temp_dir(), Arc::new(|| {}))
            .err()
            .unwrap();
        assert!(error.contains("graphite-no-such-kernel-program"));
    }

    /// Runs against a real ipykernel. Set `GRAPHITE_TEST_KERNEL_PYTHON` to a Python that has
    /// `ipykernel` installed and run with `--ignored`.
    #[test]
    #[ignore = "needs a Python with ipykernel (GRAPHITE_TEST_KERNEL_PYTHON)"]
    fn a_real_ipykernel_runs_code_reports_errors_and_can_be_interrupted() {
        let Ok(python) = std::env::var("GRAPHITE_TEST_KERNEL_PYTHON") else {
            panic!("set GRAPHITE_TEST_KERNEL_PYTHON");
        };
        let spec = KernelSpec {
            name: "python3".into(),
            display_name: "Python 3".into(),
            language: "python".into(),
            argv: vec![
                python,
                "-m".into(),
                "ipykernel_launcher".into(),
                "-f".into(),
                "{connection_file}".into(),
            ],
            env: Default::default(),
            interrupt_mode: "signal".into(),
        };
        let handle = launch(&spec, &std::env::temp_dir(), Arc::new(|| {})).unwrap();
        let id = handle.execute("print('hi')\n1 + 1");
        let events = wait_for(&handle, |events| {
            finished(events, &id)
                && events.iter().any(|event| {
                    matches!(
                        event,
                        KernelEvent::Output { output, .. } if output["output_type"] == "stream" && output["text"] == "hi\n"
                    )
                })
        });
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Output { output, .. } if output["output_type"] == "stream" && output["text"] == "hi\n"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Output { output, .. } if output["data"]["text/plain"] == "2"
        )));
        let id = handle.execute("1 / 0");
        let events = wait_for(&handle, |events| {
            finished(events, &id)
                && events.iter().any(|event| {
                    matches!(
                        event,
                        KernelEvent::Output { output, .. } if output["ename"] == "ZeroDivisionError"
                    )
                })
        });
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Output { output, .. } if output["ename"] == "ZeroDivisionError"
        )));
        let id = handle.execute(
            "import time\nend = time.time() + 30
while time.time() < end:
    pass",
        );
        wait_for(&handle, |events| {
            events.iter().any(
                |event| matches!(event, KernelEvent::ExecuteInput { parent, .. } if parent == &id),
            )
        });
        // Give the kernel time to be inside the loop; an interrupt before that is lost. (A busy loop,
        // because on Windows Python only notices the interrupt once `time.sleep` returns.)
        std::thread::sleep(Duration::from_millis(1500));
        handle.interrupt();
        // The reply and the KeyboardInterrupt output travel on different channels, so the
        // output can arrive after the reply; wait for both.
        let events = wait_for(&handle, |events| {
            finished(events, &id)
                && events.iter().any(|event| {
                    matches!(
                        event,
                        KernelEvent::Output { output, .. } if output["ename"] == "KeyboardInterrupt"
                    )
                })
        });
        assert!(events.iter().any(|event| matches!(
            event,
            KernelEvent::Output { output, .. } if output["ename"] == "KeyboardInterrupt"
        )));
        handle.shutdown();
        wait_for(&handle, |events| {
            events
                .iter()
                .any(|event| matches!(event, KernelEvent::Exited { .. }))
        });
    }

    #[test]
    fn only_the_tail_of_error_output_is_kept() {
        let mut kept = String::new();
        keep_tail(&mut kept, &"é".repeat(KEPT_ERROR_OUTPUT));
        assert!(kept.len() <= KEPT_ERROR_OUTPUT);
        assert!(kept.chars().all(|c| c == 'é'));
        keep_tail(&mut kept, "end");
        assert!(kept.ends_with("end"));
    }

    #[test]
    fn a_scripted_handle_encodes_requests_for_the_test_to_inspect() {
        let (command_tx, command_rx) = channel();
        let (_event_tx, event_rx) = channel();
        let handle = KernelHandle::from_parts(command_tx, event_rx, "k", true);
        let id = handle.execute("1 + 1");
        handle.interrupt();
        let Command::Send {
            control: false,
            frames,
        } = command_rx.recv().unwrap()
        else {
            panic!("expected a shell message");
        };
        let message = decode(b"k", &frames).unwrap();
        assert_eq!(message.id(), id);
        assert_eq!(message.content["code"], "1 + 1");
        assert!(matches!(
            command_rx.recv().unwrap(),
            Command::Send { control: true, .. }
        ));
        assert!(matches!(command_rx.recv().unwrap(), Command::Signal));
        drop(handle);
        assert!(matches!(command_rx.recv().unwrap(), Command::Kill));
    }
}
