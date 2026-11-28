//! Talking to a debug adapter: starting or connecting to it, framing the messages it sends
//! and receives, and the threads that read them.

use super::*;

impl DapSession {
    pub fn spawn(config: &LaunchConfig, cwd: &Path) -> Result<Self, DebugError> {
        if let Some(server) = &config.server {
            return Self::connect(server);
        }
        if let Some(pipe) = &config.pipe {
            return Self::connect_pipe(pipe);
        }
        let adapter = config
            .adapter
            .as_ref()
            .ok_or_else(|| DebugError::MissingAdapter(config.adapter_type.clone()))?;
        let mut child = Command::new(&adapter.command);
        child.hide_console();
        child.args(&adapter.args).envs(&adapter.env);
        let mut child = child
            .current_dir(adapter.cwd.as_deref().unwrap_or(cwd))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .map(|stdin| Box::new(stdin) as Box<dyn Write + Send>);
        let stdout = child.stdout.take().ok_or(DebugError::Stopped)?;
        let stderr = child.stderr.take().ok_or(DebugError::Stopped)?;
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        thread::Builder::new()
            .name("graphite-dap-reader".into())
            .spawn(move || read_messages(stdout, sender))?;
        let (stderr_sender, _stderr_receiver) = mpsc::sync_channel::<String>(64);
        thread::Builder::new()
            .name("graphite-dap-stderr".into())
            .spawn(move || read_stderr(stderr, stderr_sender))?;
        Ok(Self {
            child: Some(child),
            stdin,
            socket: None,
            closer: None,
            events,
            sequence: 1,
        })
    }

    /// Connects to an adapter listening on a named pipe (Windows) or a local socket (Unix).
    /// Only local pipes are accepted: the path may come from an extension.
    pub fn connect_pipe(path: &Path) -> Result<Self, DebugError> {
        if !is_local_pipe(path) {
            return Err(DebugError::Transport(format!(
                "Debug adapter pipe {} is not a local pipe",
                path.display()
            )));
        }
        #[cfg(windows)]
        let stream = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        #[cfg(unix)]
        let stream = std::os::unix::net::UnixStream::connect(path)?;
        let reader = stream.try_clone()?;
        #[cfg(unix)]
        let closer: Option<Box<dyn Fn() + Send>> = {
            let handle = stream.try_clone()?;
            Some(Box::new(move || {
                let _ = handle.shutdown(Shutdown::Both);
            }))
        };
        #[cfg(windows)]
        let closer: Option<Box<dyn Fn() + Send>> = None;
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        thread::Builder::new()
            .name("graphite-dap-reader".into())
            .spawn(move || read_messages(reader, sender))?;
        Ok(Self {
            child: None,
            stdin: Some(Box::new(stream)),
            socket: None,
            closer,
            events,
            sequence: 1,
        })
    }

    /// A session whose adapter runs inside the extension host: the messages Graphite sends come
    /// out of `outgoing`, and what the adapter says is fed to the returned sender.
    pub fn inline(outgoing: mpsc::Sender<Value>) -> (Self, SyncSender<AdapterEvent>) {
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        let session = Self {
            child: None,
            stdin: Some(Box::new(InlineWriter {
                buffer: Vec::new(),
                sink: outgoing,
            })),
            socket: None,
            closer: None,
            events,
            sequence: 1,
        };
        (session, sender)
    }

    /// Connects to an adapter that is already listening. Only loopback addresses are accepted:
    /// the address may come from an extension, which must not make Graphite dial out.
    pub fn connect(server: &AdapterServer) -> Result<Self, DebugError> {
        let address = (server.host.as_str(), server.port)
            .to_socket_addrs()?
            .find(|address| address.ip().is_loopback())
            .ok_or_else(|| {
                DebugError::Transport(format!(
                    "Debug adapter server {}:{} is not a local address",
                    server.host, server.port
                ))
            })?;
        let socket = Arc::new(TcpStream::connect_timeout(
            &address,
            Duration::from_secs(5),
        )?);
        let reader = SharedTcp(socket.clone());
        let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        thread::Builder::new()
            .name("graphite-dap-reader".into())
            .spawn(move || read_messages(reader, sender))?;
        Ok(Self {
            child: None,
            stdin: Some(Box::new(SharedTcp(socket.clone()))),
            socket: Some(socket),
            closer: None,
            events,
            sequence: 1,
        })
    }

    /// A session without an adapter process, for tests that drive the protocol directly.
    #[cfg(test)]
    pub(super) fn detached(writer: Box<dyn Write + Send>) -> Self {
        let (_sender, events) = mpsc::sync_channel(1);
        Self {
            child: None,
            stdin: Some(writer),
            socket: None,
            closer: None,
            events,
            sequence: 1,
        }
    }

    pub fn send_request(&mut self, command: &str, arguments: Value) -> Result<i64, DebugError> {
        let seq = self.sequence;
        self.sequence += 1;
        self.send_message(serde_json::json!({
            "seq": seq,
            "type": "request",
            "command": command,
            "arguments": arguments,
        }))?;
        Ok(seq)
    }

    pub fn send_response(
        &mut self,
        request_seq: i64,
        command: &str,
        success: bool,
        body: Value,
        message: Option<&str>,
    ) -> Result<i64, DebugError> {
        let seq = self.sequence;
        self.sequence += 1;
        let mut response = serde_json::json!({
            "seq": seq,
            "type": "response",
            "request_seq": request_seq,
            "command": command,
            "success": success,
            "body": body,
        });
        if let Some(message) = message {
            response["message"] = Value::String(message.to_owned());
        }
        self.send_message(response)?;
        Ok(seq)
    }

    pub fn try_event(&self) -> Result<Option<AdapterEvent>, DebugError> {
        match self.events.try_recv() {
            Ok(AdapterEvent::Failed(error)) => Err(DebugError::Transport(error)),
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Ok(Some(AdapterEvent::Closed)),
        }
    }

    pub fn is_running(&mut self) -> Result<bool, DebugError> {
        match self.child.as_mut() {
            Some(child) => Ok(child.try_wait()?.is_none()),
            None => Ok(true),
        }
    }

    pub fn stop(&mut self) -> Result<(), DebugError> {
        self.stdin.take();
        if let Some(socket) = self.socket.take() {
            // Wakes the reader thread, which would otherwise wait for the adapter forever.
            let _ = socket.shutdown(Shutdown::Both);
        }
        if let Some(close) = self.closer.take() {
            close();
        }
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            let _ = child.wait();
        }
        Ok(())
    }

    fn send_message(&mut self, value: Value) -> Result<(), DebugError> {
        let stdin = self.stdin.as_mut().ok_or(DebugError::Stopped)?;
        let payload = serde_json::to_vec(&value)?;
        write!(stdin, "Content-Length: {}\r\n\r\n", payload.len())?;
        stdin.write_all(&payload)?;
        stdin.flush()?;
        Ok(())
    }
}

impl Drop for DapSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Whether `path` names a pipe on this machine: `\\.\pipe\name` on Windows, an absolute path
/// elsewhere.
fn is_local_pipe(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let text = path
            .to_string_lossy()
            .to_ascii_lowercase()
            .replace('/', "\\");
        text.starts_with("\\\\.\\pipe\\") && text.len() > "\\\\.\\pipe\\".len()
    }
    #[cfg(not(windows))]
    {
        path.is_absolute()
    }
}

/// Receives the framed messages Graphite writes (`Content-Length` header and JSON body) and
/// hands each to the extension host as a value.
struct InlineWriter {
    buffer: Vec<u8>,
    sink: mpsc::Sender<Value>,
}

impl Write for InlineWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        while let Some(header_end) = self
            .buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            let header = String::from_utf8_lossy(&self.buffer[..header_end]).into_owned();
            let length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())?
                })
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "a message has no length")
                })?;
            let end = header_end + 4 + length;
            if self.buffer.len() < end {
                break;
            }
            let message: Value = serde_json::from_slice(&self.buffer[header_end + 4..end])
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            self.buffer.drain(..end);
            self.sink.send(message).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the adapter is gone")
            })?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One TCP connection used from several places. It is shared, not duplicated with `try_clone`:
/// on Windows a duplicated socket is inheritable, so a process started later (a terminal, a task)
/// would keep the connection open after the session ends.
struct SharedTcp(Arc<TcpStream>);

impl Read for SharedTcp {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        (&*self.0).read(buffer)
    }
}

impl Write for SharedTcp {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        (&*self.0).write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        (&*self.0).flush()
    }
}

fn read_messages(stdout: impl Read, sender: SyncSender<AdapterEvent>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let event = match read_message(&mut reader) {
            Ok(Some(message)) => AdapterEvent::Message(message),
            Ok(None) => AdapterEvent::Closed,
            Err(error) => AdapterEvent::Failed(error.to_string()),
        };
        let last = !matches!(event, AdapterEvent::Message(_));
        let delivered = sender.send(event).is_ok();
        // The session is read by the event loop, which otherwise waits for input.
        crate::app::wake::request();
        if last || !delivered {
            break;
        }
    }
}

fn read_stderr(stderr: impl Read, sender: SyncSender<String>) {
    let reader = BufReader::new(stderr);
    for line in reader.lines().map_while(Result::ok) {
        let _ = sender.try_send(line);
        crate::app::wake::request();
    }
}

pub(super) fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, DebugError> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = String::new();
        let count = reader.read_line(&mut line)?;
        if count == 0 {
            return Ok(None);
        }
        header_bytes += count;
        if header_bytes > MAX_HEADER_BYTES {
            return Err(DebugError::Transport("DAP header is too large".into()));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.trim().split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| DebugError::Transport("invalid DAP Content-Length".into()))?,
                );
            }
        }
    }
    let length =
        length.ok_or_else(|| DebugError::Transport("missing DAP Content-Length".into()))?;
    if length > MAX_MESSAGE_BYTES {
        return Err(DebugError::Transport("DAP message is too large".into()));
    }
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    Ok(Some(serde_json::from_slice(&payload)?))
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    fn frame(value: &Value) -> Vec<u8> {
        let payload = serde_json::to_vec(value).unwrap();
        let mut bytes = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
        bytes.extend(payload);
        bytes
    }

    #[test]
    fn an_inline_adapter_receives_whole_messages_however_the_bytes_arrive() {
        let (sender, messages) = mpsc::channel();
        let (mut session, _events) = DapSession::inline(sender);
        let first = serde_json::json!({"seq": 1, "type": "request", "command": "initialize"});
        let second = serde_json::json!({"seq": 2, "type": "request", "command": "launch", "arguments": {"text": "é\n"}});
        // The writer gets the header and the body apart, and two messages in one piece.
        let mut stream = frame(&first);
        stream.extend(frame(&second));
        let stdin = session.stdin.as_mut().unwrap();
        for chunk in stream.chunks(7) {
            stdin.write_all(chunk).unwrap();
        }
        assert_eq!(messages.try_iter().collect::<Vec<_>>(), [first, second]);
        // A send through the session is framed and unframed again.
        let seq = session
            .send_request("threads", serde_json::json!({}))
            .unwrap();
        let sent = messages.try_recv().unwrap();
        assert_eq!(sent["seq"], seq);
        assert_eq!(sent["command"], "threads");
    }

    #[test]
    fn an_inline_adapter_reports_a_closed_session_and_garbage() {
        let (sender, messages) = mpsc::channel();
        let (mut session, events) = DapSession::inline(sender);
        events
            .send(AdapterEvent::Message(serde_json::json!({"type": "event"})))
            .unwrap();
        assert!(matches!(
            session.events.try_recv(),
            Ok(AdapterEvent::Message(_))
        ));
        // A message the extension host side dropped is a broken pipe for the writer.
        drop(messages);
        let error = session
            .send_request("next", serde_json::json!({}))
            .unwrap_err();
        assert!(
            matches!(error, DebugError::Read(ref io) if io.kind() == std::io::ErrorKind::BrokenPipe),
            "{error}"
        );
        // A header without a length is invalid.
        let (sender, _messages) = mpsc::channel();
        let (mut session, _events) = DapSession::inline(sender);
        let error = session
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"X: 1\r\n\r\n")
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn only_local_pipes_are_connected() {
        #[cfg(windows)]
        {
            assert!(is_local_pipe(Path::new(r"\\.\pipe\dap")));
            assert!(is_local_pipe(Path::new("//./pipe/dap")));
            assert!(!is_local_pipe(Path::new(r"\\server\pipe\dap")));
            assert!(!is_local_pipe(Path::new(r"C:\Users\me\file.txt")));
            assert!(!is_local_pipe(Path::new(r"\\.\pipe\")));
        }
        #[cfg(not(windows))]
        {
            assert!(is_local_pipe(Path::new("/tmp/dap.sock")));
            assert!(!is_local_pipe(Path::new("relative.sock")));
        }
        assert!(DapSession::connect_pipe(Path::new("relative-or-remote")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_session_talks_to_an_adapter_over_a_local_socket() {
        use std::io::Read;
        use std::os::unix::net::UnixListener;
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("dap.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let adapter = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = BufReader::new(stream.try_clone().unwrap());
            let message = read_message(&mut request).unwrap().unwrap();
            stream
                .write_all(&frame(&serde_json::json!({"seq": 1, "type": "response", "command": message["command"]})))
                .unwrap();
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest);
        });
        let mut session = DapSession::connect_pipe(&path).unwrap();
        session
            .send_request("initialize", serde_json::json!({}))
            .unwrap();
        let reply = match session.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            AdapterEvent::Message(message) => message,
            other => panic!("{other:?}"),
        };
        assert_eq!(reply["command"], "initialize");
        session.stop().unwrap();
        adapter.join().unwrap();
    }
}
