use crate::{manifest::*, Result, MAX_DOWNLOAD};
use std::{collections::HashSet, fs, io::Read, path::Path};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
const MAX_EXPANDED: u64 = 500 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;

// Treat both Windows and POSIX separators consistently, even on Linux.
pub fn safe_path(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':', '\0'])
        && name.split('/').all(|p| {
            !p.is_empty()
                && p != "."
                && p != ".."
                && !p.ends_with(['.', ' '])
                && ![
                    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
                    "LPT8", "LPT9",
                ]
                .contains(
                    &p.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                )
        })
}
pub fn inspect(path: &Path) -> Result<Metadata> {
    with_archive(path, |zip| metadata(zip, path))
}
fn with_archive<T>(
    path: &Path,
    f: impl FnOnce(&mut ZipArchive<fs::File>) -> Result<T>,
) -> Result<T> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_DOWNLOAD {
        return Err("Package exceeds 100 MiB".into());
    }
    let mut zip = ZipArchive::new(file).map_err(|e| e.to_string())?;
    if zip.len() > MAX_ENTRIES {
        return Err("Package has too many entries".into());
    }
    let mut size = 0u64;
    let mut names = HashSet::new();
    let mut files = HashSet::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().trim_end_matches('/');
        let key = name.to_ascii_lowercase();
        if !safe_path(name) || !names.insert(key.clone()) {
            return Err(format!("Unsafe or duplicate archive path: {name}"));
        }
        if entry
            .unix_mode()
            .is_some_and(|m| ![0, 0o100000, 0o040000].contains(&(m & 0o170000)))
        {
            return Err("Links and special files are forbidden".into());
        }
        size = size
            .checked_add(entry.size())
            .ok_or("Package size overflow")?;
        if size > MAX_EXPANDED {
            return Err("Package exceeds 500 MiB expanded".into());
        }
        if !entry.is_dir() {
            files.insert(key);
        }
    }
    for name in &names {
        let mut parent = name.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if files.contains(prefix) {
                return Err("Archive file/directory collision".into());
            }
            parent = prefix;
        }
    }
    f(&mut zip)
}
fn json(zip: &mut ZipArchive<fs::File>, name: &str) -> Result<serde_json::Value> {
    let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
    if entry.size() > 1024 * 1024 {
        return Err("Manifest exceeds 1 MiB".into());
    }
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Manifest exceeds 1 MiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn metadata(zip: &mut ZipArchive<fs::File>, path: &Path) -> Result<Metadata> {
    match path.extension().and_then(|x| x.to_str()) {
        Some("graphite-plugin") => {
            let m: Manifest = serde_json::from_value(json(zip, "graphite-plugin.json")?)
                .map_err(|e| e.to_string())?;
            m.validate()?;
            let has_entry = zip.by_name(&m.main).map(|e| !e.is_dir()).unwrap_or(false)
                || (m.runtime == crate::manifest::PluginRuntime::Native
                    && !m.main.ends_with(".exe")
                    && zip
                        .by_name(&format!("{}.exe", m.main))
                        .map(|e| !e.is_dir())
                        .unwrap_or(false));
            if !has_entry {
                return Err("Missing plugin entrypoint".into());
            }
            Ok(Metadata::Native(m))
        }
        Some("vsix") => {
            let m = json(zip, "extension/package.json")?;
            let text = |key| {
                m.get(key)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("Missing VSIX {key}"))
            };
            let id = format!("{}.{}", text("publisher")?, text("name")?).to_ascii_lowercase();
            if !valid_id(&id) {
                return Err("Invalid VSIX identity".into());
            }
            let version = text("version")?;
            semver::Version::parse(&version).map_err(|e| e.to_string())?;
            Ok(Metadata::Vsix(Vsix {
                id,
                version,
                description: text("description").unwrap_or_default(),
                engines: m.get("engines").cloned().unwrap_or_default(),
                main: text("main").ok(),
                commands: vsix_commands(&m),
                activity_bar: vsix_activity_bar(&m),
                snippets: vsix_snippets(&m),
                debuggers: vsix_debuggers(&m),
                activation_events: vsix_activation_events(&m),
                keybindings: vsix_keybindings(&m),
                menus: vsix_menus(&m),
                custom_editors: vsix_custom_editors(&m),
                notebooks: vsix_notebooks(&m),
                language_comments: vsix_language_comments(&m, |path| {
                    json(zip, &format!("extension/{path}")).ok()
                }),
                contributions: m
                    .get("contributes")
                    .and_then(|v| v.as_object())
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default(),
            }))
        }
        _ => Err("Expected .graphite-plugin or .vsix package".into()),
    }
}
pub fn extract(path: &Path, destination: &Path) -> Result<Metadata> {
    with_archive(path, |zip| {
        let m = metadata(zip, path)?;
        fs::create_dir(destination).map_err(|e| e.to_string())?;
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let out = destination.join(entry.name());
            if entry.is_dir() {
                fs::create_dir_all(out).map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out)
                .map_err(|e| e.to_string())?;
            let n = std::io::copy(
                &mut entry.by_ref().take(MAX_EXPANDED - total + 1),
                &mut file,
            )
            .map_err(|e| e.to_string())?;
            total += n;
            if total > MAX_EXPANDED || n != entry.size() {
                return Err("Invalid expanded package size".into());
            }
            drop(file);
            #[cfg(unix)]
            if let Metadata::Native(ref manifest) = m {
                if manifest.runtime == crate::manifest::PluginRuntime::Native
                    && entry.name() == manifest.main
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&out, fs::Permissions::from_mode(0o755));
                }
            }
        }
        Ok(m)
    })
}
pub fn pack(source: &Path) -> Result<std::path::PathBuf> {
    let canonical = source.canonicalize().map_err(|e| e.to_string())?;
    let source = canonical.as_path();
    let m: Manifest = serde_json::from_slice(
        &fs::read(source.join("graphite-plugin.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    m.validate()?;
    let destination = source.with_file_name(format!("{}-{}.graphite-plugin", m.id, m.version));
    let temp = destination.with_extension("packing");
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut zip = ZipWriter::new(file);
        fn walk(
            root: &Path,
            dir: &Path,
            zip: &mut ZipWriter<fs::File>,
            count: &mut usize,
            total: &mut u64,
        ) -> Result<()> {
            for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || name == "target" {
                    continue;
                }
                let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if meta.file_type().is_symlink() {
                    return Err("Cannot package symlinks".into());
                }
                if meta.is_dir() {
                    walk(root, &path, zip, count, total)?;
                } else if meta.is_file() {
                    let name = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .ok_or("Non UTF-8 package path")?
                        .replace('\\', "/");
                    if !safe_path(&name)
                        || ["pem", "key", "p12", "pfx", "db", "sqlite"]
                            .iter()
                            .any(|ext| name.ends_with(&format!(".{ext}")))
                    {
                        return Err("Unsafe or sensitive package path".into());
                    }
                    *count += 1;
                    *total += meta.len();
                    if *count > MAX_ENTRIES || *total > MAX_EXPANDED {
                        return Err("Package limits exceeded".into());
                    }
                    zip.start_file(
                        name,
                        SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Deflated),
                    )
                    .map_err(|e| e.to_string())?;
                    std::io::copy(&mut fs::File::open(path).map_err(|e| e.to_string())?, zip)
                        .map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }
        walk(source, source, &mut zip, &mut 0, &mut 0)?;
        zip.finish().map_err(|e| e.to_string())?;
        if temp.metadata().map_err(|e| e.to_string())?.len() > MAX_DOWNLOAD {
            return Err("Package exceeds 100 MiB".into());
        }
        // Validate using its final extension before publishing.
        with_archive(&temp, |z| metadata(z, &destination))?;
        if destination.exists() {
            return Err("Output package already exists".into());
        }
        fs::rename(&temp, &destination).map_err(|e| e.to_string())?;
        Ok(destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
