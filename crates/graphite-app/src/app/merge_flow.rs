//! Opening, closing and driving the merge editor.

use super::merge_editor::{BasePlacement, MergeEditor, MergeLayout};
use super::*;

impl App {
    /// Window size and line height used to place the merge panes; a fixed size without a window.
    fn merge_window(&self) -> (f32, f32, f32) {
        self.gpu.as_ref().map_or((1280.0, 720.0, 20.0), |gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height, gpu.renderer.line_height())
        })
    }

    /// Text of a file for merging: the open document when there is one, otherwise the disk.
    fn merge_source_text(&self, path: &Path) -> Result<String, String> {
        if let Some(index) = self.documents.id_for_path(path) {
            return Ok(self.documents[index].buffer.text());
        }
        crate::file_format::read(path, &self.translator)
            .map(|decoded| decoded.text)
            .map_err(|error| error.to_string())
    }

    /// Opens the merge editor on a file that has conflict markers.
    pub(crate) fn open_merge_editor(&mut self, path: PathBuf) -> Result<(), String> {
        let text = self.merge_source_text(&path)?;
        let editor = MergeEditor::new(path, &text);
        if editor.state.is_empty() {
            return Err(self.t("merge.error.no_conflicts").into());
        }
        let count = editor.state.len();
        self.merge_editor = Some(editor);
        self.status = self.tf("merge.opened", &[("count", &count)]);
        Ok(())
    }

    pub(crate) fn close_merge_editor(&mut self) {
        if self.merge_editor.take().is_some() {
            self.status = self.t("merge.closed").into();
        }
    }

    /// Re-reads the file after it was saved, so edits made by hand show up in the panes.
    pub(crate) fn reload_merge_editor_for(&mut self, path: &Path) {
        if !self
            .merge_editor
            .as_ref()
            .is_some_and(|editor| crate::same_file_path(&editor.path, path))
        {
            return;
        }
        let Ok(text) = self.merge_source_text(path) else {
            return;
        };
        if let Some(editor) = self.merge_editor.as_mut() {
            editor.reload(&text);
        }
    }

    pub(crate) fn merge_set_layout(&mut self, layout: MergeLayout) {
        if let Some(editor) = self.merge_editor.as_mut() {
            editor.layout = layout;
        }
    }

    /// Shows the base pane in `placement`, or hides it when it is already there.
    pub(crate) fn merge_toggle_base(&mut self, placement: BasePlacement) {
        if let Some(editor) = self.merge_editor.as_mut() {
            editor.base = if editor.base == placement {
                BasePlacement::Hidden
            } else {
                placement
            };
            if editor.base != BasePlacement::Hidden
                && !editor.state.parsed().conflicts().any(|block| block.base.is_some())
            {
                self.status = self.t("merge.no_base").into();
            }
        }
    }

    pub(crate) fn merge_toggle_unchanged(&mut self) {
        if let Some(editor) = self.merge_editor.as_mut() {
            editor.hide_unchanged = !editor.hide_unchanged;
            editor.refresh();
            editor.scroll_to_active();
        }
    }

    pub(crate) fn merge_goto(&mut self, forward: bool) {
        let found = self.merge_editor.as_mut().is_some_and(|editor| {
            if forward {
                editor.goto_next_unresolved()
            } else {
                editor.goto_previous_unresolved()
            }
        });
        if !found {
            self.status = self.t("merge.none_left").into();
        }
    }

    pub(crate) fn merge_scroll_rows(&mut self, rows: isize) {
        let (width, height, line_height) = self.merge_window();
        self.merge_scroll(rows, width, height, line_height);
    }

    /// One page of rows, for the page keys.
    pub(crate) fn merge_scroll_page(&mut self, forward: bool) {
        let (width, height, line_height) = self.merge_window();
        let window = self
            .merge_geometry(width, height, line_height)
            .map_or(1, |(_, window)| window) as isize;
        self.merge_scroll(if forward { window } else { -window }, width, height, line_height);
    }

    pub(crate) fn merge_handle_click(&mut self, x: f32, y: f32) {
        let (width, height, line_height) = self.merge_window();
        self.merge_click(x, y, width, height, line_height);
    }
}

impl App {
    /// Keys of the merge editor. Every key runs the command that does the same from the palette.
    pub(crate) fn handle_merge_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => self.run_ui_command("graphite.merge.close"),
            Key::Named(NamedKey::ArrowDown) => {
                self.run_ui_command("merge.goToNextUnhandledConflict")
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.run_ui_command("merge.goToPreviousUnhandledConflict")
            }
            Key::Named(NamedKey::PageDown) => self.merge_scroll_page(true),
            Key::Named(NamedKey::PageUp) => self.merge_scroll_page(false),
            _ => {
                let key = event.text.as_deref().unwrap_or("").to_ascii_lowercase();
                match key.as_str() {
                    "l" => {
                        let column = self
                            .merge_editor
                            .as_ref()
                            .is_some_and(|editor| editor.layout == MergeLayout::Column);
                        self.run_ui_command(if column {
                            "merge.mixedLayout"
                        } else {
                            "merge.columnLayout"
                        });
                    }
                    "s" => self.run_ui_command("merge.showBase"),
                    "h" => self.run_ui_command("merge.showNonConflictingChanges"),
                    _ => {}
                }
            }
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }
}
