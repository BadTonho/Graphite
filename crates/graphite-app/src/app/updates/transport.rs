//! GitHub Releases update transport.

use super::state::{extract_asset_sha256_from_notes, ReleaseAsset, ReleaseInfo, UpdateChannel};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

pub(crate) trait UpdateTransport: Send + Sync {
    fn fetch_latest_release(
        &self,
        repo: &str,
        channel: UpdateChannel,
    ) -> Result<ReleaseInfo, String>;
    fn download_asset(&self, url: &str) -> Result<Vec<u8>, String>;
}

pub(crate) struct GitHubReleasesTransport {
    client: Client,
}

impl GitHubReleasesTransport {
    pub(crate) fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("Graphite-Editor/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("Failed to initialize HTTP client: {e}"))?;
        Ok(Self { client })
    }
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

fn select_asset(payload: &GitHubReleasePayload) -> Option<ReleaseAsset> {
    let hint = platform_asset_hint();
    let chosen = payload
        .assets
        .iter()
        .find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.contains(hint) && !lower.ends_with(".sha256")
        })
        .or_else(|| {
            payload
                .assets
                .iter()
                .find(|a| !a.name.to_ascii_lowercase().ends_with(".sha256"))
        })?;
    let sha256 = extract_asset_sha256_from_notes(&payload.body, &chosen.name);
    Some(ReleaseAsset {
        name: chosen.name.clone(),
        download_url: chosen.browser_download_url.clone(),
        sha256,
    })
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

impl UpdateTransport for GitHubReleasesTransport {
    fn fetch_latest_release(
        &self,
        repo: &str,
        channel: UpdateChannel,
    ) -> Result<ReleaseInfo, String> {
        let base = "https://api.github.com/repos";
        match channel {
            UpdateChannel::Stable => {
                let url = format!("{base}/{repo}/releases/latest");
                let resp = self
                    .client
                    .get(&url)
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .map_err(|e| format!("Update check request failed: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("GitHub Releases returned HTTP {}", resp.status()));
                }
                let payload: GitHubReleasePayload = resp
                    .json()
                    .map_err(|e| format!("Invalid GitHub Release JSON: {e}"))?;
                Ok(into_release_info(payload))
            }
            UpdateChannel::Prerelease => {
                let url = format!("{base}/{repo}/releases?per_page=10");
                let resp = self
                    .client
                    .get(&url)
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .map_err(|e| format!("Update check request failed: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("GitHub Releases returned HTTP {}", resp.status()));
                }
                let payloads: Vec<GitHubReleasePayload> = resp
                    .json()
                    .map_err(|e| format!("Invalid GitHub Releases JSON: {e}"))?;
                payloads
                    .into_iter()
                    .find(|p| !p.draft)
                    .map(into_release_info)
                    .ok_or_else(|| "No published releases found".to_owned())
            }
        }
    }

    fn download_asset(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = self
            .client
            .get(url)
            .send()
            .map_err(|e| format!("Update download failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Download returned HTTP {}", resp.status()));
        }
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed reading update bytes: {e}"))
    }
}
