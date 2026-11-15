//! Hover cards of the Source Control view: a label for each icon button, and the details of the
//! Graph commit under the pointer.

use super::scm_graph_ui::graph_header_buttons;
use super::scm_ui::{
    changes_header_buttons, commit_arrow, discard_button, ScmListRowKind, ScmPanelLayout,
    ScmPrimaryAction,
};
use super::*;
use crate::vcs::scm_graph;

/// What the pointer is over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tip {
    /// The locale key of a button label.
    Button(&'static str),
    /// A Graph commit.
    Commit(usize),
}

/// What is under the point `(x, y)` (sidebar coordinates) of a laid out panel.
pub(crate) fn tip_at(
    layout: &ScmPanelLayout,
    (x, y): (f32, f32),
    cell_width: f32,
    repositories_collapsed: bool,
    primary: ScmPrimaryAction,
    discardable: impl Fn(usize) -> bool,
) -> Option<Tip> {
    if !repositories_collapsed {
        for (rect, key) in [
            (layout.repo_sync, "scm.tip.sync"),
            (layout.repo_pull_request, "scm.tip.pull_request"),
            (layout.repo_more, "scm.tip.more"),
            (layout.refresh, "scm.tip.refresh"),
        ] {
            if rect.contains(x, y) {
                return Some(Tip::Button(key));
            }
        }
    }
    if primary == ScmPrimaryAction::Commit && commit_arrow(layout.commit_button).contains(x, y) {
        return Some(Tip::Button("scm.tip.commit_options"));
    }
    let row = layout.list_rows.iter().find(|row| row.bounds.contains(x, y))?;
    match row.kind {
        ScmListRowKind::Section { staged, .. } => {
            if !staged {
                let (history, stash) = changes_header_buttons(row.action, cell_width);
                if history.contains(x, y) {
                    return Some(Tip::Button("scm.tip.history"));
                }
                if stash.contains(x, y) {
                    return Some(Tip::Button("scm.tip.stash"));
                }
            }
            row.action.contains(x, y).then_some(Tip::Button(if staged {
                "scm.tip.unstage_all"
            } else {
                "scm.tip.stage_all"
            }))
        }
        ScmListRowKind::Folder { staged, .. } => {
            row.action.contains(x, y).then_some(Tip::Button(if staged {
                "scm.tip.unstage_folder"
            } else {
                "scm.tip.stage_folder"
            }))
        }
        ScmListRowKind::File { index, staged } => {
            if !staged && discardable(index) && discard_button(row.action, cell_width).contains(x, y)
            {
                return Some(Tip::Button("scm.tip.discard"));
            }
            row.action.contains(x, y).then_some(Tip::Button(if staged {
                "scm.tip.unstage"
            } else {
                "scm.tip.stage"
            }))
        }
        ScmListRowKind::GraphHeader { .. } => {
            let [fetch, pull, push, refresh] = graph_header_buttons(row.action, cell_width);
            [
                (fetch, "scm.tip.fetch"),
                (pull, "scm.tip.pull"),
                (push, "scm.tip.push"),
                (refresh, "scm.tip.refresh"),
            ]
            .into_iter()
            .find(|(rect, _)| rect.contains(x, y))
            .map(|(_, key)| Tip::Button(key))
        }
        ScmListRowKind::GraphCommit { index } => Some(Tip::Commit(index)),
        ScmListRowKind::Extension(_) => None,
    }
}

impl App {
    /// The tip under the pointer, when the Source Control view is what the pointer is over.
    fn scm_tip_at(&self, x: f32, y: f32, window: (f32, f32), lh: f32, cw: f32) -> Option<Tip> {
        use crate::app::activity_bar::Activity;
        if self.zen_mode
            || !self.view_preferences.primary_sidebar_visible
            || self.active_activity() != Activity::SourceControl
            || self.active_plugin_activity.is_some()
            || self.shows_scm_welcome_view()
            || !self.primary_sidebar_contains(x, y, window.0, window.1, lh)
        {
            return None;
        }
        let local = (self.sidebar_local_x(x, window.0), self.sidebar_local_y(y));
        let layout = self.scm_sidebar_layout_for(window.1, lh, cw);
        tip_at(
            &layout,
            local,
            cw,
            self.scm_repositories_collapsed,
            self.scm_primary_action(),
            |index| self.discard_target_at(index).is_some(),
        )
    }

    /// Whether the tip under `(x, y)` differs from the one under `(px, py)`, so the card has to be
    /// redrawn when the pointer moves between two buttons.
    pub(crate) fn scm_tip_changed(
        &self,
        (x, y): (f32, f32),
        (px, py): (f32, f32),
        window: (f32, f32),
        lh: f32,
        cw: f32,
    ) -> bool {
        self.scm_visible
            && self.scm_tip_at(x, y, window, lh, cw) != self.scm_tip_at(px, py, window, lh, cw)
    }

    /// The hover card under the pointer.
    pub(crate) fn draw_scm_tooltip(&self, renderer: &mut Renderer) {
        if self.scm_graph_menu.is_some()
            || self.scm_more_menu.is_some()
            || self.scm_confirm.is_some()
            || self.explorer_context_menu.is_some()
            || self.scm_graph_resize_dragging
        {
            return;
        }
        let (width, height) = renderer.size();
        let (cw, lh) = (renderer.char_width().max(4.0), renderer.line_height());
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        let lines = match self.scm_tip_at(mx, my, (width, height), lh, cw) {
            Some(Tip::Button(key)) => vec![self.t(key).to_owned()],
            Some(Tip::Commit(index)) => match self.scm_graph_rows.get(index) {
                Some(row) => scm_graph::tooltip_lines(&row.commit),
                None => return,
            },
            None => return,
        };
        let longest = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
        let box_width = ((longest as f32 * cw) + 20.0).min((width - 16.0).max(60.0));
        let box_height = lines.len() as f32 * lh + 12.0;
        let x = if mx + 14.0 + box_width <= width - 4.0 {
            mx + 14.0
        } else {
            (mx - box_width - 6.0).max(4.0)
        };
        let y = if my + 18.0 + box_height <= height - 4.0 {
            my + 18.0
        } else {
            (my - box_height - 6.0).max(4.0)
        };
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, box_width, box_height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, box_width - 2.0, box_height - 2.0, bg);
        for (index, line) in lines.iter().enumerate() {
            let clipped = Self::help_menu_visible_label(line, cw, (box_width - 20.0).max(0.0));
            renderer.draw_text(
                &clipped,
                x + 10.0,
                y + 6.0 + index as f32 * lh,
                if index == 0 { text } else { dim },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::scm_ui::{scm_panel_layout_with_graph, ScmListView};
    use super::*;
    use crate::scm::{FileStatus, Snapshot};

    fn layout(snapshot: &Snapshot) -> ScmPanelLayout {
        scm_panel_layout_with_graph(
            snapshot, 300.0, 700.0, 20.0, 9.0, 0, false, false, false, &[], true, false, 2, 0,
            None, &ScmListView::LIST,
        )
    }

    fn centre(rect: super::super::scm_ui::ScmRect) -> (f32, f32) {
        (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            files: vec![
                FileStatus {
                    path: "a.txt".into(),
                    unstaged: true,
                    ..Default::default()
                },
                FileStatus {
                    path: "b.txt".into(),
                    staged: true,
                    ..Default::default()
                },
            ],
            has_commits: true,
            ..Default::default()
        }
    }

    #[test]
    fn the_repository_buttons_have_tips_unless_the_section_is_collapsed() {
        let snapshot = snapshot();
        let layout = layout(&snapshot);
        let tip = |point, collapsed| {
            tip_at(&layout, point, 9.0, collapsed, ScmPrimaryAction::Commit, |_| true)
        };
        assert_eq!(tip(centre(layout.repo_sync), false), Some(Tip::Button("scm.tip.sync")));
        assert_eq!(
            tip(centre(layout.repo_pull_request), false),
            Some(Tip::Button("scm.tip.pull_request"))
        );
        assert_eq!(tip(centre(layout.repo_more), false), Some(Tip::Button("scm.tip.more")));
        assert_eq!(tip(centre(layout.repo_more), true), None);
        assert_eq!(tip(centre(layout.commit_field), false), None);
    }

    #[test]
    fn only_the_arrow_of_a_commit_button_has_a_tip_and_only_while_it_commits() {
        let snapshot = snapshot();
        let layout = layout(&snapshot);
        let arrow = centre(commit_arrow(layout.commit_button));
        let on = |primary| tip_at(&layout, arrow, 9.0, false, primary, |_| true);
        assert_eq!(
            on(ScmPrimaryAction::Commit),
            Some(Tip::Button("scm.tip.commit_options"))
        );
        assert_eq!(on(ScmPrimaryAction::PublishBranch), None);
        let label = (layout.commit_button.x + 10.0, layout.commit_button.y + 5.0);
        assert_eq!(
            tip_at(&layout, label, 9.0, false, ScmPrimaryAction::Commit, |_| true),
            None
        );
    }

    #[test]
    fn file_rows_name_stage_unstage_and_discard() {
        let snapshot = snapshot();
        let layout = layout(&snapshot);
        let file = |wanted_staged| {
            layout
                .list_rows
                .iter()
                .find(|row| matches!(row.kind, ScmListRowKind::File { staged, .. } if staged == wanted_staged))
                .copied()
                .unwrap()
        };
        let changes = file(false);
        let stage = centre(changes.action);
        let discard = centre(discard_button(changes.action, 9.0));
        let tip = |point, can_discard| {
            tip_at(&layout, point, 9.0, false, ScmPrimaryAction::Commit, |_| can_discard)
        };
        assert_eq!(tip(stage, true), Some(Tip::Button("scm.tip.stage")));
        assert_eq!(tip(discard, true), Some(Tip::Button("scm.tip.discard")));
        assert_eq!(tip(discard, false), None, "no discard button on a conflicted file");
        assert_eq!(
            tip(centre(file(true).action), true),
            Some(Tip::Button("scm.tip.unstage"))
        );
        assert_eq!(
            tip((changes.bounds.x + 30.0, changes.bounds.y + 4.0), true),
            None,
            "the file name has no tip"
        );
    }

    #[test]
    fn graph_buttons_and_commits_have_tips() {
        let snapshot = snapshot();
        let layout = layout(&snapshot);
        let header = layout
            .list_rows
            .iter()
            .find(|row| matches!(row.kind, ScmListRowKind::GraphHeader { .. }))
            .copied()
            .unwrap();
        let [fetch, pull, push, refresh] = graph_header_buttons(header.action, 9.0);
        for (rect, key) in [
            (fetch, "scm.tip.fetch"),
            (pull, "scm.tip.pull"),
            (push, "scm.tip.push"),
            (refresh, "scm.tip.refresh"),
        ] {
            assert_eq!(
                tip_at(&layout, centre(rect), 9.0, false, ScmPrimaryAction::Commit, |_| true),
                Some(Tip::Button(key))
            );
        }
        let commit = layout
            .list_rows
            .iter()
            .find(|row| matches!(row.kind, ScmListRowKind::GraphCommit { .. }))
            .copied()
            .unwrap();
        assert_eq!(
            tip_at(&layout, centre(commit.bounds), 9.0, false, ScmPrimaryAction::Commit, |_| true),
            Some(Tip::Commit(0))
        );
    }

    #[test]
    fn every_tip_label_is_translated_in_both_languages() {
        let keys = [
            "scm.tip.sync",
            "scm.tip.pull_request",
            "scm.tip.more",
            "scm.tip.refresh",
            "scm.tip.commit_options",
            "scm.tip.history",
            "scm.tip.stash",
            "scm.tip.stage_all",
            "scm.tip.unstage_all",
            "scm.tip.stage",
            "scm.tip.unstage",
            "scm.tip.stage_folder",
            "scm.tip.unstage_folder",
            "scm.tip.discard",
            "scm.tip.fetch",
            "scm.tip.pull",
            "scm.tip.push",
        ];
        let english = graphite_i18n::Translator::english();
        let pt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../locales/pt-BR/scm.json"
            ))
            .unwrap(),
        )
        .unwrap();
        for key in keys {
            assert!(english.has(key), "{key}");
            assert!(pt.get(key).is_some(), "{key} in pt-BR");
        }
    }
}
