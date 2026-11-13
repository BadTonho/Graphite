//! The multi-file diff view: opening it from the repository changes, driving it and drawing it.

use super::*;
use crate::multi_diff::{FileInput, FileStatus, LineKind, MultiDiff, Piece, RowRef};
use crate::scm_changes::ChangeSet;

/// Pixels reserved above the rows for the title line.
const TITLE_LINES: f32 = 1.6;
/// Share of the width taken by the file list while it has the keyboard.
const LIST_SHARE: f32 = 0.28;

impl App {
    fn multi_diff_window(&self) -> (f32, f32, f32) {
        self.gpu.as_ref().map_or((1280.0, 720.0, 20.0), |gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height, gpu.renderer.line_height())
        })
    }

    fn multi_diff_rows_in_window(&self) -> usize {
        let (_, height, line_height) = self.multi_diff_window();
        (((height - line_height * TITLE_LINES) / line_height).floor() as usize).max(1)
    }

    fn multi_diff_title(&self, set: ChangeSet, files: usize) -> String {
        let key = match set {
            ChangeSet::All => "multi_diff.title.all",
            ChangeSet::Staged => "multi_diff.title.staged",
            ChangeSet::Unstaged => "multi_diff.title.unstaged",
        };
        self.tf(key, &[("count", &files)])
    }

    /// Opens the changes of the repository in one view. With an event loop the files are read on
    /// a background thread; without one (tests) they are read at once.
    pub(crate) fn open_multi_diff(&mut self, set: ChangeSet) {
        let root = self.scm_workspace_root();
        let translator = self.translator;
        let collect = move || {
            let snapshot = crate::scm::snapshot(&root);
            if let Some(error) = snapshot.error {
                return Err(error);
            }
            crate::scm_changes::collect(&root, &snapshot.files, set, &translator)
        };
        match self.event_proxy.clone() {
            Some(proxy) => {
                self.status = self.t("multi_diff.loading").into();
                std::thread::Builder::new()
                    .name("graphite-multi-diff".into())
                    .spawn(move || {
                        let _ = proxy.send_event(UserEvent::MultiDiffReady {
                            set,
                            result: collect(),
                        });
                    })
                    .ok();
            }
            None => self.show_multi_diff(set, collect()),
        }
    }

    /// Shows the collected changes, or says why there is nothing to show.
    pub(crate) fn show_multi_diff(&mut self, set: ChangeSet, result: Result<Vec<FileInput>, String>) {
        match result {
            Err(error) => {
                self.status = self.tf("multi_diff.failed", &[("error", &error)]);
            }
            Ok(inputs) if inputs.is_empty() => {
                self.status = self.t("multi_diff.empty").into();
            }
            Ok(inputs) => {
                let title = self.multi_diff_title(set, inputs.len());
                self.multi_diff = Some(MultiDiff::new(title, inputs));
                self.status = self.t("multi_diff.opened").into();
            }
        }
    }

    pub(crate) fn close_multi_diff(&mut self) {
        if self.multi_diff.take().is_some() {
            self.status = self.t("multi_diff.closed").into();
        }
    }

    pub(crate) fn multi_diff_collapse(&mut self, collapse: bool) {
        if let Some(diff) = self.multi_diff.as_mut() {
            if collapse {
                diff.collapse_all();
            } else {
                diff.expand_all();
            }
        }
    }

    pub(crate) fn multi_diff_step(&mut self, forward: bool) {
        let Some(diff) = self.multi_diff.as_mut() else {
            return;
        };
        let moved = if forward {
            diff.next_change()
        } else {
            diff.previous_change()
        };
        if !moved {
            self.status = self.t("multi_diff.no_changes").into();
        }
    }

    /// Gives the keyboard to the file list, or takes it back.
    pub(crate) fn multi_diff_toggle_list(&mut self) {
        if let Some(diff) = self.multi_diff.as_mut() {
            diff.list_focus = !diff.list_focus;
        }
    }

    /// Collapses or expands the file the selection or the current change is in.
    pub(crate) fn multi_diff_toggle_current_file(&mut self) {
        if let Some(diff) = self.multi_diff.as_mut() {
            let file = diff.selected_file;
            diff.toggle_file(file);
        }
    }

    /// Opens the file of the current change in the editor at that change and closes the view.
    pub(crate) fn multi_diff_go_to_file(&mut self) {
        let Some(diff) = self.multi_diff.as_ref() else {
            return;
        };
        let (file, line) = match diff.current_location() {
            Some(location) if !diff.list_focus => location,
            _ => (diff.selected_file, 1),
        };
        let Some(entry) = diff.files.get(file) else {
            return;
        };
        let deleted = entry.status == FileStatus::Deleted;
        let path = self.scm_workspace_root().join(&entry.path);
        if deleted || !path.is_file() {
            self.status = self.tf("multi_diff.file_missing", &[("path", &entry.path)]);
            return;
        }
        self.multi_diff = None;
        self.open_file(path);
        self.editor_mut().set_cursor_line_col(line.saturating_sub(1), 0);
    }

    /// Keys of the multi-file diff.
    pub(crate) fn handle_multi_diff_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        let list = self.multi_diff.as_ref().is_some_and(|diff| diff.list_focus);
        let window = self.multi_diff_rows_in_window() as isize;
        match &event.logical_key {
            Key::Named(NamedKey::Escape) if list => self.multi_diff_toggle_list(),
            Key::Named(NamedKey::Escape) => self.run_ui_command("graphite.multiDiff.close"),
            Key::Named(NamedKey::ArrowDown) if list => {
                if let Some(diff) = self.multi_diff.as_mut() {
                    diff.select_file(1);
                }
            }
            Key::Named(NamedKey::ArrowUp) if list => {
                if let Some(diff) = self.multi_diff.as_mut() {
                    diff.select_file(-1);
                }
            }
            Key::Named(NamedKey::ArrowDown) => self.run_ui_command("multiDiffEditor.goToNextChange"),
            Key::Named(NamedKey::ArrowUp) => self.run_ui_command("multiDiffEditor.goToPreviousChange"),
            Key::Named(NamedKey::Enter) if list => {
                self.run_ui_command("graphite.multiDiff.toggleFile")
            }
            Key::Named(NamedKey::Enter) => self.run_ui_command("multiDiffEditor.goToFile"),
            Key::Named(NamedKey::Tab) => self.run_ui_command("multiDiffEditor.focusFileList"),
            Key::Named(NamedKey::PageDown) => {
                if let Some(diff) = self.multi_diff.as_mut() {
                    diff.scroll_by(window, window as usize);
                }
            }
            Key::Named(NamedKey::PageUp) => {
                if let Some(diff) = self.multi_diff.as_mut() {
                    diff.scroll_by(-window, window as usize);
                }
            }
            _ => match event.text.as_deref().unwrap_or("").to_ascii_lowercase().as_str() {
                "c" => self.run_ui_command("multiDiffEditor.collapseAll"),
                "e" => self.run_ui_command("multiDiffEditor.expandAll"),
                " " => self.run_ui_command("graphite.multiDiff.toggleFile"),
                _ => {}
            },
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn multi_diff_scroll_rows(&mut self, rows: isize) {
        let window = self.multi_diff_rows_in_window();
        if let Some(diff) = self.multi_diff.as_mut() {
            diff.scroll_by(rows, window);
        }
    }

    /// A click on a file header collapses or expands that file.
    pub(crate) fn multi_diff_click(&mut self, x: f32, y: f32) {
        let (width, _, line_height) = self.multi_diff_window();
        let top = line_height * TITLE_LINES;
        let Some(diff) = self.multi_diff.as_mut() else {
            return;
        };
        let list_width = if diff.list_focus { width * LIST_SHARE } else { 0.0 };
        if y < top {
            return;
        }
        let slot = ((y - top) / line_height).floor() as usize;
        if x < list_width {
            // The file list: select the file and bring it into view.
            if slot < diff.files.len() {
                diff.selected_file = slot;
                diff.select_file(0);
            }
            return;
        }
        if let Some(RowRef::Header(file)) = diff.rows().get(diff.scroll + slot).copied() {
            diff.selected_file = file;
            diff.toggle_file(file);
        }
    }

    pub(crate) fn draw_multi_diff(&self, renderer: &mut Renderer, width: f32, height: f32, line_height: f32) {
        let Some(diff) = self.multi_diff.as_ref() else {
            return;
        };
        let char_width = renderer.char_width();
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        let background = rgb(Theme::color(Theme::BG, 0xffffff));
        renderer.draw_rect(0.0, 0.0, width, height, background);
        let (added, removed) = diff.totals();
        renderer.draw_text(
            &self.tf(
                "multi_diff.header",
                &[("title", &diff.title), ("added", &added), ("removed", &removed)],
            ),
            PAD,
            line_height * 0.2,
            text,
        );
        renderer.draw_text(self.t("multi_diff.hint"), PAD, line_height * 0.9, dim);
        let top = line_height * TITLE_LINES;
        let window = (((height - top) / line_height).floor() as usize).max(1);
        let list_width = if diff.list_focus { width * LIST_SHARE } else { 0.0 };
        let green = rgb(Theme::color(0x1d3a27, 0xdcf5e1));
        let red = rgb(Theme::color(0x4a2326, 0xfbe3e4));
        let header_bar = rgb(Theme::color(0x2d2d30, 0xececec));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        let selected_row = rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff));

        if diff.list_focus {
            renderer.draw_rect(0.0, top, list_width, height - top, rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)));
            renderer.set_clip(Some([0.0, top, list_width, height]));
            for (index, file) in diff.files.iter().enumerate().take(window) {
                let y = top + line_height * index as f32;
                if index == diff.selected_file {
                    renderer.draw_rect(0.0, y, list_width, line_height, selected_row);
                }
                let columns = ((list_width - PAD * 2.0) / char_width).max(0.0) as usize;
                renderer.draw_text(&file.path.chars().take(columns).collect::<String>(), PAD, y, text);
            }
            renderer.set_clip(None);
        }

        let body_x = list_width;
        let body_width = width - list_width;
        let columns = (((body_width - PAD * 2.0) / char_width).max(0.0)) as usize;
        renderer.set_clip(Some([body_x, top, width, height]));
        for slot in 0..window {
            let Some(row) = diff.rows().get(diff.scroll + slot).copied() else {
                break;
            };
            let y = top + line_height * slot as f32;
            match row {
                RowRef::Header(index) => {
                    let file = &diff.files[index];
                    renderer.draw_rect(body_x, y, body_width, line_height, header_bar);
                    let chevron = if file.collapsed { "\u{25b8}" } else { "\u{25be}" };
                    let tag = match file.status {
                        FileStatus::Added => "A",
                        FileStatus::Deleted => "D",
                        FileStatus::Modified => "M",
                    };
                    let counts = if file.binary {
                        self.t("multi_diff.binary").to_owned()
                    } else {
                        format!("+{} -{}", file.added, file.removed)
                    };
                    renderer.draw_text(
                        &format!("{chevron} {tag}  {}   {counts}", file.path)
                            .chars()
                            .take(columns)
                            .collect::<String>(),
                        body_x + PAD,
                        y,
                        text,
                    );
                    let current_file = diff
                        .current
                        .and_then(|current| diff.changes().get(current))
                        .map(|change| change.file);
                    if current_file == Some(index) && diff.changes().get(diff.current.unwrap_or(0)).is_some_and(|change| change.piece.is_none()) {
                        renderer.draw_rect(body_x, y, 3.0, line_height, accent);
                    }
                }
                RowRef::Piece(file_index, piece_index) => match &diff.files[file_index].pieces[piece_index] {
                    Piece::Gap(count) => {
                        renderer.draw_text(
                            &self.tf("multi_diff.gap", &[("count", count)]),
                            body_x + PAD + char_width * 12.0,
                            y,
                            dim,
                        );
                    }
                    Piece::Line(line) => {
                        match line.kind {
                            LineKind::Added => renderer.draw_rect(body_x, y, body_width, line_height, green),
                            LineKind::Removed => renderer.draw_rect(body_x, y, body_width, line_height, red),
                            LineKind::Context => {}
                        }
                        let number = |value: Option<usize>| match value {
                            Some(value) => format!("{value:>5}"),
                            None => "     ".to_owned(),
                        };
                        let sign = match line.kind {
                            LineKind::Added => '+',
                            LineKind::Removed => '-',
                            LineKind::Context => ' ',
                        };
                        renderer.draw_text(
                            &format!("{} {} ", number(line.old_line), number(line.new_line)),
                            body_x + PAD,
                            y,
                            dim,
                        );
                        let text_x = body_x + PAD + char_width * 12.0;
                        let room = columns.saturating_sub(12);
                        renderer.draw_text(
                            &format!("{sign} {}", line.text).chars().take(room).collect::<String>(),
                            text_x,
                            y,
                            text,
                        );
                        let is_current = diff
                            .current
                            .and_then(|current| diff.changes().get(current))
                            .is_some_and(|change| {
                                change.file == file_index && change.piece == Some(piece_index)
                            });
                        if is_current {
                            renderer.draw_rect(body_x, y, 3.0, line_height, accent);
                        }
                    }
                },
            }
        }
        renderer.set_clip(None);
    }
}
