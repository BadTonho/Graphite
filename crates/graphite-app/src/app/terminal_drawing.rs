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
            self.t("terminal.title"),
            SIDEBAR_WIDTH + PAD,
            top,
            rgb(if !self.terminal_problems {
                0xffffff
            } else {
                0x858585
            }),
        );
        renderer.draw_text(
            self.t("terminal.problems"),
            SIDEBAR_WIDTH + cell_width * 9.0,
            top,
            rgb(if self.terminal_problems {
                0xffffff
            } else {
                0x858585
            }),
        );
        let profile_label = self
            .terminal_profiles
            .get(self.selected_terminal_profile)
            .map(|profile| profile.name.as_str())
            .unwrap_or_else(|| self.t("terminal.no_profile"));
        renderer.draw_text(
            &self.tf(
                "terminal.shell",
                &[("name", &profile_label.chars().take(9).collect::<String>())],
            ),
            SIDEBAR_WIDTH + cell_width * 19.0,
            top,
            rgb(0xd4d4d4),
        );
        renderer.draw_text("+", SIDEBAR_WIDTH + cell_width * 34.0, top, rgb(0xd4d4d4));
        renderer.draw_text(
            self.t("terminal.split"),
            SIDEBAR_WIDTH + cell_width * 38.0,
            top,
            rgb(0xd4d4d4),
        );
        renderer.draw_text(
            self.t("common.close"),
            SIDEBAR_WIDTH + cell_width * 45.0,
            top,
            rgb(0xd4d4d4),
        );
        renderer.draw_text("<", SIDEBAR_WIDTH + cell_width * 52.0, top, rgb(0xd4d4d4));
        renderer.draw_text(" >", SIDEBAR_WIDTH + cell_width * 54.0, top, rgb(0xd4d4d4));
        let mut tab_x = SIDEBAR_WIDTH + cell_width * 57.0;
        for (index, name) in self
            .terminal_names
            .iter()
            .enumerate()
            .skip(self.terminal_tab_offset)
        {
            let label = format!(" {} ", name);
            let tab_width = label.chars().count() as f32 * cell_width * 1.25;
            if tab_x + tab_width > width - PAD {
                break;
            }
            renderer.draw_text(
                &label,
                tab_x,
                top,
                rgb(if index == self.active_terminal {
                    0xffffff
                } else {
                    0x858585
                }),
            );
            tab_x += tab_width;
        }
        if self.terminal_problems {
            let diagnostics = self.diagnostic_entries();
            if diagnostics.is_empty() {
                renderer.draw_text(
                    self.t("terminal.no_diagnostics"),
                    SIDEBAR_WIDTH + PAD,
                    top + line_height * 1.5,
                    rgb(0x858585),
                );
            }
            for (index, (document, diagnostic)) in diagnostics
                .iter()
                .take(((panel_height / line_height) as usize).saturating_sub(1))
                .enumerate()
            {
                let severity = match diagnostic.severity {
                    Some(lsp_types::DiagnosticSeverity::ERROR) => {
                        self.t("diagnostic.severity.error")
                    }
                    Some(lsp_types::DiagnosticSeverity::WARNING) => {
                        self.t("diagnostic.severity.warning")
                    }
                    Some(lsp_types::DiagnosticSeverity::INFORMATION) => {
                        self.t("diagnostic.severity.info")
                    }
                    _ => self.t("diagnostic.severity.hint"),
                };
                let text = format!(
                    "{}:{}:{}  [{}] {}",
                    self.documents[*document]
                        .buffer
                        .path()
                        .and_then(Path::file_name)
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| self.t("common.untitled").into()),
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
        } else if !self.task_output.is_empty() {
            let visible = ((panel_height / line_height) as usize).saturating_sub(2);
            let start = self.task_output.len().saturating_sub(visible);
            for (index, line) in self.task_output.iter().skip(start).enumerate() {
                let clean: String = line
                    .chars()
                    .map(|ch| if ch.is_control() { ' ' } else { ch })
                    .take((((width - SIDEBAR_WIDTH - PAD * 2.0) / cell_width).max(1.0)) as usize)
                    .collect();
                renderer.draw_text(
                    &clean,
                    SIDEBAR_WIDTH + PAD,
                    top + line_height * (index as f32 + 1.5),
                    rgb(0xd4d4d4),
                );
            }
        } else if !self.terminals.is_empty() {
            let body_top = top + line_height * 1.5;
            let body_height = panel_height - line_height * 1.5;
            if let Some(second) = self
                .split_terminal
                .filter(|index| *index < self.terminals.len())
            {
                let split_x = SIDEBAR_WIDTH + (width - SIDEBAR_WIDTH) / 2.0;
                if let Some(first) = self.terminals.get(self.active_terminal) {
                    draw_terminal_snapshot(
                        renderer,
                        &first.snapshot(),
                        SIDEBAR_WIDTH + PAD,
                        split_x - SIDEBAR_WIDTH - PAD,
                        body_top,
                        body_height,
                        line_height,
                        cell_width,
                        self.translator,
                    );
                }
                if let Some(second) = self.terminals.get(second) {
                    draw_terminal_snapshot(
                        renderer,
                        &second.snapshot(),
                        split_x + PAD,
                        width - split_x - PAD,
                        body_top,
                        body_height,
                        line_height,
                        cell_width,
                        self.translator,
                    );
                }
                renderer.draw_rect(split_x, body_top, 1.0, body_height, rgb(0x454545));
            } else if let Some(terminal) = self.terminals.get(self.active_terminal) {
                draw_terminal_snapshot(
                    renderer,
                    &terminal.snapshot(),
                    SIDEBAR_WIDTH + PAD,
                    width - SIDEBAR_WIDTH - PAD,
                    body_top,
                    body_height,
                    line_height,
                    cell_width,
                    self.translator,
                );
            }
        }
    }
}

fn draw_terminal_snapshot(
    renderer: &mut Renderer,
    snapshot: &terminal::TerminalSnapshot,
    left: f32,
    width: f32,
    top: f32,
    height: f32,
    line_height: f32,
    cell_width: f32,
    translator: graphite_i18n::Translator,
) {
    for (row_index, row) in snapshot.rows.iter().enumerate() {
        let y = top + line_height * row_index as f32;
        if y + line_height > top + height {
            break;
        }
        for (column, cell) in row.iter().enumerate() {
            let x = left + column as f32 * cell_width;
            if x + cell_width > left + width {
                break;
            }
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
        let x = left + column as f32 * cell_width;
        let y = top + row as f32 * line_height;
        if x < left + width && y < top + height {
            renderer.draw_rect(x, y, 2.0, line_height, rgb(0xd4d4d4));
        }
    }
    if let Some(code) = snapshot.exit_code {
        let exited = translator.format("terminal.shell_exited", &[("code", &code)]);
        renderer.draw_text(&exited, left, top, rgb(0x858585));
    }
}
