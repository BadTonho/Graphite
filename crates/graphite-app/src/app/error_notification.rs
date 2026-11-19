//! Bottom-right error notification card with View Full Error and Report Error actions.

use super::error_log::{ErrorNotificationItem, ErrorSeverity};
use super::theme::Theme;
use crate::App;
use graphite_render::{rgb, Renderer};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ToastRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl ToastRect {
    pub(crate) fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

#[derive(Debug, Default)]
pub(crate) struct ErrorNotificationState {
    pub active: Option<ErrorNotificationItem>,
    pub queued: VecDeque<ErrorNotificationItem>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ErrorNotificationLayout {
    pub card_rect: ToastRect,
    pub close_rect: ToastRect,
    pub view_full_rect: ToastRect,
    pub report_issue_rect: ToastRect,
    pub message_x: f32,
    pub message_y: f32,
    pub max_chars_per_line: usize,
}

fn severity_accent(severity: ErrorSeverity) -> u32 {
    match severity {
        ErrorSeverity::Critical => 0xe51400,
        ErrorSeverity::High => 0xf14c4c,
        ErrorSeverity::Medium => 0xcca700,
        ErrorSeverity::Low => 0x3794ff,
    }
}

fn severity_label(app: &App, severity: ErrorSeverity) -> &str {
    let key = match severity {
        ErrorSeverity::Critical => "error_notification.severity.critical",
        ErrorSeverity::High => "error_notification.severity.high",
        ErrorSeverity::Medium => "error_notification.severity.medium",
        ErrorSeverity::Low => "error_notification.severity.low",
    };
    app.t(key)
}

pub(crate) fn error_notification_layout(
    window_w: f32,
    window_h: f32,
    status_bar_visible: bool,
    line_height: f32,
    char_width: f32,
) -> ErrorNotificationLayout {
    let cw = char_width.max(7.0);
    let lh = line_height.max(16.0);
    let card_w = (window_w - 32.0).clamp(260.0, 410.0);
    let card_h = (lh * 6.2).clamp(112.0, 148.0);
    let status_h = if status_bar_visible { lh } else { 0.0 };
    let card_x = (window_w - card_w - 16.0).max(8.0);
    let card_y = (window_h - status_h - card_h - 14.0).max(8.0);

    let close_size = 20.0;
    let close_rect = ToastRect {
        x: card_x + card_w - close_size - 8.0,
        y: card_y + 8.0,
        w: close_size,
        h: close_size,
    };

    let btn_h = (lh + 8.0).max(24.0);
    let btn_y = card_y + card_h - btn_h - 10.0;
    let btn_gap = 8.0;
    let inner_w = card_w - 24.0;
    let btn_w = ((inner_w - btn_gap) * 0.5).max(96.0);

    let view_full_rect = ToastRect {
        x: card_x + 12.0,
        y: btn_y,
        w: btn_w,
        h: btn_h,
    };
    let report_issue_rect = ToastRect {
        x: view_full_rect.x + btn_w + btn_gap,
        y: btn_y,
        w: btn_w,
        h: btn_h,
    };

    ErrorNotificationLayout {
        card_rect: ToastRect {
            x: card_x,
            y: card_y,
            w: card_w,
            h: card_h,
        },
        close_rect,
        view_full_rect,
        report_issue_rect,
        message_x: card_x + 12.0,
        message_y: card_y + lh + 12.0,
        max_chars_per_line: ((card_w - 24.0) / cw).floor().max(24.0) as usize,
    }
}

fn wrap_preview_lines(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let single_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.is_empty() {
        return vec!["Unknown error".to_owned()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in single_line.split(' ') {
        let word_len = word.chars().count();
        let cur_len = current.chars().count();
        if !current.is_empty() && cur_len + 1 + word_len > max_chars {
            lines.push(current);
            current = String::new();
            if lines.len() >= max_lines {
                break;
            }
        }
        if current.is_empty() {
            if word_len > max_chars {
                let truncated: String = word.chars().take(max_chars.saturating_sub(1)).collect();
                current = format!("{truncated}…");
            } else {
                current.push_str(word);
            }
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    } else if lines.len() == max_lines && !current.is_empty() {
        if let Some(last) = lines.last_mut() {
            let trimmed: String = last.chars().take(max_chars.saturating_sub(1)).collect();
            *last = format!("{trimmed}…");
        }
    }
    lines
}

/// Shortens a native overlay (`[x, y, width, height]`) so it ends above `card`. Native webview
/// surfaces are drawn over everything the renderer paints, so without this they would hide the
/// card. An overlay that does not overlap the card horizontally, or that starts below its top, is
/// returned unchanged; one with no room above it keeps a single row of pixels.
pub(crate) fn clip_overlay_above(bounds: [f64; 4], card: ToastRect) -> [f64; 4] {
    let [x, y, w, h] = bounds;
    let (card_x, card_y) = (f64::from(card.x), f64::from(card.y));
    let overlaps_horizontally = x < card_x + f64::from(card.w) && x + w > card_x;
    let reaches_card = y + h > card_y;
    if !overlaps_horizontally || !reaches_card {
        return bounds;
    }
    [x, y, w, (card_y - 4.0 - y).max(1.0)]
}

impl App {
    /// `bounds` made to leave room for the visible error notification, if any.
    pub(crate) fn clip_overlay_for_notification(&self, bounds: [f64; 4]) -> [f64; 4] {
        let (Some(gpu), Some(_)) = (&self.gpu, &self.error_notifications.active) else {
            return bounds;
        };
        let (width, height) = gpu.renderer.size();
        let layout = error_notification_layout(
            width,
            height,
            self.view_preferences.status_bar_visible && !self.zen_mode,
            gpu.renderer.line_height(),
            gpu.renderer.char_width(),
        );
        clip_overlay_above(bounds, layout.card_rect)
    }

    pub(crate) fn push_error_notification(&mut self, item: ErrorNotificationItem) {
        if self.error_notifications.active.is_none() {
            self.error_notifications.active = Some(item);
        } else {
            self.error_notifications.queued.push_back(item);
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn dismiss_error_notification(&mut self) {
        self.error_notifications.active = self.error_notifications.queued.pop_front();
    }

    pub(crate) fn view_full_error_notification(&mut self, item: &ErrorNotificationItem) {
        self.new_untitled_document();
        let details = format!(
            "# Graphite Error Details\n\n- Severity: `{:?}`\n- Source: `{}`\n- Timestamp (unix ms): `{}`\n- OS: `{}`\n- Version: `{}`\n- Log File: `{}`\n\n## Message\n\n```\n{}\n```\n",
            item.severity,
            item.source,
            item.timestamp_unix_ms,
            std::env::consts::OS,
            env!("CARGO_PKG_VERSION"),
            item.log_file.display(),
            item.message
        );
        self.editor_mut().insert(&details);
        self.status = format!("Opened full error details ({})", item.source);
    }

    pub(crate) fn report_error_to_github_issues(&mut self, item: &ErrorNotificationItem) {
        let summary = format!("[{:?}] {}: {}", item.severity, item.source, item.message);
        let draft = self.build_issue_report_draft(Some(&summary));
        crate::tooling::webview::open_external_https(&draft.github_url);
        self.status = self.t("error_notification.opened_issue_page").to_owned();
    }

    pub(crate) fn error_notification_click(&mut self, x: f32, y: f32) -> bool {
        let Some(active) = self.error_notifications.active.clone() else {
            return false;
        };
        let Some((w, h, lh, cw)) = self.gpu.as_ref().map(|gpu| {
            let (w, h) = gpu.renderer.size();
            (w, h, gpu.renderer.line_height(), gpu.renderer.char_width())
        }) else {
            return false;
        };
        let layout = error_notification_layout(
            w,
            h,
            self.view_preferences.status_bar_visible && !self.zen_mode,
            lh,
            cw,
        );
        if !layout.card_rect.contains(x, y) {
            return false;
        }
        if layout.close_rect.contains(x, y) {
            self.dismiss_error_notification();
        } else if layout.view_full_rect.contains(x, y) {
            self.view_full_error_notification(&active);
            self.dismiss_error_notification();
        } else if layout.report_issue_rect.contains(x, y) {
            self.report_error_to_github_issues(&active);
            self.dismiss_error_notification();
        }
        true
    }

    pub(crate) fn draw_error_notification(&self, renderer: &mut Renderer) {
        let Some(item) = self.error_notifications.active.as_ref() else {
            return;
        };
        let (w, h) = renderer.size();
        let lh = renderer.line_height();
        let cw = renderer.char_width();
        let layout = error_notification_layout(
            w,
            h,
            self.view_preferences.status_bar_visible && !self.zen_mode,
            lh,
            cw,
        );

        let bg = rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3));
        let border = rgb(Theme::color(Theme::ACTIVE_ROW, 0xd0d0d0));
        let fg = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let fg_muted = rgb(Theme::color(Theme::DIM, 0x616161));
        let accent = rgb(severity_accent(item.severity));

        let c = layout.card_rect;
        renderer.draw_rect(c.x, c.y, c.w, c.h, border);
        renderer.draw_rect(c.x + 1.0, c.y + 1.0, c.w - 2.0, c.h - 2.0, bg);
        renderer.draw_rect(c.x + 1.0, c.y + 1.0, 4.0, c.h - 2.0, accent);

        let badge = format!("{} · {}", severity_label(self, item.severity), item.source);
        renderer.draw_text(&badge, c.x + 12.0, c.y + 8.0, accent);
        renderer.draw_text(
            "x",
            layout.close_rect.x + 6.0,
            layout.close_rect.y + 2.0,
            fg_muted,
        );

        for (idx, line) in wrap_preview_lines(&item.message, layout.max_chars_per_line, 2)
            .into_iter()
            .enumerate()
        {
            renderer.draw_text(
                &line,
                layout.message_x,
                layout.message_y + idx as f32 * lh,
                fg,
            );
        }

        let btn_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0xe4e6f1));
        let primary_bg = rgb(Theme::color(Theme::STATUS, 0x005a9e));
        let btn_fg = rgb(0xffffff);

        let v = layout.view_full_rect;
        renderer.draw_rect(v.x, v.y, v.w, v.h, btn_bg);
        let view_label = self.t("error_notification.view_full");
        renderer.draw_text(view_label, v.x + 8.0, v.y + (v.h - lh) * 0.5, fg);

        let r = layout.report_issue_rect;
        renderer.draw_rect(r.x, r.y, r.w, r.h, primary_bg);
        let report_label = self.t("error_notification.report_issue");
        renderer.draw_text(report_label, r.x + 8.0, r.y + (r.h - lh) * 0.5, btn_fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_item(source: &str, message: &str) -> ErrorNotificationItem {
        ErrorNotificationItem {
            severity: ErrorSeverity::High,
            source: source.into(),
            message: message.into(),
            timestamp_unix_ms: 1_700_000_000_000,
            log_file: PathBuf::from("logs/errors/high.jsonl"),
        }
    }

    #[test]
    fn layout_anchors_bottom_right_above_status_bar_without_overlapping_buttons() {
        let layout = error_notification_layout(1280.0, 720.0, true, 18.0, 8.0);
        assert!(layout.card_rect.x + layout.card_rect.w <= 1280.0);
        assert!(layout.card_rect.y + layout.card_rect.h <= 720.0 - 18.0);
        assert!(layout.view_full_rect.x + layout.view_full_rect.w < layout.report_issue_rect.x);
        assert!(layout.close_rect.y < layout.view_full_rect.y);
    }

    #[test]
    fn overlays_stop_above_the_notification_card_and_are_otherwise_untouched() {
        let card = ToastRect {
            x: 800.0,
            y: 600.0,
            w: 400.0,
            h: 120.0,
        };
        // A side bar column reaching the bottom is cut off above the card.
        assert_eq!(
            clip_overlay_above([900.0, 100.0, 300.0, 900.0], card),
            [900.0, 100.0, 300.0, 496.0]
        );
        // Left of the card, or ending above it: unchanged.
        let left = [0.0, 100.0, 500.0, 900.0];
        assert_eq!(clip_overlay_above(left, card), left);
        let short = [900.0, 100.0, 300.0, 400.0];
        assert_eq!(clip_overlay_above(short, card), short);
        // No room above the card: one row stays so the surface is still valid.
        assert_eq!(
            clip_overlay_above([900.0, 610.0, 300.0, 50.0], card)[3],
            1.0
        );
    }

    #[test]
    fn view_full_error_opens_untitled_tab_with_error_details() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let item = sample_item("updates.verify", "SHA-256 verification failed");

        app.view_full_error_notification(&item);
        let content = app.editor().text();
        assert!(content.contains("updates.verify"));
        assert!(content.contains("SHA-256 verification failed"));
        assert!(content.contains("high.jsonl"));
    }

    #[test]
    fn report_error_builds_github_issues_url_and_queue_advances_on_dismiss() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let first = sample_item("lsp", "server crashed");
        let second = sample_item("git", "fetch failed");

        app.push_error_notification(first.clone());
        app.push_error_notification(second);
        assert_eq!(
            app.error_notifications
                .active
                .as_ref()
                .map(|i| i.source.as_str()),
            Some("lsp")
        );

        let draft = app.build_issue_report_draft(Some(&format!(
            "[{:?}] {}: {}",
            first.severity, first.source, first.message
        )));
        assert!(draft.github_url.starts_with("https://github.com/"));
        assert!(draft.github_url.contains("/issues/new?title="));

        app.dismiss_error_notification();
        assert_eq!(
            app.error_notifications
                .active
                .as_ref()
                .map(|i| i.source.as_str()),
            Some("git")
        );
    }
}
