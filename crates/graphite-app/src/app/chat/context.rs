//! Builds workspace and active-editor context for chat and inline-chat prompts.

use super::*;
use graphite_chat::{ActiveFileContext, ChatContext};

impl App {
    pub(crate) fn gather_chat_context(&self) -> ChatContext {
        let doc_id = self.active_document();
        let tab = &self.documents[doc_id];
        let editor = self.editor();
        let (cursor_line, cursor_col) = editor.cursor_line_col();
        let selection = editor.selected_text();

        let total_lines = tab.buffer.len_lines();
        let excerpt_start = cursor_line.saturating_sub(40);
        let excerpt_end = (cursor_line + 80).min(total_lines);
        let mut excerpt = String::new();
        for line in excerpt_start..excerpt_end {
            excerpt.push_str(&format!("{}: {}\n", line + 1, tab.buffer.line(line)));
            if excerpt.len() > 8_000 {
                break;
            }
        }

        let path = tab
            .buffer
            .path()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("untitled"));
        let language = tab
            .language
            .map(|lang| format!("{lang:?}"))
            .unwrap_or_else(|| "PlainText".into());

        let mut diagnostics = Vec::new();
        for diag in tab.diagnostics.iter().chain(tab.schema_diagnostics.iter()) {
            diagnostics.push(format!("L{}: {}", diag.range.start.line + 1, diag.message));
        }

        ChatContext {
            workspace_root: self.tree.root().to_path_buf(),
            active_file: Some(ActiveFileContext {
                path,
                language,
                cursor_line,
                cursor_col,
                selection,
                content_excerpt: excerpt,
            }),
            diagnostics,
            referenced_files: Vec::new(),
        }
    }
}
