//! Flattened, expandable file tree built on top of `graphite-workspace`.

use graphite_workspace::SearchResult;
use graphite_workspace::{Entry, Workspace};
use std::{collections::HashSet, path::PathBuf};

pub struct TreeRow {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
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

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        self.workspace.search(query)
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
