//! Centered dialog with the GitHub sign-in code, so it is easy to read and to copy.

use super::*;
use winit::keyboard::{Key, NamedKey};

const DIALOG_HEIGHT: f32 = 230.0;
/// Size of the code relative to the editor font.
const CODE_SCALE: f32 = 2.4;

/// The code being confirmed on github.com.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitHubCodeDialog {
    pub(crate) user_code: String,
    pub(crate) verification_uri: String,
}

/// Geometry of the dialog and its two buttons, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    x: f32,
    y: f32,
    width: f32,
    copy: (f32, f32, f32, f32),
    close: (f32, f32, f32, f32),
}

fn layout(window_width: f32, window_height: f32, char_width: f32) -> Layout {
    let width = (window_width * 0.6).clamp(360.0, 620.0).min((window_width - 24.0).max(0.0));
    let x = ((window_width - width) * 0.5).max(12.0);
    let y = ((window_height - DIALOG_HEIGHT) * 0.5).max(12.0);
    let button_height = 30.0;
    let button_y = y + DIALOG_HEIGHT - button_height - 16.0;
    let copy_width = (char_width * 18.0).max(140.0);
    let close_width = (char_width * 10.0).max(100.0);
    let copy_x = x + width - copy_width - close_width - 28.0;
    let close_x = x + width - close_width - 16.0;
    Layout {
        x,
        y,
        width,
        copy: (copy_x, button_y, copy_width, button_height),
        close: (close_x, button_y, close_width, button_height),
    }
}

fn inside(rect: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

impl App {
    pub(crate) fn github_dialog_open(&self) -> bool {
        self.github_code_dialog.is_some()
    }

    /// Puts the code on the system clipboard and says so.
    pub(crate) fn copy_github_code(&mut self) {
        let Some(dialog) = self.github_code_dialog.clone() else {
            return;
        };
        let mut clipboard = super::clipboard::SystemClipboard;
        match super::clipboard::ClipboardAccess::set_text(&mut clipboard, &dialog.user_code) {
            Ok(()) => self.status = self.tf("github.dialog.copied", &[("code", &dialog.user_code)]),
            Err(()) => self.status = self.t("github.dialog.copy_failed").into(),
        }
    }

    pub(crate) fn close_github_dialog(&mut self) {
        self.github_code_dialog = None;
    }

    /// Keys while the dialog is open: Enter copies, Escape closes. Other keys are ignored.
    pub(crate) fn handle_github_dialog_key(&mut self, key: &Key) -> bool {
        if !self.github_dialog_open() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Enter) => self.copy_github_code(),
            Key::Named(NamedKey::Escape) => self.close_github_dialog(),
            _ => {}
        }
        true
    }

    /// Clicks while the dialog is open. Clicks outside it are absorbed, so the editor does not
    /// react behind the dialog.
    pub(crate) fn handle_github_dialog_click(
        &mut self,
        x: f32,
        y: f32,
        window_width: f32,
        window_height: f32,
        char_width: f32,
    ) -> bool {
        if !self.github_dialog_open() {
            return false;
        }
        let geometry = layout(window_width, window_height, char_width);
        if inside(geometry.copy, x, y) {
            self.copy_github_code();
        } else if inside(geometry.close, x, y) {
            self.close_github_dialog();
        }
        true
    }

    pub(crate) fn draw_github_dialog(&self, renderer: &mut Renderer) {
        let Some(dialog) = &self.github_code_dialog else {
            return;
        };
        let (width, height) = renderer.size();
        let geometry = layout(width as f32, height as f32, renderer.char_width());
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let (x, y, panel_width) = (geometry.x, geometry.y, geometry.width);
        renderer.draw_rect(x, y, panel_width, DIALOG_HEIGHT, border);
        renderer.draw_rect(x + 1.0, y + 1.0, panel_width - 2.0, DIALOG_HEIGHT - 2.0, bg);
        renderer.draw_text(self.t("github.dialog.title"), x + 16.0, y + 14.0, text);
        renderer.draw_text(
            &self.tf("github.dialog.instructions", &[("uri", &dialog.verification_uri)]),
            x + 16.0,
            y + 44.0,
            dim,
        );
        // The code itself, in its own band so it stands out.
        renderer.draw_rect(x + 12.0, y + 72.0, panel_width - 24.0, 72.0, border);
        let code_height = renderer.line_height() * CODE_SCALE;
        renderer.draw_text_scaled(
            &dialog.user_code,
            x + 26.0,
            y + 72.0 + (72.0 - code_height) * 0.5,
            text,
            CODE_SCALE,
        );
        for (rect, label) in [
            (geometry.copy, self.t("github.dialog.copy")),
            (geometry.close, self.t("github.dialog.close")),
        ] {
            renderer.draw_rect(rect.0, rect.1, rect.2, rect.3, border);
            renderer.draw_rect(rect.0 + 1.0, rect.1 + 1.0, rect.2 - 2.0, rect.3 - 2.0, bg);
            renderer.draw_text(label, rect.0 + 12.0, rect.1 + 7.0, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_are_inside_the_dialog_and_hit_testing_follows_them() {
        let geometry = layout(1000.0, 800.0, 9.0);
        let dialog_right = geometry.x + geometry.width;
        for rect in [geometry.copy, geometry.close] {
            assert!(rect.0 >= geometry.x && rect.0 + rect.2 <= dialog_right);
        }
        let (cx, cy) = (geometry.copy.0 + 5.0, geometry.copy.1 + 5.0);
        assert!(inside(geometry.copy, cx, cy));
        assert!(!inside(geometry.close, cx, cy));
    }

    #[test]
    fn dialog_is_centered_and_stays_on_small_windows() {
        let wide = layout(1000.0, 800.0, 9.0);
        assert!((wide.x - (1000.0 - wide.width) / 2.0).abs() < 0.5);
        let small = layout(300.0, 200.0, 9.0);
        assert!(small.width <= 300.0 - 24.0);
        assert!(small.x >= 0.0 && small.y >= 0.0);
    }

    #[test]
    fn enter_copies_the_code_and_escape_closes_the_dialog() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.github_code_dialog = Some(GitHubCodeDialog {
            user_code: "DF22-2D91".into(),
            verification_uri: "https://github.com/login/device".into(),
        });
        assert!(app.handle_github_dialog_key(&Key::Named(NamedKey::Enter)));
        assert!(app.github_dialog_open());
        assert!(app.handle_github_dialog_key(&Key::Named(NamedKey::Escape)));
        assert!(!app.github_dialog_open());
        // Keys are not captured when no dialog is open.
        assert!(!app.handle_github_dialog_key(&Key::Named(NamedKey::Enter)));
    }
}
