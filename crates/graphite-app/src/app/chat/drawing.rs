//! GPU rendering for the Chat panel, Quick Chat, Inline Chat, and Provider Setup dialog.

use super::layout::{ComposerMenu, Rect};
use super::ui::chat_session_badge;
use super::*;
use graphite_chat::TurnStatus;

impl App {
    pub(crate) fn draw_chat_overlays(&self, r: &mut Renderer, w: f32, h: f32, lh: f32) {
        let cw = r.char_width();
        if self.chat.visible {
            self.draw_chat_panel(r, w, h, lh, cw);
        }
        if let Some(inline) = &self.chat.inline_chat {
            self.draw_inline_chat(r, w, h, lh, cw, inline);
        }
        if let Some(dialog) = &self.chat.provider_dialog {
            self.draw_provider_dialog(r, w, h, lh, cw, dialog);
        }
    }

    fn draw_chat_panel(&self, r: &mut Renderer, w: f32, h: f32, lh: f32, cw: f32) {
        let layout = self.chat_panel_layout(w, h, lh);
        let bounds = layout.bounds;
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }
        r.set_clip(Some([
            bounds.x,
            bounds.y,
            bounds.x + bounds.w,
            bounds.bottom(),
        ]));
        fill(r, bounds, Theme::color(Theme::SIDEBAR, 0xf3f3f3));
        r.draw_rect(
            bounds.x,
            bounds.y,
            1.0,
            bounds.h,
            rgb(Theme::color(0x3f3f46, 0xd0d0d0)),
        );
        let session = self.chat.active_session();
        let connection = self.chat.active_connection();
        let running = self.chat.running_turn.is_some();
        let read_only = connection.is_none() && !session.turns.is_empty();
        let selectable = !running
            && !self
                .chat
                .inline_chat
                .as_ref()
                .is_some_and(|chat| chat.running);
        let buttons = [
            (
                layout.toolbar[0],
                self.t("chat.new"),
                ControlStyle::enabled(selectable),
            ),
            (
                layout.toolbar[1],
                self.t("chat.history"),
                if self.chat.show_agent_sessions {
                    ControlStyle::Selected
                } else {
                    ControlStyle::Normal
                },
            ),
        ];
        for (rect, label, style) in buttons {
            draw_control(r, rect, label, lh, cw, style);
        }
        let destination = match connection {
            Some(item) => format!(
                "▾ {} · {} · {}",
                self.t("chat.connection"),
                item.name,
                item.model
            ),
            None => self
                .t(if read_only {
                    "chat.unavailable_short"
                } else {
                    "chat.choose_connection"
                })
                .into(),
        };
        draw_control(
            r,
            layout.connection,
            &destination,
            lh,
            cw,
            ControlStyle::enabled(selectable),
        );

        if !layout.history.is_empty() {
            for (rect, item) in layout.history.iter().zip(self.chat.filtered_sessions()) {
                let label = format!("{} {}", chat_session_badge(item), item.title);
                let style = if session.id == item.id {
                    ControlStyle::Selected
                } else {
                    ControlStyle::Normal
                };
                draw_control(r, *rect, &label, lh, cw, style);
            }
        }
        if let Some([title, accept, discard]) = layout.edits {
            draw_label(r, title, self.t("chat.pending_edits"), lh, cw, foreground());
            draw_control(
                r,
                accept,
                self.t("chat.accept_all"),
                lh,
                cw,
                ControlStyle::Normal,
            );
            draw_control(
                r,
                discard,
                self.t("chat.discard_all"),
                lh,
                cw,
                ControlStyle::Normal,
            );
        }
        if session.turns.is_empty() && connection.is_none() {
            if layout.setup.bottom() <= layout.body.bottom() {
                let width = text_capacity(layout.setup_heading, cw);
                for (i, line) in wrap_text(self.t("chat.no_connection"), width)
                    .iter()
                    .take(3)
                    .enumerate()
                {
                    let x = layout.setup_heading.x
                        + ((layout.setup_heading.w - cw * line.chars().count() as f32) / 2.0)
                            .max(0.0);
                    r.draw_text(
                        line,
                        x,
                        layout.setup_heading.y + i as f32 * lh,
                        foreground(),
                    );
                }
                draw_control(
                    r,
                    layout.setup,
                    self.t("chat.configure_first"),
                    lh,
                    cw,
                    ControlStyle::Primary,
                );
            }
        } else {
            self.draw_chat_messages(r, layout.body, lh, cw, read_only);
        }
        if let (Some((title, buttons)), Some((pending, _))) =
            (layout.approval, &self.chat.pending_approval)
        {
            let label = format!("{} {}", self.t("chat.approval"), pending.summary);
            draw_label(r, title, &label, lh, cw, foreground());
            for (i, label) in ["chat.allow_once", "chat.allow_always", "chat.deny"]
                .into_iter()
                .enumerate()
            {
                if i != 1 || self.chat_allows_always() {
                    draw_control(r, buttons[i], self.t(label), lh, cw, ControlStyle::Normal);
                }
            }
        }

        if layout.composer.y >= layout.connection.bottom() {
            let enabled = connection.is_some() && !running;
            let mode = format!(
                "▾ {}  {}",
                self.t("chat.mode"),
                self.t(mode_label(session.mode))
            );
            draw_control(
                r,
                layout.mode,
                &mode,
                lh,
                cw,
                ControlStyle::enabled(enabled),
            );
            draw_control(
                r,
                layout.context,
                self.t("chat.attach_context"),
                lh,
                cw,
                ControlStyle::enabled(enabled),
            );
            for &(index, rect) in &layout.attachments {
                let label = format!("× {}", self.t(ATTACHMENT_LABELS[index]));
                draw_control(r, rect, &label, lh, cw, ControlStyle::Selected);
            }
            let prompt = if read_only {
                self.t("chat.read_only")
            } else {
                self.t("chat.compose_placeholder")
            };
            let (label, color) = if self.chat.input.is_empty() {
                (prompt.to_owned(), muted())
            } else {
                let caret = if self.chat.focused && enabled && self.chat.composer_menu.is_none() {
                    "|"
                } else {
                    ""
                };
                let input = format!("{}{caret}", self.chat.input);
                let skip = input
                    .chars()
                    .count()
                    .saturating_sub(text_capacity(layout.prompt, cw));
                (input.chars().skip(skip).collect(), foreground())
            };
            paint_control(
                r,
                layout.prompt,
                if self.chat.focused && enabled {
                    ControlStyle::Focused
                } else {
                    ControlStyle::Field
                },
            );
            draw_label(r, layout.prompt, &label, lh, cw, color);
            let send_enabled = enabled && !self.chat.input.trim().is_empty();
            draw_control(
                r,
                layout.send,
                self.t(if running { "chat.stop" } else { "chat.send" }),
                lh,
                cw,
                if running || send_enabled {
                    ControlStyle::Primary
                } else {
                    ControlStyle::Disabled
                },
            );
        }
        if let (Some(menu), Some(rows)) = (self.chat.composer_menu, layout.menu) {
            let labels = match menu {
                ComposerMenu::Mode => MODE_LABELS,
                ComposerMenu::Context => ATTACHMENT_LABELS,
            };
            for (i, rect) in rows.into_iter().enumerate() {
                let label = if menu == ComposerMenu::Context && self.chat.attachments.selected()[i]
                {
                    format!("✓ {}", self.t(labels[i]))
                } else {
                    self.t(labels[i]).into()
                };
                draw_control(
                    r,
                    rect,
                    &label,
                    lh,
                    cw,
                    if i == self.chat.composer_choice {
                        ControlStyle::Selected
                    } else {
                        ControlStyle::Normal
                    },
                );
            }
        }
        r.set_clip(None);
    }

    pub(super) fn chat_message_lines(
        &self,
        capacity: usize,
        read_only: bool,
    ) -> Vec<(String, Color, bool)> {
        let mut lines = Vec::new();
        if read_only {
            lines.extend(
                wrap_text(self.t("chat.connection_unavailable"), capacity)
                    .into_iter()
                    .map(|line| (line, muted(), false)),
            );
            lines.push((String::new(), muted(), false));
        }
        for turn in &self.chat.active_session().turns {
            lines.push((self.t("chat.you").into(), accent_text(), true));
            lines.extend(
                wrap_text(&turn.prompt, capacity)
                    .into_iter()
                    .map(|line| (line, foreground(), true)),
            );
            lines.push((String::new(), muted(), false));
            lines.push((self.t("chat.assistant").into(), accent_text(), false));
            let reply = turn.assistant_text();
            if !reply.is_empty() {
                lines.extend(
                    wrap_text(&reply, capacity)
                        .into_iter()
                        .map(|line| (line, foreground(), false)),
                );
            }
            let status = match &turn.status {
                TurnStatus::Streaming => Some((self.t("chat.streaming").to_owned(), muted())),
                TurnStatus::WaitingApproval => {
                    Some((self.t("chat.waiting_approval").to_owned(), accent_text()))
                }
                TurnStatus::Error { message } => Some((
                    format!("{} {message}", self.t("chat.error_prefix")),
                    error_color(),
                )),
                TurnStatus::Cancelled => Some((self.t("chat.cancelled").to_owned(), muted())),
                TurnStatus::Done
                    if self.chat.show_context_usage
                        && (turn.usage.input_tokens > 0 || turn.usage.output_tokens > 0) =>
                {
                    Some((
                        format!(
                            "{} {} / {}",
                            self.t("chat.tokens"),
                            turn.usage.input_tokens,
                            turn.usage.output_tokens
                        ),
                        muted(),
                    ))
                }
                TurnStatus::Done => None,
            };
            if let Some((status, color)) = status {
                lines.extend(
                    wrap_text(&status, capacity)
                        .into_iter()
                        .map(|line| (line, color, false)),
                );
            }
            lines.push((String::new(), muted(), false));
        }

        lines
    }

    fn draw_chat_messages(&self, r: &mut Renderer, body: Rect, lh: f32, cw: f32, read_only: bool) {
        let lines = self.chat_message_lines(text_capacity(body, cw), read_only);
        let rows = (body.h / lh).floor() as usize;
        let end = lines.len().saturating_sub(
            self.chat
                .transcript_offset
                .min(lines.len().saturating_sub(rows)),
        );
        let start = end.saturating_sub(rows);
        for (i, (line, color, user)) in lines[start..end].iter().enumerate() {
            let row = body.row(body.y + i as f32 * lh, lh);
            if *user {
                fill(r, row, Theme::color(Theme::ACTIVE_ROW, 0xe4eef6));
            }
            draw_label(r, row, line, lh, cw, *color);
        }
    }

    fn draw_inline_chat(
        &self,
        r: &mut Renderer,
        w: f32,
        h: f32,
        lh: f32,
        cw: f32,
        inline: &super::InlineChatState,
    ) {
        let rect = self.focused_group_rect(w, h, lh);
        let ix = rect.x + PAD * 2.0;
        let iy = rect.y + lh * 2.0;
        let iw = (rect.width - PAD * 4.0).max(260.0);
        let ih = lh
            * if inline.proposed_replacement.is_some() {
                6.5
            } else {
                3.5
            };
        let max_chars = ((iw - PAD * 2.0) / cw.max(1.0)).max(16.0) as usize;
        let text_color = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));

        r.draw_rect(ix, iy, iw, ih, rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)));
        r.draw_rect(ix, iy, iw, 2.0, rgb(Theme::color(Theme::STATUS, 0x007acc)));
        r.draw_text(
            &truncate_chars(&format!("Inline Chat: {}|", inline.prompt), max_chars),
            ix + PAD,
            iy + PAD * 0.5,
            text_color,
        );
        let status = if inline.running {
            "Generating...".to_owned()
        } else if let Some(err) = &inline.error {
            format!("Error: {err}")
        } else if let Some(rep) = &inline.proposed_replacement {
            rep.lines().next().unwrap_or("").to_owned()
        } else {
            "Enter to ask, Esc to close".to_owned()
        };
        r.draw_text(
            &truncate_chars(&status, max_chars),
            ix + PAD,
            iy + lh * 1.4,
            text_color,
        );
        if inline.proposed_replacement.is_some() {
            r.draw_text(
                "[Keep (Ctrl+Enter)]   [Continue in Chat]   [Close (Esc)]",
                ix + PAD,
                iy + ih - lh * 1.2,
                rgb(Theme::color(Theme::STATUS, 0x007acc)),
            );
        }
    }
}

pub(super) const ATTACHMENT_LABELS: [&str; 3] = [
    "chat.attach_file",
    "chat.attach_selection",
    "chat.attach_diagnostics",
];
pub(super) const MODE_LABELS: [&str; 3] = ["chat.ask", "chat.edit", "chat.agent"];

pub(super) fn mode_label(mode: graphite_chat::AgentMode) -> &'static str {
    MODE_LABELS[match mode {
        graphite_chat::AgentMode::Ask => 0,
        graphite_chat::AgentMode::Edit => 1,
        graphite_chat::AgentMode::Agent => 2,
    }]
}

#[derive(Clone, Copy)]
pub(super) enum ControlStyle {
    Normal,
    Primary,
    Disabled,
    Selected,
    Field,
    Focused,
}

impl ControlStyle {
    pub fn enabled(enabled: bool) -> Self {
        if enabled {
            Self::Normal
        } else {
            Self::Disabled
        }
    }
}

pub(super) fn fill(r: &mut Renderer, rect: Rect, color: u32) {
    if rect.w > 0.0 && rect.h > 0.0 {
        r.draw_rect(rect.x, rect.y, rect.w, rect.h, rgb(color));
    }
}

pub(super) fn foreground() -> Color {
    rgb(Theme::color(Theme::TEXT, 0x1e1e1e))
}
pub(super) fn muted() -> Color {
    rgb(Theme::color(0xa9a9a9, 0x595959))
}
pub(super) fn accent_text() -> Color {
    rgb(Theme::color(0x79c0ff, 0x18588a))
}
pub(super) fn error_color() -> Color {
    rgb(Theme::color(0xffa0a0, 0x9c2525))
}

pub(super) fn text_capacity(rect: Rect, cw: f32) -> usize {
    ((rect.w - PAD * 2.0) / cw.max(1.0)).floor().max(1.0) as usize
}

pub(super) fn draw_label(
    r: &mut Renderer,
    rect: Rect,
    label: &str,
    lh: f32,
    cw: f32,
    color: Color,
) {
    r.draw_text(
        &truncate_chars(label, text_capacity(rect, cw)),
        rect.x + PAD,
        rect.y + (rect.h - lh) * 0.5,
        color,
    );
}

pub(super) fn paint_control(r: &mut Renderer, rect: Rect, style: ControlStyle) {
    let border = match style {
        ControlStyle::Primary | ControlStyle::Focused => Theme::color(0x539ee3, 0x1767a1),
        ControlStyle::Selected => Theme::color(0x627d96, 0x7898b2),
        _ => Theme::color(0x48484d, 0xb6b9bd),
    };
    let bg = match style {
        ControlStyle::Primary => Theme::color(0x15578b, 0x15578b),
        ControlStyle::Selected => Theme::color(0x303f4d, 0xd9e9f6),
        ControlStyle::Field | ControlStyle::Focused => Theme::color(Theme::BG, 0xffffff),
        ControlStyle::Disabled => Theme::color(0x282829, 0xebedef),
        ControlStyle::Normal => Theme::color(0x323236, 0xf8f8f8),
    };
    fill(r, rect, border);
    fill(
        r,
        Rect::new(rect.x + 1.0, rect.y + 1.0, rect.w - 2.0, rect.h - 2.0),
        bg,
    );
}

pub(super) fn draw_control(
    r: &mut Renderer,
    rect: Rect,
    label: &str,
    lh: f32,
    cw: f32,
    style: ControlStyle,
) {
    paint_control(r, rect, style);
    let color = match style {
        ControlStyle::Primary => rgb(0xffffff),
        ControlStyle::Disabled => muted(),
        _ => foreground(),
    };
    draw_label(r, rect, label, lh, cw, color);
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

pub(super) fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let width = max_chars.max(1);
    let mut out = Vec::new();
    for mut line in text.lines() {
        while let Some((limit, _)) = line.char_indices().nth(width) {
            let prefix = &line[..limit];
            let cut = prefix
                .rfind(char::is_whitespace)
                .filter(|&space| space >= limit / 2 && !prefix[..space].trim().is_empty())
                .unwrap_or(limit);
            out.push(line[..cut].to_owned());
            line = if cut < limit {
                line[cut..].trim_start()
            } else {
                &line[cut..]
            };
        }
        out.push(line.to_owned());
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}
