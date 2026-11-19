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
    /// Optional local project directory linked for rapid development.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_path: Option<PathBuf>,
}

impl Installed {
    pub fn install_dir(&self, store_root: &Path) -> PathBuf {
        self.dev_path
            .clone()
            .unwrap_or_else(|| store_root.join(&self.directory))
    }
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
                let mut record: Installed =
                    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if !crate::manifest::valid_id(record.metadata.id())
                    || !package::safe_path(&record.directory)
                    || record.directory.contains('/')
                {
                    return Err("Invalid plugin registry".into());
                }
                if let Some(dev_path) = &record.dev_path {
                    if let Ok(manifest) = read_dev_manifest(dev_path) {
                        if manifest.id == record.metadata.id() {
                            if record.enabled && record.trusted_version.is_some() {
                                record.trusted_version = Some(manifest.version.clone());
                            }
                            record.metadata = Metadata::Native(manifest);
                        }
                    }
                }
                // Older records predate the command index. Their original
                // package manifest is already in the extracted directory.
                if let Metadata::Vsix(vsix) = &mut record.metadata {
                    if vsix.commands.is_empty() {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.commands = crate::manifest::vsix_commands(&manifest);
                            }
                        }
                    }
                    if vsix.activity_bar.is_empty() {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.activity_bar = crate::manifest::vsix_activity_bar(&manifest);
                            }
                        }
                    }
                    if vsix.custom_editors.is_empty()
                        && vsix
                            .contributions
                            .iter()
                            .any(|name| name == "customEditors")
                    {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.custom_editors =
                                    crate::manifest::vsix_custom_editors(&manifest);
                                vsix.activation_events =
                                    crate::manifest::vsix_activation_events(&manifest);
                            }
                        }
                    }
                    if vsix.grammars.is_empty()
                        && vsix.contributions.iter().any(|name| name == "grammars")
                    {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.grammars = crate::manifest::vsix_grammars(&manifest);
                                vsix.languages = crate::manifest::vsix_languages(&manifest);
                            }
                        }
                    }
                    if vsix.notebooks.is_empty()
                        && vsix.contributions.iter().any(|name| name == "notebooks")
                    {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.notebooks = crate::manifest::vsix_notebooks(&manifest);
                                vsix.activation_events =
                                    crate::manifest::vsix_activation_events(&manifest);
                            }
                        }
                    }
                    if vsix.debuggers.is_empty()
                        && vsix.contributions.iter().any(|name| name == "debuggers")
                    {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.debuggers = crate::manifest::vsix_debuggers(&manifest);
                            }
                        }
                    }
                    if vsix.language_comments.is_empty() {
                        let path = self
                            .root
                            .join(&record.directory)
                            .join("extension/package.json");
                        if let Ok(file) = fs::File::open(path) {
                            use std::io::Read;
                            if let Ok(manifest) =
                                serde_json::from_reader(file.take(1024 * 1024 + 1))
                            {
                                vsix.language_comments = crate::manifest::vsix_language_comments(
                                    &manifest,
                                    |configuration| {
                                        let path = self
                                            .root
                                            .join(&record.directory)
                                            .join("extension")
                                            .join(configuration);
                                        let bytes = fs::read(path).ok()?;
                                        (bytes.len() <= 1024 * 1024)
                                            .then(|| serde_json::from_slice(&bytes).ok())
                                            .flatten()
                                    },
                                );
                            }
                        }
                    }
                }
                result.push(record);
            }
        }
        result.sort_by(|a, b| a.metadata.id().cmp(b.metadata.id()));
        Ok(result)
    }
    fn publish(&self, record: &Installed, replace_directory: Option<&str>) -> Result<()> {
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
            if let Some(expected) = replace_directory {
                let current: Installed = serde_json::from_reader(
                    fs::File::open(&path).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                if current.directory != expected {
                    return Err("The installed package changed; review the new installation".into());
                }
            }
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
        self.install_package(package_path, None, None)
    }

    pub fn install_reviewed(
        &self,
        package_path: &Path,
        id: &str,
        version: &str,
    ) -> Result<Installed> {
        self.install_package(package_path, Some((id, version)), None)
    }

    pub fn install_update(
        &self,
        package_path: &Path,
        id: &str,
        directory: &str,
        version: &str,
    ) -> Result<Installed> {
        self.install_package(package_path, Some((id, version)), Some(directory))
    }

    fn install_package(
        &self,
        package_path: &Path,
        reviewed: Option<(&str, &str)>,
        replace_directory: Option<&str>,
    ) -> Result<Installed> {
        let metadata = package::inspect(package_path)?;
        if reviewed
            .is_some_and(|(id, version)| metadata.id() != id || metadata.version() != version)
        {
            return Err("Package no longer matches the reviewed version".into());
        }
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
                dev_path: None,
            };
            self.publish(&record, replace_directory)?;
            // Old immutable directories are deliberately retained until removal; a live host may own one.
            Ok(record)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(staging);
            let _ = fs::remove_dir_all(directory);
        }
        result
    }
    pub fn link_dev(&self, project_dir: &Path) -> Result<Installed> {
        let canonical = fs::canonicalize(project_dir).map_err(|e| e.to_string())?;
        let manifest = read_dev_manifest(&canonical)?;
        if crate::manifest::resolve_entrypoint(&canonical, &manifest).is_none() {
            return Err(format!("Plugin entry script not found: {}", manifest.main));
        }
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let record = Installed {
            trusted_version: Some(manifest.version.clone()),
            directory: format!("{}@dev", manifest.id),
            metadata: Metadata::Native(manifest),
            enabled: true,
            dev_path: Some(canonical),
        };
        self.publish(&record, None)?;
        Ok(record)
    }

    pub fn enable(&self, id: &str, trusted: bool) -> Result<()> {
        let record = self.get(id)?;
        self.enable_installation(id, &record.directory, trusted)
    }
    pub fn enable_installation(&self, id: &str, directory: &str, trusted: bool) -> Result<()> {
        if !trusted {
            return Err("Explicit trust is required to execute plugin code".into());
        }
        let mut record = self.get(id)?;
        if record.directory != directory {
            return Err("The installed package changed; review the new installation".into());
        }
        // A VSIX without a Node.js entry point (themes, snippets, language packs) runs no code;
        // enabling it only applies its declarative contributions.
        record.enabled = true;
        record.trusted_version = Some(record.metadata.version().into());
        self.publish(&record, Some(directory))
    }
    pub fn disable(&self, id: &str) -> Result<()> {
        let mut record = self.get(id)?;
        record.enabled = false;
        self.publish(&record, Some(&record.directory))
    }
    pub fn get(&self, id: &str) -> Result<Installed> {
        self.list()?
            .into_iter()
            .find(|p| p.metadata.id() == id)
            .ok_or_else(|| "Plugin is not installed".into())
    }
    pub fn unlink_dev(&self, id: &str) -> Result<()> {
        let record = self.get(id)?;
        if record.dev_path.is_none() {
            return Err("Plugin is not linked in development mode".into());
        }
        self.remove(id)
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

fn read_dev_manifest(project_dir: &Path) -> Result<crate::manifest::Manifest> {
    let bytes = fs::read(project_dir.join("graphite-plugin.json")).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Manifest exceeds 1 MiB".into());
    }
    let manifest: crate::manifest::Manifest =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    manifest.validate()?;
    Ok(manifest)
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
