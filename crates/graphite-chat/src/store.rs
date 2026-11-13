//! The saved model service settings of a profile.

use crate::provider::ProviderConfig;
use std::fs;
use std::path::Path;

/// File name of the saved settings, inside the profile folder.
pub const SETTINGS_FILE: &str = "chat-provider.json";

/// Writes the settings into `dir`. The file holds the API key, so on Unix it is readable only
/// by its owner.
pub fn save_provider(dir: &Path, config: &ProviderConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let path = dir.join(SETTINGS_FILE);
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    restrict_to_owner(&path)
}

/// The saved settings in `dir`, if there is a readable file.
pub fn load_provider(dir: &Path) -> Option<ProviderConfig> {
    let bytes = fs::read(dir.join(SETTINGS_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Forgets the saved settings, key included. A missing file is not an error.
pub fn delete_provider(dir: &Path) -> Result<(), String> {
    match fs::remove_file(dir.join(SETTINGS_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> Result<(), String> {
    // Windows files inherit the ACL of the user's profile folder, which only that user can read.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("graphite-chat-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn config() -> ProviderConfig {
        ProviderConfig {
            kind: ProviderKind::Anthropic,
            base_url: String::new(),
            model: "m".into(),
            api_key: Some("sk-secret".into()),
        }
    }

    #[test]
    fn settings_round_trip_and_delete_is_idempotent() {
        let dir = temp_dir("roundtrip");
        assert_eq!(load_provider(&dir), None);
        save_provider(&dir, &config()).unwrap();
        assert_eq!(load_provider(&dir), Some(config()));
        delete_provider(&dir).unwrap();
        assert_eq!(load_provider(&dir), None);
        delete_provider(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_file_reads_as_no_settings() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SETTINGS_FILE), "{not json").unwrap();
        assert_eq!(load_provider(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn the_saved_file_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("mode");
        save_provider(&dir, &config()).unwrap();
        let mode = fs::metadata(dir.join(SETTINGS_FILE)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(&dir);
    }
}
