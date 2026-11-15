//! Stash operations of the SCM view. They run the `git` command like [`crate::scm_remote`] and
//! are called on a worker thread.

use std::path::Path;

use super::scm_remote::run_git;

/// Which changes a new stash takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StashMode {
    /// Staged and unstaged changes to tracked files.
    Tracked,
    /// The above plus new (untracked) files.
    WithUntracked,
    /// Only what is staged.
    Staged,
}

/// Stashes changes according to `mode`.
pub(crate) fn push(root: &Path, mode: StashMode) -> Result<String, String> {
    let args: &[&str] = match mode {
        StashMode::Tracked => &["stash", "push"],
        StashMode::WithUntracked => &["stash", "push", "--include-untracked"],
        StashMode::Staged => &["stash", "push", "--staged"],
    };
    run_git(root, args)
}

/// Applies the latest stash and keeps it in the list.
pub(crate) fn apply_latest(root: &Path) -> Result<String, String> {
    run_git(root, &["stash", "apply"])
}

/// Applies the latest stash and removes it from the list.
pub(crate) fn pop_latest(root: &Path) -> Result<String, String> {
    run_git(root, &["stash", "pop"])
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

    fn repository() -> crate::test_support::TempDir {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "core.autocrlf", "false"]);
        fs::write(root.join("tracked.txt"), "one\n").unwrap();
        fs::write(root.join("other.txt"), "x\n").unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "first"]);
        dir
    }

    #[test]
    fn modes_choose_what_goes_into_the_stash() {
        let dir = repository();
        let root = dir.path();
        fs::write(root.join("tracked.txt"), "two\n").unwrap();
        fs::write(root.join("new.txt"), "new\n").unwrap();

        push(root, StashMode::Tracked).unwrap();
        assert_eq!(fs::read_to_string(root.join("tracked.txt")).unwrap(), "one\n");
        assert!(root.join("new.txt").exists(), "untracked files stay");

        apply_latest(root).unwrap();
        assert_eq!(fs::read_to_string(root.join("tracked.txt")).unwrap(), "two\n");
        git(root, &["checkout", "--", "tracked.txt"]);

        push(root, StashMode::WithUntracked).unwrap_or_default();
        assert!(!root.join("new.txt").exists(), "untracked files are stashed too");
        pop_latest(root).unwrap();
        assert!(root.join("new.txt").exists());
    }

    #[test]
    fn staged_mode_leaves_unstaged_changes_alone() {
        let dir = repository();
        let root = dir.path();
        fs::write(root.join("tracked.txt"), "staged\n").unwrap();
        git(root, &["add", "tracked.txt"]);
        fs::write(root.join("other.txt"), "x\nunstaged\n").unwrap();

        push(root, StashMode::Staged).unwrap();
        assert_eq!(fs::read_to_string(root.join("tracked.txt")).unwrap(), "one\n");
        assert_eq!(
            fs::read_to_string(root.join("other.txt")).unwrap(),
            "x\nunstaged\n",
            "unstaged changes stay in the working tree"
        );
    }

    #[test]
    fn popping_with_nothing_stashed_reports_it() {
        let dir = repository();
        assert!(pop_latest(dir.path()).is_err());
        assert!(apply_latest(dir.path()).is_err());
    }
}
