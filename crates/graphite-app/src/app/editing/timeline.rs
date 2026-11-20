//! The timeline of the active file: its saved local history versions together with the Git
//! commits that touched it, newest first. Picking an entry opens a diff with the current file.

use crate::process_util::HideConsole;
use super::go_menu::{GoPicker, GoPickerKind, GoSymbolItem};
use super::local_history::format_utc;
use super::*;
use std::process::Command;

/// How many commits of the file are listed.
const COMMIT_LIMIT: usize = 30;

/// A commit that touched a file: abbreviated hash, author time (seconds) and subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileCommit {
    pub hash: String,
    pub seconds: u64,
    pub subject: String,
}

/// Parses `git log --format=%h%x09%at%x09%s` lines.
pub(crate) fn parse_commits(output: &str) -> Vec<FileCommit> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let hash = parts.next()?.trim();
            let seconds = parts.next()?.trim().parse().ok()?;
            let subject = parts.next().unwrap_or_default().trim();
            (!hash.is_empty()).then(|| FileCommit {
                hash: hash.to_owned(),
                seconds,
                subject: subject.to_owned(),
            })
        })
        .collect()
}

/// `file` relative to `root`, with forward slashes. Either path may be in canonical form
/// (the verbatim prefix on Windows), so every pairing is tried.
fn relative_to(file: &Path, root: &Path) -> Option<String> {
    let canonical_file = file.canonicalize().ok();
    let canonical_root = root.canonicalize().ok();
    let pairings = [
        Some((file, root)),
        canonical_file.as_deref().map(|file| (file, root)),
        canonical_root.as_deref().map(|root| (file, root)),
        canonical_file.as_deref().zip(canonical_root.as_deref()),
    ];
    let relative = pairings
        .into_iter()
        .flatten()
        .find_map(|(file, root)| file.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    relative
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .hide_console()
        .current_dir(root)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

impl App {
    /// The file the timeline shows: the one it was pinned to when following is off, else the
    /// active file.
    fn timeline_file(&self) -> Option<PathBuf> {
        if !self.timeline_follow {
            if let Some(pinned) = &self.timeline_pinned {
                return Some(pinned.clone());
            }
        }
        self.documents[self.command_document()]
            .buffer
            .path()
            .map(Path::to_path_buf)
    }

    /// Opens the timeline picker for the active (or pinned) file.
    pub(crate) fn open_timeline(&mut self) {
        let Some(file) = self.timeline_file() else {
            self.status = self.t("history.no_file").into();
            return;
        };
        let mut entries: Vec<(u64, GoSymbolItem)> = Vec::new();
        for version in self.history_versions_of(&file) {
            let millis = version
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u64>().ok())
                .unwrap_or(0);
            entries.push((
                millis / 1000,
                GoSymbolItem {
                    path: version,
                    line: 0,
                    name: format!("{}  {}", format_utc(millis), self.t("timeline.saved")),
                    scope: String::new(),
                },
            ));
        }
        entries.extend(self.commit_entries(&file));
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        let items: Vec<GoSymbolItem> = entries.into_iter().map(|(_, item)| item).collect();
        let message = items
            .is_empty()
            .then(|| self.t("timeline.empty").to_owned());
        self.timeline_target = Some(file);
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        self.go_picker = Some(GoPicker {
            kind: GoPickerKind::Timeline,
            query: String::new(),
            items,
            selected: 0,
            loading: false,
            generation: self.go_picker_generation,
            message,
        });
    }

    /// The commits of `file`, each with a copy of the file as it was saved under the profile so
    /// the entry can be opened and compared like a local history version.
    fn commit_entries(&self, file: &Path) -> Vec<(u64, GoSymbolItem)> {
        let root = self.tree.root_for(file).to_path_buf();
        let Some(relative) = relative_to(file, &root) else {
            return Vec::new();
        };
        let limit = format!("-n{COMMIT_LIMIT}");
        let Some(log) = git(
            &root,
            &["log", &limit, "--format=%h%x09%at%x09%s", "--", &relative],
        ) else {
            return Vec::new();
        };
        let folder = self.profile_root.join("timeline");
        if std::fs::create_dir_all(&folder).is_err() {
            return Vec::new();
        }
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        parse_commits(&log)
            .into_iter()
            .filter_map(|commit| {
                let content = git(&root, &["show", &format!("{}:{relative}", commit.hash)])?;
                let copy = folder.join(format!("{}-{name}", commit.hash));
                std::fs::write(&copy, content).ok()?;
                Some((
                    commit.seconds,
                    GoSymbolItem {
                        path: copy,
                        line: 0,
                        name: format!(
                            "{}  {}  {}",
                            format_utc(commit.seconds * 1000),
                            commit.hash,
                            commit.subject
                        ),
                        scope: String::new(),
                    },
                ))
            })
            .collect()
    }

    /// Compares the picked timeline entry with the file the timeline is for.
    pub(crate) fn compare_timeline_entry(&mut self, entry: PathBuf) {
        match self.timeline_target.clone() {
            Some(file) => self.open_diff_paths(entry, file),
            None => self.status = self.t("history.no_origin").into(),
        }
    }

    pub(crate) fn toggle_timeline_follow(&mut self) {
        self.timeline_follow = !self.timeline_follow;
        self.timeline_pinned = if self.timeline_follow {
            None
        } else {
            self.documents[self.command_document()]
                .buffer
                .path()
                .map(Path::to_path_buf)
        };
        self.status = self
            .t(if self.timeline_follow {
                "timeline.following"
            } else {
                "timeline.pinned"
            })
            .into();
    }

    /// Opens the entry that is selected in an open timeline, or the timeline itself.
    pub(crate) fn timeline_compare_selected(&mut self) {
        if self
            .go_picker
            .as_ref()
            .is_some_and(|picker| picker.kind == GoPickerKind::Timeline)
        {
            self.accept_go_picker();
        } else {
            self.open_timeline();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn commit_lines_are_parsed_and_bad_lines_skipped() {
        let log = "abc1234\t1700000000\tfix a bug\ndef5678\t1699999000\tsubject\twith tab\nbroken\nxyz\tnotatime\tx\n";
        assert_eq!(
            parse_commits(log),
            vec![
                FileCommit {
                    hash: "abc1234".into(),
                    seconds: 1_700_000_000,
                    subject: "fix a bug".into()
                },
                FileCommit {
                    hash: "def5678".into(),
                    seconds: 1_699_999_000,
                    subject: "subject\twith tab".into()
                },
            ]
        );
    }

    fn sh(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .args(["-c", "core.autocrlf=false"])
            .args(args)
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "git {args:?}");
    }

    /// A repository with two commits of `a.txt`, opened in an app with a profile.
    fn app_with_history() -> (App, TempDir, PathBuf) {
        let dir = TempDir::new();
        let root = dir.path();
        sh(root, &["init", "-q"]);
        let file = root.join("a.txt");
        std::fs::write(&file, "one\n").unwrap();
        sh(root, &["add", "a.txt"]);
        sh(root, &["commit", "-q", "-m", "first"]);
        std::fs::write(&file, "two\n").unwrap();
        sh(root, &["commit", "-q", "-am", "second"]);
        let mut app = App::new_with_options(root.into(), None, true);
        app.profile_root = root.join("profile");
        app.profiles = Some(crate::app::phase5::Profiles::open(root).unwrap());
        app.open_file(file.clone());
        (app, dir, file)
    }

    #[test]
    fn the_timeline_lists_commits_and_saved_versions_newest_first() {
        let (mut app, _dir, _file) = app_with_history();
        let document = app.active_document();
        app.record_local_history(document);
        app.run_ui_command("timeline.focus");
        let picker = app.go_picker.as_ref().expect("the timeline picker");
        assert_eq!(picker.kind, GoPickerKind::Timeline);
        let names: Vec<_> = picker.items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names.iter().any(|name| name.contains("second")));
        assert!(names.iter().any(|name| name.contains("first")));
        assert!(names
            .iter()
            .any(|name| name.contains(app.t("timeline.saved"))));
        // A commit entry is a copy of the file as it was.
        let first = picker
            .items
            .iter()
            .find(|i| i.name.contains("first"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&first.path)
                .unwrap()
                .replace("\r\n", "\n"),
            "one\n"
        );
    }

    #[test]
    fn picking_an_entry_compares_it_with_the_file() {
        let (mut app, _dir, file) = app_with_history();
        app.run_ui_command("timeline.focus");
        assert_eq!(app.timeline_target.as_deref(), Some(file.as_path()));
        app.run_ui_command("timeline.compareSelected");
        // Tests have no event loop, so reaching the diff loader is the observable result.
        assert_eq!(app.status, app.t("scm.error.diff_needs_loop"));
    }

    #[test]
    fn refresh_opens_it_again_and_an_untracked_file_has_only_saved_versions() {
        let (mut app, dir, _file) = app_with_history();
        let other = dir.path().join("untracked.txt");
        std::fs::write(&other, "x").unwrap();
        app.open_file(other);
        app.run_ui_command("timeline.refresh");
        let picker = app.go_picker.as_ref().unwrap();
        assert!(picker.items.is_empty());
        assert_eq!(picker.message.as_deref(), Some(app.t("timeline.empty")));
    }

    #[test]
    fn following_can_be_switched_off_to_keep_showing_one_file() {
        let (mut app, dir, file) = app_with_history();
        app.run_ui_command("timeline.toggleFollowActiveEditor");
        assert!(!app.timeline_follow);
        assert_eq!(app.status, app.t("timeline.pinned"));
        let other = dir.path().join("other.txt");
        std::fs::write(&other, "x").unwrap();
        app.open_file(other);
        app.run_ui_command("timeline.focus");
        assert_eq!(
            app.timeline_target.as_deref(),
            Some(file.as_path()),
            "still the pinned file"
        );
        app.run_ui_command("timeline.toggleFollowActiveEditor");
        assert!(app.timeline_follow);
        assert_eq!(app.status, app.t("timeline.following"));
    }
}
