//! Applies staged updates (`updates/pending-install.json`) on Windows and Linux.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct PendingInstall {
    tag: String,
    staged_path: PathBuf,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct JustUpdatedMarker {
    pub tag: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub html_url: String,
}

fn target_binary_name() -> &'static str {
    if cfg!(windows) {
        "graphite.exe"
    } else {
        "graphite"
    }
}

fn backup_path_for(target_exe: &Path) -> PathBuf {
    let mut name = target_exe
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| target_binary_name().into());
    name.push(".old");
    target_exe.with_file_name(name)
}

fn temp_path_for(target_exe: &Path) -> PathBuf {
    let mut name = target_exe
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| target_binary_name().into());
    name.push(".new");
    target_exe.with_file_name(name)
}

/// Removes any leftover `.old` executable from a previous Windows update swap.
pub(crate) fn cleanup_old_binary(target_exe: &Path) {
    let old = backup_path_for(target_exe);
    if old.exists() {
        let _ = fs::remove_file(old);
    }
}

/// Extracts the Graphite binary from a `.zip` update package, or copies a raw binary,
/// into `dest_temp`.
fn stage_binary_to_temp(staged_path: &Path, dest_temp: &Path) -> Result<(), String> {
    let is_zip = staged_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
    if !is_zip {
        fs::copy(staged_path, dest_temp)
            .map(|_| ())
            .map_err(|err| format!("Failed copying staged binary: {err}"))?;
        return Ok(());
    }

    let file = fs::File::open(staged_path)
        .map_err(|err| format!("Failed opening staged zip {}: {err}", staged_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|err| format!("Invalid update zip archive: {err}"))?;
    let wanted = target_binary_name();

    let mut matched_index = None;
    for idx in 0..archive.len() {
        let entry = archive
            .by_index(idx)
            .map_err(|err| format!("Failed reading zip entry: {err}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let base = name.rsplit('/').next().unwrap_or(&name);
        if base.eq_ignore_ascii_case(wanted) {
            matched_index = Some(idx);
            break;
        }
    }

    let index =
        matched_index.ok_or_else(|| format!("Update archive does not contain '{wanted}'"))?;
    let mut entry = archive
        .by_index(index)
        .map_err(|err| format!("Failed opening '{wanted}' from update zip: {err}"))?;
    let mut out = fs::File::create(dest_temp)
        .map_err(|err| format!("Failed creating temp update binary: {err}"))?;
    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|err| format!("Failed reading '{wanted}' from update zip: {err}"))?;
    out.write_all(&buf)
        .map_err(|err| format!("Failed writing temp update binary: {err}"))?;
    Ok(())
}

/// Replaces `target_exe` with `temp_bin`. On Windows, renames the running `.exe` to `.old`
/// first because Windows locks running executables against overwrite.
fn replace_executable(temp_bin: &Path, target_exe: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(temp_bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(temp_bin, perms);
        }
        fs::rename(temp_bin, target_exe)
            .map_err(|err| format!("Failed replacing executable: {err}"))?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        let backup = backup_path_for(target_exe);
        let _ = fs::remove_file(&backup);
        if target_exe.exists() {
            fs::rename(target_exe, &backup)
                .map_err(|err| format!("Failed moving current executable aside: {err}"))?;
        }
        if let Err(err) = fs::rename(temp_bin, target_exe) {
            // Roll back if moving the new binary into place failed.
            if backup.exists() && !target_exe.exists() {
                let _ = fs::rename(&backup, target_exe);
            }
            return Err(format!("Failed moving new executable into place: {err}"));
        }
        Ok(())
    }
}

/// Checks `profile_root/updates/pending-install.json` and applies the staged update onto
/// `target_exe` if present. Returns `Ok(Some(tag))` when an update was applied.
pub(crate) fn apply_pending_update_in(
    profile_root: &Path,
    target_exe: &Path,
) -> Result<Option<String>, String> {
    cleanup_old_binary(target_exe);
    let marker_path = profile_root.join("updates").join("pending-install.json");
    let Ok(raw) = fs::read_to_string(&marker_path) else {
        return Ok(None);
    };
    let pending: PendingInstall =
        serde_json::from_str(&raw).map_err(|err| format!("Invalid pending-install.json: {err}"))?;
    if !pending.staged_path.is_file() {
        let _ = fs::remove_file(&marker_path);
        return Err(format!(
            "Staged update file is missing: {}",
            pending.staged_path.display()
        ));
    }

    let temp_bin = temp_path_for(target_exe);
    let _ = fs::remove_file(&temp_bin);
    if let Err(err) = stage_binary_to_temp(&pending.staged_path, &temp_bin) {
        let _ = fs::remove_file(&temp_bin);
        return Err(err);
    }

    if let Err(err) = replace_executable(&temp_bin, target_exe) {
        let _ = fs::remove_file(&temp_bin);
        return Err(err);
    }

    let just_updated = JustUpdatedMarker {
        tag: pending.tag.clone(),
        version: pending
            .version
            .unwrap_or_else(|| pending.tag.trim_start_matches('v').to_owned()),
        notes: pending.notes.unwrap_or_default(),
        html_url: pending.html_url.unwrap_or_default(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&just_updated) {
        let _ = fs::write(profile_root.join("updates").join("just-updated.json"), json);
    }

    let _ = fs::remove_file(&marker_path);
    Ok(Some(pending.tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_zip_with_binary(zip_path: &Path, entry_name: &str, bytes: &[u8]) {
        let file = fs::File::create(zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file(entry_name, options).unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn applies_raw_binary_and_cleans_pending_marker() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        let updates = profile.join("updates");
        fs::create_dir_all(&updates).unwrap();

        let target_exe = dir.path().join(target_binary_name());
        fs::write(&target_exe, b"old-binary").unwrap();

        let staged = updates.join(target_binary_name());
        fs::write(&staged, b"new-binary-v0.2.0").unwrap();
        fs::write(
            updates.join("pending-install.json"),
            serde_json::json!({
                "tag": "v0.2.0",
                "staged_path": staged,
            })
            .to_string(),
        )
        .unwrap();

        let applied = apply_pending_update_in(&profile, &target_exe).unwrap();
        assert_eq!(applied.as_deref(), Some("v0.2.0"));
        assert_eq!(fs::read(&target_exe).unwrap(), b"new-binary-v0.2.0");
        assert!(!updates.join("pending-install.json").exists());

        cleanup_old_binary(&target_exe);
        assert!(!backup_path_for(&target_exe).exists());
    }

    #[test]
    fn extracts_binary_from_zip_archive_and_replaces_target() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        let updates = profile.join("updates");
        fs::create_dir_all(&updates).unwrap();

        let target_exe = dir.path().join(target_binary_name());
        fs::write(&target_exe, b"old-binary").unwrap();

        let zip_path = updates.join("graphite-0.2.0.zip");
        let entry = format!("graphite-0.2.0/{}", target_binary_name());
        write_zip_with_binary(&zip_path, &entry, b"zip-binary-v0.2.0");
        fs::write(
            updates.join("pending-install.json"),
            serde_json::json!({
                "tag": "v0.2.0",
                "staged_path": zip_path,
            })
            .to_string(),
        )
        .unwrap();

        let applied = apply_pending_update_in(&profile, &target_exe).unwrap();
        assert_eq!(applied.as_deref(), Some("v0.2.0"));
        assert_eq!(fs::read(&target_exe).unwrap(), b"zip-binary-v0.2.0");
    }

    #[test]
    fn missing_staged_file_keeps_original_executable_intact() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        let updates = profile.join("updates");
        fs::create_dir_all(&updates).unwrap();

        let target_exe = dir.path().join(target_binary_name());
        fs::write(&target_exe, b"old-binary").unwrap();

        fs::write(
            updates.join("pending-install.json"),
            serde_json::json!({
                "tag": "v0.2.0",
                "staged_path": updates.join("missing.zip"),
            })
            .to_string(),
        )
        .unwrap();

        let err = apply_pending_update_in(&profile, &target_exe).unwrap_err();
        assert!(err.contains("missing"));
        assert_eq!(fs::read(&target_exe).unwrap(), b"old-binary");
        assert!(!updates.join("pending-install.json").exists());
    }

    #[test]
    fn writes_just_updated_marker_with_notes_when_update_applies() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        let updates = profile.join("updates");
        fs::create_dir_all(&updates).unwrap();

        let target_exe = dir.path().join(target_binary_name());
        fs::write(&target_exe, b"old-binary").unwrap();

        let staged = updates.join(target_binary_name());
        fs::write(&staged, b"new-binary-v0.2.0").unwrap();
        fs::write(
            updates.join("pending-install.json"),
            serde_json::json!({
                "tag": "v0.2.0",
                "version": "0.2.0",
                "notes": "## New features\n- Auto update changelog",
                "html_url": "https://github.com/BadTonho/Graphite/releases/tag/v0.2.0",
                "staged_path": staged,
            })
            .to_string(),
        )
        .unwrap();

        let tag = apply_pending_update_in(&profile, &target_exe).unwrap();
        assert_eq!(tag.as_deref(), Some("v0.2.0"));
        let marker_raw = fs::read_to_string(updates.join("just-updated.json")).unwrap();
        let marker: JustUpdatedMarker = serde_json::from_str(&marker_raw).unwrap();
        assert_eq!(marker.tag, "v0.2.0");
        assert_eq!(marker.version, "0.2.0");
        assert!(marker.notes.contains("Auto update changelog"));
    }
}
