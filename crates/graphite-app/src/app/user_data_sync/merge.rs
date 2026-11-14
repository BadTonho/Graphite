//! Three-way key-level JSON merge and conflict resolution for User Data Sync.

use super::state::{ConflictChoice, SyncConflict, SyncSnapshot};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

fn parse_json_object(raw: Option<&str>) -> Option<Map<String, Value>> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return Some(Map::new());
    }
    match serde_json::from_str::<Value>(raw).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

fn value_display(v: Option<&Value>) -> Option<String> {
    v.and_then(|val| serde_json::to_string(val).ok())
}

/// Performs a 3-way merge of two JSON objects against `base`, emitting conflicts when both sides
/// changed the same key to different values.
pub(crate) fn merge_json_objects(
    resource: &str,
    base_raw: Option<&str>,
    local_raw: Option<&str>,
    remote_raw: Option<&str>,
    conflicts: &mut Vec<SyncConflict>,
) -> Option<String> {
    if local_raw == remote_raw {
        return local_raw.map(str::to_owned);
    }
    let Some(local_map) = parse_json_object(local_raw) else {
        return merge_scalar_resource(
            resource, "content", base_raw, local_raw, remote_raw, conflicts,
        );
    };
    let Some(remote_map) = parse_json_object(remote_raw) else {
        return merge_scalar_resource(
            resource, "content", base_raw, local_raw, remote_raw, conflicts,
        );
    };
    let base_map = parse_json_object(base_raw).unwrap_or_default();

    let mut keys = BTreeSet::new();
    keys.extend(base_map.keys().cloned());
    keys.extend(local_map.keys().cloned());
    keys.extend(remote_map.keys().cloned());

    let mut merged = Map::new();
    for key in keys {
        let b = base_map.get(&key);
        let l = local_map.get(&key);
        let r = remote_map.get(&key);

        if l == r {
            if let Some(val) = l {
                merged.insert(key, val.clone());
            }
        } else if l == b {
            if let Some(val) = r {
                merged.insert(key, val.clone());
            }
        } else if r == b {
            if let Some(val) = l {
                merged.insert(key, val.clone());
            }
        } else {
            // Both sides modified or added the key differently; preserve local and record conflict.
            if let Some(val) = l {
                merged.insert(key.clone(), val.clone());
            }
            conflicts.push(SyncConflict {
                resource: resource.to_owned(),
                key,
                local_value: value_display(l),
                remote_value: value_display(r),
                resolved: None,
            });
        }
    }

    serde_json::to_string_pretty(&Value::Object(merged)).ok()
}

fn merge_scalar_resource(
    resource: &str,
    key: &str,
    base: Option<&str>,
    local: Option<&str>,
    remote: Option<&str>,
    conflicts: &mut Vec<SyncConflict>,
) -> Option<String> {
    if local == remote {
        return local.map(str::to_owned);
    }
    if local == base {
        return remote.map(str::to_owned);
    }
    if remote == base {
        return local.map(str::to_owned);
    }
    conflicts.push(SyncConflict {
        resource: resource.to_owned(),
        key: key.to_owned(),
        local_value: local.map(str::to_owned),
        remote_value: remote.map(str::to_owned),
        resolved: None,
    });
    local.map(str::to_owned)
}

fn merge_string_maps(
    resource: &str,
    base: &BTreeMap<String, String>,
    local: &BTreeMap<String, String>,
    remote: &BTreeMap<String, String>,
    conflicts: &mut Vec<SyncConflict>,
) -> BTreeMap<String, String> {
    let mut keys = BTreeSet::new();
    keys.extend(base.keys().cloned());
    keys.extend(local.keys().cloned());
    keys.extend(remote.keys().cloned());

    let mut out = BTreeMap::new();
    for key in keys {
        let merged = merge_scalar_resource(
            resource,
            &key,
            base.get(&key).map(String::as_str),
            local.get(&key).map(String::as_str),
            remote.get(&key).map(String::as_str),
            conflicts,
        );
        if let Some(val) = merged {
            out.insert(key, val);
        }
    }
    out
}

fn merge_plugin_lists(base: &[String], local: &[String], remote: &[String]) -> Vec<String> {
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let local_set: BTreeSet<&str> = local.iter().map(String::as_str).collect();
    let remote_set: BTreeSet<&str> = remote.iter().map(String::as_str).collect();

    let mut all: BTreeSet<&str> = BTreeSet::new();
    all.extend(local_set.iter().copied());
    all.extend(remote_set.iter().copied());

    let mut out = Vec::new();
    for plugin in all {
        let in_base = base_set.contains(plugin);
        let in_local = local_set.contains(plugin);
        let in_remote = remote_set.contains(plugin);
        let keep = match (in_base, in_local, in_remote) {
            (true, false, true) | (true, true, false) => false, // removed intentionally on one side
            _ => in_local || in_remote,
        };
        if keep {
            out.push(plugin.to_owned());
        }
    }
    out
}

/// Three-way merge across all User Data Sync categories.
pub(crate) fn merge_snapshots(
    base: Option<&SyncSnapshot>,
    local: &SyncSnapshot,
    remote: &SyncSnapshot,
) -> (SyncSnapshot, Vec<SyncConflict>) {
    let empty_base = SyncSnapshot::default();
    let base = base.unwrap_or(&empty_base);
    let mut conflicts = Vec::new();

    let settings_json = merge_json_objects(
        "settings.json",
        base.settings_json.as_deref(),
        local.settings_json.as_deref(),
        remote.settings_json.as_deref(),
        &mut conflicts,
    );
    let keybindings_json = merge_scalar_resource(
        "keybindings.json",
        "entries",
        base.keybindings_json.as_deref(),
        local.keybindings_json.as_deref(),
        remote.keybindings_json.as_deref(),
        &mut conflicts,
    );
    let snippets = merge_string_maps(
        "snippets",
        &base.snippets,
        &local.snippets,
        &remote.snippets,
        &mut conflicts,
    );
    let plugins = merge_plugin_lists(&base.plugins, &local.plugins, &remote.plugins);
    let profiles_json = merge_scalar_resource(
        "profiles.json",
        "profiles",
        base.profiles_json.as_deref(),
        local.profiles_json.as_deref(),
        remote.profiles_json.as_deref(),
        &mut conflicts,
    );

    let merged = SyncSnapshot {
        settings_json,
        keybindings_json,
        snippets,
        plugins,
        profiles_json,
        updated_at_epoch_s: super::state::now_epoch_s(),
    };
    (merged, conflicts)
}

/// Applies explicit user choices to resolve pending conflicts onto `snapshot`.
pub(crate) fn apply_conflict_resolutions(
    snapshot: &mut SyncSnapshot,
    conflicts: &mut [SyncConflict],
    default_choice: ConflictChoice,
) {
    let mut settings_map = parse_json_object(snapshot.settings_json.as_deref()).unwrap_or_default();
    let mut settings_touched = false;

    for conflict in conflicts.iter_mut() {
        let choice = conflict.resolved.unwrap_or(default_choice);
        conflict.resolved = Some(choice);
        let chosen = match choice {
            ConflictChoice::AcceptLocal | ConflictChoice::KeepMerged => &conflict.local_value,
            ConflictChoice::AcceptRemote => &conflict.remote_value,
        };
        match conflict.resource.as_str() {
            "settings.json" => {
                settings_touched = true;
                match chosen {
                    Some(raw_val) => {
                        let parsed = serde_json::from_str::<Value>(raw_val)
                            .unwrap_or_else(|_| Value::String(raw_val.clone()));
                        settings_map.insert(conflict.key.clone(), parsed);
                    }
                    None => {
                        settings_map.remove(&conflict.key);
                    }
                }
            }
            "keybindings.json" => {
                snapshot.keybindings_json = chosen.clone();
            }
            "snippets" => match chosen {
                Some(body) => {
                    snapshot.snippets.insert(conflict.key.clone(), body.clone());
                }
                None => {
                    snapshot.snippets.remove(&conflict.key);
                }
            },
            "profiles.json" => {
                snapshot.profiles_json = chosen.clone();
            }
            _ => {}
        }
    }

    if settings_touched {
        if let Ok(json) = serde_json::to_string_pretty(&Value::Object(settings_map)) {
            snapshot.settings_json = Some(json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_way_settings_merge_combines_disjoint_keys_and_flags_conflicts() {
        let base = SyncSnapshot {
            settings_json: Some(r#"{"editor.fontSize": 14, "editor.tabSize": 4}"#.into()),
            ..Default::default()
        };
        let local = SyncSnapshot {
            settings_json: Some(
                r#"{"editor.fontSize": 16, "editor.tabSize": 4, "files.autoSave": "afterDelay"}"#
                    .into(),
            ),
            ..Default::default()
        };
        let remote = SyncSnapshot {
            settings_json: Some(
                r#"{"editor.fontSize": 12, "editor.tabSize": 2, "window.zoomLevel": 1}"#.into(),
            ),
            ..Default::default()
        };

        let (mut merged, mut conflicts) = merge_snapshots(Some(&base), &local, &remote);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "editor.fontSize");

        let obj = parse_json_object(merged.settings_json.as_deref()).unwrap();
        assert_eq!(obj.get("editor.tabSize").and_then(Value::as_i64), Some(2));
        assert_eq!(
            obj.get("files.autoSave").and_then(Value::as_str),
            Some("afterDelay")
        );
        assert_eq!(obj.get("window.zoomLevel").and_then(Value::as_i64), Some(1));
        assert_eq!(obj.get("editor.fontSize").and_then(Value::as_i64), Some(16));

        apply_conflict_resolutions(&mut merged, &mut conflicts, ConflictChoice::AcceptRemote);
        let obj2 = parse_json_object(merged.settings_json.as_deref()).unwrap();
        assert_eq!(
            obj2.get("editor.fontSize").and_then(Value::as_i64),
            Some(12)
        );
    }

    #[test]
    fn plugin_list_merge_respects_additions_and_deletions() {
        let base = vec!["a".to_owned(), "b".to_owned()];
        let local = vec!["a".to_owned(), "c".to_owned()]; // removed b, added c
        let remote = vec!["a".to_owned(), "b".to_owned(), "d".to_owned()]; // added d
        let merged = merge_plugin_lists(&base, &local, &remote);
        assert_eq!(merged, vec!["a", "c", "d"]);
    }
}
