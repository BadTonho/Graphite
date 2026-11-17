//! Applies staged updates (`updates/pending-install.json`) on Windows and Linux.

use ring::digest::{Context, SHA256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Upper bound for the binary extracted from an update archive.
const MAX_EXTRACTED_BINARY_BYTES: u64 = 1 << 30;

#[derive(serde::Deserialize)]
struct PendingInstall {
    tag: String,
    staged_path: PathBuf,
    /// SHA-256 of the staged package recorded when it was verified; re-checked before install.
    #[serde(default)]
    sha256: Option<String>,
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

/// Removes any leftover `.old` executable from a previous update swap. Called once the new
/// version has started successfully, which is when the rollback copy is no longer needed.
pub(crate) fn cleanup_old_binary(target_exe: &Path) {
    let old = backup_path_for(target_exe);
    if old.exists() {
        let _ = fs::remove_file(old);
    }
}

/// Restores the executable that was moved aside by the last update. Used when the freshly
/// installed binary cannot be launched.
pub(crate) fn rollback_update(target_exe: &Path) -> Result<(), String> {
    let backup = backup_path_for(target_exe);
    if !backup.is_file() {
        return Err("No previous executable is available to roll back to".into());
    }
    let broken = target_exe.with_extension("broken");
    let _ = fs::remove_file(&broken);
    if target_exe.exists() {
        fs::rename(target_exe, &broken)
            .map_err(|err| format!("Failed moving the new executable aside: {err}"))?;
    }
    if let Err(err) = fs::rename(&backup, target_exe) {
        if broken.exists() && !target_exe.exists() {
            let _ = fs::rename(&broken, target_exe);
        }
        return Err(format!("Failed restoring the previous executable: {err}"));
    }
    let _ = fs::remove_file(&broken);
    Ok(())
}

/// Streaming SHA-256 of a file (lowercase hex).
pub(crate) fn sha256_of_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|err| format!("Failed opening {}: {err}", path.display()))?;
    let mut ctx = Context::new(&SHA256);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|err| format!("Failed reading {}: {err}", path.display()))?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(ctx
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Rejects files that are not a native executable for the running platform, so a truncated or
/// mislabeled package can never replace a working editor.
fn validate_executable(path: &Path) -> Result<(), String> {
    let mut magic = [0u8; 4];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .map_err(|err| format!("Update binary is unreadable or too small: {err}"))?;
    let valid = if cfg!(windows) {
        magic.starts_with(b"MZ")
    } else if cfg!(target_os = "macos") {
        matches!(
            magic,
            [0xCF, 0xFA, 0xED, 0xFE] | [0xCA, 0xFE, 0xBA, 0xBE] | [0xFE, 0xED, 0xFA, 0xCF]
        )
    } else {
        magic == *b"\x7fELF"
    };
    if valid {
        Ok(())
    } else {
        Err("Update binary is not a valid executable for this platform".into())
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
        let name = entry
            .name()
            .map_err(|err| format!("Failed reading zip entry name: {err}"))?
            .replace('\\', "/");
        let base = name.rsplit('/').next().unwrap_or(&name);
        if base.eq_ignore_ascii_case(wanted) {
            matched_index = Some(idx);
            break;
        }
    }

    let index =
        matched_index.ok_or_else(|| format!("Update archive does not contain '{wanted}'"))?;
    let entry = archive
        .by_index(index)
        .map_err(|err| format!("Failed opening '{wanted}' from update zip: {err}"))?;
    if entry.size() > MAX_EXTRACTED_BINARY_BYTES {
        return Err(format!(
            "'{wanted}' in the update archive is unreasonably large"
        ));
    }
    let mut out = fs::File::create(dest_temp)
        .map_err(|err| format!("Failed creating temp update binary: {err}"))?;
    // `take` bounds the real output even if the archive lies about the entry size.
    let copied = std::io::copy(&mut entry.take(MAX_EXTRACTED_BINARY_BYTES + 1), &mut out)
        .map_err(|err| format!("Failed extracting '{wanted}' from update zip: {err}"))?;
    if copied > MAX_EXTRACTED_BINARY_BYTES {
        return Err(format!(
            "'{wanted}' in the update archive is unreasonably large"
        ));
    }
    out.flush()
        .map_err(|err| format!("Failed writing temp update binary: {err}"))?;
    Ok(())
}

/// Replaces `target_exe` with `temp_bin`, keeping the old executable as `.old` so the swap
/// can be rolled back. Windows locks a running `.exe` against overwrite but allows renaming
/// it, and Linux allows renaming over a running binary, so one strategy serves both.
fn replace_executable(temp_bin: &Path, target_exe: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(temp_bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(temp_bin, perms);
        }
    }

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

/// A staged package must live inside `<profile>/updates/`; the marker file is plain JSON on
/// disk and must not be able to point the installer at arbitrary files.
fn ensure_staged_inside_updates(profile_root: &Path, staged: &Path) -> Result<(), String> {
    let updates_dir = profile_root.join("updates");
    let canonical_updates = fs::canonicalize(&updates_dir)
        .map_err(|err| format!("Updates directory is unavailable: {err}"))?;
    let canonical_staged = fs::canonicalize(staged)
        .map_err(|err| format!("Staged update file is unavailable: {err}"))?;
    if canonical_staged.starts_with(&canonical_updates) {
        Ok(())
    } else {
        Err("Staged update file is outside the updates directory".into())
    }
}

/// Checks `profile_root/updates/pending-install.json` and applies the staged update onto
/// `target_exe` if present. Returns `Ok(Some(tag))` when an update was applied.
/// An invalid or unusable marker is discarded so it is not retried on every launch.
pub(crate) fn apply_pending_update_in(
    profile_root: &Path,
    target_exe: &Path,
) -> Result<Option<String>, String> {
    cleanup_old_binary(target_exe);
    let marker_path = profile_root.join("updates").join("pending-install.json");
    let Ok(raw) = fs::read_to_string(&marker_path) else {
        return Ok(None);
    };
    let pending: PendingInstall = match serde_json::from_str(&raw) {
        Ok(pending) => pending,
        Err(err) => {
            let _ = fs::remove_file(&marker_path);
            return Err(format!("Invalid pending-install.json: {err}"));
        }
    };
    let result = install_pending(profile_root, target_exe, &pending);
    // Whatever the outcome, the marker is consumed: a failed install must not loop forever.
    let _ = fs::remove_file(&marker_path);
    result?;

    let just_updated = JustUpdatedMarker {
        tag: pending.tag.clone(),
        version: pending
            .version
            .unwrap_or_else(|| pending.tag.trim_start_matches('v').to_owned()),
        notes: pending.notes.unwrap_or_default(),
        html_url: pending.html_url.unwrap_or_default(),
    };
    let marker_written = serde_json::to_string_pretty(&just_updated)
        .map_err(|err| err.to_string())
        .and_then(|json| {
            fs::write(profile_root.join("updates").join("just-updated.json"), json)
                .map_err(|err| err.to_string())
        });
    if let Err(err) = marker_written {
        crate::app::error_log::record(
            crate::app::error_log::ErrorSeverity::Low,
            "updates.changelog_marker",
            format!("Could not record the post-update changelog marker: {err}"),
        );
    }
    Ok(Some(pending.tag))
}

fn install_pending(
    profile_root: &Path,
    target_exe: &Path,
    pending: &PendingInstall,
) -> Result<(), String> {
    if !pending.staged_path.is_file() {
        return Err(format!(
            "Staged update file is missing: {}",
            pending.staged_path.display()
        ));
    }
    ensure_staged_inside_updates(profile_root, &pending.staged_path)?;
    if let Some(expected) = pending.sha256.as_deref() {
        let actual = sha256_of_file(&pending.staged_path)?;
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(
                "Staged update changed on disk after verification; refusing to install".into(),
            );
        }
    }

    let temp_bin = temp_path_for(target_exe);
    let _ = fs::remove_file(&temp_bin);
    let staged = stage_binary_to_temp(&pending.staged_path, &temp_bin)
        .and_then(|()| validate_executable(&temp_bin));
    if let Err(err) = staged {
        let _ = fs::remove_file(&temp_bin);
        return Err(err);
    }
    if let Err(err) = replace_executable(&temp_bin, target_exe) {
        let _ = fs::remove_file(&temp_bin);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal bytes that pass the platform executable-header check.
    fn fake_binary(payload: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = if cfg!(windows) {
            b"MZ\x90\x00".to_vec()
        } else if cfg!(target_os = "macos") {
            vec![0xCF, 0xFA, 0xED, 0xFE]
        } else {
            b"\x7fELF".to_vec()
        };
        bytes.extend_from_slice(payload.as_bytes());
        bytes
    }

    fn write_zip_with_binary(zip_path: &Path, entry_name: &str, bytes: &[u8]) {
        let file = fs::File::create(zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file(entry_name, options).unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap();
    }

    struct Fixture {
        _dir: crate::test_support::TempDir,
        profile: PathBuf,
        updates: PathBuf,
        target_exe: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        let updates = profile.join("updates");
        fs::create_dir_all(&updates).unwrap();
        let target_exe = dir.path().join(target_binary_name());
        fs::write(&target_exe, b"old-binary").unwrap();
        Fixture {
            _dir: dir,
            profile,
            updates,
            target_exe,
        }
    }

    fn write_marker(f: &Fixture, value: serde_json::Value) {
        fs::write(f.updates.join("pending-install.json"), value.to_string()).unwrap();
    }

    #[test]
    fn applies_raw_binary_and_cleans_pending_marker() {
        let f = fixture();
        let staged = f.updates.join(target_binary_name());
        let new_bin = fake_binary("new-binary-v0.2.0");
        fs::write(&staged, &new_bin).unwrap();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": staged }),
        );

        let applied = apply_pending_update_in(&f.profile, &f.target_exe).unwrap();
        assert_eq!(applied.as_deref(), Some("v0.2.0"));
        assert_eq!(fs::read(&f.target_exe).unwrap(), new_bin);
        assert!(!f.updates.join("pending-install.json").exists());
        assert_eq!(
            fs::read(backup_path_for(&f.target_exe)).unwrap(),
            b"old-binary"
        );

        cleanup_old_binary(&f.target_exe);
        assert!(!backup_path_for(&f.target_exe).exists());
    }

    #[test]
    fn extracts_binary_from_zip_archive_and_replaces_target() {
        let f = fixture();
        let zip_path = f.updates.join("graphite-0.2.0.zip");
        let entry = format!("graphite-0.2.0/{}", target_binary_name());
        let new_bin = fake_binary("zip-binary-v0.2.0");
        write_zip_with_binary(&zip_path, &entry, &new_bin);
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": zip_path }),
        );

        let applied = apply_pending_update_in(&f.profile, &f.target_exe).unwrap();
        assert_eq!(applied.as_deref(), Some("v0.2.0"));
        assert_eq!(fs::read(&f.target_exe).unwrap(), new_bin);
    }

    #[test]
    fn missing_staged_file_keeps_original_executable_intact() {
        let f = fixture();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": f.updates.join("missing.zip") }),
        );
        let err = apply_pending_update_in(&f.profile, &f.target_exe).unwrap_err();
        assert!(err.contains("missing"));
        assert_eq!(fs::read(&f.target_exe).unwrap(), b"old-binary");
        assert!(!f.updates.join("pending-install.json").exists());
    }

    #[test]
    fn non_executable_payload_is_rejected_and_original_survives() {
        let f = fixture();
        let staged = f.updates.join(target_binary_name());
        fs::write(&staged, b"<html>404 not found</html>").unwrap();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": staged }),
        );

        let err = apply_pending_update_in(&f.profile, &f.target_exe).unwrap_err();
        assert!(err.contains("not a valid executable"), "{err}");
        assert_eq!(fs::read(&f.target_exe).unwrap(), b"old-binary");
        assert!(!temp_path_for(&f.target_exe).exists());
        assert!(!f.updates.join("pending-install.json").exists());
    }

    #[test]
    fn staged_file_outside_updates_dir_is_refused() {
        let f = fixture();
        let outside = f.profile.join("elsewhere.bin");
        fs::write(&outside, fake_binary("evil")).unwrap();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": outside }),
        );

        let err = apply_pending_update_in(&f.profile, &f.target_exe).unwrap_err();
        assert!(err.contains("outside the updates directory"), "{err}");
        assert_eq!(fs::read(&f.target_exe).unwrap(), b"old-binary");
    }

    #[test]
    fn staged_file_modified_after_verification_is_refused() {
        let f = fixture();
        let staged = f.updates.join(target_binary_name());
        let good = fake_binary("good");
        fs::write(&staged, &good).unwrap();
        let digest = sha256_of_file(&staged).unwrap();
        fs::write(&staged, fake_binary("swapped")).unwrap();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": staged, "sha256": digest }),
        );

        let err = apply_pending_update_in(&f.profile, &f.target_exe).unwrap_err();
        assert!(err.contains("changed on disk"), "{err}");
        assert_eq!(fs::read(&f.target_exe).unwrap(), b"old-binary");

        // The same marker with the matching digest installs normally.
        fs::write(&staged, &good).unwrap();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": staged, "sha256": digest }),
        );
        assert!(apply_pending_update_in(&f.profile, &f.target_exe)
            .unwrap()
            .is_some());
    }

    #[test]
    fn invalid_marker_is_discarded_instead_of_retried_forever() {
        let f = fixture();
        fs::write(f.updates.join("pending-install.json"), "{not json").unwrap();
        assert!(apply_pending_update_in(&f.profile, &f.target_exe).is_err());
        assert!(!f.updates.join("pending-install.json").exists());
        assert_eq!(apply_pending_update_in(&f.profile, &f.target_exe), Ok(None));
    }

    #[test]
    fn rollback_restores_previous_executable() {
        let f = fixture();
        let staged = f.updates.join(target_binary_name());
        fs::write(&staged, fake_binary("new")).unwrap();
        write_marker(
            &f,
            serde_json::json!({ "tag": "v0.2.0", "staged_path": staged }),
        );
        apply_pending_update_in(&f.profile, &f.target_exe).unwrap();

        rollback_update(&f.target_exe).unwrap();
        assert_eq!(fs::read(&f.target_exe).unwrap(), b"old-binary");
        assert!(!backup_path_for(&f.target_exe).exists());
        assert!(rollback_update(&f.target_exe).is_err());
    }

    #[test]
    fn writes_just_updated_marker_with_notes_when_update_applies() {
        let f = fixture();
        let staged = f.updates.join(target_binary_name());
        fs::write(&staged, fake_binary("new-binary-v0.2.0")).unwrap();
        write_marker(
            &f,
            serde_json::json!({
                "tag": "v0.2.0",
                "version": "0.2.0",
                "notes": "## New features\n- Auto update changelog",
                "html_url": "https://github.com/BadTonho/Graphite/releases/tag/v0.2.0",
                "staged_path": staged,
            }),
        );

        let tag = apply_pending_update_in(&f.profile, &f.target_exe).unwrap();
        assert_eq!(tag.as_deref(), Some("v0.2.0"));
        let marker_raw = fs::read_to_string(f.updates.join("just-updated.json")).unwrap();
        let marker: JustUpdatedMarker = serde_json::from_str(&marker_raw).unwrap();
        assert_eq!(marker.tag, "v0.2.0");
        assert_eq!(marker.version, "0.2.0");
        assert!(marker.notes.contains("Auto update changelog"));
    }
}
