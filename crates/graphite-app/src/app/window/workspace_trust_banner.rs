//! Top-of-window Workspace Trust banner with two prominent action buttons.

use super::*;

#[derive(Clone, Copy, Debug)]
struct BannerRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl BannerRect {
    fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

#[derive(Clone, Copy, Debug)]
struct TrustBannerLayout {
    panel: BannerRect,
    trust_btn: BannerRect,
    restrict_btn: BannerRect,
    text_x: f32,
    title_y: f32,
    subtitle_y: f32,
}

fn banner_layout(window_width: f32, top_offset: f32, lh: f32) -> TrustBannerLayout {
    let margin = 16.0;
    let width = (window_width - margin * 2.0).clamp(280.0, 660.0);
    let x = ((window_width - width) * 0.5).max(8.0);
    let y = top_offset + 8.0;
    let pad = 16.0;
    let btn_h = (lh * 2.0).max(36.0);
    let btn_gap = 12.0;
    let btn_w = ((width - pad * 2.0 - btn_gap) * 0.5).max(80.0);
    let title_y = y + pad * 0.8;
    let subtitle_y = title_y + lh * 1.3;
    let btn_y = subtitle_y + lh * 1.5;
    let height = (btn_y + btn_h + pad * 0.9) - y;

    TrustBannerLayout {
        panel: BannerRect {
            x,
            y,
            w: width,
            h: height,
        },
        trust_btn: BannerRect {
            x: x + pad,
            y: btn_y,
            w: btn_w,
            h: btn_h,
        },
        restrict_btn: BannerRect {
            x: x + pad + btn_w + btn_gap,
            y: btn_y,
            w: btn_w,
            h: btn_h,
        },
        text_x: x + pad,
        title_y,
        subtitle_y,
    }
}

impl App {
    pub(crate) fn should_show_workspace_trust_banner(&self) -> bool {
        !self.zen_mode
            && self.workspace_trust.is_some()
            && !self.vsix_workspace_trusted()
            && self.workspace_trust_dismissed_root.as_deref() != Some(self.tree.root())
    }

    pub(crate) fn draw_workspace_trust_banner(&self, renderer: &mut Renderer) {
        if !self.should_show_workspace_trust_banner() {
            return;
        }
        let (window_width, _) = renderer.size();
        let lh = renderer.line_height();
        let cw = renderer.char_width().max(1.0);
        let layout = banner_layout(window_width, self.top_menu_height(), lh);

        let border = rgb(Theme::color(Theme::STATUS, 0x007acc));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let fg = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x9d9d9d));

        renderer.draw_rect(
            layout.panel.x,
            layout.panel.y,
            layout.panel.w,
            layout.panel.h,
            border,
        );
        renderer.draw_rect(
            layout.panel.x + 1.0,
            layout.panel.y + 1.0,
            layout.panel.w - 2.0,
            layout.panel.h - 2.0,
            bg,
        );

        let max_chars = ((layout.panel.w - 32.0) / cw).max(1.0) as usize;
        let title: String = self
            .t("workspace_trust.banner.title")
            .chars()
            .take(max_chars)
            .collect();
        let subtitle: String = self
            .t("workspace_trust.banner.subtitle")
            .chars()
            .take(max_chars)
            .collect();
        renderer.draw_text(&title, layout.text_x, layout.title_y, fg);
        renderer.draw_text(&subtitle, layout.text_x, layout.subtitle_y, dim);

        let buttons = [
            (
                layout.trust_btn,
                self.t("workspace_trust.banner.trust"),
                0x0e639c,
            ),
            (
                layout.restrict_btn,
                self.t("workspace_trust.banner.restrict"),
                0x3b4252,
            ),
        ];
        for (rect, label, color) in buttons {
            renderer.draw_rect(rect.x, rect.y, rect.w, rect.h, rgb(color));
            let btn_chars = ((rect.w - 16.0) / cw).max(1.0) as usize;
            let text: String = label.chars().take(btn_chars).collect();
            let text_w = text.chars().count() as f32 * cw;
            let text_x = rect.x + ((rect.w - text_w) * 0.5).max(8.0);
            let text_y = rect.y + (rect.h - lh) * 0.5;
            renderer.draw_text(&text, text_x, text_y, rgb(0xffffff));
        }
    }

    pub(crate) fn click_workspace_trust_banner(&mut self, mx: f32, my: f32) -> bool {
        if !self.should_show_workspace_trust_banner() {
            return false;
        }
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let (window_width, _) = gpu.renderer.size();
        let lh = gpu.renderer.line_height();
        let layout = banner_layout(window_width, self.top_menu_height(), lh);
        if !layout.panel.contains(mx, my) {
            return false;
        }
        if layout.trust_btn.contains(mx, my) {
            self.run_ui_command("graphite.security.trustWorkspace");
            self.activate_secondary_plugin();
        } else if layout.restrict_btn.contains(mx, my) {
            self.workspace_trust_dismissed_root = Some(self.tree.root().to_path_buf());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_shows_when_workspace_untrusted_and_hides_after_trust_or_restrict() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().into(), None, true);
        app.workspace_trust = Some(
            crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap(),
        );
        assert!(app.should_show_workspace_trust_banner());

        app.workspace_trust_dismissed_root = Some(app.tree.root().to_path_buf());
        assert!(!app.should_show_workspace_trust_banner());

        app.workspace_trust_dismissed_root = None;
        assert!(app.should_show_workspace_trust_banner());

        app.run_ui_command("graphite.security.trustWorkspace");
        assert!(!app.should_show_workspace_trust_banner());
    }
}
