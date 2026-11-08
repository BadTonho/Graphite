use super::*;

#[derive(Default)]
pub(super) struct SaveCapabilities {
    pub will_save: bool,
    pub wait_until: bool,
    pub save: bool,
    pub include_text: bool,
}
impl SaveCapabilities {
    pub fn from_sync(sync: Option<&Value>) -> Self {
        Self {
            will_save: sync
                .and_then(|v| v.get("willSave"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            wait_until: sync
                .and_then(|v| v.get("willSaveWaitUntil"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            save: sync
                .and_then(|v| v.get("save"))
                .is_some_and(|v| v == &Value::Bool(true) || v.is_object()),
            include_text: sync
                .and_then(|v| v.pointer("/save/includeText"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
    pub fn did_save(&self, uri: &str, text: String) -> Option<Message> {
        if !self.save {
            return None;
        }
        let mut params = json!({"textDocument":{"uri":uri}});
        if self.include_text {
            params["text"] = json!(text);
        }
        Some(Message::Notification(Notification {
            method: "textDocument/didSave".into(),
            params,
        }))
    }
}
pub(super) fn save_edits(
    value: Value,
) -> shoppe_core_api::ServiceResult<Vec<shoppe_core_api::TextEdit>> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let edits: Vec<lsp_types::TextEdit> = serde_json::from_value(value)
        .map_err(|e| shoppe_core_api::ServiceError::Protocol(e.to_string()))?;
    Ok(edits
        .into_iter()
        .map(|e| shoppe_core_api::TextEdit {
            start: shoppe_core_api::Position {
                line: e.range.start.line,
                character: e.range.start.character,
            },
            end: shoppe_core_api::Position {
                line: e.range.end.line,
                character: e.range.end.character,
            },
            text: e.new_text,
        })
        .collect())
}

#[cfg(test)]
mod save_tests {
    use super::*;
    #[test]
    fn save_notifications_respect_negotiated_capabilities_and_text_inclusion() {
        assert!(SaveCapabilities::from_sync(Some(&json!(2)))
            .did_save("file:///a", "x".into())
            .is_none());
        let caps = SaveCapabilities::from_sync(Some(
            &json!({"willSave":true,"willSaveWaitUntil":true,"save":true}),
        ));
        assert!(caps.will_save && caps.wait_until);
        let Message::Notification(message) = caps.did_save("file:///a", "x".into()).unwrap() else {
            panic!()
        };
        assert!(message.params.get("text").is_none());
        let caps = SaveCapabilities::from_sync(Some(&json!({"save":{"includeText":true}})));
        let Message::Notification(message) = caps.did_save("file:///a", "written".into()).unwrap()
        else {
            panic!()
        };
        assert_eq!(message.params["text"], "written");
        assert!(save_edits(json!([{"newText":"invalid"}])).is_err());
        assert!(save_edits(Value::Null).unwrap().is_empty());
    }
}

pub(super) fn full_text_change(text: String) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text,
    }
}

pub(super) fn text_sync_kind(capability: Option<&Value>) -> u64 {
    match capability {
        Some(value) if value.is_number() => value.as_u64().unwrap_or(0),
        Some(value) => value.get("change").and_then(Value::as_u64).unwrap_or(0),
        None => 0,
    }
}

pub(super) fn change_params(
    uri: &str,
    previous: &str,
    text: &str,
    version: i32,
    sync_kind: u64,
) -> Option<DidChangeTextDocumentParams> {
    let change = match sync_kind {
        1 => full_text_change(text.to_owned()),
        2 => incremental_text_change(previous, text),
        _ => return None,
    };
    Some(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.parse().ok()?,
            version,
        },
        content_changes: vec![change],
    })
}

pub(super) fn incremental_text_change(old: &str, new: &str) -> TextDocumentContentChangeEvent {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let mut prefix = 0;
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_chars.len().saturating_sub(prefix)
        && suffix < new_chars.len().saturating_sub(prefix)
        && old_chars[old_chars.len() - 1 - suffix] == new_chars[new_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let old_end = old_chars.len() - suffix;
    let new_end = new_chars.len() - suffix;
    let start = position_at_char(old, prefix);
    let end = position_at_char(old, old_end);
    let replacement: String = new_chars[prefix..new_end].iter().collect();
    let range_length = utf16_len(&old_chars[prefix..old_end]) as u32;
    TextDocumentContentChangeEvent {
        range: Some(Range::new(start, end)),
        range_length: Some(range_length),
        text: replacement,
    }
}

pub fn position_at_char(text: &str, char_index: usize) -> Position {
    let prefix: String = text.chars().take(char_index).collect();
    let line = prefix.bytes().filter(|b| *b == b'\n').count();
    let column = prefix.rsplit('\n').next().unwrap_or_default();
    Position::new(
        line as u32,
        utf16_len(&column.chars().collect::<Vec<_>>()) as u32,
    )
}

pub(super) fn utf16_len(chars: &[char]) -> usize {
    chars.iter().map(|c| c.len_utf16()).sum()
}

pub fn path_to_uri(path: &Path) -> Option<String> {
    shoppe_core_api::DocumentUri::from_path(path)
        .ok()
        .map(|uri| uri.as_str().to_owned())
}
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    shoppe_core_api::DocumentUri::parse(uri)
        .ok()?
        .to_path()
        .ok()
}

pub fn diagnostics_are_current(
    current_revision: Option<u64>,
    diagnostic_version: Option<i32>,
) -> bool {
    diagnostic_version.map_or(true, |version| {
        current_revision == Some(version.max(0) as u64)
    })
}

pub(super) fn hover_to_string(hover: Hover) -> String {
    match hover.contents {
        lsp_types::HoverContents::Scalar(markup) => match markup {
            lsp_types::MarkedString::String(s) => s,
            lsp_types::MarkedString::LanguageString(s) => s.value,
        },
        lsp_types::HoverContents::Array(items) => items
            .into_iter()
            .map(|item| match item {
                lsp_types::MarkedString::String(s) => s,
                lsp_types::MarkedString::LanguageString(s) => s.value,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value,
    }
}
