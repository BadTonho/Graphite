use super::*;

pub(crate) const TERMINAL_HEADER_HEIGHT: f32 = 32.0;
pub(crate) const TERMINAL_GUTTER_WIDTH: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalHeaderTab {
    Problems,
    Output,
    DebugConsole,
    Terminal,
    Ports,
    Comments,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalToolbarAction {
    SelectSession(usize),
    PrevSessionTabs,
    NextSessionTabs,
    CycleProfile,
    NewTerminal,
    SplitTerminal,
    KillTerminal,
    MoreActions,
    ToggleMaximize,
    ClosePanel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeaderHitRect<T> {
    pub item: T,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl<T: Copy> HeaderHitRect<T> {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

pub(crate) struct TerminalHeaderLayout {
    pub tabs: Vec<HeaderHitRect<TerminalHeaderTab>>,
    pub actions: Vec<HeaderHitRect<TerminalToolbarAction>>,
    pub shell_rect: HeaderHitRect<TerminalToolbarAction>,
    pub shell_label: String,
    pub divider_x: f32,
}

pub(crate) fn short_shell_name(profile: Option<&crate::config::TerminalProfile>) -> String {
    let Some(profile) = profile else {
        return "shell".to_owned();
    };
    let stem = profile
        .path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&profile.path)
        .trim();
    let stem_no_ext = stem
        .strip_suffix(".exe")
        .or_else(|| stem.strip_suffix(".EXE"))
        .unwrap_or(stem)
        .to_ascii_lowercase();
    if !stem_no_ext.is_empty() {
        return match stem_no_ext.as_str() {
            "powershell" => "powershell".to_owned(),
            "pwsh" => "pwsh".to_owned(),
            "cmd" => "cmd".to_owned(),
            "bash" => "bash".to_owned(),
            "sh" => "sh".to_owned(),
            "zsh" => "zsh".to_owned(),
            other => other.chars().take(14).collect(),
        };
    }
    let fallback = profile
        .name
        .split_whitespace()
        .next()
        .unwrap_or("shell")
        .to_ascii_lowercase();
    if fallback.is_empty() {
        "shell".to_owned()
    } else {
        fallback.chars().take(14).collect()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_header_layout(
    panel_x: f32,
    panel_y: f32,
    panel_width: f32,
    cell_width: f32,
    tab_labels: [(TerminalHeaderTab, &str); 6],
    shell_label: &str,
    session_names: &[String],
    tab_offset: usize,
) -> TerminalHeaderLayout {
    let cw = cell_width.max(4.0);
    let btn_y = panel_y + 5.0;
    let btn_h = 22.0;

    // 1. Lay out right-aligned toolbar icons from the right edge leftward.
    let mut rx = panel_x + panel_width - 8.0;
    let mut right_actions = Vec::with_capacity(16);

    let close_w = 22.0;
    rx -= close_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::ClosePanel,
        x: rx,
        y: btn_y,
        w: close_w,
        h: btn_h,
    });

    let max_w = 22.0;
    rx -= 4.0 + max_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::ToggleMaximize,
        x: rx,
        y: btn_y,
        w: max_w,
        h: btn_h,
    });

    let divider_x = rx - 5.0;
    rx -= 10.0;

    let more_w = 22.0;
    rx -= more_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::MoreActions,
        x: rx,
        y: btn_y,
        w: more_w,
        h: btn_h,
    });

    let kill_w = 22.0;
    rx -= 2.0 + kill_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::KillTerminal,
        x: rx,
        y: btn_y,
        w: kill_w,
        h: btn_h,
    });

    let split_w = 22.0;
    rx -= 2.0 + split_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::SplitTerminal,
        x: rx,
        y: btn_y,
        w: split_w,
        h: btn_h,
    });

    let chevron_w = 16.0;
    rx -= 4.0 + chevron_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::CycleProfile,
        x: rx,
        y: btn_y,
        w: chevron_w,
        h: btn_h,
    });

    let new_w = 20.0;
    rx -= new_w;
    right_actions.push(HeaderHitRect {
        item: TerminalToolbarAction::NewTerminal,
        x: rx,
        y: btn_y,
        w: new_w,
        h: btn_h,
    });

    let shell_w = 22.0 + shell_label.chars().count() as f32 * cw + 8.0;
    rx -= 6.0 + shell_w;
    let shell_rect = HeaderHitRect {
        item: TerminalToolbarAction::CycleProfile,
        x: rx,
        y: btn_y,
        w: shell_w,
        h: btn_h,
    };
    right_actions.push(shell_rect);

    // 2. Lay out left-side panel view tabs (`Problems`, `Output`, `Debug Console`, `Terminal`, `Ports`, `Comments`).
    let mut tabs = Vec::with_capacity(tab_labels.len());
    let mut tx = panel_x + 10.0;
    let toolbar_min_x = shell_rect.x;
    for (item, label) in tab_labels {
        let w = label.chars().count() as f32 * cw + 16.0;
        if !tabs.is_empty() && tx + w > toolbar_min_x - 8.0 {
            break;
        }
        tabs.push(HeaderHitRect {
            item,
            x: tx,
            y: btn_y,
            w,
            h: btn_h,
        });
        tx += w + 6.0;
    }

    // 3. When multiple terminals exist, fit session switcher pills/arrows strictly between tabs and toolbar.
    let tabs_right_edge = tabs.last().map_or(panel_x + 10.0, |t| t.x + t.w);
    if session_names.len() > 1 && shell_rect.x - tabs_right_edge > 52.0 {
        let mut sx = shell_rect.x - 6.0;
        let arrow_w = 16.0;
        sx -= arrow_w;
        right_actions.push(HeaderHitRect {
            item: TerminalToolbarAction::NextSessionTabs,
            x: sx,
            y: btn_y,
            w: arrow_w,
            h: btn_h,
        });
        sx -= 2.0 + arrow_w;
        right_actions.push(HeaderHitRect {
            item: TerminalToolbarAction::PrevSessionTabs,
            x: sx,
            y: btn_y,
            w: arrow_w,
            h: btn_h,
        });

        let mut pill_x = sx - 6.0;
        let min_session_x = tabs_right_edge + 8.0;
        for (index, name) in session_names.iter().enumerate().skip(tab_offset) {
            let short_len = name.chars().count().min(14);
            let pill_w = short_len as f32 * cw + 12.0;
            if pill_x - pill_w < min_session_x {
                break;
            }
            pill_x -= pill_w;
            right_actions.push(HeaderHitRect {
                item: TerminalToolbarAction::SelectSession(index),
                x: pill_x,
                y: btn_y,
                w: pill_w,
                h: btn_h,
            });
            pill_x -= 4.0;
        }
    }

    right_actions.reverse();

    TerminalHeaderLayout {
        tabs,
        actions: right_actions,
        shell_rect,
        shell_label: shell_label.to_owned(),
        divider_x,
    }
}

impl App {
    pub(crate) fn terminal_header_layout(
        &self,
        panel_x: f32,
        panel_y: f32,
        panel_width: f32,
        cell_width: f32,
    ) -> TerminalHeaderLayout {
        let tab_labels = [
            (TerminalHeaderTab::Problems, self.t("terminal.tab.problems")),
            (TerminalHeaderTab::Output, self.t("terminal.tab.output")),
            (
                TerminalHeaderTab::DebugConsole,
                self.t("terminal.tab.debug_console"),
            ),
            (TerminalHeaderTab::Terminal, self.t("terminal.tab.terminal")),
            (TerminalHeaderTab::Ports, self.t("terminal.tab.ports")),
            (TerminalHeaderTab::Comments, self.t("terminal.tab.comments")),
        ];
        let profile = self.terminal_profiles.get(self.selected_terminal_profile);
        let shell_label = short_shell_name(profile);
        compute_terminal_header_layout(
            panel_x,
            panel_y,
            panel_width,
            cell_width,
            tab_labels,
            &shell_label,
            &self.terminal_names,
            self.terminal_tab_offset,
        )
    }

    fn header_tab_label(&self, tab: TerminalHeaderTab) -> &str {
        match tab {
            TerminalHeaderTab::Problems => self.t("terminal.tab.problems"),
            TerminalHeaderTab::Output => self.t("terminal.tab.output"),
            TerminalHeaderTab::DebugConsole => self.t("terminal.tab.debug_console"),
            TerminalHeaderTab::Terminal => self.t("terminal.tab.terminal"),
            TerminalHeaderTab::Ports => self.t("terminal.tab.ports"),
            TerminalHeaderTab::Comments => self.t("terminal.tab.comments"),
        }
    }

    pub(crate) fn draw_terminal_panel(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        if !self.view_preferences.panel_visible {
            return;
        }
        let bounds = self.panel_bounds(width, height, line_height);
        let panel_x = bounds.x;
        let panel_width = bounds.width;
        let panel_height = bounds.height;
        let top = bounds.y;

        renderer.draw_rect(panel_x, top, panel_width, panel_height, rgb(0x181818));
        renderer.draw_rect(panel_x, top, panel_width, 1.0, rgb(0x2b2b2b));

        let layout = self.terminal_header_layout(panel_x, top, panel_width, cell_width);
        let active_tab = if self.terminal_problems {
            TerminalHeaderTab::Problems
        } else {
            TerminalHeaderTab::Terminal
        };
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        let text_y = top + ((TERMINAL_HEADER_HEIGHT - line_height) * 0.5).max(2.0);

        for rect in &layout.tabs {
            let is_active = rect.item == active_tab;
            let is_hovered = rect.contains(mx, my);
            if is_active {
                draw_pill_background(renderer, rect.x, rect.y, rect.w, rect.h, rgb(0x37373d));
            } else if is_hovered {
                draw_pill_background(renderer, rect.x, rect.y, rect.w, rect.h, rgb(0x2a2d2e));
            }
            renderer.draw_text(
                self.header_tab_label(rect.item),
                rect.x + 8.0,
                text_y,
                rgb(if is_active {
                    0xffffff
                } else if is_hovered {
                    0xcccccc
                } else {
                    0x9d9d9d
                }),
            );
        }

        for rect in &layout.actions {
            let is_hovered = rect.contains(mx, my);
            if is_hovered {
                draw_pill_background(renderer, rect.x, rect.y, rect.w, rect.h, rgb(0x2a2d2e));
            }
            let is_shell_indicator = rect.x == layout.shell_rect.x && rect.w == layout.shell_rect.w;
            if is_shell_indicator {
                draw_shell_badge_icon(
                    renderer,
                    rect.x + 4.0,
                    top + (TERMINAL_HEADER_HEIGHT - 11.0) * 0.5,
                    rgb(0xcccccc),
                );
                renderer.draw_text(&layout.shell_label, rect.x + 22.0, text_y, rgb(0xcccccc));
                continue;
            }
            draw_toolbar_action(
                renderer,
                rect,
                top,
                text_y,
                line_height,
                &self.terminal_names,
                self.active_terminal,
            );
        }

        renderer.draw_rect(layout.divider_x, top + 9.0, 1.0, 14.0, rgb(0x454545));

        let body_top = top + TERMINAL_HEADER_HEIGHT + 4.0;
        let body_height = (panel_height - TERMINAL_HEADER_HEIGHT - 4.0).max(0.0);

        if self.terminal_problems {
            let diagnostics = self.diagnostic_entries();
            if diagnostics.is_empty() {
                renderer.draw_text(
                    self.t("terminal.no_diagnostics"),
                    panel_x + TERMINAL_GUTTER_WIDTH,
                    body_top,
                    rgb(0x858585),
                );
            }
            let visible_rows = (body_height / line_height).max(0.0) as usize;
            for (index, (document, diagnostic)) in diagnostics.iter().take(visible_rows).enumerate()
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
                    panel_x + TERMINAL_GUTTER_WIDTH,
                    body_top + line_height * index as f32,
                    rgb(0xd4d4d4),
                );
            }
        } else if let Some(status) = self.terminal_status.as_deref() {
            renderer.draw_text(
                status,
                panel_x + TERMINAL_GUTTER_WIDTH,
                body_top,
                rgb(0xf14c4c),
            );
        } else if !self.task_output.is_empty() {
            let visible = (body_height / line_height).max(0.0) as usize;
            let start = self.task_output.len().saturating_sub(visible);
            for (index, line) in self.task_output.iter().skip(start).enumerate() {
                let clean: String = line
                    .chars()
                    .map(|ch| if ch.is_control() { ' ' } else { ch })
                    .take(
                        (((panel_width - TERMINAL_GUTTER_WIDTH - PAD) / cell_width).max(1.0))
                            as usize,
                    )
                    .collect();
                renderer.draw_text(
                    &clean,
                    panel_x + TERMINAL_GUTTER_WIDTH,
                    body_top + line_height * index as f32,
                    rgb(0xd4d4d4),
                );
            }
        } else if !self.terminals.is_empty() {
            if let Some(second) = self
                .split_terminal
                .filter(|index| *index < self.terminals.len())
            {
                let half_w = panel_width * 0.5;
                let split_x = panel_x + half_w;
                if let Some(first) = self.terminals.get(self.active_terminal) {
                    draw_terminal_snapshot(
                        renderer,
                        &first.snapshot(),
                        panel_x,
                        half_w,
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
                        split_x,
                        panel_width - half_w,
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
                    panel_x,
                    panel_width,
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

fn draw_pill_background(renderer: &mut Renderer, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w > 2.0 && h > 2.0 {
        renderer.draw_rect(x, y + 1.0, w, h - 2.0, color);
        renderer.draw_rect(x + 1.0, y, w - 2.0, h, color);
    } else {
        renderer.draw_rect(x, y, w, h, color);
    }
}

fn draw_outline_box(renderer: &mut Renderer, x: f32, y: f32, w: f32, h: f32, color: Color) {
    renderer.draw_rect(x, y, w, 1.0, color);
    renderer.draw_rect(x, y + h - 1.0, w, 1.0, color);
    renderer.draw_rect(x, y, 1.0, h, color);
    renderer.draw_rect(x + w - 1.0, y, 1.0, h, color);
}

fn draw_shell_badge_icon(renderer: &mut Renderer, x: f32, y: f32, color: Color) {
    draw_outline_box(renderer, x, y, 14.0, 11.0, color);
    // Tiny `>_` inside the terminal badge.
    renderer.draw_rect(x + 3.0, y + 3.0, 1.4, 1.4, color);
    renderer.draw_rect(x + 4.4, y + 4.4, 1.4, 1.4, color);
    renderer.draw_rect(x + 3.0, y + 5.8, 1.4, 1.4, color);
    renderer.draw_rect(x + 7.0, y + 6.4, 3.5, 1.2, color);
}

fn draw_toolbar_action(
    renderer: &mut Renderer,
    rect: &HeaderHitRect<TerminalToolbarAction>,
    top: f32,
    text_y: f32,
    _line_height: f32,
    session_names: &[String],
    active_terminal: usize,
) {
    let fg = rgb(0xcccccc);
    let cx = (rect.x + rect.w * 0.5).round();
    let cy = (top + TERMINAL_HEADER_HEIGHT * 0.5).round();

    match rect.item {
        TerminalToolbarAction::CycleProfile => {
            // Downward chevron next to `+`.
            renderer.draw_rect(cx - 3.5, cy - 1.5, 1.5, 1.5, fg);
            renderer.draw_rect(cx - 2.0, cy, 1.5, 1.5, fg);
            renderer.draw_rect(cx - 0.7, cy + 1.0, 1.4, 1.5, fg);
            renderer.draw_rect(cx + 0.5, cy, 1.5, 1.5, fg);
            renderer.draw_rect(cx + 2.0, cy - 1.5, 1.5, 1.5, fg);
        }
        TerminalToolbarAction::NewTerminal => {
            renderer.draw_rect(cx - 4.5, cy - 0.5, 9.0, 1.2, fg);
            renderer.draw_rect(cx - 0.5, cy - 4.5, 1.2, 9.0, fg);
        }
        TerminalToolbarAction::SplitTerminal => {
            draw_outline_box(renderer, cx - 6.0, cy - 5.0, 12.0, 10.0, fg);
            renderer.draw_rect(cx - 0.5, cy - 4.0, 1.2, 8.0, fg);
        }
        TerminalToolbarAction::KillTerminal => {
            // Handle + lid + 3-wall body + vertical ribs.
            renderer.draw_rect(cx - 1.5, cy - 5.5, 3.0, 1.2, fg);
            renderer.draw_rect(cx - 4.5, cy - 4.0, 9.0, 1.2, fg);
            renderer.draw_rect(cx - 3.5, cy - 2.5, 1.2, 7.5, fg);
            renderer.draw_rect(cx + 2.3, cy - 2.5, 1.2, 7.5, fg);
            renderer.draw_rect(cx - 3.5, cy + 3.8, 7.0, 1.2, fg);
            renderer.draw_rect(cx - 1.3, cy - 1.2, 1.0, 4.0, fg);
            renderer.draw_rect(cx + 0.3, cy - 1.2, 1.0, 4.0, fg);
        }
        TerminalToolbarAction::MoreActions => {
            renderer.draw_rect(cx - 5.0, cy - 1.0, 2.0, 2.0, fg);
            renderer.draw_rect(cx - 1.0, cy - 1.0, 2.0, 2.0, fg);
            renderer.draw_rect(cx + 3.0, cy - 1.0, 2.0, 2.0, fg);
        }
        TerminalToolbarAction::ToggleMaximize => {
            // Four corner L-brackets (`[ ]` expand icon).
            let ox = cx - 5.0;
            let oy = cy - 5.0;
            renderer.draw_rect(ox, oy, 3.5, 1.2, fg);
            renderer.draw_rect(ox, oy, 1.2, 3.5, fg);
            renderer.draw_rect(ox + 6.5, oy, 3.5, 1.2, fg);
            renderer.draw_rect(ox + 8.8, oy, 1.2, 3.5, fg);
            renderer.draw_rect(ox, oy + 8.8, 3.5, 1.2, fg);
            renderer.draw_rect(ox, oy + 6.5, 1.2, 3.5, fg);
            renderer.draw_rect(ox + 6.5, oy + 8.8, 3.5, 1.2, fg);
            renderer.draw_rect(ox + 8.8, oy + 6.5, 1.2, 3.5, fg);
        }
        TerminalToolbarAction::ClosePanel => {
            for step in -3..=3 {
                let offset = step as f32 * 1.25;
                renderer.draw_rect(cx + offset - 0.7, cy + offset - 0.7, 1.5, 1.5, fg);
                renderer.draw_rect(cx + offset - 0.7, cy - offset - 0.7, 1.5, 1.5, fg);
            }
        }
        TerminalToolbarAction::PrevSessionTabs => {
            renderer.draw_text("<", rect.x + 4.0, text_y, fg);
        }
        TerminalToolbarAction::NextSessionTabs => {
            renderer.draw_text(">", rect.x + 4.0, text_y, fg);
        }
        TerminalToolbarAction::SelectSession(index) => {
            if let Some(name) = session_names.get(index) {
                let short: String = name.chars().take(14).collect();
                let active = index == active_terminal;
                if active {
                    draw_pill_background(renderer, rect.x, rect.y, rect.w, rect.h, rgb(0x2d2d30));
                }
                renderer.draw_text(
                    &short,
                    rect.x + 6.0,
                    text_y,
                    rgb(if active { 0xffffff } else { 0x9d9d9d }),
                );
            }
        }
    }
}

fn draw_gutter_circle(renderer: &mut Renderer, x: f32, y: f32, color: Color) {
    // 7x7 hollow ring using perimeter segments so any underlying panel surface stays intact.
    renderer.draw_rect(x + 2.0, y, 3.0, 1.2, color);
    renderer.draw_rect(x + 2.0, y + 5.8, 3.0, 1.2, color);
    renderer.draw_rect(x, y + 2.0, 1.2, 3.0, color);
    renderer.draw_rect(x + 5.8, y + 2.0, 1.2, 3.0, color);
    renderer.draw_rect(x + 1.0, y + 1.0, 1.2, 1.2, color);
    renderer.draw_rect(x + 4.8, y + 1.0, 1.2, 1.2, color);
    renderer.draw_rect(x + 1.0, y + 4.8, 1.2, 1.2, color);
    renderer.draw_rect(x + 4.8, y + 4.8, 1.2, 1.2, color);
}

#[allow(clippy::too_many_arguments)]
fn draw_terminal_snapshot(
    renderer: &mut Renderer,
    snapshot: &terminal::TerminalSnapshot,
    pane_x: f32,
    pane_width: f32,
    top: f32,
    height: f32,
    line_height: f32,
    cell_width: f32,
    translator: graphite_i18n::Translator,
) {
    let left = pane_x + TERMINAL_GUTTER_WIDTH;
    let width = (pane_width - TERMINAL_GUTTER_WIDTH - 8.0).max(0.0);

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
        if x < left + width && y + line_height <= top + height {
            let block_w = cell_width.max(7.0);
            let block_h = (line_height - 2.0).max(10.0);
            renderer.draw_rect(x, y + 1.0, block_w, block_h, rgb(0xd4d4d4));
            if let Some(cell) = snapshot.rows.get(row).and_then(|cells| cells.get(column)) {
                if cell.character != ' ' && !cell.hidden && !cell.wide_spacer {
                    renderer.draw_text(&cell.character.to_string(), x, y, rgb(0x181818));
                }
            }
        }
    }

    // Shell integration: hollow status circle in the left gutter on each prompt/command row.
    let mut row_colors: Vec<Option<u32>> = vec![None; snapshot.rows.len()];
    for mark in &snapshot.marks {
        let display_row = mark.row - snapshot.top_row;
        if display_row < 0 || display_row as usize >= snapshot.rows.len() {
            continue;
        }
        let slot = &mut row_colors[display_row as usize];
        match mark.event {
            crate::terminal_shell::ShellEvent::PromptStart => {
                if slot.is_none() {
                    *slot = Some(0x858585);
                }
            }
            crate::terminal_shell::ShellEvent::CommandEnd(code) => {
                *slot = Some(match code {
                    Some(0) => 0x0dbc79,
                    Some(_) => 0xf14c4c,
                    None => 0x858585,
                });
            }
            crate::terminal_shell::ShellEvent::Cwd(_) => {}
        }
    }
    if row_colors.iter().all(Option::is_none)
        && snapshot.top_row == 0
        && snapshot
            .rows
            .first()
            .is_some_and(|cells| cells.iter().any(|cell| cell.character != ' '))
    {
        if let Some(first) = row_colors.first_mut() {
            *first = Some(0x858585);
        }
    }

    let circle_x = pane_x + 8.0;
    for (display_row, color) in row_colors.into_iter().enumerate() {
        let Some(hex) = color else {
            continue;
        };
        let row_y = top + line_height * display_row as f32;
        if row_y + line_height > top + height {
            break;
        }
        let circle_y = row_y + ((line_height - 7.0) * 0.5).max(1.0);
        draw_gutter_circle(renderer, circle_x, circle_y, rgb(hex));
    }

    // Subtle right-edge scrollbar thumb.
    if pane_width > 12.0 && height > 12.0 {
        renderer.draw_rect(
            pane_x + pane_width - 5.0,
            top + 2.0,
            3.0,
            (height - 8.0).clamp(16.0, 64.0),
            rgb(0x424242),
        );
    }

    if let Some(code) = snapshot.exit_code {
        let exited = translator.format("terminal.shell_exited", &[("code", &code)]);
        renderer.draw_text(&exited, left, top, rgb(0x858585));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_non_overlapping<T: Copy + std::fmt::Debug>(rects: &[HeaderHitRect<T>]) {
        for (i, a) in rects.iter().enumerate() {
            assert!(a.w > 0.0 && a.h > 0.0, "empty rect {a:?}");
            for b in rects.iter().skip(i + 1) {
                let overlap_x = a.x < b.x + b.w && b.x < a.x + a.w;
                let overlap_y = a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(
                    !(overlap_x && overlap_y),
                    "overlapping rects: {a:?} vs {b:?}"
                );
            }
        }
    }

    #[test]
    fn header_layout_tabs_and_toolbar_do_not_overlap_in_en_or_pt_br() {
        let en = graphite_i18n::Translator::english();
        let pt = graphite_i18n::Translator::new(
            graphite_i18n::Language::parse("pt-BR").expect("pt-BR catalog exists"),
        );

        for translator in [en, pt] {
            let tab_labels = [
                (
                    TerminalHeaderTab::Problems,
                    translator.text("terminal.tab.problems"),
                ),
                (
                    TerminalHeaderTab::Output,
                    translator.text("terminal.tab.output"),
                ),
                (
                    TerminalHeaderTab::DebugConsole,
                    translator.text("terminal.tab.debug_console"),
                ),
                (
                    TerminalHeaderTab::Terminal,
                    translator.text("terminal.tab.terminal"),
                ),
                (
                    TerminalHeaderTab::Ports,
                    translator.text("terminal.tab.ports"),
                ),
                (
                    TerminalHeaderTab::Comments,
                    translator.text("terminal.tab.comments"),
                ),
            ];
            let sessions = vec!["PowerShell 1".to_owned(), "PowerShell 2".to_owned()];
            let layout = compute_terminal_header_layout(
                308.0,
                560.0,
                1100.0,
                8.0,
                tab_labels,
                "powershell",
                &sessions,
                0,
            );
            assert_eq!(layout.tabs.len(), 6);
            assert_non_overlapping(&layout.tabs);
            assert_non_overlapping(&layout.actions);

            let last_tab = layout.tabs.last().unwrap();
            let first_action = layout.actions.first().unwrap();
            assert!(
                last_tab.x + last_tab.w < first_action.x,
                "tabs overlapped actions: tab={last_tab:?}, action={first_action:?}"
            );
            let last_action = layout.actions.last().unwrap();
            assert!(last_action.x + last_action.w <= 308.0 + 1100.0);
        }
    }

    #[test]
    fn short_shell_name_extracts_clean_lowercase_executable_stem() {
        let ps = crate::config::TerminalProfile {
            name: "PowerShell".into(),
            path: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
            args: Vec::new(),
        };
        let pwsh = crate::config::TerminalProfile {
            name: "PowerShell 7".into(),
            path: "/usr/bin/pwsh".into(),
            args: Vec::new(),
        };
        let cmd = crate::config::TerminalProfile {
            name: "Command Prompt".into(),
            path: "cmd.exe".into(),
            args: Vec::new(),
        };
        assert_eq!(short_shell_name(Some(&ps)), "powershell");
        assert_eq!(short_shell_name(Some(&pwsh)), "pwsh");
        assert_eq!(short_shell_name(Some(&cmd)), "cmd");
        assert_eq!(short_shell_name(None), "shell");
    }
}
