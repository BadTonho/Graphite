//! Workspace folder model: a lazily-loaded file tree that respects `.gitignore`.

mod find;
mod quick_open;
mod search;

pub use quick_open::FileIndex;
pub use search::{parse_glob_list, validate_search, SearchOptions};

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

#[derive(Debug, Clone)]
pub struct Workspace {
    roots: Vec<PathBuf>,
    workspace_file: Option<PathBuf>,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let requested_root = root.as_ref().to_path_buf();
        if !requested_root.is_dir() {
            return Err(WorkspaceError::NotADirectory(requested_root));
        }
        let root = requested_root.canonicalize().unwrap_or(requested_root);
        Ok(Self {
            roots: vec![root],
            workspace_file: None,
        })
    }

    pub fn from_roots(roots: impl IntoIterator<Item = PathBuf>) -> Result<Self, WorkspaceError> {
        let mut canonical_roots = Vec::new();
        for requested in roots {
            if !requested.is_dir() {
                return Err(WorkspaceError::NotADirectory(requested));
            }
            let root = requested.canonicalize().unwrap_or(requested);
            if !canonical_roots.contains(&root) {
                canonical_roots.push(root);
            }
        }
        if canonical_roots.is_empty() {
            return Err(WorkspaceError::NotADirectory(PathBuf::new()));
        }
        Ok(Self {
            roots: canonical_roots,
            workspace_file: None,
        })
    }

    pub fn open_code_workspace(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&source)
            .map_err(|error| format!("invalid workspace file: {error}"))?;
        let folders = value
            .get("folders")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "workspace file must contain a folders array".to_owned())?;
        let base = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .unwrap_or_else(|_| {
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            });
        let roots = folders
            .iter()
            .map(|folder| {
                folder
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "each workspace folder must have a path".to_owned())
                    .map(|folder_path| {
                        let folder_path = PathBuf::from(folder_path);
                        if folder_path.is_absolute() {
                            folder_path
                        } else {
                            base.join(folder_path)
                        }
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut workspace = Self::from_roots(roots).map_err(|error| error.to_string())?;
        workspace.workspace_file = Some(path.to_path_buf());
        Ok(workspace)
    }

    pub fn save_code_workspace(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        let base = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .unwrap_or_else(|_| {
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            });
        let folders = self
            .roots
            .iter()
            .map(|root| {
                let path = root
                    .strip_prefix(&base)
                    .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| root.display().to_string());
                serde_json::json!({"path": path})
            })
            .collect::<Vec<_>>();
        let contents = serde_json::to_vec_pretty(&serde_json::json!({"folders": folders}))
            .map_err(|error| error.to_string())?;
        std::fs::write(path, contents).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn workspace_file(&self) -> Option<&Path> {
        self.workspace_file.as_deref()
    }

    pub fn add_root(&mut self, root: impl AsRef<Path>) -> Result<bool, WorkspaceError> {
        let requested = root.as_ref().to_path_buf();
        if !requested.is_dir() {
            return Err(WorkspaceError::NotADirectory(requested));
        }
        let root = requested.canonicalize().unwrap_or(requested);
        if self.roots.contains(&root) {
            return Ok(false);
        }
        self.roots.push(root);
        Ok(true)
    }

    pub fn remove_root(&mut self, root: impl AsRef<Path>) -> bool {
        if self.roots.len() <= 1 {
            return false;
        }
        let requested_path = root.as_ref();
        let requested = requested_path
            .canonicalize()
            .unwrap_or_else(|_| requested_path.to_path_buf());
        let Some(index) = self
            .roots
            .iter()
            .position(|existing| existing == &requested)
        else {
            return false;
        };
        self.roots.remove(index);
        if self.roots.len() == 1 {
            self.workspace_file = None;
        }
        true
    }

    pub fn root(&self) -> &Path {
        &self.roots[0]
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
        let dir = std::env::temp_dir().join("graphite_ws_test");
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
    fn code_workspace_round_trips_multiple_relative_roots() {
        let dir =
            std::env::temp_dir().join(format!("graphite_workspace_file_{}", std::process::id()));
        let first = dir.join("first");
        let second = dir.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(first.join("one.txt"), "needle one").unwrap();
        std::fs::write(second.join("two.txt"), "needle two").unwrap();
        let workspace = Workspace::from_roots([first.clone(), second.clone()]).unwrap();
        let file = dir.join("project.code-workspace");
        workspace.save_code_workspace(&file).unwrap();

        let reopened = Workspace::open_code_workspace(&file).unwrap();
        assert_eq!(reopened.roots(), workspace.roots());
        let fuzzy = reopened.fuzzy_files("two", 10, || false);
        assert_eq!(fuzzy.len(), 1);
        assert_eq!(fuzzy[0].file_name().unwrap(), "two.txt");
        assert_eq!(reopened.search("needle").len(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn multi_root_workspace_can_remove_roots_but_keeps_one() {
        let dir =
            std::env::temp_dir().join(format!("graphite_workspace_remove_{}", std::process::id()));
        let first = dir.join("first");
        let second = dir.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let mut workspace = Workspace::from_roots([first.clone(), second.clone()]).unwrap();
        assert!(workspace.remove_root(&first));
        assert_eq!(workspace.roots().len(), 1);
        assert!(!workspace.remove_root(&second));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn searches_files_inside_workspace() {
        let dir = std::env::temp_dir().join("graphite_ws_search_test");
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
        let dir = std::env::temp_dir().join(format!("graphite_ws_nested_{}", std::process::id()));
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
        let dir =
            std::env::temp_dir().join(format!("graphite_cancel_search_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("file.txt"), "needle").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        assert!(ws.search_cancellable("needle", || true).is_empty());
        assert_eq!(ws.search("needle").len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancellation_interrupts_scanning_a_large_file_without_matches() {
        let dir =
            std::env::temp_dir().join(format!("graphite_cancel_large_{}", std::process::id()));
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
        let dir = std::env::temp_dir().join(format!("graphite_quick_open_{}", std::process::id()));
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
    fn an_index_built_once_answers_many_queries_like_a_fresh_walk() {
        let dir = std::env::temp_dir().join(format!("graphite_file_index_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for folder in ["src/app", "src/util", "tests", "docs"] {
            fs::create_dir_all(dir.join(folder)).unwrap();
        }
        for index in 0..60 {
            let folder = ["src/app", "src/util", "tests", "docs"][index % 4];
            fs::write(dir.join(folder).join(format!("file_{index}.rs")), "").unwrap();
        }
        fs::write(dir.join("Ärger.md"), "").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        let index = ws.file_index(|| false).unwrap();
        assert_eq!(index.len(), 61);

        for query in ["", "f1", "srcapp", "file_5", "rs", "ärg", "zzz"] {
            for limit in [1, 5, 100] {
                assert_eq!(
                    index.fuzzy(query, limit, || false),
                    ws.fuzzy_files(query, limit, || false),
                    "{query} {limit}"
                );
            }
        }
        // Matching ignores case, also beyond ASCII, and follows the folders typed.
        assert!(index.fuzzy("ärg", 1, || false)[0].ends_with("Ärger.md"));
        assert!(index
            .fuzzy("srcapp", 3, || false)
            .iter()
            .all(|path| path.parent().unwrap().ends_with("src/app")));
        // The best few equal the head of the complete ranking.
        let all = index.fuzzy("rs", usize::MAX, || false);
        assert_eq!(index.fuzzy("rs", 7, || false), all[..7]);
        assert!(index.fuzzy("rs", 7, || true).is_empty(), "cancelled");
        assert!(ws.file_index(|| true).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn search_prefers_unsaved_content_without_including_ignored_files() {
        let dir =
            std::env::temp_dir().join(format!("graphite_overlay_search_{}", std::process::id()));
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
