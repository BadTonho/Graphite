use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum CliAction {
    Open {
        paths: Vec<PathBuf>,
        location: Option<(PathBuf, usize, usize)>,
    },
    Diff {
        left: PathBuf,
        right: PathBuf,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CliRequest {
    pub action: CliAction,
    pub wait: bool,
}

pub(crate) fn send_to_existing(request: &CliRequest) -> Result<bool, String> {
    send_to_socket(&socket_name(), request)
}

fn send_to_socket(name: &str, request: &CliRequest) -> Result<bool, String> {
    let name = name
        .to_owned()
        .to_ns_name::<GenericNamespaced>()
        .map_err(|error| error.to_string())?;
    let mut stream = match LocalSocketStream::connect(name) {
        Ok(stream) => stream,
        Err(_) => return Ok(false),
    };
    let mut json = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    json.push(b'\n');
    stream.write_all(&json).map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|error| error.to_string())?;
    if response.trim() == "done" {
        Ok(true)
    } else {
        Err("The existing editor rejected the CLI request".into())
    }
}

pub(crate) fn start_ipc_server(
    proxy: winit::event_loop::EventLoopProxy<crate::UserEvent>,
) -> std::io::Result<()> {
    let name = socket_name()
        .to_ns_name::<GenericNamespaced>()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let listener = ListenerOptions::new().name(name).create_sync()?;
    std::thread::Builder::new()
        .name("graphite-cli-ipc".into())
        .spawn(move || {
            for connection in listener.incoming().filter_map(Result::ok) {
                let proxy = proxy.clone();
                std::thread::Builder::new()
                    .name("graphite-cli-client".into())
                    .spawn(move || {
                        let mut connection = BufReader::new(connection);
                        let mut line = String::new();
                        if connection.read_line(&mut line).is_err() {
                            return;
                        }
                        let Ok(request) = serde_json::from_str::<CliRequest>(&line) else {
                            return;
                        };
                        let (acknowledge, acknowledged) = mpsc::channel();
                        if proxy
                            .send_event(crate::UserEvent::CliRequest {
                                request,
                                acknowledge,
                            })
                            .is_err()
                        {
                            return;
                        }
                        if acknowledged.recv().is_ok() {
                            let _ = connection.get_mut().write_all(b"done\n");
                            let _ = connection.get_mut().flush();
                        }
                    })
                    .ok();
            }
        })?;
    Ok(())
}

fn socket_name() -> String {
    use std::hash::{Hash, Hasher};
    let identity = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "local-user".into());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    identity.hash(&mut hasher);
    format!("graphite-{:016x}", hasher.finish())
}

pub(crate) fn parse_args(
    args: impl IntoIterator<Item = String>,
    cwd: PathBuf,
) -> Result<CliRequest, String> {
    let mut args = args.into_iter().peekable();
    let mut wait = false;
    let mut diff = false;
    let mut goto = None;
    let mut paths = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--wait" | "-w" => wait = true,
            "--diff" | "-d" => diff = true,
            "--goto" | "-g" => {
                let spec = args.next().ok_or("Expected file:line:column after -g")?;
                goto = Some(parse_location(&spec)?);
            }
            "--help" | "-h" => return Err(
                "Usage: code [path...] | code -d left right | code -g file:line:column [--wait]"
                    .into(),
            ),
            value if value.starts_with('-') => return Err(format!("Unknown option: {value}")),
            value => paths.push(PathBuf::from(value)),
        }
    }
    let action = if diff {
        if goto.is_some() || paths.len() != 2 {
            return Err(
                "Diff mode requires exactly two paths and cannot be combined with -g".into(),
            );
        }
        CliAction::Diff {
            left: absolute(&cwd, paths.remove(0)),
            right: absolute(&cwd, paths.remove(0)),
        }
    } else {
        if paths.is_empty() {
            paths.push(PathBuf::from("."));
        }
        let location = goto.map(|(file, line, column)| (absolute(&cwd, file), line, column));
        CliAction::Open {
            paths: paths.into_iter().map(|path| absolute(&cwd, path)).collect(),
            location,
        }
    };
    Ok(CliRequest { action, wait })
}

fn absolute(cwd: &std::path::Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

pub(crate) fn parse_location(spec: &str) -> Result<(PathBuf, usize, usize), String> {
    let (file_line, column) = spec.rsplit_once(':').ok_or("Expected file:line:column")?;
    let (file, line) = file_line
        .rsplit_once(':')
        .ok_or("Expected file:line:column")?;
    let line = line
        .parse::<usize>()
        .map_err(|_| "Line must be a positive integer")?;
    let column = column
        .parse::<usize>()
        .map_err(|_| "Column must be a positive integer")?;
    if file.is_empty() || line == 0 || column == 0 {
        return Err("File, line and column must be non-empty and positive".into());
    }
    Ok((PathBuf::from(file), line, column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_drive_path_by_splitting_numeric_suffixes() {
        let (file, line, column) = parse_location(r"C:\work\main.rs:12:5").unwrap();
        assert_eq!(file, PathBuf::from(r"C:\work\main.rs"));
        assert_eq!((line, column), (12, 5));
    }

    #[test]
    fn resolves_relative_paths_and_diff_arguments() {
        let cwd = PathBuf::from(r"C:\workspace");
        let request = parse_args(
            ["-d", "left.rs", "right.rs"].into_iter().map(String::from),
            cwd.clone(),
        )
        .unwrap();
        assert_eq!(
            request.action,
            CliAction::Diff {
                left: cwd.join("left.rs"),
                right: cwd.join("right.rs")
            }
        );
    }

    #[test]
    fn rejects_missing_or_invalid_arguments() {
        assert!(parse_location("file.rs:0:1").is_err());
        assert!(parse_args(
            ["-d", "one"].into_iter().map(String::from),
            PathBuf::from(".")
        )
        .is_err());
        assert!(parse_args(
            ["--unknown"].into_iter().map(String::from),
            PathBuf::from(".")
        )
        .is_err());
    }

    #[test]
    fn request_serializes_wait_mode_for_local_ipc() {
        let request = CliRequest {
            action: CliAction::Open {
                paths: vec![PathBuf::from("a.rs")],
                location: None,
            },
            wait: true,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<CliRequest>(&json).unwrap(), request);
    }

    #[test]
    fn forwards_cli_request_over_a_local_socket() {
        static NEXT_SOCKET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = format!(
            "graphite-test-{}-{}",
            std::process::id(),
            NEXT_SOCKET.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let local_name = name.clone().to_ns_name::<GenericNamespaced>().unwrap();
        let listener = ListenerOptions::new()
            .name(local_name)
            .create_sync()
            .unwrap();
        let server = std::thread::spawn(move || {
            let mut incoming = listener.incoming();
            let connection = incoming.next().unwrap().unwrap();
            let mut reader = BufReader::new(connection);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: CliRequest = serde_json::from_str(&line).unwrap();
            assert!(request.wait);
            assert!(matches!(request.action, CliAction::Open { .. }));
            reader.get_mut().write_all(b"done\n").unwrap();
        });
        let request = CliRequest {
            action: CliAction::Open {
                paths: vec![PathBuf::from("forwarded.rs")],
                location: None,
            },
            wait: true,
        };
        assert!(send_to_socket(&name, &request).unwrap());
        server.join().unwrap();
    }
}
