//! File operations in the explorer: select, rename, move (by rename or drag and drop), delete.

use super::*;
use std::fs;
use winit::keyboard::{Key, NamedKey};

/// What the explorer prompt is asking for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExplorerPromptKind {
    /// A new name for `path`.
    Rename,
    /// The name of a file to create inside the folder `path`.
    NewFile,
    /// The name of a folder to create inside the folder `path`.
    NewFolder,
}

/// Name typed into the explorer prompt. A value with folders moves the entry (rename) or creates
/// the missing folders (new file or folder).
#[derive(Clone, Debug)]
pub(crate) struct ExplorerPrompt {
    pub(crate) path: PathBuf,
    pub(crate) value: String,
    pub(crate) kind: ExplorerPromptKind,
}

/// Moves `from` to `to`, creating missing parent folders. Never overwrites an existing entry.
pub(crate) fn move_entry(from: &Path, to: &Path) -> Result<(), String> {
    if to.starts_with(from) {
        return Err("a folder cannot be moved into itself".into());
    }
    if fs::symlink_metadata(to).is_ok() {
        return Err(format!("{} already exists", to.display()));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::rename(from, to).map_err(|error| error.to_string())
}

/// Deletes a file or a folder with its contents.
pub(crate) fn remove_entry(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|error| error.to_string())
}

fn unique_copy_destination(desired: &Path) -> PathBuf {
    if fs::symlink_metadata(desired).is_err() {
        return desired.to_path_buf();
    }
    let parent = desired.parent().unwrap_or_else(|| Path::new(""));
    let stem = desired
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("copy");
    let ext = desired.extension().and_then(|s| s.to_str());
    for index in 1..1000 {
        let suffix = if index == 1 {
            " copy".to_string()
        } else {
            format!(" copy {index}")
        };
        let candidate_name = match ext {
            Some(ext) if !desired.is_dir() => format!("{stem}{suffix}.{ext}"),
            _ => format!("{stem}{suffix}"),
        };
        let candidate = parent.join(candidate_name);
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    desired.to_path_buf()
}

fn copy_entry_recursive(from: &Path, to: &Path) -> Result<(), String> {
    if to.starts_with(from) && from.is_dir() {
        return Err("a folder cannot be copied into itself".into());
    }
    let meta = fs::symlink_metadata(from).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        fs::create_dir_all(to).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_entry_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(from, to).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl App {
    pub(crate) fn explorer_select(&mut self, path: PathBuf) {
        self.explorer_selected = Some(path);
        self.explorer_focused = true;
    }

    pub(crate) fn explorer_has_selection(&self) -> bool {
        self.explorer_focused && self.explorer_selected.is_some()
    }

    /// Marks the selected explorer entry to be moved on the next paste.
    pub(crate) fn explorer_cut(&mut self) {
        if let Some(path) = self.explorer_selected.clone() {
            self.explorer_clipboard = Some((path, true));
        }
    }

    /// Marks the selected explorer entry to be copied on the next paste.
    pub(crate) fn explorer_copy_entry(&mut self) {
        if let Some(path) = self.explorer_selected.clone() {
            self.explorer_clipboard = Some((path, false));
        }
    }

    /// Pastes the staged explorer entry into the selected folder (or parent folder of a selected file).
    pub(crate) fn explorer_paste(&mut self) -> Result<(), String> {
        let Some((source, is_cut)) = self.explorer_clipboard.clone() else {
            return Ok(());
        };
        if !source.exists() {
            self.explorer_clipboard = None;
            return Err("source entry no longer exists".into());
        }
        let target_dir = match self.explorer_selected.as_deref() {
            Some(sel) if sel.is_dir() => sel.to_path_buf(),
            Some(sel) => sel
                .parent()
                .unwrap_or_else(|| self.tree.root())
                .to_path_buf(),
            None => self.tree.root().to_path_buf(),
        };
        let file_name = source
            .file_name()
            .ok_or_else(|| "cannot paste root entry".to_owned())?;
        let raw_dest = target_dir.join(file_name);
        if is_cut {
            if raw_dest == source {
                self.explorer_clipboard = None;
                return Ok(());
            }
            move_entry(&source, &raw_dest)?;
            self.explorer_clipboard = None;
            self.explorer_selected = Some(raw_dest);
        } else {
            let dest = unique_copy_destination(&raw_dest);
            copy_entry_recursive(&source, &dest)?;
            self.explorer_selected = Some(dest);
        }
        self.tree.refresh();
        Ok(())
    }

    /// Opens the selected explorer entry (or workspace root) in a new detached Graphite window.
    pub(crate) fn explorer_open_in_new_window(&mut self) -> Result<(), String> {
        let target = self
            .explorer_selected
            .clone()
            .unwrap_or_else(|| self.tree.root().to_path_buf());
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(exe)
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Opens the rename prompt for the selected explorer entry.
    pub(crate) fn explorer_rename(&mut self) {
        let Some(path) = self.explorer_selected.clone() else {
            return;
        };
        let value = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.explorer_prompt = Some(ExplorerPrompt {
            path,
            value,
            kind: ExplorerPromptKind::Rename,
        });
        self.status = self.t("explorer.rename.hint").into();
    }

    /// Reads the workspace folders from disk again.
    pub(crate) fn refresh_explorer(&mut self) {
        self.tree.refresh();
        self.status = self.t("explorer.refreshed").into();
    }

    /// Opens the selected file as a preview and leaves the keyboard focus in the explorer.
    pub(crate) fn explorer_open_preserving_focus(&mut self) {
        let Some(path) = self.explorer_selected.clone().filter(|path| path.is_file()) else {
            return;
        };
        self.open_file_preview(path);
        self.explorer_focused = true;
    }

    /// Asks for the name of a new file or folder, created inside the selected folder (the
    /// folder of the selected file, or the workspace folder when nothing is selected).
    pub(crate) fn explorer_new_entry(&mut self, folder: bool) {
        let parent = match self.explorer_selected.clone() {
            Some(selected) if selected.is_dir() => selected,
            Some(selected) => selected
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.tree.root().to_path_buf()),
            None => self.tree.root().to_path_buf(),
        };
        self.tree.set_expanded(&parent, true);
        self.tree.refresh();
        self.explorer_prompt = Some(ExplorerPrompt {
            path: parent,
            value: String::new(),
            kind: if folder {
                ExplorerPromptKind::NewFolder
            } else {
                ExplorerPromptKind::NewFile
            },
        });
        self.status = self.t("explorer.new.hint").into();
    }

    fn apply_explorer_creation(&mut self, prompt: ExplorerPrompt) {
        let Some(relative) = Self::checked_relative_name(prompt.value.trim()) else {
            self.status = self.t("explorer.rename.invalid").into();
            return;
        };
        let target = prompt.path.join(relative);
        if target.exists() {
            self.status = self.t("explorer.new.exists").into();
            return;
        }
        let created = if prompt.kind == ExplorerPromptKind::NewFolder {
            fs::create_dir_all(&target)
        } else {
            target
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| fs::File::create_new(&target).map(drop))
        };
        if let Err(error) = created {
            self.status = error.to_string();
            return;
        }
        self.tree.set_expanded(&prompt.path, true);
        self.tree.refresh();
        self.explorer_select(target.clone());
        if prompt.kind == ExplorerPromptKind::NewFile {
            self.open_file(target);
        }
        self.status = self.t("explorer.new.done").into();
    }

    /// The name as a relative path, or `None` when it is empty, absolute or climbs out of the
    /// folder.
    fn checked_relative_name(value: &str) -> Option<&Path> {
        let relative = Path::new(value);
        let escapes = relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        });
        (!(value.is_empty() || relative.is_absolute() || escapes)).then_some(relative)
    }

    /// Routes keys to the explorer rename prompt while it is open.
    pub(crate) fn handle_explorer_prompt_key(&mut self, key: &Key, text: Option<&str>) -> bool {
        let Some(prompt) = self.explorer_prompt.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => self.explorer_prompt = None,
            Key::Named(NamedKey::Enter) => {
                if let Some(prompt) = self.explorer_prompt.take() {
                    if prompt.kind == ExplorerPromptKind::Rename {
                        self.apply_explorer_rename(prompt);
                    } else {
                        self.apply_explorer_creation(prompt);
                    }
                }
            }
            Key::Named(NamedKey::Backspace) => {
                prompt.value.pop();
            }
            _ => {
                if let Some(text) = text.filter(|text| !text.chars().any(char::is_control)) {
                    prompt.value.push_str(text);
                }
            }
        }
        true
    }

    fn apply_explorer_rename(&mut self, prompt: ExplorerPrompt) {
        let value = prompt.value.trim();
        // Relative names only: no absolute paths and no `..` that could leave the folder.
        let relative = Path::new(value);
        let escapes = relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        });
        if value.is_empty() || relative.is_absolute() || escapes {
            self.status = self.t("explorer.rename.invalid").into();
            return;
        }
        let Some(parent) = prompt.path.parent() else {
            return;
        };
        let to = parent.join(value);
        if to == prompt.path {
            return;
        }
        match self.move_explorer_entry(&prompt.path, &to) {
            Ok(()) => self.status = self.t("explorer.rename.done").into(),
            Err(error) => self.status = error,
        }
    }

    /// Moves an entry on disk, then keeps the open documents, the selection and the tree in step.
    pub(crate) fn move_explorer_entry(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        let dirty = self.documents.iter().any(|tab| {
            tab.is_dirty() && tab.buffer.path().is_some_and(|open| open.starts_with(from))
        });
        if dirty {
            return Err(self.t("explorer.unsaved_changes").into());
        }
        move_entry(from, to)?;
        for (_, document) in self.documents.iter_mut() {
            let Some(open) = document.buffer.path().map(Path::to_path_buf) else {
                continue;
            };
            if let Ok(rest) = open.strip_prefix(from) {
                let target = to.join(rest);
                document.buffer.set_path(target);
            }
        }
        if self.explorer_selected.as_deref() == Some(from) {
            self.explorer_selected = Some(to.to_path_buf());
        }
        self.tree.refresh();
        Ok(())
    }

    /// Asks before deleting the selected entry using a non-blocking in-app confirmation modal.
    pub(crate) fn explorer_delete(&mut self) {
        let Some(path) = self.explorer_selected.clone() else {
            return;
        };
        let norm_path = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self
            .tree
            .roots()
            .iter()
            .any(|root| std::fs::canonicalize(root).unwrap_or_else(|_| root.clone()) == norm_path)
        {
            return;
        }
        self.explorer_delete_confirm = Some(path);
    }

    pub(crate) fn confirm_explorer_delete(&mut self) {
        let Some(path) = self.explorer_delete_confirm.take() else {
            return;
        };
        match self.remove_explorer_entry(&path) {
            Ok(()) => self.status = self.t("explorer.delete.done").into(),
            Err(error) => self.status = error,
        }
    }

    pub(crate) fn handle_explorer_delete_key(&mut self, key: &Key) -> bool {
        if self.explorer_delete_confirm.is_none() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Enter) => self.confirm_explorer_delete(),
            Key::Named(NamedKey::Escape) => self.explorer_delete_confirm = None,
            _ => {}
        }
        true
    }

    fn explorer_delete_modal_rects(
        width: f32,
        height: f32,
    ) -> (
        (f32, f32, f32, f32),
        (f32, f32, f32, f32),
        (f32, f32, f32, f32),
    ) {
        let panel_width = (width * 0.6)
            .clamp(340.0, 520.0)
            .min((width - 24.0).max(0.0));
        let panel_height = 128.0;
        let x = ((width - panel_width) * 0.5).max(12.0);
        let y = ((height - panel_height) * 0.5).max(12.0);
        let btn_w = 104.0;
        let btn_h = 28.0;
        let btn_y = y + panel_height - btn_h - 16.0;
        let cancel_x = x + panel_width - btn_w - 16.0;
        let confirm_x = cancel_x - btn_w - 10.0;
        (
            (x, y, panel_width, panel_height),
            (confirm_x, btn_y, btn_w, btn_h),
            (cancel_x, btn_y, btn_w, btn_h),
        )
    }

    pub(crate) fn click_explorer_delete_confirm(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
    ) -> bool {
        if self.explorer_delete_confirm.is_none() {
            return false;
        }
        let (panel, confirm, cancel) = Self::explorer_delete_modal_rects(width, height);
        let in_rect =
            |r: (f32, f32, f32, f32)| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3;
        if in_rect(confirm) {
            self.confirm_explorer_delete();
        } else if in_rect(cancel) || !in_rect(panel) {
            self.explorer_delete_confirm = None;
        }
        true
    }

    pub(crate) fn draw_explorer_delete_confirm(&self, renderer: &mut Renderer) {
        let Some(path) = &self.explorer_delete_confirm else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let ((x, y, panel_width, panel_height), confirm, cancel) =
            Self::explorer_delete_modal_rects(width, height);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let danger_bg = rgb(0xb42318);
        let cancel_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let btn_fg = rgb(0xffffff);

        renderer.draw_rect(x, y, panel_width, panel_height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_width - 2.0).max(0.0),
            (panel_height - 2.0).max(0.0),
            bg,
        );
        renderer.draw_text(self.t("explorer.delete.title"), x + 16.0, y + 14.0, text);

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let message = self.tf("explorer.delete.confirm", &[("name", &name)]);
        let clipped = Self::help_menu_visible_label(&message, cw, (panel_width - 32.0).max(0.0));
        renderer.draw_text(&clipped, x + 16.0, y + 44.0, dim);

        renderer.draw_rect(confirm.0, confirm.1, confirm.2, confirm.3, danger_bg);
        let confirm_label = self.t("explorer.delete.title");
        let confirm_tx =
            confirm.0 + (confirm.2 - confirm_label.chars().count() as f32 * cw).max(0.0) * 0.5;
        renderer.draw_text(confirm_label, confirm_tx, confirm.1 + 5.0, btn_fg);

        renderer.draw_rect(cancel.0, cancel.1, cancel.2, cancel.3, cancel_bg);
        let cancel_label = self.t("explorer.delete.cancel");
        let cancel_tx =
            cancel.0 + (cancel.2 - cancel_label.chars().count() as f32 * cw).max(0.0) * 0.5;
        renderer.draw_text(cancel_label, cancel_tx, cancel.1 + 5.0, text);
    }

    /// Deletes an entry on disk after the user confirmed it.
    pub(crate) fn remove_explorer_entry(&mut self, path: &Path) -> Result<(), String> {
        let dirty = self.documents.iter().any(|tab| {
            tab.is_dirty() && tab.buffer.path().is_some_and(|open| open.starts_with(path))
        });
        if dirty {
            return Err(self.t("explorer.unsaved_changes").into());
        }
        remove_entry(path)?;
        if self
            .explorer_selected
            .as_deref()
            .is_some_and(|selected| selected.starts_with(path))
        {
            self.explorer_selected = None;
        }
        self.tree.refresh();
        Ok(())
    }

    /// Explorer row under the mouse, using the same geometry as the explorer click handler.
    pub(crate) fn explorer_row_at_mouse(&self) -> Option<usize> {
        let lh = self.gpu.as_ref()?.renderer.line_height();
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        if mx < 0.0 || mx >= self.sidebar_layout_width_px() {
            return None;
        }
        let sidebar_lh = lh;
        let first_row_y = sidebar_lh * 1.5;
        (my >= first_row_y).then(|| ((my - first_row_y) / sidebar_lh) as usize)
    }

    /// Ends a drag that started on an explorer row. Dropping on a folder (or on a file, which
    /// stands for its folder) moves the dragged entry into it.
    pub(crate) fn finish_explorer_drag(&mut self, row: Option<usize>) {
        let Some(from) = self.explorer_drag.take() else {
            return;
        };
        // Workspace roots are not moved; they are not entries inside a folder.
        if self.tree.roots().iter().any(|root| root == &from) {
            return;
        }
        let Some(row) = row.and_then(|index| self.tree.rows.get(index)) else {
            return;
        };
        let destination = if row.entry.is_dir {
            row.entry.path.clone()
        } else {
            match row.entry.path.parent() {
                Some(parent) => parent.to_path_buf(),
                None => return,
            }
        };
        let Some(name) = from.file_name() else {
            return;
        };
        let to = destination.join(name);
        if to == from || to.parent() == from.parent() {
            return;
        }
        match self.move_explorer_entry(&from, &to) {
            Ok(()) => self.status = self.t("explorer.move.done").into(),
            Err(error) => self.status = error,
        }
    }

    pub(crate) fn explorer_inline_prompt_active(&self) -> bool {
        self.explorer_prompt.is_some()
            && !self.zen_mode
            && self.view_preferences.primary_sidebar_visible
            && self.active_activity() == crate::app::activity_bar::Activity::Explorer
            && self.active_plugin_activity.is_none()
    }

    pub(crate) fn explorer_inline_insert_index(&self) -> Option<(usize, usize)> {
        let prompt = self.explorer_prompt.as_ref()?;
        if prompt.kind == ExplorerPromptKind::Rename {
            return None;
        }
        let norm_parent =
            std::fs::canonicalize(&prompt.path).unwrap_or_else(|_| prompt.path.clone());
        if let Some((idx, row)) = self.tree.rows.iter().enumerate().find(|(_, row)| {
            std::fs::canonicalize(&row.entry.path).unwrap_or_else(|_| row.entry.path.clone())
                == norm_parent
        }) {
            Some((idx + 1, row.depth + 1))
        } else {
            Some((0, 0))
        }
    }

    pub(crate) fn explorer_inline_rename_match(&self, path: &Path) -> bool {
        let Some(prompt) = &self.explorer_prompt else {
            return false;
        };
        if prompt.kind != ExplorerPromptKind::Rename {
            return false;
        }
        let norm_prompt =
            std::fs::canonicalize(&prompt.path).unwrap_or_else(|_| prompt.path.clone());
        let norm_row = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        norm_prompt == norm_row
    }

    pub(crate) fn draw_explorer_inline_input(
        &self,
        renderer: &mut Renderer,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        cw: f32,
        value: &str,
    ) {
        let focus_border = rgb(Theme::color(Theme::STATUS, 0x007fd4));
        let input_bg = rgb(Theme::color(Theme::BG, 0x1e1e1e));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        renderer.draw_rect(x, y + 1.0, width, (height - 2.0).max(4.0), focus_border);
        renderer.draw_rect(
            x + 1.0,
            y + 2.0,
            (width - 2.0).max(0.0),
            (height - 4.0).max(2.0),
            input_bg,
        );
        let max_w = (width - 10.0).max(0.0);
        let visible = Self::help_menu_visible_label(&format!("{value}▏"), cw, max_w);
        renderer.draw_text(&visible, x + 5.0, y + 1.0, text);
    }

    /// Draws the explorer rename prompt above the window when the Explorer sidebar is hidden.
    pub(crate) fn draw_explorer_prompt(&self, renderer: &mut Renderer) {
        if self.explorer_inline_prompt_active() {
            return;
        }
        let Some(prompt) = &self.explorer_prompt else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let panel_width = (width as f32 * 0.72)
            .clamp(360.0, 700.0)
            .min((width as f32 - 24.0).max(0.0));
        let panel_height = 120.0;
        let x = ((width as f32 - panel_width) * 0.5).max(12.0);
        let y = ((height as f32 - panel_height) * 0.5).max(12.0);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, panel_width, panel_height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_width - 2.0).max(0.0),
            panel_height - 2.0,
            bg,
        );
        let (title, hint) = match prompt.kind {
            ExplorerPromptKind::Rename => ("explorer.rename.prompt", "explorer.rename.hint"),
            ExplorerPromptKind::NewFile => ("explorer.new_file.prompt", "explorer.new.hint"),
            ExplorerPromptKind::NewFolder => ("explorer.new_folder.prompt", "explorer.new.hint"),
        };
        renderer.draw_text(self.t(title), x + 16.0, y + 18.0, text);
        let max_chars = ((panel_width - 32.0) / cw.max(1.0)).floor() as usize;
        let input = Self::help_menu_visible_label(
            &format!("{}▏", prompt.value),
            cw,
            (max_chars as f32 * cw).max(0.0),
        );
        renderer.draw_rect(x + 12.0, y + 48.0, panel_width - 24.0, 28.0, border);
        renderer.draw_text(&input, x + 18.0, y + 54.0, text);
        renderer.draw_text(self.t(hint), x + 16.0, y + 90.0, dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_files() -> (crate::test_support::TempDir, App) {
        let dir = crate::test_support::TempDir::new();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        let app = App::new_with_options(dir.path().into(), None, true);
        (dir, app)
    }

    #[test]
    fn rename_moves_the_file_and_repoints_its_open_document() {
        let (dir, mut app) = app_with_files();
        let from = dir.path().join("a.txt");
        app.open_file(from.clone());
        let id = app.active_document();
        app.explorer_select(from.clone());
        app.explorer_rename();
        app.explorer_prompt.as_mut().unwrap().value = "src/renamed.txt".into();
        let prompt = app.explorer_prompt.take().unwrap();
        app.apply_explorer_rename(prompt);

        let to = dir.path().join("src/renamed.txt");
        assert!(to.exists());
        assert!(!from.exists());
        assert_eq!(app.documents[id].buffer.path(), Some(to.as_path()));
        assert_eq!(app.explorer_selected.as_deref(), Some(to.as_path()));
        // `src` is collapsed, so the moved file is not a visible row until it is expanded.
        assert!(!app
            .tree
            .rows
            .iter()
            .any(|row| row.entry.name == "renamed.txt"));
    }

    #[test]
    fn move_never_overwrites_and_refuses_folders_into_themselves() {
        let (dir, mut app) = app_with_files();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        assert!(app.move_explorer_entry(&a, &b).is_err());
        assert!(a.exists() && b.exists());
        let src = dir.path().join("src");
        assert!(app.move_explorer_entry(&src, &src.join("nested")).is_err());
    }

    #[test]
    fn drag_drop_moves_into_a_folder_row_and_ignores_the_same_place() {
        let (dir, mut app) = app_with_files();
        let a = dir.path().join("a.txt");
        app.explorer_drag = Some(a.clone());
        let src_row = app
            .tree
            .rows
            .iter()
            .position(|row| row.entry.name == "src")
            .unwrap();
        app.finish_explorer_drag(Some(src_row));
        assert!(dir.path().join("src/a.txt").exists());
        assert!(!a.exists());

        // Expand the folder to show the moved file, then drop it onto its own row.
        let src_row = app
            .tree
            .rows
            .iter()
            .position(|row| row.entry.name == "src")
            .unwrap();
        app.tree.activate(src_row);
        let moved = dir.path().join("src/a.txt");
        app.explorer_drag = Some(moved.clone());
        let same = app
            .tree
            .rows
            .iter()
            .position(|row| row.entry.name == "a.txt")
            .unwrap();
        app.finish_explorer_drag(Some(same));
        assert!(moved.exists());
    }

    #[test]
    fn delete_removes_entries_and_refuses_when_edits_would_be_lost() {
        let (dir, mut app) = app_with_files();
        let b = dir.path().join("b.txt");
        app.open_file(b.clone());
        app.editor_mut().insert("unsaved");
        assert!(app.remove_explorer_entry(&b).is_err());
        assert!(b.exists());

        let id = app.active_document();
        let text = app.editor().text();
        app.documents[id].buffer.mark_saved_snapshot(&text);
        app.remove_explorer_entry(&b).unwrap();
        assert!(!b.exists());
    }

    #[test]
    fn cut_copy_and_paste_entries_preserve_and_move_files() {
        let (dir, mut app) = app_with_files();
        let a = dir.path().join("a.txt");
        let src = dir.path().join("src");

        // Copy + paste in same directory creates "a copy.txt".
        app.explorer_select(a.clone());
        app.explorer_copy_entry();
        app.explorer_paste().unwrap();
        assert!(a.exists());
        assert!(dir.path().join("a copy.txt").exists());

        // Cut + paste into `src/` moves the file.
        app.explorer_select(dir.path().join("a copy.txt"));
        app.explorer_cut();
        app.explorer_select(src.clone());
        app.explorer_paste().unwrap();
        assert!(!dir.path().join("a copy.txt").exists());
        assert!(src.join("a copy.txt").exists());
    }

    #[test]
    fn new_entry_and_rename_use_inline_explorer_rows_when_sidebar_is_visible() {
        let (dir, mut app) = app_with_files();
        app.view_preferences.primary_sidebar_visible = true;
        app.explorer_new_entry(false);
        assert!(app.explorer_inline_prompt_active());
        assert_eq!(app.explorer_inline_insert_index(), Some((0, 0)));

        let src = dir.path().join("src");
        app.explorer_select(src.clone());
        app.explorer_new_entry(true);
        let src_row = app
            .tree
            .rows
            .iter()
            .position(|row| row.entry.name == "src")
            .unwrap();
        assert_eq!(app.explorer_inline_insert_index(), Some((src_row + 1, 1)));

        app.explorer_select(dir.path().join("a.txt"));
        app.explorer_rename();
        assert_eq!(app.explorer_inline_insert_index(), None);
        assert!(app.explorer_inline_rename_match(&dir.path().join("a.txt")));
    }

    #[test]
    fn explorer_delete_uses_non_blocking_modal_and_confirms_or_cancels() {
        let (dir, mut app) = app_with_files();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");

        app.explorer_select(a.clone());
        app.explorer_delete();
        assert_eq!(app.explorer_delete_confirm.as_deref(), Some(a.as_path()));
        assert!(a.exists());

        assert!(app.handle_explorer_delete_key(&Key::Named(NamedKey::Escape)));
        assert!(app.explorer_delete_confirm.is_none());
        assert!(a.exists());

        app.explorer_select(b.clone());
        app.explorer_delete();
        assert!(app.handle_explorer_delete_key(&Key::Named(NamedKey::Enter)));
        assert!(app.explorer_delete_confirm.is_none());
        assert!(!b.exists());
    }
}
