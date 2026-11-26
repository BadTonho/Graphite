//! Remote transport abstraction for User Data Sync and GitHub Gist implementation.

use super::state::SyncSnapshot;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) const SYNC_GIST_FILENAME: &str = "graphite-sync.json";
pub(crate) const SYNC_GIST_DESCRIPTION: &str = "Graphite User Data Sync";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteSyncBundle {
    pub remote_id: String,
    pub snapshot: SyncSnapshot,
}

pub(crate) trait SyncTransport: Send + Sync {
    fn fetch_snapshot(
        &self,
        token: &str,
        remote_id: Option<&str>,
    ) -> Result<Option<RemoteSyncBundle>, String>;

    fn push_snapshot(
        &self,
        token: &str,
        remote_id: Option<&str>,
        snapshot: &SyncSnapshot,
    ) -> Result<String, String>;

    fn delete_snapshot(&self, token: &str, remote_id: &str) -> Result<(), String>;
}

/// Private GitHub Gist backend for User Data Sync.
pub(crate) struct GitHubGistTransport {
    client: Client,
    api_base: String,
}

impl GitHubGistTransport {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .user_agent("Graphite-Editor")
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            api_base: "https://api.github.com".into(),
        })
    }

    #[cfg(test)]
    pub fn with_base_url(api_base: impl Into<String>) -> Result<Self, String> {
        let mut transport = Self::new()?;
        transport.api_base = api_base.into();
        Ok(transport)
    }

    fn get_gist(&self, token: &str, remote_id: &str) -> Result<Option<Value>, String> {
        let url = format!("{}/gists/{remote_id}", self.api_base);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("GitHub Gist fetch failed: HTTP {}", resp.status()));
        }
        resp.json::<Value>().map(Some).map_err(|e| e.to_string())
    }

    fn discover_remote_id(&self, token: &str) -> Result<Option<String>, String> {
        let url = format!("{}/gists?per_page=30", self.api_base);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!(
                "GitHub Gist discovery failed: HTTP {}",
                resp.status()
            ));
        }
        let list = resp.json::<Vec<Value>>().map_err(|e| e.to_string())?;
        for item in list {
            let has_file = item
                .get("files")
                .and_then(|f| f.get(SYNC_GIST_FILENAME))
                .is_some();
            if has_file {
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    return Ok(Some(id.to_owned()));
                }
            }
        }
        Ok(None)
    }
}

pub(crate) fn parse_gist_snapshot(gist: &Value) -> Result<Option<RemoteSyncBundle>, String> {
    let id = gist
        .get("id")
        .and_then(Value::as_str)
        .ok_or("GitHub Gist response missing id")?
        .to_owned();
    let Some(content) = gist
        .get("files")
        .and_then(|f| f.get(SYNC_GIST_FILENAME))
        .and_then(|file| file.get("content"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let snapshot: SyncSnapshot =
        serde_json::from_str(content).map_err(|e| format!("Invalid synced Gist payload: {e}"))?;
    Ok(Some(RemoteSyncBundle {
        remote_id: id,
        snapshot,
    }))
}

impl SyncTransport for GitHubGistTransport {
    fn fetch_snapshot(
        &self,
        token: &str,
        remote_id: Option<&str>,
    ) -> Result<Option<RemoteSyncBundle>, String> {
        let target_id = match remote_id {
            Some(id) if !id.trim().is_empty() => id.to_owned(),
            _ => match self.discover_remote_id(token)? {
                Some(found) => found,
                None => return Ok(None),
            },
        };
        let Some(gist) = self.get_gist(token, &target_id)? else {
            return Ok(None);
        };
        parse_gist_snapshot(&gist)
    }

    fn push_snapshot(
        &self,
        token: &str,
        remote_id: Option<&str>,
        snapshot: &SyncSnapshot,
    ) -> Result<String, String> {
        let content = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;
        let payload = json!({
            "description": SYNC_GIST_DESCRIPTION,
            "public": false,
            "files": {
                SYNC_GIST_FILENAME: {
                    "content": content
                }
            }
        });

        let resp = match remote_id.filter(|id| !id.trim().is_empty()) {
            Some(id) => {
                let url = format!("{}/gists/{id}", self.api_base);
                self.client
                    .patch(&url)
                    .bearer_auth(token)
                    .header("Accept", "application/vnd.github+json")
                    .json(&payload)
                    .send()
                    .map_err(|e| e.to_string())?
            }
            None => {
                let url = format!("{}/gists", self.api_base);
                self.client
                    .post(&url)
                    .bearer_auth(token)
                    .header("Accept", "application/vnd.github+json")
                    .json(&payload)
                    .send()
                    .map_err(|e| e.to_string())?
            }
        };

        if !resp.status().is_success() {
            return Err(format!("GitHub Gist push failed: HTTP {}", resp.status()));
        }
        let body = resp.json::<Value>().map_err(|e| e.to_string())?;
        body.get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "GitHub Gist push response missing id".into())
    }

    fn delete_snapshot(&self, token: &str, remote_id: &str) -> Result<(), String> {
        let url = format!("{}/gists/{remote_id}", self.api_base);
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() == 404 || resp.status().is_success() {
            return Ok(());
        }
        Err(format!("GitHub Gist delete failed: HTTP {}", resp.status()))
    }
}

pub(crate) const SYNC_REPO_NAME: &str = "graphite-sync";
pub(crate) const SYNC_REPO_FILE: &str = "graphite-sync.json";
const SYNC_REPO_DESCRIPTION: &str = "Graphite User Data Sync";

/// Private GitHub repository backend for User Data Sync.
///
/// The snapshot is one file (`graphite-sync.json`) in a repository named `graphite-sync` of the
/// signed-in account. The repository is created private; one that is public is never written to,
/// because the settings could hold private paths or values.
pub(crate) struct GitHubRepoTransport {
    client: Client,
    api_base: String,
}

impl GitHubRepoTransport {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .user_agent("Graphite-Editor")
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            api_base: "https://api.github.com".into(),
        })
    }

    #[cfg(test)]
    pub fn with_base_url(api_base: impl Into<String>) -> Result<Self, String> {
        let mut transport = Self::new()?;
        transport.api_base = api_base.into();
        Ok(transport)
    }

    fn send(
        &self,
        token: &str,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
        accept: &str,
    ) -> Result<reqwest::blocking::Response, String> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.api_base))
            .bearer_auth(token)
            .header("Accept", accept);
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().map_err(|e| e.to_string())
    }

    fn json(
        &self,
        token: &str,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<reqwest::blocking::Response, String> {
        self.send(token, method, path, body, "application/vnd.github+json")
    }

    /// `owner/name` of the sync repository: the stored one, or the signed-in account's.
    fn full_name(&self, token: &str, remote_id: Option<&str>) -> Result<String, String> {
        if let Some(id) = remote_id.map(str::trim).filter(|id| id.contains('/')) {
            return Ok(id.to_owned());
        }
        let response = self.json(token, reqwest::Method::GET, "/user", None)?;
        if !response.status().is_success() {
            return Err(format!(
                "GitHub account lookup failed: HTTP {}",
                response.status()
            ));
        }
        let user = response.json::<Value>().map_err(|e| e.to_string())?;
        let login = user
            .get("login")
            .and_then(Value::as_str)
            .ok_or("GitHub account response missing login")?;
        Ok(format!("{login}/{SYNC_REPO_NAME}"))
    }

    fn repository(&self, token: &str, full_name: &str) -> Result<Option<Value>, String> {
        let response = self.json(
            token,
            reqwest::Method::GET,
            &format!("/repos/{full_name}"),
            None,
        )?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!(
                "GitHub repository lookup failed: HTTP {}",
                response.status()
            ));
        }
        response
            .json::<Value>()
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// The sync file: its blob id and text, or `None` when it does not exist yet.
    fn file(&self, token: &str, full_name: &str) -> Result<Option<(String, String)>, String> {
        let path = format!("/repos/{full_name}/contents/{SYNC_REPO_FILE}");
        let response = self.json(token, reqwest::Method::GET, &path, None)?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!(
                "GitHub repository fetch failed: HTTP {}",
                response.status()
            ));
        }
        let body = response.json::<Value>().map_err(|e| e.to_string())?;
        let sha = body
            .get("sha")
            .and_then(Value::as_str)
            .ok_or("GitHub file response missing sha")?
            .to_owned();
        let encoded = body.get("content").and_then(Value::as_str).unwrap_or("");
        let text = if encoded.is_empty() {
            // Files over 1 MB come without content; the raw form has no such limit.
            let raw = self.send(
                token,
                reqwest::Method::GET,
                &path,
                None,
                "application/vnd.github.raw+json",
            )?;
            if !raw.status().is_success() {
                return Err(format!(
                    "GitHub repository fetch failed: HTTP {}",
                    raw.status()
                ));
            }
            raw.text().map_err(|e| e.to_string())?
        } else {
            decode_content(encoded)?
        };
        Ok(Some((sha, text)))
    }
}

/// Decodes the base64 text GitHub returns, which is wrapped in newlines.
pub(crate) fn decode_content(encoded: &str) -> Result<String, String> {
    use base64::Engine;
    let compact: String = encoded.split_whitespace().collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact)
        .map_err(|e| format!("Invalid file content from GitHub: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("Synced file is not valid UTF-8: {e}"))
}

/// Fails unless the repository is private.
pub(crate) fn ensure_private(repository: &Value) -> Result<(), String> {
    if repository.get("private").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(
            "The sync repository is public; Graphite will not upload settings to it. \
             Make it private or delete it."
                .to_owned(),
        )
    }
}

impl SyncTransport for GitHubRepoTransport {
    fn fetch_snapshot(
        &self,
        token: &str,
        remote_id: Option<&str>,
    ) -> Result<Option<RemoteSyncBundle>, String> {
        let full_name = self.full_name(token, remote_id)?;
        let Some(repository) = self.repository(token, &full_name)? else {
            return Ok(None);
        };
        ensure_private(&repository)?;
        let Some((_, text)) = self.file(token, &full_name)? else {
            return Ok(None);
        };
        let snapshot: SyncSnapshot = serde_json::from_str(&text)
            .map_err(|e| format!("Invalid synced repository payload: {e}"))?;
        Ok(Some(RemoteSyncBundle {
            remote_id: full_name,
            snapshot,
        }))
    }

    fn push_snapshot(
        &self,
        token: &str,
        remote_id: Option<&str>,
        snapshot: &SyncSnapshot,
    ) -> Result<String, String> {
        use base64::Engine;
        let mut full_name = self.full_name(token, remote_id)?;
        match self.repository(token, &full_name)? {
            Some(repository) => ensure_private(&repository)?,
            None => {
                let created = self.json(
                    token,
                    reqwest::Method::POST,
                    "/user/repos",
                    Some(&json!({
                        "name": SYNC_REPO_NAME,
                        "description": SYNC_REPO_DESCRIPTION,
                        "private": true,
                        "auto_init": true,
                    })),
                )?;
                if !created.status().is_success() {
                    return Err(format!(
                        "GitHub repository creation failed: HTTP {}",
                        created.status()
                    ));
                }
                let created = created.json::<Value>().map_err(|e| e.to_string())?;
                ensure_private(&created)?;
                if let Some(name) = created.get("full_name").and_then(Value::as_str) {
                    full_name = name.to_owned();
                }
            }
        }
        let existing = self.file(token, &full_name)?;
        let text = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;
        let mut body = json!({
            "message": "Update Graphite sync data",
            "content": base64::engine::general_purpose::STANDARD.encode(text),
        });
        if let Some((sha, _)) = existing {
            body["sha"] = Value::String(sha);
        }
        let response = self.json(
            token,
            reqwest::Method::PUT,
            &format!("/repos/{full_name}/contents/{SYNC_REPO_FILE}"),
            Some(&body),
        )?;
        let status = response.status();
        if !status.is_success() {
            let hint = if matches!(status.as_u16(), 409 | 422) {
                " (the file changed on GitHub; sync again)"
            } else {
                ""
            };
            return Err(format!(
                "GitHub repository push failed: HTTP {status}{hint}"
            ));
        }
        Ok(full_name)
    }

    /// Removes the sync file. The repository itself is left alone: it is the user's.
    fn delete_snapshot(&self, token: &str, remote_id: &str) -> Result<(), String> {
        let Some((sha, _)) = self.file(token, remote_id)? else {
            return Ok(());
        };
        let response = self.json(
            token,
            reqwest::Method::DELETE,
            &format!("/repos/{remote_id}/contents/{SYNC_REPO_FILE}"),
            Some(&json!({"message": "Remove Graphite sync data", "sha": sha})),
        )?;
        if response.status().as_u16() == 404 || response.status().is_success() {
            return Ok(());
        }
        Err(format!(
            "GitHub repository delete failed: HTTP {}",
            response.status()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gist_snapshot_extracts_id_and_snapshot() {
        let snap = SyncSnapshot {
            settings_json: Some("{\"editor.tabSize\":2}".into()),
            ..Default::default()
        };
        let content = serde_json::to_string(&snap).unwrap();
        let gist = json!({
            "id": "gist-123",
            "files": {
                SYNC_GIST_FILENAME: { "content": content }
            }
        });
        let parsed = parse_gist_snapshot(&gist).unwrap().unwrap();
        assert_eq!(parsed.remote_id, "gist-123");
        assert_eq!(
            parsed.snapshot.settings_json.as_deref(),
            Some("{\"editor.tabSize\":2}")
        );
    }

    #[test]
    fn with_base_url_constructs_transport() {
        assert!(GitHubGistTransport::with_base_url("http://127.0.0.1:0").is_ok());
    }

    // ---- repository backend, against a small local HTTP server

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<(String, String, String)>>>;

    /// Answers `(method, path)` with a fixed status and body, and records every request.
    fn serve(routes: Vec<(&'static str, &'static str, u16, String)>) -> (String, Log) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let recorded = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let mut reader = BufReader::new(&stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                    continue;
                }
                let mut length = 0usize;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" || header.is_empty() {
                        break;
                    }
                    if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_owned();
                let path = parts.next().unwrap_or("").to_owned();
                recorded.lock().unwrap().push((
                    method.clone(),
                    path.clone(),
                    String::from_utf8_lossy(&body).into_owned(),
                ));
                let (status, payload) = routes
                    .iter()
                    .find(|(m, p, _, _)| *m == method && *p == path)
                    .map(|(_, _, status, payload)| (*status, payload.clone()))
                    .unwrap_or((404, "{}".to_owned()));
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = (&stream).write_all(response.as_bytes());
            }
        });
        (base, log)
    }

    fn requests(log: &Log, method: &str) -> Vec<(String, String)> {
        log.lock()
            .unwrap()
            .iter()
            .filter(|(m, _, _)| m == method)
            .map(|(_, path, body)| (path.clone(), body.clone()))
            .collect()
    }

    fn encoded(snapshot: &SyncSnapshot) -> String {
        use base64::Engine;
        let text = serde_json::to_string_pretty(snapshot).unwrap();
        // GitHub wraps the base64 in newlines.
        let raw = base64::engine::general_purpose::STANDARD.encode(text);
        raw.as_bytes()
            .chunks(60)
            .map(|chunk| std::str::from_utf8(chunk).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn sample() -> SyncSnapshot {
        SyncSnapshot {
            settings_json: Some("{\"editor.tabSize\":2}".into()),
            ..Default::default()
        }
    }

    #[test]
    fn pushing_creates_a_private_repository_and_the_file_when_none_exists() {
        let (base, log) = serve(vec![
            ("GET", "/user", 200, r#"{"login":"octo"}"#.into()),
            (
                "POST",
                "/user/repos",
                201,
                r#"{"full_name":"octo/graphite-sync","private":true}"#.into(),
            ),
            (
                "PUT",
                "/repos/octo/graphite-sync/contents/graphite-sync.json",
                201,
                "{}".into(),
            ),
        ]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        let id = transport.push_snapshot("tok", None, &sample()).unwrap();
        assert_eq!(id, "octo/graphite-sync");

        let created = requests(&log, "POST");
        assert_eq!(created.len(), 1);
        let created: Value = serde_json::from_str(&created[0].1).unwrap();
        assert_eq!(created["private"], true);
        assert_eq!(created["name"], SYNC_REPO_NAME);

        let put = requests(&log, "PUT");
        let put: Value = serde_json::from_str(&put[0].1).unwrap();
        assert!(
            put.get("sha").is_none(),
            "a new file has no blob to replace"
        );
        let text = decode_content(put["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            serde_json::from_str::<SyncSnapshot>(&text).unwrap(),
            sample()
        );
    }

    #[test]
    fn pushing_to_an_existing_file_replaces_it_by_its_blob_id() {
        let (base, log) = serve(vec![
            (
                "GET",
                "/repos/octo/graphite-sync",
                200,
                r#"{"private":true}"#.into(),
            ),
            (
                "GET",
                "/repos/octo/graphite-sync/contents/graphite-sync.json",
                200,
                json!({"sha": "abc123", "content": encoded(&sample())}).to_string(),
            ),
            (
                "PUT",
                "/repos/octo/graphite-sync/contents/graphite-sync.json",
                200,
                "{}".into(),
            ),
        ]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        let id = transport
            .push_snapshot("tok", Some("octo/graphite-sync"), &sample())
            .unwrap();
        assert_eq!(id, "octo/graphite-sync");
        // The stored name means no account lookup and no new repository.
        assert!(requests(&log, "POST").is_empty());
        assert!(!log
            .lock()
            .unwrap()
            .iter()
            .any(|(_, path, _)| path == "/user"));
        let put: Value = serde_json::from_str(&requests(&log, "PUT")[0].1).unwrap();
        assert_eq!(put["sha"], "abc123");
    }

    #[test]
    fn a_public_repository_is_never_written_to() {
        let (base, log) = serve(vec![(
            "GET",
            "/repos/octo/graphite-sync",
            200,
            r#"{"private":false}"#.into(),
        )]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        let error = transport
            .push_snapshot("tok", Some("octo/graphite-sync"), &sample())
            .unwrap_err();
        assert!(error.contains("public"), "{error}");
        assert!(requests(&log, "PUT").is_empty());
        assert!(transport
            .fetch_snapshot("tok", Some("octo/graphite-sync"))
            .unwrap_err()
            .contains("public"));
    }

    #[test]
    fn fetching_reads_the_snapshot_from_the_wrapped_base64() {
        let (base, _) = serve(vec![
            (
                "GET",
                "/repos/octo/graphite-sync",
                200,
                r#"{"private":true}"#.into(),
            ),
            (
                "GET",
                "/repos/octo/graphite-sync/contents/graphite-sync.json",
                200,
                json!({"sha": "abc123", "content": encoded(&sample())}).to_string(),
            ),
        ]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        let bundle = transport
            .fetch_snapshot("tok", Some("octo/graphite-sync"))
            .unwrap()
            .expect("a snapshot");
        assert_eq!(bundle.remote_id, "octo/graphite-sync");
        assert_eq!(bundle.snapshot, sample());
    }

    #[test]
    fn fetching_without_a_repository_or_file_finds_nothing() {
        let (base, _) = serve(vec![("GET", "/user", 200, r#"{"login":"octo"}"#.into())]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        // /repos/octo/graphite-sync answers 404: no repository yet.
        assert_eq!(transport.fetch_snapshot("tok", None).unwrap(), None);

        let (base, _) = serve(vec![(
            "GET",
            "/repos/octo/graphite-sync",
            200,
            r#"{"private":true}"#.into(),
        )]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        // The repository is there but the file is not.
        assert_eq!(
            transport
                .fetch_snapshot("tok", Some("octo/graphite-sync"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn resetting_deletes_only_the_file_never_the_repository() {
        let (base, log) = serve(vec![
            (
                "GET",
                "/repos/octo/graphite-sync/contents/graphite-sync.json",
                200,
                json!({"sha": "abc123", "content": encoded(&sample())}).to_string(),
            ),
            (
                "DELETE",
                "/repos/octo/graphite-sync/contents/graphite-sync.json",
                200,
                "{}".into(),
            ),
        ]);
        let transport = GitHubRepoTransport::with_base_url(base).unwrap();
        transport
            .delete_snapshot("tok", "octo/graphite-sync")
            .unwrap();
        let deletes = requests(&log, "DELETE");
        assert_eq!(deletes.len(), 1);
        assert!(deletes[0].0.ends_with("/contents/graphite-sync.json"));
        let body: Value = serde_json::from_str(&deletes[0].1).unwrap();
        assert_eq!(body["sha"], "abc123");
    }

    #[test]
    fn privacy_check_requires_an_explicit_true() {
        assert!(ensure_private(&json!({"private": true})).is_ok());
        assert!(ensure_private(&json!({"private": false})).is_err());
        assert!(ensure_private(&json!({})).is_err());
    }
}
