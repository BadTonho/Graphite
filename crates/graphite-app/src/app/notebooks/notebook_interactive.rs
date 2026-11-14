//! The interactive window and the REPL in the editor: opening them, sending code to them, their
//! input box and its keys.

use super::clipboard::{copy_or_cut, paste, SystemClipboard};
use super::*;
use crate::notebook::{InteractiveKind, Session};

impl App {
    /// Whether the active editor is an interactive window, and whether its input box has the
    /// keyboard.
    pub(crate) fn interactive_context(&self) -> (bool, bool) {
        match self.active_notebook().and_then(Session::interactive) {
            Some(state) => (true, state.input_focused),
            None => (false, false),
        }
    }

    /// Opens a new interactive window or REPL in a tab. `language` is the language of the code
    /// that will be sent to it, which helps choose a kernel.
    pub(crate) fn open_interactive(&mut self, kind: InteractiveKind, language: Option<String>) {
        let mut session = Session::new_interactive(kind);
        session.set_language_hint(language);
        let document = Document {
            buffer: Buffer::new(),
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
            notebook: Some(Box::new(session)),
        };
        let id = self.documents.push(document);
        self.activate_editor_document(id);
        self.sync_notebook_buffer(id);
        self.interactive_document = Some(id);
        self.status = self.t("notebook.interactive.opened").into();
    }

    /// The open interactive window, if there is one.
    fn existing_interactive(&self) -> Option<DocumentId> {
        let id = self.interactive_document?;
        let document = self.documents.get(id)?;
        document
            .notebook
            .as_deref()
            .and_then(Session::interactive)
            .map(|_| id)
    }

    /// Runs what is in the input box. The input stays in the box when no kernel can be had.
    pub(crate) fn interactive_execute(&mut self) -> Result<(), String> {
        if self
            .active_notebook()
            .and_then(Session::interactive)
            .is_none()
        {
            return Err(self.t("notebook.interactive.not_open").into());
        }
        let blank = self
            .active_notebook()
            .and_then(Session::interactive)
            .is_some_and(|state| state.input_text().trim().is_empty());
        if blank {
            return Ok(());
        }
        self.ensure_kernel()?;
        let document = self.active_document();
        let Some(session) = self.documents[document].notebook.as_deref_mut() else {
            return Ok(());
        };
        let Some(index) = session.submit_input() else {
            return Ok(());
        };
        let sent = session.run_cells(&[index]);
        self.with_notebook(|_| {});
        sent.map(|_| ())
    }

    /// Sends `code` from an editor to the interactive window, opening one if needed.
    pub(crate) fn send_to_interactive(
        &mut self,
        code: &str,
        language: Option<String>,
    ) -> Result<(), String> {
        if code.trim().is_empty() {
            return Err(self.t("notebook.interactive.nothing_to_send").into());
        }
        match self.existing_interactive() {
            Some(id) => self.activate_editor_document(id),
            None => self.open_interactive(InteractiveKind::Window, language),
        }
        self.with_notebook(|session| {
            session.set_input_text(code);
            session.focus_input(true);
        });
        self.interactive_execute()
    }

    /// The selected text of the active editor, or its current line, and the language of the file.
    pub(crate) fn text_to_send(&self) -> (String, Option<String>) {
        let document = self.command_document();
        let tab = &self.documents[document];
        let editor = self.editor();
        let text = editor
            .selected_text()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| {
                let line = editor.cursor_line_col().0;
                tab.buffer
                    .line(line)
                    .trim_end_matches(['\n', '\r'])
                    .to_owned()
            });
        let language = tab
            .buffer
            .path()
            .and_then(|path| path.extension())
            .and_then(|extension| {
                match extension.to_string_lossy().to_ascii_lowercase().as_str() {
                    "py" => Some("python"),
                    "jl" => Some("julia"),
                    "r" => Some("r"),
                    "js" => Some("javascript"),
                    _ => None,
                }
                .map(str::to_owned)
            });
        (text, language)
    }

    /// Keys while the input box has the keyboard.
    pub(crate) fn handle_interactive_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        let alt = self.modifiers.alt_key();
        let character = event.text.as_deref().unwrap_or("").to_owned();
        let lower = character.to_ascii_lowercase();
        let repl = self
            .active_notebook()
            .and_then(Session::interactive)
            .is_some_and(|state| state.kind == InteractiveKind::Repl);
        let tab_text = " ".repeat(self.tab_size.max(1));
        macro_rules! caret {
            ($method:ident) => {
                self.with_notebook(|session| {
                    session.edit_input(|editor| {
                        let anchor = editor
                            .selection()
                            .map(|selection| selection.anchor)
                            .unwrap_or(editor.cursor().char_idx);
                        editor.$method();
                        if shift {
                            let head = editor.cursor().char_idx;
                            editor.set_selection(anchor, head);
                        }
                    });
                })
            };
        }
        // Enter runs in the REPL and adds a line in the window; Shift flips that.
        let enter_runs = ctrl || (repl && !shift) || (!repl && shift);
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => self.run_ui_command("interactive.history.focus"),
            Key::Named(NamedKey::Enter) if enter_runs => self.run_ui_command("interactive.execute"),
            Key::Named(NamedKey::Enter) => self.with_notebook(|session| {
                session.edit_input(|editor| editor.insert_newline());
            }),
            Key::Named(NamedKey::Tab) => self.with_notebook(|session| {
                session.edit_input(|editor| editor.insert(&tab_text));
            }),
            Key::Named(NamedKey::Backspace) => self.with_notebook(|session| {
                session.edit_input(|editor| editor.backspace());
            }),
            Key::Named(NamedKey::Delete) => self.with_notebook(|session| {
                session.edit_input(|editor| editor.delete());
            }),
            Key::Named(NamedKey::ArrowLeft) => caret!(move_left),
            Key::Named(NamedKey::ArrowRight) => caret!(move_right),
            Key::Named(NamedKey::Home) => caret!(move_home),
            Key::Named(NamedKey::End) => caret!(move_end),
            Key::Named(NamedKey::ArrowUp) => {
                let at_edge = self
                    .active_notebook()
                    .is_some_and(|session| session.input_caret_at_edge(true));
                if at_edge && !shift {
                    self.run_ui_command("interactive.history.previous");
                } else {
                    self.with_notebook(|session| {
                        session.edit_input(|editor| editor.move_vertical(-1));
                    });
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                let at_edge = self
                    .active_notebook()
                    .is_some_and(|session| session.input_caret_at_edge(false));
                if at_edge && !shift {
                    self.run_ui_command("interactive.history.next");
                } else {
                    self.with_notebook(|session| {
                        session.edit_input(|editor| editor.move_vertical(1));
                    });
                }
            }
            Key::Named(NamedKey::PageUp) => self.notebook_scroll(-10.0),
            Key::Named(NamedKey::PageDown) => self.notebook_scroll(10.0),
            _ if ctrl && lower == "a" => self.with_notebook(|session| {
                session.edit_input(|editor| {
                    let end = editor.len_chars();
                    editor.set_selection(0, end);
                });
            }),
            _ if ctrl && lower == "z" => self.with_notebook(|session| {
                session.edit_input(|editor| {
                    editor.undo();
                });
            }),
            _ if ctrl && lower == "y" => self.with_notebook(|session| {
                session.edit_input(|editor| {
                    editor.redo();
                });
            }),
            _ if ctrl && matches!(lower.as_str(), "c" | "x") => {
                let cut = lower == "x";
                self.with_notebook(|session| {
                    session.edit_input(|editor| {
                        copy_or_cut(editor, &mut SystemClipboard, cut);
                    });
                })
            }
            _ if ctrl && lower == "v" => self.with_notebook(|session| {
                session.edit_input(|editor| {
                    paste(editor, &mut SystemClipboard);
                });
            }),
            _ if !ctrl && !alt => {
                if !character.is_empty() && !character.chars().any(char::is_control) {
                    self.with_notebook(|session| {
                        session.edit_input(|editor| editor.insert(&character));
                    });
                }
            }
            _ => {}
        }
    }

    /// A click on an interactive window: in the docked input box it focuses the box and puts the
    /// caret there; above it, it gives the keyboard back to the cells. Returns whether the click
    /// was in the box.
    pub(crate) fn interactive_click(&mut self, mx: f32, my: f32, box_top: f32) -> bool {
        let (_, line_height, char_width) = self.notebook_viewport();
        let Some(state) = self.active_notebook().and_then(Session::interactive) else {
            return false;
        };
        if my < box_top {
            self.with_notebook(|session| session.focus_input(false));
            return false;
        }
        let shown = state.input_lines();
        let (cursor_line, _) = state.input.view(&state.input_selection).cursor_line_col();
        let first = cursor_line
            .saturating_sub(shown - 1)
            .min(state.input.len_lines().saturating_sub(shown));
        let row = (((my - box_top) / line_height).floor() as isize - 1).max(0) as usize;
        let line = first + row;
        let column = (((mx - 16.0 - char_width * 2.0) / char_width)
            .round()
            .max(0.0)) as usize;
        self.with_notebook(|session| {
            session.focus_input(true);
            session.edit_input(|editor| {
                let line = line.min(editor.len_lines().saturating_sub(1));
                editor.set_cursor_line_col(line, column);
            });
        });
        true
    }
}
