//! Remote and history operations of the SCM view. They run the `git` command, so they use the
//! same credentials and remotes as the terminal. Callers run them on a worker thread.

use std::path::Path;
use std::process::Command;

/// Runs `git` in `root` and returns its trimmed standard output.
pub(crate) fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.is_empty() {
            format!("git {} failed", args.first().copied().unwrap_or_default())
        } else {
            message
        })
    }
}

pub(crate) fn pull(root: &Path) -> Result<String, String> {
    run_git(root, &["pull", "--no-rebase"])
}

/// Pushes the current branch. A branch with no upstream yet is published to `origin`, as the
/// VS Code Publish action does.
pub(crate) fn push(root: &Path) -> Result<String, String> {
    match run_git(root, &["push"]) {
        Err(error) if error.contains("no upstream") || error.contains("has no upstream") => {
            run_git(root, &["push", "-u", "origin", "HEAD"])
        }
        other => other,
    }
}

pub(crate) fn fetch(root: &Path) -> Result<String, String> {
    run_git(root, &["fetch", "--prune"])
}

/// Like VS Code's Sync: pull first, then push. A failed pull stops before the push.
pub(crate) fn sync(root: &Path) -> Result<String, String> {
    pull(root)?;
    push(root)
}

/// Stashes every change, including new files.
pub(crate) fn stash(root: &Path) -> Result<String, String> {
    run_git(root, &["stash", "push", "--include-untracked"])
}

/// Recent commits as a graph, newest first.
pub(crate) fn history(root: &Path) -> Result<String, String> {
    run_git(root, &["log", "--oneline", "--decorate", "--graph", "-n", "300"])
}

/// GitHub compare page for the current branch, which offers "Create pull request".
pub(crate) fn pull_request_url(root: &Path) -> Result<String, String> {
    let remote = run_git(root, &["remote", "get-url", "origin"])
        .map_err(|_| "No remote named origin".to_owned())?;
    let (owner, repo) =
        parse_github_remote(&remote).ok_or_else(|| "The origin remote is not on GitHub".to_owned())?;
    let branch = run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(format!("https://github.com/{owner}/{repo}/compare/{branch}?expand=1"))
}

/// `(owner, repo)` of a GitHub remote URL, in HTTPS or SSH form.
pub(crate) fn parse_github_remote(remote: &str) -> Option<(String, String)> {
    let remote = remote.trim();
    let path = ["https://github.com/", "http://github.com/", "ssh://git@github.com/", "git@github.com:"]
        .iter()
        .find_map(|prefix| remote.strip_prefix(prefix))?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let owner = parts.next().filter(|owner| !owner.is_empty())?;
    let repo = parts.next().filter(|repo| !repo.is_empty())?;
    if parts.next().is_some() {
        return None;
    }
    Some((owner.to_owned(), repo.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn github_remotes_are_read_in_https_and_ssh_forms() {
        assert_eq!(
            parse_github_remote("https://github.com/BadTonho/Graphite.git"),
            Some(("BadTonho".into(), "Graphite".into()))
        );
        assert_eq!(
            parse_github_remote("git@github.com:BadTonho/Graphite.git\n"),
            Some(("BadTonho".into(), "Graphite".into()))
        );
        assert_eq!(parse_github_remote("https://gitlab.com/a/b.git"), None);
        assert_eq!(parse_github_remote("https://github.com/only-owner"), None);
        assert_eq!(parse_github_remote("https://github.com/a/b/c"), None);
    }

    /// Runs git with a fixed identity so commits work on any machine.
    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .args(args)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }

    #[test]
    fn push_pull_stash_and_history_work_against_a_local_remote() {
        let dir = crate::test_support::TempDir::new();
        let remote = dir.path().join("remote.git");
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(dir.path(), &["init", "--bare", "-b", "main", remote.to_str().unwrap()]);
        git(&work, &["init", "-b", "main"]);
        git(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);
        fs::write(work.join("a.txt"), "one").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-m", "first"]);

        assert!(push(&work).is_ok(), "push of the first commit");
        assert!(sync(&work).is_ok(), "sync with nothing new");
        assert!(fetch(&work).is_ok());
        assert!(history(&work).unwrap().contains("first"));

        fs::write(work.join("a.txt"), "two").unwrap();
        stash(&work).unwrap();
        assert_eq!(fs::read_to_string(work.join("a.txt")).unwrap(), "one");

        // A remote that cannot be reached is reported, not hidden.
        git(&work, &["remote", "set-url", "origin", dir.path().join("missing.git").to_str().unwrap()]);
        assert!(push(&work).is_err());
        assert!(sync(&work).is_err());
        assert!(pull_request_url(&work).unwrap_err().contains("GitHub"));
    }
}
