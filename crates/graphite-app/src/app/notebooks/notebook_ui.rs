//! Notebook documents in the editor: keeping the session and the document text in step, the keys
//! and the mouse.

use super::clipboard::{copy_or_cut, paste, SystemClipboard};
use super::*;
use crate::notebook::{self, CellKind, Mode, Notebook, Session};

/// Turns the document into a notebook when it is the text of a valid `.ipynb` file. The document
/// text becomes the notebook as this program writes it, which counts as the saved text, so a file
/// formatted differently is not reported as changed just for being opened.
pub(crate) fn attach_notebook(tab: &mut Document) {
    let is_notebook = tab
        .buffer
        .path()
        .and_then(|path| path.extension())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ipynb"));
    if !is_notebook {
        return;
    }
    if let Ok(parsed) = Notebook::parse(&tab.buffer.text()) {
        let session = Session::from_notebook(parsed);
        let canonical = session.to_json();
        tab.buffer.set_derived_text(&canonical);
        tab.buffer.mark_saved_snapshot(&canonical);
        let mut session = session;
        session.synced_revision = tab.buffer.revision();
        tab.notebook = Some(Box::new(session));
    }
}

impl App {
    /// The notebook of the active editor, if it shows one.
    pub(crate) fn active_notebook(&self) -> Option<&Session> {
        self.documents[self.active_document()].notebook.as_deref()
    }

    /// Re-reads the document text into the session when the text changed behind its back (the
    /// file was reloaded from disk).
    pub(crate) fn refresh_notebook(&mut self, document: DocumentId) {
        let tab = &mut self.documents[document];
        let Some(session) = tab.notebook.as_mut() else {
            return;
        };
        if session.synced_revision == tab.buffer.revision() {
            return;
        }
        session.synced_revision = tab.buffer.revision();
        if let Ok(parsed) = Notebook::parse(&tab.buffer.text()) {
            // Only adopt the text when it differs from what the session would write, so an echo
            // of our own save does not reset the editing state.
            if parsed != session.export() {
                session.reload(parsed);
            }
            // A clean document shows the notebook the way this program writes it.
            if !tab.buffer.is_dirty() {
                let canonical = session.to_json();
                tab.buffer.set_derived_text(&canonical);
                tab.buffer.mark_saved_snapshot(&canonical);
                session.synced_revision = tab.buffer.revision();
            }
        }
    }

    /// Writes the session into the document text, which makes the document dirty when it
    /// differs from the file.
    pub(crate) fn sync_notebook_buffer(&mut self, document: DocumentId) {
        let tab = &mut self.documents[document];
        let Some(session) = tab.notebook.as_mut() else {
            return;
        };
        let text = session.to_json();
        tab.buffer.set_derived_text(&text);
        // The history of an interactive window that was never saved is scratch: closing it does
        // not ask what to do with it.
        if session.interactive().is_some() && tab.buffer.path().is_none() {
            tab.buffer.mark_saved_snapshot(&text);
        }
        session.synced_revision = tab.buffer.revision();
    }

    /// Height available to the cells of the focused editor group, line height and cell width.
    pub(crate) fn notebook_viewport(&self) -> (f32, f32, f32) {
        let Some(gpu) = self.gpu.as_ref() else {
            return (600.0, 20.0, 9.0);
        };
        let (width, height) = gpu.renderer.size();
        let (lh, cw) = (gpu.renderer.line_height(), gpu.renderer.char_width());
        let focused = self.editor_groups.focused();
        let rect = self
            .group_rectangles(width, height, lh)
            .into_iter()
            .find(|rect| rect.id == focused);
        let body = rect.map_or(height, |rect| rect.height) - self.editor_tab_strip_height(lh);
        let docked = self.interactive_input_height(lh);
        ((body - lh - docked).max(lh * 3.0), lh, cw)
    }

    /// Runs a change on the active notebook, then keeps the document text and the scroll in step.
    pub(crate) fn with_notebook(&mut self, change: impl FnOnce(&mut Session)) {
        let document = self.active_document();
        let (viewport, line_height, _) = self.notebook_viewport();
        self.refresh_notebook(document);
        let Some(session) = self.documents[document].notebook.as_mut() else {
            return;
        };
        change(session);
        let layouts = notebook::layout(session, line_height);
        let focus = session.focus;
        session.scroll = notebook::scroll_to_show(&layouts, focus, session.scroll, viewport);
        session.scroll = notebook::clamp_scroll(&layouts, session.scroll, viewport);
        self.sync_notebook_buffer(document);
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    /// Whether the active editor is a notebook, for the context keys.
    pub(crate) fn notebook_context(&self) -> (bool, bool) {
        match self.active_notebook() {
            Some(session) => (true, session.mode == Mode::Edit),
            None => (false, false),
        }
    }

    /// Keys of a notebook: cell commands in command mode, text editing in edit mode.
    pub(crate) fn handle_notebook_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        if self.interactive_context().1 {
            self.handle_interactive_key(event);
            return;
        }
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        let alt = self.modifiers.alt_key();
        let editing = self.notebook_context().1;
        let character = event.text.as_deref().unwrap_or("").to_owned();
        let lower = character.to_ascii_lowercase();
        // Undo and redo span text and cells in both modes.
        if ctrl && matches!(lower.as_str(), "z" | "y") {
            let redo = lower == "y" || shift;
            self.run_ui_command(if redo {
                "notebook.redo"
            } else {
                "notebook.undo"
            });
            return;
        }
        if editing {
            self.handle_notebook_edit_key(event, ctrl, shift, alt, &character);
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Enter) if shift => {
                self.run_ui_command("notebook.cell.executeAndSelectBelow")
            }
            Key::Named(NamedKey::Enter) if ctrl => self.run_ui_command("notebook.cell.execute"),
            Key::Named(NamedKey::Enter) if alt => {
                self.run_ui_command("notebook.cell.executeAndInsertBelow")
            }
            Key::Named(NamedKey::Enter) => self.run_ui_command("notebook.cell.edit"),
            Key::Named(NamedKey::ArrowDown) if shift => {
                self.with_notebook(|session| session.focus_by(1, true))
            }
            Key::Named(NamedKey::ArrowUp) if shift => {
                self.with_notebook(|session| session.focus_by(-1, true))
            }
            Key::Named(NamedKey::ArrowDown) if ctrl && shift => {
                self.run_ui_command("notebook.cell.moveDown")
            }
            Key::Named(NamedKey::ArrowDown) => self.run_ui_command("notebook.focusNextEditor"),
            Key::Named(NamedKey::ArrowUp) => self.run_ui_command("notebook.focusPreviousEditor"),
            Key::Named(NamedKey::Home) => self.with_notebook(|session| session.set_focus(0, false)),
            Key::Named(NamedKey::End) => {
                self.with_notebook(|session| session.set_focus(usize::MAX, false))
            }
            Key::Named(NamedKey::Delete) => self.run_ui_command("notebook.cell.delete"),
            _ if ctrl => match lower.as_str() {
                "a" => self.run_ui_command("graphite.notebook.selectAllCells"),
                "c" => self.run_ui_command("notebook.cell.copy"),
                "x" => self.run_ui_command("notebook.cell.cut"),
                "v" => self.run_ui_command("notebook.cell.paste"),
                _ => {}
            },
            _ if alt => {}
            _ => match lower.as_str() {
                "j" => self.run_ui_command("notebook.focusNextEditor"),
                "k" => self.run_ui_command("notebook.focusPreviousEditor"),
                "a" => self.run_ui_command("notebook.cell.insertCodeCellAbove"),
                "b" => self.run_ui_command("notebook.cell.insertCodeCellBelow"),
                "m" if shift => self.run_ui_command("notebook.cell.joinBelow"),
                "m" => self.run_ui_command("notebook.cell.changeToMarkdown"),
                "y" => self.run_ui_command("notebook.cell.changeToCode"),
                "r" => self.run_ui_command("notebook.cell.changeToRaw"),
                "x" => self.run_ui_command("notebook.cell.cut"),
                "c" => self.run_ui_command("notebook.cell.copy"),
                "v" => self.run_ui_command("notebook.cell.paste"),
                "z" => self.run_ui_command("notebook.undo"),
                "o" => self.run_ui_command("notebook.cell.toggleOutputs"),
                "h" => self.run_ui_command("graphite.notebook.toggleCellInput"),
                "l" => self.run_ui_command("notebook.toggleLineNumbers"),
                "d" => {
                    let pending = self.active_notebook().is_some_and(|s| s.delete_pending);
                    if pending {
                        self.run_ui_command("notebook.cell.delete");
                    } else {
                        self.with_notebook(|session| session.delete_pending = true);
                    }
                }
                _ => {}
            },
        }
    }

    fn handle_notebook_edit_key(
        &mut self,
        event: &winit::event::KeyEvent,
        ctrl: bool,
        shift: bool,
        alt: bool,
        character: &str,
    ) {
        use winit::keyboard::{Key, NamedKey};
        let tab_text = " ".repeat(self.tab_size.max(1));
        let lower = character.to_ascii_lowercase();
        // Moves the caret, extending the selection when Shift is held.
        macro_rules! caret {
            ($method:ident) => {
                self.with_notebook(|session| {
                    session.edit(|editor| {
                        let anchor = editor
                            .selection()
                            .map(|selection| selection.anchor)
                            .unwrap_or(editor.cursor().char_idx);
                        editor.$method();
                        if shift {
                            let head = editor.cursor().char_idx;
                            editor.set_selection(anchor, head);
                        }
                    })
                })
            };
        }
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => self.run_ui_command("notebook.cell.quitEdit"),
            Key::Named(NamedKey::Enter) if shift => {
                self.run_ui_command("notebook.cell.quitEdit");
                self.run_ui_command("notebook.cell.executeAndSelectBelow");
            }
            Key::Named(NamedKey::Enter) if ctrl => {
                self.run_ui_command("notebook.cell.quitEdit");
                self.run_ui_command("notebook.cell.execute");
            }
            Key::Named(NamedKey::Enter) if alt => {
                self.run_ui_command("notebook.cell.executeAndInsertBelow")
            }
            Key::Named(NamedKey::Enter) => {
                self.with_notebook(|session| session.edit(|editor| editor.insert_newline()))
            }
            Key::Named(NamedKey::Tab) => {
                self.with_notebook(|session| session.edit(|editor| editor.insert(&tab_text)))
            }
            Key::Named(NamedKey::Backspace) => {
                self.with_notebook(|session| session.edit(|editor| editor.backspace()))
            }
            Key::Named(NamedKey::Delete) => {
                self.with_notebook(|session| session.edit(|editor| editor.delete()))
            }
            Key::Named(NamedKey::ArrowLeft) => caret!(move_left),
            Key::Named(NamedKey::ArrowRight) => caret!(move_right),
            Key::Named(NamedKey::Home) => caret!(move_home),
            Key::Named(NamedKey::End) => caret!(move_end),
            Key::Named(NamedKey::ArrowUp) => self.with_notebook(|session| {
                session.move_vertical(-1, false);
            }),
            Key::Named(NamedKey::ArrowDown) => self.with_notebook(|session| {
                session.move_vertical(1, false);
            }),
            _ if ctrl && lower == "a" => self.with_notebook(|session| {
                session.edit(|editor| {
                    let end = editor.len_chars();
                    editor.set_selection(0, end);
                })
            }),
            _ if ctrl && matches!(lower.as_str(), "c" | "x") => {
                let cut = lower == "x";
                self.with_notebook(|session| {
                    session.edit(|editor| {
                        copy_or_cut(editor, &mut SystemClipboard, cut);
                    })
                })
            }
            _ if ctrl && lower == "v" => self.with_notebook(|session| {
                session.edit(|editor| {
                    paste(editor, &mut SystemClipboard);
                })
            }),
            _ if ctrl && shift && (character == "_" || character == "-") => {
                self.run_ui_command("notebook.cell.split")
            }
            _ if !ctrl
                && !alt
                && !character.is_empty()
                && !character.chars().any(char::is_control) =>
            {
                let text = character.to_owned();
                self.with_notebook(|session| session.edit(|editor| editor.insert(&text)));
            }
            _ => {}
        }
    }

    /// A click inside a notebook: selects the cell, and a click on its input starts editing at
    /// that place.
    pub(crate) fn notebook_click(
        &mut self,
        mx: f32,
        my: f32,
        rect: super::editor_groups::GroupRect,
    ) {
        let document = self.active_document();
        let (_, line_height, char_width) = self.notebook_viewport();
        let top = rect.y + self.editor_tab_strip_height(line_height) + line_height;
        // In an interactive window the input box is docked under the cells.
        if self
            .active_notebook()
            .and_then(Session::interactive)
            .is_some()
        {
            let (viewport, _, _) = self.notebook_viewport();
            let box_top = top + viewport;
            if self.interactive_click(mx, my, box_top) {
                return;
            }
        }
        let Some(session) = self.documents[document].notebook.as_ref() else {
            return;
        };
        let layouts = notebook::layout(session, line_height);
        let y = my - top + session.scroll;
        let Some(index) = notebook::cell_at(&layouts, y) else {
            return;
        };
        let cell = layouts[index];
        let in_input = y >= cell.input_top(line_height) && y < cell.output_top(line_height);
        let line = ((y - cell.input_top(line_height)) / line_height)
            .floor()
            .max(0.0) as usize;
        let gutter = if session.line_numbers {
            char_width * 5.0
        } else {
            0.0
        };
        let column = (((mx - rect.x - 16.0 - gutter) / char_width)
            .round()
            .max(0.0)) as usize;
        self.with_notebook(|session| {
            session.set_focus(index, false);
            if in_input {
                session.enter_edit();
                session.edit(|editor| {
                    let line = line.min(editor.len_lines().saturating_sub(1));
                    editor.set_cursor_line_col(line, column);
                });
            } else {
                session.exit_edit();
            }
        });
    }

    /// The mouse wheel over a notebook.
    pub(crate) fn notebook_scroll(&mut self, lines: f32) {
        let document = self.active_document();
        let (viewport, line_height, _) = self.notebook_viewport();
        let Some(session) = self.documents[document].notebook.as_mut() else {
            return;
        };
        let layouts = notebook::layout(session, line_height);
        session.scroll =
            notebook::clamp_scroll(&layouts, session.scroll + lines * line_height, viewport);
    }

    pub(crate) fn notebook_insert(&mut self, kind: CellKind, below: bool) {
        self.with_notebook(|session| {
            if below {
                session.insert_below(kind);
            } else {
                session.insert_above(kind);
            }
            session.enter_edit();
        });
    }
}

impl App {
    /// Height of the input box docked under the cells of an interactive window; none for a
    /// notebook.
    pub(crate) fn interactive_input_height(&self, line_height: f32) -> f32 {
        self.active_notebook()
            .and_then(Session::interactive)
            .map_or(0.0, |state| (state.input_lines() + 1) as f32 * line_height)
    }
}
