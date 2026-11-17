//! OpenSSH host configuration parser and transport over the system `ssh` CLI.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SshHostConfig {
    pub alias: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub identity_file: Option<String>,
}

impl SshHostConfig {
    pub fn destination(&self) -> String {
        let host = self.hostname.as_deref().unwrap_or(&self.alias);
        match &self.user {
            Some(user) if !user.is_empty() => format!("{user}@{host}"),
            _ => host.to_owned(),
        }
    }
}

/// Validates an SSH host alias/user/destination token to reject leading dashes and control chars.
pub(crate) fn validate_ssh_token(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("SSH target cannot be empty".into());
    }
    if trimmed.starts_with('-') {
        return Err(format!("SSH target cannot start with '-': {trimmed}"));
    }
    if trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("SSH target contains whitespace or control characters".into());
    }
    Ok(())
}

/// Parses an OpenSSH client config file (`~/.ssh/config`) into named host blocks.
pub(crate) fn parse_ssh_config(text: &str) -> Vec<SshHostConfig> {
    let mut hosts = Vec::new();
    let mut current: Option<SshHostConfig> = None;

    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, |c: char| c.is_whitespace() || c == '=');
        let key = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        let value = parts.next().unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }

        if key == "host" {
            if let Some(prev) = current.take() {
                hosts.push(prev);
            }
            let alias = value.split_whitespace().next().unwrap_or("").trim();
            if !alias.is_empty() && !alias.contains('*') && !alias.contains('?') {
                current = Some(SshHostConfig {
                    alias: alias.to_owned(),
                    hostname: None,
                    user: None,
                    port: None,
                    identity_file: None,
                });
            }
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };
        match key.as_str() {
            "hostname" => entry.hostname = Some(value.to_owned()),
            "user" => entry.user = Some(value.to_owned()),
            "port" => entry.port = value.parse::<u16>().ok(),
            "identityfile" => entry.identity_file = Some(value.to_owned()),
            _ => {}
        }
    }

    if let Some(last) = current.take() {
        hosts.push(last);
    }
    hosts
}

pub(crate) fn default_ssh_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var_os("HOME").map(PathBuf::from);
    home.map(|h| h.join(".ssh").join("config"))
}

pub(crate) fn load_ssh_hosts(config_path: Option<&Path>) -> Vec<SshHostConfig> {
    let owned;
    let path = match config_path {
        Some(p) => p,
        None => match default_ssh_config_path() {
            Some(p) => {
                owned = p;
                &owned
            }
            None => return Vec::new(),
        },
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_ssh_config(&raw)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SshCommandOutput {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) trait SshTransport: Send + Sync {
    fn exec(&self, target: &SshHostConfig, remote_cmd: &str) -> Result<SshCommandOutput, String>;
    #[cfg(test)]
    fn build_forward_args(
        &self,
        target: &SshHostConfig,
        local_port: u16,
        remote_port: u16,
    ) -> Result<Vec<String>, String>;
}

/// Executes commands and builds port-forwarding invocations using the system OpenSSH `ssh` binary.
#[derive(Debug, Clone, Default)]
pub(crate) struct OpenSshTransport {
    pub binary: Option<PathBuf>,
}

impl OpenSshTransport {
    fn program(&self) -> &Path {
        self.binary.as_deref().unwrap_or_else(|| Path::new("ssh"))
    }

    pub fn base_args(target: &SshHostConfig) -> Result<Vec<String>, String> {
        validate_ssh_token(&target.alias)?;
        let dest = target.destination();
        validate_ssh_token(&dest)?;
        let mut args = vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "ConnectTimeout=10".into(),
        ];
        if let Some(port) = target.port {
            args.push("-p".into());
            args.push(port.to_string());
        }
        if let Some(id) = &target.identity_file {
            args.push("-i".into());
            args.push(id.clone());
        }
        args.push("--".into());
        args.push(dest);
        Ok(args)
    }
}

impl SshTransport for OpenSshTransport {
    fn exec(&self, target: &SshHostConfig, remote_cmd: &str) -> Result<SshCommandOutput, String> {
        let mut args = Self::base_args(target)?;
        args.push(remote_cmd.to_owned());
        let output = Command::new(self.program())
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to invoke ssh: {e}"))?;
        Ok(SshCommandOutput {
            status_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    #[cfg(test)]
    fn build_forward_args(
        &self,
        target: &SshHostConfig,
        local_port: u16,
        remote_port: u16,
    ) -> Result<Vec<String>, String> {
        validate_ssh_token(&target.alias)?;
        let dest = target.destination();
        validate_ssh_token(&dest)?;
        let mut args = vec![
            "-N".into(),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            "-o".into(),
            "BatchMode=yes".into(),
            "-L".into(),
            format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}"),
        ];
        if let Some(port) = target.port {
            args.push("-p".into());
            args.push(port.to_string());
        }
        args.push("--".into());
        args.push(dest);
        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_config_hosts_and_skips_wildcards() {
        let cfg = "\
# Global defaults
Host *
    ServerAliveInterval 60

Host devbox
    HostName 10.0.0.42
    User alice
    Port 2222
    IdentityFile ~/.ssh/id_ed25519

Host staging
    HostName staging.example.com
";
        let hosts = parse_ssh_config(cfg);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "devbox");
        assert_eq!(hosts[0].destination(), "alice@10.0.0.42");
        assert_eq!(hosts[0].port, Some(2222));
        assert_eq!(hosts[1].alias, "staging");
        assert_eq!(hosts[1].destination(), "staging.example.com");
    }

    #[test]
    fn rejects_option_injection_in_ssh_target_and_builds_forward_args() {
        let bad = SshHostConfig {
            alias: "-oProxyCommand=calc".into(),
            hostname: None,
            user: None,
            port: None,
            identity_file: None,
        };
        assert!(OpenSshTransport::base_args(&bad).is_err());

        let ok = SshHostConfig {
            alias: "prod".into(),
            hostname: Some("prod.internal".into()),
            user: Some("deploy".into()),
            port: Some(2200),
            identity_file: None,
        };
        let args = OpenSshTransport::default()
            .build_forward_args(&ok, 8080, 3000)
            .unwrap();
        assert!(args
            .windows(2)
            .any(|w| w == ["-L", "127.0.0.1:8080:127.0.0.1:3000"]));
        assert!(args.windows(2).any(|w| w == ["--", "deploy@prod.internal"]));
    }
}
