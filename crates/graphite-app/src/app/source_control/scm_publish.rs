//! Modal dialog for publishing a local Git repository to GitHub (Public or Private).

use super::*;
use winit::keyboard::{Key, NamedKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScmPublishModal {
    pub repo_name: String,
    pub branch: String,
}

impl App {
    pub(crate) fn open_publish_repo_modal(&mut self) {
        let has_repo_scope = self
            .github_session
            .as_ref()
            .is_some_and(|s| s.scope.split_whitespace().any(|scope| scope == "repo"));
        if !has_repo_scope {
            self.status = self.t("scm.publish.sign_in_required").into();
            self.start_github_sign_in_with_scope("repo");
            return;
        }
        let repo_name = self.tree.root_name();
        let branch = self
            .scm_snapshot
            .branch
            .clone()
            .unwrap_or_else(|| "main".into());
        self.scm_publish_modal = Some(ScmPublishModal { repo_name, branch });
    }

    pub(crate) fn start_publish_repo(&mut self, is_private: bool) {
        let Some(modal) = self.scm_publish_modal.take() else {
            return;
        };
        let Some(session) = self.github_session.clone() else {
            self.status = self.t("scm.publish.sign_in_required").into();
            self.start_github_sign_in_with_scope("repo");
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let root = self.scm_workspace_root();
        let translator = self.translator;
        self.status = self.t("scm.publish.running").into();
        std::thread::Builder::new()
            .name("graphite-scm-publish".into())
            .spawn(move || {
                let outcome = crate::vcs::scm_remote::publish_github_repository(
                    &root,
                    &session.token,
                    &modal.repo_name,
                    &modal.branch,
                    is_private,
                );
                let message = match outcome {
                    Ok(url) => translator.format("scm.publish.done", &[("url", &url)]),
                    Err(error) => {
                        translator.format("scm.error.operation_failed", &[("error", &error)])
                    }
                };
                let snapshot = crate::scm::snapshot(&root);
                let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                    message,
                    snapshot: Some(snapshot),
                });
            })
            .ok();
    }

    pub(crate) fn handle_scm_publish_key(&mut self, key: &Key) -> bool {
        if self.scm_publish_modal.is_none() {
            return false;
        }
        if matches!(key, Key::Named(NamedKey::Escape)) {
            self.scm_publish_modal = None;
        }
        true
    }

    fn publish_modal_rects(
        width: f32,
        height: f32,
    ) -> (
        (f32, f32, f32, f32),
        (f32, f32, f32, f32),
        (f32, f32, f32, f32),
        (f32, f32, f32, f32),
    ) {
        let panel_w = (width * 0.62)
            .clamp(360.0, 500.0)
            .min((width - 24.0).max(0.0));
        let panel_h = 188.0;
        let x = ((width - panel_w) * 0.5).max(12.0);
        let y = ((height - panel_h) * 0.5).max(12.0);
        let btn_w = (panel_w - 32.0).max(40.0);
        let btn_h = 28.0;
        let public_rect = (x + 16.0, y + 66.0, btn_w, btn_h);
        let private_rect = (x + 16.0, y + 102.0, btn_w, btn_h);
        let cancel_rect = (x + panel_w - 112.0, y + panel_h - btn_h - 14.0, 96.0, btn_h);
        (
            (x, y, panel_w, panel_h),
            public_rect,
            private_rect,
            cancel_rect,
        )
    }

    pub(crate) fn click_scm_publish_modal(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
    ) -> bool {
        if self.scm_publish_modal.is_none() {
            return false;
        }
        let (panel, public_rect, private_rect, cancel_rect) =
            Self::publish_modal_rects(width, height);
        let in_rect =
            |r: (f32, f32, f32, f32)| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3;
        if in_rect(public_rect) {
            self.start_publish_repo(false);
        } else if in_rect(private_rect) {
            self.start_publish_repo(true);
        } else if in_rect(cancel_rect) || !in_rect(panel) {
            self.scm_publish_modal = None;
        }
        true
    }

    pub(crate) fn draw_scm_publish_modal(&self, renderer: &mut Renderer) {
        let Some(modal) = &self.scm_publish_modal else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let ((x, y, panel_w, panel_h), public_rect, private_rect, cancel_rect) =
            Self::publish_modal_rects(width, height);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let primary_bg = rgb(Theme::color(Theme::STATUS, 0x0e639c));
        let secondary_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let btn_fg = rgb(0xffffff);

        renderer.draw_rect(x, y, panel_w, panel_h, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_w - 2.0).max(0.0),
            (panel_h - 2.0).max(0.0),
            bg,
        );
        renderer.draw_text(self.t("scm.publish.title"), x + 16.0, y + 14.0, text);
        let subtitle = self.tf("scm.publish.subtitle", &[("name", &modal.repo_name)]);
        let clipped = Self::help_menu_visible_label(&subtitle, cw, (panel_w - 32.0).max(0.0));
        renderer.draw_text(&clipped, x + 16.0, y + 40.0, dim);

        for (rect, key, bg_color, fg_color) in [
            (public_rect, "scm.publish.public", primary_bg, btn_fg),
            (private_rect, "scm.publish.private", primary_bg, btn_fg),
            (cancel_rect, "scm.publish.cancel", secondary_bg, text),
        ] {
            renderer.draw_rect(rect.0, rect.1, rect.2, rect.3, bg_color);
            let label = self.t(key);
            let tx = rect.0 + (rect.2 - label.chars().count() as f32 * cw).max(0.0) * 0.5;
            renderer.draw_text(label, tx, rect.1 + 5.0, fg_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn publish_modal_opens_when_signed_in_and_dismisses_on_escape() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.github_session = Some(graphite_auth::Session {
            token: "test-token".into(),
            login: "tester".into(),
            name: None,
            scope: "repo".into(),
        });
        app.scm_snapshot.branch = Some("master".into());

        app.open_publish_repo_modal();
        let modal = app.scm_publish_modal.as_ref().expect("modal opened");
        assert_eq!(modal.branch, "master");

        assert!(app.handle_scm_publish_key(&Key::Named(NamedKey::Escape)));
        assert!(app.scm_publish_modal.is_none());
    }
}
