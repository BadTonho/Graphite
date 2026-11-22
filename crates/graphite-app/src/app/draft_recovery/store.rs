use crate::file_format::{Encoding, LineEnding};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

pub(super) const VERSION: u8 = 1;
const TEMP_PREFIX: &str = ".pending-";

#[derive(Serialize, Deserialize)]
pub(super) struct Draft {
    pub version: u8,
    pub timestamp: u64,
    pub original: Option<NativePath>,
    pub label: Option<String>,
    pub base: Option<[u8; 32]>,
    pub revision: u64,
    pub encoding: Encoding,
    pub ending: LineEnding,
    pub text: String,
}

// PathBuf's JSON representation cannot encode all native file names.
#[cfg(unix)]
type PathUnit = u8;
#[cfg(windows)]
type PathUnit = u16;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct NativePath(Vec<PathUnit>);

impl NativePath {
    pub fn new(path: &Path) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Self(path.as_os_str().as_bytes().to_vec())
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Self(path.as_os_str().encode_wide().collect())
        }
    }

    pub fn path(&self) -> PathBuf {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(self.0.clone()).into()
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&self.0).into()
        }
    }
}

pub(super) struct Session {
    pub directory: PathBuf,
    _lock: File,
}

impl Session {
    pub fn create(profile: &Path) -> io::Result<Arc<Self>> {
        let root = root(profile);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.recursive(true).create(&root)?;
        let directory = tempfile::Builder::new()
            .prefix("window-")
            .tempdir_in(root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.path().join("session.lock"))?;
        lock.lock()?;
        Ok(Arc::new(Self {
            directory: directory.keep(),
            _lock: lock,
        }))
    }

    pub fn acquire(directory: &Path) -> io::Result<Option<Arc<Self>>> {
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.join("session.lock"))
        {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => {
                // The previous process may have stopped before renaming a partial checkpoint.
                if let Err(error) = remove_partial_snapshots(directory) {
                    super::report("drafts.remove_partial", &error);
                }
                Ok(Some(Arc::new(Self {
                    directory: directory.into(),
                    _lock: lock,
                })))
            }
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}

fn remove_partial_snapshots(directory: &Path) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX)
        {
            remove(&entry.path())?;
        }
    }
    Ok(())
}

pub(super) fn root(profile: &Path) -> PathBuf {
    profile.join("draft-recovery")
}

pub(super) fn read(path: &Path) -> io::Result<Draft> {
    let draft: Draft =
        serde_json::from_reader(BufReader::new(File::open(path)?)).map_err(|error| {
            io::Error::from(error.io_error_kind().unwrap_or(io::ErrorKind::InvalidData))
        })?;
    if draft.version != VERSION
        || draft
            .original
            .as_ref()
            .is_some_and(|p| !p.path().is_absolute())
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(draft)
}

pub(super) fn write(path: &Path, draft: &Draft) -> io::Result<()> {
    let mut temp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .tempfile_in(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
    serde_json::to_writer(temp.as_file_mut(), draft).map_err(|error| {
        io::Error::from(error.io_error_kind().unwrap_or(io::ErrorKind::InvalidData))
    })?;
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

pub(super) fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
