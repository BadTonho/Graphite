//! Local history (timeline): a copy of each file is kept in the profile when it is saved,
//! so earlier versions can be reopened even without Git commits.

use super::go_menu::GoSymbolItem;
use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// Versions kept per file; the oldest are removed when a new one is recorded.
pub(crate) const HISTORY_LIMIT: usize = 50;

/// Stable 64-bit FNV-1a hash, used to name a file's history folder.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// `YYYY-MM-DD HH:MM:SS UTC` for milliseconds since the Unix epoch.
pub(crate) fn format_utc(millis: u64) -> String {
    let seconds = (millis / 1000) as i64;
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60,
        time % 60
    )
}

impl App {
    /// Folder that holds the versions of one file.
    fn local_history_dir(&self, path: &Path) -> PathBuf {
        let key = fnv1a(path.to_string_lossy().as_bytes());
        self.profile_root.join("history").join(format!("{key:016x}"))
    }

    pub(crate) fn is_local_history_file(&self, path: &Path) -> bool {
        path.starts_with(self.profile_root.join("history"))
    }

    /// Stores the document's text as a new version, unless it equals the latest version.
    /// History is kept only for profiles; isolated sessions do not write to disk.
    pub(crate) fn record_local_history(&self, document: DocumentId) {
        if self.profiles.is_none() {
            return;
        }
        let Some(tab) = self.documents.get(document) else {
            return;
        };
        let Some(path) = tab.buffer.path().map(Path::to_path_buf) else {
            return;
        };
        let text = tab.buffer.text();
        let dir = self.local_history_dir(&path);
        let versions = list_versions(&dir);
        if let Some(latest) = versions.last() {
            if fs::read_to_string(latest).is_ok_and(|saved| saved == text) {
                return;
            }
        }
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_millis() as u64);
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = fs::write(dir.join("path.txt"), path.display().to_string());
        if fs::write(dir.join(format!("{millis:020}.txt")), text).is_err() {
            return;
        }
        let mut versions = list_versions(&dir);
        while versions.len() > HISTORY_LIMIT {
            let oldest = versions.remove(0);
            let _ = fs::remove_file(oldest);
        }
    }

    /// Opens the picker that lists the saved versions of the active file, newest first.
    pub(crate) fn open_local_history(&mut self) -> bool {
        let Some(path) = self.documents[self.active_document()]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            self.status = self.t("history.no_file").into();
            return false;
        };
        let mut versions = list_versions(&self.local_history_dir(&path));
        versions.reverse();
        if versions.is_empty() {
            self.status = self.t("history.none").into();
            return false;
        }
        let items = versions
            .into_iter()
            .map(|version| {
                let millis = version
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(|stem| stem.parse::<u64>().ok())
                    .unwrap_or(0);
                let lines = fs::read_to_string(&version).map_or(0, |text| text.lines().count());
                GoSymbolItem {
                    path: version,
                    line: 0,
                    name: format!("{}  ({lines} lines)", format_utc(millis)),
                    scope: String::new(),
                }
            })
            .collect();
        self.open_locations_picker(items);
        true
    }
}

/// Version files of one history folder, oldest first (names sort by time).
fn list_versions(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut versions: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .filter(|path| path.file_stem().is_some_and(|stem| stem != "path"))
        .collect();
    versions.sort();
    versions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_labels_are_calendar_correct() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(951_782_400_000), "2000-02-29 00:00:00 UTC");
        assert_eq!(format_utc(1_700_000_000_000), "2023-11-14 22:13:20 UTC");
    }

    #[test]
    fn history_records_changed_text_only_and_keeps_the_newest_versions() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("doc.txt");
        std::fs::write(&path, "one").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");
        app.profiles = Some(crate::app::phase5::Profiles::open(dir.path()).unwrap());
        app.open_file(path.clone());
        let id = app.active_document();

        app.record_local_history(id);
        app.record_local_history(id);
        let folder = app.local_history_dir(&path);
        assert_eq!(list_versions(&folder).len(), 1);

        app.editor_mut().insert("x");
        app.record_local_history(id);
        assert_eq!(list_versions(&folder).len(), 2);

        for _ in 0..HISTORY_LIMIT {
            app.editor_mut().insert("y");
            std::thread::sleep(std::time::Duration::from_millis(2));
            app.record_local_history(id);
        }
        assert_eq!(list_versions(&folder).len(), HISTORY_LIMIT);
        assert!(app.is_local_history_file(&list_versions(&folder)[0]));
    }

    #[test]
    fn opening_history_lists_versions_and_isolated_sessions_write_nothing() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("doc.txt");
        std::fs::write(&path, "one").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");
        app.open_file(path.clone());
        let id = app.active_document();
        app.record_local_history(id);
        assert!(!app.profile_root.exists());

        assert!(!app.open_local_history());
        assert!(app.status.len() > 0);
    }
}
