//! Read-only custom views: a hex dump for files that are not text, and a readable rendering of
//! Jupyter notebooks (`.ipynb`). The view is a read-only document, so it can never be saved over
//! the original file.

use super::*;
use serde_json::Value;

/// Bytes per line of the hex view.
const HEX_WIDTH: usize = 16;

/// Largest file shown in the hex view; longer files are cut and say so.
pub(crate) const HEX_VIEW_LIMIT: usize = 1 << 20;

/// Hex view text for a file's bytes, truncated to [`HEX_VIEW_LIMIT`].
pub(crate) fn hex_view(bytes: &[u8]) -> String {
    if bytes.len() <= HEX_VIEW_LIMIT {
        return hex_dump(bytes);
    }
    let mut text = hex_dump(&bytes[..HEX_VIEW_LIMIT]);
    text.push_str(&format!(
        "... {} more bytes not shown\n",
        bytes.len() - HEX_VIEW_LIMIT
    ));
    text
}

/// Offset, hex bytes and ASCII columns for each line of `bytes`.
pub(crate) fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (line, chunk) in bytes.chunks(HEX_WIDTH).enumerate() {
        out.push_str(&format!("{:08x}  ", line * HEX_WIDTH));
        for index in 0..HEX_WIDTH {
            match chunk.get(index) {
                Some(byte) => out.push_str(&format!("{byte:02x} ")),
                None => out.push_str("   "),
            }
            if index == 7 {
                out.push(' ');
            }
        }
        out.push_str(" |");
        out.extend(chunk.iter().map(|byte| {
            if (0x20..0x7f).contains(byte) {
                *byte as char
            } else {
                '.'
            }
        }));
        out.push_str("|\n");
    }
    out
}

/// Source or output text: a JSON string, or an array of strings (as notebooks store lines).
fn joined_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(lines)) => lines.iter().filter_map(Value::as_str).collect::<String>(),
        _ => String::new(),
    }
}

/// Renders the cells of a notebook as plain text: each cell with its kind, then its outputs.
pub(crate) fn notebook_view(json: &str) -> Result<String, String> {
    let notebook: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let cells = notebook
        .get("cells")
        .and_then(Value::as_array)
        .ok_or_else(|| "not a notebook: no cells".to_owned())?;
    let mut out = String::new();
    for (index, cell) in cells.iter().enumerate() {
        let kind = cell
            .get("cell_type")
            .and_then(Value::as_str)
            .unwrap_or("code");
        out.push_str(&format!("---- cell {} ({kind}) ----\n", index + 1));
        let source = joined_text(cell.get("source"));
        out.push_str(&source);
        if !source.is_empty() && !source.ends_with('\n') {
            out.push('\n');
        }
        for output in cell
            .get("outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = match output.get("output_type").and_then(Value::as_str) {
                Some("stream") => joined_text(output.get("text")),
                Some("error") => {
                    let name = output
                        .get("ename")
                        .and_then(Value::as_str)
                        .unwrap_or("error");
                    let value = output.get("evalue").and_then(Value::as_str).unwrap_or("");
                    format!("{name}: {value}\n")
                }
                _ => joined_text(output.get("data").and_then(|data| data.get("text/plain"))),
            };
            if !text.is_empty() {
                out.push_str("---- output ----\n");
                out.push_str(&text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
    Ok(out)
}

impl App {
    /// Opens `text` as a read-only document for `path`, replacing nothing on disk.
    pub(crate) fn open_read_only_view(&mut self, path: PathBuf, text: &str) {
        let mut buffer = Buffer::from_text(&path, text);
        buffer.set_read_only(true);
        let tab = Document {
            buffer,
            language: None,
            highlights: None,
            sticky_headers_cache: None,
            highlighter: IncrementalHighlighter::new(),
            diagnostics: Vec::new(),
            inlay_hints: Vec::new(),
            inlay_revision: None,
            provided_folds: None,
            schema_diagnostics: Vec::new(),
            schema_revision: None,
            lsp_revision: None,
            indentation: graphite_text::Indentation {
                insert_spaces: self.insert_spaces,
                tab_size: self.tab_size,
            },
            saved_format: (
                crate::file_format::Encoding::Utf8,
                crate::file_format::LineEnding::Lf,
            ),
            encoding: crate::file_format::Encoding::Utf8,
            line_ending: crate::file_format::LineEnding::Lf,
                    notebook: None,
        };
        self.push_navigation_location();
        let id = self.documents.push(tab);
        self.activate_editor_document(id);
        self.push_navigation_location();
        self.status = self.tf("view.read_only", &[("path", &path.display())]);
    }

    /// Opens a notebook as a readable view. Returns false when the file is not a notebook.
    #[allow(dead_code)]
    pub(crate) fn try_open_notebook_view(&mut self, path: &Path) -> bool {
        let is_notebook = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ipynb"));
        if !is_notebook {
            return false;
        }
        let Ok(json) = std::fs::read_to_string(path) else {
            return false;
        };
        match notebook_view(&json) {
            Ok(text) => {
                self.open_read_only_view(path.to_path_buf(), &text);
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_dump_shows_offsets_hex_and_ascii() {
        let dump = hex_dump(b"AB\x00\xffz");
        assert!(dump.starts_with("00000000  41 42 00 ff 7a "));
        assert!(dump.ends_with("|AB..z|\n"));
        assert_eq!(dump.lines().count(), 1);
        let long = hex_dump(&[0u8; 17]);
        assert!(long.lines().nth(1).unwrap().starts_with("00000010  00 "));
    }

    #[test]
    fn notebook_cells_and_outputs_are_rendered_as_text() {
        let json = r##"{
            "cells": [
                {"cell_type": "markdown", "source": ["# Title\n", "text"], "outputs": []},
                {"cell_type": "code", "source": "print(1)",
                 "outputs": [
                    {"output_type": "stream", "text": ["1\n"]},
                    {"output_type": "execute_result", "data": {"text/plain": "2"}},
                    {"output_type": "error", "ename": "E", "evalue": "boom"}
                 ]}
            ]
        }"##;
        let text = notebook_view(json).unwrap();
        assert!(text.contains("---- cell 1 (markdown) ----\n# Title\ntext\n"));
        assert!(text.contains("---- cell 2 (code) ----\nprint(1)\n"));
        assert!(text.contains("1\n"));
        assert!(text.contains("2\n"));
        assert!(text.contains("E: boom\n"));
        assert!(notebook_view("{}").is_err());
        assert!(notebook_view("not json").is_err());
    }

    #[test]
    fn binary_files_open_as_read_only_hex() {
        let dir = crate::test_support::TempDir::new();
        let binary = dir.path().join("data.bin");
        std::fs::write(&binary, [0u8, 159, 146, 150]).unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(binary.clone());
        let id = app.active_document();
        assert!(app.documents[id].buffer.is_read_only());
        assert!(app.editor().text().starts_with("00000000  00 9f 92 96"));
        assert!(app.documents[id].notebook.is_none());
    }
}
