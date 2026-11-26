//! The changes lists shown as a folder tree: folders first, then files, each sorted by name.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// One row of a tree of changed files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TreeRow {
    /// A folder; `first` is the snapshot index of its first file in the list, `count` how many
    /// files are below it.
    Folder {
        path: PathBuf,
        depth: usize,
        first: usize,
        count: usize,
    },
    File {
        index: usize,
        depth: usize,
    },
}

#[derive(Default)]
struct Node {
    folders: BTreeMap<String, Node>,
    files: Vec<(String, usize)>,
    first: Option<usize>,
    count: usize,
}

/// Which folders are collapsed, per list (`true` is the staged list).
pub(crate) type Collapsed = HashSet<(bool, PathBuf)>;

/// The rows of the tree of `files` (snapshot index and path). Files in collapsed folders and the
/// folders inside them are left out.
pub(crate) fn tree_rows(
    files: &[(usize, &Path)],
    staged: bool,
    collapsed: &Collapsed,
) -> Vec<TreeRow> {
    let mut root = Node::default();
    for (index, path) in files {
        let parts: Vec<String> = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let Some((name, folders)) = parts.split_last() else {
            continue;
        };
        let mut node = &mut root;
        for folder in folders {
            node = node.folders.entry(folder.clone()).or_default();
            node.first.get_or_insert(*index);
            node.count += 1;
        }
        node.files.push((name.clone(), *index));
    }
    let mut rows = Vec::new();
    emit(&root, Path::new(""), 0, staged, collapsed, &mut rows);
    rows
}

fn emit(
    node: &Node,
    parent: &Path,
    depth: usize,
    staged: bool,
    collapsed: &Collapsed,
    rows: &mut Vec<TreeRow>,
) {
    for (name, folder) in &node.folders {
        let path = parent.join(name);
        rows.push(TreeRow::Folder {
            path: path.clone(),
            depth,
            first: folder.first.unwrap_or(0),
            count: folder.count,
        });
        if !collapsed.contains(&(staged, path.clone())) {
            emit(folder, &path, depth + 1, staged, collapsed, rows);
        }
    }
    let mut files = node.files.clone();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    rows.extend(
        files
            .into_iter()
            .map(|(_, index)| TreeRow::File { index, depth }),
    );
}

#[cfg(test)]
/// The indexes of the files at or below `folder`, from the `(index, path)` pairs of one list.
pub(crate) fn files_under(files: &[(usize, &Path)], folder: &Path) -> Vec<usize> {
    files
        .iter()
        .filter(|(_, path)| path.starts_with(folder))
        .map(|(index, _)| *index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;

    fn rows(paths: &[&str], collapsed: &Collapsed) -> Vec<TreeRow> {
        let list: Vec<(usize, &Path)> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (index, Path::new(*path)))
            .collect();
        tree_rows(&list, false, collapsed)
    }

    fn describe(rows: &[TreeRow], paths: &[&str]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                TreeRow::Folder {
                    path, depth, count, ..
                } => format!("{}{}/ ({count})", "  ".repeat(*depth), path.display())
                    .replace('\\', "/"),
                TreeRow::File { index, depth } => {
                    let name = Path::new(paths[*index])
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    format!("{}{name}", "  ".repeat(*depth))
                }
            })
            .collect()
    }

    #[test]
    fn folders_come_before_files_and_everything_is_sorted() {
        let paths = [
            "z.txt",
            "src/b.rs",
            "src/a.rs",
            "src/ui/w.rs",
            "a.txt",
            "docs/x.md",
        ];
        let tree = rows(&paths, &Collapsed::new());
        assert_eq!(
            describe(&tree, &paths),
            [
                "docs/ (1)",
                "  x.md",
                "src/ (3)",
                "  src/ui/ (1)",
                "    w.rs",
                "  a.rs",
                "  b.rs",
                "a.txt",
                "z.txt",
            ]
        );
    }

    #[test]
    fn a_collapsed_folder_hides_its_files_and_subfolders_but_keeps_its_count() {
        let paths = ["src/a.rs", "src/ui/w.rs", "top.txt"];
        let mut collapsed = Collapsed::new();
        collapsed.insert((false, PathBuf::from("src")));
        let tree = rows(&paths, &collapsed);
        assert_eq!(describe(&tree, &paths), ["src/ (2)", "top.txt"]);
        // Collapsing is per list: the same folder in the staged list stays open.
        let list: Vec<(usize, &Path)> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (index, Path::new(*path)))
            .collect();
        assert_eq!(tree_rows(&list, true, &collapsed).len(), 5);
    }

    #[test]
    fn a_folder_remembers_its_first_file_and_depth() {
        let paths = ["a/b/c.txt", "a/d.txt"];
        let tree = rows(&paths, &Collapsed::new());
        assert_eq!(
            tree[0],
            TreeRow::Folder {
                path: PathBuf::from("a"),
                depth: 0,
                first: 0,
                count: 2
            }
        );
        assert!(matches!(
            &tree[1],
            TreeRow::Folder {
                depth: 1,
                first: 0,
                count: 1,
                ..
            }
        ));
    }

    #[test]
    fn files_under_a_folder_are_found_by_path_prefix_not_by_name_prefix() {
        let list: Vec<(usize, &Path)> = vec![
            (0, Path::new("src/a.rs")),
            (1, Path::new("src2/b.rs")),
            (2, Path::new("src/ui/c.rs")),
        ];
        assert_eq!(files_under(&list, Path::new("src")), [0, 2]);
        assert!(files_under(&list, Path::new("none")).is_empty());
    }

    fn snapshot(paths: &[(&str, bool)]) -> crate::scm::Snapshot {
        crate::scm::Snapshot {
            files: paths
                .iter()
                .map(|(path, staged)| crate::scm::FileStatus {
                    path: (*path).into(),
                    staged: *staged,
                    unstaged: !*staged,
                    ..Default::default()
                })
                .collect(),
            has_commits: true,
            ..Default::default()
        }
    }

    fn kinds(
        snapshot: &crate::scm::Snapshot,
        view: &super::super::scm_ui::ScmListView<'_>,
    ) -> Vec<super::super::scm_ui::ScmListRowKind> {
        super::super::scm_ui::scm_panel_layout_with_graph(
            snapshot,
            300.0,
            900.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
            &[],
            false,
            false,
            0,
            0,
            None,
            view,
        )
        .list_rows
        .into_iter()
        .map(|row| row.kind)
        .collect()
    }

    #[test]
    fn the_layout_lists_folders_in_tree_mode_and_hides_collapsed_ones() {
        use super::super::scm_ui::{ScmListRowKind as K, ScmListView};
        let snapshot = snapshot(&[("src/a.rs", false), ("src/b.rs", false), ("top.txt", false)]);
        let flat = kinds(&snapshot, &ScmListView::LIST);
        assert!(flat.iter().all(|kind| !matches!(kind, K::Folder { .. })));
        assert_eq!(
            flat.iter()
                .filter(|kind| matches!(kind, K::File { .. }))
                .count(),
            3
        );

        let mut collapsed = Collapsed::new();
        let open = ScmListView {
            tree: true,
            collapsed_folders: Some(&collapsed),
        };
        let tree = kinds(&snapshot, &open);
        assert!(matches!(
            tree[1],
            K::Folder {
                staged: false,
                depth: 0,
                count: 2,
                collapsed: false,
                ..
            }
        ));
        assert_eq!(
            tree.iter()
                .filter(|kind| matches!(kind, K::File { .. }))
                .count(),
            3
        );

        collapsed.insert((false, PathBuf::from("src")));
        let closed = ScmListView {
            tree: true,
            collapsed_folders: Some(&collapsed),
        };
        let tree = kinds(&snapshot, &closed);
        assert!(matches!(
            tree[1],
            K::Folder {
                collapsed: true,
                ..
            }
        ));
        assert_eq!(
            tree.iter()
                .filter(|kind| matches!(kind, K::File { .. }))
                .count(),
            1,
            "only top.txt is left"
        );
    }

    #[test]
    fn folders_open_and_close_and_stage_what_is_below_them() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm.snapshot = snapshot(&[
            ("src/a.rs", false),
            ("src2/b.rs", false),
            ("src/ui/c.rs", false),
            ("src/staged.rs", true),
        ]);
        app.toggle_scm_folder(false, "src".into());
        assert!(app
            .scm
            .collapsed_folders
            .contains(&(false, PathBuf::from("src"))));
        app.toggle_scm_folder(false, "src".into());
        assert!(app.scm.collapsed_folders.is_empty());

        app.status.clear();
        app.stage_folder(false, Path::new("src"));
        let mut paths = app.scm.batch_paths.clone();
        paths.sort();
        assert_eq!(
            paths,
            [PathBuf::from("src/a.rs"), PathBuf::from("src/ui/c.rs")],
            "not src2, and not the already staged file"
        );
        // Without an event loop the action stops at this message, which shows it was started.
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));

        app.stage_folder(true, Path::new("src"));
        assert_eq!(app.scm.batch_paths, [PathBuf::from("src/staged.rs")]);
    }

    #[test]
    fn a_folder_is_the_leading_part_of_a_files_path() {
        use super::super::scm_ui::folder_path;
        assert_eq!(folder_path(Path::new("a/b/c.txt"), 0), PathBuf::from("a"));
        assert_eq!(folder_path(Path::new("a/b/c.txt"), 1), PathBuf::from("a/b"));
    }

    #[test]
    fn an_empty_list_has_no_rows() {
        assert!(tree_rows(&[], false, &Collapsed::new()).is_empty());
    }
}
