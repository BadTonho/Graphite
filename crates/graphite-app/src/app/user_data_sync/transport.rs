//! Remote transport abstraction for User Data Sync and GitHub Gist implementation.

use super::state::SyncSnapshot;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) const SYNC_GIST_FILENAME: &str = "graphite-sync.json";
pub(crate) const SYNC_GIST_DESCRIPTION: &str = "Graphite User Data Sync";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteSyncBundle {
    pub gist_id: String,
    pub snapshot: SyncSnapshot,
}

pub(crate) trait SyncTransport: Send + Sync {
    fn fetch_snapshot(
        &self,
        token: &str,
        gist_id: Option<&str>,
    ) -> Result<Option<RemoteSyncBundle>, String>;

    fn push_snapshot(
        &self,
        token: &str,
        gist_id: Option<&str>,
        snapshot: &SyncSnapshot,
    ) -> Result<String, String>;

    fn delete_snapshot(&self, token: &str, gist_id: &str) -> Result<(), String>;
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

    fn get_gist(&self, token: &str, gist_id: &str) -> Result<Option<Value>, String> {
        let url = format!("{}/gists/{gist_id}", self.api_base);
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

    fn discover_gist_id(&self, token: &str) -> Result<Option<String>, String> {
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
        gist_id: id,
        snapshot,
    }))
}

impl SyncTransport for GitHubGistTransport {
    fn fetch_snapshot(
        &self,
        token: &str,
        gist_id: Option<&str>,
    ) -> Result<Option<RemoteSyncBundle>, String> {
        let target_id = match gist_id {
            Some(id) if !id.trim().is_empty() => id.to_owned(),
            _ => match self.discover_gist_id(token)? {
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
        gist_id: Option<&str>,
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

        let resp = match gist_id.filter(|id| !id.trim().is_empty()) {
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

    fn delete_snapshot(&self, token: &str, gist_id: &str) -> Result<(), String> {
        let url = format!("{}/gists/{gist_id}", self.api_base);
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
        assert_eq!(parsed.gist_id, "gist-123");
        assert_eq!(
            parsed.snapshot.settings_json.as_deref(),
            Some("{\"editor.tabSize\":2}")
        );
    }

    #[test]
    fn with_base_url_constructs_transport() {
        assert!(GitHubGistTransport::with_base_url("http://127.0.0.1:0").is_ok());
    }
}
