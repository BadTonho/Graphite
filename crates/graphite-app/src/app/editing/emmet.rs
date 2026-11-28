use super::*;
use crate::process_util::HideConsole;
use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

const EMMET_BUNDLE: &str = include_str!("../../../runtime/emmet/dist/emmet.cjs");
const EMMET_WORKER: &str = include_str!("../../../runtime/emmet-worker.cjs");

pub(crate) struct EmmetRuntime {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl EmmetRuntime {
    pub(crate) fn start(node: &std::ffi::OsStr) -> Result<Self, String> {
        Self::start_in(node, &config::app_config_dir())
    }

    fn start_in(node: &std::ffi::OsStr, config_root: &Path) -> Result<Self, String> {
        let runtime_dir = config_root.join("runtime");
        std::fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;
        let bundle = runtime_dir.join("emmet.cjs");
        let worker = runtime_dir.join("emmet-worker.cjs");
        std::fs::write(&bundle, EMMET_BUNDLE).map_err(|e| e.to_string())?;
        std::fs::write(&worker, EMMET_WORKER).map_err(|e| e.to_string())?;
        let mut child = Command::new(node)
            .hide_console()
            .arg(worker)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Cannot start Emmet runtime: {e}"))?;
        let input = BufWriter::new(child.stdin.take().ok_or("Emmet input unavailable")?);
        let output = BufReader::new(child.stdout.take().ok_or("Emmet output unavailable")?);
        Ok(Self {
            child,
            input,
            output,
        })
    }

    pub(crate) fn expand(&mut self, abbreviation: &str, syntax: &str) -> Option<String> {
        let request = serde_json::json!({"abbreviation":abbreviation,"syntax":syntax});
        serde_json::to_writer(&mut self.input, &request).ok()?;
        self.input.write_all(b"\n").ok()?;
        self.input.flush().ok()?;
        let mut response = String::new();
        if self.output.read_line(&mut response).ok()? == 0 {
            return None;
        }
        serde_json::from_str::<serde_json::Value>(&response)
            .ok()?
            .get("value")?
            .as_str()
            .map(str::to_owned)
    }
}

impl Drop for EmmetRuntime {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl App {
    pub(crate) fn can_expand_emmet_at_cursor(&self) -> bool {
        let document = &self.documents[self.command_document()];
        let Some(path) = document.buffer.path() else {
            return false;
        };
        if !matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "html" | "htm" | "jsx" | "js" | "mjs" | "css"
        ) {
            return false;
        }
        let text = document.buffer.text().chars().collect::<Vec<_>>();
        let cursor = self.editor().cursor().char_idx;
        text.into_iter()
            .take(cursor)
            .rev()
            .take_while(|character| !character.is_whitespace())
            .next()
            .is_some()
    }

    pub(crate) fn expand_emmet_at_cursor(&mut self) -> bool {
        let path = self.documents[self.command_document()]
            .buffer
            .path()
            .map(|p| p.to_path_buf());
        let Some(path) = path else {
            return false;
        };
        let language = match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "html" | "htm" => "html",
            "jsx" => "jsx",
            "js" | "mjs" => "javascript",
            "css" => "css",
            _ => return false,
        };
        let editor = self.editor();
        let cursor = editor.cursor().char_idx;
        let text = editor.text();
        let before = text.chars().take(cursor).collect::<String>();
        let start = before
            .char_indices()
            .rev()
            .find(|(_, ch)| ch.is_whitespace())
            .map_or(0, |(idx, _)| before[..idx].chars().count() + 1);
        let abbreviation = &before[start..];
        let node = self
            .service_configuration()
            .ok()
            .and_then(|configuration| {
                configuration
                    .get("graphite.plugins.nodePath")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "node".into());
        if self.emmet_runtime.is_none() {
            self.emmet_runtime = EmmetRuntime::start(std::ffi::OsStr::new(&node)).ok();
        }
        let Some(expanded) = self
            .emmet_runtime
            .as_mut()
            .and_then(|runtime| runtime.expand(abbreviation, language))
        else {
            return false;
        };
        let end = cursor;
        self.editor_mut()
            .apply_transaction(&[graphite_text::TextReplacement {
                range: start..end,
                text: expanded,
            }])
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_upstream_html_jsx_and_css_abbreviations_and_rejects_invalid_input() {
        let dir = crate::test_support::TempDir::new();
        let mut runtime = EmmetRuntime::start_in(std::ffi::OsStr::new("node"), dir.path()).unwrap();
        let nested = runtime.expand("ul>li.item$*2", "html").unwrap();
        assert!(nested.contains("class=\"item1\""));
        assert!(nested.contains("class=\"item2\""));
        let jsx = runtime.expand("div.card#main{hello}", "jsx").unwrap();
        assert!(jsx.contains("id=\"main\""));
        assert!(jsx.contains("hello"));
        assert!(runtime.expand("d:f", "css").is_some());
        assert!(runtime.expand("???", "html").is_none());
    }
}
