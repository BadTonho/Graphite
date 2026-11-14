use crate::{DocumentId, ServiceError, ServiceResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Schemes an extension's file system provider serves. Documents under them are identified by
/// their virtual URI, which has no local path.
static VIRTUAL_SCHEMES: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());

/// Schemes with a built-in meaning that a provider can never take over.
const RESERVED_SCHEMES: [&str; 11] = [
    "file",
    "untitled",
    "http",
    "https",
    "javascript",
    "data",
    "blob",
    "vbscript",
    "ws",
    "wss",
    "ftp",
];

/// Makes `scheme` a virtual document scheme. Returns false for an invalid or reserved scheme.
pub fn register_virtual_scheme(scheme: &str) -> bool {
    let scheme = scheme.to_ascii_lowercase();
    let valid = scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid || RESERVED_SCHEMES.contains(&scheme.as_str()) {
        return false;
    }
    VIRTUAL_SCHEMES
        .write()
        .map(|mut schemes| schemes.insert(scheme))
        .unwrap_or(false)
}

pub fn unregister_virtual_scheme(scheme: &str) {
    if let Ok(mut schemes) = VIRTUAL_SCHEMES.write() {
        schemes.remove(&scheme.to_ascii_lowercase());
    }
}

pub fn is_virtual_scheme(scheme: &str) -> bool {
    VIRTUAL_SCHEMES
        .read()
        .map(|schemes| schemes.contains(&scheme.to_ascii_lowercase()))
        .unwrap_or(false)
}
/// Validated URI: never use lossy OS-path conversion at a protocol boundary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DocumentUri(String);
impl<'de> Deserialize<'de> for DocumentUri {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
impl DocumentUri {
    pub fn parse(value: &str) -> ServiceResult<Self> {
        let parsed =
            url::Url::parse(value).map_err(|e| ServiceError::InvalidArguments(e.to_string()))?;
        match parsed.scheme() {
            "file" => {
                if parsed.query().is_some()
                    || parsed.fragment().is_some()
                    || parsed.to_file_path().is_err()
                {
                    return Err(ServiceError::InvalidArguments(
                        "Invalid local file URI".into(),
                    ));
                }
            }
            "untitled"
                if !parsed.path().is_empty()
                    && parsed.query().is_none()
                    && parsed.fragment().is_none() => {}
            other if is_virtual_scheme(other) => {}
            other => return Err(ServiceError::UnsupportedUri(other.into())),
        }
        Ok(Self(parsed.to_string()))
    }
    pub fn from_path(path: &Path) -> ServiceResult<Self> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        url::Url::from_file_path(path)
            .map(|uri| Self(uri.to_string()))
            .map_err(|_| {
                ServiceError::InvalidArguments("Path must be absolute and URI-encodable".into())
            })
    }
    pub fn untitled(id: DocumentId) -> Self {
        Self(format!("untitled:document/{}", id.0))
    }
    pub fn to_path(&self) -> ServiceResult<PathBuf> {
        url::Url::parse(&self.0)
            .map_err(|e| ServiceError::InvalidArguments(e.to_string()))?
            .to_file_path()
            .map_err(|_| ServiceError::UnsupportedUri(self.0.clone()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_untitled(&self) -> bool {
        self.0.starts_with("untitled:")
    }
    /// The scheme of the URI (`file`, `untitled`, or a virtual scheme).
    pub fn scheme(&self) -> &str {
        self.0.split_once(':').map_or("", |(scheme, _)| scheme)
    }
    /// A document served by an extension's file system provider.
    pub fn is_virtual(&self) -> bool {
        !matches!(self.scheme(), "file" | "untitled")
    }
}
pub trait FileSystem {
    fn read(&self, uri: &DocumentUri) -> ServiceResult<Vec<u8>>;
    fn read_only(&self, uri: &DocumentUri) -> ServiceResult<bool>;
}
pub struct LocalFileSystem;
impl FileSystem for LocalFileSystem {
    fn read(&self, uri: &DocumentUri) -> ServiceResult<Vec<u8>> {
        std::fs::read(uri.to_path()?).map_err(|e| ServiceError::Io(e.to_string()))
    }
    fn read_only(&self, uri: &DocumentUri) -> ServiceResult<bool> {
        let path = uri.to_path()?;
        let read_only = std::fs::metadata(&path)
            .map_err(|error| ServiceError::Io(error.to_string()))?
            .permissions()
            .readonly();
        Ok(read_only || std::fs::OpenOptions::new().write(true).open(path).is_err())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uris_round_trip_reserved_unicode_and_untitled() {
        let path = std::env::temp_dir().join("a # percent% Unicode-\u{1f30e}.rs");
        let uri = DocumentUri::from_path(&path).unwrap();
        assert_eq!(uri.to_path().unwrap(), path);
        assert!(uri.as_str().contains("%23"));
        let untitled = DocumentUri::untitled(DocumentId(9));
        assert_eq!(DocumentUri::parse(untitled.as_str()).unwrap(), untitled);
        assert!(untitled.to_path().is_err());
        assert!(matches!(
            DocumentUri::parse("https://example.com/a"),
            Err(ServiceError::UnsupportedUri(_))
        ));
        assert!(DocumentUri::parse("file:///test?query").is_err());
    }
    #[test]
    fn registered_virtual_schemes_parse_and_have_no_local_path() {
        assert!(DocumentUri::parse("vfstest-a:/docs/a.md").is_err());
        assert!(register_virtual_scheme("vfstest-a"));
        let uri = DocumentUri::parse("vfstest-a:/docs/a%20b.md").unwrap();
        assert!(uri.is_virtual() && !uri.is_untitled());
        assert_eq!(uri.scheme(), "vfstest-a");
        assert!(matches!(uri.to_path(), Err(ServiceError::UnsupportedUri(_))));
        let with_authority = DocumentUri::parse("vfstest-a://host/x").unwrap();
        assert_eq!(with_authority.as_str(), "vfstest-a://host/x");
        unregister_virtual_scheme("vfstest-a");
        assert!(DocumentUri::parse("vfstest-a:/docs/a.md").is_err());
    }

    #[test]
    fn reserved_and_invalid_schemes_cannot_be_registered() {
        for scheme in ["file", "untitled", "HTTPS", "javascript", "data", "1abc", "a b", ""] {
            assert!(!register_virtual_scheme(scheme), "{scheme}");
        }
        assert!(!is_virtual_scheme("file"));
        assert!(!DocumentUri::from_path(&std::env::temp_dir()).unwrap().is_virtual());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_not_lossy() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/path-\xff".to_vec()));
        assert_eq!(
            DocumentUri::from_path(&path).unwrap().to_path().unwrap(),
            path
        );
    }
}
