use crate::{download_limit, is_vsix, size_limit_label, Result};
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
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub license: Option<License>,
    #[serde(default)]
    pub open_issues_count: u64,
    #[serde(default)]
    pub default_branch: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct License {
    pub name: String,
    #[serde(default)]
    pub spdx_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    #[serde(default)]
    pub download_count: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub published_at: Option<String>,
    pub assets: Vec<Asset>,
    pub downloads: u64,
    pub size: u64,
}
#[derive(Clone, Debug)]
pub struct RepositoryDetails {
    pub repository: Repository,
    pub readme: Option<String>,
    pub readme_html: Option<String>,
    pub release: Option<ReleaseInfo>,
}
#[derive(Deserialize)]
struct Search {
    items: Vec<Repository>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    assets: Vec<Asset>,
    draft: bool,
    prerelease: bool,
}
#[derive(Deserialize)]
struct Readme {
    content: String,
    encoding: String,
    #[serde(default)]
    path: Option<String>,
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
        let url = reqwest::Url::parse(base).map_err(|error| error.to_string())?;
        let is_loopback = url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        });
        if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback) {
            return Err(
                "GitHub catalog must use HTTPS (HTTP is allowed for loopback fixtures)".into(),
            );
        }
        let mut builder = Client::builder()
            .user_agent("Graphite-Plugins/0.1")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30));
        if is_loopback {
            builder = builder.no_proxy();
        }
        Ok(Self {
            client: builder.build().map_err(|e| e.to_string())?,
            base: base.into(),
            cache: HashMap::new(),
            blocked_until: None,
        })
    }
    fn get_raw(&mut self, url: &str) -> Result<reqwest::blocking::Response> {
        self.get_raw_with_timeout(url, Duration::from_secs(30))
    }
    fn get_raw_with_timeout(
        &mut self,
        url: &str,
        timeout: Duration,
    ) -> Result<reqwest::blocking::Response> {
        if self.blocked_until.is_some_and(|t| Instant::now() < t) {
            return Err("GitHub rate limit reached; retry later".into());
        }
        let response = self
            .client
            .get(url)
            .timeout(timeout)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|error| format!("{:#}", error.without_url()))?;
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
        Ok(response)
    }
    fn get(&mut self, url: &str) -> Result<reqwest::blocking::Response> {
        self.get_raw(url)?
            .error_for_status()
            .map_err(|error| error.to_string())
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
        let response =
            self.get_raw(&format!("{}/repos/{repository}/releases/latest", self.base))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(format!("No stable GitHub release found for {repository}"));
        }
        let response = response
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let release: Release = response.json().map_err(|e| e.to_string())?;
        if release.draft || release.prerelease {
            return Err("No stable release available".into());
        }
        Ok(release
            .assets
            .into_iter()
            .filter(|a| a.name.ends_with(".graphite-plugin") || is_vsix(Path::new(&a.name)))
            .collect())
    }

    pub fn details(&mut self, repository: &str) -> Result<RepositoryDetails> {
        validate_repository(repository)?;
        let base = self.base.clone();
        let repository_info: Repository = self
            .get(&format!("{base}/repos/{repository}"))?
            .json()
            .map_err(|error| error.to_string())?;
        let mut readme_path = None;
        let readme = match self.get_raw(&format!("{base}/repos/{repository}/readme"))? {
            response if response.status() == reqwest::StatusCode::NOT_FOUND => None,
            response => {
                let response = response
                    .error_for_status()
                    .map_err(|error| error.to_string())?;
                let readme: Readme = response.json().map_err(|error| error.to_string())?;
                if readme.encoding != "base64" {
                    return Err("GitHub returned an unsupported README encoding".into());
                }
                readme_path = readme.path;
                Some(decode_base64(&readme.content)?)
            }
        };
        let readme_html = if let Some(readme) = &readme {
            // The README endpoint's HTML media type uses GitHub's own Markup
            // renderer and sanitizer, matching how README content is rendered
            // on github.com more closely than the generic Markdown endpoint.
            let rendered_readme = match self
                .client
                .get(format!("{base}/repos/{repository}/readme"))
                .header("Accept", "application/vnd.github.html+json")
                .send()
            {
                Ok(response) if response.status().is_success() => response
                    .text()
                    .ok()
                    .map(decode_github_html_response)
                    .filter(|html| {
                        !html.trim().is_empty()
                            && html.len() <= 1_500_000
                            && rendered_html_matches_readme(html, readme)
                    }),
                _ => None,
            };
            rendered_readme.or_else(|| {
                // Keep the general GFM endpoint as a fallback for API proxies
                // that do not honor GitHub's custom README media type.
                let branch = repository_info.default_branch.as_deref().unwrap_or("HEAD");
                let readme_path = readme_path.as_deref().unwrap_or("README.md");
                let markdown =
                    rewrite_relative_markdown_images(readme, repository, branch, readme_path);
                let markdown =
                    rewrite_relative_markdown_links(&markdown, repository, branch, readme_path);
                self.client
                    .post(format!("{base}/markdown"))
                    .header("Accept", "text/html")
                    .json(&serde_json::json!({
                        "text": markdown,
                        "mode": "gfm",
                        "context": repository,
                    }))
                    .send()
                    .ok()
                    .and_then(|response| response.error_for_status().ok())
                    .and_then(|response| response.text().ok())
                    .filter(|html| !html.is_empty() && html.len() <= 1_500_000)
            })
        } else {
            None
        };
        let release = match self.get_raw(&format!("{base}/repos/{repository}/releases/latest"))? {
            response if response.status() == reqwest::StatusCode::NOT_FOUND => None,
            response => {
                let response = response
                    .error_for_status()
                    .map_err(|error| error.to_string())?;
                let release: Release = response.json().map_err(|error| error.to_string())?;
                if release.draft || release.prerelease {
                    None
                } else {
                    let assets = release
                        .assets
                        .into_iter()
                        .filter(|asset| {
                            asset.name.ends_with(".graphite-plugin")
                                || is_vsix(Path::new(&asset.name))
                        })
                        .collect::<Vec<_>>();
                    let downloads = assets.iter().map(|asset| asset.download_count).sum();
                    let size = assets.iter().map(|asset| asset.size).sum();
                    Some(ReleaseInfo {
                        tag_name: release.tag_name,
                        name: release.name,
                        published_at: release.published_at,
                        assets,
                        downloads,
                        size,
                    })
                }
            }
        };
        Ok(RepositoryDetails {
            repository: repository_info,
            readme,
            readme_html,
            release,
        })
    }
    pub fn download(&mut self, asset: &Asset, destination: &Path) -> Result<()> {
        let url = reqwest::Url::parse(&asset.browser_download_url).map_err(|e| e.to_string())?;
        if self.base.starts_with("https:")
            && (url.scheme() != "https" || url.host_str() != Some("github.com"))
        {
            return Err("Expected GitHub HTTPS release asset".into());
        }
        let vsix = is_vsix(Path::new(&asset.name));
        let limit = download_limit(vsix);
        let label = size_limit_label(vsix);
        if asset.size > limit {
            return Err(format!("Asset exceeds {label}"));
        }
        let timeout = Duration::from_secs(if vsix { 30 * 60 } else { 30 });
        let response = self
            .get_raw_with_timeout(url.as_str(), timeout)?
            .error_for_status()
            .map_err(|error| error.without_url().to_string())?;
        if self.base.starts_with("https:") {
            validate_release_asset_final_url(response.url())?;
        }
        if response.content_length().is_some_and(|n| n > limit) {
            return Err(format!("Download exceeds {label}"));
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let count = std::io::copy(&mut response.take(limit + 1), &mut file)
                .map_err(|e| e.to_string())?;
            if count > limit || count != asset.size {
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

fn validate_repository(repository: &str) -> Result<()> {
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        })
    {
        return Err("Invalid GitHub repository".into());
    }
    Ok(())
}

fn decode_base64(encoded: &str) -> Result<String> {
    let mut bytes = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut accumulator = 0u32;
    let mut bits = 0u8;
    for byte in encoded.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("GitHub returned invalid README data".into()),
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((accumulator >> bits) as u8);
        }
    }
    String::from_utf8(bytes).map_err(|_| "GitHub README is not UTF-8".into())
}

fn decode_github_html_response(body: String) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) else {
        return body;
    };
    if let Some(content) = value
        .get("content")
        .or_else(|| value.get("html"))
        .and_then(serde_json::Value::as_str)
    {
        return content.to_owned();
    }
    if let Some(html) = value.as_str() {
        return html.to_owned();
    }
    body
}

fn rendered_html_matches_readme(html: &str, markdown: &str) -> bool {
    markdown
        .split_whitespace()
        .filter_map(|word| {
            let word = word.trim_matches(|character: char| !character.is_alphanumeric());
            (word.chars().count() >= 4 && !word.starts_with("http") && !word.starts_with("www"))
                .then_some(word)
        })
        .take(8)
        .any(|word| html.contains(word))
}

fn rewrite_relative_markdown_images(
    markdown: &str,
    repository: &str,
    branch: &str,
    readme_path: &str,
) -> String {
    let Some((owner, name)) = repository.split_once('/') else {
        return markdown.to_owned();
    };
    let Ok(repo_root) = reqwest::Url::parse(&format!(
        "https://raw.githubusercontent.com/{owner}/{name}/{branch}/"
    )) else {
        return markdown.to_owned();
    };
    let mut branch_root = repo_root.clone();
    let directory = readme_path.rsplit_once('/').map_or("", |(path, _)| path);
    if !directory.is_empty() {
        let Ok(directory_url) = branch_root.join(&format!("{directory}/")) else {
            return markdown.to_owned();
        };
        branch_root = directory_url;
    }
    let mut output = String::with_capacity(markdown.len());
    let mut cursor = 0;
    while let Some(relative_start) = markdown[cursor..].find("![") {
        let start = cursor + relative_start;
        let Some(alt_end) = matching_markdown_delimiter(markdown, start + 1, '[', ']') else {
            break;
        };
        if markdown.as_bytes().get(alt_end + 1) != Some(&b'(') {
            output.push_str(&markdown[cursor..start + 2]);
            cursor = start + 2;
            continue;
        }
        let mut target_start = alt_end + 2;
        while markdown
            .as_bytes()
            .get(target_start)
            .is_some_and(u8::is_ascii_whitespace)
        {
            target_start += 1;
        }
        let angled = markdown.as_bytes().get(target_start) == Some(&b'<');
        if angled {
            target_start += 1;
        }
        let Some(target_end) = markdown_image_target_end(markdown, target_start, angled) else {
            output.push_str(&markdown[cursor..start + 2]);
            cursor = start + 2;
            continue;
        };
        output.push_str(&markdown[cursor..target_start]);
        let target = &markdown[target_start..target_end];
        if let Some(resolved) = resolve_markdown_image_target(target, &branch_root, &repo_root) {
            output.push_str(&resolved);
        }
        output.push_str(&markdown[target_end..]);
        cursor = target_end;
    }
    output.push_str(&markdown[cursor..]);
    output
}

fn rewrite_relative_markdown_links(
    markdown: &str,
    repository: &str,
    branch: &str,
    readme_path: &str,
) -> String {
    let Some((owner, name)) = repository.split_once('/') else {
        return markdown.to_owned();
    };
    let Ok(repo_root) =
        reqwest::Url::parse(&format!("https://github.com/{owner}/{name}/blob/{branch}/"))
    else {
        return markdown.to_owned();
    };
    let mut directory_base = repo_root.clone();
    let directory = readme_path.rsplit_once('/').map_or("", |(path, _)| path);
    if !directory.is_empty() {
        let Ok(url) = directory_base.join(&format!("{directory}/")) else {
            return markdown.to_owned();
        };
        directory_base = url;
    }
    let mut output = String::with_capacity(markdown.len());
    let mut cursor = 0;
    while let Some(relative_start) = markdown[cursor..].find('[') {
        let start = cursor + relative_start;
        if start > 0 && markdown.as_bytes()[start - 1] == b'!' {
            output.push_str(&markdown[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }
        let Some(label_end) = matching_markdown_delimiter(markdown, start, '[', ']') else {
            break;
        };
        if markdown.as_bytes().get(label_end + 1) != Some(&b'(') {
            output.push_str(&markdown[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }
        let mut target_start = label_end + 2;
        while markdown
            .as_bytes()
            .get(target_start)
            .is_some_and(u8::is_ascii_whitespace)
        {
            target_start += 1;
        }
        let angled = markdown.as_bytes().get(target_start) == Some(&b'<');
        if angled {
            target_start += 1;
        }
        let Some(target_end) = markdown_image_target_end(markdown, target_start, angled) else {
            output.push_str(&markdown[cursor..start + 1]);
            cursor = start + 1;
            continue;
        };
        output.push_str(&markdown[cursor..target_start]);
        if let Some(target) = resolve_markdown_link_target(
            &markdown[target_start..target_end],
            &directory_base,
            &repo_root,
        ) {
            output.push_str(&target);
        }
        output.push_str(&markdown[target_end..]);
        cursor = target_end;
    }
    output.push_str(&markdown[cursor..]);
    output
}

fn matching_markdown_delimiter(
    input: &str,
    opening: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (offset, character) in input[opening..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(opening + offset);
            }
        }
    }
    None
}

fn markdown_image_target_end(input: &str, start: usize, angled: bool) -> Option<usize> {
    let mut escaped = false;
    let mut parentheses = 0usize;
    for (offset, character) in input[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if angled && character == '>' {
            return Some(start + offset);
        } else if !angled && character == '(' {
            parentheses += 1;
        } else if !angled && character == ')' {
            if parentheses == 0 {
                return Some(start + offset);
            }
            parentheses -= 1;
        } else if !angled && character.is_whitespace() {
            return Some(start + offset);
        }
    }
    None
}

fn resolve_markdown_image_target(
    target: &str,
    directory_base: &reqwest::Url,
    repo_root: &reqwest::Url,
) -> Option<String> {
    if let Ok(url) = reqwest::Url::parse(target) {
        return (url.scheme() == "https").then(|| url.to_string());
    }
    if target.is_empty() || target.starts_with("//") || target.starts_with('#') {
        return None;
    }
    let (base, relative) = if target.starts_with('/') {
        (repo_root, target.trim_start_matches('/'))
    } else {
        (directory_base, target)
    };
    let url = base.join(relative).ok()?;
    (url.scheme() == "https" && url.host_str() == Some("raw.githubusercontent.com"))
        .then(|| url.to_string())
}

fn resolve_markdown_link_target(
    target: &str,
    directory_base: &reqwest::Url,
    repo_root: &reqwest::Url,
) -> Option<String> {
    if target.starts_with('#') {
        return Some(target.to_owned());
    }
    if let Ok(url) = reqwest::Url::parse(target) {
        return (url.scheme() == "https").then(|| url.to_string());
    }
    if target.is_empty() || target.starts_with("//") {
        return None;
    }
    let (base, relative) = if target.starts_with('/') {
        (repo_root, target.trim_start_matches('/'))
    } else {
        (directory_base, target)
    };
    let url = base.join(relative).ok()?;
    (url.scheme() == "https" && url.host_str() == Some("github.com")).then(|| url.to_string())
}

fn validate_release_asset_final_url(url: &reqwest::Url) -> Result<()> {
    let Some(host) = url.host_str() else {
        return Err("GitHub redirected to a URL without a host".into());
    };
    let allowed_host = host == "github.com"
        || host.ends_with(".github.com")
        || host == "githubusercontent.com"
        || host.ends_with(".githubusercontent.com");
    if url.scheme() != "https" || !allowed_host {
        return Err("GitHub redirected to an insecure or untrusted download URL".into());
    }
    Ok(())
}

#[cfg(test)]
mod redirect_tests {
    use super::*;

    #[test]
    fn final_release_redirect_must_remain_https_on_github_domains() {
        for allowed in [
            "https://github.com/owner/repo/releases/download/v1/plugin.vsix",
            "https://release-assets.githubusercontent.com/download/plugin.vsix",
        ] {
            assert!(
                validate_release_asset_final_url(&reqwest::Url::parse(allowed).unwrap()).is_ok()
            );
        }
        for rejected in [
            "http://github.com/owner/repo/plugin.vsix",
            "https://github.com.evil.invalid/plugin.vsix",
            "https://attacker.invalid/plugin.vsix",
        ] {
            assert!(
                validate_release_asset_final_url(&reqwest::Url::parse(rejected).unwrap()).is_err()
            );
        }
    }

    #[test]
    fn custom_catalog_http_base_is_limited_to_loopback_fixtures() {
        assert!(Catalog::with_base("https://api.github.com").is_ok());
        assert!(Catalog::with_base("http://127.0.0.1:8000").is_ok());
        assert!(Catalog::with_base("http://example.com").is_err());
    }

    #[test]
    fn decodes_github_readme_content_as_utf8() {
        assert_eq!(
            decode_base64("IyBIZWxsbyBXb3JsZAo=").unwrap(),
            "# Hello World\n"
        );
        assert_eq!(decode_base64("Y2Fmw6k=").unwrap(), "café");
        assert!(decode_base64("%%%=").is_err());
    }

    #[test]
    fn decodes_html_representation_from_raw_or_json_github_response() {
        let html = "<h1>Plugin</h1>";
        assert_eq!(decode_github_html_response(html.into()), html);
        assert_eq!(decode_github_html_response(format!("\"{html}\"")), html);
        assert_eq!(
            decode_github_html_response(format!("{{\"content\":\"{html}\"}}")),
            html
        );
        assert_eq!(
            decode_github_html_response(format!("{{\"html\":\"{html}\"}}")),
            html
        );
    }

    #[test]
    fn rejects_empty_html_representations_that_do_not_contain_readme_content() {
        assert!(rendered_html_matches_readme(
            "<h1>⚡ Amarillo — Roblox Studio Bridge</h1>",
            "# ⚡ Amarillo — Roblox Studio Bridge"
        ));
        assert!(!rendered_html_matches_readme(
            "<!doctype html><html><body></body></html>",
            "# ⚡ Amarillo — Roblox Studio Bridge"
        ));
    }

    #[test]
    fn relative_readme_images_resolve_from_the_readme_directory_over_https() {
        let rendered_input = rewrite_relative_markdown_images(
            "![logo](images/logo.svg) ![root](/assets/root.png) ![remote](https://img.example/logo.png) ![unsafe](http://bad.invalid/logo.png) [guide](docs/guide.md)",
            "author/plugin",
            "main",
            "docs/README.md",
        );
        assert!(rendered_input.contains(
            "![logo](https://raw.githubusercontent.com/author/plugin/main/docs/images/logo.svg)"
        ));
        assert!(rendered_input.contains(
            "![root](https://raw.githubusercontent.com/author/plugin/main/assets/root.png)"
        ));
        assert!(rendered_input.contains("![remote](https://img.example/logo.png)"));
        assert!(rendered_input.contains("![unsafe]()"));
        assert!(rendered_input.contains("[guide](docs/guide.md)"));
    }

    #[test]
    fn relative_readme_links_resolve_to_the_repository_and_reject_insecure_schemes() {
        let links = rewrite_relative_markdown_links(
            "[guide](guides/start.md) [root](/LICENSE) [anchor](#usage) [external](https://example.test) [unsafe](javascript:alert(1)) ![logo](img/logo.png)",
            "author/plugin",
            "main",
            "docs/README.md",
        );
        assert!(links
            .contains("[guide](https://github.com/author/plugin/blob/main/docs/guides/start.md)"));
        assert!(links.contains("[root](https://github.com/author/plugin/blob/main/LICENSE)"));
        assert!(links.contains("[anchor](#usage)"));
        assert!(links.contains("[external](https://example.test/)"));
        assert!(links.contains("[unsafe]()"));
        assert!(links.contains("![logo](img/logo.png)"));
    }
}
