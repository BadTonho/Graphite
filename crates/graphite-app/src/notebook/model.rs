//! The `.ipynb` document: reading and writing the JSON format without losing anything this
//! program does not understand.
//!
//! Every object keeps the fields it does not know in an `extra` map, outputs and metadata stay as
//! raw JSON, and the file is written the way Jupyter writes it (sorted keys, one-space indent,
//! source split into lines, a final newline), so saving an untouched notebook changes nothing
//! but formatting that already differed.

use serde_json::{json, Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellKind {
    Code,
    Markdown,
    Raw,
}

impl CellKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            CellKind::Code => "code",
            CellKind::Markdown => "markdown",
            CellKind::Raw => "raw",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "code" => Some(CellKind::Code),
            "markdown" => Some(CellKind::Markdown),
            "raw" => Some(CellKind::Raw),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cell {
    pub kind: CellKind,
    /// Cell id; required from format 4.5 on.
    pub id: Option<String>,
    pub source: String,
    /// Outputs as stored; only code cells have them.
    pub outputs: Vec<Value>,
    pub execution_count: Option<u64>,
    pub metadata: Map<String, Value>,
    /// Fields of the cell this program does not use, kept to write them back.
    extra: Map<String, Value>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// A new cell id: eight lowercase hex digits, unique within the process and unlikely to collide
/// with ids written by other tools.
pub(crate) fn new_cell_id() -> String {
    let counter = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            elapsed.subsec_nanos() as u64 ^ elapsed.as_secs()
        });
    let mixed = nanos
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(counter.wrapping_mul(0xbf58_476d_1ce4_e5b9));
    format!("{:08x}", (mixed >> 16) as u32 ^ mixed as u32)
}

impl Cell {
    pub(crate) fn new(kind: CellKind, source: &str) -> Self {
        Self {
            kind,
            id: Some(new_cell_id()),
            source: source.to_owned(),
            outputs: Vec::new(),
            execution_count: None,
            metadata: Map::new(),
            extra: Map::new(),
        }
    }

    /// Changes the kind of the cell. Leaving the code kind drops what only code cells carry.
    pub(crate) fn set_kind(&mut self, kind: CellKind) {
        self.kind = kind;
        if kind != CellKind::Code {
            self.outputs.clear();
            self.execution_count = None;
        }
    }

    fn from_json(value: Value) -> Result<Self, String> {
        let Value::Object(mut object) = value else {
            return Err("a cell is not an object".into());
        };
        let kind = object
            .remove("cell_type")
            .as_ref()
            .and_then(Value::as_str)
            .and_then(CellKind::parse)
            .ok_or_else(|| "a cell has an unknown cell_type".to_owned())?;
        let source = match object.remove("source") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(text)) => text,
            Some(Value::Array(lines)) => lines
                .iter()
                .map(|line| {
                    line.as_str()
                        .ok_or_else(|| "a source line is not a string".to_owned())
                })
                .collect::<Result<String, _>>()?,
            Some(_) => return Err("the source of a cell is not text".into()),
        };
        let id = match object.remove("id") {
            Some(Value::String(id)) => Some(id),
            _ => None,
        };
        let outputs = match object.remove("outputs") {
            Some(Value::Array(outputs)) if kind == CellKind::Code => outputs,
            Some(other) if kind != CellKind::Code => {
                // Not valid for this kind; keep it rather than drop it.
                object.insert("outputs".into(), other);
                Vec::new()
            }
            _ => Vec::new(),
        };
        let execution_count = match object.remove("execution_count") {
            Some(Value::Number(number)) => number.as_u64(),
            Some(Value::Null) | None => None,
            Some(other) => {
                object.insert("execution_count".into(), other);
                None
            }
        };
        let metadata = match object.remove("metadata") {
            Some(Value::Object(metadata)) => metadata,
            _ => Map::new(),
        };
        Ok(Self {
            kind,
            id,
            source,
            outputs,
            execution_count,
            metadata,
            extra: object,
        })
    }

    fn to_json(&self) -> Value {
        let mut object = self.extra.clone();
        object.insert("cell_type".into(), json!(self.kind.name()));
        object.insert("metadata".into(), Value::Object(self.metadata.clone()));
        object.insert("source".into(), source_lines(&self.source));
        if let Some(id) = &self.id {
            object.insert("id".into(), json!(id));
        }
        if self.kind == CellKind::Code {
            object.insert("outputs".into(), Value::Array(self.outputs.clone()));
            object.insert("execution_count".into(), json!(self.execution_count));
        }
        Value::Object(object)
    }
}

/// Splits source into the array of lines Jupyter stores: every line keeps its line break and the
/// last one has none.
fn source_lines(source: &str) -> Value {
    Value::Array(
        source
            .split_inclusive('\n')
            .map(|line| json!(line))
            .collect(),
    )
}

/// A notebook of format 4.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Notebook {
    pub cells: Vec<Cell>,
    pub metadata: Map<String, Value>,
    pub nbformat: u64,
    pub nbformat_minor: u64,
    extra: Map<String, Value>,
}

impl Notebook {
    /// An empty notebook with one empty code cell.
    pub(crate) fn new() -> Self {
        Self {
            cells: vec![Cell::new(CellKind::Code, "")],
            metadata: Map::new(),
            nbformat: 4,
            nbformat_minor: 5,
            extra: Map::new(),
        }
    }

    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let Value::Object(mut object) = value else {
            return Err("not a notebook: the top level is not an object".into());
        };
        let nbformat = object
            .remove("nbformat")
            .and_then(|value| value.as_u64())
            .ok_or_else(|| "not a notebook: no nbformat".to_owned())?;
        if nbformat != 4 {
            return Err(format!("notebook format {nbformat} is not supported"));
        }
        let nbformat_minor = object
            .remove("nbformat_minor")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let cells = match object.remove("cells") {
            Some(Value::Array(cells)) => cells,
            _ => return Err("not a notebook: no cells".into()),
        };
        let mut cells = cells
            .into_iter()
            .map(Cell::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        // From 4.5 every cell has an id, and ids are unique.
        if nbformat_minor >= 5 {
            let mut seen = std::collections::HashSet::new();
            for cell in &mut cells {
                let keep = cell
                    .id
                    .as_ref()
                    .is_some_and(|id| !id.is_empty() && seen.insert(id.clone()));
                if !keep {
                    loop {
                        let id = new_cell_id();
                        if seen.insert(id.clone()) {
                            cell.id = Some(id);
                            break;
                        }
                    }
                }
            }
        }
        let metadata = match object.remove("metadata") {
            Some(Value::Object(metadata)) => metadata,
            _ => Map::new(),
        };
        Ok(Self {
            cells,
            metadata,
            nbformat,
            nbformat_minor,
            extra: object,
        })
    }

    /// The notebook as Jupyter writes it.
    pub(crate) fn to_json(&self) -> String {
        let mut object = self.extra.clone();
        object.insert("nbformat".into(), json!(self.nbformat));
        object.insert("nbformat_minor".into(), json!(self.nbformat_minor));
        object.insert("metadata".into(), Value::Object(self.metadata.clone()));
        object.insert(
            "cells".into(),
            Value::Array(self.cells.iter().map(Cell::to_json).collect()),
        );
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
        serde::Serialize::serialize(&Value::Object(object), &mut serializer)
            .expect("a JSON value always serializes");
        let mut text = String::from_utf8(out).expect("JSON is UTF-8");
        text.push('\n');
        text
    }

    /// Name of the kernel the notebook asks for, if any.
    pub(crate) fn kernel_name(&self) -> Option<&str> {
        self.metadata.get("kernelspec")?.get("name")?.as_str()
    }

    /// Language of the notebook's code cells, if it says.
    pub(crate) fn language(&self) -> Option<&str> {
        self.metadata
            .get("language_info")
            .and_then(|info| info.get("name"))
            .or_else(|| {
                self.metadata
                    .get("kernelspec")
                    .and_then(|spec| spec.get("language"))
            })?
            .as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
 "cells": [
  {
   "cell_type": "markdown",
   "id": "aaaa1111",
   "metadata": {"tags": ["intro"]},
   "source": ["# Title\n", "text"],
   "future_field": {"keep": [1, 2]}
  },
  {
   "cell_type": "code",
   "execution_count": 3,
   "id": "bbbb2222",
   "metadata": {"collapsed": true},
   "outputs": [
    {"name": "stdout", "output_type": "stream", "text": ["hi\n"], "extra_output_field": 1},
    {"output_type": "execute_result", "data": {"text/plain": ["2"]}, "execution_count": 3, "metadata": {}}
   ],
   "source": "print('hi')\n1 + 1"
  }
 ],
 "metadata": {
  "kernelspec": {"display_name": "Python 3", "language": "python", "name": "python3"},
  "language_info": {"name": "python", "version": "3.12"},
  "custom": {"deep": {"er": null}}
 },
 "nbformat": 4,
 "nbformat_minor": 5,
 "top_level_unknown": "stay"
}"##;

    #[test]
    fn reads_cells_with_array_or_string_source() {
        let notebook = Notebook::parse(SAMPLE).unwrap();
        assert_eq!(notebook.cells.len(), 2);
        assert_eq!(notebook.cells[0].kind, CellKind::Markdown);
        assert_eq!(notebook.cells[0].source, "# Title\ntext");
        assert_eq!(notebook.cells[1].source, "print('hi')\n1 + 1");
        assert_eq!(notebook.cells[1].execution_count, Some(3));
        assert_eq!(notebook.cells[1].outputs.len(), 2);
        assert_eq!(notebook.cells[0].id.as_deref(), Some("aaaa1111"));
        assert_eq!(notebook.kernel_name(), Some("python3"));
        assert_eq!(notebook.language(), Some("python"));
    }

    #[test]
    fn writing_keeps_every_field_it_does_not_understand() {
        let notebook = Notebook::parse(SAMPLE).unwrap();
        let written: Value = serde_json::from_str(&notebook.to_json()).unwrap();
        let original: Value = serde_json::from_str(SAMPLE).unwrap();
        // Same content; only the storage of source (string or lines) may differ.
        assert_eq!(written["top_level_unknown"], original["top_level_unknown"]);
        assert_eq!(written["metadata"], original["metadata"]);
        assert_eq!(
            written["cells"][0]["future_field"],
            original["cells"][0]["future_field"]
        );
        assert_eq!(
            written["cells"][0]["metadata"],
            original["cells"][0]["metadata"]
        );
        assert_eq!(
            written["cells"][1]["outputs"],
            original["cells"][1]["outputs"]
        );
        assert_eq!(
            written["cells"][1]["metadata"],
            original["cells"][1]["metadata"]
        );
        assert_eq!(
            written["cells"][1]["source"],
            json!(["print('hi')\n", "1 + 1"])
        );
        assert_eq!(written["cells"][0]["source"], json!(["# Title\n", "text"]));
    }

    #[test]
    fn a_written_notebook_reads_back_identically_and_is_stable() {
        let notebook = Notebook::parse(SAMPLE).unwrap();
        let first = notebook.to_json();
        let again = Notebook::parse(&first).unwrap();
        assert_eq!(again, notebook);
        assert_eq!(again.to_json(), first);
    }

    #[test]
    fn output_follows_the_jupyter_layout() {
        let text = Notebook::parse(SAMPLE).unwrap().to_json();
        assert!(text.ends_with("}\n"));
        assert!(text.starts_with("{\n \"cells\": [\n  {\n   \"cell_type\": \"markdown\""));
        // Keys are sorted: cells, metadata, nbformat, nbformat_minor, top_level_unknown.
        let positions: Vec<usize> = [
            "\"cells\"",
            "\"metadata\": {\n  \"custom\"",
            "\"nbformat\"",
            "\"nbformat_minor\"",
            "\"top_level_unknown\"",
        ]
        .iter()
        .map(|key| text.find(key).unwrap_or_else(|| panic!("{key}")))
        .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn unicode_is_not_escaped_and_crlf_survives() {
        let mut notebook = Notebook::new();
        notebook.cells[0].source = "print('olá 🌎')\r\nx".into();
        let text = notebook.to_json();
        assert!(text.contains("olá 🌎"));
        let back = Notebook::parse(&text).unwrap();
        assert_eq!(back.cells[0].source, "print('olá 🌎')\r\nx");
    }

    #[test]
    fn missing_pieces_get_defaults_and_markdown_has_no_output_fields() {
        let notebook = Notebook::parse(
            r#"{"nbformat":4,"nbformat_minor":2,"cells":[{"cell_type":"code","source":[]},{"cell_type":"markdown"}]}"#,
        )
        .unwrap();
        assert_eq!(notebook.cells[0].source, "");
        assert!(notebook.cells[0].outputs.is_empty());
        assert_eq!(notebook.cells[0].id, None);
        let written: Value = serde_json::from_str(&notebook.to_json()).unwrap();
        assert_eq!(written["cells"][0]["execution_count"], Value::Null);
        assert_eq!(written["cells"][0]["outputs"], json!([]));
        assert!(written["cells"][1].get("outputs").is_none());
        assert!(written["cells"][1].get("execution_count").is_none());
        // Format 4.2 has no cell ids and none are invented.
        assert!(written["cells"][0].get("id").is_none());
    }

    #[test]
    fn format_4_5_gives_every_cell_a_unique_id() {
        let notebook = Notebook::parse(
            r#"{"nbformat":4,"nbformat_minor":5,"cells":[
                {"cell_type":"code","id":"dup","source":"","outputs":[],"execution_count":null,"metadata":{}},
                {"cell_type":"code","id":"dup","source":"","outputs":[],"execution_count":null,"metadata":{}},
                {"cell_type":"markdown","source":"","metadata":{}}]}"#,
        )
        .unwrap();
        let ids: Vec<_> = notebook
            .cells
            .iter()
            .map(|cell| cell.id.clone().unwrap())
            .collect();
        assert_eq!(ids[0], "dup");
        assert_ne!(ids[1], "dup");
        assert_ne!(ids[2], ids[1]);
        assert_ne!(ids[2], "dup");
        assert!(ids.iter().all(|id| !id.is_empty()));
    }

    #[test]
    fn invalid_notebooks_are_rejected_with_a_reason() {
        for (text, needle) in [
            ("not json", "expected"),
            ("[]", "object"),
            (r#"{"cells":[]}"#, "nbformat"),
            (r#"{"nbformat":3,"cells":[]}"#, "not supported"),
            (r#"{"nbformat":4}"#, "cells"),
            (r#"{"nbformat":4,"cells":[1]}"#, "not an object"),
            (
                r#"{"nbformat":4,"cells":[{"cell_type":"video"}]}"#,
                "cell_type",
            ),
            (
                r#"{"nbformat":4,"cells":[{"cell_type":"code","source":5}]}"#,
                "not text",
            ),
            (
                r#"{"nbformat":4,"cells":[{"cell_type":"code","source":[1]}]}"#,
                "not a string",
            ),
        ] {
            let error = Notebook::parse(text).unwrap_err();
            assert!(error.contains(needle), "{text}: {error}");
        }
    }

    #[test]
    fn changing_a_cell_to_text_drops_its_outputs() {
        let mut notebook = Notebook::parse(SAMPLE).unwrap();
        notebook.cells[1].set_kind(CellKind::Markdown);
        assert!(notebook.cells[1].outputs.is_empty());
        assert_eq!(notebook.cells[1].execution_count, None);
        let written: Value = serde_json::from_str(&notebook.to_json()).unwrap();
        assert!(written["cells"][1].get("outputs").is_none());
        notebook.cells[1].set_kind(CellKind::Code);
        let written: Value = serde_json::from_str(&notebook.to_json()).unwrap();
        assert_eq!(written["cells"][1]["outputs"], json!([]));
    }

    #[test]
    fn new_cell_ids_do_not_repeat() {
        let ids: std::collections::HashSet<_> = (0..1000).map(|_| new_cell_id()).collect();
        assert_eq!(ids.len(), 1000);
        assert!(ids.iter().all(|id| id.len() == 8));
    }

    #[test]
    fn a_new_notebook_has_one_empty_code_cell() {
        let notebook = Notebook::new();
        assert_eq!(notebook.cells.len(), 1);
        assert_eq!(notebook.cells[0].kind, CellKind::Code);
        assert_eq!(Notebook::parse(&notebook.to_json()).unwrap(), notebook);
    }
}
