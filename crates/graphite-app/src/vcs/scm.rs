//! Native Git operations used by the SCM view. All callers run this module on
//! a worker thread so a large repository cannot stall the window event loop.

use gix::{bstr::ByteSlice, status::Item as StatusItem};
use graphite_i18n::Translator;
use similar::TextDiff;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Whether a file-system event can change what the SCM view shows. Reading files (inotify
/// reports `Access` events) and Git's own bookkeeping inside `.git` (index and lock files,
/// rewritten by every status refresh) must not trigger another refresh, or the refresh
/// feeds itself. Build output under `target` is ignored for the same reason.
pub(crate) fn is_relevant_watch_event(event: &notify::Event) -> bool {
    use notify::EventKind;
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.is_empty() || event.paths.iter().any(|path| is_relevant_watch_path(path))
}

fn is_relevant_watch_path(path: &Path) -> bool {
    let names: Vec<_> = path.components().map(|part| part.as_os_str()).collect();
    if let Some(git) = names.iter().position(|name| *name == ".git") {
        // Branch switches and commits move HEAD or refs; everything else is Git internals.
        return names
            .get(git + 1)
            .is_some_and(|name| *name == "HEAD" || *name == "refs" || *name == "packed-refs");
    }
    !names.iter().any(|name| *name == "target")
}

/// How a file differs from the commit or the index it is compared with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChange,
    /// A new file that git does not track yet.
    Untracked,
    Conflict,
}

impl ChangeKind {
    /// The letter shown at the end of the row.
    pub(crate) fn letter(self) -> &'static str {
        match self {
            Self::Modified | Self::TypeChange => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::Untracked => "U",
            Self::Conflict => "!",
        }
    }

    /// The color of the letter, as `0xAARRGGBB`.
    pub(crate) fn color(self) -> u32 {
        match self {
            Self::Modified | Self::TypeChange => 0xffe2c08d,
            Self::Added => 0xff81b88b,
            Self::Untracked | Self::Renamed | Self::Copied => 0xff73c991,
            Self::Deleted => 0xffc74e39,
            Self::Conflict => 0xffe4676b,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FileStatus {
    pub path: PathBuf,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
    /// How the staged side differs from HEAD; `None` when nothing is staged.
    pub staged_kind: Option<ChangeKind>,
    /// How the working tree differs from the index; `None` when it does not.
    pub worktree_kind: Option<ChangeKind>,
}

impl FileStatus {
    /// The change shown for this file in the staged or the unstaged list.
    pub(crate) fn kind(&self, staged: bool) -> ChangeKind {
        if self.conflicted {
            return ChangeKind::Conflict;
        }
        let kind = if staged {
            self.staged_kind
        } else {
            self.worktree_kind
        };
        kind.unwrap_or(if self.untracked && !staged {
            ChangeKind::Untracked
        } else {
            ChangeKind::Modified
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineChange {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub branch: Option<String>,
    pub branches: Vec<String>,
    /// Remote-tracking branches such as `origin/main` (never `origin/HEAD`).
    pub remote_branches: Vec<String>,
    pub files: Vec<FileStatus>,
    pub line_changes: BTreeMap<PathBuf, Vec<(usize, LineChange)>>,
    pub has_commits: bool,
    pub has_remote: bool,
    /// Short name of the branch the current branch tracks, such as `origin/main`.
    pub upstream: Option<String>,
    /// Commits the current branch has that its upstream lacks.
    pub ahead: usize,
    /// Commits the upstream has that the current branch lacks.
    pub behind: usize,
    pub error: Option<String>,
}

pub(crate) fn snapshot(root: &Path) -> Snapshot {
    match snapshot_inner(root) {
        Ok(snapshot) => snapshot,
        Err(error) => Snapshot {
            error: Some(error),
            ..Snapshot::default()
        },
    }
}

fn snapshot_inner(root: &Path) -> Result<Snapshot, String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string());
    let branches = repo
        .references()
        .map_err(display_error)?
        .local_branches()
        .map_err(display_error)?
        .filter_map(Result::ok)
        .map(|reference| reference.name().shorten().to_string())
        .collect::<Vec<_>>();
    let mut remote_branches = repo
        .references()
        .map_err(display_error)?
        .remote_branches()
        .map_err(display_error)?
        .filter_map(Result::ok)
        .map(|reference| reference.name().shorten().to_string())
        .filter(|name| !name.ends_with("/HEAD"))
        .collect::<Vec<_>>();
    remote_branches.sort();
    let mut files = BTreeMap::<PathBuf, FileStatus>::new();
    let changes = repo
        .status(gix::progress::Discard)
        .map_err(display_error)?
        .into_iter(std::iter::empty())
        .map_err(display_error)?;
    for item in changes {
        let item = item.map_err(display_error)?;
        let path = PathBuf::from(item.location().to_str_lossy().into_owned());
        let change = files.entry(path.clone()).or_insert_with(|| FileStatus {
            path,
            ..FileStatus::default()
        });
        match item {
            StatusItem::TreeIndex(tree_change) => {
                change.staged = true;
                change.staged_kind = Some(match tree_change {
                    gix::diff::index::Change::Addition { .. } => ChangeKind::Added,
                    gix::diff::index::Change::Deletion { .. } => ChangeKind::Deleted,
                    gix::diff::index::Change::Modification { .. } => ChangeKind::Modified,
                    gix::diff::index::Change::Rewrite { copy: true, .. } => ChangeKind::Copied,
                    gix::diff::index::Change::Rewrite { .. } => ChangeKind::Renamed,
                });
            }
            StatusItem::IndexWorktree(item) => {
                change.unstaged = true;
                if let Some(summary) = item.summary() {
                    let summary = format!("{summary:?}");
                    change.untracked |= summary == "Added";
                    change.conflicted |= summary == "Conflict";
                    change.worktree_kind = Some(match summary.as_str() {
                        "Removed" => ChangeKind::Deleted,
                        "Added" => ChangeKind::Untracked,
                        "IntentToAdd" => ChangeKind::Added,
                        "Renamed" => ChangeKind::Renamed,
                        "Copied" => ChangeKind::Copied,
                        "TypeChange" => ChangeKind::TypeChange,
                        "Conflict" => ChangeKind::Conflict,
                        _ => ChangeKind::Modified,
                    });
                }
            }
        }
    }
    if let Ok(index) = repo.open_index() {
        for entry in index.entries() {
            if entry.stage() != gix::index::entry::Stage::Unconflicted {
                if let Ok(path) = entry.path(&index).to_str() {
                    let path = PathBuf::from(path);
                    let change = files.entry(path.clone()).or_insert_with(|| FileStatus {
                        path,
                        ..FileStatus::default()
                    });
                    change.conflicted = true;
                }
            }
        }
    }
    let files = files.into_values().collect::<Vec<_>>();
    let line_changes = collect_line_changes(&repo, &files);
    let has_commits = repo.head_commit().is_ok();
    let has_remote = !repo.remote_names().is_empty();
    let tracking = match (&branch, has_remote && has_commits) {
        (Some(branch), true) => {
            super::scm_remote::upstream_status(repo.workdir().unwrap_or(root), branch)
        }
        _ => Default::default(),
    };
    Ok(Snapshot {
        upstream: tracking.name.filter(|_| !tracking.gone),
        ahead: tracking.ahead,
        behind: tracking.behind,
        branch,
        branches,
        remote_branches,
        files,
        line_changes,
        has_commits,
        has_remote,
        error: None,
    })
}

fn collect_line_changes(
    repo: &gix::Repository,
    files: &[FileStatus],
) -> BTreeMap<PathBuf, Vec<(usize, LineChange)>> {
    let Some(workdir) = repo.workdir() else {
        return BTreeMap::new();
    };
    let Ok(tree) = repo.head_tree_id_or_empty() else {
        return BTreeMap::new();
    };
    let Ok(head_index) = repo.index_from_tree(&tree) else {
        return BTreeMap::new();
    };
    let mut changes_by_path = BTreeMap::new();
    for file in files {
        if file.conflicted {
            continue;
        }
        let relative = file.path.to_string_lossy().replace('\\', "/");
        let relative_bstr = relative.as_bytes().as_bstr();
        let base = head_index
            .entries()
            .iter()
            .find(|entry| {
                entry.stage() == gix::index::entry::Stage::Unconflicted
                    && entry.path(&head_index) == relative_bstr
            })
            .and_then(|entry| repo.find_blob(entry.id).ok())
            .and_then(|blob| String::from_utf8(blob.data.to_vec()).ok())
            .unwrap_or_default();
        let changed = fs::read_to_string(workdir.join(&file.path)).unwrap_or_default();
        let decorations = diff_line_changes(&base, &changed);
        if !decorations.is_empty() {
            changes_by_path.insert(file.path.clone(), decorations);
        }
    }
    changes_by_path
}

fn diff_line_changes(base: &str, changed: &str) -> Vec<(usize, LineChange)> {
    let mut result = BTreeMap::<usize, LineChange>::new();
    for op in TextDiff::from_lines(base, changed).ops() {
        match *op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete { new_index, .. } => {
                let line = new_index.min(changed.lines().count().saturating_sub(1));
                result.insert(line, LineChange::Deleted);
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for line in new_index..new_index + new_len {
                    result.insert(line, LineChange::Added);
                }
            }
            similar::DiffOp::Replace {
                new_index,
                old_len,
                new_len,
                ..
            } => {
                let paired = old_len.min(new_len);
                for line in new_index..new_index + paired {
                    result.insert(line, LineChange::Modified);
                }
                for line in new_index + paired..new_index + new_len {
                    result.insert(line, LineChange::Added);
                }
                if old_len > new_len {
                    let line = (new_index + new_len).min(changed.lines().count().saturating_sub(1));
                    result.insert(line, LineChange::Deleted);
                }
            }
        }
    }
    result.into_iter().collect()
}

pub(crate) fn checkout_branch(
    root: &Path,
    branch: &str,
    translator: &Translator,
) -> Result<(), String> {
    let snapshot = snapshot_inner(root)?;
    if !snapshot.files.is_empty() {
        return Err(translator.text("scm.error.checkout_dirty").into());
    }
    let local = snapshot
        .branches
        .iter()
        .any(|candidate| candidate == branch);
    let remote = snapshot
        .remote_branches
        .iter()
        .any(|candidate| candidate == branch);
    if !local && !remote {
        return Err(translator.format("scm.error.unknown_branch", &[("branch", &branch)]));
    }
    // A remote-tracking branch such as `origin/feature` becomes a local branch that tracks it.
    let arguments: &[&str] = if local {
        &["switch", "--", branch]
    } else {
        &["switch", "--track", "--", branch]
    };
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(arguments)
        .output()
        .map_err(|error| translator.format("scm.error.checkout_start", &[("error", &error)]))?;
    if output.status.success() {
        Ok(())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if message.is_empty() {
            translator.format("scm.error.checkout_failed", &[("status", &output.status)])
        } else {
            message
        })
    }
}

pub(crate) fn stage_file(root: &Path, path: &Path, translator: &Translator) -> Result<(), String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let relative_bstr = relative_text.as_bytes().as_bstr();
    let absolute = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_stage").to_owned())?
        .join(&relative);
    let mut index = open_or_default_index(&repo)?;
    // Re-staging a resolved conflict removes all non-zero index stages.
    index.remove_entries(|_, entry_path, _| entry_path == relative_bstr);
    if absolute.exists() {
        let bytes = fs::read(&absolute).map_err(display_error)?;
        let blob = repo.write_blob(bytes).map_err(display_error)?.detach();
        let mode = file_mode(&absolute);
        index.dangerously_push_entry(
            Default::default(),
            blob,
            gix::index::entry::Flags::from_stage(gix::index::entry::Stage::Unconflicted),
            mode,
            relative_bstr,
        );
    }
    index.sort_entries();
    index.remove_tree();
    index
        .write(gix::index::write::Options::default())
        .map_err(display_error)
}

pub(crate) fn unstage_file(
    root: &Path,
    path: &Path,
    translator: &Translator,
) -> Result<(), String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let relative_bstr = relative_text.as_bytes().as_bstr();
    let mut index = open_or_default_index(&repo)?;
    let head_index = repo
        .index_from_tree(&repo.head_tree_id_or_empty().map_err(display_error)?)
        .map_err(display_error)?;
    let head_entry = head_index
        .entries()
        .iter()
        .find(|entry| {
            entry.stage() == gix::index::entry::Stage::Unconflicted
                && entry.path(&head_index) == relative_bstr
        })
        .map(|entry| (entry.id, entry.mode));
    index.remove_entries(|_, entry_path, _| entry_path == relative_bstr);
    if let Some((id, mode)) = head_entry {
        index.dangerously_push_entry(
            Default::default(),
            id,
            gix::index::entry::Flags::from_stage(gix::index::entry::Stage::Unconflicted),
            mode,
            relative_bstr,
        );
    }
    index.sort_entries();
    index.remove_tree();
    index
        .write(gix::index::write::Options::default())
        .map_err(display_error)
}

pub(crate) fn stage_files(
    root: &Path,
    paths: &[PathBuf],
    translator: &Translator,
) -> Result<(), String> {
    for path in paths {
        stage_file(root, path, translator)?;
    }
    Ok(())
}

pub(crate) fn unstage_files(
    root: &Path,
    paths: &[PathBuf],
    translator: &Translator,
) -> Result<(), String> {
    for path in paths {
        unstage_file(root, path, translator)?;
    }
    Ok(())
}

pub(crate) fn commit(
    root: &Path,
    message: &str,
    translator: &Translator,
) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err(translator.text("scm.error.empty_commit").into());
    }
    let repo = gix::discover(root).map_err(display_error)?;
    let index = open_or_default_index(&repo)?;
    if index
        .entries()
        .iter()
        .any(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
    {
        return Err(translator.text("scm.error.unresolved_conflicts").into());
    }
    let mut editor = repo
        .edit_tree(gix::hash::ObjectId::empty_tree(repo.object_hash()))
        .map_err(display_error)?;
    for entry in index.entries() {
        let path = entry
            .path(&index)
            .to_str()
            .map_err(|_| translator.text("scm.error.non_utf8_paths"))?;
        let kind = match entry.mode.bits() {
            0o100755 => gix::object::tree::EntryKind::BlobExecutable,
            0o120000 => gix::object::tree::EntryKind::Link,
            0o160000 => gix::object::tree::EntryKind::Commit,
            _ => gix::object::tree::EntryKind::Blob,
        };
        editor.upsert(path, kind, entry.id).map_err(display_error)?;
    }
    let tree = editor.write().map_err(display_error)?.detach();
    let parent = repo.head_commit().ok().map(|commit| commit.id().detach());
    let parents = parent.iter().copied().collect::<Vec<_>>();
    let id = repo
        .commit("HEAD", message, tree, parents)
        .map_err(display_error)?
        .detach();
    Ok(id.to_string())
}

pub(crate) fn diff_text(left: &str, right: &str) -> String {
    TextDiff::from_lines(left, right)
        .unified_diff()
        .header("left", "right")
        .to_string()
}

pub(crate) fn head_file_text(
    root: &Path,
    path: &Path,
    translator: &Translator,
) -> Result<String, String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let relative_bstr = relative_text.as_bytes().as_bstr();
    let index = repo
        .index_from_tree(&repo.head_tree_id_or_empty().map_err(display_error)?)
        .map_err(display_error)?;
    let entry = index
        .entries()
        .iter()
        .find(|entry| entry.path(&index) == relative_bstr)
        .ok_or_else(|| translator.text("scm.error.file_not_in_head"))?;
    let blob = repo.find_blob(entry.id).map_err(display_error)?;
    String::from_utf8(blob.data.to_vec())
        .map_err(|_| translator.text("scm.error.head_not_utf8").into())
}

pub(crate) fn stage_hunk(
    root: &Path,
    path: &Path,
    hunk: usize,
    translator: &Translator,
) -> Result<(), String> {
    update_selected_hunk(root, path, hunk, true, translator)
}

pub(crate) fn stage_line(
    root: &Path,
    path: &Path,
    line: usize,
    translator: &Translator,
) -> Result<(), String> {
    let (base, work) = index_and_worktree(root, path, translator)?;
    let staged = apply_line_selection(&base, &work, line, true)
        .ok_or_else(|| translator.text("scm.error.no_unstaged_line"))?;
    write_index_blob(root, path, staged.as_bytes(), translator)
}

pub(crate) fn revert_hunk(
    root: &Path,
    path: &Path,
    hunk: usize,
    translator: &Translator,
) -> Result<(), String> {
    update_selected_hunk(root, path, hunk, false, translator)
}

pub(crate) fn revert_line(
    root: &Path,
    path: &Path,
    line: usize,
    translator: &Translator,
) -> Result<(), String> {
    let (index, work) = index_and_worktree(root, path, translator)?;
    let reverted = apply_line_selection(&work, &index, line, false)
        .ok_or_else(|| translator.text("scm.error.no_unstaged_line"))?;
    let repo = gix::discover(root).map_err(display_error)?;
    let absolute = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_revert").to_owned())?
        .join(relative_path(&repo, path, translator)?);
    fs::write(absolute, reverted).map_err(display_error)
}

/// Select one source line from a diff. `source_is_changed` chooses line numbers
/// from `changed` (staging) or `base` (reverting); unaffected lines retain the
/// base version. Equal-sized replacement blocks are split line by line.
fn apply_line_selection(
    base: &str,
    changed: &str,
    line: usize,
    source_is_changed: bool,
) -> Option<String> {
    let old: Vec<_> = base.split_inclusive('\n').collect();
    let new: Vec<_> = changed.split_inclusive('\n').collect();
    let diff = TextDiff::from_lines(base, changed);
    let mut output = String::new();
    let mut old_pos = 0usize;
    let mut found = false;

    for op in diff.ops() {
        match *op {
            similar::DiffOp::Equal { old_index, len, .. } => {
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                }
                output.push_str(&old[old_index..old_index + len].concat());
                old_pos = old_index + len;
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                }
                let selected =
                    !source_is_changed && (old_index..old_index + old_len).contains(&line);
                if selected {
                    found = true;
                } else {
                    output.push_str(&old[old_index..old_index + old_len].concat());
                }
                old_pos = old_index + old_len;
            }
            similar::DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                    old_pos = old_index;
                }
                if source_is_changed && (new_index..new_index + new_len).contains(&line) {
                    output.push_str(new[new_index + line.saturating_sub(new_index)]);
                    found = true;
                }
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                }
                if old_len == new_len {
                    for offset in 0..old_len {
                        let source_line = if source_is_changed {
                            new_index + offset
                        } else {
                            old_index + offset
                        };
                        if source_line == line {
                            output.push_str(new[new_index + offset]);
                            found = true;
                        } else {
                            output.push_str(old[old_index + offset]);
                        }
                    }
                } else {
                    let source_range = if source_is_changed {
                        new_index..new_index + new_len
                    } else {
                        old_index..old_index + old_len
                    };
                    if source_range.contains(&line) {
                        output.push_str(&new[new_index..new_index + new_len].concat());
                        found = true;
                    } else {
                        output.push_str(&old[old_index..old_index + old_len].concat());
                    }
                }
                old_pos = old_index + old_len;
            }
        }
    }
    if old_pos < old.len() {
        output.push_str(&old[old_pos..].concat());
    }
    found.then_some(output)
}

fn update_selected_hunk(
    root: &Path,
    path: &Path,
    hunk: usize,
    stage: bool,
    translator: &Translator,
) -> Result<(), String> {
    let (index_text, work_text) = index_and_worktree(root, path, translator)?;
    let (base, changed) = if stage {
        (&index_text, &work_text)
    } else {
        (&work_text, &index_text)
    };
    let merged = apply_hunk_selection(base, changed, hunk)
        .ok_or_else(|| translator.text("scm.error.no_hunk"))?;
    if stage {
        write_index_blob(root, path, merged.as_bytes(), translator)
    } else {
        let repo = gix::discover(root).map_err(display_error)?;
        let absolute = repo
            .workdir()
            .ok_or_else(|| translator.text("scm.error.bare_revert").to_owned())?
            .join(relative_path(&repo, path, translator)?);
        fs::write(absolute, merged).map_err(display_error)
    }
}

pub(crate) fn merge_conflict(
    root: &Path,
    path: &Path,
    translator: &Translator,
) -> Result<[Option<String>; 3], String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let relative_bstr = relative_text.as_bytes().as_bstr();
    let index = repo.open_index().map_err(display_error)?;
    let mut sides: [Option<String>; 3] = [None, None, None];
    for entry in index
        .entries()
        .iter()
        .filter(|entry| entry.path(&index) == relative_bstr)
    {
        let stage = entry.stage() as usize;
        if (1..=3).contains(&stage) {
            let blob = repo.find_blob(entry.id).map_err(display_error)?;
            sides[stage - 1] = Some(
                String::from_utf8(blob.data.to_vec())
                    .map_err(|_| translator.text("scm.error.conflict_not_utf8"))?,
            );
        }
    }
    if sides.iter().all(Option::is_none) {
        return Err(translator.text("scm.error.no_conflict").into());
    }
    Ok(sides)
}

pub(crate) fn resolve_conflict(
    root: &Path,
    path: &Path,
    contents: &str,
    translator: &Translator,
) -> Result<(), String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let absolute = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_conflict").to_owned())?
        .join(&relative);
    fs::write(&absolute, contents).map_err(display_error)?;
    stage_file(root, path, translator)
}

fn write_index_blob(
    root: &Path,
    path: &Path,
    bytes: &[u8],
    translator: &Translator,
) -> Result<(), String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let relative_bstr = relative_text.as_bytes().as_bstr();
    let mut index = open_or_default_index(&repo)?;
    index.remove_entries(|_, entry_path, entry| {
        entry_path == relative_bstr && entry.stage() == gix::index::entry::Stage::Unconflicted
    });
    let blob = repo.write_blob(bytes).map_err(display_error)?.detach();
    let absolute = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_stage").to_owned())?
        .join(&relative);
    index.dangerously_push_entry(
        Default::default(),
        blob,
        gix::index::entry::Flags::from_stage(gix::index::entry::Stage::Unconflicted),
        file_mode(&absolute),
        relative_bstr,
    );
    index.sort_entries();
    index.remove_tree();
    index
        .write(gix::index::write::Options::default())
        .map_err(display_error)
}

fn index_and_worktree(
    root: &Path,
    path: &Path,
    translator: &Translator,
) -> Result<(String, String), String> {
    let repo = gix::discover(root).map_err(display_error)?;
    let relative = relative_path(&repo, path, translator)?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    let relative_bstr = relative_text.as_bytes().as_bstr();
    let absolute = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_repository").to_owned())?
        .join(&relative);
    let work = fs::read_to_string(&absolute).map_err(display_error)?;
    let index = open_or_default_index(&repo)?;
    let entry = index.entries().iter().find(|entry| {
        entry.path(&index) == relative_bstr
            && entry.stage() == gix::index::entry::Stage::Unconflicted
    });
    let base = if let Some(entry) = entry {
        let blob = repo.find_blob(entry.id).map_err(display_error)?;
        String::from_utf8(blob.data.to_vec())
            .map_err(|_| translator.text("scm.error.index_not_utf8"))?
    } else {
        String::new()
    };
    Ok((base, work))
}

fn relative_path(
    repo: &gix::Repository,
    path: &Path,
    translator: &Translator,
) -> Result<PathBuf, String> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| translator.text("scm.error.bare_repository"))?;
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workdir.join(path)
    };
    let relative = path
        .strip_prefix(workdir)
        .map_err(|_| translator.text("scm.error.path_outside"))?;
    if relative
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(translator.text("scm.error.path_escapes").into());
    }
    Ok(PathBuf::from(relative.to_string_lossy().replace('\\', "/")))
}

fn open_or_default_index(repo: &gix::Repository) -> Result<gix::index::File, String> {
    gix::index::File::at_or_default(
        repo.index_path(),
        repo.object_hash(),
        false,
        gix::index::decode::Options::default(),
    )
    .map_err(display_error)
}

fn file_mode(path: &Path) -> gix::index::entry::Mode {
    #[cfg(not(unix))]
    let _ = path;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0) {
            return gix::index::entry::Mode::FILE_EXECUTABLE;
        }
    }
    gix::index::entry::Mode::FILE
}

fn apply_hunk_selection(base: &str, changed: &str, selected: usize) -> Option<String> {
    let old: Vec<_> = base.split_inclusive('\n').collect();
    let new: Vec<_> = changed.split_inclusive('\n').collect();
    let diff = TextDiff::from_lines(base, changed);
    let ops = diff.ops();
    let mut output = String::new();
    let mut old_pos = 0usize;
    let mut hunk_index = 0usize;
    let mut found = false;
    let mut changed_group = false;
    for op in ops {
        match *op {
            similar::DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                if changed_group {
                    hunk_index += 1;
                    changed_group = false;
                }
                output.push_str(&old[old_index..old_index + len].concat());
                old_pos = old_index + len;
                let _ = new_index;
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                if !changed_group {
                    changed_group = true;
                }
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                }
                if hunk_index != selected {
                    output.push_str(&old[old_index..old_index + old_len].concat());
                }
                old_pos = old_index + old_len;
                found |= hunk_index == selected;
            }
            similar::DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                if !changed_group {
                    changed_group = true;
                }
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                    old_pos = old_index;
                }
                if hunk_index == selected {
                    output.push_str(&new[new_index..new_index + new_len].concat());
                    found = true;
                }
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                if !changed_group {
                    changed_group = true;
                }
                if old_pos < old_index {
                    output.push_str(&old[old_pos..old_index].concat());
                }
                old_pos = old_index + old_len;
                if hunk_index == selected {
                    output.push_str(&new[new_index..new_index + new_len].concat());
                    found = true;
                } else {
                    output.push_str(&old[old_index..old_index + old_len].concat());
                }
            }
        }
    }
    if changed_group {
        hunk_index += 1;
    }
    if old_pos < old.len() {
        output.push_str(&old[old_pos..].concat());
    }
    (found && hunk_index > selected).then_some(output)
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Puts a file of an initialised repository in the conflicted state, with base, ours and theirs
/// stored as index stages, the way a stopped merge leaves it.
#[cfg(test)]
pub(crate) fn mark_conflicted_for_tests(
    root: &Path,
    name: &str,
    base: &str,
    ours: &str,
    theirs: &str,
) {
    let repository = gix::discover(root).unwrap();
    let mut index = open_or_default_index(&repository).unwrap();
    index.remove_entries(|_, path, _| path == name.as_bytes().as_bstr());
    for (stage, text) in [
        (gix::index::entry::Stage::Base, base),
        (gix::index::entry::Stage::Ours, ours),
        (gix::index::entry::Stage::Theirs, theirs),
    ] {
        let id = repository.write_blob(text).unwrap().detach();
        index.dangerously_push_entry(
            Default::default(),
            id,
            stage.into(),
            gix::index::entry::Mode::FILE,
            name.as_bytes().as_bstr(),
        );
    }
    index.sort_entries();
    index.write(gix::index::write::Options::default()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn translator() -> Translator {
        Translator::english()
    }

    fn repo() -> (TempDir, PathBuf) {
        let dir = TempDir::new();
        gix::init(dir.path()).unwrap();
        let file = dir.path().join("note.txt");
        fs::write(&file, "one\n").unwrap();
        (dir, file)
    }

    #[test]
    fn snapshot_lists_untracked_and_staging_writes_index() {
        let (dir, file) = repo();
        let initial = snapshot(dir.path());
        assert!(initial
            .files
            .iter()
            .any(|item| item.untracked && item.path == PathBuf::from("note.txt")));
        assert_eq!(
            initial.line_changes.get(Path::new("note.txt")),
            Some(&vec![(0, LineChange::Added)])
        );
        stage_file(dir.path(), &file, &translator()).unwrap();
        let status = snapshot(dir.path());
        assert!(status
            .files
            .iter()
            .any(|item| item.staged && !item.unstaged));
    }

    #[test]
    fn each_list_shows_how_the_file_changed_on_its_own_side() {
        let (dir, file) = repo();
        let find = |snapshot: &Snapshot| {
            snapshot
                .files
                .iter()
                .find(|item| item.path == PathBuf::from("note.txt"))
                .cloned()
                .unwrap()
        };
        // A new file is untracked, then added once staged.
        let new = find(&snapshot(dir.path()));
        assert_eq!(new.kind(false), ChangeKind::Untracked);
        assert_eq!(new.kind(false).letter(), "U");
        stage_file(dir.path(), &file, &translator()).unwrap();
        let staged = find(&snapshot(dir.path()));
        assert_eq!(staged.staged_kind, Some(ChangeKind::Added));
        assert_eq!(staged.kind(true).letter(), "A");

        // After committing, an edit is a modification in the working tree only.
        commit(dir.path(), "first", &translator()).unwrap();
        std::fs::write(&file, "changed\n").unwrap();
        let edited = find(&snapshot(dir.path()));
        assert_eq!(edited.kind(false), ChangeKind::Modified);
        assert_eq!(edited.staged_kind, None);

        // Staging a deletion and deleting in the working tree are both deletions.
        std::fs::remove_file(&file).unwrap();
        let removed = find(&snapshot(dir.path()));
        assert_eq!(removed.kind(false), ChangeKind::Deleted);
        stage_file(dir.path(), &file, &translator()).unwrap();
        let removed = find(&snapshot(dir.path()));
        assert_eq!(removed.kind(true), ChangeKind::Deleted);
        assert_eq!(removed.kind(true).letter(), "D");
    }

    #[test]
    fn staged_and_worktree_changes_are_reported_separately_for_same_file() {
        let (dir, file) = repo();
        stage_file(dir.path(), &file, &translator()).unwrap();
        fs::write(&file, "two\n").unwrap();
        let status = snapshot(dir.path());
        let changed = status
            .files
            .iter()
            .find(|item| item.path == Path::new("note.txt"))
            .unwrap();
        assert!(changed.staged);
        assert!(changed.unstaged);
    }

    #[test]
    fn batch_stage_and_unstage_update_all_files() {
        let (dir, first) = repo();
        let second = dir.path().join("other.txt");
        fs::write(&second, "other\n").unwrap();
        let paths = vec![first, second];
        stage_files(dir.path(), &paths, &translator()).unwrap();
        assert!(snapshot(dir.path()).files.iter().all(|item| item.staged));
        unstage_files(dir.path(), &paths, &translator()).unwrap();
        assert!(snapshot(dir.path()).files.iter().all(|item| item.untracked));
    }

    #[test]
    fn partial_stage_and_revert_keep_unselected_changes_and_unstage_restores_new_file() {
        let (dir, file) = repo();
        fs::write(&file, "first\nstable\nlast\n").unwrap();
        stage_file(dir.path(), &file, &translator()).unwrap();
        fs::write(&file, "first changed 🌎\nstable\nlast changed\n").unwrap();
        stage_hunk(dir.path(), &file, 0, &translator()).unwrap();
        let (staged, worktree) = index_and_worktree(dir.path(), &file, &translator()).unwrap();
        assert_eq!(staged, "first changed 🌎\nstable\nlast\n");
        assert_eq!(worktree, "first changed 🌎\nstable\nlast changed\n");
        revert_hunk(dir.path(), &file, 0, &translator()).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "first changed 🌎\nstable\nlast\n"
        );
        assert!(snapshot(dir.path())
            .files
            .iter()
            .any(|item| item.staged && !item.unstaged));
        unstage_file(dir.path(), &file, &translator()).unwrap();
        assert!(snapshot(dir.path()).files.iter().any(|item| item.untracked));
    }

    #[test]
    fn commits_index_tree_and_returns_the_created_commit_id() {
        let (dir, file) = repo();
        let mut repository = gix::discover(dir.path()).unwrap();
        let mut config = repository.config_snapshot_mut();
        config
            .append_config(
                [
                    "user.name=Graphite Test",
                    "user.email=graphite@example.invalid",
                ],
                gix::config::Source::Local,
            )
            .unwrap();
        config.commit().unwrap();
        drop(repository);
        stage_file(dir.path(), &file, &translator()).unwrap();
        let id = commit(dir.path(), "initial content", &translator()).unwrap();
        assert_eq!(id.len(), 40);
        let repository = gix::discover(dir.path()).unwrap();
        assert_eq!(
            repository
                .head_commit()
                .unwrap()
                .message()
                .unwrap()
                .title
                .to_string(),
            "initial content"
        );
        assert!(head_file_text(dir.path(), &file, &translator())
            .unwrap()
            .contains("one"));
        assert!(snapshot(dir.path()).files.is_empty());
    }

    #[test]
    fn conflict_stages_are_exposed_and_resolution_replaces_them_with_stage_zero() {
        let (dir, file) = repo();
        let repository = gix::discover(dir.path()).unwrap();
        let mut index = open_or_default_index(&repository).unwrap();
        index.remove_entries(|_, path, _| path == b"note.txt".as_bstr());
        for (stage, text) in [
            (gix::index::entry::Stage::Base, "base\n"),
            (gix::index::entry::Stage::Ours, "ours\n"),
            (gix::index::entry::Stage::Theirs, "theirs\n"),
        ] {
            let id = repository.write_blob(text).unwrap().detach();
            index.dangerously_push_entry(
                Default::default(),
                id,
                stage.into(),
                gix::index::entry::Mode::FILE,
                b"note.txt".as_bstr(),
            );
        }
        index.sort_entries();
        index.write(gix::index::write::Options::default()).unwrap();
        drop(index);
        drop(repository);
        assert_eq!(
            merge_conflict(dir.path(), &file, &translator()).unwrap(),
            [
                Some("base\n".into()),
                Some("ours\n".into()),
                Some("theirs\n".into())
            ]
        );
        assert!(snapshot(dir.path())
            .files
            .iter()
            .any(|item| item.conflicted));
        resolve_conflict(dir.path(), &file, "resolved 🌎\n", &translator()).unwrap();
        assert!(merge_conflict(dir.path(), &file, &translator()).is_err());
        assert_eq!(
            index_and_worktree(dir.path(), &file, &translator()).unwrap(),
            ("resolved 🌎\n".into(), "resolved 🌎\n".into())
        );
    }

    #[test]
    fn partial_hunks_preserve_unicode_and_other_changes() {
        let base = "olá\nfixo\nfim\n";
        let changed = "olá 🌎\nfixo\nfim +\n";
        let first = apply_hunk_selection(base, changed, 0).unwrap();
        assert_eq!(first, "olá 🌎\nfixo\nfim\n");
        let second = apply_hunk_selection(base, changed, 1).unwrap();
        assert_eq!(second, "olá\nfixo\nfim +\n");
    }

    #[test]
    fn stage_and_revert_select_one_line_inside_a_multiline_change() {
        let (dir, file) = repo();
        fs::write(&file, "first\nsecond\nthird\n").unwrap();
        stage_file(dir.path(), &file, &translator()).unwrap();
        fs::write(&file, "first 🌎\nsecond 🌎\nthird\n").unwrap();
        stage_line(dir.path(), &file, 0, &translator()).unwrap();
        assert_eq!(
            index_and_worktree(dir.path(), &file, &translator()).unwrap(),
            (
                "first 🌎\nsecond\nthird\n".into(),
                "first 🌎\nsecond 🌎\nthird\n".into(),
            )
        );
        revert_line(dir.path(), &file, 1, &translator()).unwrap();
        assert_eq!(
            fs::read_to_string(file).unwrap(),
            "first 🌎\nsecond\nthird\n"
        );
    }

    #[test]
    fn diff_is_readable_and_reports_invalid_repository() {
        assert!(diff_text("a\n", "b\n").contains("-a"));
        let dir = TempDir::new();
        assert!(snapshot(dir.path()).error.is_some());
    }

    #[test]
    fn snapshot_omits_gitignored_files_and_checkout_refuses_dirty_worktrees() {
        let (dir, file) = repo();
        fs::write(dir.path().join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(dir.path().join("ignored.txt"), "secret local content\n").unwrap();
        let snapshot = snapshot(dir.path());
        assert!(!snapshot
            .files
            .iter()
            .any(|item| item.path == PathBuf::from("ignored.txt")));
        let error = checkout_branch(dir.path(), "main", &translator()).unwrap_err();
        assert!(error.contains("worktree"));
        assert_eq!(fs::read_to_string(file).unwrap(), "one\n");
    }

    #[test]
    fn line_decorations_distinguish_added_modified_and_deleted_lines() {
        assert_eq!(diff_line_changes("", "🧪\n"), vec![(0, LineChange::Added)]);
        assert_eq!(
            diff_line_changes("one\n", "ONE\n"),
            vec![(0, LineChange::Modified)]
        );
        assert_eq!(
            diff_line_changes("gone\n", ""),
            vec![(0, LineChange::Deleted)]
        );
    }

    #[test]
    fn watch_events_ignore_reads_git_internals_and_build_output() {
        use notify::{event::AccessKind, Event, EventKind};
        let modify = |path: &str| {
            Event::new(EventKind::Modify(notify::event::ModifyKind::Any)).add_path(path.into())
        };
        assert!(is_relevant_watch_event(&modify("/w/src/main.rs")));
        assert!(is_relevant_watch_event(&modify("/w/.git/HEAD")));
        assert!(is_relevant_watch_event(&modify("/w/.git/refs/heads/main")));
        assert!(!is_relevant_watch_event(&modify("/w/.git/index")));
        assert!(!is_relevant_watch_event(&modify("/w/.git/index.lock")));
        assert!(!is_relevant_watch_event(&modify(
            "/w/target/debug/graphite"
        )));
        let read = Event::new(EventKind::Access(AccessKind::Any)).add_path("/w/src/a.rs".into());
        assert!(!is_relevant_watch_event(&read));
    }
}
