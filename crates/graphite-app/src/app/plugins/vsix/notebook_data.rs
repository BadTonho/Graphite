//! Converting between the notebook data an extension works with (`NotebookDataDto`: cells with
//! output items made of bytes) and Graphite's notebook model (nbformat cells and outputs).

use crate::notebook::{Cell, CellKind, Notebook};
use base64::Engine;
use serde_json::{json, Map, Value};

const STDOUT: &str = "application/vnd.code.notebook.stdout";
const STDERR: &str = "application/vnd.code.notebook.stderr";
const ERROR: &str = "application/vnd.code.notebook.error";
/// `CellKind.Markup` and `CellKind.Code` of the extension API.
const MARKUP: u64 = 1;
const CODE: u64 = 2;

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode(text: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .unwrap_or_default()
}

/// The bytes of a `VSBuffer` as the bridge hands them over.
fn bytes_of(value: &Value) -> Vec<u8> {
    value["$bytes"]
        .as_str()
        .or_else(|| value["$graphiteBytes"].as_str())
        .or_else(|| value.as_str())
        .map(decode)
        .unwrap_or_default()
}

fn bytes_value(bytes: &[u8]) -> Value {
    json!({ "$graphiteBytes": encode(bytes) })
}

/// Whether notebooks store the data of `mime` as base64 text.
fn is_binary(mime: &str) -> bool {
    (mime.starts_with("image/") && mime != "image/svg+xml")
        || mime.starts_with("audio/")
        || mime.starts_with("video/")
        || mime == "application/pdf"
}

fn is_json(mime: &str) -> bool {
    mime == "application/json" || mime.ends_with("+json")
}

/// The text of a notebook value: a string, or a list of lines.
fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(lines) => lines.iter().filter_map(Value::as_str).collect(),
        other => other.to_string(),
    }
}

/// One output of the extension as a notebook output, with the id the extension gave it.
pub(super) fn output_from_dto(output: &Value, execution_count: Option<u64>) -> (String, Value) {
    let id = output["outputId"].as_str().unwrap_or_default().to_owned();
    let items = output["items"].as_array().cloned().unwrap_or_default();
    let mime = |item: &Value| item["mime"].as_str().unwrap_or_default().to_owned();
    if let Some(item) = items.iter().find(|item| mime(item) == ERROR) {
        let error: Value = serde_json::from_slice(&bytes_of(&item["valueBytes"])).unwrap_or(Value::Null);
        let stack = error["stack"].as_str().unwrap_or_default();
        let traceback: Vec<Value> = if stack.is_empty() {
            Vec::new()
        } else {
            stack.split('\n').map(|line| json!(line)).collect()
        };
        return (
            id,
            json!({
                "output_type": "error",
                "ename": error["name"].as_str().unwrap_or("Error"),
                "evalue": error["message"].as_str().unwrap_or_default(),
                "traceback": traceback,
            }),
        );
    }
    if let Some(item) = items
        .iter()
        .find(|item| matches!(mime(item).as_str(), STDOUT | STDERR))
    {
        let name = if mime(item) == STDERR { "stderr" } else { "stdout" };
        let text: String = items
            .iter()
            .filter(|other| mime(other) == mime(item))
            .map(|other| String::from_utf8_lossy(&bytes_of(&other["valueBytes"])).into_owned())
            .collect();
        return (id, json!({"output_type": "stream", "name": name, "text": text}));
    }
    let mut data = Map::new();
    for item in &items {
        let mime = mime(item);
        let bytes = bytes_of(&item["valueBytes"]);
        let value = if is_binary(&mime) {
            json!(encode(&bytes))
        } else if is_json(&mime) {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)))
        } else {
            json!(String::from_utf8_lossy(&bytes))
        };
        data.insert(mime, value);
    }
    let metadata = if output["metadata"].is_object() {
        output["metadata"].clone()
    } else {
        json!({})
    };
    let mut notebook_output = json!({"data": data, "metadata": metadata});
    match execution_count {
        Some(count) => {
            notebook_output["output_type"] = json!("execute_result");
            notebook_output["execution_count"] = json!(count);
        }
        None => notebook_output["output_type"] = json!("display_data"),
    }
    (id, notebook_output)
}

/// A notebook output as the extension receives it, with `id` as its output id.
pub(super) fn output_to_dto(output: &Value, id: &str) -> Value {
    let item = |mime: &str, bytes: &[u8]| json!({"mime": mime, "valueBytes": bytes_value(bytes)});
    let items: Vec<Value> = match output["output_type"].as_str() {
        Some("stream") => {
            let mime = if output["name"] == "stderr" { STDERR } else { STDOUT };
            vec![item(mime, text_of(&output["text"]).as_bytes())]
        }
        Some("error") => {
            let stack = output["traceback"]
                .as_array()
                .map(|lines| lines.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            let error = json!({
                "name": output["ename"].as_str().unwrap_or("Error"),
                "message": output["evalue"].as_str().unwrap_or_default(),
                "stack": stack,
            });
            vec![item(ERROR, error.to_string().as_bytes())]
        }
        _ => output["data"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(mime, value)| {
                let bytes = if is_binary(mime) {
                    decode(&text_of(value))
                } else if is_json(mime) && !value.is_string() {
                    value.to_string().into_bytes()
                } else {
                    text_of(value).into_bytes()
                };
                item(mime, &bytes)
            })
            .collect(),
    };
    let mut dto = json!({"outputId": id, "items": items});
    if output["metadata"].as_object().is_some_and(|metadata| !metadata.is_empty()) {
        dto["metadata"] = output["metadata"].clone();
    }
    dto
}

/// The language of the code cells of a notebook, from its metadata.
fn notebook_language(metadata: &Map<String, Value>) -> String {
    metadata
        .get("language_info")
        .and_then(|info| info["name"].as_str())
        .or_else(|| {
            metadata
                .get("kernelspec")
                .and_then(|spec| spec["language"].as_str())
        })
        .unwrap_or("plaintext")
        .to_owned()
}

/// The `NotebookDataDto` of a notebook, for a serializer.
pub(super) fn notebook_to_dto(notebook: &Notebook) -> Value {
    let default_language = notebook_language(&notebook.metadata);
    let cells: Vec<Value> = notebook
        .cells
        .iter()
        .map(|cell| {
            let code = cell.kind == CellKind::Code;
            let language = if code {
                cell.metadata
                    .get("vscode")
                    .and_then(|vscode| vscode["languageId"].as_str())
                    .unwrap_or(&default_language)
                    .to_owned()
            } else {
                "markdown".to_owned()
            };
            let outputs: Vec<Value> = cell
                .outputs
                .iter()
                .enumerate()
                .map(|(index, output)| output_to_dto(output, &format!("output-{index}")))
                .collect();
            let mut dto = json!({
                "source": cell.source,
                "language": language,
                "cellKind": if code { CODE } else { MARKUP },
                "outputs": outputs,
                "metadata": cell.metadata,
            });
            if let Some(count) = cell.execution_count {
                dto["internalMetadata"] = json!({"executionOrder": count});
            }
            dto
        })
        .collect();
    json!({"cells": cells, "metadata": notebook.metadata})
}

/// The notebook a serializer produced (`NotebookDataDto`).
pub(super) fn notebook_from_dto(data: &Value) -> Notebook {
    let data = if data["value"].is_object() { &data["value"] } else { data };
    let mut notebook = Notebook::new();
    notebook.cells = data["cells"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|dto| {
            let code = dto["cellKind"].as_u64() == Some(CODE);
            let mut cell = Cell::new(if code { CellKind::Code } else { CellKind::Markdown }, dto["source"].as_str().unwrap_or_default());
            if let Some(metadata) = dto["metadata"].as_object() {
                cell.metadata = metadata.clone();
            }
            if code {
                let count = dto["internalMetadata"]["executionOrder"].as_u64();
                cell.execution_count = count;
                cell.outputs = dto["outputs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|output| output_from_dto(output, None).1)
                    .collect();
                if let Some(language) = dto["language"].as_str().filter(|language| !language.is_empty()) {
                    let vscode = cell
                        .metadata
                        .entry("vscode")
                        .or_insert_with(|| json!({}));
                    if let Some(object) = vscode.as_object_mut() {
                        object.insert("languageId".into(), json!(language));
                    }
                }
            }
            cell
        })
        .collect();
    if notebook.cells.is_empty() {
        notebook.cells.push(Cell::new(CellKind::Code, ""));
    }
    if let Some(metadata) = data["metadata"].as_object() {
        notebook.metadata = metadata.clone();
    }
    notebook
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto_item(mime: &str, bytes: &[u8]) -> Value {
        json!({"mime": mime, "valueBytes": {"$bytes": encode(bytes)}})
    }

    #[test]
    fn streams_errors_and_rich_outputs_become_notebook_outputs() {
        let stream = json!({"outputId": "o1", "items": [dto_item(STDOUT, b"hi\n"), dto_item(STDOUT, b"there\n")]});
        let (id, output) = output_from_dto(&stream, None);
        assert_eq!(id, "o1");
        assert_eq!(output, json!({"output_type": "stream", "name": "stdout", "text": "hi\nthere\n"}));

        let error = json!({"outputId": "o2", "items": [dto_item(ERROR,
            br#"{"name":"Boom","message":"bad","stack":"line1\nline2"}"#)]});
        let (_, output) = output_from_dto(&error, None);
        assert_eq!(output["output_type"], "error");
        assert_eq!(output["ename"], "Boom");
        assert_eq!(output["traceback"], json!(["line1", "line2"]));

        let rich = json!({"outputId": "o3", "metadata": {"k": 1}, "items": [
            dto_item("text/plain", b"42"),
            dto_item("image/png", &[1, 2, 3]),
            dto_item("application/json", br#"{"a":1}"#),
            dto_item("image/svg+xml", b"<svg/>"),
        ]});
        let (_, output) = output_from_dto(&rich, Some(7));
        assert_eq!(output["output_type"], "execute_result");
        assert_eq!(output["execution_count"], 7);
        assert_eq!(output["data"]["text/plain"], "42");
        assert_eq!(output["data"]["image/png"], encode(&[1, 2, 3]));
        assert_eq!(output["data"]["application/json"], json!({"a": 1}));
        assert_eq!(output["data"]["image/svg+xml"], "<svg/>", "svg is text");
        assert_eq!(output["metadata"], json!({"k": 1}));
        assert_eq!(output_from_dto(&rich, None).1["output_type"], "display_data");
    }

    #[test]
    fn notebook_outputs_round_trip_through_the_extension_form() {
        let outputs = [
            json!({"output_type": "stream", "name": "stderr", "text": ["a\n", "b"]}),
            json!({"output_type": "error", "ename": "E", "evalue": "v", "traceback": ["t1", "t2"]}),
            json!({"output_type": "display_data", "data": {"text/plain": ["x\n", "y"], "image/png": encode(&[9, 8])}, "metadata": {}}),
        ];
        let (_, stream) = output_from_dto(&output_to_dto(&outputs[0], "s"), None);
        assert_eq!(stream["text"], "a\nb");
        assert_eq!(stream["name"], "stderr");
        let (_, error) = output_from_dto(&output_to_dto(&outputs[1], "e"), None);
        assert_eq!((error["ename"].clone(), error["traceback"].clone()), (json!("E"), json!(["t1", "t2"])));
        let (id, display) = output_from_dto(&output_to_dto(&outputs[2], "d"), None);
        assert_eq!(id, "d");
        assert_eq!(display["data"]["text/plain"], "x\ny");
        assert_eq!(display["data"]["image/png"], encode(&[9, 8]));
    }

    #[test]
    fn notebooks_round_trip_through_notebook_data() {
        let dto = json!({
            "metadata": {"custom": true, "language_info": {"name": "python"}},
            "cells": [
                {"cellKind": 1, "source": "# Title", "language": "markdown", "outputs": []},
                {"cellKind": 2, "source": "print(1)", "language": "python", "metadata": {"tag": 1},
                 "internalMetadata": {"executionOrder": 3},
                 "outputs": [{"outputId": "a", "items": [dto_item(STDOUT, b"1\n")]}]},
            ]
        });
        let notebook = notebook_from_dto(&json!({"value": dto}));
        assert_eq!(notebook.cells.len(), 2);
        assert_eq!(notebook.cells[0].kind, CellKind::Markdown);
        assert_eq!(notebook.cells[1].execution_count, Some(3));
        assert_eq!(notebook.cells[1].outputs[0]["text"], "1\n");
        assert_eq!(notebook.cells[1].metadata["tag"], 1);
        assert_eq!(notebook.metadata["custom"], true);
        let back = notebook_to_dto(&notebook);
        assert_eq!(back["cells"][0]["cellKind"], 1);
        assert_eq!(back["cells"][0]["language"], "markdown");
        assert_eq!(back["cells"][1]["cellKind"], 2);
        assert_eq!(back["cells"][1]["language"], "python");
        assert_eq!(back["cells"][1]["internalMetadata"]["executionOrder"], 3);
        assert_eq!(back["cells"][1]["outputs"][0]["items"][0]["mime"], STDOUT);
        assert_eq!(
            back["cells"][1]["outputs"][0]["items"][0]["valueBytes"],
            json!({"$graphiteBytes": encode(b"1\n")})
        );
        assert_eq!(back["metadata"]["custom"], true);
    }

    #[test]
    fn an_empty_serializer_result_still_opens_with_one_cell() {
        let notebook = notebook_from_dto(&json!({"cells": [], "metadata": {}}));
        assert_eq!(notebook.cells.len(), 1);
        assert_eq!(notebook.cells[0].kind, CellKind::Code);
    }
}
