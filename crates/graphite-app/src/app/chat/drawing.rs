//! GPU rendering for the Chat panel, Quick Chat, Inline Chat, and Provider Setup dialog.

use super::ui::{chat_session_badge, mask_api_key};
use super::*;
use graphite_chat::{EditReviewStatus, TurnStatus};

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
        let (bx, by, bw, bh) = self.chat_panel_bounds(w, h, lh);
        if bw <= 0.0 || bh <= 0.0 {
            return;
        }
        r.draw_rect(bx, by, bw, bh, rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)));
        r.draw_rect(bx, by, 1.0, bh, rgb(Theme::color(0x3f3f46, 0xd0d0d0)));

        let session = self.chat.active_session();
        let text_color = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim_color = rgb(Theme::color(Theme::DIM, 0x616161));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        let max_chars = ((bw - PAD * 2.0) / cw.max(1.0)).max(12.0) as usize;

        r.draw_rect(bx, by, bw, lh * 2.0, rgb(Theme::color(0x252526, 0xe8e8e8)));
        let mode = self.t(match session.mode {
            graphite_chat::AgentMode::Ask => "chat.ask",
            graphite_chat::AgentMode::Edit => "chat.edit",
            graphite_chat::AgentMode::Agent => "chat.agent",
        });
        draw_chat_buttons(
            r,
            bx,
            by,
            bw,
            cw,
            [mode, self.t("chat.new"), self.t("chat.history")],
            text_color,
        );
        let connection = self.chat.active_connection();
        let label = if let Some(connection) = connection {
            format!("{} \u{00b7} {}", connection.name, connection.model)
        } else if session.turns.is_empty() {
            self.t("chat.configure").into()
        } else {
            self.t("chat.read_only").into()
        };
        let running = self.chat.running_turn.is_some();
        let label_width = if running {
            max_chars * 3 / 4
        } else {
            max_chars
        };
        r.draw_text(
            &truncate_chars(&label, label_width),
            bx + PAD,
            by + lh,
            accent,
        );
        if running {
            r.draw_text(self.t("chat.stop"), bx + bw * 0.75, by + lh, accent);
        }
        let mut cursor_y = by + lh * 2.25;

        // Optional agent sessions list
        if self.chat.show_agent_sessions {
            let sessions = self.chat.filtered_sessions();
            for s in sessions.iter().take(6) {
                if cursor_y + lh > by + bh - lh * 3.0 {
                    break;
                }
                if s.id == self.chat.active_session_id {
                    r.draw_rect(
                        bx + 2.0,
                        cursor_y,
                        bw - 4.0,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let label = format!("{} {}", chat_session_badge(s), s.title);
                r.draw_text(
                    &truncate_chars(&label, max_chars),
                    bx + PAD,
                    cursor_y,
                    text_color,
                );
                cursor_y += lh;
            }
            r.draw_rect(bx, cursor_y, bw, 1.0, rgb(Theme::color(0x3f3f46, 0xd0d0d0)));
            cursor_y += 4.0;
        }

        // Pending edits summary bar
        let pending_count = session
            .pending_edits
            .iter()
            .filter(|e| e.status == EditReviewStatus::Pending)
            .count();
        if pending_count > 0 {
            r.draw_rect(
                bx + 2.0,
                cursor_y,
                bw - 4.0,
                lh * 1.3,
                rgb(Theme::color(0x2a2d2e, 0xfff3cd)),
            );
            let bar = format!("{pending_count} pending edit(s): [Accept All]  [Discard All]");
            r.draw_text(
                &truncate_chars(&bar, max_chars),
                bx + PAD,
                cursor_y + 2.0,
                text_color,
            );
            cursor_y += lh * 1.4;
        }

        // Conversation lines
        let bottom_reserve = if self.chat.pending_approval.is_some() {
            lh * 5.5
        } else {
            lh * 3.5
        };
        let body_bottom = by + bh - bottom_reserve;
        let mut lines: Vec<(String, Color)> = Vec::new();
        if session.turns.is_empty() {
            let hint = if connection.is_some() {
                "chat.compose_hint"
            } else {
                "chat.no_connection"
            };
            lines.extend(
                wrap_text(self.t(hint), max_chars)
                    .into_iter()
                    .map(|line| (line, dim_color)),
            );
        } else {
            for turn in &session.turns {
                for wrapped in wrap_text(&format!("You: {}", turn.prompt), max_chars) {
                    lines.push((wrapped, accent));
                }
                let reply = turn.assistant_text();
                if !reply.is_empty() {
                    for wrapped in wrap_text(&reply, max_chars) {
                        lines.push((wrapped, text_color));
                    }
                }
                match &turn.status {
                    TurnStatus::Streaming => lines.push(("...streaming...".into(), dim_color)),
                    TurnStatus::WaitingApproval => {
                        lines.push(("Waiting for tool approval...".into(), accent))
                    }
                    TurnStatus::Error { message } => {
                        for wrapped in wrap_text(&format!("Error: {message}"), max_chars) {
                            lines.push((wrapped, rgb(0xd16969)));
                        }
                    }
                    TurnStatus::Cancelled => lines.push(("[Cancelled]".into(), dim_color)),
                    TurnStatus::Done => {
                        if self.chat.show_context_usage
                            && (turn.usage.input_tokens > 0 || turn.usage.output_tokens > 0)
                        {
                            lines.push((
                                format!(
                                    "tokens: {} in / {} out",
                                    turn.usage.input_tokens, turn.usage.output_tokens
                                ),
                                dim_color,
                            ));
                        }
                    }
                }
                lines.push((String::new(), dim_color));
            }
        }

        let visible_rows = ((body_bottom - cursor_y) / lh).floor().max(1.0) as usize;
        let max_scroll = lines.len().saturating_sub(visible_rows);
        let start = if self.chat.scroll == 0 {
            max_scroll
        } else {
            self.chat.scroll.min(max_scroll)
        };
        for (idx, (line, color)) in lines.iter().skip(start).take(visible_rows).enumerate() {
            r.draw_text(line, bx + PAD, cursor_y + idx as f32 * lh, *color);
        }

        // Pending tool approval bar
        if let Some((pending, _)) = &self.chat.pending_approval {
            let ay = by + bh - lh * 5.3;
            r.draw_rect(
                bx + 2.0,
                ay,
                bw - 4.0,
                lh * 1.9,
                rgb(Theme::color(0x332b00, 0xfff3cd)),
            );
            r.draw_text(
                &truncate_chars(&format!("Approve: {}", pending.summary), max_chars),
                bx + PAD,
                ay + 2.0,
                text_color,
            );
            let ask_read = session.mode == graphite_chat::AgentMode::Ask
                && graphite_chat::ToolApprovalPolicy::is_read_only(&pending.name);
            draw_chat_buttons(
                r,
                bx,
                ay + lh,
                bw,
                cw,
                [
                    self.t("chat.allow_once"),
                    if ask_read {
                        ""
                    } else {
                        self.t("chat.allow_always")
                    },
                    self.t("chat.deny"),
                ],
                accent,
            );
        }

        let attachments = &self.chat.attachments;
        let attach_y = by + bh - lh * 3.3;
        let selected = [
            attachments.active_file.is_some(),
            attachments.selection.is_some(),
            !attachments.diagnostics.is_empty(),
        ];
        let labels = [
            self.t("chat.attach_file"),
            self.t("chat.attach_selection"),
            self.t("chat.attach_diagnostics"),
        ];
        let col = bw / 3.0;
        for (index, label) in labels.iter().enumerate() {
            let x = bx + col * index as f32;
            let toggle = if selected[index] { "\u{2713}" } else { "+" };
            let width = ((col - PAD) / cw.max(1.0)).max(1.0) as usize;
            let label = truncate_chars(&format!("{toggle} {label}"), width);
            r.draw_text(
                &label,
                x + PAD * 0.5,
                attach_y,
                if selected[index] { accent } else { dim_color },
            );
        }
        // Input box
        let iy = by + bh - lh * 2.1;
        r.draw_rect(
            bx + 4.0,
            iy,
            bw - 8.0,
            lh * 1.8,
            rgb(Theme::color(Theme::BG, 0xffffff)),
        );
        let caret = if self.chat.focused { "|" } else { "" };
        let input_label = format!("> {}{}", self.chat.input, caret);
        let display_input = if input_label.chars().count() > max_chars {
            let skip = input_label.chars().count() - max_chars;
            input_label.chars().skip(skip).collect::<String>()
        } else {
            input_label
        };
        r.draw_text(&display_input, bx + PAD, iy + PAD * 0.4, text_color);
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

    fn draw_provider_dialog(
        &self,
        r: &mut Renderer,
        w: f32,
        h: f32,
        lh: f32,
        cw: f32,
        dialog: &super::state::ProviderDialogState,
    ) {
        let (bx, by, box_w, box_h) = super::state::ProviderDialogState::bounds(w, h, lh);
        let max_chars = ((box_w - PAD * 2.0) / cw.max(1.0)).max(1.0) as usize;
        let text_color = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        let draft = &dialog.draft;
        r.draw_rect(
            bx,
            by,
            box_w,
            box_h,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        r.draw_rect(bx, by, box_w, 2.0, accent);
        r.draw_text(self.t("chat.configure"), bx + PAD, by, text_color);
        let saved = self.chat.session_keys.contains_key(&draft.id) || draft.credential.is_some();
        let key = if !dialog.api_key.is_empty() {
            mask_api_key(&dialog.api_key)
        } else if saved {
            self.t("chat.key_saved").to_owned()
        } else {
            String::new()
        };
        let selected_name = self
            .chat
            .connections
            .get(&draft.id)
            .map(|item| item.name.as_str())
            .unwrap_or_else(|| self.t("chat.new_connection"));
        let fields = [
            format!("<  {selected_name}  >"),
            format!("{}  {}", self.t("chat.name"), draft.name),
            format!(
                "{}  < {} >",
                self.t("chat.provider"),
                if draft.provider == super::connections::Provider::Compatible {
                    self.t("chat.compatible_provider")
                } else {
                    draft.provider.label()
                }
            ),
            format!(
                "{}  {}",
                self.t("chat.base_url"),
                if draft.provider.has_fixed_endpoint() {
                    draft.provider.kind().default_base_url()
                } else if draft.provider != super::connections::Provider::Compatible
                    && draft.base_url.is_empty()
                {
                    self.t("chat.official_endpoint")
                } else {
                    &draft.base_url
                }
            ),
            format!("{}  {}", self.t("chat.model"), draft.model),
            format!("{}  {key}", self.t("chat.api_key")),
        ];
        for (idx, label) in fields.iter().enumerate() {
            let y = by + lh * (idx + 1) as f32;
            if idx == dialog.focus_field {
                r.draw_rect(
                    bx + 4.0,
                    y,
                    box_w - 8.0,
                    lh,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            r.draw_text(&truncate_chars(label, max_chars), bx + PAD, y, text_color);
        }
        draw_chat_buttons(
            r,
            bx,
            by + lh * 8.0,
            box_w,
            cw,
            [
                self.t("chat.add"),
                self.t("chat.use"),
                self.t("chat.remove"),
            ],
            accent,
        );
        draw_chat_buttons(
            r,
            bx,
            by + lh * 9.0,
            box_w,
            cw,
            [
                self.t("chat.save"),
                if dialog.offer_session_key {
                    self.t("chat.use_for_session")
                } else {
                    ""
                },
                self.t("chat.cancel"),
            ],
            accent,
        );
        if let Some(message) = &dialog.message {
            for (idx, line) in wrap_text(message, max_chars)
                .into_iter()
                .take(2)
                .enumerate()
            {
                r.draw_text(&line, bx + PAD, by + lh * (10 + idx) as f32, text_color);
            }
        }
    }
}

fn draw_chat_buttons(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    width: f32,
    cw: f32,
    labels: [&str; 3],
    color: Color,
) {
    let cell = width / 3.0;
    let chars = ((cell - PAD) / cw.max(1.0)).max(1.0) as usize;
    for (index, label) in labels.into_iter().enumerate() {
        renderer.draw_text(
            &truncate_chars(label, chars),
            x + cell * index as f32 + PAD,
            y,
            color,
        );
    }
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

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let width = max_chars.max(8);
    let mut out = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        for ch in raw_line.chars() {
            current.push(ch);
            if current.chars().count() >= width {
                out.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}
