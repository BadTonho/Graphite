//! Commits made through the `git` command, so hooks, signing and identity configuration behave as
//! they do in the terminal. Callers run them on a worker thread.

use std::path::Path;

use super::scm_remote::{self, run_git};

/// Which changes a commit takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommitScope {
    /// What is staged; when nothing is staged, everything.
    Smart,
    /// Only what is staged.
    Staged,
    /// Stage every change first, new files included.
    All,
}

/// What happens after the commit succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PostCommit {
    Nothing,
    Push,
    Sync,
}

/// The kind of commit chosen in the menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommitOptions {
    pub scope: CommitScope,
    /// Replace the last commit instead of adding one.
    pub amend: bool,
    /// Add a `Signed-off-by` line.
    pub signoff: bool,
    pub post: PostCommit,
}

impl CommitOptions {
    pub(crate) const PLAIN: Self = Self {
        scope: CommitScope::Smart,
        amend: false,
        signoff: false,
        post: PostCommit::Nothing,
    };
}

/// A commit ready to run: the options with the scope decided and the message taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitRequest {
    pub message: String,
    pub stage_all: bool,
    pub amend: bool,
    pub signoff: bool,
    pub post: PostCommit,
}

/// Commits, then pushes or syncs when asked to. A failed commit stops before that.
pub(crate) fn commit(root: &Path, request: &CommitRequest) -> Result<String, String> {
    let message = request.message.trim();
    if message.is_empty() && !request.amend {
        return Err("Type a commit message first".into());
    }
    if request.stage_all {
        run_git(root, &["add", "-A"])?;
    }
    let mut args = vec!["commit"];
    if request.signoff {
        args.push("--signoff");
    }
    if request.amend {
        args.push("--amend");
    }
    if message.is_empty() {
        args.push("--no-edit");
    } else {
        args.extend(["-m", message]);
    }
    let output = run_git(root, &args)?;
    let summary = output.lines().next().unwrap_or_default().to_owned();
    match request.post {
        PostCommit::Nothing => Ok(summary),
        PostCommit::Push => scm_remote::push(root).map(|_| summary),
        PostCommit::Sync => scm_remote::sync(root).map(|_| summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) -> String {
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
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn request(message: &str) -> CommitRequest {
        CommitRequest {
            message: message.into(),
            stage_all: false,
            amend: false,
            signoff: false,
            post: PostCommit::Nothing,
        }
    }

    /// A repository with one commit, and the identity the commands under test need.
    fn repository() -> crate::test_support::TempDir {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "core.autocrlf", "false"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.invalid"]);
        fs::write(root.join("a.txt"), "a\n").unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "base"]);
        dir
    }

    #[test]
    fn only_staged_changes_are_committed_unless_everything_is_staged_first() {
        let dir = repository();
        let root = dir.path();
        fs::write(root.join("a.txt"), "edited\n").unwrap();
        fs::write(root.join("new.txt"), "new\n").unwrap();
        git(root, &["add", "a.txt"]);

        commit(root, &request("staged only")).unwrap();
        assert_eq!(
            git(root, &["show", "--name-only", "--format=", "HEAD"]),
            "a.txt"
        );
        assert_eq!(git(root, &["status", "--porcelain"]), "?? new.txt");

        let all = CommitRequest {
            stage_all: true,
            ..request("everything")
        };
        commit(root, &all).unwrap();
        assert_eq!(
            git(root, &["show", "--name-only", "--format=", "HEAD"]),
            "new.txt"
        );
        assert_eq!(git(root, &["status", "--porcelain"]), "");
    }

    #[test]
    fn amend_replaces_the_last_commit_and_keeps_its_message_without_a_new_one() {
        let dir = repository();
        let root = dir.path();
        let before = git(root, &["rev-list", "--count", "HEAD"]);
        fs::write(root.join("a.txt"), "more\n").unwrap();
        git(root, &["add", "a.txt"]);
        let amend = CommitRequest {
            amend: true,
            ..request("")
        };
        commit(root, &amend).unwrap();
        assert_eq!(git(root, &["rev-list", "--count", "HEAD"]), before);
        assert_eq!(git(root, &["log", "-1", "--format=%s"]), "base");

        let reworded = CommitRequest {
            amend: true,
            ..request("better")
        };
        commit(root, &reworded).unwrap();
        assert_eq!(git(root, &["log", "-1", "--format=%s"]), "better");
    }

    #[test]
    fn signing_off_adds_the_trailer() {
        let dir = repository();
        let root = dir.path();
        fs::write(root.join("a.txt"), "x\n").unwrap();
        git(root, &["add", "a.txt"]);
        let signed = CommitRequest {
            signoff: true,
            ..request("signed")
        };
        commit(root, &signed).unwrap();
        assert!(git(root, &["log", "-1", "--format=%B"]).contains("Signed-off-by: Test"));
    }

    #[test]
    fn an_empty_message_or_nothing_to_commit_is_an_error() {
        let dir = repository();
        let root = dir.path();
        assert!(commit(root, &request("  ")).is_err());
        assert!(commit(root, &request("nothing changed")).is_err());
        assert_eq!(git(root, &["log", "-1", "--format=%s"]), "base");
    }

    #[test]
    fn a_failing_hook_stops_the_commit_and_the_push() {
        let dir = repository();
        let root = dir.path();
        let hook = root.join(".git/hooks/pre-commit");
        fs::create_dir_all(hook.parent().unwrap()).unwrap();
        fs::write(&hook, "#!/bin/sh\necho blocked >&2\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(root.join("a.txt"), "x\n").unwrap();
        git(root, &["add", "a.txt"]);
        let pushing = CommitRequest {
            post: PostCommit::Push,
            ..request("blocked")
        };
        let error = commit(root, &pushing).unwrap_err();
        assert!(error.contains("blocked"), "{error}");
        assert_eq!(git(root, &["log", "-1", "--format=%s"]), "base");
    }

    #[test]
    fn commit_and_push_and_sync_publish_the_new_commit() {
        let dir = crate::test_support::TempDir::new();
        let remote = dir.path().join("remote.git");
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(
            dir.path(),
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                remote.to_str().unwrap(),
            ],
        );
        git(&work, &["init", "-q", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        fs::write(work.join("a.txt"), "a\n").unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-q", "-m", "first"]);

        let push = CommitRequest {
            post: PostCommit::Push,
            stage_all: true,
            ..request("first push")
        };
        fs::write(work.join("a.txt"), "b\n").unwrap();
        commit(&work, &push).unwrap();
        assert_eq!(
            git(&remote, &["log", "-1", "--format=%s", "main"]),
            "first push"
        );

        fs::write(work.join("a.txt"), "c\n").unwrap();
        let sync = CommitRequest {
            post: PostCommit::Sync,
            stage_all: true,
            ..request("then sync")
        };
        commit(&work, &sync).unwrap();
        assert_eq!(
            git(&remote, &["log", "-1", "--format=%s", "main"]),
            "then sync"
        );
    }
}
