//! Flattened, expandable file tree built on top of `graphite-workspace`.

use graphite_workspace::{Entry, Workspace};
use std::{collections::HashSet, path::PathBuf};

pub struct TreeRow {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
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
}

impl FileTree {
    pub fn new(workspace: Workspace) -> Self {
        let mut tree = Self {
            workspace,
            expanded: HashSet::new(),
            rows: Vec::new(),
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
        let root = self.workspace.root().to_path_buf();
        self.collect(&root, 0, &mut rows);
        self.rows = rows;
    }

    fn collect(&self, dir: &std::path::Path, depth: usize, out: &mut Vec<TreeRow>) {
        for entry in self.workspace.list_dir(dir) {
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            let path = entry.path.clone();
            out.push(TreeRow {
                entry,
                depth,
                expanded,
            });
            if expanded {
                self.collect(&path, depth + 1, out);
            }
        }
    }
}
