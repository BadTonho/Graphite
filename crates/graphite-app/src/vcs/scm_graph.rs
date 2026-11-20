//! Git commit graph loading and topological lane assignment for the Source Control Graph view.

use crate::process_util::HideConsole;
use std::path::Path;

use super::scm_remote::run_git;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphRef {
    HeadBranch(String),
    LocalBranch(String),
    RemoteBranch(String),
    Tag(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphCommit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub author: String,
    /// Commit date as `YYYY-MM-DD HH:MM:SS +ZZZZ`; empty when git did not give one.
    pub date: String,
    pub refs: Vec<GraphRef>,
    pub is_head: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GraphEdge {
    pub from_col: usize,
    pub to_col: usize,
    pub color_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphRow {
    pub commit: GraphCommit,
    pub lane: usize,
    pub color_index: usize,
    pub max_lanes: usize,
    /// Edges in the upper half of the row (from incoming active lanes down to `lane` or straight through).
    pub upper_edges: Vec<GraphEdge>,
    /// Edges in the lower half of the row (from `lane` down to parent lanes or straight through).
    pub lower_edges: Vec<GraphEdge>,
}

fn parse_refs(raw: &str) -> (Vec<GraphRef>, bool) {
    let mut refs = Vec::new();
    let mut is_head = false;
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(branch) = part.strip_prefix("HEAD -> ") {
            is_head = true;
            refs.push(GraphRef::HeadBranch(branch.trim().to_owned()));
        } else if part == "HEAD" {
            is_head = true;
        } else if let Some(tag) = part.strip_prefix("tag: ") {
            refs.push(GraphRef::Tag(tag.trim().to_owned()));
        } else if part.contains('/') {
            refs.push(GraphRef::RemoteBranch(part.to_owned()));
        } else {
            refs.push(GraphRef::LocalBranch(part.to_owned()));
        }
    }
    (refs, is_head)
}

pub(crate) fn parse_graph_log(output: &str, head_hash: Option<&str>) -> Vec<GraphCommit> {
    let mut commits = Vec::new();
    for line in output.lines() {
        let mut fields = line.split('\x1f');
        let Some(hash) = fields.next().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let parents_raw = fields.next().unwrap_or("");
        let author = fields.next().unwrap_or("").trim().to_owned();
        let refs_raw = fields.next().unwrap_or("");
        let subject = fields.next().unwrap_or("").trim().to_owned();
        let date = fields.next().unwrap_or("").trim().to_owned();

        let parents = parents_raw
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        let (refs, ref_is_head) = parse_refs(refs_raw);
        let is_head = ref_is_head || head_hash.is_some_and(|head| head == hash);
        let short_hash = hash.chars().take(7).collect();
        commits.push(GraphCommit {
            hash: hash.to_owned(),
            short_hash,
            parents,
            subject,
            author,
            date,
            refs,
            is_head,
        });
    }
    commits
}

#[cfg(test)]
/// The lines of the hover card of a commit: subject, who and when, the id and the refs on it.
pub(crate) fn tooltip_lines(commit: &GraphCommit) -> Vec<String> {
    let mut lines = vec![commit.subject.clone()];
    let who = match (commit.author.is_empty(), commit.date.is_empty()) {
        (false, false) => format!("{} · {}", commit.author, commit.date),
        (false, true) => commit.author.clone(),
        (true, false) => commit.date.clone(),
        (true, true) => String::new(),
    };
    if !who.is_empty() {
        lines.push(who);
    }
    lines.push(commit.hash.clone());
    let refs = commit
        .refs
        .iter()
        .map(|reference| match reference {
            GraphRef::HeadBranch(name)
            | GraphRef::LocalBranch(name)
            | GraphRef::RemoteBranch(name) => name.clone(),
            GraphRef::Tag(name) => format!("tag: {name}"),
        })
        .collect::<Vec<_>>();
    if !refs.is_empty() {
        lines.push(refs.join(", "));
    }
    lines
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommitDiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Parses `git show --shortstat --format=` output into file, insertion, and deletion counts.
pub(crate) fn parse_shortstat(output: &str) -> Option<CommitDiffStats> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| line.contains("file changed") || line.contains("files changed"))?;
    let mut stats = CommitDiffStats::default();
    for part in line.split(',').map(str::trim) {
        let count = part
            .split_whitespace()
            .next()
            .and_then(|num| num.parse::<usize>().ok())
            .unwrap_or(0);
        if part.contains("file") {
            stats.files_changed = count;
        } else if part.contains("insertion") {
            stats.insertions = count;
        } else if part.contains("deletion") {
            stats.deletions = count;
        }
    }
    Some(stats)
}

/// Loads `--shortstat` counts for `hash` in `root`.
pub(crate) fn load_commit_shortstat(root: &Path, hash: &str) -> Option<CommitDiffStats> {
    if !is_valid_commit_hash(hash) {
        return None;
    }
    let output = run_git(root, &["show", "--shortstat", "--format=", hash]).ok()?;
    parse_shortstat(&output)
}

/// Returns `https://github.com/{owner}/{repo}` when `origin` points to GitHub.
pub(crate) fn github_repo_base_url(root: &Path) -> Option<String> {
    let remote = run_git(root, &["remote", "get-url", "origin"]).ok()?;
    let (owner, repo) = super::scm_remote::parse_github_remote(&remote)?;
    Some(format!("https://github.com/{owner}/{repo}"))
}

/// Builds the GitHub commit URL from a repository base URL and commit hash.
pub(crate) fn commit_github_url(base_url: &str, hash: &str) -> String {
    format!("{}/commit/{hash}", base_url.trim_end_matches('/'))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitChangedFile {
    pub path: String,
    pub status: char,
}

/// Parses `git diff-tree --no-commit-id --name-status -r --root` output.
pub(crate) fn parse_commit_files(output: &str) -> Vec<CommitChangedFile> {
    let mut files = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split('\t');
        let Some(status_raw) = parts.next() else {
            continue;
        };
        let status = status_raw.chars().next().unwrap_or('M');
        // Renames/copies emit `<status>\t<old>\t<new>`; pick the final path.
        let Some(path) = parts.next_back().filter(|p| !p.is_empty()) else {
            continue;
        };
        files.push(CommitChangedFile {
            path: path.replace('\\', "/"),
            status,
        });
    }
    files
}

/// Lists files changed by `hash` in `root`.
pub(crate) fn load_commit_files(root: &Path, hash: &str) -> Result<Vec<CommitChangedFile>, String> {
    if !is_valid_commit_hash(hash) {
        return Err("Invalid commit hash".to_owned());
    }
    let output = run_git(
        root,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-status",
            "-r",
            "--root",
            hash,
        ],
    )?;
    Ok(parse_commit_files(&output))
}

/// Loads `(before, after)` file contents for `rel_path` at `hash` vs its first parent.
pub(crate) fn load_commit_file_comparison(
    root: &Path,
    hash: &str,
    rel_path: &str,
) -> Result<(String, String), String> {
    if !is_valid_commit_hash(hash) {
        return Err("Invalid commit hash".to_owned());
    }
    let clean = rel_path.trim().replace('\\', "/");
    let show_raw = |spec: &str| -> String {
        std::process::Command::new("git")
            .hide_console()
            .arg("-C")
            .arg(root)
            .args(["show", spec])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default()
    };
    let left = show_raw(&format!("{hash}^:{clean}"));
    let right = show_raw(&format!("{hash}:{clean}"));
    Ok((left, right))
}

fn is_valid_commit_hash(hash: &str) -> bool {
    !hash.is_empty() && hash.len() <= 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parses `YYYY-MM-DD HH:MM:SS +ZZZZ` into Unix epoch seconds.
pub(crate) fn parse_git_date_unix(date_str: &str) -> Option<i64> {
    let mut parts = date_str.split_whitespace();
    let ymd = parts.next()?;
    let hms = parts.next()?;
    let tz = parts.next().unwrap_or("+0000");

    let mut d = ymd.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;

    let mut t = hms.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let min: i64 = t.next()?.parse().ok()?;
    let sec: i64 = t.next()?.parse().ok()?;

    let tz_sign = if tz.starts_with('-') { -1i64 } else { 1i64 };
    let tz_digits = tz.trim_start_matches(['+', '-']);
    let tz_offset = if tz_digits.len() == 4 {
        let h = tz_digits[..2].parse::<i64>().ok()?;
        let m = tz_digits[2..].parse::<i64>().ok()?;
        tz_sign * (h * 3600 + m * 60)
    } else {
        0
    };

    // Howard Hinnant's days_from_civil algorithm (UTC days since 1970-01-01).
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let utc = days * 86400 + hour * 3600 + min * 60 + sec - tz_offset;
    Some(utc)
}

/// Formats relative time + readable date (`YYYY-MM-DD HH:MM`) for the commit hover card.
pub(crate) fn format_commit_when(
    date_str: &str,
    now_unix: i64,
    translator: &graphite_i18n::Translator,
) -> String {
    let trimmed = date_str.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let short_dt = trimmed.get(..16).unwrap_or(trimmed);
    let Some(ts) = parse_git_date_unix(trimmed) else {
        return short_dt.to_owned();
    };
    let diff = (now_unix - ts).max(0);
    let rel = if diff < 60 {
        translator.text("scm.tip.time.just_now").to_owned()
    } else if diff < 3600 {
        let mins = diff / 60;
        if mins == 1 {
            translator.text("scm.tip.time.minute_ago").to_owned()
        } else {
            translator.format("scm.tip.time.minutes_ago", &[("count", &mins)])
        }
    } else if diff < 86400 {
        let hours = diff / 3600;
        if hours == 1 {
            translator.text("scm.tip.time.hour_ago").to_owned()
        } else {
            translator.format("scm.tip.time.hours_ago", &[("count", &hours)])
        }
    } else if diff < 86400 * 30 {
        let days = diff / 86400;
        if days == 1 {
            translator.text("scm.tip.time.day_ago").to_owned()
        } else {
            translator.format("scm.tip.time.days_ago", &[("count", &days)])
        }
    } else if diff < 86400 * 365 {
        let months = (diff / (86400 * 30)).max(1);
        if months == 1 {
            translator.text("scm.tip.time.month_ago").to_owned()
        } else {
            translator.format("scm.tip.time.months_ago", &[("count", &months)])
        }
    } else {
        let years = (diff / (86400 * 365)).max(1);
        if years == 1 {
            translator.text("scm.tip.time.year_ago").to_owned()
        } else {
            translator.format("scm.tip.time.years_ago", &[("count", &years)])
        }
    };
    format!("{rel} ({short_dt})")
}

#[derive(Clone, Debug)]
struct ActiveLane {
    expected_hash: String,
    color_index: usize,
}

/// Assigns visual lane columns and upper/lower connector edges for each commit row.
pub(crate) fn assign_graph_lanes(commits: Vec<GraphCommit>) -> Vec<GraphRow> {
    let mut active: Vec<Option<ActiveLane>> = Vec::new();
    let mut next_color: usize = 0;
    let mut rows = Vec::with_capacity(commits.len());

    let alloc_slot = |lanes: &mut Vec<Option<ActiveLane>>, entry: ActiveLane| -> usize {
        if let Some(idx) = lanes.iter().position(Option::is_none) {
            lanes[idx] = Some(entry);
            idx
        } else {
            lanes.push(Some(entry));
            lanes.len() - 1
        }
    };

    for commit in commits {
        let incoming_len = active.len();
        let matching_cols: Vec<usize> = active
            .iter()
            .enumerate()
            .filter_map(|(col, slot)| {
                slot.as_ref()
                    .filter(|lane| lane.expected_hash == commit.hash)
                    .map(|_| col)
            })
            .collect();

        let (lane, color_index) = if let Some(&first_col) = matching_cols.first() {
            let color = active[first_col].as_ref().map_or(0, |l| l.color_index);
            (first_col, color)
        } else {
            let color = next_color;
            next_color = next_color.wrapping_add(1);
            let col = alloc_slot(
                &mut active,
                ActiveLane {
                    expected_hash: commit.hash.clone(),
                    color_index: color,
                },
            );
            (col, color)
        };

        let mut upper_edges = Vec::new();
        for (col, slot) in active.iter().enumerate() {
            if col >= incoming_len {
                continue;
            }
            if let Some(l) = slot {
                if l.expected_hash == commit.hash {
                    upper_edges.push(GraphEdge {
                        from_col: col,
                        to_col: lane,
                        color_index: l.color_index,
                    });
                } else {
                    upper_edges.push(GraphEdge {
                        from_col: col,
                        to_col: col,
                        color_index: l.color_index,
                    });
                }
            }
        }

        // Clear any merged lanes that were also pointing to this commit.
        for &col in matching_cols.iter().skip(1) {
            active[col] = None;
        }

        let mut lower_edges = Vec::new();
        if let Some(first_parent) = commit.parents.first() {
            if let Some(existing_col) = active.iter().enumerate().find_map(|(col, slot)| {
                (col != lane)
                    .then(|| {
                        slot.as_ref()
                            .filter(|l| &l.expected_hash == first_parent)
                            .map(|_| col)
                    })
                    .flatten()
            }) {
                // First parent is already tracked in another lane: connect to it and free `lane`.
                active[lane] = None;
                lower_edges.push(GraphEdge {
                    from_col: lane,
                    to_col: existing_col,
                    color_index,
                });
            } else {
                active[lane] = Some(ActiveLane {
                    expected_hash: first_parent.clone(),
                    color_index,
                });
                lower_edges.push(GraphEdge {
                    from_col: lane,
                    to_col: lane,
                    color_index,
                });
            }
        } else {
            active[lane] = None;
        }

        for parent in commit.parents.iter().skip(1) {
            if let Some(existing_col) = active
                .iter()
                .position(|slot| slot.as_ref().is_some_and(|l| &l.expected_hash == parent))
            {
                let p_color = active[existing_col]
                    .as_ref()
                    .map_or(color_index, |l| l.color_index);
                lower_edges.push(GraphEdge {
                    from_col: lane,
                    to_col: existing_col,
                    color_index: p_color,
                });
            } else {
                let p_color = next_color;
                next_color = next_color.wrapping_add(1);
                let p_col = alloc_slot(
                    &mut active,
                    ActiveLane {
                        expected_hash: parent.clone(),
                        color_index: p_color,
                    },
                );
                lower_edges.push(GraphEdge {
                    from_col: lane,
                    to_col: p_col,
                    color_index: p_color,
                });
            }
        }

        // Straight vertical edges for all other continuing lanes.
        for (col, slot) in active.iter().enumerate() {
            if col == lane {
                continue;
            }
            if let Some(l) = slot {
                if !lower_edges
                    .iter()
                    .any(|e| e.from_col == col && e.to_col == col)
                {
                    lower_edges.push(GraphEdge {
                        from_col: col,
                        to_col: col,
                        color_index: l.color_index,
                    });
                }
            }
        }

        while active.last().is_some_and(Option::is_none) {
            active.pop();
        }
        let max_lanes = (lane + 1).max(incoming_len).max(active.len());

        rows.push(GraphRow {
            commit,
            lane,
            color_index,
            max_lanes,
            upper_edges,
            lower_edges,
        });
    }
    rows
}

/// Loads up to `limit` commits from all refs in topological date-order with lane layout.
pub(crate) fn load_graph_commits(root: &Path, limit: usize) -> Result<Vec<GraphRow>, String> {
    let head = run_git(root, &["rev-parse", "HEAD"]).ok();
    let limit_arg = limit.max(1).to_string();
    let output = run_git(
        root,
        &[
            "log",
            "--all",
            "--date-order",
            "-n",
            &limit_arg,
            "--format=%H%x1f%P%x1f%an%x1f%D%x1f%s%x1f%ci",
        ],
    )?;
    let commits = parse_graph_log(&output, head.as_deref());
    Ok(assign_graph_lanes(commits))
}

#[cfg(test)]
/// Returns the commit summary and diff for inspecting a commit clicked in the Graph.
pub(crate) fn commit_details(root: &Path, hash: &str) -> Result<String, String> {
    run_git(root, &["show", "--stat", "--patch", hash])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_refs_and_parents_from_git_log_output() {
        let log = concat!(
            "aaaa1111aaaa1111aaaa1111aaaa1111aaaa1111\x1fbbbb2222bbbb2222bbbb2222bbbb2222bbbb2222 cccc3333cccc3333cccc3333cccc3333cccc3333\x1fAlice\x1fHEAD -> main, origin/main, tag: v1.0\x1fMerge branch 'feat'\n",
            "bbbb2222bbbb2222bbbb2222bbbb2222bbbb2222\x1f\x1fBob\x1f\x1fInitial commit\n"
        );
        let commits = parse_graph_log(log, None);
        assert_eq!(commits.len(), 2);
        assert!(commits[0].is_head);
        assert_eq!(commits[0].parents.len(), 2);
        assert_eq!(
            commits[0].refs,
            vec![
                GraphRef::HeadBranch("main".into()),
                GraphRef::RemoteBranch("origin/main".into()),
                GraphRef::Tag("v1.0".into()),
            ]
        );
        assert!(!commits[1].is_head);
        assert!(commits[1].parents.is_empty());
    }

    #[test]
    fn the_date_is_the_last_field_and_missing_dates_are_allowed() {
        let log = "aaaa\x1f\x1fAlice\x1fHEAD -> main, tag: v1\x1fFix it\x1f2026-01-02 03:04:05 +0000\nbbbb\x1f\x1fBob\x1f\x1fOld\n";
        let commits = parse_graph_log(log, None);
        assert_eq!(commits[0].date, "2026-01-02 03:04:05 +0000");
        assert_eq!(commits[1].date, "");
        assert_eq!(
            tooltip_lines(&commits[0]),
            [
                "Fix it",
                "Alice · 2026-01-02 03:04:05 +0000",
                "aaaa",
                "main, tag: v1"
            ]
        );
        assert_eq!(tooltip_lines(&commits[1]), ["Old", "Bob", "bbbb"]);
    }

    #[test]
    fn assigns_lanes_for_branch_and_merge() {
        let log = concat!(
            "c3\x1fc2 c1\x1fAlice\x1fHEAD -> main\x1fMerge feat\n",
            "c2\x1fc0\x1fAlice\x1ffeat\x1fFeat work\n",
            "c1\x1fc0\x1fAlice\x1f\x1fMain work\n",
            "c0\x1f\x1fAlice\x1f\x1fInit\n"
        );
        let rows = assign_graph_lanes(parse_graph_log(log, None));
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].lane, 0);
        // Merge commit c3 opens a second lane for c1.
        assert!(rows[0]
            .lower_edges
            .iter()
            .any(|e| e.from_col == 0 && e.to_col == 1));
        // c1 merges back into lane 0 for shared parent c0.
        assert!(rows[2]
            .lower_edges
            .iter()
            .any(|e| e.from_col == 1 && e.to_col == 0));
        assert_eq!(rows[3].lane, 0);
    }

    #[test]
    fn loads_real_git_repository_graph() {
        let temp = crate::test_support::TempDir::new();
        let root = temp.path();
        assert!(run_git(root, &["init"]).is_ok());
        let _ = run_git(root, &["config", "user.name", "Test"]);
        let _ = run_git(root, &["config", "user.email", "test@example.com"]);
        fs::write(root.join("a.txt"), "hello\n").unwrap();
        run_git(root, &["add", "."]).unwrap();
        run_git(root, &["commit", "-m", "first commit"]).unwrap();

        let rows = load_graph_commits(root, 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].commit.is_head);
        assert_eq!(rows[0].commit.subject, "first commit");
        assert!(
            rows[0].commit.date.starts_with("20"),
            "{}",
            rows[0].commit.date
        );
        assert!(commit_details(root, &rows[0].commit.hash)
            .unwrap()
            .contains("first commit"));
    }

    #[test]
    fn parses_shortstat_and_formats_commit_when_and_github_url() {
        assert_eq!(
            parse_shortstat(" 7 files changed, 316 insertions(+), 3 deletions(-)\n"),
            Some(CommitDiffStats {
                files_changed: 7,
                insertions: 316,
                deletions: 3,
            })
        );
        assert_eq!(
            parse_shortstat(" 1 file changed, 4 insertions(+)\n"),
            Some(CommitDiffStats {
                files_changed: 1,
                insertions: 4,
                deletions: 0,
            })
        );
        assert_eq!(parse_shortstat(""), None);

        assert_eq!(
            commit_github_url("https://github.com/owner/repo", "abc1234"),
            "https://github.com/owner/repo/commit/abc1234"
        );

        let ts = parse_git_date_unix("2026-11-16 20:41:26 -0300").unwrap();
        let translator = graphite_i18n::Translator::english();
        let when = format_commit_when("2026-11-16 20:41:26 -0300", ts + 480, &translator);
        assert!(when.contains("8 minutes ago"), "{when}");
        assert!(when.contains("2026-11-16 20:41"), "{when}");
    }

    #[test]
    fn loads_commit_changed_files_and_parent_comparison() {
        let temp = crate::test_support::TempDir::new();
        let root = temp.path();
        assert!(run_git(root, &["init"]).is_ok());
        let _ = run_git(root, &["config", "user.name", "Test"]);
        let _ = run_git(root, &["config", "user.email", "test@example.com"]);

        fs::write(root.join("a.txt"), "v1\n").unwrap();
        run_git(root, &["add", "."]).unwrap();
        run_git(root, &["commit", "-m", "init"]).unwrap();

        fs::write(root.join("a.txt"), "v2\n").unwrap();
        fs::write(root.join("b.txt"), "added\n").unwrap();
        run_git(root, &["add", "."]).unwrap();
        run_git(root, &["commit", "-m", "second"]).unwrap();

        let rows = load_graph_commits(root, 10).unwrap();
        let second_hash = &rows[0].commit.hash;
        let files = load_commit_files(root, second_hash).unwrap();
        assert_eq!(
            files,
            vec![
                CommitChangedFile {
                    path: "a.txt".into(),
                    status: 'M'
                },
                CommitChangedFile {
                    path: "b.txt".into(),
                    status: 'A'
                },
            ]
        );

        let (left_a, right_a) = load_commit_file_comparison(root, second_hash, "a.txt").unwrap();
        assert_eq!(left_a.replace("\r\n", "\n"), "v1\n");
        assert_eq!(right_a.replace("\r\n", "\n"), "v2\n");

        let (left_b, right_b) = load_commit_file_comparison(root, second_hash, "b.txt").unwrap();
        assert_eq!(left_b, "");
        assert_eq!(right_b.replace("\r\n", "\n"), "added\n");
    }
}
