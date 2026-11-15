//! Tags, remotes and stash entries: listing them for the pickers and the operations on them.
//! They run the `git` command; callers use a worker thread for anything slow.

use std::path::Path;

use super::scm_remote::run_git;

/// Whether `name` is safe to hand to git as a name: not empty, not an option, no whitespace.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Whether `reference` is a stash entry such as `stash@{2}`.
pub(crate) fn is_stash_ref(reference: &str) -> bool {
    reference
        .strip_prefix("stash@{")
        .and_then(|rest| rest.strip_suffix('}'))
        .is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

/// The tags, newest first.
pub(crate) fn tags(root: &Path) -> Vec<String> {
    run_git(root, &["tag", "--list", "--sort=-creatordate"])
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The names of the configured remotes.
pub(crate) fn remotes(root: &Path) -> Vec<String> {
    run_git(root, &["remote"])
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The stash entries as `stash@{0}: On main: message`, newest first.
pub(crate) fn stashes(root: &Path) -> Vec<String> {
    run_git(root, &["stash", "list", "--format=%gd: %gs"])
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// The `stash@{n}` at the start of an entry of [`stashes`].
pub(crate) fn stash_ref(entry: &str) -> &str {
    entry.split(':').next().unwrap_or_default()
}

pub(crate) fn delete_tag(root: &Path, name: &str) -> Result<String, String> {
    if !plain(name) {
        return Err(format!("'{name}' is not a valid tag name"));
    }
    run_git(root, &["tag", "--delete", name])?;
    Ok(format!("Deleted tag '{name}'"))
}

/// Deletes a branch on its remote; `remote_branch` is like `origin/feature`.
pub(crate) fn delete_remote_branch(root: &Path, remote_branch: &str) -> Result<String, String> {
    let (remote, branch) = remote_branch
        .split_once('/')
        .filter(|(remote, branch)| plain(remote) && plain(branch))
        .ok_or_else(|| format!("'{remote_branch}' is not a remote branch"))?;
    run_git(root, &["push", remote, "--delete", branch])?;
    Ok(format!("Deleted '{remote_branch}'"))
}

fn stash_operation(root: &Path, verb: &str, reference: &str) -> Result<String, String> {
    if !is_stash_ref(reference) {
        return Err(format!("'{reference}' is not a stash entry"));
    }
    run_git(root, &["stash", verb, reference])
}

pub(crate) fn stash_apply(root: &Path, reference: &str) -> Result<String, String> {
    stash_operation(root, "apply", reference)
}

pub(crate) fn stash_pop(root: &Path, reference: &str) -> Result<String, String> {
    stash_operation(root, "pop", reference)
}

pub(crate) fn stash_drop(root: &Path, reference: &str) -> Result<String, String> {
    stash_operation(root, "drop", reference)
}

pub(crate) fn stash_drop_all(root: &Path) -> Result<String, String> {
    run_git(root, &["stash", "clear"])?;
    Ok("Dropped all stashes".into())
}

pub(crate) fn add_remote(root: &Path, name: &str, url: &str) -> Result<String, String> {
    let url = url.trim();
    if !plain(name) || url.is_empty() || url.starts_with('-') {
        return Err("A remote needs a name and a URL".into());
    }
    run_git(root, &["remote", "add", "--", name, url])?;
    Ok(format!("Added remote '{name}'"))
}

pub(crate) fn remove_remote(root: &Path, name: &str) -> Result<String, String> {
    if !plain(name) {
        return Err(format!("'{name}' is not a valid remote name"));
    }
    run_git(root, &["remote", "remove", name])?;
    Ok(format!("Removed remote '{name}'"))
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
        fs::write(root.join("a.txt"), "a\n").unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "base"]);
        dir
    }

    #[test]
    fn stash_references_are_recognised_strictly() {
        assert!(is_stash_ref("stash@{0}"));
        assert!(is_stash_ref("stash@{12}"));
        for bad in ["", "stash@{}", "stash@{x}", "stash@{0} ", "main", "--all", "stash@{-1}"] {
            assert!(!is_stash_ref(bad), "{bad}");
        }
        assert_eq!(stash_ref("stash@{3}: On main: wip"), "stash@{3}");
    }

    #[test]
    fn tags_are_listed_and_deleted() {
        let dir = repository();
        let root = dir.path();
        assert!(tags(root).is_empty());
        git(root, &["tag", "v1"]);
        git(root, &["tag", "v2"]);
        let mut found = tags(root);
        found.sort();
        assert_eq!(found, ["v1", "v2"]);
        delete_tag(root, "v1").unwrap();
        assert_eq!(tags(root), ["v2"]);
        assert!(delete_tag(root, "missing").is_err());
        assert!(delete_tag(root, "--all").is_err());
        assert!(delete_tag(root, "a b").is_err());
    }

    #[test]
    fn stashes_are_listed_and_applied_popped_and_dropped_by_entry() {
        let dir = repository();
        let root = dir.path();
        for text in ["one", "two", "three"] {
            fs::write(root.join("a.txt"), format!("{text}\n")).unwrap();
            git(root, &["stash", "push", "-m", text]);
        }
        let entries = stashes(root);
        assert_eq!(entries.len(), 3);
        assert!(entries[0].starts_with("stash@{0}") && entries[0].contains("three"));

        // Apply keeps the entry, pop removes it.
        stash_apply(root, stash_ref(&entries[2])).unwrap();
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "one\n");
        assert_eq!(stashes(root).len(), 3);
        git(root, &["checkout", "--", "a.txt"]);
        stash_pop(root, "stash@{1}").unwrap();
        assert_eq!(stashes(root).len(), 2);
        git(root, &["checkout", "--", "a.txt"]);
        stash_drop(root, "stash@{0}").unwrap();
        assert_eq!(stashes(root).len(), 1);
        stash_drop_all(root).unwrap();
        assert!(stashes(root).is_empty());

        assert!(stash_pop(root, "main").is_err());
        assert!(stash_drop(root, "--all").is_err());
        assert!(stash_apply(root, "stash@{9}").is_err());
    }

    #[test]
    fn remotes_are_added_listed_and_removed() {
        let dir = repository();
        let root = dir.path();
        assert!(remotes(root).is_empty());
        add_remote(root, "origin", "https://example.invalid/a.git").unwrap();
        assert_eq!(remotes(root), ["origin"]);
        assert!(add_remote(root, "origin", "https://example.invalid/b.git").is_err());
        assert!(add_remote(root, "x", "").is_err());
        assert!(add_remote(root, "x", "--upload-pack=evil").is_err());
        assert!(add_remote(root, "-x", "https://example.invalid/c.git").is_err());
        remove_remote(root, "origin").unwrap();
        assert!(remotes(root).is_empty());
        assert!(remove_remote(root, "origin").is_err());
    }

    #[test]
    fn a_remote_branch_is_deleted_on_its_remote() {
        let dir = crate::test_support::TempDir::new();
        let remote = dir.path().join("remote.git");
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(dir.path(), &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
        git(&work, &["init", "-q", "-b", "main"]);
        git(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);
        fs::write(work.join("a.txt"), "a\n").unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-q", "-m", "base"]);
        git(&work, &["push", "-q", "origin", "main:topic", "main"]);

        delete_remote_branch(&work, "origin/topic").unwrap();
        let heads = run_git(&remote, &["branch", "--list"]).unwrap();
        assert!(heads.contains("main") && !heads.contains("topic"), "{heads}");
        assert!(delete_remote_branch(&work, "topic").is_err());
        assert!(delete_remote_branch(&work, "origin/--all").is_err());
        assert!(delete_remote_branch(&work, "origin/missing").is_err());
    }
}
