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
    /// Digest declared in the release notes (<hex>  <asset>), if any.
    pub sha256: Option<String>,
    /// Companion <asset>.sha256 file, used when the notes do not declare a digest.
    #[serde(default)]
    pub sha256_url: Option<String>,
    /// Companion <asset>.sig Ed25519 signature file (mandatory for installation).
    #[serde(default)]
    pub signature_url: Option<String>,
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

    pub(crate) fn save(&self, profile_root: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(profile_root)?;
        let path = profile_root.join("updates-config.json");
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
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

fn is_sha256_hex(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Parses a checksum document (`<hex>  <file>` lines, sha256sum style, or a bare digest) and
/// returns the digest declared for `asset_name`. Lines naming another file are never used.
pub(crate) fn extract_asset_sha256_from_notes(notes: &str, asset_name: &str) -> Option<String> {
    let mut bare_candidates: Vec<String> = Vec::new();
    for line in notes.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', '`']).trim();
        let mut parts = trimmed.split_whitespace();
        let Some(first) = parts.next() else {
            continue;
        };
        if let Some(hex) = first.strip_prefix("sha256:") {
            // `sha256:<hex> [file]` or `sha256: <hex> [file]`
            let (digest, file) = if hex.is_empty() {
                (parts.next().unwrap_or(""), parts.next())
            } else {
                (hex, parts.next())
            };
            if !is_sha256_hex(digest) {
                continue;
            }
            match file.map(|f| f.trim_matches(['*', '`'])) {
                Some(name) if name == asset_name => return Some(digest.to_ascii_lowercase()),
                Some(_) => {}
                None => bare_candidates.push(digest.to_ascii_lowercase()),
            }
            continue;
        }
        if is_sha256_hex(first) {
            match parts
                .next()
                .map(|f| f.trim_start_matches('*').trim_matches('`'))
            {
                Some(name) if name == asset_name => return Some(first.to_ascii_lowercase()),
                Some(_) => {}
                None => bare_candidates.push(first.to_ascii_lowercase()),
            }
        }
    }
    // A bare digest is only trusted when it is the single one declared (unambiguous).
    bare_candidates.dedup();
    if bare_candidates.len() == 1 {
        bare_candidates.pop()
    } else {
        None
    }
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

    #[test]
    fn checksum_parser_never_uses_a_digest_declared_for_another_file() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let notes = format!("{a}  graphite-linux-x64.tar.gz\n{b} *graphite-windows-x64.zip\n");
        assert_eq!(
            extract_asset_sha256_from_notes(&notes, "graphite-windows-x64.zip").as_deref(),
            Some(b.as_str())
        );
        assert_eq!(
            extract_asset_sha256_from_notes(&notes, "graphite-macos.zip"),
            None
        );

        // `sha256:` lines naming another file are skipped; a malformed line does not abort.
        let notes = format!("sha256:zzzz\nsha256:{a} other.zip\nsha256: {b} mine.zip\n");
        assert_eq!(
            extract_asset_sha256_from_notes(&notes, "mine.zip").as_deref(),
            Some(b.as_str())
        );
        assert_eq!(extract_asset_sha256_from_notes(&notes, "third.zip"), None);

        // A bare digest is accepted only when unambiguous.
        assert_eq!(
            extract_asset_sha256_from_notes(&format!("sha256:{a}"), "x.zip").as_deref(),
            Some(a.as_str())
        );
        assert_eq!(
            extract_asset_sha256_from_notes(&format!("sha256:{a}\nsha256:{b}"), "x.zip"),
            None
        );
    }

    #[test]
    fn config_save_reports_failures_and_round_trips() {
        let dir = crate::test_support::TempDir::new();
        let mut config = UpdateConfig::default();
        config.channel = UpdateChannel::Prerelease;
        config.save(dir.path()).unwrap();
        assert_eq!(
            UpdateConfig::load_or_default(dir.path()).channel,
            UpdateChannel::Prerelease
        );

        let file_as_root = dir.path().join("occupied");
        std::fs::write(&file_as_root, b"x").unwrap();
        assert!(config.save(&file_as_root).is_err());
    }
}
