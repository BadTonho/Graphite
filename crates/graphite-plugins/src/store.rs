use crate::{manifest::Metadata, package, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installed {
    pub metadata: Metadata,
    pub enabled: bool,
    pub trusted_version: Option<String>,
    /// Immutable installation directory, relative to the store.
    pub directory: String,
}
#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn list(&self) -> Result<Vec<Installed>> {
        let mut result = Vec::new();
        if !self.root.exists() {
            return Ok(result);
        }
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|x| x == "json") {
                let record: Installed =
                    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if !crate::manifest::valid_id(record.metadata.id())
                    || !package::safe_path(&record.directory)
                    || record.directory.contains('/')
                {
                    return Err("Invalid plugin registry".into());
                }
                result.push(record);
            }
        }
        result.sort_by(|a, b| a.metadata.id().cmp(b.metadata.id()));
        Ok(result)
    }
    fn publish(&self, record: &Installed) -> Result<()> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.root.join(format!("{}.json", record.metadata.id()));
        let temp = self.root.join(format!(".{}.tmp", record.metadata.id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            use std::io::Write;
            file.write_all(&serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            drop(file);
            atomic_replace(&temp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
    pub fn install(&self, package_path: &Path) -> Result<Installed> {
        let metadata = package::inspect(package_path)?;
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let unique = format!(
            "{}@{}-{}-{}",
            metadata.id(),
            metadata.version(),
            std::process::id(),
            next_id()
        );
        let staging = self.root.join(format!(".staging-{unique}"));
        let directory = self.root.join(&unique);
        let result = (|| {
            let extracted = package::extract(package_path, &staging)?;
            if extracted.id() != metadata.id() || extracted.version() != metadata.version() {
                return Err("Package changed during installation".into());
            }
            fs::rename(&staging, &directory).map_err(|e| e.to_string())?;
            let record = Installed {
                metadata,
                enabled: false,
                trusted_version: None,
                directory: unique,
            };
            self.publish(&record)?;
            // Old immutable directories are deliberately retained until removal; a live host may own one.
            Ok(record)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(staging);
            let _ = fs::remove_dir_all(directory);
        }
        result
    }
    pub fn enable(&self, id: &str, trusted: bool) -> Result<()> {
        let mut record = self.get(id)?;
        if !matches!(record.metadata, Metadata::Native(_)) {
            return Err("VSIX execution is unavailable".into());
        }
        if !trusted {
            return Err("Explicit trust is required to execute plugin code".into());
        }
        record.enabled = true;
        record.trusted_version = Some(record.metadata.version().into());
        self.publish(&record)
    }
    pub fn disable(&self, id: &str) -> Result<()> {
        let mut record = self.get(id)?;
        record.enabled = false;
        self.publish(&record)
    }
    pub fn get(&self, id: &str) -> Result<Installed> {
        self.list()?
            .into_iter()
            .find(|p| p.metadata.id() == id)
            .ok_or_else(|| "Plugin is not installed".into())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.get(id)?;
        fs::remove_file(self.root.join(format!("{id}.json"))).map_err(|e| e.to_string())?;
        // Only direct, non-symlink children with this validated identity prefix are eligible.
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(&format!("{id}@"))
                && entry.file_type().map_err(|e| e.to_string())?.is_dir()
            {
                fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    // Nanosecond time avoids collisions across process restarts; counter covers same-tick installs.
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    time.wrapping_add(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}
#[cfg(not(windows))]
fn atomic_replace(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).map_err(|e| e.to_string())
}
#[cfg(windows)]
fn atomic_replace(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
