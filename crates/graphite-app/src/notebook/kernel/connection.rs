//! The connection file a kernel is started with: where its sockets are and the signing key.

use super::protocol::random_id;
use serde_json::json;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Connection {
    pub ip: String,
    pub key: String,
    pub shell_port: u16,
    pub iopub_port: u16,
    pub stdin_port: u16,
    pub control_port: u16,
    pub hb_port: u16,
}

impl Connection {
    /// Picks five free local ports and a new random key.
    pub(crate) fn allocate() -> std::io::Result<Self> {
        // Hold every listener until all ports are chosen so they are distinct.
        let listeners = (0..5)
            .map(|_| TcpListener::bind("127.0.0.1:0"))
            .collect::<Result<Vec<_>, _>>()?;
        let ports = listeners
            .iter()
            .map(|listener| listener.local_addr().map(|address| address.port()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            ip: "127.0.0.1".into(),
            key: random_id(),
            shell_port: ports[0],
            iopub_port: ports[1],
            stdin_port: ports[2],
            control_port: ports[3],
            hb_port: ports[4],
        })
    }

    pub(crate) fn address(&self, port: u16) -> String {
        format!("{}:{port}", self.ip)
    }

    /// The contents of the connection file.
    pub(crate) fn to_json(&self, kernel_name: &str) -> String {
        json!({
            "ip": self.ip,
            "transport": "tcp",
            "signature_scheme": "hmac-sha256",
            "key": self.key,
            "shell_port": self.shell_port,
            "iopub_port": self.iopub_port,
            "stdin_port": self.stdin_port,
            "control_port": self.control_port,
            "hb_port": self.hb_port,
            "kernel_name": kernel_name,
        })
        .to_string()
    }

    /// Writes the connection file where only the current user can read it, as it holds the key.
    pub(crate) fn write(&self, kernel_name: &str) -> std::io::Result<PathBuf> {
        let path = std::env::temp_dir().join(format!("graphite-kernel-{}.json", random_id()));
        write_private(&path, &self.to_json(kernel_name))?;
        Ok(path)
    }
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    // The temporary directory of a Windows user is not readable by other users.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocated_ports_are_distinct_and_the_key_is_random() {
        let one = Connection::allocate().unwrap();
        let two = Connection::allocate().unwrap();
        let ports = [
            one.shell_port,
            one.iopub_port,
            one.stdin_port,
            one.control_port,
            one.hb_port,
        ];
        let distinct: std::collections::HashSet<_> = ports.iter().collect();
        assert_eq!(distinct.len(), 5);
        assert!(ports.iter().all(|port| *port > 0));
        assert_ne!(one.key, two.key);
        assert_eq!(one.address(80), "127.0.0.1:80");
    }

    #[test]
    fn the_connection_file_has_every_field_a_kernel_needs() {
        let connection = Connection::allocate().unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&connection.to_json("python3")).unwrap();
        assert_eq!(value["transport"], "tcp");
        assert_eq!(value["signature_scheme"], "hmac-sha256");
        assert_eq!(value["ip"], "127.0.0.1");
        assert_eq!(value["key"], connection.key.as_str());
        assert_eq!(value["shell_port"], connection.shell_port);
        assert_eq!(value["hb_port"], connection.hb_port);
        assert_eq!(value["kernel_name"], "python3");
    }

    #[test]
    fn writing_creates_a_new_private_file_each_time() {
        let connection = Connection::allocate().unwrap();
        let first = connection.write("k").unwrap();
        let second = connection.write("k").unwrap();
        assert_ne!(first, second);
        assert!(std::fs::read_to_string(&first)
            .unwrap()
            .contains(&connection.key));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&first).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0);
        }
        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
    }
}
