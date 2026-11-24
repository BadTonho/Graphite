//! Builds workspace and active-editor context for chat and inline-chat prompts.

use super::*;
use graphite_chat::{ActiveFileContext, ChatContext, FileAttachment};

#[derive(Default)]
pub(super) struct ChatAttachments {
    pub active_file: Option<ActiveFileContext>,
    pub selection: Option<FileAttachment>,
    pub diagnostics: Vec<String>,
}

impl ChatAttachments {
    pub fn selected(&self) -> [bool; 3] {
        [
            self.active_file.is_some(),
            self.selection.is_some(),
            !self.diagnostics.is_empty(),
        ]
    }
}

impl App {
    pub(super) fn chat_context_for_turn(&self) -> ChatContext {
        let attached = &self.chat.attachments;
        ChatContext {
            workspace_root: self.tree.root().to_path_buf(),
            active_file: attached.active_file.clone(),
            diagnostics: attached.diagnostics.clone(),
            referenced_files: attached.selection.iter().cloned().collect(),
        }
    }

    pub(super) fn toggle_chat_attachment(&mut self, attachment: usize) {
        let current = self.gather_chat_context();
        let attached = &mut self.chat.attachments;
        let was_selected = attached.selected()[attachment.min(2)];
        match attachment {
            0 => {
                attached.active_file = if attached.active_file.is_none() {
                    current.active_file.map(|mut file| {
                        file.selection = None;
                        file
                    })
                } else {
                    None
                };
            }
            1 => {
                attached.selection = if attached.selection.is_none() {
                    current.active_file.and_then(|file| {
                        file.selection
                            .filter(|text| !text.is_empty())
                            .map(|content| FileAttachment {
                                path: file.path,
                                content: content.chars().take(8_000).collect(),
                            })
                    })
                } else {
                    None
                };
            }
            _ => {
                if attached.diagnostics.is_empty() {
                    attached.diagnostics = current.diagnostics;
                } else {
                    attached.diagnostics.clear();
                }
            }
        }
        if !was_selected && !attached.selected()[attachment.min(2)] {
            let available = match attachment {
                0 => self.t("chat.no_file"),
                1 => self.t("chat.no_selection"),
                _ => self.t("chat.no_diagnostics"),
            };
            self.status = available.into();
        }
    }

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
