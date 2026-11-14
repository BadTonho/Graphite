//! GPU rendering for the Chat panel, Quick Chat, Inline Chat, and Provider Setup dialog.

use super::ui::{chat_session_badge, mask_api_key, provider_kind_label};
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

        // Header toolbar
        r.draw_rect(bx, by, bw, lh * 1.4, rgb(Theme::color(0x252526, 0xe8e8e8)));
        let header = format!("[{}] [+New] [Sessions] [Key] [X]", session.mode.label());
        r.draw_text(
            &truncate_chars(&header, max_chars),
            bx + PAD,
            by + PAD * 0.4,
            text_color,
        );

        let mut cursor_y = by + lh * 1.6;

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
            lh * 4.4
        } else {
            lh * 2.4
        };
        let body_bottom = by + bh - bottom_reserve;
        let mut lines: Vec<(String, Color)> = Vec::new();
        if session.turns.is_empty() {
            let model_info = self
                .chat
                .provider
                .as_ref()
                .map(|p| format!("{} ({})", provider_kind_label(p.kind), p.model))
                .unwrap_or_else(|| "No model configured — click [Key]".into());
            lines.push((format!("Chat ready: {model_info}"), dim_color));
            if self.chat.show_context_usage {
                lines.push((
                    "Context: active file, selection & diagnostics attached".into(),
                    dim_color,
                ));
            }
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
            let ay = by + bh - lh * 4.2;
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
            r.draw_text("[Allow Once]  [Always]  [Deny]", bx + PAD, ay + lh, accent);
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
        let box_w = (w * 0.56).clamp(340.0, 560.0);
        let box_h = lh * 9.0;
        let bx = (w - box_w) * 0.5;
        let by = (h - box_h) * 0.28;
        let max_chars = ((box_w - PAD * 2.0) / cw.max(1.0)).max(20.0) as usize;
        let text_color = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));

        r.draw_rect(
            bx,
            by,
            box_w,
            box_h,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        r.draw_rect(
            bx,
            by,
            box_w,
            2.0,
            rgb(Theme::color(Theme::STATUS, 0x007acc)),
        );
        r.draw_text(
            "Chat Model Provider Settings",
            bx + PAD,
            by + PAD * 0.4,
            text_color,
        );

        let fields = [
            format!("Provider (Space): {}", provider_kind_label(dialog.kind)),
            format!(
                "Base URL: {}",
                if dialog.base_url.is_empty() {
                    "<default>"
                } else {
                    &dialog.base_url
                }
            ),
            format!("Model: {}", dialog.model),
            format!("API Key: {}", mask_api_key(&dialog.api_key)),
        ];
        for (idx, label) in fields.iter().enumerate() {
            let fy = by + lh * (1.4 + idx as f32 * 1.3);
            if idx == dialog.focus_field {
                r.draw_rect(
                    bx + 4.0,
                    fy,
                    box_w - 8.0,
                    lh * 1.1,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            r.draw_text(
                &truncate_chars(label, max_chars),
                bx + PAD,
                fy + 2.0,
                text_color,
            );
        }

        r.draw_text(
            "[Save (Enter)]          [Cancel (Esc)]",
            bx + PAD,
            by + box_h - lh * 1.4,
            rgb(Theme::color(Theme::STATUS, 0x007acc)),
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
