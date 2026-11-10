//! Open VSX registry client. Downloaded VSIX files still pass through `package::inspect` and
//! `Store::install`; registry metadata is never treated as trusted executable code.
use crate::{package, Result, MAX_DOWNLOAD};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{io::Read, path::Path, time::Duration};

const DEFAULT_REGISTRY: &str = "https://open-vsx.org/api";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub namespace: String,
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    extensions: Vec<Extension>,
}
#[derive(Deserialize)]
struct VersionResponse {
    namespace: String,
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    files: Files,
}
#[derive(Deserialize)]
struct Files {
    download: String,
}

#[derive(Clone)]
pub struct Registry {
    client: Client,
    base: String,
}

impl Registry {
    pub fn new() -> Result<Self> {
        Self::with_base(DEFAULT_REGISTRY)
    }
    /// Custom bases are intended for local registry fixtures. Non-loopback HTTP is rejected.
    pub fn with_base(base: &str) -> Result<Self> {
        let url = reqwest::Url::parse(base).map_err(|e| e.to_string())?;
        if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback(url.host_str())) {
            return Err(
                "Open VSX registry must use HTTPS (HTTP is allowed for loopback fixtures)".into(),
            );
        }
        Ok(Self {
            client: Client::builder()
                .user_agent("Graphite-OpenVSX/0.5")
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())?,
            base: base.trim_end_matches('/').into(),
        })
    }
    pub fn search(&self, query: &str, size: usize) -> Result<Vec<Extension>> {
        if query.len() > 256 || size == 0 || size > 50 {
            return Err("Invalid Open VSX search parameters".into());
        }
        let mut url =
            reqwest::Url::parse(&format!("{}/-/search", self.base)).map_err(|e| e.to_string())?;
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("size", &size.to_string());
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut items = response
            .json::<SearchResponse>()
            .map_err(|e| e.to_string())?
            .extensions;
        items.retain(|item| valid_part(&item.namespace) && valid_part(&item.name));
        Ok(items)
    }
    pub fn latest(&self, namespace: &str, name: &str) -> Result<Extension> {
        if !valid_part(namespace) || !valid_part(name) {
            return Err("Invalid Open VSX extension identity".into());
        }
        let url = format!("{}/{}", self.base, format!("{namespace}/{name}/latest"));
        let version = self
            .client
            .get(url)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<VersionResponse>()
            .map_err(|e| e.to_string())?;
        if !valid_part(&version.namespace)
            || !valid_part(&version.name)
            || version.namespace != namespace
            || version.name != name
            || semver::Version::parse(&version.version).is_err()
        {
            return Err("Invalid Open VSX version metadata".into());
        }
        Ok(Extension {
            namespace: version.namespace,
            name: version.name,
            display_name: format!("{}.{}", namespace, name),
            version: version.version,
            description: version.description,
        })
    }
    pub fn download_latest(
        &self,
        namespace: &str,
        name: &str,
        destination: &Path,
    ) -> Result<Extension> {
        if !valid_part(namespace) || !valid_part(name) {
            return Err("Invalid Open VSX extension identity".into());
        }
        let url = format!("{}/{}/{}/latest", self.base, namespace, name);
        let metadata = self
            .client
            .get(url)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<VersionResponse>()
            .map_err(|e| e.to_string())?;
        if metadata.namespace != namespace
            || metadata.name != name
            || semver::Version::parse(&metadata.version).is_err()
        {
            return Err("Open VSX response identity mismatch".into());
        }
        let download = reqwest::Url::parse(&metadata.files.download).map_err(|e| e.to_string())?;
        let base_url = reqwest::Url::parse(&self.base).map_err(|e| e.to_string())?;
        if download.scheme() != "https"
            && !(download.scheme() == "http"
                && is_loopback(download.host_str())
                && is_loopback(base_url.host_str()))
        {
            return Err("Open VSX download URL must use HTTPS".into());
        }
        let response = self
            .client
            .get(download)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        if response.url().scheme() != "https"
            && !(response.url().scheme() == "http"
                && is_loopback(response.url().host_str())
                && is_loopback(base_url.host_str()))
        {
            return Err("Open VSX redirected to an insecure download URL".into());
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_DOWNLOAD)
        {
            return Err("VSIX exceeds 100 MiB".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_DOWNLOAD + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DOWNLOAD {
            return Err("VSIX exceeds 100 MiB".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|e| e.to_string())?;
        use std::io::Write;
        if let Err(error) = file.write_all(&bytes) {
            drop(file);
            let _ = std::fs::remove_file(destination);
            return Err(error.to_string());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        let inspected = match package::inspect(destination) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = std::fs::remove_file(destination);
                return Err(error);
            }
        };
        if inspected.id() != format!("{}.{}", namespace, name).to_ascii_lowercase()
            || inspected.version() != metadata.version
        {
            let _ = std::fs::remove_file(destination);
            return Err("Downloaded VSIX does not match Open VSX identity or version".into());
        }
        Ok(Extension {
            namespace: metadata.namespace,
            name: metadata.name,
            display_name: format!("{}.{}", namespace, name),
            version: metadata.version,
            description: metadata.description,
        })
    }

    /// Downloads and installs a VSIX through the same archive validation and immutable
    /// installation path as a local package. Execution remains disabled until supported host
    /// activation and both trust gates are available.
    pub fn install_latest(
        &self,
        namespace: &str,
        name: &str,
        store: &crate::store::Store,
    ) -> Result<crate::store::Installed> {
        std::fs::create_dir_all(&store.root).map_err(|e| e.to_string())?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let temp = store.root.join(format!(
            ".open-vsx-{}-{}.vsix",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let result = self
            .download_latest(namespace, name, &temp)
            .and_then(|_| store.install(&temp));
        let _ = std::fs::remove_file(temp);
        result
    }
}

fn valid_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}
fn is_loopback(host: Option<&str>) -> bool {
    host.is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host == "127.0.0.1"
            || host == "::1"
            || host == "[::1]"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_insecure_remote_registry_and_invalid_extension_ids() {
        assert!(Registry::with_base("http://registry.example").is_err());
        assert!(Registry::with_base("http://127.0.0.1:8080").is_ok());
        assert!(Registry::new().is_ok());
        assert!(!valid_part("../outside"));
        assert!(!valid_part(""));
    }
    #[test]
    fn bounds_search_parameters() {
        let registry = Registry::new().unwrap();
        assert!(registry.search("x", 0).is_err());
        assert!(registry.search("x", 51).is_err());
        assert!(registry.latest("../bad", "extension").is_err());
    }

    #[test]
    fn search_uses_a_local_registry_fixture_without_marketplace_access() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            let body = r#"{"extensions":[{"namespace":"publisher","name":"extension","version":"1.2.3","displayName":"Fixture","description":"local fixture"},{"namespace":"../bad","name":"skip"}]}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let registry = Registry::with_base(&format!("http://{address}")).unwrap();
        let found = registry.search("fixture", 5).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, "1.2.3");
        server.join().unwrap();
    }

    #[test]
    fn latest_rejects_registry_identity_mismatch() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            let body = r#"{"namespace":"other","name":"extension","version":"1.2.3","files":{"download":"http://127.0.0.1/package.vsix"}}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let registry = Registry::with_base(&format!("http://{address}")).unwrap();
        assert!(registry.latest("publisher", "extension").is_err());
        server.join().unwrap();
    }

    #[test]
    fn latest_download_is_inspected_and_installed_from_a_local_fixture() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut archive = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut archive));
            zip.start_file(
                "extension/package.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                br#"{"publisher":"publisher","name":"extension","version":"1.2.3","description":"Fixture","main":"./dist/main.js","engines":{"vscode":"^1.90.0"}}"#,
            )
            .unwrap();
            zip.start_file(
                "extension/dist/main.js",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"module.exports = {}; ").unwrap();
            zip.finish().unwrap();
        }
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 2048];
                let count = stream.read(&mut request).unwrap();
                let path = String::from_utf8_lossy(&request[..count]);
                if path.starts_with("GET /publisher/extension/latest") {
                    let body = format!(
                        r#"{{"namespace":"publisher","name":"extension","version":"1.2.3","description":"Fixture","files":{{"download":"http://{address}/package.vsix"}}}}"#
                    );
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                } else {
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", archive.len()).unwrap();
                    stream.write_all(&archive).unwrap();
                }
            }
        });
        let root = std::env::temp_dir().join(format!(
            "graphite-open-vsx-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let old_package = root.join("old.vsix");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&old_package).unwrap());
            zip.start_file(
                "extension/package.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(br#"{"publisher":"publisher","name":"extension","version":"1.0.0"}"#)
                .unwrap();
            zip.finish().unwrap();
        }
        let store = crate::store::Store::new(root.join("store"));
        let old = store.install(&old_package).unwrap();
        assert_eq!(old.metadata.version(), "1.0.0");
        let registry = Registry::with_base(&format!("http://{address}")).unwrap();
        let updated = registry
            .install_latest("publisher", "extension", &store)
            .unwrap();
        assert_eq!(updated.metadata.version(), "1.2.3");
        assert_eq!(updated.metadata.id(), "publisher.extension");
        assert!(!updated.enabled);
        assert!(updated.trusted_version.is_none());
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(root.join("store").join(old.directory).is_dir());
        server.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn latest_download_rejects_a_package_that_disagrees_with_registry_metadata() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut archive = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut archive));
            zip.start_file(
                "extension/package.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(br#"{"publisher":"publisher","name":"other","version":"1.2.3"}"#)
                .unwrap();
            zip.finish().unwrap();
        }
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0u8; 2048];
                let count = stream.read(&mut request).unwrap();
                let path = String::from_utf8_lossy(&request[..count]);
                if path.starts_with("GET /publisher/extension/latest") {
                    let body = format!(
                        r#"{{"namespace":"publisher","name":"extension","version":"1.2.3","files":{{"download":"http://{address}/package.vsix"}}}}"#
                    );
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                } else {
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", archive.len()).unwrap();
                    stream.write_all(&archive).unwrap();
                }
            }
        });
        let root = std::env::temp_dir().join(format!(
            "graphite-open-vsx-mismatch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let destination = root.join("mismatch.vsix");
        let registry = Registry::with_base(&format!("http://{address}")).unwrap();
        assert!(registry
            .download_latest("publisher", "extension", &destination)
            .is_err());
        assert!(!destination.exists());
        server.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
