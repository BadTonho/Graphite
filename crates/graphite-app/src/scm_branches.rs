//! Branch and history-rewriting operations of the SCM view: amend, create a branch, merge and
//! rebase, plus finishing or abandoning a merge or rebase that stopped on conflicts. They run the
//! `git` command like [`crate::scm_remote`], so hooks, identity and configuration match the
//! terminal. Callers run them on a worker thread.

use std::path::Path;
use std::process::Command;

/// A merge or rebase that stopped and waits for the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Merge,
    Rebase,
}

struct Output {
    success: bool,
    stdout: String,
    stderr: String,
}

fn git(root: &Path, args: &[&str], editor_free: bool) -> Result<Output, String> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .args(args);
    if editor_free {
        // `rebase --continue` and merges would otherwise open an editor nobody can see.
        command
            .env("GIT_EDITOR", "true")
            .env("GIT_SEQUENCE_EDITOR", "true");
    }
    let output = command.output().map_err(|error| error.to_string())?;
    Ok(Output {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

/// The message to show for a failed command. Git prints some failures (merge conflicts) on
/// standard output, so that is used when standard error is empty.
fn failure(output: &Output, operation: &str) -> String {
    let text = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let conflicted = output.stdout.contains("CONFLICT") || output.stderr.contains("CONFLICT");
    // The last line that is not advice (`hint: ...`) says what went wrong.
    let line = text
        .lines()
        .rev()
        .find(|line| !line.trim_start().starts_with("hint:"))
        .or_else(|| text.lines().last())
        .unwrap_or(text);
    match (text.is_empty(), conflicted) {
        (true, _) => format!("git {operation} failed"),
        (false, true) => {
            format!("{line} — resolve the conflicts, stage the files and continue, or abort")
        }
        (false, false) => line.to_owned(),
    }
}

fn run(root: &Path, args: &[&str], editor_free: bool) -> Result<String, String> {
    let output = git(root, args, editor_free)?;
    if output.success {
        Ok(output.stdout.lines().last().unwrap_or_default().to_owned())
    } else {
        Err(failure(&output, args.first().copied().unwrap_or_default()))
    }
}

/// Whether `name` is safe to hand to git as a branch or ref argument. A leading `-` would be read
/// as an option, and control characters or spaces are never part of a ref.
fn plain_ref(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Replaces the last commit with the staged changes, and with `message` when given.
pub(crate) fn amend(root: &Path, message: Option<&str>) -> Result<String, String> {
    match message.map(str::trim).filter(|message| !message.is_empty()) {
        Some(message) => run(root, &["commit", "--amend", "-m", message], true),
        None => run(root, &["commit", "--amend", "--no-edit"], true),
    }
}

/// Creates `name` at HEAD and switches to it. Uncommitted changes come along.
pub(crate) fn create_branch(root: &Path, name: &str) -> Result<String, String> {
    let name = name.trim();
    if !plain_ref(name) {
        return Err(format!("'{name}' is not a valid branch name"));
    }
    let check = git(root, &["check-ref-format", "--branch", name], false)?;
    if !check.success {
        return Err(format!("'{name}' is not a valid branch name"));
    }
    run(root, &["switch", "-c", name], false)?;
    Ok(format!("Switched to new branch '{name}'"))
}

/// Deletes the local branch `name`. Without `force` git refuses a branch that is not fully merged
/// (and the one that is checked out, with or without it).
pub(crate) fn delete_branch(root: &Path, name: &str, force: bool) -> Result<String, String> {
    if !plain_ref(name) {
        return Err(format!("'{name}' is not a valid branch name"));
    }
    run(
        root,
        &["branch", if force { "-D" } else { "-d" }, "--", name],
        false,
    )?;
    Ok(format!("Deleted branch '{name}'"))
}

/// Merges `branch` into the current branch. A conflict leaves the merge in progress.
pub(crate) fn merge(root: &Path, branch: &str) -> Result<String, String> {
    if !plain_ref(branch) {
        return Err(format!("'{branch}' is not a valid branch name"));
    }
    run(root, &["merge", "--no-edit", branch], true)
}

/// Rebases the current branch onto `onto`. A conflict leaves the rebase in progress.
pub(crate) fn rebase(root: &Path, onto: &str) -> Result<String, String> {
    if !plain_ref(onto) {
        return Err(format!("'{onto}' is not a valid branch name"));
    }
    run(root, &["rebase", onto], true)
}

/// The merge or rebase waiting for the user, if any.
pub(crate) fn operation_in_progress(root: &Path) -> Option<Operation> {
    let git_dir = git(root, &["rev-parse", "--absolute-git-dir"], false).ok()?;
    if !git_dir.success {
        return None;
    }
    let dir = Path::new(&git_dir.stdout);
    if dir.join("rebase-merge").exists() || dir.join("rebase-apply").exists() {
        Some(Operation::Rebase)
    } else if dir.join("MERGE_HEAD").exists() {
        Some(Operation::Merge)
    } else {
        None
    }
}

/// Abandons the merge or rebase in progress and restores the state before it started.
pub(crate) fn abort(root: &Path) -> Result<String, String> {
    match operation_in_progress(root) {
        Some(Operation::Merge) => {
            run(root, &["merge", "--abort"], true).map(|_| "Merge aborted".into())
        }
        Some(Operation::Rebase) => {
            run(root, &["rebase", "--abort"], true).map(|_| "Rebase aborted".into())
        }
        None => Err("No merge or rebase is in progress".into()),
    }
}

/// Continues the rebase in progress after the conflicts were resolved and staged. A merge is
/// finished with a normal commit instead.
pub(crate) fn continue_operation(root: &Path) -> Result<String, String> {
    match operation_in_progress(root) {
        Some(Operation::Rebase) => run(root, &["rebase", "--continue"], true),
        Some(Operation::Merge) => run(root, &["commit", "--no-edit"], true),
        None => Err("No merge or rebase is in progress".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Runs git with a fixed identity so commits work on any machine.
    fn sh(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// Repository with `main` holding one commit that writes `base` to `file.txt`.
    fn repository() -> crate::test_support::TempDir {
        let dir = crate::test_support::TempDir::new();
        sh(dir.path(), &["init", "-q", "-b", "main"]);
        // Keep files byte-for-byte on Windows so the assertions below do not depend on autocrlf.
        sh(dir.path(), &["config", "core.autocrlf", "false"]);
        // Operations here set their own identity through the environment of this process.
        std::env::set_var("GIT_AUTHOR_NAME", "Test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "test@example.invalid");
        std::env::set_var("GIT_COMMITTER_NAME", "Test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "test@example.invalid");
        write(dir.path(), "file.txt", "base\n");
        commit(dir.path(), "base");
        dir
    }

    fn write(root: &Path, name: &str, text: &str) {
        fs::write(root.join(name), text).unwrap();
    }

    fn commit(root: &Path, message: &str) {
        sh(root, &["add", "-A"]);
        sh(root, &["commit", "-q", "-m", message]);
    }

    fn head_message(root: &Path) -> String {
        sh(root, &["log", "-1", "--format=%s"])
    }

    fn commit_count(root: &Path) -> usize {
        sh(root, &["rev-list", "--count", "HEAD"]).parse().unwrap()
    }

    fn current_branch(root: &Path) -> String {
        sh(root, &["rev-parse", "--abbrev-ref", "HEAD"])
    }

    #[test]
    fn amend_replaces_the_last_commit_without_adding_one() {
        let dir = repository();
        write(dir.path(), "file.txt", "base\nmore\n");
        sh(dir.path(), &["add", "-A"]);
        let before = commit_count(dir.path());
        amend(dir.path(), None).unwrap();
        assert_eq!(commit_count(dir.path()), before);
        assert_eq!(head_message(dir.path()), "base", "the message is kept");
        assert_eq!(
            sh(dir.path(), &["show", "HEAD:file.txt"]),
            "base\nmore",
            "the staged change joined the commit"
        );
    }

    #[test]
    fn amend_with_a_message_rewrites_it_and_ignores_blank_ones() {
        let dir = repository();
        amend(dir.path(), Some("  better message  ")).unwrap();
        assert_eq!(head_message(dir.path()), "better message");
        amend(dir.path(), Some("   ")).unwrap();
        assert_eq!(head_message(dir.path()), "better message", "blank keeps it");
    }

    #[test]
    fn amend_in_a_repository_without_commits_fails_clearly() {
        let dir = crate::test_support::TempDir::new();
        sh(dir.path(), &["init", "-q", "-b", "main"]);
        assert!(amend(dir.path(), None).is_err());
    }

    #[test]
    fn create_branch_switches_to_it_and_keeps_uncommitted_changes() {
        let dir = repository();
        write(dir.path(), "file.txt", "dirty\n");
        let message = create_branch(dir.path(), "  feature/x ").unwrap();
        assert_eq!(message, "Switched to new branch 'feature/x'");
        assert_eq!(current_branch(dir.path()), "feature/x");
        assert_eq!(
            fs::read_to_string(dir.path().join("file.txt")).unwrap(),
            "dirty\n"
        );
    }

    #[test]
    fn create_branch_refuses_bad_and_existing_names() {
        let dir = repository();
        for name in [
            "",
            "  ",
            "-bad",
            "has space",
            "a..b",
            "x.lock",
            "end/",
            "~tilde",
            "a\tb",
        ] {
            assert!(
                create_branch(dir.path(), name).is_err(),
                "{name:?} should be refused"
            );
        }
        create_branch(dir.path(), "dup").unwrap();
        sh(dir.path(), &["switch", "-q", "main"]);
        let error = create_branch(dir.path(), "dup").unwrap_err();
        assert!(error.contains("already exists"), "{error}");
        assert_eq!(current_branch(dir.path()), "main", "nothing changed");
    }

    /// `topic` adds `topic.txt`; `main` adds `main.txt`; they do not conflict.
    fn diverged() -> crate::test_support::TempDir {
        let dir = repository();
        sh(dir.path(), &["switch", "-q", "-c", "topic"]);
        write(dir.path(), "topic.txt", "topic\n");
        commit(dir.path(), "topic work");
        sh(dir.path(), &["switch", "-q", "main"]);
        write(dir.path(), "main.txt", "main\n");
        commit(dir.path(), "main work");
        dir
    }

    #[test]
    fn merge_brings_in_the_other_branch_with_a_merge_commit() {
        let dir = diverged();
        merge(dir.path(), "topic").unwrap();
        assert!(dir.path().join("topic.txt").is_file());
        assert!(dir.path().join("main.txt").is_file());
        assert_eq!(
            sh(dir.path(), &["rev-list", "--parents", "-n", "1", "HEAD"])
                .split(' ')
                .count(),
            3
        );
        assert_eq!(operation_in_progress(dir.path()), None);
    }

    #[test]
    fn merging_an_unknown_branch_or_option_like_name_is_refused() {
        let dir = diverged();
        assert!(merge(dir.path(), "nope").is_err());
        let error = merge(dir.path(), "--abort").unwrap_err();
        assert!(error.contains("not a valid branch name"), "{error}");
        assert_eq!(operation_in_progress(dir.path()), None);
    }

    /// `topic` and `main` both change the same line of `file.txt`.
    fn conflicting() -> crate::test_support::TempDir {
        let dir = repository();
        sh(dir.path(), &["switch", "-q", "-c", "topic"]);
        write(dir.path(), "file.txt", "topic line\n");
        commit(dir.path(), "topic edit");
        sh(dir.path(), &["switch", "-q", "main"]);
        write(dir.path(), "file.txt", "main line\n");
        commit(dir.path(), "main edit");
        dir
    }

    #[test]
    fn a_conflicting_merge_stays_in_progress_and_can_be_aborted() {
        let dir = conflicting();
        let error = merge(dir.path(), "topic").unwrap_err();
        assert!(error.contains("resolve the conflicts"), "{error}");
        assert_eq!(operation_in_progress(dir.path()), Some(Operation::Merge));
        assert!(fs::read_to_string(dir.path().join("file.txt"))
            .unwrap()
            .contains("<<<<<<<"));

        assert_eq!(abort(dir.path()).unwrap(), "Merge aborted");
        assert_eq!(operation_in_progress(dir.path()), None);
        assert_eq!(
            fs::read_to_string(dir.path().join("file.txt")).unwrap(),
            "main line\n"
        );
    }

    #[test]
    fn a_conflicting_merge_is_finished_after_resolving() {
        let dir = conflicting();
        merge(dir.path(), "topic").unwrap_err();
        write(dir.path(), "file.txt", "resolved\n");
        sh(dir.path(), &["add", "file.txt"]);
        continue_operation(dir.path()).unwrap();
        assert_eq!(operation_in_progress(dir.path()), None);
        assert_eq!(
            fs::read_to_string(dir.path().join("file.txt")).unwrap(),
            "resolved\n"
        );
        assert!(head_message(dir.path()).starts_with("Merge branch 'topic'"));
    }

    #[test]
    fn rebase_replays_the_branch_on_top_of_the_target() {
        let dir = diverged();
        sh(dir.path(), &["switch", "-q", "topic"]);
        rebase(dir.path(), "main").unwrap();
        assert_eq!(current_branch(dir.path()), "topic");
        assert!(
            dir.path().join("main.txt").is_file(),
            "main's work is underneath"
        );
        assert_eq!(head_message(dir.path()), "topic work");
        assert_eq!(
            sh(dir.path(), &["rev-list", "--parents", "-n", "1", "HEAD"])
                .split(' ')
                .count(),
            2,
            "history stays linear"
        );
    }

    #[test]
    fn a_conflicting_rebase_can_be_continued_or_aborted() {
        let dir = conflicting();
        sh(dir.path(), &["switch", "-q", "topic"]);
        let error = rebase(dir.path(), "main").unwrap_err();
        assert!(error.contains("resolve the conflicts"), "{error}");
        assert_eq!(operation_in_progress(dir.path()), Some(Operation::Rebase));

        write(dir.path(), "file.txt", "resolved\n");
        sh(dir.path(), &["add", "file.txt"]);
        continue_operation(dir.path()).unwrap();
        assert_eq!(operation_in_progress(dir.path()), None);
        assert_eq!(
            fs::read_to_string(dir.path().join("file.txt")).unwrap(),
            "resolved\n"
        );

        // Start again from the original topic commit and abort this time.
        sh(dir.path(), &["reset", "-q", "--hard", "ORIG_HEAD"]);
        rebase(dir.path(), "main").unwrap_err();
        assert_eq!(abort(dir.path()).unwrap(), "Rebase aborted");
        assert_eq!(operation_in_progress(dir.path()), None);
        assert_eq!(current_branch(dir.path()), "topic");
        assert_eq!(
            fs::read_to_string(dir.path().join("file.txt")).unwrap(),
            "topic line\n"
        );
    }

    #[test]
    fn abort_and_continue_without_an_operation_explain_themselves() {
        let dir = repository();
        assert_eq!(
            abort(dir.path()).unwrap_err(),
            "No merge or rebase is in progress"
        );
        assert_eq!(
            continue_operation(dir.path()).unwrap_err(),
            "No merge or rebase is in progress"
        );
        assert_eq!(operation_in_progress(dir.path()), None);
        let outside = crate::test_support::TempDir::new();
        assert_eq!(
            operation_in_progress(outside.path()),
            None,
            "not a repository"
        );
    }

    #[test]
    fn ref_arguments_reject_options_and_whitespace() {
        assert!(plain_ref("main"));
        assert!(plain_ref("origin/feature-1"));
        assert!(!plain_ref(""));
        assert!(!plain_ref("-f"));
        assert!(!plain_ref("a b"));
        assert!(!plain_ref("a\nb"));
    }

    #[test]
    fn delete_branch_removes_merged_branches_and_protects_unmerged_and_current_ones() {
        let dir = diverged();
        // `topic` has a commit `main` does not: the safe delete refuses it.
        let error = delete_branch(dir.path(), "topic", false).unwrap_err();
        assert!(error.contains("not fully merged"), "{error}");
        assert_eq!(
            sh(dir.path(), &["branch", "--list", "topic"]).trim(),
            "topic"
        );

        merge(dir.path(), "topic").unwrap();
        assert_eq!(
            delete_branch(dir.path(), "topic", false).unwrap(),
            "Deleted branch 'topic'"
        );
        assert!(sh(dir.path(), &["branch", "--list", "topic"]).is_empty());

        // The checked out branch can not be deleted, even forced.
        assert!(delete_branch(dir.path(), "main", true).is_err());
        assert_eq!(current_branch(dir.path()), "main");
    }

    #[test]
    fn a_forced_delete_drops_an_unmerged_branch() {
        let dir = diverged();
        delete_branch(dir.path(), "topic", true).unwrap();
        assert!(sh(dir.path(), &["branch", "--list", "topic"]).is_empty());
    }

    #[test]
    fn delete_branch_refuses_unknown_and_option_like_names() {
        let dir = repository();
        assert!(delete_branch(dir.path(), "nope", false).is_err());
        let error = delete_branch(dir.path(), "--force", false).unwrap_err();
        assert!(error.contains("not a valid branch name"), "{error}");
        assert!(delete_branch(dir.path(), "", true).is_err());
    }

    #[test]
    fn snapshots_list_remote_branches_without_the_head_alias() {
        let remote = repository();
        sh(remote.path(), &["branch", "feature"]);
        let clone = crate::test_support::TempDir::new();
        sh(
            clone.path(),
            &["clone", "-q", &remote.path().display().to_string(), "."],
        );
        let snapshot = crate::scm::snapshot(clone.path());
        assert_eq!(snapshot.remote_branches, ["origin/feature", "origin/main"]);
        assert_eq!(snapshot.branches, ["main"]);
        let plain = crate::scm::snapshot(remote.path());
        assert!(
            plain.remote_branches.is_empty(),
            "a repository without remotes"
        );
    }
}
