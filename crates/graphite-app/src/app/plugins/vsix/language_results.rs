//! Converts language-provider results from the Code OSS extension host (1-based ranges,
//! serialized URIs) into the LSP shapes the editor already knows how to present.

use super::*;

/// LSP `Range` for a Code OSS `IRange` (1-based line and column).
pub(super) fn range(range: &Value) -> Option<Value> {
    let coordinate = |name: &str| range[name].as_u64().and_then(|value| value.checked_sub(1));
    Some(json!({
        "start": {"line": coordinate("startLineNumber")?, "character": coordinate("startColumn")?},
        "end": {"line": coordinate("endLineNumber")?, "character": coordinate("endColumn")?},
    }))
}

/// `file://` URI string for a URI given as text or as a serialized URI object.
/// Only local files can be shown, so other schemes yield `None`.
pub(super) fn uri(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return text.starts_with("file:").then(|| text.to_owned());
    }
    if value["scheme"]
        .as_str()
        .is_some_and(|scheme| scheme != "file")
    {
        return None;
    }
    crate::lsp::path_to_uri(&workbench::file_path(value)?)
}

/// LSP `Location[]` from `ILocationDto[]` or `ILocationLinkDto[]` (a single item is accepted).
pub(super) fn locations(result: &Value) -> Value {
    let items: Vec<&Value> = match result {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![result],
        _ => Vec::new(),
    };
    let locations = items
        .into_iter()
        .filter_map(|item| {
            let target = uri(&item["uri"])?;
            let range = range(
                item.get("targetSelectionRange")
                    .filter(|range| range.is_object())
                    .unwrap_or(&item["range"]),
            )?;
            Some(json!({"uri": target, "range": range}))
        })
        .collect::<Vec<_>>();
    json!(locations)
}

/// LSP `SignatureHelp` from `ISignatureHelpDto`.
pub(super) fn signature_help(result: &Value) -> Value {
    if !result["signatures"].is_array() {
        return Value::Null;
    }
    let text = |value: &Value| {
        value
            .as_str()
            .or_else(|| value["value"].as_str())
            .map(str::to_owned)
    };
    let signatures = result["signatures"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|signature| {
            let label = signature["label"].as_str()?;
            let parameters = signature["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|parameter| {
                    let label = match &parameter["label"] {
                        Value::String(label) => label.clone(),
                        // `[start, end]` offsets into the signature label.
                        Value::Array(offsets) => {
                            let start = offsets.first()?.as_u64()? as usize;
                            let end = offsets.get(1)?.as_u64()? as usize;
                            label
                                .chars()
                                .skip(start)
                                .take(end.checked_sub(start)?)
                                .collect()
                        }
                        _ => return None,
                    };
                    Some(
                        json!({"label": label, "documentation": text(&parameter["documentation"])}),
                    )
                })
                .collect::<Vec<_>>();
            Some(json!({
                "label": label,
                "documentation": text(&signature["documentation"]),
                "parameters": parameters,
            }))
        })
        .collect::<Vec<_>>();
    json!({
        "signatures": signatures,
        "activeSignature": result["activeSignature"].as_u64().unwrap_or(0),
        "activeParameter": result["activeParameter"].as_u64().unwrap_or(0),
    })
}

/// Flattened `(name, scope, line)` rows for a tree of `DocumentSymbol`s.
pub(super) fn symbols(result: &Value) -> Vec<(String, String, usize)> {
    fn walk(items: &Value, scope: &str, rows: &mut Vec<(String, String, usize)>) {
        for item in items.as_array().into_iter().flatten() {
            let Some(name) = item["name"].as_str() else {
                continue;
            };
            let located = item
                .get("selectionRange")
                .filter(|range| range.is_object())
                .unwrap_or(&item["range"]);
            // `SymbolInformation` carries a location instead of a range.
            let located = if located["startLineNumber"].is_null() {
                &item["location"]["range"]
            } else {
                located
            };
            if let Some(line) = located["startLineNumber"].as_u64() {
                rows.push((
                    name.to_owned(),
                    scope.to_owned(),
                    line.saturating_sub(1) as usize,
                ));
            }
            let child_scope = if scope.is_empty() {
                name.to_owned()
            } else {
                format!("{scope} › {name}")
            };
            walk(&item["children"], &child_scope, rows);
        }
    }
    let mut rows = Vec::new();
    walk(result, "", &mut rows);
    rows
}

/// Per-file LSP text edits from an `IWorkspaceEditDto`, with the version each edit expects.
/// The DTO is accepted bare or wrapped as `{ "value": ... }`.
pub(super) fn workspace_edit_changes(dto: &Value) -> Option<Value> {
    let edits = dto
        .pointer("/value/edits")
        .or_else(|| dto.get("edits"))?
        .as_array()?;
    let mut changes = serde_json::Map::new();
    for edit in edits {
        // File operations and notebook edits have no text edit; they are not supported.
        let Some(text_edit) = edit.get("textEdit").filter(|edit| edit.is_object()) else {
            return None;
        };
        let target = uri(&edit["resource"])?;
        let entry =
            json!({"range": range(&text_edit["range"])?, "newText": text_edit["text"].as_str()?});
        changes
            .entry(target)
            .or_insert_with(|| json!([]))
            .as_array_mut()?
            .push(entry);
    }
    Some(json!({"changes": changes}))
}

/// LSP code actions (title, kind, edit) from `ICodeActionListDto`. Actions that only run an
/// extension command, and disabled actions, are left out because they cannot be applied here.
pub(super) fn code_actions(result: &Value) -> Vec<Value> {
    result["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|action| action["disabled"].is_null())
        .filter_map(|action| {
            let edit = workspace_edit_changes(action.get("edit")?)?;
            Some(json!({
                "title": action["title"].as_str()?,
                "kind": action["kind"].as_str(),
                "isPreferred": action["isPreferred"].as_bool().unwrap_or(false),
                "edit": edit,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> Value {
        json!({"$mid": 1, "scheme": "file", "authority": "", "path": path})
    }

    fn span(line: u64, from: u64, to: u64) -> Value {
        json!({"startLineNumber": line, "startColumn": from, "endLineNumber": line, "endColumn": to})
    }

    #[test]
    fn ranges_become_zero_based_and_reject_missing_coordinates() {
        assert_eq!(
            range(&span(3, 2, 5)).unwrap(),
            json!({"start":{"line":2,"character":1},"end":{"line":2,"character":4}})
        );
        assert!(range(&json!({"startLineNumber": 0, "startColumn": 1})).is_none());
        assert!(range(&json!({})).is_none());
    }

    #[test]
    fn locations_accept_plain_locations_links_and_skip_remote_schemes() {
        let result = json!([
            {"uri": file("/work/a.rs"), "range": span(1, 1, 4)},
            {"uri": file("/work/b.rs"), "range": span(9, 1, 20), "targetSelectionRange": span(10, 5, 8)},
            {"uri": {"scheme": "https", "path": "/x"}, "range": span(1, 1, 2)},
            {"uri": file("/work/c.rs")},
        ]);
        let converted = locations(&result);
        let items = converted.as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert!(items[0]["uri"].as_str().unwrap().ends_with("/work/a.rs"));
        assert_eq!(
            items[1]["range"]["start"],
            json!({"line": 9, "character": 4})
        );
        assert_eq!(locations(&json!(null)), json!([]));
        assert_eq!(
            locations(&json!({"uri": file("/work/a.rs"), "range": span(2, 1, 2)}))
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn signature_help_resolves_parameter_offsets_and_markdown() {
        let result = json!({
            "signatures": [{
                "label": "add(a: i32, b: i32)",
                "documentation": {"value": "Adds two numbers"},
                "parameters": [{"label": [4, 10]}, {"label": "b: i32", "documentation": "second"}],
            }],
            "activeSignature": 0,
            "activeParameter": 1,
        });
        let help = signature_help(&result);
        assert_eq!(help["signatures"][0]["parameters"][0]["label"], "a: i32");
        assert_eq!(help["signatures"][0]["documentation"], "Adds two numbers");
        assert_eq!(
            help["signatures"][0]["parameters"][1]["documentation"],
            "second"
        );
        assert_eq!(help["activeParameter"], 1);
        assert!(signature_help(&json!(null)).is_null());
    }

    #[test]
    fn symbols_flatten_children_with_their_scope() {
        let result = json!([{
            "name": "Outer", "range": span(1, 1, 9), "selectionRange": span(1, 8, 13),
            "children": [{"name": "inner", "range": span(3, 1, 4), "selectionRange": span(3, 5, 10)}],
        }, {"name": "legacy", "location": {"range": span(20, 1, 3)}}]);
        assert_eq!(
            symbols(&result),
            vec![
                ("Outer".into(), "".into(), 0),
                ("inner".into(), "Outer".into(), 2),
                ("legacy".into(), "".into(), 19),
            ]
        );
    }

    #[test]
    fn workspace_edits_group_text_edits_by_file_and_reject_file_operations() {
        let dto = json!({"edits": [
            {"resource": file("/work/a.rs"), "textEdit": {"range": span(1, 1, 3), "text": "x"}},
            {"resource": file("/work/a.rs"), "textEdit": {"range": span(2, 1, 3), "text": "y"}},
            {"resource": file("/work/b.rs"), "textEdit": {"range": span(1, 1, 3), "text": "z"}},
        ]});
        let changes = workspace_edit_changes(&dto).unwrap();
        let map = changes["changes"].as_object().unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.values().next().unwrap().as_array().unwrap().len(), 2);
        assert_eq!(
            workspace_edit_changes(&json!({"value": dto})).unwrap(),
            changes
        );
        let operation = json!({"edits": [{"oldResource": file("/a"), "newResource": file("/b")}]});
        assert!(workspace_edit_changes(&operation).is_none());
        assert!(workspace_edit_changes(&json!({})).is_none());
    }

    #[test]
    fn code_actions_keep_only_enabled_actions_with_text_edits() {
        let edit = json!({"edits": [
            {"resource": file("/work/a.rs"), "textEdit": {"range": span(1, 1, 3), "text": "x"}},
        ]});
        let result = json!({"cacheId": 1, "actions": [
            {"title": "Fix it", "kind": "quickfix", "isPreferred": true, "edit": edit},
            {"title": "Run a command", "command": {"id": "ext.run"}},
            {"title": "Disabled", "edit": edit, "disabled": "Not here"},
        ]});
        let actions = code_actions(&result);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0]["title"], "Fix it");
        assert_eq!(actions[0]["isPreferred"], true);
        assert!(code_actions(&json!(null)).is_empty());
    }
}
