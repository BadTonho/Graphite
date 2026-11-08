use crate::{DocumentId, ServiceError, ServiceResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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
