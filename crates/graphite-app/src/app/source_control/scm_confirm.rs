//! The confirmation dialog of destructive source control actions (discarding changes).

use super::scm_remote_ui::RemoteOp;
use super::*;
use crate::scm_discard::DiscardTarget;

/// What a confirmed dialog does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ScmConfirmAction {
    Discard(Vec<DiscardTarget>),
    DropAllStashes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScmConfirm {
    pub message: String,
    pub action: ScmConfirmAction,
    /// Locale keys of the title and of the button that confirms.
    pub title_key: &'static str,
    pub confirm_key: &'static str,
}

type Rect = (f32, f32, f32, f32);

fn modal_rects(width: f32, height: f32) -> (Rect, Rect, Rect) {
    let panel_width = (width * 0.6)
        .clamp(340.0, 560.0)
        .min((width - 24.0).max(0.0));
    let panel_height = 128.0;
    let x = ((width - panel_width) * 0.5).max(12.0);
    let y = ((height - panel_height) * 0.5).max(12.0);
    let (button_width, button_height) = (104.0, 28.0);
    let button_y = y + panel_height - button_height - 16.0;
    let cancel_x = x + panel_width - button_width - 16.0;
    let confirm_x = cancel_x - button_width - 10.0;
    (
        (x, y, panel_width, panel_height),
        (confirm_x, button_y, button_width, button_height),
        (cancel_x, button_y, button_width, button_height),
    )
}

fn inside(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

impl App {
    /// Asks before discarding the working tree changes of `targets`.
    pub(crate) fn confirm_discard(&mut self, targets: Vec<DiscardTarget>) {
        let message = match targets.as_slice() {
            [] => {
                self.status = self.t("scm.error.nothing_to_discard").into();
                return;
            }
            [only] => {
                let name = only
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| only.path.to_string_lossy().into_owned());
                let key = if only.untracked {
                    "scm.confirm.discard_untracked"
                } else {
                    "scm.confirm.discard_file"
                };
                self.tf(key, &[("name", &name)])
            }
            many => self.tf(
                "scm.confirm.discard_many",
                &[("count", &many.len().to_string())],
            ),
        };
        self.scm_confirm = Some(ScmConfirm {
            message,
            action: ScmConfirmAction::Discard(targets),
            title_key: "scm.confirm.title",
            confirm_key: "scm.confirm.discard",
        });
    }

    /// Asks before removing every stash entry.
    pub(crate) fn confirm_drop_all_stashes(&mut self) {
        self.scm_confirm = Some(ScmConfirm {
            message: self.t("scm.confirm.drop_stashes").into(),
            action: ScmConfirmAction::DropAllStashes,
            title_key: "scm.confirm.drop_stashes_title",
            confirm_key: "scm.confirm.drop",
        });
    }

    /// The files of the Changes list as discard targets (conflicted files are left out).
    pub(crate) fn discard_all_targets(&self) -> Vec<DiscardTarget> {
        self.scm_snapshot
            .files
            .iter()
            .filter(|file| file.unstaged && !file.conflicted)
            .map(|file| DiscardTarget {
                path: file.path.clone(),
                untracked: file.untracked,
            })
            .collect()
    }

    /// The Changes list entry at `index` as a discard target, if it can be discarded.
    pub(crate) fn discard_target_at(&self, index: usize) -> Option<DiscardTarget> {
        self.scm_snapshot
            .files
            .get(index)
            .filter(|file| file.unstaged && !file.conflicted)
            .map(|file| DiscardTarget {
                path: file.path.clone(),
                untracked: file.untracked,
            })
    }

    pub(crate) fn accept_scm_confirm(&mut self) {
        let Some(confirm) = self.scm_confirm.take() else {
            return;
        };
        match confirm.action {
            ScmConfirmAction::Discard(targets) => self.run_scm_remote(RemoteOp::Discard(targets)),
            ScmConfirmAction::DropAllStashes => self.run_scm_remote(RemoteOp::StashDropAll),
        }
    }

    pub(crate) fn handle_scm_confirm_key(&mut self, key: &Key) -> bool {
        if self.scm_confirm.is_none() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Enter) => self.accept_scm_confirm(),
            Key::Named(NamedKey::Escape) => self.scm_confirm = None,
            _ => {}
        }
        true
    }

    /// A click while the dialog is open. Always swallowed; clicking outside cancels.
    pub(crate) fn click_scm_confirm(&mut self, x: f32, y: f32, width: f32, height: f32) -> bool {
        if self.scm_confirm.is_none() {
            return false;
        }
        let (panel, confirm, cancel) = modal_rects(width, height);
        if inside(confirm, x, y) {
            self.accept_scm_confirm();
        } else if inside(cancel, x, y) || !inside(panel, x, y) {
            self.scm_confirm = None;
        }
        true
    }

    pub(crate) fn draw_scm_confirm(&self, renderer: &mut Renderer) {
        let Some(confirm) = &self.scm_confirm else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let ((x, y, panel_width, panel_height), accept, cancel) = modal_rects(width, height);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, panel_width, panel_height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_width - 2.0).max(0.0),
            (panel_height - 2.0).max(0.0),
            bg,
        );
        renderer.draw_text(self.t(confirm.title_key), x + 16.0, y + 14.0, text);
        let clipped =
            Self::help_menu_visible_label(&confirm.message, cw, (panel_width - 32.0).max(0.0));
        renderer.draw_text(&clipped, x + 16.0, y + 44.0, dim);
        let button = |renderer: &mut Renderer, rect: Rect, fill: Color, label: &str, fg: Color| {
            renderer.draw_rect(rect.0, rect.1, rect.2, rect.3, fill);
            let label_x = rect.0 + (rect.2 - label.chars().count() as f32 * cw).max(0.0) * 0.5;
            renderer.draw_text(label, label_x, rect.1 + 5.0, fg);
        };
        button(
            renderer,
            accept,
            rgb(0xb42318),
            self.t(confirm.confirm_key),
            rgb(0xffffff),
        );
        button(
            renderer,
            cancel,
            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
            self.t("scm.confirm.cancel"),
            text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scm::FileStatus;
    use crate::test_support::TempDir;

    fn app(dir: &TempDir) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm_snapshot.files = vec![
            FileStatus {
                path: "a.txt".into(),
                unstaged: true,
                ..Default::default()
            },
            FileStatus {
                path: "new.txt".into(),
                unstaged: true,
                untracked: true,
                ..Default::default()
            },
            FileStatus {
                path: "clash.txt".into(),
                unstaged: true,
                conflicted: true,
                ..Default::default()
            },
            FileStatus {
                path: "staged.txt".into(),
                staged: true,
                ..Default::default()
            },
        ];
        app
    }

    #[test]
    fn only_unstaged_non_conflicted_files_can_be_discarded() {
        let dir = TempDir::new();
        let app = app(&dir);
        let all = app.discard_all_targets();
        assert_eq!(all.len(), 2);
        assert!(all[1].untracked);
        assert!(app.discard_target_at(0).is_some());
        assert!(
            app.discard_target_at(2).is_none(),
            "conflicts are resolved, not discarded"
        );
        assert!(
            app.discard_target_at(3).is_none(),
            "staged files have nothing to discard"
        );
        assert!(app.discard_target_at(9).is_none());
    }

    #[test]
    fn the_message_names_the_file_and_warns_about_deleting_untracked_ones() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.confirm_discard(vec![app.discard_target_at(0).unwrap()]);
        let tracked = app.scm_confirm.clone().unwrap().message;
        assert!(tracked.contains("a.txt"));
        app.confirm_discard(vec![app.discard_target_at(1).unwrap()]);
        let untracked = app.scm_confirm.clone().unwrap().message;
        assert!(untracked.contains("new.txt"));
        assert_ne!(tracked, untracked);
        app.confirm_discard(app.discard_all_targets());
        assert!(app.scm_confirm.clone().unwrap().message.contains('2'));
    }

    #[test]
    fn dropping_every_stash_asks_first_and_names_its_own_action() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.confirm_drop_all_stashes();
        let confirm = app.scm_confirm.clone().unwrap();
        assert_eq!(confirm.action, ScmConfirmAction::DropAllStashes);
        assert_ne!(confirm.title_key, "scm.confirm.title");
        assert_eq!(confirm.message, app.t("scm.confirm.drop_stashes"));
        app.status.clear();
        app.handle_scm_confirm_key(&Key::Named(NamedKey::Enter));
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
    }

    #[test]
    fn nothing_to_discard_shows_no_dialog() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.confirm_discard(Vec::new());
        assert!(app.scm_confirm.is_none());
        assert_eq!(app.status, app.t("scm.error.nothing_to_discard"));
    }

    #[test]
    fn escape_and_outside_clicks_cancel_and_enter_or_the_button_confirm() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        assert!(!app.handle_scm_confirm_key(&Key::Named(NamedKey::Escape)));
        assert!(!app.click_scm_confirm(1.0, 1.0, 1200.0, 760.0));

        app.confirm_discard(app.discard_all_targets());
        assert!(app.handle_scm_confirm_key(&Key::Named(NamedKey::Escape)));
        assert!(app.scm_confirm.is_none());

        app.confirm_discard(app.discard_all_targets());
        assert!(app.click_scm_confirm(1.0, 1.0, 1200.0, 760.0), "swallowed");
        assert!(app.scm_confirm.is_none(), "an outside click cancels");

        app.confirm_discard(app.discard_all_targets());
        let (_, (x, y, w, h), _) = modal_rects(1200.0, 760.0);
        app.status.clear();
        assert!(app.click_scm_confirm(x + w * 0.5, y + h * 0.5, 1200.0, 760.0));
        assert!(app.scm_confirm.is_none());
        // Without an event loop the operation stops at this message, which shows it was started.
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));

        app.confirm_discard(app.discard_all_targets());
        app.status.clear();
        assert!(app.handle_scm_confirm_key(&Key::Named(NamedKey::Enter)));
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
    }
}
