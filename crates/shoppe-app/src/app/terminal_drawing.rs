use super::*;

impl App {
    pub(crate) fn draw_terminal_panel(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        let panel_height = terminal_panel_height(height);
        let top = height - line_height - panel_height;
        renderer.draw_rect(
            SIDEBAR_WIDTH,
            top,
            width - SIDEBAR_WIDTH,
            panel_height,
            rgb(0x181818),
        );
        renderer.draw_rect(
            SIDEBAR_WIDTH,
            top,
            width - SIDEBAR_WIDTH,
            line_height * 1.35,
            rgb(0x252526),
        );
        renderer.draw_text(
            "TERMINAL",
            SIDEBAR_WIDTH + PAD,
            top,
            rgb(if !self.terminal_problems {
                0xffffff
            } else {
                0x858585
            }),
        );
        renderer.draw_text(
            "PROBLEMS",
            SIDEBAR_WIDTH + cell_width * 9.0,
            top,
            rgb(if self.terminal_problems {
                0xffffff
            } else {
                0x858585
            }),
        );
        if self.terminal_problems {
            let diagnostics = &self.tabs[self.active_tab].diagnostics;
            if diagnostics.is_empty() {
                renderer.draw_text(
                    "No diagnostics",
                    SIDEBAR_WIDTH + PAD,
                    top + line_height * 1.5,
                    rgb(0x858585),
                );
            }
            for (index, diagnostic) in diagnostics
                .iter()
                .take(((panel_height / line_height) as usize).saturating_sub(1))
                .enumerate()
            {
                let severity = match diagnostic.severity {
                    Some(lsp_types::DiagnosticSeverity::ERROR) => "error",
                    Some(lsp_types::DiagnosticSeverity::WARNING) => "warning",
                    Some(lsp_types::DiagnosticSeverity::INFORMATION) => "info",
                    _ => "hint",
                };
                let text = format!(
                    "{}:{}  [{}] {}",
                    diagnostic.range.start.line + 1,
                    diagnostic.range.start.character + 1,
                    severity,
                    diagnostic.message
                );
                renderer.draw_text(
                    &text,
                    SIDEBAR_WIDTH + PAD,
                    top + line_height * (index as f32 + 1.5),
                    rgb(0xd4d4d4),
                );
            }
        } else if let Some(status) = self.terminal_status.as_deref() {
            renderer.draw_text(
                status,
                SIDEBAR_WIDTH + PAD,
                top + line_height * 1.5,
                rgb(0xf14c4c),
            );
        } else if let Some(terminal) = &self.terminal {
            let snapshot = terminal.snapshot();
            for (row_index, row) in snapshot.rows.iter().enumerate() {
                let y = top + line_height * (row_index as f32 + 1.5);
                if y + line_height > height - line_height {
                    break;
                }
                for (column, cell) in row.iter().enumerate() {
                    let x = SIDEBAR_WIDTH + PAD + column as f32 * cell_width;
                    if cell.background != [30, 30, 30] {
                        let hex = ((cell.background[0] as u32) << 16)
                            | ((cell.background[1] as u32) << 8)
                            | cell.background[2] as u32;
                        renderer.draw_rect(x, y, cell_width, line_height, rgb(hex));
                    }
                    if cell.character != ' ' && !cell.hidden && !cell.wide_spacer {
                        let hex = ((cell.foreground[0] as u32) << 16)
                            | ((cell.foreground[1] as u32) << 8)
                            | cell.foreground[2] as u32;
                        renderer.draw_text(&cell.character.to_string(), x, y, rgb(hex));
                    }
                }
            }
            if let Some((row, column)) = snapshot.cursor {
                let x = SIDEBAR_WIDTH + PAD + column as f32 * cell_width;
                let y = top + line_height * (row as f32 + 1.5);
                renderer.draw_rect(x, y, 2.0, line_height, rgb(0xd4d4d4));
            }
            if let Some(exited) = snapshot.exited {
                renderer.draw_text(
                    &exited,
                    SIDEBAR_WIDTH + PAD,
                    top + line_height * 1.5,
                    rgb(0x858585),
                );
            }
        }
    }
}
