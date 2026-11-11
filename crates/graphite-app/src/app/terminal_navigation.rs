//! Terminal navigation: jumping between shell prompts and opening links in the output.

use super::*;
use crate::terminal_links::{link_at, TerminalLink};
use crate::terminal_shell::ShellEvent;

impl App {
    /// Scrolls the active terminal to the previous or next shell prompt.
    pub(crate) fn jump_terminal_prompt(&mut self, forward: bool) {
        let Some(terminal) = self.terminals.get(self.active_terminal) else {
            return;
        };
        let snapshot = terminal.snapshot();
        let prompts = snapshot
            .marks
            .iter()
            .filter(|mark| mark.event == ShellEvent::PromptStart)
            .map(|mark| mark.row);
        let top = snapshot.top_row;
        let target = if forward {
            prompts.filter(|row| *row > top).min()
        } else {
            prompts.filter(|row| *row < top).max()
        };
        match target {
            Some(row) => terminal.scroll_to_row(row),
            None => self.status = self.t("terminal.no_prompt").into(),
        }
    }

    /// Ctrl+click on terminal output: opens an https link or a `file:line` reference.
    /// Returns whether the click landed on a link.
    pub(crate) fn open_terminal_link_at(
        &mut self,
        mx: f32,
        my: f32,
        panel_x: f32,
        panel_top: f32,
        line_height: f32,
        cell_width: f32,
    ) -> bool {
        let body_top = panel_top + line_height * 1.5;
        if my < body_top || mx < panel_x + PAD {
            return false;
        }
        let row = ((my - body_top) / line_height) as usize;
        let column = ((mx - (panel_x + PAD)) / cell_width) as usize;
        let Some(terminal) = self.terminals.get(self.active_terminal) else {
            return false;
        };
        let snapshot = terminal.snapshot();
        let Some(cells) = snapshot.rows.get(row) else {
            return false;
        };
        let line: String = cells
            .iter()
            .map(|cell| if cell.hidden { ' ' } else { cell.character })
            .collect();
        let Some(link) = link_at(&line, column) else {
            return false;
        };
        match link {
            TerminalLink::Url(url) => {
                if url.starts_with("https://") {
                    super::plugins::drawing::open_external_https(&url);
                    self.status = self.tf("terminal.link.opened", &[("target", &url)]);
                } else {
                    self.status = self.t("terminal.link.https_only").into();
                }
            }
            TerminalLink::File { path, line, column } => {
                let base = snapshot
                    .cwd
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.tree.root().to_path_buf());
                let candidate = if Path::new(&path).is_absolute() {
                    PathBuf::from(&path)
                } else {
                    base.join(&path)
                };
                if candidate.is_file() {
                    self.open_file(candidate);
                    self.editor_mut()
                        .set_cursor_line_col(line.saturating_sub(1), column.saturating_sub(1));
                    self.reveal_cursor = true;
                } else {
                    self.status = self.tf("terminal.link.not_found", &[("path", &path)]);
                }
            }
        }
        true
    }
}
