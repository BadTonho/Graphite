//! Drawing of a notebook inside an editor group.

use super::editor_groups::GroupRect;
use super::*;
use crate::notebook::rich::Block;
use crate::notebook::{self, CellKind, Mode};

fn tint(dark: u32, light: u32) -> Color {
    rgb(Theme::color(dark, light))
}

impl App {
    /// Draws the cells of the notebook of `document` below the tab strip of `rect`.
    pub(super) fn draw_notebook(
        &self,
        r: &mut Renderer,
        rect: GroupRect,
        top: f32,
        lh: f32,
        cw: f32,
        document: DocumentId,
    ) {
        let Some(session) = self.documents[document].notebook.as_deref() else {
            return;
        };
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        // The strip above the cells names the kernel and what it is doing.
        let strip_top = rect.y + top;
        r.set_clip(Some([rect.x, strip_top, rect.x + rect.width, strip_top + lh]));
        r.draw_rect(rect.x, strip_top, rect.width, lh, rgb(Theme::color(0x2d2d30, 0xececec)));
        r.draw_text(&self.notebook_kernel_label(session), rect.x + 16.0, strip_top, dim);
        let body_top = strip_top + lh;
        let body_height = (rect.height - top - lh).max(0.0);
        let active_group = rect.id == self.editor_groups.focused();
        let layouts = notebook::layout(session, lh);
        r.set_clip(Some([rect.x, body_top, rect.x + rect.width, body_top + body_height]));
        r.draw_rect(rect.x, body_top, rect.width, body_height, rgb(Theme::color(Theme::BG, 0xffffff)));
        let left = rect.x + 16.0;
        let gutter = if session.line_numbers { cw * 5.0 } else { 0.0 };
        let text_left = left + gutter;
        let columns = (((rect.x + rect.width - text_left - PAD) / cw).max(1.0)) as usize;
        for (index, (state, cell)) in session.cells.iter().zip(&layouts).enumerate() {
            let y = body_top + cell.top - session.scroll;
            if y + cell.height < body_top || y > body_top + body_height {
                continue;
            }
            let selected = session.is_selected(index);
            let focused = index == session.focus;
            if selected {
                r.draw_rect(rect.x, y, rect.width, cell.height - lh * 0.6, tint(0x2a2d33, 0xf0f6ff));
            }
            if focused {
                let bar = if session.mode == Mode::Edit && active_group {
                    tint(0x3fb950, 0x1a7f37)
                } else {
                    rgb(Theme::color(Theme::STATUS, 0x007acc))
                };
                r.draw_rect(rect.x, y, 4.0, cell.height - lh * 0.6, bar);
            }
            let header = match state.cell.kind {
                CellKind::Code => match state.cell.execution_count {
                    _ if session.run_state(state.uid).is_some() => "[*]".to_owned(),
                    Some(count) => format!("[{count}]"),
                    None => "[ ]".to_owned(),
                },
                CellKind::Markdown => "markdown".to_owned(),
                CellKind::Raw => "raw".to_owned(),
            };
            r.draw_text(&header, left, y, dim);
            // The input.
            let input_y = y + lh;
            r.draw_rect(
                text_left - PAD * 0.5,
                input_y,
                rect.width - (text_left - rect.x) - PAD,
                lh * cell.input_lines as f32,
                tint(0x252526, 0xf7f7f7),
            );
            if state.input_collapsed {
                r.draw_text(&format!("… {}", self.t("notebook.input_collapsed")), text_left, input_y, dim);
            } else {
                let buffer = &state.buffer;
                let selection = buffer.view(&state.selection).selection();
                for line in 0..buffer.len_lines() {
                    let line_y = input_y + lh * line as f32;
                    if line_y + lh < body_top || line_y > body_top + body_height {
                        continue;
                    }
                    let content = buffer.line(line);
                    let content = content.trim_end_matches(['\n', '\r']);
                    if session.line_numbers {
                        r.draw_text(&format!("{:>4}", line + 1), left, line_y, dim);
                    }
                    if let (Some(selection), true) = (selection, focused && session.mode == Mode::Edit) {
                        let start = buffer.line_start_char(line);
                        let width = content.chars().count();
                        let (from, to) = (selection.anchor.min(selection.head), selection.anchor.max(selection.head));
                        let low = from.max(start);
                        let high = to.min(start + width + 1);
                        if low < high {
                            r.draw_rect(
                                text_left + cw * (low - start) as f32,
                                line_y,
                                cw * (high - low) as f32,
                                lh,
                                tint(0x264f78, 0xadd6ff),
                            );
                        }
                    }
                    r.draw_text(&content.chars().take(columns).collect::<String>(), text_left, line_y, text);
                }
                if focused && session.mode == Mode::Edit && active_group {
                    let (line, column) = buffer.view(&state.selection).cursor_line_col();
                    r.draw_rect(
                        text_left + cw * column as f32,
                        input_y + lh * line as f32,
                        2.0,
                        lh,
                        rgb(Theme::color(Theme::CURSOR, 0x000000)),
                    );
                }
            }
            // The outputs.
            if cell.output_lines > 0 {
                let output_y = y + cell.output_top(lh) - cell.top;
                if state.output_collapsed {
                    r.draw_text(&format!("… {}", self.t("notebook.output_collapsed")), text_left, output_y, dim);
                } else {
                    let mut row = 0usize;
                    for block in state.blocks().iter() {
                        let rows = block.rows(lh);
                        match block {
                            Block::Lines(lines) => {
                                for content in lines {
                                    let line_y = output_y + lh * row as f32;
                                    row += 1;
                                    // Long outputs are only drawn where they can be seen.
                                    if line_y + lh < body_top || line_y > body_top + body_height {
                                        continue;
                                    }
                                    r.draw_text(&content.chars().take(columns).collect::<String>(), text_left, line_y, dim);
                                }
                            }
                            Block::Image(image) => {
                                r.draw_image(&image.image, text_left, output_y + lh * row as f32, image.width, image.height);
                                row += rows;
                            }
                        }
                    }
                }
            }
        }
        r.set_clip(None);
    }
}

impl App {
    /// The kernel and its state, for the strip above the cells.
    pub(super) fn notebook_kernel_label(&self, session: &notebook::Session) -> String {
        let name = session
            .kernel_spec()
            .map(|spec| spec.display_name.clone())
            .or_else(|| session.metadata().kernel_name().map(str::to_owned))
            .unwrap_or_else(|| self.t("notebook.no_kernel").to_owned());
        let state = match session.kernel_status() {
            notebook::KernelStatus::None => "notebook.kernel.not_started",
            notebook::KernelStatus::Starting => "notebook.kernel.starting",
            notebook::KernelStatus::Idle => "notebook.kernel.idle",
            notebook::KernelStatus::Busy => "notebook.kernel.busy",
            notebook::KernelStatus::Dead(_) => "notebook.kernel.stopped",
        };
        format!("{name} — {}", self.t(state))
    }
}
