//! Flattened, expandable file tree built on top of `graphite-workspace`.

use crate::file_icons::FileIcon;
use graphite_workspace::{Entry, Workspace};
use std::{collections::HashSet, path::PathBuf};

pub struct TreeRow {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
    pub icon: FileIcon,
    pub workspace_root: bool,
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

    pub fn is_in_workspace(&self, path: &std::path::Path) -> bool {
        self.workspace
            .roots()
            .iter()
            .any(|root| path.starts_with(root))
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

    pub fn open_workspace_file(path: &std::path::Path) -> Result<Workspace, String> {
        Workspace::open_code_workspace(path)
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
                rows.push(TreeRow {
                    entry: Entry {
                        path: root.clone(),
                        name,
                        is_dir: true,
                    },
                    depth: 0,
                    expanded,
                    icon: crate::file_icons::for_entry("", true, expanded),
                    workspace_root: true,
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
        for entry in self.workspace.list_dir(dir) {
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            let path = entry.path.clone();
            let icon = crate::file_icons::for_entry(&entry.name, entry.is_dir, expanded);
            out.push(TreeRow {
                entry,
                depth,
                expanded,
                icon,
                workspace_root: false,
            });
            if expanded {
                self.collect(&path, depth + 1, out);
            }
        }
    }
}
