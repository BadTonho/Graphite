//! Right-click menu of a commit in the source control Graph: check it out, branch or tag it,
//! cherry-pick or revert it, and copy its id or message.

use super::scm_branch_ui::NameTarget;
use super::scm_remote_ui::RemoteOp;
use super::*;
use crate::app::clipboard::{ClipboardAccess, SystemClipboard};
use crate::app::explorer_context_menu::place_context_menu;

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 6.0;
const MIN_MENU_WIDTH: f32 = 240.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GraphMenuAction {
    Checkout,
    CreateBranch,
    CreateTag,
    CherryPick,
    Revert,
    CopyId,
    CopyMessage,
}

impl GraphMenuAction {
    fn label_key(self) -> &'static str {
        match self {
            Self::Checkout => "scm.graph.menu.checkout",
            Self::CreateBranch => "scm.graph.menu.create_branch",
            Self::CreateTag => "scm.graph.menu.create_tag",
            Self::CherryPick => "scm.graph.menu.cherry_pick",
            Self::Revert => "scm.graph.menu.revert",
            Self::CopyId => "scm.graph.menu.copy_id",
            Self::CopyMessage => "scm.graph.menu.copy_message",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GraphMenuRow {
    Action {
        action: GraphMenuAction,
        enabled: bool,
    },
    Separator,
}

impl GraphMenuRow {
    fn is_selectable(self) -> bool {
        matches!(self, Self::Action { enabled: true, .. })
    }

    fn height(self) -> f32 {
        match self {
            Self::Separator => SEPARATOR_HEIGHT,
            Self::Action { .. } => ROW_HEIGHT,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GraphContextMenu {
    pub hash: String,
    pub subject: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub rows: Vec<GraphMenuRow>,
}

impl GraphContextMenu {
    fn height(&self) -> f32 {
        PADDING * 2.0 + self.rows.iter().map(|row| row.height()).sum::<f32>()
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height()
    }

    /// The action under the point, if it is an enabled one.
    fn action_at(&self, x: f32, y: f32) -> Option<(usize, GraphMenuAction)> {
        if !self.contains(x, y) {
            return None;
        }
        let mut top = self.y + PADDING;
        for (index, row) in self.rows.iter().enumerate() {
            if y >= top && y < top + row.height() {
                return match row {
                    GraphMenuRow::Action {
                        action,
                        enabled: true,
                    } => Some((index, *action)),
                    _ => None,
                };
            }
            top += row.height();
        }
        None
    }

    fn step(&mut self, forward: bool) {
        let len = self.rows.len();
        for offset in 1..=len {
            let candidate = if forward {
                (self.selected + offset) % len
            } else {
                (self.selected + len - offset) % len
            };
            if self.rows[candidate].is_selectable() {
                self.selected = candidate;
                return;
            }
        }
    }
}

fn menu_rows(is_head: bool) -> Vec<GraphMenuRow> {
    let action = |action, enabled| GraphMenuRow::Action { action, enabled };
    vec![
        action(GraphMenuAction::Checkout, !is_head),
        GraphMenuRow::Separator,
        action(GraphMenuAction::CreateBranch, true),
        action(GraphMenuAction::CreateTag, true),
        GraphMenuRow::Separator,
        action(GraphMenuAction::CherryPick, !is_head),
        action(GraphMenuAction::Revert, true),
        GraphMenuRow::Separator,
        action(GraphMenuAction::CopyId, true),
        action(GraphMenuAction::CopyMessage, true),
    ]
}

impl App {
    /// Right click in the sidebar: opens the commit menu when the point is on a Graph commit.
    pub(crate) fn scm_graph_right_click(&mut self, x: f32, y: f32) -> bool {
        let (window_width, window_height, lh, cw) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (w, h) = gpu.renderer.size();
                (w, h, gpu.renderer.line_height(), gpu.renderer.char_width())
            })
            .unwrap_or((1200.0, 760.0, 18.0, 8.0));
        let Some(commit) = self
            .scm_graph_commit_at(x, y, (window_width, window_height), lh, cw)
            .and_then(|index| self.scm.graph_rows.get(index))
        else {
            self.scm.graph_menu = None;
            return false;
        };
        let (hash, subject, is_head) = (
            commit.commit.hash.clone(),
            commit.commit.subject.clone(),
            commit.commit.is_head,
        );
        self.scm.graph_selected_hash = Some(hash.clone());
        self.workbench_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.terminal_toolbar_menu = None;
        self.activity_bar_menu = None;
        self.explorer_context_menu = None;
        let bottom_inset = if self.view_preferences.status_bar_visible {
            lh
        } else {
            0.0
        };
        self.open_scm_graph_menu(
            hash,
            subject,
            is_head,
            (x, y),
            (window_width, window_height),
            cw,
            bottom_inset,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_scm_graph_menu(
        &mut self,
        hash: String,
        subject: String,
        is_head: bool,
        anchor: (f32, f32),
        window: (f32, f32),
        cw: f32,
        bottom_inset: f32,
    ) {
        let rows = menu_rows(is_head);
        let widest = rows
            .iter()
            .filter_map(|row| match row {
                GraphMenuRow::Action { action, .. } => {
                    Some(self.t(action.label_key()).chars().count())
                }
                GraphMenuRow::Separator => None,
            })
            .max()
            .unwrap_or(20);
        let width = (widest as f32 * cw.max(6.0) + 28.0).max(MIN_MENU_WIDTH);
        let height = PADDING * 2.0 + rows.iter().map(|row| row.height()).sum::<f32>();
        let (x, y) = place_context_menu(anchor, (width, height), window, bottom_inset);
        let selected = rows.iter().position(|row| row.is_selectable()).unwrap_or(0);
        self.scm.graph_menu = Some(GraphContextMenu {
            hash,
            subject,
            x,
            y,
            width,
            selected,
            rows,
        });
    }

    pub(crate) fn scm_graph_menu_pointer_moved(&mut self, x: f32, y: f32) {
        if let Some(menu) = self.scm.graph_menu.as_mut() {
            if let Some((index, _)) = menu.action_at(x, y) {
                menu.selected = index;
            }
        }
    }

    /// A left click while the menu is open: runs the action under it, and always closes the menu.
    /// Returns whether the click landed inside the menu.
    pub(crate) fn scm_graph_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some(menu) = self.scm.graph_menu.take() else {
            return false;
        };
        let inside = menu.contains(x, y);
        if let Some((_, action)) = menu.action_at(x, y) {
            self.run_scm_graph_action(&menu.hash, &menu.subject, action);
        }
        inside
    }

    pub(crate) fn scm_graph_menu_key(&mut self, key: &Key) -> bool {
        let Some(menu) = self.scm.graph_menu.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => self.scm.graph_menu = None,
            Key::Named(NamedKey::ArrowDown) => menu.step(true),
            Key::Named(NamedKey::ArrowUp) => menu.step(false),
            Key::Named(NamedKey::Enter) => {
                let chosen = match menu.rows.get(menu.selected) {
                    Some(GraphMenuRow::Action {
                        action,
                        enabled: true,
                    }) => Some(*action),
                    _ => None,
                };
                let (hash, subject) = (menu.hash.clone(), menu.subject.clone());
                self.scm.graph_menu = None;
                if let Some(action) = chosen {
                    self.run_scm_graph_action(&hash, &subject, action);
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn run_scm_graph_action(
        &mut self,
        hash: &str,
        subject: &str,
        action: GraphMenuAction,
    ) {
        match action {
            GraphMenuAction::Checkout => self.run_scm_remote(RemoteOp::CheckoutCommit(hash.into())),
            GraphMenuAction::CreateBranch => {
                self.open_name_prompt(NameTarget::BranchAt(hash.into()));
            }
            GraphMenuAction::CreateTag => self.open_name_prompt(NameTarget::TagAt(hash.into())),
            GraphMenuAction::CherryPick => self.run_scm_remote(RemoteOp::CherryPick(hash.into())),
            GraphMenuAction::Revert => self.run_scm_remote(RemoteOp::RevertCommit(hash.into())),
            GraphMenuAction::CopyId => self.copy_graph_text(hash),
            GraphMenuAction::CopyMessage => self.copy_graph_text(subject),
        }
    }

    fn copy_graph_text(&mut self, text: &str) {
        self.status = if SystemClipboard.set_text(text).is_ok() {
            self.t("scm.graph.copied").into()
        } else {
            self.t("misc.clipboard_failed").into()
        };
    }

    pub(crate) fn draw_scm_graph_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.scm.graph_menu else {
            return;
        };
        let (x, y, width, height) = (menu.x, menu.y, menu.width, menu.height());
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, bg);
        let mut top = y + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            match row {
                GraphMenuRow::Separator => {
                    renderer.draw_rect(x + 1.0, top + 4.0, width - 2.0, 1.0, border);
                }
                GraphMenuRow::Action { action, enabled } => {
                    if *enabled && menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            ROW_HEIGHT,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    let fg = if *enabled { text } else { dim };
                    renderer.draw_text(self.t(action.label_key()), x + 14.0, top + 4.0, fg);
                }
            }
            top += row.height();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn app_with_menu(dir: &TempDir, is_head: bool) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_scm_graph_menu(
            "abcdef1234567".into(),
            "fix the thing".into(),
            is_head,
            (100.0, 100.0),
            (1200.0, 760.0),
            8.0,
            0.0,
        );
        app
    }

    /// The centre of the row showing `action`.
    fn row_point(app: &App, action: GraphMenuAction) -> (f32, f32) {
        let menu = app.scm.graph_menu.as_ref().unwrap();
        let mut top = menu.y + PADDING;
        for row in &menu.rows {
            if matches!(row, GraphMenuRow::Action { action: a, .. } if *a == action) {
                return (menu.x + 20.0, top + ROW_HEIGHT * 0.5);
            }
            top += row.height();
        }
        panic!("{action:?} is not in the menu");
    }

    #[test]
    fn the_head_commit_cannot_be_checked_out_or_cherry_picked() {
        let dir = TempDir::new();
        let app = app_with_menu(&dir, true);
        let rows = &app.scm.graph_menu.as_ref().unwrap().rows;
        for blocked in [GraphMenuAction::Checkout, GraphMenuAction::CherryPick] {
            assert!(rows.contains(&GraphMenuRow::Action {
                action: blocked,
                enabled: false
            }));
        }
        let app = app_with_menu(&dir, false);
        assert!(app
            .scm
            .graph_menu
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .all(|row| matches!(row, GraphMenuRow::Separator) || row.is_selectable()));
    }

    #[test]
    fn branch_and_tag_ask_for_a_name_for_that_commit() {
        let dir = TempDir::new();
        let mut app = app_with_menu(&dir, false);
        let (x, y) = row_point(&app, GraphMenuAction::CreateBranch);
        assert!(app.scm_graph_menu_click(x, y));
        assert!(app.scm.graph_menu.is_none(), "a click closes the menu");
        assert_eq!(
            app.branch_name_prompt.as_ref().unwrap().target,
            NameTarget::BranchAt("abcdef1234567".into())
        );

        let mut app = app_with_menu(&dir, false);
        let (x, y) = row_point(&app, GraphMenuAction::CreateTag);
        app.scm_graph_menu_click(x, y);
        assert_eq!(
            app.branch_name_prompt.as_ref().unwrap().target,
            NameTarget::TagAt("abcdef1234567".into())
        );
    }

    #[test]
    fn git_actions_are_handed_to_the_worker_and_disabled_rows_do_nothing() {
        let dir = TempDir::new();
        let mut app = app_with_menu(&dir, false);
        let (x, y) = row_point(&app, GraphMenuAction::CherryPick);
        app.status.clear();
        app.scm_graph_menu_click(x, y);
        // Without an event loop the operation stops here, which shows it was started.
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));

        let mut app = app_with_menu(&dir, true);
        let (x, y) = row_point(&app, GraphMenuAction::CherryPick);
        app.status.clear();
        assert!(app.scm_graph_menu_click(x, y), "the click was inside");
        assert!(app.status.is_empty(), "a disabled row does nothing");
        assert!(app.scm.graph_menu.is_none());
    }

    #[test]
    fn clicking_outside_closes_the_menu_and_is_not_swallowed() {
        let dir = TempDir::new();
        let mut app = app_with_menu(&dir, false);
        assert!(!app.scm_graph_menu_click(5.0, 5.0));
        assert!(app.scm.graph_menu.is_none());
    }

    #[test]
    fn the_keyboard_skips_disabled_rows_and_escape_closes() {
        let dir = TempDir::new();
        let mut app = app_with_menu(&dir, true);
        // Checkout is disabled, so the first selectable row is Create Branch.
        let menu = app.scm.graph_menu.as_ref().unwrap();
        assert_eq!(
            menu.rows[menu.selected],
            GraphMenuRow::Action {
                action: GraphMenuAction::CreateBranch,
                enabled: true
            }
        );
        assert!(app.scm_graph_menu_key(&Key::Named(NamedKey::ArrowDown)));
        let menu = app.scm.graph_menu.as_ref().unwrap();
        assert_eq!(
            menu.rows[menu.selected],
            GraphMenuRow::Action {
                action: GraphMenuAction::CreateTag,
                enabled: true
            }
        );
        assert!(app.scm_graph_menu_key(&Key::Named(NamedKey::Enter)));
        assert!(matches!(
            app.branch_name_prompt.as_ref().unwrap().target,
            NameTarget::TagAt(_)
        ));
        assert!(!app.scm_graph_menu_key(&Key::Named(NamedKey::Escape)));

        let mut app = app_with_menu(&dir, false);
        assert!(app.scm_graph_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.scm.graph_menu.is_none());
    }

    #[test]
    fn naming_a_branch_at_a_commit_hands_both_to_git() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_name_prompt(NameTarget::BranchAt("abcd1234".into()));
        app.handle_branch_name_prompt_key(&Key::Character("x".into()), Some("topic"));
        app.status.clear();
        app.handle_branch_name_prompt_key(&Key::Named(NamedKey::Enter), None);
        assert!(app.branch_name_prompt.is_none());
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
        assert!(RemoteOp::CherryPick("abcd".into()).needs_saved_documents());
        assert!(!RemoteOp::CreateTag("v1".into(), "abcd".into()).needs_saved_documents());
    }
}
