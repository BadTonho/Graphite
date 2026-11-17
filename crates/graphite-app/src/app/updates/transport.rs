//! GitHub Releases update transport.

use super::state::{
    extract_asset_sha256_from_notes, ReleaseAsset, ReleaseInfo, SemVer, UpdateChannel,
};
use reqwest::blocking::{Client, Response};
use ring::digest::{Context, SHA256};
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

/// Error text returned by [`UpdateTransport::download_to_file`] when the caller aborted.
pub(crate) const DOWNLOAD_CANCELLED: &str = "Update download cancelled";
/// Hard cap on update package size (protects disk and memory from hostile servers).
const MAX_PACKAGE_BYTES: u64 = 1 << 30;

pub(crate) trait UpdateTransport: Send + Sync {
    fn fetch_latest_release(
        &self,
        repo: &str,
        channel: UpdateChannel,
    ) -> Result<ReleaseInfo, String>;
    /// Downloads a small text document (signature or checksum file).
    fn download_text(&self, url: &str, max_bytes: usize) -> Result<String, String>;
    /// Streams `url` into `dest` and returns the lowercase SHA-256 of the written bytes.
    /// `progress(downloaded, total)` returns `false` to abort with [`DOWNLOAD_CANCELLED`].
    fn download_to_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<String, String>;
}

/// Copies `reader` into `dest` in chunks, hashing on the fly and reporting progress.
/// `dest` is removed on any failure.
pub(crate) fn write_stream(
    mut reader: impl Read,
    total: Option<u64>,
    dest: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<String, String> {
    let result = (|| {
        let mut file = std::fs::File::create(dest)
            .map_err(|e| format!("Failed creating {}: {e}", dest.display()))?;
        let mut ctx = Context::new(&SHA256);
        let mut buf = vec![0u8; 64 * 1024];
        let mut written: u64 = 0;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("Failed reading update bytes: {e}"))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > MAX_PACKAGE_BYTES {
                return Err("Update package exceeds the maximum allowed size".to_owned());
            }
            ctx.update(&buf[..n]);
            file.write_all(&buf[..n])
                .map_err(|e| format!("Failed writing update package: {e}"))?;
            if !progress(written, total) {
                return Err(DOWNLOAD_CANCELLED.to_owned());
            }
        }
        file.flush()
            .map_err(|e| format!("Failed writing update package: {e}"))?;
        if written == 0 {
            return Err("Downloaded update package is empty".to_owned());
        }
        if let Some(expected) = total {
            if expected != written {
                return Err(format!(
                    "Update download was truncated ({written} of {expected} bytes)"
                ));
            }
        }
        Ok(ctx
            .finish()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

pub(crate) struct GitHubReleasesTransport {
    api: Client,
    download: Client,
}

impl GitHubReleasesTransport {
    pub(crate) fn new() -> Result<Self, String> {
        let agent = concat!("Graphite-Editor/", env!("CARGO_PKG_VERSION"));
        let build = |builder: reqwest::blocking::ClientBuilder| {
            builder
                .user_agent(agent)
                .build()
                .map_err(|e| format!("Failed to initialize HTTP client: {e}"))
        };
        // API calls are small: a short total timeout is fine. A package download can
        // legitimately take minutes on a slow link, so it gets a generous total bound (which
        // also guarantees a stalled connection eventually fails) plus a connect timeout.
        let api = build(Client::builder().timeout(Duration::from_secs(20)))?;
        let download = build(
            Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(30 * 60)),
        )?;
        Ok(Self { api, download })
    }

    fn get(&self, client: &Client, url: &str) -> Result<Response, String> {
        if !url.starts_with("https://") {
            return Err(format!("Refusing non-HTTPS update URL: {url}"));
        }
        let resp = client
            .get(url)
            .send()
            .map_err(|e| format!("Update request failed: {e}"))?;
        if resp.status().is_success() {
            Ok(resp)
        } else {
            Err(http_error(&resp))
        }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, String> {
        if !url.starts_with("https://") {
            return Err(format!("Refusing non-HTTPS update URL: {url}"));
        }
        let resp = self
            .api
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| format!("Update check request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(http_error(&resp));
        }
        resp.json()
            .map_err(|e| format!("Invalid GitHub Release JSON: {e}"))
    }
}

/// Builds a human-readable error for a failed GitHub response, recognising rate limiting.
fn http_error(resp: &Response) -> String {
    let status = resp.status();
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    rate_limit_message(
        status.as_u16(),
        header("x-ratelimit-remaining").as_deref(),
        header("x-ratelimit-reset").as_deref(),
        header("retry-after").as_deref(),
    )
    .unwrap_or_else(|| format!("GitHub returned HTTP {status}"))
}

pub(crate) fn rate_limit_message(
    status: u16,
    remaining: Option<&str>,
    reset_unix: Option<&str>,
    retry_after_secs: Option<&str>,
) -> Option<String> {
    let limited =
        status == 429 || (status == 403 && (remaining == Some("0") || retry_after_secs.is_some()));
    if !limited {
        return None;
    }
    let when = if let Some(secs) = retry_after_secs.and_then(|s| s.trim().parse::<u64>().ok()) {
        format!("retry in about {secs}s")
    } else if let Some(reset) = reset_unix.and_then(|s| s.trim().parse::<u64>().ok()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!(
            "retry in about {} minute(s)",
            reset.saturating_sub(now) / 60 + 1
        )
    } else {
        "try again later".to_owned()
    };
    Some(format!("GitHub API rate limit exceeded; {when}"))
}
#[derive(Deserialize)]
struct GitHubReleasePayload {
    tag_name: String,
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    body: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GitHubAssetPayload>,
}

#[derive(Deserialize)]
struct GitHubAssetPayload {
    name: String,
    browser_download_url: String,
}

fn platform_asset_hint() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn arch_tokens(arch: &str) -> &'static [&'static str] {
    match arch {
        "x86_64" => &["x64", "x86_64", "amd64"],
        "aarch64" => &["arm64", "aarch64"],
        _ => &[],
    }
}

const ALL_ARCH_TOKENS: &[&str] = &["x64", "x86_64", "amd64", "arm64", "aarch64"];
const METADATA_SUFFIXES: &[&str] = &[".sha256", ".sig", ".asc", ".txt", ".md", ".json", ".sbom"];

fn is_metadata_asset(lower_name: &str) -> bool {
    METADATA_SUFFIXES.iter().any(|s| lower_name.ends_with(s))
}

// The installer swaps only the main executable. Native packages, AppImages and tarballs
// need their own installation flow and must never be mistaken for an update payload.
fn is_installable_update(name: &str, os: &str) -> bool {
    name.ends_with(".zip")
        || (os == "windows"
            && name.ends_with(".exe")
            && !name.contains("setup")
            && !name.contains("installer"))
        || name.ends_with(".bin")
        || ALL_ARCH_TOKENS
            .iter()
            .any(|arch| name.ends_with(&format!("-{arch}")))
}

/// Picks the package for `os`/`arch`. There is deliberately no fallback to an asset built for
/// another platform: installing the wrong binary would break the editor.
fn select_asset_for(payload: &GitHubReleasePayload, os: &str, arch: &str) -> Option<ReleaseAsset> {
    let ours = arch_tokens(arch);
    let chosen = payload
        .assets
        .iter()
        .filter_map(|a| {
            let lower = a.name.to_ascii_lowercase();
            if is_metadata_asset(&lower)
                || !lower.contains(os)
                || !is_installable_update(&lower, os)
            {
                return None;
            }
            let score = if ours.iter().any(|t| lower.contains(t)) {
                2
            } else if ALL_ARCH_TOKENS.iter().any(|t| lower.contains(t)) {
                return None;
            } else {
                1
            };
            let dedicated_update = lower.starts_with("graphite-update-") && lower.ends_with(".zip");
            Some(((score, dedicated_update), a))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, a)| a)?;
    let companion = |suffix: &str| {
        let wanted = format!("{}{suffix}", chosen.name);
        payload
            .assets
            .iter()
            .find(|a| a.name == wanted)
            .map(|a| a.browser_download_url.clone())
    };
    Some(ReleaseAsset {
        name: chosen.name.clone(),
        download_url: chosen.browser_download_url.clone(),
        sha256: extract_asset_sha256_from_notes(&payload.body, &chosen.name),
        sha256_url: companion(".sha256"),
        signature_url: companion(".sig"),
    })
}

fn select_asset(payload: &GitHubReleasePayload) -> Option<ReleaseAsset> {
    select_asset_for(payload, platform_asset_hint(), std::env::consts::ARCH)
}

fn into_release_info(payload: GitHubReleasePayload) -> ReleaseInfo {
    let version = payload
        .tag_name
        .trim()
        .strip_prefix('v')
        .unwrap_or(payload.tag_name.trim())
        .to_owned();
    let asset = select_asset(&payload);
    ReleaseInfo {
        tag_name: payload.tag_name,
        version,
        prerelease: payload.prerelease,
        notes: payload.body,
        html_url: payload.html_url,
        asset,
    }
}

/// Newest published (non-draft) release by semantic version; releases whose tag is not a
/// valid version are ignored.
fn newest_published(payloads: Vec<GitHubReleasePayload>) -> Option<GitHubReleasePayload> {
    payloads
        .into_iter()
        .filter(|p| !p.draft)
        .filter_map(|p| SemVer::parse(&p.tag_name).map(|v| (v, p)))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, p)| p)
}

impl UpdateTransport for GitHubReleasesTransport {
    fn fetch_latest_release(
        &self,
        repo: &str,
        channel: UpdateChannel,
    ) -> Result<ReleaseInfo, String> {
        let base = "https://api.github.com/repos";
        match channel {
            UpdateChannel::Stable => {
                let payload: GitHubReleasePayload =
                    self.get_json(&format!("{base}/{repo}/releases/latest"))?;
                Ok(into_release_info(payload))
            }
            UpdateChannel::Prerelease => {
                let payloads: Vec<GitHubReleasePayload> =
                    self.get_json(&format!("{base}/{repo}/releases?per_page=30"))?;
                newest_published(payloads)
                    .map(into_release_info)
                    .ok_or_else(|| "No published releases found".to_owned())
            }
        }
    }

    fn download_text(&self, url: &str, max_bytes: usize) -> Result<String, String> {
        let resp = self.get(&self.api, url)?;
        let mut buf = Vec::new();
        resp.take(max_bytes as u64 + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed reading {url}: {e}"))?;
        if buf.len() > max_bytes {
            return Err("Update metadata file is unexpectedly large".into());
        }
        String::from_utf8(buf).map_err(|_| "Update metadata file is not valid UTF-8".to_owned())
    }

    fn download_to_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<String, String> {
        let resp = self.get(&self.download, url)?;
        let total = resp.content_length();
        write_stream(resp, total, dest, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(tag: &str, draft: bool, assets: &[&str], body: &str) -> GitHubReleasePayload {
        GitHubReleasePayload {
            tag_name: tag.into(),
            prerelease: false,
            draft,
            body: body.into(),
            html_url: String::new(),
            assets: assets
                .iter()
                .map(|n| GitHubAssetPayload {
                    name: (*n).into(),
                    browser_download_url: format!("https://example.invalid/{n}"),
                })
                .collect(),
        }
    }

    #[test]
    fn asset_selection_matches_platform_and_arch_without_cross_platform_fallback() {
        let p = payload(
            "v1.0.0",
            false,
            &[
                "graphite-linux-x64.zip",
                "graphite-windows-arm64.zip",
                "graphite-windows-x64.zip",
                "graphite-windows-x64.zip.sig",
                "graphite-windows-x64.zip.sha256",
                "source.zip",
            ],
            "",
        );
        let win = select_asset_for(&p, "windows", "x86_64").unwrap();
        assert_eq!(win.name, "graphite-windows-x64.zip");
        assert_eq!(
            win.signature_url.as_deref(),
            Some("https://example.invalid/graphite-windows-x64.zip.sig")
        );
        assert!(win.sha256_url.is_some());
        assert_eq!(
            select_asset_for(&p, "windows", "aarch64").unwrap().name,
            "graphite-windows-arm64.zip"
        );
        assert_eq!(
            select_asset_for(&p, "linux", "x86_64").unwrap().name,
            "graphite-linux-x64.zip"
        );
        // Wrong arch only, other OS only, or just source archives: nothing is selected.
        assert!(select_asset_for(&p, "linux", "aarch64").is_none());
        assert!(select_asset_for(&p, "macos", "x86_64").is_none());
        assert!(select_asset_for(
            &payload("v1.0.0", false, &["source.zip"], ""),
            "windows",
            "x86_64"
        )
        .is_none());
    }

    #[test]
    fn release_package_formats_never_override_the_dedicated_update_archive() {
        let assets = [
            "graphite-update-v1.0.0-linux-x64.zip",
            "graphite-v1.0.0-linux-x64.tar.gz",
            "graphite-v1.0.0-linux-x64.AppImage",
            "graphite-v1.0.0-linux-debian12-x64.deb",
            "graphite-v1.0.0-linux-fedora44-x64.rpm",
            "graphite-v1.0.0-linux-arch-x64.pkg.tar.zst",
            "graphite-update-v1.0.0-windows-x64.zip",
            "graphite-v1.0.0-windows-x64.zip",
            "graphite-v1.0.0-windows-x64-setup.exe",
        ];
        for ordered in [assets.to_vec(), assets.into_iter().rev().collect()] {
            let release = payload("v1.0.0", false, &ordered, "");
            for os in ["windows", "linux"] {
                assert_eq!(
                    select_asset_for(&release, os, "x86_64").unwrap().name,
                    format!("graphite-update-v1.0.0-{os}-x64.zip")
                );
            }
        }
        let native_only = payload(
            "v1.0.0",
            false,
            &[
                "graphite-linux-x64.AppImage",
                "graphite-linux-x64.tar.gz",
                "graphite-linux-x64.deb",
                "graphite-windows-x64-setup.exe",
                "graphite-windows-x64-installer.exe",
            ],
            "",
        );
        assert!(select_asset_for(&native_only, "linux", "x86_64").is_none());
        assert!(select_asset_for(&native_only, "windows", "x86_64").is_none());
    }

    #[test]
    fn prerelease_channel_picks_newest_version_and_skips_drafts_and_bad_tags() {
        let picked = newest_published(vec![
            payload("v0.9.0-rc.1", false, &[], ""),
            payload("v1.2.0", true, &[], ""),
            payload("nightly", false, &[], ""),
            payload("v1.1.0", false, &[], ""),
            payload("v1.1.1-beta.1", false, &[], ""),
        ])
        .unwrap();
        assert_eq!(picked.tag_name, "v1.1.1-beta.1");
        assert!(newest_published(vec![payload("v1.0.0", true, &[], "")]).is_none());
    }

    #[test]
    fn rate_limit_responses_are_reported_clearly() {
        let msg = rate_limit_message(403, Some("0"), Some("0"), None).unwrap();
        assert!(msg.contains("rate limit"));
        assert!(rate_limit_message(429, None, None, Some("90"))
            .unwrap()
            .contains("90s"));
        assert!(rate_limit_message(403, Some("12"), None, None).is_none());
        assert!(rate_limit_message(500, None, None, None).is_none());
    }

    #[test]
    fn write_stream_hashes_reports_progress_and_supports_cancel() {
        let dir = crate::test_support::TempDir::new();
        let dest = dir.path().join("pkg.bin");
        let data = vec![7u8; 200_000];
        let mut last = 0;
        let sha = write_stream(&data[..], Some(data.len() as u64), &dest, &mut |done, _| {
            last = done;
            true
        })
        .unwrap();
        assert_eq!(last, data.len() as u64);
        assert_eq!(sha, super::super::state::sha256_hex(&data));
        assert_eq!(std::fs::read(&dest).unwrap(), data);

        let err = write_stream(&data[..], None, &dest, &mut |_, _| false).unwrap_err();
        assert_eq!(err, DOWNLOAD_CANCELLED);
        assert!(!dest.exists(), "partial file must be removed on cancel");

        let err = write_stream(&data[..], Some(1), &dest, &mut |_, _| true).unwrap_err();
        assert!(err.contains("truncated"));
        assert!(write_stream(&b""[..], None, &dest, &mut |_, _| true).is_err());
    }
}
