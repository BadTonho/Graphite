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
        self.profile_root
            .join("history")
            .join(format!("{key:016x}"))
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
        let mut millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_millis() as u64);
        // Two saves in the same millisecond must not overwrite each other's file.
        let latest_millis = versions
            .last()
            .and_then(|latest| latest.file_stem()?.to_str()?.parse::<u64>().ok());
        if let Some(latest) = latest_millis.filter(|latest| *latest >= millis) {
            millis = latest + 1;
        }
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

/// What picking a saved version of the active file does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryAction {
    Restore,
    Compare,
    Delete,
}

impl App {
    /// The saved versions of the active file, newest first, and the file they belong to.
    fn active_file_versions(&self) -> Option<(PathBuf, Vec<PathBuf>)> {
        let path = self.documents[self.command_document()]
            .buffer
            .path()?
            .to_path_buf();
        let mut versions = list_versions(&self.local_history_dir(&path));
        versions.reverse();
        Some((path, versions))
    }

    /// The saved versions of `file`, newest first.
    pub(crate) fn history_versions_of(&self, file: &Path) -> Vec<PathBuf> {
        let mut versions = list_versions(&self.local_history_dir(file));
        versions.reverse();
        versions
    }

    /// The file a saved version was made from, read from the note kept in its folder.
    pub(crate) fn history_origin(&self, version: &Path) -> Option<PathBuf> {
        if !self.is_local_history_file(version) {
            return None;
        }
        let note = fs::read_to_string(version.parent()?.join("path.txt")).ok()?;
        Some(PathBuf::from(note.trim()))
    }

    /// Opens the version picker for `action`; picking one runs it.
    pub(crate) fn open_history_picker(&mut self, action: HistoryAction) {
        if !self.open_local_history() {
            return;
        }
        if let Some(picker) = self.go_picker.as_mut() {
            picker.kind = match action {
                HistoryAction::Restore => super::go_menu::GoPickerKind::HistoryRestore,
                HistoryAction::Compare => super::go_menu::GoPickerKind::HistoryCompare,
                HistoryAction::Delete => super::go_menu::GoPickerKind::HistoryDelete,
            };
        }
    }

    /// Replaces the text of `target` with the text of a saved version, as one undoable edit.
    pub(crate) fn restore_history_version(&mut self, version: &Path, target: &Path) {
        let Ok(text) = fs::read_to_string(version) else {
            self.status = self.t("history.unreadable").into();
            return;
        };
        self.open_file(target.to_path_buf());
        {
            let mut editor = self.editor_mut();
            let length = editor.len_chars();
            editor.set_selection(0, length);
            editor.insert(&text);
            editor.set_selection(0, 0);
        }
        self.status = self.t("history.restored").into();
    }

    pub(crate) fn compare_history_version(&mut self, version: PathBuf, target: PathBuf) {
        self.open_diff_paths(version, target);
    }

    pub(crate) fn delete_history_version(&mut self, version: &Path) {
        if !self.is_local_history_file(version) {
            return;
        }
        match fs::remove_file(version) {
            Ok(()) => self.status = self.t("history.deleted").into(),
            Err(error) => self.status = error.to_string(),
        }
    }

    /// Compares the active file with its most recent saved version.
    pub(crate) fn compare_with_previous_version(&mut self) {
        let Some((path, versions)) = self.active_file_versions() else {
            self.status = self.t("history.no_file").into();
            return;
        };
        match versions.into_iter().next() {
            Some(latest) => self.compare_history_version(latest, path),
            None => self.status = self.t("history.none").into(),
        }
    }

    /// Saves a new version of the active file now.
    pub(crate) fn create_history_entry(&mut self) {
        let document = self.command_document();
        if self.documents[document].buffer.path().is_none() {
            self.status = self.t("history.no_file").into();
            return;
        }
        let before = self
            .active_file_versions()
            .map_or(0, |(_, versions)| versions.len());
        self.record_local_history(document);
        let after = self
            .active_file_versions()
            .map_or(0, |(_, versions)| versions.len());
        self.status = if after > before {
            self.t("history.created").into()
        } else {
            self.t("history.unchanged").into()
        };
    }

    /// Removes every saved version of the active file.
    pub(crate) fn delete_all_history(&mut self) {
        let Some((path, _)) = self.active_file_versions() else {
            self.status = self.t("history.no_file").into();
            return;
        };
        let _ = fs::remove_dir_all(self.local_history_dir(&path));
        self.status = self.t("history.all_deleted").into();
    }

    /// With a saved version open in the editor, puts its text back into the original file.
    pub(crate) fn restore_open_history_version(&mut self) {
        let Some(version) = self.documents[self.command_document()]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            return;
        };
        match self.history_origin(&version) {
            Some(origin) => self.restore_history_version(&version, &origin),
            None => self.status = self.t("history.not_a_version").into(),
        }
    }
}

/// Log files under the profile (`*.log`, up to two folders deep), newest first.
fn log_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                if depth < 2 {
                    walk(&path, depth + 1, out);
                }
            } else if path.extension().is_some_and(|ext| ext == "log") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(root, 0, &mut files);
    files.sort_by_key(|path| {
        std::cmp::Reverse(
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok(),
        )
    });
    files
}

impl App {
    /// Lists the log files of the profile; picking one opens it.
    pub(crate) fn open_log_picker(&mut self) {
        let items: Vec<super::go_menu::GoSymbolItem> = log_files(&self.profile_root)
            .into_iter()
            .map(|path| super::go_menu::GoSymbolItem {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                scope: path
                    .parent()
                    .map(|parent| parent.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path,
                line: 0,
            })
            .collect();
        if items.is_empty() {
            self.status = self.t("logs.none").into();
            return;
        }
        self.open_locations_picker(items);
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
        assert!(!app.status.is_empty());
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use crate::test_support::TempDir;

    /// An app with `doc.txt` open and a profile, so versions are written to disk.
    fn app() -> (App, TempDir, PathBuf) {
        let dir = TempDir::new();
        let path = dir.path().join("doc.txt");
        std::fs::write(&path, "one").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");
        app.profiles = Some(crate::app::phase5::Profiles::open(dir.path()).unwrap());
        app.open_file(path.clone());
        (app, dir, path)
    }

    fn text(app: &App) -> String {
        app.editor().text()
    }

    /// Records "one", then edits to "onex" and records that.
    fn two_versions(app: &mut App) {
        let document = app.active_document();
        app.record_local_history(document);
        app.editor_mut().insert("x");
        std::thread::sleep(std::time::Duration::from_millis(2));
        app.record_local_history(document);
    }

    fn version_count(app: &App, path: &Path) -> usize {
        list_versions(&app.local_history_dir(path)).len()
    }

    #[test]
    fn creating_an_entry_saves_a_version_once_and_reports_no_change_the_second_time() {
        let (mut app, _dir, path) = app();
        app.run_ui_command("workbench.action.localHistory.create");
        assert_eq!(version_count(&app, &path), 1);
        assert_eq!(app.status, app.t("history.created"));
        app.run_ui_command("workbench.action.localHistory.create");
        assert_eq!(version_count(&app, &path), 1);
        assert_eq!(app.status, app.t("history.unchanged"));
    }

    #[test]
    fn deleting_all_history_removes_every_version() {
        let (mut app, _dir, path) = app();
        two_versions(&mut app);
        assert_eq!(version_count(&app, &path), 2);
        app.run_ui_command("workbench.action.localHistory.deleteAll");
        assert_eq!(version_count(&app, &path), 0);
    }

    #[test]
    fn restoring_from_the_picker_puts_the_chosen_version_back_as_one_undo_step() {
        let (mut app, _dir, _path) = app();
        two_versions(&mut app);
        app.editor_mut().insert("y");
        let edited = text(&app);
        assert_ne!(edited, "one");
        app.run_ui_command("workbench.action.localHistory.restoreViaPicker");
        let picker = app.go_picker.as_mut().expect("the version picker");
        assert_eq!(picker.items.len(), 2, "newest first");
        picker.selected = 1;
        app.accept_go_picker();
        assert_eq!(text(&app), "one");
        assert_eq!(app.status, app.t("history.restored"));
        app.run_ui_command("undo");
        assert_eq!(text(&app), edited);
    }

    #[test]
    fn deleting_from_the_picker_removes_that_version_only() {
        let (mut app, _dir, path) = app();
        two_versions(&mut app);
        app.run_ui_command("workbench.action.localHistory.delete");
        app.go_picker.as_mut().unwrap().selected = 0;
        app.accept_go_picker();
        assert_eq!(version_count(&app, &path), 1);
        assert_eq!(app.status, app.t("history.deleted"));
    }

    #[test]
    fn restoring_through_an_open_version_writes_it_into_the_original_file() {
        let (mut app, _dir, path) = app();
        two_versions(&mut app);
        let oldest = list_versions(&app.local_history_dir(&path))[0].clone();
        app.open_file(oldest);
        app.run_ui_command("workbench.action.localHistory.restoreViaEditor");
        assert_eq!(text(&app), "one");
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(path.as_path())
        );
    }

    #[test]
    fn restoring_from_an_ordinary_file_says_it_is_not_a_version() {
        let (mut app, _dir, _path) = app();
        app.run_ui_command("workbench.action.localHistory.restoreViaEditor");
        assert_eq!(app.status, app.t("history.not_a_version"));
    }

    #[test]
    fn comparing_needs_a_saved_version_and_a_window() {
        let (mut app, _dir, _path) = app();
        app.run_ui_command("workbench.action.localHistory.compareWithPrevious");
        assert_eq!(app.status, app.t("history.none"));
        let document = app.active_document();
        app.record_local_history(document);
        app.run_ui_command("workbench.action.localHistory.compareWithPrevious");
        assert_eq!(
            app.status,
            app.t("scm.error.diff_needs_loop"),
            "it reached the diff"
        );
    }

    #[test]
    fn the_open_command_lists_the_versions() {
        let (mut app, _dir, _path) = app();
        two_versions(&mut app);
        app.run_ui_command("workbench.action.localHistory.open");
        assert_eq!(app.go_picker.as_ref().unwrap().items.len(), 2);
    }
}
