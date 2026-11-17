//! Update channels, strict semantic version comparison, SHA-256 verification, and update state.

use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UpdateChannel {
    #[default]
    Stable,
    Prerelease,
}

impl UpdateChannel {
    pub(crate) fn from_str_lossy(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "stable" | "release" => Some(Self::Stable),
            "prerelease" | "pre-release" | "insiders" | "beta" => Some(Self::Prerelease),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Prerelease => "prerelease",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
}

impl SemVer {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().strip_prefix('v').unwrap_or(raw.trim());
        let no_build = trimmed.split('+').next()?;
        let (core, prerelease) = match no_build.split_once('-') {
            Some((c, p)) if !p.is_empty() => (c, Some(p.to_owned())),
            _ => (no_build, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_prerelease(a, b),
            })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_prerelease(a: &str, b: &str) -> Ordering {
    let mut ai = a.split('.');
    let mut bi = b.split('.');
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(sa), Some(sb)) => {
                let ord = match (sa.parse::<u64>(), sb.parse::<u64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => sa.cmp(sb),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReleaseInfo {
    pub tag_name: String,
    pub version: String,
    pub prerelease: bool,
    pub notes: String,
    pub html_url: String,
    pub asset: Option<ReleaseAsset>,
}

impl ReleaseInfo {
    pub(crate) fn is_newer_than(&self, current_version: &str) -> bool {
        match (SemVer::parse(&self.version), SemVer::parse(current_version)) {
            (Some(remote), Some(local)) => remote > local,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateStatus {
    Idle,
    Checking,
    Available(ReleaseInfo),
    Downloading {
        tag: String,
        progress_pct: u8,
    },
    ReadyToRestart {
        release: ReleaseInfo,
        staged_path: PathBuf,
    },
    Cancelling,
    UpToDate {
        checked_version: String,
    },
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UpdateConfig {
    pub channel: UpdateChannel,
    pub repo: String,
    pub auto_check: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            channel: UpdateChannel::Stable,
            repo: "BadTonho/Graphite".to_owned(),
            auto_check: true,
        }
    }
}

impl UpdateConfig {
    pub(crate) fn load_or_default(profile_root: &Path) -> Self {
        let path = profile_root.join("updates-config.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self, profile_root: &Path) {
        let _ = std::fs::create_dir_all(profile_root);
        let path = profile_root.join("updates-config.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let sum = digest(&SHA256, bytes);
    let mut out = String::with_capacity(64);
    for b in sum.as_ref() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub(crate) fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<(), String> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid expected SHA-256 digest format".into());
    }
    let actual = sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "SHA-256 verification failed: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

pub(crate) fn extract_asset_sha256_from_notes(notes: &str, asset_name: &str) -> Option<String> {
    for line in notes.lines() {
        let trimmed = line.trim();
        if let Some(hex) = trimmed.strip_prefix("sha256:") {
            let token = hex.split_whitespace().next()?;
            if token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(token.to_ascii_lowercase());
            }
        }
        let mut parts = trimmed.split_whitespace();
        if let (Some(hash), Some(file)) = (parts.next(), parts.next()) {
            let file_clean = file.trim_start_matches('*');
            if hash.len() == 64
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
                && file_clean == asset_name
            {
                return Some(hash.to_ascii_lowercase());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_ordering_and_sha256_verification_reject_stale_and_tampered_artifacts() {
        let current = "0.5.0";
        assert!(SemVer::parse("v0.5.1").unwrap() > SemVer::parse(current).unwrap());
        assert!(SemVer::parse("0.5.1").unwrap() > SemVer::parse("0.5.1-rc.1").unwrap());
        assert!(SemVer::parse("0.5.1-rc.2").unwrap() > SemVer::parse("0.5.1-rc.1").unwrap());
        assert!(SemVer::parse("0.5.0").unwrap() <= SemVer::parse(current).unwrap());

        let payload = b"graphite-installer-binary";
        let valid = sha256_hex(payload);
        assert!(verify_sha256(payload, &valid).is_ok());

        let err = verify_sha256(b"tampered-installer-binary", &valid).unwrap_err();
        assert!(err.contains("SHA-256 verification failed"));

        let notes = format!("{valid}  graphite-windows-x64.zip\n");
        assert_eq!(
            extract_asset_sha256_from_notes(&notes, "graphite-windows-x64.zip").as_deref(),
            Some(valid.as_str())
        );
    }
}
