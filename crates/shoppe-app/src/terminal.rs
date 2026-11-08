use alacritty_terminal::{
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    term::{cell::Cell, color::Colors, Config, Term},
    vte::ansi::{Color, NamedColor, Processor, Rgb},
};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy)]
pub struct TerminalCell {
    pub character: char,
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub hidden: bool,
    pub wide_spacer: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TerminalSnapshot {
    pub rows: Vec<Vec<TerminalCell>>,
    pub cursor: Option<(usize, usize)>,
    pub exited: Option<String>,
}

enum WorkerMessage {
    Input(Vec<u8>),
    Output(Vec<u8>),
    Resize {
        rows: u16,
        columns: u16,
        pixel_width: u16,
        pixel_height: u16,
    },
    Scroll(i32),
    Stop,
    PtyWrite(String),
}

struct TerminalSize {
    rows: usize,
    columns: usize,
}
impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

#[derive(Clone)]
struct TerminalEvents(Sender<WorkerMessage>);

impl EventListener for TerminalEvents {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            let _ = self.0.send(WorkerMessage::PtyWrite(text));
        }
    }
}

pub struct TerminalSession {
    sender: Sender<WorkerMessage>,
    snapshot: Arc<Mutex<TerminalSnapshot>>,
    worker: Option<JoinHandle<()>>,
}

impl TerminalSession {
    #[cfg(test)]
    pub fn start(
        root: &Path,
        shell_override: Option<&str>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let shell = shell_override
            .map(str::to_owned)
            .or_else(|| {
                std::env::var("GRAPHITE_SHELL")
                    .ok()
                    .filter(|shell| !shell.trim().is_empty())
            })
            .or_else(|| {
                std::env::var("VSCODESHOPPE_SHELL")
                    .ok()
                    .filter(|shell| !shell.trim().is_empty())
            })
            .unwrap_or_else(default_shell);
        Self::start_profile(
            root,
            &crate::config::TerminalProfile {
                name: "Default".into(),
                path: shell,
                args: Vec::new(),
            },
            wake,
        )
    }

    pub fn start_profile(
        root: &Path,
        profile: &crate::config::TerminalProfile,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        let mut command = CommandBuilder::new(&profile.path);
        command.args(&profile.args);
        command.cwd(root);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        #[cfg(windows)]
        if (profile.path.to_ascii_lowercase().contains("powershell")
            || profile.path.to_ascii_lowercase().contains("pwsh"))
            && !profile
                .args
                .iter()
                .any(|arg| arg.eq_ignore_ascii_case("-NoLogo"))
        {
            command.arg("-NoLogo");
        }
        let mut child = pair.slave.spawn_command(command).map_err(|e| {
            format!(
                "Could not start shell '{}' ({}): {e}",
                profile.name, profile.path
            )
        })?;
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let master = pair.master;
        let snapshot = Arc::new(Mutex::new(TerminalSnapshot::default()));
        let snapshot_worker = snapshot.clone();
        let (sender, receiver) = mpsc::channel();
        let worker_sender = sender.clone();
        let reader_sender = sender.clone();
        thread::Builder::new()
            .name("shoppe-pty-reader".into())
            .spawn(move || {
                let mut reader = reader;
                let mut bytes = [0u8; 8192];
                loop {
                    match reader.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(n) => {
                            if reader_sender
                                .send(WorkerMessage::Output(bytes[..n].to_vec()))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let worker = thread::Builder::new()
            .name("shoppe-terminal".into())
            .spawn(move || {
                run_terminal_worker(
                    receiver,
                    worker_sender,
                    snapshot_worker,
                    wake,
                    master,
                    writer,
                    &mut child,
                );
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender,
            snapshot,
            worker: Some(worker),
        })
    }

    pub fn snapshot(&self) -> TerminalSnapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn input(&self, bytes: Vec<u8>) {
        let _ = self.sender.send(WorkerMessage::Input(bytes));
    }

    pub fn resize(&self, rows: u16, columns: u16, pixel_width: u16, pixel_height: u16) {
        let _ = self.sender.send(WorkerMessage::Resize {
            rows: rows.max(1),
            columns: columns.max(1),
            pixel_width,
            pixel_height,
        });
    }

    pub fn scroll(&self, delta: i32) {
        let _ = self.sender.send(WorkerMessage::Scroll(delta));
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerMessage::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_terminal_worker(
    receiver: Receiver<WorkerMessage>,
    sender: Sender<WorkerMessage>,
    snapshot: Arc<Mutex<TerminalSnapshot>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    mut writer: Box<dyn Write + Send>,
    child: &mut Box<dyn portable_pty::Child + Send + Sync>,
) {
    let mut term = Term::new(
        Config::default(),
        &TerminalSize {
            rows: 24,
            columns: 80,
        },
        TerminalEvents(sender.clone()),
    );
    let mut parser: Processor = Processor::new();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            if let Ok(mut state) = snapshot.lock() {
                state.exited = Some(format!("Shell exited ({})", status.exit_code()));
            }
            wake();
            break;
        }
        match receiver.recv_timeout(std::time::Duration::from_millis(30)) {
            Ok(WorkerMessage::Input(bytes)) => {
                let _ = writer.write_all(&bytes);
                let _ = writer.flush();
            }
            Ok(WorkerMessage::Output(bytes)) => {
                parser.advance(&mut term, &bytes);
                publish_snapshot(&term, &snapshot);
                wake();
            }
            Ok(WorkerMessage::Resize {
                rows,
                columns,
                pixel_width,
                pixel_height,
            }) => {
                let _ = master.resize(PtySize {
                    rows,
                    cols: columns,
                    pixel_width,
                    pixel_height,
                });
                term.resize(TerminalSize {
                    rows: rows as usize,
                    columns: columns as usize,
                });
                publish_snapshot(&term, &snapshot);
                wake();
            }
            Ok(WorkerMessage::Scroll(delta)) => {
                term.scroll_display(Scroll::Delta(-delta));
                publish_snapshot(&term, &snapshot);
                wake();
            }
            Ok(WorkerMessage::PtyWrite(text)) => {
                let _ = writer.write_all(text.as_bytes());
            }
            Ok(WorkerMessage::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn publish_snapshot(term: &Term<TerminalEvents>, snapshot: &Arc<Mutex<TerminalSnapshot>>) {
    let grid = term.grid();
    let offset = grid.display_offset() as i32;
    let mut rows = vec![Vec::with_capacity(grid.columns()); grid.screen_lines()];
    for indexed in grid.display_iter() {
        let row = (indexed.point.line.0 + offset) as usize;
        if row < rows.len() {
            let cell: &Cell = indexed.cell;
            rows[row].push(TerminalCell {
                character: cell.c,
                foreground: color_rgb(cell.fg, term.colors(), true),
                background: color_rgb(cell.bg, term.colors(), false),
                hidden: cell
                    .flags
                    .contains(alacritty_terminal::term::cell::Flags::HIDDEN),
                wide_spacer: cell.flags.intersects(
                    alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER
                        | alacritty_terminal::term::cell::Flags::LEADING_WIDE_CHAR_SPACER,
                ),
            });
        }
    }
    if let Ok(mut state) = snapshot.lock() {
        state.rows = rows;
        let point = grid.cursor.point;
        let line = (point.line.0 + offset) as usize;
        state.cursor = (line < grid.screen_lines()).then_some((line, point.column.0));
    }
}

fn color_rgb(color: Color, colors: &Colors, foreground: bool) -> [u8; 3] {
    let fallback = if foreground {
        [220, 220, 220]
    } else {
        [30, 30, 30]
    };
    match color {
        Color::Spec(Rgb { r, g, b }) => [r, g, b],
        Color::Indexed(index) => colors[index as usize]
            .map(rgb)
            .unwrap_or_else(|| xterm_color(index)),
        Color::Named(name) => {
            if let Some(rgb) = colors[name as usize] {
                return [rgb.r, rgb.g, rgb.b];
            }
            named_color(name).unwrap_or(fallback)
        }
    }
}

fn rgb(color: Rgb) -> [u8; 3] {
    [color.r, color.g, color.b]
}

fn named_color(color: NamedColor) -> Option<[u8; 3]> {
    use NamedColor::*;
    Some(match color {
        Black => [0, 0, 0],
        Red => [205, 49, 49],
        Green => [13, 188, 121],
        Yellow => [229, 229, 16],
        Blue => [36, 114, 200],
        Magenta => [188, 63, 188],
        Cyan => [17, 168, 205],
        White => [229, 229, 229],
        BrightBlack => [102, 102, 102],
        BrightRed => [241, 76, 76],
        BrightGreen => [35, 209, 139],
        BrightYellow => [245, 245, 67],
        BrightBlue => [59, 142, 234],
        BrightMagenta => [214, 112, 214],
        BrightCyan => [41, 184, 219],
        BrightWhite => [255, 255, 255],
        _ => return None,
    })
}

fn xterm_color(index: u8) -> [u8; 3] {
    const BASIC: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 49, 49],
        [13, 188, 121],
        [229, 229, 16],
        [36, 114, 200],
        [188, 63, 188],
        [17, 168, 205],
        [229, 229, 229],
        [102, 102, 102],
        [241, 76, 76],
        [35, 209, 139],
        [245, 245, 67],
        [59, 142, 234],
        [214, 112, 214],
        [41, 184, 219],
        [255, 255, 255],
    ];
    match index {
        0..=15 => BASIC[index as usize],
        16..=231 => {
            let n = index - 16;
            let component = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [component(n / 36), component((n / 6) % 6), component(n % 6)]
        }
        _ => {
            let v = 8 + (index - 232).saturating_mul(10);
            [v, v, v]
        }
    }
}

#[cfg(test)]
fn default_shell() -> String {
    #[cfg(windows)]
    {
        return "powershell.exe".into();
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "sh".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_parser_preserves_terminal_text_and_color() {
        let (events, _rx) = mpsc::channel();
        let mut term = Term::new(
            Config::default(),
            &TerminalSize {
                rows: 3,
                columns: 20,
            },
            TerminalEvents(events),
        );
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, "hello \x1b[31mred\x1b[0m 🌎".as_bytes());
        let snapshot = Arc::new(Mutex::new(TerminalSnapshot::default()));
        publish_snapshot(&term, &snapshot);
        let state = snapshot.lock().unwrap();
        assert_eq!(state.rows[0][0].character, 'h');
        assert_eq!(state.rows[0][6].character, 'r');
        assert_eq!(state.rows[0][6].foreground, [205, 49, 49]);
        assert!(state.rows[0].iter().any(|cell| cell.character == '🌎'));
    }

    #[test]
    fn ansi_256_color_cube_is_mapped() {
        assert_eq!(xterm_color(196), [255, 0, 0]);
        assert_eq!(xterm_color(244), [128, 128, 128]);
    }

    #[test]
    fn ansi_cursor_movement_replaces_text_and_reset_restores_color() {
        let (events, _rx) = mpsc::channel();
        let mut term = Term::new(
            Config::default(),
            &TerminalSize {
                rows: 2,
                columns: 20,
            },
            TerminalEvents(events),
        );
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, b"abc\r\x1b[31mZ\x1b[0mX");
        let snapshot = Arc::new(Mutex::new(TerminalSnapshot::default()));
        publish_snapshot(&term, &snapshot);
        let state = snapshot.lock().unwrap();
        assert_eq!(state.rows[0][0].character, 'Z');
        assert_eq!(state.rows[0][0].foreground, [205, 49, 49]);
        assert_eq!(state.rows[0][1].character, 'X');
        assert_ne!(state.rows[0][1].foreground, state.rows[0][0].foreground);
        assert_eq!(state.rows[0][2].character, 'c');
    }

    #[test]
    fn named_profile_arguments_are_passed_to_the_shell_process() {
        let temp = crate::test_support::TempDir::new();
        #[cfg(windows)]
        let profile = crate::config::TerminalProfile {
            name: "Command Prompt test".into(),
            path: "cmd.exe".into(),
            args: vec!["/C".into(), "echo".into(), "SHOPPE_PROFILE_ARGS".into()],
        };
        #[cfg(not(windows))]
        let profile = crate::config::TerminalProfile {
            name: "Shell args test".into(),
            path: "/bin/sh".into(),
            args: vec!["-c".into(), "printf SHOPPE_PROFILE_ARGS".into()],
        };
        let session =
            TerminalSession::start_profile(temp.path(), &profile, Arc::new(|| {})).unwrap();
        let mut output_seen = false;
        for _ in 0..100 {
            let text = session
                .snapshot()
                .rows
                .iter()
                .flat_map(|row| row.iter().map(|cell| cell.character))
                .collect::<String>();
            if text.contains("SHOPPE_PROFILE_ARGS") {
                output_seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            output_seen,
            "profile arguments were not passed to the shell"
        );
    }

    #[test]
    fn missing_terminal_executable_returns_a_profile_specific_error() {
        let temp = crate::test_support::TempDir::new();
        let profile = crate::config::TerminalProfile {
            name: "Missing test shell".into(),
            path: "shoppe-no-such-shell-84729".into(),
            args: Vec::new(),
        };
        let result = TerminalSession::start_profile(temp.path(), &profile, Arc::new(|| {}));
        let error = match result {
            Ok(_) => panic!("missing terminal unexpectedly started"),
            Err(error) => error,
        };
        assert!(error.contains("Missing test shell"));
        assert!(error.contains("shoppe-no-such-shell-84729"));
    }

    #[test]
    fn pty_captures_shell_output_and_reports_exit() {
        let temp = crate::test_support::TempDir::new();
        let root = temp.path();
        #[cfg(windows)]
        let shell = "cmd.exe";
        #[cfg(not(windows))]
        let shell = "/bin/sh";
        let session = TerminalSession::start(&root, Some(shell), Arc::new(|| {})).unwrap();
        #[cfg(windows)]
        session.input(b"echo SHOPPE_V03_READY\r".to_vec());
        #[cfg(not(windows))]
        session.input(b"echo SHOPPE_V03_READY\r".to_vec());
        let mut output_seen = false;
        for _ in 0..100 {
            let text = session
                .snapshot()
                .rows
                .iter()
                .flat_map(|row| row.iter().map(|cell| cell.character))
                .collect::<String>();
            if text.contains("SHOPPE_V03_READY") {
                output_seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            output_seen,
            "shell output was not received by the PTY reader"
        );
        session.resize(10, 30, 8, 16);
        let mut resized = false;
        for _ in 0..100 {
            let snapshot = session.snapshot();
            if snapshot.rows.len() == 10 && snapshot.rows.first().is_some_and(|row| row.len() == 30)
            {
                resized = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(resized, "PTY resize was not reflected by the terminal grid");
        session.input(b"exit\r".to_vec());
        let mut exited = false;
        for _ in 0..100 {
            if session.snapshot().exited.is_some() {
                exited = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(exited, "terminal did not report the shell exit");
        drop(session);
    }
}
