//! Throwing away working tree changes. Callers ask the user first; these run on a worker thread.

use std::path::{Path, PathBuf};

use super::scm_remote::run_git;

/// A file whose working tree changes are discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscardTarget {
    pub path: PathBuf,
    /// The file is new and git does not track it: discarding deletes it.
    pub untracked: bool,
}

/// How many paths go to one `git` command, so a huge selection stays below command line limits.
const BATCH: usize = 100;

/// Restores the tracked files to the staged content and deletes the untracked ones. Staged
/// changes are kept.
pub(crate) fn discard(root: &Path, targets: &[DiscardTarget]) -> Result<String, String> {
    let paths = |untracked: bool| -> Vec<String> {
        targets
            .iter()
            .filter(|target| target.untracked == untracked)
            .map(|target| target.path.to_string_lossy().into_owned())
            .collect()
    };
    for batch in paths(false).chunks(BATCH) {
        let mut args = vec!["restore", "--worktree", "--"];
        args.extend(batch.iter().map(String::as_str));
        run_git(root, &args)?;
    }
    for batch in paths(true).chunks(BATCH) {
        let mut args = vec!["clean", "-f", "-d", "--"];
        args.extend(batch.iter().map(String::as_str));
        run_git(root, &args)?;
    }
    Ok(format!("Discarded changes in {} file(s)", targets.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn target(path: &str, untracked: bool) -> DiscardTarget {
        DiscardTarget {
            path: PathBuf::from(path),
            untracked,
        }
    }

    #[test]
    fn tracked_files_are_restored_and_untracked_ones_deleted() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "core.autocrlf", "false"]);
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), "a\n").unwrap();
        fs::write(root.join("sub/b.txt"), "b\n").unwrap();
        fs::write(root.join("gone.txt"), "gone\n").unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "base"]);

        fs::write(root.join("a.txt"), "edited\n").unwrap();
        fs::write(root.join("sub/b.txt"), "edited\n").unwrap();
        fs::remove_file(root.join("gone.txt")).unwrap();
        fs::write(root.join("new.txt"), "new\n").unwrap();
        fs::create_dir(root.join("newdir")).unwrap();
        fs::write(root.join("newdir/c.txt"), "c\n").unwrap();

        discard(
            root,
            &[
                target("a.txt", false),
                target("gone.txt", false),
                target("new.txt", true),
                target("newdir", true),
            ],
        )
        .unwrap();
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "a\n");
        assert_eq!(fs::read_to_string(root.join("gone.txt")).unwrap(), "gone\n");
        assert!(!root.join("new.txt").exists());
        assert!(!root.join("newdir").exists());
        assert_eq!(
            fs::read_to_string(root.join("sub/b.txt")).unwrap(),
            "edited\n",
            "files that were not chosen keep their edits"
        );
    }

    #[test]
    fn staged_changes_survive_a_discard_of_the_working_tree() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "core.autocrlf", "false"]);
        fs::write(root.join("a.txt"), "a\n").unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "base"]);
        fs::write(root.join("a.txt"), "staged\n").unwrap();
        git(root, &["add", "a.txt"]);
        fs::write(root.join("a.txt"), "unstaged\n").unwrap();

        discard(root, &[target("a.txt", false)]).unwrap();
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "staged\n");
    }

    #[test]
    fn a_path_that_git_does_not_know_is_an_error_and_options_are_not_read() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        assert!(discard(root, &[target("missing.txt", false)]).is_err());
        // A path that looks like an option is a path after `--`.
        assert!(discard(root, &[target("--all", false)]).is_err());
        assert_eq!(
            discard(root, &[]).unwrap(),
            "Discarded changes in 0 file(s)"
        );
    }
}
