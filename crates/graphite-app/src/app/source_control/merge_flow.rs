//! Opening, closing and driving the merge editor.

use super::merge_editor::{BasePlacement, MergeEditor, MergeFormat, MergeLayout};
use super::*;

impl App {
    /// Window size and line height used to place the merge panes; a fixed size without a window.
    fn merge_window(&self) -> (f32, f32, f32) {
        self.gpu.as_ref().map_or((1280.0, 720.0, 20.0), |gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height, gpu.renderer.line_height())
        })
    }

    /// Text and on-disk format of a file for merging: the open document when there is one,
    /// otherwise the disk.
    fn merge_source(&self, path: &Path) -> Result<(String, MergeFormat), String> {
        if let Some(index) = self.documents.id_for_path(path) {
            let document = &self.documents[index];
            return Ok((document.buffer.text(), document.saved_format));
        }
        crate::file_format::read(path, &self.translator)
            .map(|decoded| (decoded.text, (decoded.encoding, decoded.line_ending)))
    }

    /// Opens the merge editor on a file that has conflict markers.
    pub(crate) fn open_merge_editor(&mut self, path: PathBuf) -> Result<(), String> {
        let (text, format) = self.merge_source(&path)?;
        let mut editor = MergeEditor::new(path.clone(), &text);
        if editor.state.is_empty() {
            return Err(self.t("merge.error.no_conflicts").into());
        }
        if editor
            .state
            .parsed()
            .conflicts()
            .any(|block| block.base.is_none())
        {
            if let Some(ancestor) = git_stage1_base(&path) {
                editor.set_ancestor(ancestor);
            }
        }
        editor.format = format;
        let count = editor.state.len();
        self.merge_editor = Some(editor);
        self.status = self.tf("merge.opened", &[("count", &count)]);
        Ok(())
    }

    /// Closes the merge editor. Decisions that were not written to the file are lost, so the
    /// first request with decisions pending only warns; asking again discards them.
    pub(crate) fn close_merge_editor(&mut self) {
        let Some(editor) = self.merge_editor.as_mut() else {
            return;
        };
        let decided = editor.state.len() - editor.state.unresolved_count();
        if decided > 0 && !editor.close_armed {
            editor.close_armed = true;
            self.status = self.tf("merge.close_warning", &[("count", &decided)]);
            return;
        }
        self.merge_editor = None;
        self.status = self.t("merge.closed").into();
    }

    /// Forgets the warning of a close request, so closing warns again.
    pub(crate) fn merge_reset_close_choice(&mut self) {
        if let Some(editor) = self.merge_editor.as_mut() {
            editor.close_armed = false;
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
        let Ok((text, _)) = self.merge_source(path) else {
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
                && !editor
                    .state
                    .parsed()
                    .conflicts()
                    .any(|block| block.base.is_some())
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
        self.merge_scroll(
            if forward { window } else { -window },
            width,
            height,
            line_height,
        );
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
            Key::Named(NamedKey::Enter) => self.run_ui_command("mergeEditor.acceptMerge"),
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
                    "c" => self.run_ui_command("graphite.merge.acceptCurrent"),
                    "i" => self.run_ui_command("graphite.merge.acceptIncoming"),
                    "b" if self.modifiers.shift_key() => {
                        self.run_ui_command("graphite.merge.acceptBothIncomingFirst")
                    }
                    "b" => self.run_ui_command("graphite.merge.acceptBoth"),
                    "n" => self.run_ui_command("graphite.merge.ignore"),
                    "r" => self.run_ui_command("graphite.merge.revert"),
                    "o" => self.run_ui_command("merge.openResult"),
                    "1" => self.run_ui_command("merge.toggleActiveConflictInput1"),
                    "2" => self.run_ui_command("merge.toggleActiveConflictInput2"),
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

/// Reads the common ancestor (index stage 1) for `path` from Git when 2-way conflict markers
/// lack an embedded `|||||||` base section.
fn git_stage1_base(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let top = std::process::Command::new("git")
        .arg("-C")
        .arg(parent)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !top.status.success() {
        return None;
    }
    let root = PathBuf::from(String::from_utf8_lossy(&top.stdout).trim());
    let canon_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let canon_file = std::fs::canonicalize(path).ok()?;
    let rel = canon_file
        .strip_prefix(&canon_root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let show = std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("show")
        .arg(format!(":1:{rel}"))
        .output()
        .ok()?;
    if !show.status.success() {
        return None;
    }
    String::from_utf8(show.stdout).ok()
}
