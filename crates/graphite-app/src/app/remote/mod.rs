//! Remote workspaces (OpenSSH), WebSocket tunnel relay, port forwarding, and Edit Sessions.

pub(crate) mod edit_sessions;
pub(crate) mod ssh;
pub(crate) mod tunnel;

use super::*;
use edit_sessions::{
    resolve_relative_workspace_path, EditSessionRecord, EditSessionStore, WorkingChangeFile,
};
use serde::{Deserialize, Serialize};
use ssh::{load_ssh_hosts, OpenSshTransport, SshHostConfig, SshTransport};
use tunnel::{RelayEnvelope, WebSocketTunnelClient};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ForwardedPort {
    pub local_port: u16,
    pub remote_port: u16,
    pub in_use: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionStatus {
    Disconnected,
    Connected,
    Paused,
}

#[derive(Debug)]
pub(crate) struct RemoteState {
    pub ssh_hosts: Vec<SshHostConfig>,
    pub active_host: Option<SshHostConfig>,
    pub status: ConnectionStatus,
    pub socket_paused: bool,
    pub tunnel_endpoint: Option<String>,
    pub forwarded_ports: Vec<ForwardedPort>,
    pub edit_sessions: EditSessionStore,
    pub reconnect_count: u32,
    pub last_message: Option<String>,
}

impl Default for RemoteState {
    fn default() -> Self {
        Self {
            ssh_hosts: Vec::new(),
            active_host: None,
            status: ConnectionStatus::Disconnected,
            socket_paused: false,
            tunnel_endpoint: None,
            forwarded_ports: Vec::new(),
            edit_sessions: EditSessionStore::default(),
            reconnect_count: 0,
            last_message: None,
        }
    }
}

impl RemoteState {
    pub fn load(profile_root: &Path) -> Self {
        Self {
            ssh_hosts: load_ssh_hosts(None),
            edit_sessions: EditSessionStore::load(profile_root),
            ..Self::default()
        }
    }

    pub fn isolated() -> Self {
        Self {
            edit_sessions: EditSessionStore::isolated(),
            ..Self::default()
        }
    }
}

impl App {
    pub(crate) fn reload_ssh_hosts(&mut self, config_path: Option<&Path>) -> usize {
        self.remote.ssh_hosts = load_ssh_hosts(config_path);
        let count = self.remote.ssh_hosts.len();
        self.status = format!("Loaded {count} SSH host configuration(s)");
        count
    }

    pub(crate) fn connect_ssh_host(&mut self, host: SshHostConfig) {
        self.remote.active_host = Some(host.clone());
        self.remote.status = ConnectionStatus::Connected;
        self.remote.socket_paused = false;
        self.remote
            .edit_sessions
            .log(format!("Connected remote host: {}", host.destination()));
        self.status = format!("Connected to SSH host {}", host.destination());
    }

    pub(crate) fn trigger_remote_reconnect(&mut self) {
        self.remote.reconnect_count += 1;
        self.remote.socket_paused = false;
        if let Some(host) = self.remote.active_host.clone() {
            let probe = OpenSshTransport::default().exec(&host, "true");
            match probe {
                Ok(out) if out.status_code == 0 => {
                    self.remote.status = ConnectionStatus::Connected;
                    self.status = format!("Reconnected to {}", host.destination());
                }
                Ok(out) => {
                    self.status = format!(
                        "Reconnect probe returned exit {} ({})",
                        out.status_code,
                        out.stderr.trim()
                    );
                }
                Err(err) => {
                    self.status = format!("Reconnect check failed: {err}");
                }
            }
        } else if let Some(url) = self.remote.tunnel_endpoint.clone() {
            match WebSocketTunnelClient::connect(&url) {
                Ok(mut client) => {
                    let _ = client.send_envelope(&RelayEnvelope::Ping);
                    client.close();
                    self.remote.status = ConnectionStatus::Connected;
                    self.status = format!("Reconnected tunnel {url}");
                }
                Err(err) => {
                    self.status = format!("Tunnel reconnect failed: {err}");
                }
            }
        } else {
            self.status = "No active remote host or tunnel to reconnect".into();
        }
    }

    pub(crate) fn toggle_pause_socket_writing(&mut self) {
        self.remote.socket_paused = !self.remote.socket_paused;
        if self.remote.socket_paused && self.remote.status == ConnectionStatus::Connected {
            self.remote.status = ConnectionStatus::Paused;
        } else if !self.remote.socket_paused && self.remote.status == ConnectionStatus::Paused {
            self.remote.status = ConnectionStatus::Connected;
        }
        self.status = if self.remote.socket_paused {
            "Paused remote socket writing"
        } else {
            "Resumed remote socket writing"
        }
        .into();
    }

    pub(crate) fn close_unused_remote_ports(&mut self) -> usize {
        let before = self.remote.forwarded_ports.len();
        self.remote.forwarded_ports.retain(|p| p.in_use);
        let closed_forward = before - self.remote.forwarded_ports.len();

        let owned_before = self.ports.listeners.len();
        let active_pids: std::collections::HashSet<u32> = self
            .ports
            .listeners
            .iter()
            .filter(|l| self.ports.can_stop(l.process_id))
            .map(|l| l.process_id)
            .collect();
        self.ports
            .listeners
            .retain(|l| active_pids.contains(&l.process_id));
        let closed_local = owned_before.saturating_sub(self.ports.listeners.len());

        let total = closed_forward + closed_local;
        self.status = format!("Closed {total} unused forwarded port(s)");
        total
    }

    pub(crate) fn open_remote_settings(&mut self) {
        self.open_preference_file("remote-settings.json");
        self.status = "Opened remote settings file".into();
    }

    pub(crate) fn store_current_edit_session(
        &mut self,
        custom_name: Option<&str>,
    ) -> EditSessionRecord {
        let root = self.tree.root().to_path_buf();
        let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        let mut files = Vec::new();
        for doc in self.documents.iter() {
            let Some(path) = doc.buffer.path() else {
                continue;
            };
            let Some(rel_path) = path
                .strip_prefix(&root)
                .ok()
                .map(std::path::Path::to_path_buf)
                .or_else(|| {
                    path.strip_prefix(&canonical_root)
                        .ok()
                        .map(std::path::Path::to_path_buf)
                })
                .or_else(|| {
                    std::fs::canonicalize(path).ok().and_then(|cp| {
                        cp.strip_prefix(&canonical_root)
                            .ok()
                            .map(|p| p.to_path_buf())
                    })
                })
            else {
                continue;
            };
            let rel = rel_path.to_string_lossy().replace('\\', "/");
            if (doc.is_dirty() || !rel.is_empty()) && !rel.is_empty() {
                files.push(WorkingChangeFile {
                    relative_path: rel,
                    content: doc.buffer.text(),
                    dirty: doc.is_dirty(),
                });
            }
        }
        let branch = crate::vcs::scm::snapshot(&root).branch;
        let remote_host = self
            .remote
            .active_host
            .as_ref()
            .map(SshHostConfig::destination);
        let name = custom_name
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Edit Session ({} files)", files.len()));
        let record = self.remote.edit_sessions.store_record(
            name,
            branch,
            remote_host,
            files,
            &self.profile_root,
        );
        self.status = format!("Stored edit session {}", record.id);
        record
    }

    pub(crate) fn apply_edit_session_record(
        &mut self,
        record: &EditSessionRecord,
    ) -> Result<usize, String> {
        let root = self.tree.root().to_path_buf();
        let mut restored = 0;
        for file in &record.files {
            let target = resolve_relative_workspace_path(&root, &file.relative_path)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, &file.content).map_err(|e| e.to_string())?;
            let existing_id = self
                .documents
                .iter()
                .find(|d| d.buffer.path() == Some(target.as_path()))
                .map(|d| d.id);
            if let Some(id) = existing_id {
                if let Some(doc) = self.documents.get_mut(id) {
                    doc.buffer.reload_text(&file.content);
                }
            } else {
                self.open_file(target);
            }
            restored += 1;
        }
        self.remote.edit_sessions.log(format!(
            "Resumed edit session {} ({restored} file(s))",
            record.id
        ));
        self.status = format!("Resumed edit session {} ({restored} file(s))", record.id);
        Ok(restored)
    }

    pub(crate) fn resume_latest_edit_session(&mut self) -> Result<usize, String> {
        let Some(latest) = self.remote.edit_sessions.sessions.first().cloned() else {
            self.status = "No stored edit sessions to resume".into();
            return Ok(0);
        };
        self.apply_edit_session_record(&latest)
    }

    pub(crate) fn resume_edit_session_from_payload(
        &mut self,
        payload: &str,
    ) -> Result<usize, String> {
        let record = EditSessionStore::parse_serialized_payload(payload)?;
        self.apply_edit_session_record(&record)
    }

    pub(crate) fn show_edit_sessions(&mut self) {
        self.new_untitled_document();
        let mut md = String::from("# Stored Edit Sessions\n\n");
        if self.remote.edit_sessions.sessions.is_empty() {
            md.push_str("_No edit sessions stored._\n");
        } else {
            for s in &self.remote.edit_sessions.sessions {
                md.push_str(&format!(
                    "- `{}` **{}** ({} files, branch: `{}`)\n",
                    s.id,
                    s.name,
                    s.files.len(),
                    s.branch.as_deref().unwrap_or("none")
                ));
            }
        }
        self.editor_mut().insert(&md);
        self.status = format!(
            "Showing {} edit session(s)",
            self.remote.edit_sessions.sessions.len()
        );
    }

    pub(crate) fn show_edit_session_output(&mut self) {
        self.new_untitled_document();
        let log = if self.remote.edit_sessions.output_log.is_empty() {
            "[Edit Sessions] Output log is empty.\n".to_owned()
        } else {
            self.remote.edit_sessions.output_log.join("\n")
        };
        self.editor_mut().insert(&log);
        self.status = "Opened Edit Sessions output channel".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_resume_delete_and_port_cleanup_work() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("hello.txt");
        std::fs::write(&file, "v1\n").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file.clone());

        let rec = app.store_current_edit_session(Some("snapshot-1"));
        assert_eq!(rec.files.len(), 1);
        std::fs::write(&file, "v2\n").unwrap();

        let restored = app.resume_latest_edit_session().unwrap();
        assert_eq!(restored, 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "v1\n");

        app.remote.forwarded_ports = vec![
            ForwardedPort {
                local_port: 3000,
                remote_port: 3000,
                in_use: true,
            },
            ForwardedPort {
                local_port: 4000,
                remote_port: 4000,
                in_use: false,
            },
        ];
        assert_eq!(app.close_unused_remote_ports(), 1);
        assert_eq!(app.remote.forwarded_ports.len(), 1);
    }
}
