//! Flattened, expandable file tree built on top of `graphite-workspace`.

use graphite_workspace::{Entry, Workspace};
use std::{collections::HashSet, path::PathBuf};

pub struct TreeRow {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
    pub icon: crate::icon_theme::Icon,
    pub workspace_root: bool,
    /// A companion file shown under its parent (file nesting).
    #[cfg_attr(not(test), allow(dead_code))]
    pub nested: bool,
    /// Number of companion files nested under this row.
    #[cfg_attr(not(test), allow(dead_code))]
    pub nested_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn expanding_and_collapsing_folders_honors_ignore_rules() {
        let temp = crate::test_support::TempDir::new();
        let root = temp.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join(".gitignore"), "src/ignored.rs\n").unwrap();
        fs::write(root.join("src/ignored.rs"), "ignored").unwrap();
        fs::write(root.join("src/main.rs"), "visible").unwrap();
        let mut tree = FileTree::new(Workspace::open(root).unwrap());
        let folder = tree
            .rows
            .iter()
            .position(|row| row.entry.name == "src")
            .unwrap();
        assert!(tree.activate(folder).is_none());
        let file = tree
            .rows
            .iter()
            .position(|row| row.entry.name == "main.rs")
            .unwrap();
        assert_eq!(tree.rows[file].depth, 1);
        assert!(!tree.rows.iter().any(|row| row.entry.name == "ignored.rs"));
        assert_eq!(
            tree.activate(file),
            Some(root.join("src/main.rs").canonicalize().unwrap())
        );
        assert!(tree.activate(folder).is_none());
        assert!(!tree.rows.iter().any(|row| row.entry.name == "main.rs"));
    }
}

pub struct FileTree {
    workspace: Workspace,
    expanded: HashSet<PathBuf>,
    pub rows: Vec<TreeRow>,
    pub(crate) show_workspace_roots: bool,
}

impl FileTree {
    pub fn new(workspace: Workspace) -> Self {
        let mut tree = Self {
            workspace,
            expanded: HashSet::new(),
            rows: Vec::new(),
            show_workspace_roots: false,
        };
        tree.rebuild();
        tree
    }

    pub fn root_name(&self) -> String {
        self.workspace
            .root()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.workspace.root().display().to_string())
    }
    pub fn root(&self) -> &std::path::Path {
        self.workspace.root()
    }

    pub fn roots(&self) -> &[PathBuf] {
        self.workspace.roots()
    }

    pub fn workspace_snapshot(&self) -> Workspace {
        self.workspace.clone()
    }

    pub fn root_for(&self, path: &std::path::Path) -> &std::path::Path {
        self.workspace
            .roots()
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .map_or(self.workspace.root(), PathBuf::as_path)
    }

    pub fn relative_path(&self, path: &std::path::Path) -> PathBuf {
        path.strip_prefix(self.root_for(path))
            .unwrap_or(path)
            .to_path_buf()
    }

    pub fn add_workspace_root(
        &mut self,
        path: PathBuf,
    ) -> Result<bool, graphite_workspace::WorkspaceError> {
        let added = self.workspace.add_root(path)?;
        if added {
            self.show_workspace_roots = true;
            self.rebuild();
        }
        Ok(added)
    }

    pub fn remove_workspace_root(&mut self, path: &std::path::Path) -> bool {
        let removed = self.workspace.remove_root(path);
        if removed {
            self.expanded.remove(path);
            self.rebuild();
        }
        removed
    }

    /// Re-lists the folders after files were created, renamed, moved or deleted.
    pub fn refresh(&mut self) {
        self.rebuild();
    }

    /// Looks the row icons up again, after the icon theme or the color theme changed.
    pub fn refresh_icons(&mut self) {
        for row in &mut self.rows {
            row.icon = crate::icon_theme::for_entry(
                &row.entry.name,
                row.entry.is_dir,
                row.expanded,
                row.workspace_root,
            );
        }
    }

    pub fn replace_workspace(&mut self, workspace: Workspace) {
        self.workspace = workspace;
        self.expanded.clear();
        self.rebuild();
    }

    /// Toggles a directory row; returns the file path if the row is a file.
    pub fn activate(&mut self, row: usize) -> Option<PathBuf> {
        let entry = self.rows.get(row)?.entry.clone();
        if entry.is_dir {
            if !self.expanded.remove(&entry.path) {
                self.expanded.insert(entry.path);
            }
            self.rebuild();
            None
        } else {
            Some(entry.path)
        }
    }

    /// Expands or collapses a folder by path. Returns whether anything changed.
    pub fn set_expanded(&mut self, path: &std::path::Path, expanded: bool) -> bool {
        let changed = if expanded {
            self.expanded.insert(path.to_path_buf())
        } else {
            self.expanded.remove(path)
        };
        if changed {
            self.rebuild();
        }
        changed
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.rebuild();
    }

    fn rebuild(&mut self) {
        let mut rows = Vec::new();
        let roots = self.workspace.roots().to_vec();
        self.show_workspace_roots = roots.len() > 1;
        for root in roots {
            if self.show_workspace_roots {
                let name = root
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.display().to_string());
                let expanded = self.expanded.contains(&root);
                let icon = crate::icon_theme::for_entry(&name, true, expanded, true);
                rows.push(TreeRow {
                    entry: Entry {
                        path: root.clone(),
                        name,
                        is_dir: true,
                    },
                    depth: 0,
                    expanded,
                    icon,
                    workspace_root: true,
                    nested: false,
                    nested_count: 0,
                });
                if expanded {
                    self.collect(&root, 1, &mut rows);
                }
            } else {
                self.collect(&root, 0, &mut rows);
            }
        }
        self.rows = rows;
    }

    fn collect(&self, dir: &std::path::Path, depth: usize, out: &mut Vec<TreeRow>) {
        let entries = self.workspace.list_dir(dir);
        // Companion files (e.g. `app.js` under `app.ts`) are listed under their parent.
        let flags: Vec<(&str, bool)> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.is_dir))
            .collect();
        let parents = crate::file_nesting::nesting_parents(&flags);
        let mut companions: Vec<Vec<usize>> = vec![Vec::new(); entries.len()];
        for (child, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                companions[*parent].push(child);
            }
        }
        for (index, entry) in entries.iter().enumerate() {
            if parents[index].is_some() {
                continue;
            }
            self.push_row(entry, depth, false, companions[index].len(), out);
            for &child in &companions[index] {
                self.push_row(&entries[child], depth + 1, true, 0, out);
            }
        }
    }

    fn push_row(
        &self,
        entry: &Entry,
        depth: usize,
        nested: bool,
        nested_count: usize,
        out: &mut Vec<TreeRow>,
    ) {
        let expanded = entry.is_dir && self.expanded.contains(&entry.path);
        let icon = crate::icon_theme::for_entry(&entry.name, entry.is_dir, expanded, false);
        out.push(TreeRow {
            entry: entry.clone(),
            depth,
            expanded,
            icon,
            workspace_root: false,
            nested,
            nested_count,
        });
        if expanded {
            self.collect(&entry.path, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod nesting_tests {
    use super::*;
    use std::fs;

    #[test]
    fn companion_files_are_listed_under_their_parent() {
        let temp = crate::test_support::TempDir::new();
        let root = temp.path();
        fs::write(root.join("app.ts"), "").unwrap();
        fs::write(root.join("app.js"), "").unwrap();
        fs::write(root.join("app.d.ts"), "").unwrap();
        fs::write(root.join("main.rs"), "").unwrap();
        let tree = FileTree::new(Workspace::open(root).unwrap());
        let names: Vec<(&str, usize, bool, usize)> = tree
            .rows
            .iter()
            .map(|row| {
                (
                    row.entry.name.as_str(),
                    row.depth,
                    row.nested,
                    row.nested_count,
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                ("app.ts", 0, false, 2),
                ("app.d.ts", 1, true, 0),
                ("app.js", 1, true, 0),
                ("main.rs", 0, false, 0),
            ]
        );
    }
}
