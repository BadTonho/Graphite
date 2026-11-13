//! Drawing and hit testing of the merge editor.

use super::merge_editor::{pane_rects, PaneRect};
use super::*;
use crate::merge::{Pane, Resolution};

const LINE_NUMBER_CHARS: usize = 5;

fn tint(dark: u32, light: u32) -> Color {
    rgb(Theme::color(dark, light))
}

impl App {
    /// Panes of the open merge editor and how many rows the smallest one can show.
    pub(crate) fn merge_geometry(
        &self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> Option<(Vec<(Pane, PaneRect)>, usize)> {
        let editor = self.merge_editor.as_ref()?;
        let top = line_height * 2.2;
        let area = PaneRect {
            x: 0.0,
            y: top,
            width,
            height: (height - top).max(line_height * 2.0),
        };
        let panes = pane_rects(editor.layout, editor.base, area);
        let window = panes
            .iter()
            .map(|(_, rect)| ((rect.height - line_height) / line_height).floor().max(1.0) as usize)
            .min()
            .unwrap_or(1);
        Some((panes, window))
    }

    fn merge_pane_title(&self, pane: Pane) -> String {
        let editor = match self.merge_editor.as_ref() {
            Some(editor) => editor,
            None => return String::new(),
        };
        let first = editor.state.block(0);
        let (key, label) = match pane {
            Pane::Current => (
                "merge.pane.current",
                first.map(|block| block.current_label.as_str()),
            ),
            Pane::Incoming => (
                "merge.pane.incoming",
                first.map(|block| block.incoming_label.as_str()),
            ),
            Pane::Base => (
                "merge.pane.base",
                first.and_then(|block| block.base_label.as_deref()),
            ),
            Pane::Result => ("merge.pane.result", None),
        };
        match label.filter(|label| !label.is_empty()) {
            Some(label) => format!("{} — {label}", self.t(key)),
            None => self.t(key).to_owned(),
        }
    }

    fn merge_resolution_label(&self, resolution: Resolution) -> &'static str {
        match resolution {
            Resolution::Unresolved => "merge.state.unresolved",
            Resolution::Ignored => "merge.state.ignored",
            Resolution::Taken(taken) if taken.current && taken.incoming => "merge.state.both",
            Resolution::Taken(taken) if taken.current => "merge.state.current",
            Resolution::Taken(_) => "merge.state.incoming",
        }
    }

    pub(crate) fn draw_merge_editor(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
    ) {
        let Some(editor) = self.merge_editor.as_ref() else {
            return;
        };
        let Some((panes, _)) = self.merge_geometry(width, height, line_height) else {
            return;
        };
        let char_width = renderer.char_width();
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        renderer.draw_rect(0.0, 0.0, width, height, rgb(Theme::color(Theme::BG, 0xffffff)));
        let unresolved = editor.state.unresolved_count();
        let summary = if unresolved == 0 {
            self.t("merge.all_resolved").to_owned()
        } else {
            self.tf("merge.unresolved_count", &[("count", &unresolved)])
        };
        renderer.draw_text(
            &format!("{} — {}", editor.path.display(), summary),
            PAD,
            line_height * 0.2,
            text,
        );
        renderer.draw_text(self.t("merge.hint"), PAD, line_height * 1.1, dim);

        let separator = tint(0x3c3c3c, 0xd0d0d0);
        for (pane, rect) in &panes {
            renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, rgb(Theme::color(Theme::BG, 0xffffff)));
            renderer.draw_rect(rect.x, rect.y, rect.width, line_height, tint(0x2d2d30, 0xececec));
            renderer.set_clip(Some([rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]));
            renderer.draw_text(&self.merge_pane_title(*pane), rect.x + PAD, rect.y, dim);
            let window = ((rect.height - line_height) / line_height).floor().max(0.0) as usize;
            let gutter = char_width * (LINE_NUMBER_CHARS as f32 + 1.0);
            let columns = (((rect.width - gutter - PAD) / char_width).max(0.0)) as usize;
            for slot in 0..window {
                let Some(&row_index) = editor.visible().get(editor.scroll + slot) else {
                    break;
                };
                let row = &editor.rows()[row_index];
                let y = rect.y + line_height * (slot as f32 + 1.0);
                let in_block = row.block.is_some();
                if in_block {
                    let background = match pane {
                        Pane::Result if row.resolution.is_handled() => tint(0x16341f, 0xdcf5e1),
                        Pane::Result => tint(0x4d3b00, 0xfff4c2),
                        Pane::Current => tint(0x23324a, 0xe3eefc),
                        Pane::Incoming => tint(0x1f3a2a, 0xe3f6e8),
                        Pane::Base => tint(0x3a3a3a, 0xececec),
                    };
                    renderer.draw_rect(rect.x, y, rect.width, line_height, background);
                    if let Some(cell) = row.cell(*pane) {
                        if cell.changed {
                            let strong = if *pane == Pane::Current {
                                tint(0x2f4f7f, 0xbcd7f7)
                            } else {
                                tint(0x2c6040, 0xb9e8c6)
                            };
                            renderer.draw_rect(rect.x, y, rect.width, line_height, strong);
                        }
                    }
                    if row.block == editor.active {
                        renderer.draw_rect(rect.x, y, 3.0, line_height, rgb(Theme::color(Theme::STATUS, 0x007acc)));
                    }
                }
                if let Some(cell) = row.cell(*pane) {
                    renderer.draw_text(
                        &format!("{:>width$}", cell.line, width = LINE_NUMBER_CHARS),
                        rect.x + PAD,
                        y,
                        dim,
                    );
                    renderer.draw_text(
                        &cell.text.chars().take(columns).collect::<String>(),
                        rect.x + PAD + gutter,
                        y,
                        text,
                    );
                }
                if *pane == Pane::Result && row.first_of_block {
                    let label = self.t(self.merge_resolution_label(row.resolution));
                    let label_x = rect.x + rect.width - (label.chars().count() as f32 + 1.0) * char_width;
                    renderer.draw_text(label, label_x, y, dim);
                }
            }
            renderer.set_clip(None);
            renderer.draw_rect(rect.x, rect.y, 1.0, rect.height, separator);
            renderer.draw_rect(rect.x, rect.y, rect.width, 1.0, separator);
        }
    }

    /// Selects the conflict under a click, if any.
    pub(crate) fn merge_click(&mut self, x: f32, y: f32, width: f32, height: f32, line_height: f32) {
        let Some((panes, _)) = self.merge_geometry(width, height, line_height) else {
            return;
        };
        let Some((_, rect)) = panes.iter().find(|(_, rect)| rect.contains(x, y)) else {
            return;
        };
        let slot = ((y - rect.y) / line_height).floor() as isize - 1;
        if slot < 0 {
            return;
        }
        if let Some(editor) = self.merge_editor.as_mut() {
            if let Some(block) = editor.block_at_visible(editor.scroll + slot as usize) {
                editor.active = Some(block);
            }
        }
    }

    /// Scrolls the merge panes by whole rows.
    pub(crate) fn merge_scroll(&mut self, rows: isize, width: f32, height: f32, line_height: f32) {
        let Some((_, window)) = self.merge_geometry(width, height, line_height) else {
            return;
        };
        if let Some(editor) = self.merge_editor.as_mut() {
            editor.scroll_by(rows, window);
        }
    }
}
