//! Git commit graph loading and topological lane assignment for the Source Control Graph view.

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
            if let Some(existing_col) = active.iter().position(|slot| {
                slot.as_ref()
                    .is_some_and(|l| &l.expected_hash == parent)
            }) {
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
                if !lower_edges.iter().any(|e| e.from_col == col && e.to_col == col) {
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
        assert!(rows[0].lower_edges.iter().any(|e| e.from_col == 0 && e.to_col == 1));
        // c1 merges back into lane 0 for shared parent c0.
        assert!(rows[2].lower_edges.iter().any(|e| e.from_col == 1 && e.to_col == 0));
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
        assert!(rows[0].commit.date.starts_with("20"), "{}", rows[0].commit.date);
        assert!(commit_details(root, &rows[0].commit.hash)
            .unwrap()
            .contains("first commit"));
    }
}
