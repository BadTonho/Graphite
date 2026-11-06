//! Workspace folder model: a lazily-loaded file tree that respects `.gitignore`.

mod quick_open;
mod search;

use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let requested_root = root.as_ref().to_path_buf();
        if !requested_root.is_dir() {
            return Err(WorkspaceError::NotADirectory(requested_root));
        }
        let root = requested_root.canonicalize().unwrap_or(requested_root);
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lists the direct children of `dir`, directories first, then alphabetical.
    /// Honors `.gitignore`; hidden files are shown except `.git`.
    pub fn list_dir(&self, dir: &Path) -> Vec<Entry> {
        let mut entries: Vec<Entry> = WalkBuilder::new(dir)
            .max_depth(Some(1))
            .standard_filters(true)
            .hidden(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build()
            .filter_map(Result::ok)
            .filter(|e| e.depth() == 1)
            .map(|e| Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: e.file_type().is_some_and(|t| t.is_dir()),
                path: e.into_path(),
            })
            .collect();
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lists_dirs_first_and_skips_git() {
        let dir = std::env::temp_dir().join("shoppe_ws_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("zdir")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join("a.txt"), "").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let names: Vec<_> = ws.list_dir(&dir).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["zdir", "a.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_non_directory() {
        assert!(Workspace::open("/definitely/not/here").is_err());
    }

    #[test]
    fn searches_files_inside_workspace() {
        let dir = std::env::temp_dir().join("shoppe_ws_search_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.rs"), "first needle needle\n🌎 needle\n").unwrap();
        fs::write(dir.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(dir.join("ignored.txt"), "needle\n").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let results = ws.search("needle");
        assert_eq!(results.len(), 3, "{results:?}");
        assert_eq!(results[0].line, 0);
        assert_eq!(&results[0].text[results[0].start..results[0].end], "needle");
        assert_eq!(results[2].line, 1);
        assert_eq!(&results[2].text[results[2].start..results[2].end], "needle");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn nested_listing_honors_workspace_ignore_rules() {
        let dir = std::env::temp_dir().join(format!("shoppe_ws_nested_{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join(".gitignore"), "src/ignored.txt\n").unwrap();
        fs::write(dir.join("src/ignored.txt"), "ignored").unwrap();
        fs::write(dir.join("src/visible.txt"), "visible").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let entries = ws.list_dir(&ws.root().join("src"));
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["visible.txt"]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancelled_search_does_not_scan_files() {
        let dir = std::env::temp_dir().join(format!("shoppe_cancel_search_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("file.txt"), "needle").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        assert!(ws.search_cancellable("needle", || true).is_empty());
        assert_eq!(ws.search("needle").len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancellation_interrupts_scanning_a_large_file_without_matches() {
        let dir = std::env::temp_dir().join(format!("shoppe_cancel_large_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("large.txt");
        fs::write(
            &path,
            format!(
                "{}needle\n",
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n".repeat(200_000)
            ),
        )
        .unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let checks = std::cell::Cell::new(0);
        let results = ws.search_cancellable("needle", || {
            checks.set(checks.get() + 1);
            checks.get() >= 5
        });
        assert!(results.is_empty());
        assert!(checks.get() >= 5);
        assert_eq!(ws.search("needle").len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fuzzy_open_ranks_files_and_honors_ignored_paths() {
        let dir = std::env::temp_dir().join(format!("shoppe_quick_open_{}", std::process::id()));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(dir.join("ignored.rs"), "").unwrap();
        fs::write(dir.join("src/main.rs"), "").unwrap();
        fs::write(dir.join("src/model.rs"), "").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let paths = ws.fuzzy_files("mnrs", 10, || false);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("main.rs"));
        assert!(ws.fuzzy_files("ignored", 10, || false).is_empty());
        assert!(ws.fuzzy_files("main", 10, || true).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn search_prefers_unsaved_content_without_including_ignored_files() {
        let dir =
            std::env::temp_dir().join(format!("shoppe_overlay_search_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(dir.join("visible.txt"), "disk needle").unwrap();
        fs::write(dir.join("ignored.txt"), "needle").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let overlays = std::collections::HashMap::from([
            (
                dir.join("visible.txt").canonicalize().unwrap(),
                "new 🌎 needle".to_owned(),
            ),
            (
                dir.join("ignored.txt").canonicalize().unwrap(),
                "needle".to_owned(),
            ),
        ]);
        let result = ws.search_with_overlays("needle", &overlays, || false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "new 🌎 needle");
        assert!(ws
            .search_with_overlays("disk", &overlays, || false)
            .is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
}
