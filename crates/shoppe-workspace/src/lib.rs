//! Workspace folder model: a lazily-loaded file tree that respects `.gitignore`.

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
}
