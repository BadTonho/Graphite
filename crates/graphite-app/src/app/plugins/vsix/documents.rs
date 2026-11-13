use super::*;
use graphite_core_api::{DocumentEdit, DocumentUri, Position, TextEdit};

impl App {
    pub(super) fn sync_vsix_documents(&self, runtime: &mut Runtime) -> Result<()> {
        let documents = self.documents().map_err(|error| error.to_string())?;
        let active = documents
            .iter()
            .find(|document| document.id == Some(self.active_document()));
        let active = active.map(|document| {
            let selections = self.editor().selections().iter().map(|selection| {
                let anchor = position(&document.text, selection.anchor);
                let head = position(&document.text, selection.head);
                json!({
                    "selectionStartLineNumber": anchor.line + 1, "selectionStartColumn": anchor.character + 1,
                    "positionLineNumber": head.line + 1, "positionColumn": head.character + 1,
                })
            }).collect::<Vec<_>>();
            json!({"uri":document.uri, "selections": selections})
        });
        let snapshot = json!({"type":"sync", "documents":documents, "active":active,
            "tabSize":self.tab_size, "insertSpaces":self.insert_spaces});
        if runtime.synced.as_ref() != Some(&snapshot) {
            runtime.session.send(snapshot.clone())?;
            runtime.synced = Some(snapshot);
        }
        Ok(())
    }

    pub(super) fn apply_vsix_editor_edits(
        &mut self,
        id: &str,
        version: u64,
        edits: &Value,
    ) -> Result<Value> {
        let uri = DocumentUri::parse(id).map_err(|error| error.to_string())?;
        let edits = edits
            .as_array()
            .ok_or("Invalid Code OSS editor edits")?
            .iter()
            .map(text_edit)
            .collect::<Result<_>>()?;
        Ok(json!(self
            .apply_edits(vec![DocumentEdit {
                uri,
                version: Some(version),
                edits
            }])
            .is_ok()))
    }

    pub(super) fn apply_vsix_workspace_edits(&mut self, dto: &Value) -> Result<Value> {
        let mut edits: Vec<DocumentEdit> = Vec::new();
        for edit in dto
            .pointer("/value/edits")
            .and_then(Value::as_array)
            .ok_or("Invalid Code OSS workspace edit")?
        {
            let uri: DocumentUri =
                serde_json::from_value(edit["resource"].clone()).map_err(|e| e.to_string())?;
            let version = edit["versionId"].as_u64();
            let text_edit = text_edit(&edit["textEdit"])?;
            if let Some(existing) = edits.iter_mut().find(|entry| entry.uri == uri) {
                if existing.version != version {
                    return Err("Conflicting Code OSS document versions".into());
                }
                existing.edits.push(text_edit);
            } else {
                edits.push(DocumentEdit {
                    uri,
                    version,
                    edits: vec![text_edit],
                });
            }
        }
        Ok(json!(self.apply_edits(edits).is_ok()))
    }
}

pub(super) fn position(text: &str, char_offset: usize) -> Position {
    let mut position = Position {
        line: 0,
        character: 0,
    };
    for ch in text.chars().take(char_offset) {
        if ch == '\n' {
            position.line += 1;
            position.character = 0;
        } else {
            position.character += ch.len_utf16() as u32;
        }
    }
    position
}

/// The char offset of a 0-based line and UTF-16 column, clamped to the end of that line (or of
/// the text, for a line past the end). The inverse of [`position`].
pub(super) fn char_offset(text: &str, line: u32, utf16_column: u32) -> usize {
    let mut offset = 0;
    let mut current = 0;
    let mut column = 0;
    for ch in text.chars() {
        if current == line {
            if ch == '\n' || column >= utf16_column {
                return offset;
            }
            column += ch.len_utf16() as u32;
        }
        if ch == '\n' {
            current += 1;
        }
        offset += 1;
    }
    offset
}

pub(super) fn text_edit(edit: &Value) -> Result<TextEdit> {
    let range = &edit["range"];
    let coordinate = |name| {
        range[name]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .and_then(|value| value.checked_sub(1))
            .ok_or("Invalid Code OSS edit position")
    };
    Ok(TextEdit {
        start: Position {
            line: coordinate("startLineNumber")?,
            character: coordinate("startColumn")?,
        },
        end: Position {
            line: coordinate("endLineNumber")?,
            character: coordinate("endColumn")?,
        },
        text: edit["text"]
            .as_str()
            .ok_or("Invalid Code OSS edit text")?
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_use_utf16_and_one_based_wire_coordinates() {
        assert_eq!(
            position("x🌎\nã", 2),
            Position {
                line: 0,
                character: 3
            }
        );
        assert_eq!(
            position("x🌎\nã", 4),
            Position {
                line: 1,
                character: 1
            }
        );
        let edit = json!({"range":{"startLineNumber":1,"startColumn":2,"endLineNumber":1,"endColumn":4},"text":"a"});
        let parsed = text_edit(&edit).unwrap();
        assert_eq!(parsed.start.character, 1);
        assert_eq!(parsed.end.character, 3);
        assert!(text_edit(&json!({"range":{"startLineNumber":0}})).is_err());
    }
}
