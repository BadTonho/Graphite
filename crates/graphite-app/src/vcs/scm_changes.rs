//! Collects the before and after text of changed files for the multi-file diff.

use crate::multi_diff::FileInput;
use crate::scm::FileStatus;
use gix::bstr::ByteSlice;
use graphite_i18n::Translator;
use std::{fs, path::Path};

/// Which changes to compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeSet {
    /// Everything that differs from the last commit: the working tree against `HEAD`.
    All,
    /// What is staged: the index against `HEAD`.
    Staged,
    /// What is not staged yet, new files included: the working tree against the index.
    Unstaged,
}

/// A file's content on one side: text, or only a note that it is not text.
#[derive(Clone, Debug)]
enum Side {
    Missing,
    Text(String),
    Binary,
}

fn side_of(bytes: Option<Vec<u8>>) -> Side {
    match bytes {
        None => Side::Missing,
        Some(bytes) if bytes.contains(&0) => Side::Binary,
        Some(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Side::Text(text),
            Err(_) => Side::Binary,
        },
    }
}

fn input(path: String, old: Side, new: Side) -> FileInput {
    let binary = matches!(old, Side::Binary) || matches!(new, Side::Binary);
    let text = |side: Side| match side {
        Side::Text(text) => Some(text),
        Side::Missing => None,
        Side::Binary => None,
    };
    FileInput {
        path,
        old: text(old),
        new: text(new),
        binary,
    }
}

/// Reads one side of every selected file of the repository at `root`. Files that are missing on
/// a side show as added or deleted; files whose two sides are equal are left out.
pub(crate) fn collect(
    root: &Path,
    files: &[FileStatus],
    set: ChangeSet,
    translator: &Translator,
) -> Result<Vec<FileInput>, String> {
    let repo = gix::discover(root).map_err(|error| error.to_string())?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_repository").to_owned())?
        .to_path_buf();
    let head = repo
        .index_from_tree(
            &repo
                .head_tree_id_or_empty()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let index = repo.open_index().ok();
    let blob_of =
        |index: &gix::index::State, relative: &str, staged_only: bool| -> Option<Vec<u8>> {
            let wanted = relative.as_bytes().as_bstr();
            let entry = index.entries().iter().find(|entry| {
                entry.path_in(index.path_backing()) == wanted
                    && (!staged_only || entry.stage() == gix::index::entry::Stage::Unconflicted)
            })?;
            repo.find_blob(entry.id).ok().map(|blob| blob.data.to_vec())
        };
    let mut inputs = Vec::new();
    for file in files {
        let selected = match set {
            ChangeSet::All => true,
            ChangeSet::Staged => file.staged,
            ChangeSet::Unstaged => file.unstaged || file.untracked,
        };
        if !selected {
            continue;
        }
        let relative = file.path.to_string_lossy().replace('\\', "/");
        let head_side = || side_of(blob_of(&head, &relative, true));
        let index_side = || match &index {
            Some(index) => side_of(blob_of(index, &relative, true)),
            None => Side::Missing,
        };
        let work_side = || side_of(fs::read(workdir.join(&file.path)).ok());
        let (old, new) = match set {
            ChangeSet::All => (head_side(), work_side()),
            ChangeSet::Staged => (head_side(), index_side()),
            ChangeSet::Unstaged => (index_side(), work_side()),
        };
        let item = input(relative, old, new);
        let unchanged = !item.binary && item.old == item.new;
        if !unchanged {
            inputs.push(item);
        }
    }
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scm;
    use crate::test_support::TempDir;

    fn translator() -> Translator {
        Translator::english()
    }

    /// A repository with `kept.txt`, `edited.txt` and `gone.txt` committed, then: `edited.txt`
    /// changed and staged, then changed again; `gone.txt` deleted; `fresh.txt` staged new;
    /// `loose.txt` untracked; `photo.bin` committed and changed (binary).
    fn repo() -> (TempDir, Vec<FileStatus>) {
        let dir = TempDir::new();
        crate::test_support::init_git_repo(dir.path());
        let write = |name: &str, bytes: &[u8]| fs::write(dir.path().join(name), bytes).unwrap();
        for (name, text) in [
            ("kept.txt", "same\n"),
            ("edited.txt", "one\ntwo\n"),
            ("gone.txt", "bye\n"),
        ] {
            write(name, text.as_bytes());
        }
        write("photo.bin", &[0, 1, 2]);
        for name in ["kept.txt", "edited.txt", "gone.txt", "photo.bin"] {
            scm::stage_file(dir.path(), &dir.path().join(name), &translator()).unwrap();
        }
        scm::commit(dir.path(), "initial", &translator()).unwrap();
        write("edited.txt", b"one\nTWO\n");
        scm::stage_file(dir.path(), &dir.path().join("edited.txt"), &translator()).unwrap();
        write("edited.txt", b"one\nTWO\nthree\n");
        fs::remove_file(dir.path().join("gone.txt")).unwrap();
        write("fresh.txt", b"new\n");
        scm::stage_file(dir.path(), &dir.path().join("fresh.txt"), &translator()).unwrap();
        write("loose.txt", b"untracked\n");
        write("photo.bin", &[9, 9, 9]);
        let files = scm::snapshot(dir.path()).files;
        (dir, files)
    }

    fn find<'a>(inputs: &'a [FileInput], path: &str) -> &'a FileInput {
        inputs
            .iter()
            .find(|input| input.path == path)
            .unwrap_or_else(|| panic!("{path} missing from {inputs:?}"))
    }

    #[test]
    fn all_changes_compare_the_working_tree_with_head() {
        let (dir, files) = repo();
        let inputs = collect(dir.path(), &files, ChangeSet::All, &translator()).unwrap();
        let edited = find(&inputs, "edited.txt");
        assert_eq!(edited.old.as_deref(), Some("one\ntwo\n"));
        assert_eq!(edited.new.as_deref(), Some("one\nTWO\nthree\n"));
        let gone = find(&inputs, "gone.txt");
        assert_eq!(
            (gone.old.as_deref(), gone.new.as_deref()),
            (Some("bye\n"), None)
        );
        let fresh = find(&inputs, "fresh.txt");
        assert_eq!(
            (fresh.old.as_deref(), fresh.new.as_deref()),
            (None, Some("new\n"))
        );
        assert_eq!(
            find(&inputs, "loose.txt").new.as_deref(),
            Some("untracked\n")
        );
        assert!(find(&inputs, "photo.bin").binary);
        assert!(!inputs.iter().any(|input| input.path == "kept.txt"));
    }

    #[test]
    fn staged_changes_compare_the_index_with_head() {
        let (dir, files) = repo();
        let inputs = collect(dir.path(), &files, ChangeSet::Staged, &translator()).unwrap();
        let mut paths: Vec<_> = inputs.iter().map(|input| input.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, ["edited.txt", "fresh.txt"]);
        assert_eq!(
            find(&inputs, "edited.txt").new.as_deref(),
            Some("one\nTWO\n")
        );
    }

    #[test]
    fn unstaged_changes_compare_the_working_tree_with_the_index() {
        let (dir, files) = repo();
        let inputs = collect(dir.path(), &files, ChangeSet::Unstaged, &translator()).unwrap();
        let edited = find(&inputs, "edited.txt");
        assert_eq!(edited.old.as_deref(), Some("one\nTWO\n"));
        assert_eq!(edited.new.as_deref(), Some("one\nTWO\nthree\n"));
        assert_eq!(find(&inputs, "loose.txt").old, None);
        assert_eq!(find(&inputs, "gone.txt").new, None);
        assert!(!inputs.iter().any(|input| input.path == "fresh.txt"));
    }

    #[test]
    fn a_directory_that_is_not_a_repository_is_an_error() {
        let dir = TempDir::new();
        assert!(collect(dir.path(), &[], ChangeSet::All, &translator()).is_err());
    }

    #[test]
    fn an_empty_selection_collects_nothing() {
        let (dir, _) = repo();
        assert!(collect(dir.path(), &[], ChangeSet::All, &translator())
            .unwrap()
            .is_empty());
    }
}
