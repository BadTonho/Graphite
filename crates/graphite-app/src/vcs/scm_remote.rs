//! Remote and history operations of the SCM view. They run the `git` command, so they use the
//! same credentials and remotes as the terminal. Callers run them on a worker thread.

use std::path::Path;
use std::process::Command;

/// Runs `git` in `root` and returns its trimmed standard output.
pub(crate) fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    // Git messages are matched in English below, so keep them out of the user's locale.
    let output = Command::new("git")
        .current_dir(root)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.is_empty() {
            format!("git {} failed", args.first().copied().unwrap_or_default())
        } else {
            message
        })
    };
    super::scm_log::record(
        args,
        match &result {
            Ok(text) => Ok(text.as_str()),
            Err(reason) => Err(reason.as_str()),
        },
    );
    result
}

/// Where a local branch stands against the branch it tracks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UpstreamStatus {
    /// Short name of the tracked branch, such as `origin/main`.
    pub name: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// The tracked branch was configured but no longer exists on the remote.
    pub gone: bool,
}

/// Reads the tracking state of `branch` with one `git` call.
pub(crate) fn upstream_status(root: &Path, branch: &str) -> UpstreamStatus {
    let reference = format!("refs/heads/{branch}");
    run_git(
        root,
        &[
            "for-each-ref",
            "--format=%(upstream:short)|%(upstream:track,nobracket)",
            &reference,
        ],
    )
    .map(|line| parse_upstream_status(&line))
    .unwrap_or_default()
}

fn parse_upstream_status(line: &str) -> UpstreamStatus {
    let (name, track) = line.split_once('|').unwrap_or((line, ""));
    let mut status = UpstreamStatus {
        name: (!name.is_empty()).then(|| name.to_owned()),
        ..UpstreamStatus::default()
    };
    for part in track.split(',').map(str::trim) {
        if part == "gone" {
            status.gone = true;
        } else if let Some(count) = part.strip_prefix("ahead ") {
            status.ahead = count.parse().unwrap_or(0);
        } else if let Some(count) = part.strip_prefix("behind ") {
            status.behind = count.parse().unwrap_or(0);
        }
    }
    status
}

pub(crate) fn init_repository(root: &Path) -> Result<String, String> {
    run_git(root, &["init"])
}

pub(crate) fn publish_github_repository(
    root: &Path,
    token: &str,
    name: &str,
    branch: &str,
    is_private: bool,
) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?;
    let body = serde_json::json!({
        "name": name,
        "private": is_private,
    });
    let response = client
        .post("https://api.github.com/user/repos")
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Graphite")
        .body(body.to_string())
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let text = response.text().map_err(|error| error.to_string())?;
    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("GitHub API error ({status})"));
        return Err(message);
    }
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let clone_url = json["clone_url"]
        .as_str()
        .ok_or_else(|| "GitHub response missing clone_url".to_owned())?;
    let html_url = json["html_url"].as_str().unwrap_or(clone_url).to_owned();
    if run_git(root, &["remote", "get-url", "origin"]).is_ok() {
        run_git(root, &["remote", "set-url", "origin", clone_url])?;
    } else {
        run_git(root, &["remote", "add", "origin", clone_url])?;
    }
    run_git(root, &["push", "-u", "origin", branch])?;
    Ok(html_url)
}

pub(crate) fn pull(root: &Path) -> Result<String, String> {
    run_git(root, &["pull", "--no-rebase"])
}

/// Pulls and replays the local commits on top of what came in.
pub(crate) fn pull_rebase(root: &Path) -> Result<String, String> {
    run_git(root, &["pull", "--rebase"])
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

/// Splits `origin/feature` into the remote and the branch, refusing anything that git could read
/// as an option.
fn split_remote_branch(remote_branch: &str) -> Result<(&str, &str), String> {
    let plain = |part: &str| {
        !part.is_empty()
            && !part.starts_with('-')
            && !part.chars().any(|c| c.is_control() || c.is_whitespace())
    };
    remote_branch
        .split_once('/')
        .filter(|(remote, branch)| plain(remote) && plain(branch))
        .ok_or_else(|| format!("'{remote_branch}' is not a remote branch"))
}

/// The folder name git would use for `url`: its last part without `.git`.
pub(crate) fn clone_folder_name(url: &str) -> Option<String> {
    let name = url
        .trim()
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\', ':'])
        .next()?
        .trim_end_matches(".git");
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_owned())
}

/// Clones `url` into a new folder inside `parent` and returns that folder.
pub(crate) fn clone(parent: &Path, url: &str) -> Result<std::path::PathBuf, String> {
    let url = url.trim();
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
        return Err("Type the address of a repository".into());
    }
    let name =
        clone_folder_name(url).ok_or_else(|| format!("Cannot tell a folder name from '{url}'"))?;
    let target = parent.join(&name);
    if target.exists()
        && target
            .read_dir()
            .map_or(true, |mut entries| entries.next().is_some())
    {
        return Err(format!(
            "'{}' already exists and is not empty",
            target.display()
        ));
    }
    run_git(parent, &["clone", "--", url, &name])?;
    Ok(target)
}

/// Pulls `remote_branch` (such as `origin/main`) into the current branch.
pub(crate) fn pull_from(root: &Path, remote_branch: &str) -> Result<String, String> {
    let (remote, branch) = split_remote_branch(remote_branch)?;
    run_git(root, &["pull", "--no-rebase", remote, branch])
}

/// Pushes the current branch to `remote` and makes it track the branch there.
pub(crate) fn push_to(root: &Path, remote: &str) -> Result<String, String> {
    if remote.is_empty()
        || remote.starts_with('-')
        || remote.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(format!("'{remote}' is not a valid remote name"));
    }
    run_git(root, &["push", "--set-upstream", remote, "HEAD"])
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
    run_git(
        root,
        &["log", "--oneline", "--decorate", "--graph", "-n", "300"],
    )
}

/// GitHub compare page for the current branch, which offers "Create pull request".
pub(crate) fn pull_request_url(root: &Path) -> Result<String, String> {
    let remote = run_git(root, &["remote", "get-url", "origin"])
        .map_err(|_| "No remote named origin".to_owned())?;
    let (owner, repo) = parse_github_remote(&remote)
        .ok_or_else(|| "The origin remote is not on GitHub".to_owned())?;
    let branch = run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(format!(
        "https://github.com/{owner}/{repo}/compare/{branch}?expand=1"
    ))
}

/// `(owner, repo)` of a GitHub remote URL, in HTTPS or SSH form.
pub(crate) fn parse_github_remote(remote: &str) -> Option<(String, String)> {
    let remote = remote.trim();
    let path = [
        "https://github.com/",
        "http://github.com/",
        "ssh://git@github.com/",
        "git@github.com:",
    ]
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
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn tracking_output_is_parsed_into_ahead_and_behind() {
        assert_eq!(parse_upstream_status(""), UpstreamStatus::default());
        assert_eq!(
            parse_upstream_status("origin/main|"),
            UpstreamStatus {
                name: Some("origin/main".into()),
                ..Default::default()
            }
        );
        let both = parse_upstream_status("origin/main|ahead 2, behind 3");
        assert_eq!((both.ahead, both.behind, both.gone), (2, 3, false));
        let behind = parse_upstream_status("origin/main|behind 1");
        assert_eq!((behind.ahead, behind.behind), (0, 1));
        assert!(parse_upstream_status("origin/main|gone").gone);
    }

    #[test]
    fn upstream_status_follows_commits_and_pushes() {
        let dir = crate::test_support::TempDir::new();
        let remote = dir.path().join("remote.git");
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(
            dir.path(),
            &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
        );
        git(&work, &["init", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        fs::write(work.join("a.txt"), "one").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-m", "first"]);
        assert_eq!(upstream_status(&work, "main").name, None);

        push(&work).unwrap();
        let pushed = upstream_status(&work, "main");
        assert_eq!(pushed.name.as_deref(), Some("origin/main"));
        assert_eq!((pushed.ahead, pushed.behind), (0, 0));

        fs::write(work.join("a.txt"), "two").unwrap();
        git(&work, &["commit", "-am", "second"]);
        let ahead = upstream_status(&work, "main");
        assert_eq!((ahead.ahead, ahead.behind), (1, 0));
    }

    #[test]
    fn the_clone_folder_is_named_like_git_does() {
        for (url, name) in [
            ("https://github.com/user/repo.git", "repo"),
            ("https://github.com/user/repo", "repo"),
            ("https://github.com/user/repo/", "repo"),
            ("git@github.com:user/repo.git", "repo"),
            ("git@host:repo.git", "repo"),
            ("C:\\repos\\thing.git", "thing"),
            ("/srv/git/thing.git", "thing"),
        ] {
            assert_eq!(clone_folder_name(url).as_deref(), Some(name), "{url}");
        }
        for bad in ["", "   ", "/", "..", ".git"] {
            assert_eq!(clone_folder_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn cloning_creates_the_folder_and_refuses_bad_or_busy_targets() {
        let dir = crate::test_support::TempDir::new();
        let source = dir.path().join("source.git");
        let work = dir.path().join("work");
        let parent = dir.path().join("parent");
        fs::create_dir_all(&work).unwrap();
        fs::create_dir_all(&parent).unwrap();
        git(
            dir.path(),
            &["init", "--bare", "-b", "main", source.to_str().unwrap()],
        );
        git(&work, &["init", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", source.to_str().unwrap()],
        );
        fs::write(work.join("a.txt"), "a").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-m", "first"]);
        git(&work, &["push", "origin", "main"]);

        let cloned = clone(&parent, source.to_str().unwrap()).unwrap();
        assert_eq!(cloned, parent.join("source"));
        assert!(cloned.join("a.txt").exists());
        assert!(
            clone(&parent, source.to_str().unwrap())
                .unwrap_err()
                .contains("already exists"),
            "a busy folder is not overwritten"
        );
        assert!(clone(&parent, "--upload-pack=evil").is_err());
        assert!(clone(&parent, "  ").is_err());
        assert!(clone(&parent, "/no/such/place.git").is_err());
    }

    #[test]
    fn pulling_from_and_pushing_to_a_chosen_remote() {
        let dir = crate::test_support::TempDir::new();
        let main_remote = dir.path().join("main.git");
        let other_remote = dir.path().join("other.git");
        let work = dir.path().join("work");
        let second = dir.path().join("second");
        fs::create_dir_all(&work).unwrap();
        for bare in [&main_remote, &other_remote] {
            git(
                dir.path(),
                &["init", "--bare", "-b", "main", bare.to_str().unwrap()],
            );
        }
        git(&work, &["init", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", main_remote.to_str().unwrap()],
        );
        git(
            &work,
            &["remote", "add", "other", other_remote.to_str().unwrap()],
        );
        fs::write(work.join("a.txt"), "one").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-m", "first"]);

        // Push to a remote that is not `origin`; the branch then tracks it.
        push_to(&work, "other").unwrap();
        assert_eq!(
            run_git(&work, &["rev-parse", "--abbrev-ref", "main@{upstream}"]).unwrap(),
            "other/main"
        );
        assert!(push_to(&work, "--all").is_err());
        assert!(push_to(&work, "").is_err());
        assert!(push_to(&work, "no such").is_err());

        // A second clone adds a commit to `other`; pull it by naming the remote branch.
        git(
            dir.path(),
            &[
                "clone",
                other_remote.to_str().unwrap(),
                second.to_str().unwrap(),
            ],
        );
        fs::write(second.join("b.txt"), "two").unwrap();
        git(&second, &["add", "b.txt"]);
        git(&second, &["commit", "-m", "second"]);
        git(&second, &["push", "origin", "main"]);
        pull_from(&work, "other/main").unwrap();
        assert!(work.join("b.txt").exists());

        assert!(pull_from(&work, "main").is_err(), "not a remote branch");
        assert!(pull_from(&work, "other/--all").is_err());
        assert!(pull_from(&work, "--all/main").is_err());
        assert!(pull_from(&work, "other/missing").is_err());
    }

    #[test]
    fn push_pull_stash_and_history_work_against_a_local_remote() {
        let dir = crate::test_support::TempDir::new();
        let remote = dir.path().join("remote.git");
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(
            dir.path(),
            &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
        );
        git(&work, &["init", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
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
        git(
            &work,
            &[
                "remote",
                "set-url",
                "origin",
                dir.path().join("missing.git").to_str().unwrap(),
            ],
        );
        assert!(push(&work).is_err());
        assert!(sync(&work).is_err());
        assert!(pull_request_url(&work).unwrap_err().contains("GitHub"));
    }
}
