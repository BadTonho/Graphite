use crate::{Result, MAX_DOWNLOAD};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Deserialize)]
pub struct Repository {
    pub full_name: String,
    pub description: Option<String>,
    pub stargazers_count: u64,
    pub html_url: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}
#[derive(Deserialize)]
struct Search {
    items: Vec<Repository>,
}
#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
    draft: bool,
    prerelease: bool,
}
pub struct Catalog {
    client: Client,
    base: String,
    cache: HashMap<(String, u32), (Instant, Vec<Repository>)>,
    blocked_until: Option<Instant>,
}
impl Catalog {
    pub fn new() -> Result<Self> {
        Self::with_base("https://api.github.com")
    }
    pub fn with_base(base: &str) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent("Graphite-Plugins/0.1")
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())?,
            base: base.into(),
            cache: HashMap::new(),
            blocked_until: None,
        })
    }
    fn get(&mut self, url: &str) -> Result<reqwest::blocking::Response> {
        if self.blocked_until.is_some_and(|t| Instant::now() < t) {
            return Err("GitHub rate limit reached; retry later".into());
        }
        let response = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        if response.status().as_u16() == 429
            || response
                .headers()
                .get("x-ratelimit-remaining")
                .is_some_and(|h| h == "0")
            || response.status().as_u16() == 403
        {
            let retry = response
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let reset = response
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|t| {
                    t.saturating_sub(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    )
                });
            self.blocked_until = Some(
                Instant::now() + Duration::from_secs(retry.or(reset).unwrap_or(60).min(86400)),
            );
        }
        response.error_for_status().map_err(|e| e.to_string())
    }
    pub fn search(&mut self, query: &str, page: u32) -> Result<Vec<Repository>> {
        if !(1..=34).contains(&page) {
            return Err("Invalid catalog page".into());
        }
        let key = (query.to_owned(), page);
        if let Some((time, results)) = self.cache.get(&key) {
            if time.elapsed() < Duration::from_secs(60) {
                return Ok(results.clone());
            }
        }
        // Encode user input as a quoted search term, never as additional GitHub qualifiers.
        let words = query.replace(['"', '\\'], " ");
        let q = if words.trim().is_empty() {
            "topic:graphite-plugin".to_owned()
        } else {
            format!("topic:graphite-plugin \"{words}\"")
        };
        let mut url = reqwest::Url::parse(&format!("{}/search/repositories", self.base))
            .map_err(|e| e.to_string())?;
        url.query_pairs_mut()
            .append_pair("q", &q)
            .append_pair("page", &page.to_string())
            .append_pair("per_page", "30")
            .append_pair("sort", "stars");
        let results: Search = self.get(url.as_str())?.json().map_err(|e| e.to_string())?;
        if self.cache.len() >= 64 {
            self.cache.clear();
        }
        self.cache
            .insert(key, (Instant::now(), results.items.clone()));
        Ok(results.items)
    }
    pub fn assets(&mut self, repository: &str) -> Result<Vec<Asset>> {
        let parts: Vec<_> = repository.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|p| {
                p.is_empty()
                    || !p
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            })
        {
            return Err("Invalid GitHub repository".into());
        }
        let response = self.get(&format!("{}/repos/{repository}/releases/latest", self.base))?;
        let release: Release = response.json().map_err(|e| e.to_string())?;
        if release.draft || release.prerelease {
            return Err("No stable release available".into());
        }
        Ok(release
            .assets
            .into_iter()
            .filter(|a| a.name.ends_with(".graphite-plugin") || a.name.ends_with(".vsix"))
            .collect())
    }
    pub fn download(&mut self, asset: &Asset, destination: &Path) -> Result<()> {
        let url = reqwest::Url::parse(&asset.browser_download_url).map_err(|e| e.to_string())?;
        if self.base.starts_with("https:")
            && (url.scheme() != "https" || url.host_str() != Some("github.com"))
        {
            return Err("Expected GitHub HTTPS release asset".into());
        }
        if asset.size > MAX_DOWNLOAD {
            return Err("Asset exceeds 100 MiB".into());
        }
        let response = self.get(url.as_str())?;
        if response.content_length().is_some_and(|n| n > MAX_DOWNLOAD) {
            return Err("Download exceeds 100 MiB".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let count = std::io::copy(&mut response.take(MAX_DOWNLOAD + 1), &mut file)
                .map_err(|e| e.to_string())?;
            if count > MAX_DOWNLOAD || count != asset.size {
                return Err("Incomplete or oversized download".into());
            }
            file.sync_all().map_err(|e| e.to_string())
        })();
        drop(file);
        if result.is_err() {
            let _ = std::fs::remove_file(destination);
        }
        result
    }
}
