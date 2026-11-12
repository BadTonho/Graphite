//! File watchers for extensions (`workspace.createFileSystemWatcher`). The host treats the
//! workspace as watched already and calls `$watch` only for folders outside it, then matches
//! each watcher's glob itself. So Graphite forwards every workspace change (except inside
//! `node_modules`) plus the changes in folders registered with `$watch`, as one `$onFileEvent`
//! without a session, which the host delivers to all watchers.

use super::*;

/// What happened to a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileChange {
    Created,
    Changed,
    Deleted,
}

struct Watch {
    folder: PathBuf,
    recursive: bool,
}

#[derive(Default)]
pub(super) struct Watches {
    /// Active watchers by the host's session number (a fractional number, kept as text).
    sessions: BTreeMap<String, Watch>,
}

/// Maps a file-system notification to the changes extensions understand. Reads and metadata
/// access are not changes, and a rename is a deletion of the old path plus a creation of the
/// new one.
pub(crate) fn changes(event: &notify::Event) -> Vec<(FileChange, PathBuf)> {
    use notify::event::{ModifyKind, RenameMode};
    use notify::EventKind;
    let kind = match &event.kind {
        EventKind::Create(_) => FileChange::Created,
        EventKind::Remove(_) => FileChange::Deleted,
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => FileChange::Deleted,
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => FileChange::Created,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            return match event.paths.as_slice() {
                [from, to] => vec![
                    (FileChange::Deleted, from.clone()),
                    (FileChange::Created, to.clone()),
                ],
                _ => Vec::new(),
            };
        }
        EventKind::Modify(ModifyKind::Metadata(_)) => return Vec::new(),
        EventKind::Modify(_) | EventKind::Any => FileChange::Changed,
        _ => return Vec::new(),
    };
    event
        .paths
        .iter()
        .map(|path| (kind, path.clone()))
        .collect()
}

impl Watches {
    fn covers(&self, session: &Watch, path: &Path) -> bool {
        if session.recursive {
            path.starts_with(&session.folder)
        } else {
            path.parent() == Some(session.folder.as_path())
        }
    }

    fn visible(&self, workspace: &Path, path: &Path) -> bool {
        let in_workspace = path.starts_with(workspace)
            && !path
                .components()
                .any(|part| part.as_os_str() == "node_modules");
        in_workspace || self.sessions.values().any(|watch| self.covers(watch, path))
    }

    /// A single `FileSystemEvents` value with every change a watcher can see, or `None`.
    pub(super) fn events_for(
        &self,
        workspace: &Path,
        changes: &[(FileChange, PathBuf)],
    ) -> Option<Value> {
        let mut buckets: [Vec<Value>; 3] = Default::default();
        for (change, path) in changes {
            if !self.visible(workspace, path) {
                continue;
            }
            let slot = match change {
                FileChange::Created => 0,
                FileChange::Changed => 1,
                FileChange::Deleted => 2,
            };
            buckets[slot].push(workbench::file_uri(path));
        }
        let [created, changed, deleted] = buckets;
        (!(created.is_empty() && changed.is_empty() && deleted.is_empty()))
            .then(|| json!({"created": created, "changed": changed, "deleted": deleted}))
    }
}

/// Answers `MainThreadFileSystemEventService` requests, or returns `None` for other services.
pub(super) fn dispatch(
    runtime: &mut Runtime,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    if actor != "MainThreadFileSystemEventService" {
        return None;
    }
    Some(match method {
        "$watch" => {
            let session = args.get(1).filter(|session| session.is_number());
            let folder = args.get(2).and_then(workbench::file_path);
            match (session, folder) {
                (Some(session), Some(folder)) => {
                    let recursive = args
                        .get(3)
                        .and_then(|options| options["recursive"].as_bool())
                        .unwrap_or(false);
                    runtime
                        .watches
                        .sessions
                        .insert(session.to_string(), Watch { folder, recursive });
                    Ok(Value::Null)
                }
                _ => Err("Invalid file watcher".into()),
            }
        }
        "$unwatch" => {
            if let Some(session) = args.first() {
                runtime.watches.sessions.remove(&session.to_string());
            }
            Ok(Value::Null)
        }
        _ => Ok(Value::Null),
    })
}

impl App {
    /// Forwards workspace file changes to the extensions that watch them.
    pub(crate) fn forward_vsix_file_events(&mut self, changes: &[(FileChange, PathBuf)]) {
        let Some(runtime) = self.plugins.vsix.as_ref() else {
            return;
        };
        if !runtime.ready {
            return;
        }
        if let Some(events) = runtime.watches.events_for(self.tree.root(), changes) {
            let _ = runtime.session.send(json!({
                "type": "call", "actor": "ExtHostFileSystemEventService",
                "method": "$onFileEvent", "args": [events],
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::{event::*, Event, EventKind};

    fn watches(folder: &str, recursive: bool) -> Watches {
        let mut watches = Watches::default();
        watches.sessions.insert(
            "0.42".into(),
            Watch {
                folder: folder.into(),
                recursive,
            },
        );
        watches
    }

    #[test]
    fn notifications_become_created_changed_and_deleted_changes() {
        let created = Event::new(EventKind::Create(CreateKind::File)).add_path("/w/a".into());
        assert_eq!(changes(&created), [(FileChange::Created, "/w/a".into())]);
        let data = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path("/w/a".into());
        assert_eq!(changes(&data), [(FileChange::Changed, "/w/a".into())]);
        let removed = Event::new(EventKind::Remove(RemoveKind::File)).add_path("/w/a".into());
        assert_eq!(changes(&removed), [(FileChange::Deleted, "/w/a".into())]);
        let renamed = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path("/w/old".into())
            .add_path("/w/new".into());
        assert_eq!(
            changes(&renamed),
            [
                (FileChange::Deleted, "/w/old".into()),
                (FileChange::Created, "/w/new".into())
            ]
        );
        let read = Event::new(EventKind::Access(AccessKind::Read)).add_path("/w/a".into());
        assert!(changes(&read).is_empty());
        let metadata = Event::new(EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)))
            .add_path("/w/a".into());
        assert!(changes(&metadata).is_empty());
    }

    #[test]
    fn workspace_changes_are_forwarded_with_extra_watched_folders_but_not_node_modules() {
        let batch = vec![
            (FileChange::Created, PathBuf::from("/w/sub/a.txt")),
            (FileChange::Deleted, PathBuf::from("/other/b.txt")),
            (
                FileChange::Changed,
                PathBuf::from("/w/node_modules/dep/c.js"),
            ),
            (FileChange::Changed, PathBuf::from("/extra/d.txt")),
        ];
        // Without watchers only the workspace is visible.
        let events = Watches::default()
            .events_for(Path::new("/w"), &batch)
            .unwrap();
        assert!(
            events.get("session").is_none(),
            "events must not carry a session"
        );
        assert_eq!(events["created"].as_array().unwrap().len(), 1);
        assert!(events["changed"].as_array().unwrap().is_empty());
        assert!(events["deleted"].as_array().unwrap().is_empty());

        // A folder registered with `$watch` adds its own changes.
        let extra = watches("/extra", true)
            .events_for(Path::new("/w"), &batch)
            .unwrap();
        assert_eq!(extra["changed"].as_array().unwrap().len(), 1);
        // A non-recursive watcher does not cover nested files.
        let shallow = watches("/extra/deeper", false)
            .events_for(Path::new("/w"), &batch)
            .unwrap();
        assert!(shallow["changed"].as_array().unwrap().is_empty());

        let nothing = [(FileChange::Created, PathBuf::from("/other/x"))];
        assert!(Watches::default()
            .events_for(Path::new("/w"), &nothing)
            .is_none());
    }
}
