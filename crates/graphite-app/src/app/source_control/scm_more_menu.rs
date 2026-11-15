//! The "…" menu of the source control view: Git commands grouped in nested submenus.
//!
//! The menu is a tree of [`MoreNode`]s. [`MoreMenu`] holds which submenus are open and which row
//! is highlighted at each level, and lays the open panels out side by side; it knows nothing
//! about git. [`App`] runs the chosen [`MoreCommand`].

use super::scm_branch_ui::{BranchPickerAction, NameTarget};
use super::scm_remote_ui::RemoteOp;
use super::scm_ui::ScmAction;
use super::*;
use crate::app::explorer_context_menu::place_context_menu;
use crate::scm_commit::{CommitOptions, CommitScope, PostCommit};
use crate::scm_stash::StashMode;

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 6.0;
const MIN_WIDTH: f32 = 200.0;
/// Room right of a label for the arrow of a submenu row.
const ARROW_ROOM: f32 = 34.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoreCommand {
    /// Commit with these options.
    Commit(CommitOptions),
    Pull,
    PullRebase,
    Push,
    Fetch,
    Sync,
    CheckoutTo,
    UndoLastCommit,
    AbortOperation,
    StageAll,
    UnstageAll,
    DiscardAll,
    Merge,
    Rebase,
    CreateBranch,
    RenameBranch,
    DeleteBranch,
    DeleteBranchForce,
    Stash,
    StashUntracked,
    StashStaged,
    StashApplyLatest,
    StashPopLatest,
    CreateTag,
    FileOpen,
    FileStage,
    FileUnstage,
    FileDiscard,
    FileAddToGitignore,
    FileCopyPath,
    FileCopyRelativePath,
}

/// Which menu is open: the "…" menu, or the menu of a file of the changes list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MoreMenuKind {
    #[default]
    Actions,
    /// The file at this index of the snapshot, in the staged or the unstaged list.
    File { index: usize, staged: bool },
    /// The arrow of the commit button.
    CommitOptions,
}

#[derive(Clone, Debug)]
pub(crate) enum MoreNode {
    Separator,
    Item {
        label: &'static str,
        /// What the row runs; `None` for a row that opens `children`.
        command: Option<MoreCommand>,
        children: Vec<MoreNode>,
    },
}

impl MoreNode {
    fn leaf(label: &'static str, command: MoreCommand) -> Self {
        Self::Item {
            label,
            command: Some(command),
            children: Vec::new(),
        }
    }

    fn submenu(label: &'static str, children: Vec<MoreNode>) -> Self {
        Self::Item {
            label,
            command: None,
            children,
        }
    }

    fn is_selectable(&self) -> bool {
        matches!(self, Self::Item { .. })
    }

    fn has_children(&self) -> bool {
        matches!(self, Self::Item { children, .. } if !children.is_empty())
    }

    fn height(&self) -> f32 {
        match self {
            Self::Separator => SEPARATOR_HEIGHT,
            Self::Item { .. } => ROW_HEIGHT,
        }
    }
}

/// The tree of the menu of `kind`.
pub(crate) fn menu_tree(kind: MoreMenuKind) -> Vec<MoreNode> {
    match kind {
        MoreMenuKind::Actions => more_menu_tree(),
        MoreMenuKind::File { staged, .. } => file_menu_tree(staged),
        MoreMenuKind::CommitOptions => commit_button_tree(),
    }
}

fn commit_options(scope: CommitScope, amend: bool, signoff: bool) -> MoreCommand {
    MoreCommand::Commit(CommitOptions {
        scope,
        amend,
        signoff,
        post: PostCommit::Nothing,
    })
}

/// The menu of the arrow of the commit button.
fn commit_button_tree() -> Vec<MoreNode> {
    use MoreNode as N;
    let then = |post| {
        MoreCommand::Commit(CommitOptions {
            post,
            ..CommitOptions::PLAIN
        })
    };
    vec![
        N::Item {
            label: "scm.more.commit",
            command: Some(MoreCommand::Commit(CommitOptions::PLAIN)),
            children: Vec::new(),
        },
        N::Item {
            label: "scm.more.commit_push",
            command: Some(then(PostCommit::Push)),
            children: Vec::new(),
        },
        N::Item {
            label: "scm.more.commit_sync",
            command: Some(then(PostCommit::Sync)),
            children: Vec::new(),
        },
    ]
}

fn file_menu_tree(staged: bool) -> Vec<MoreNode> {
    use MoreCommand as C;
    use MoreNode as N;
    let mut rows = vec![
        N::leaf("scm.file.open", C::FileOpen),
        N::Separator,
    ];
    if staged {
        rows.push(N::leaf("scm.file.unstage", C::FileUnstage));
    } else {
        rows.push(N::leaf("scm.file.stage", C::FileStage));
        rows.push(N::leaf("scm.file.discard", C::FileDiscard));
    }
    rows.extend([
        N::Separator,
        N::leaf("scm.file.add_to_gitignore", C::FileAddToGitignore),
        N::Separator,
        N::leaf("scm.file.copy_path", C::FileCopyPath),
        N::leaf("scm.file.copy_relative_path", C::FileCopyRelativePath),
    ]);
    rows
}

/// The whole "…" menu.
pub(crate) fn more_menu_tree() -> Vec<MoreNode> {
    use MoreCommand as C;
    use MoreNode as N;
    vec![
        N::leaf("scm.more.pull", C::Pull),
        N::leaf("scm.more.push", C::Push),
        N::leaf("scm.more.checkout_to", C::CheckoutTo),
        N::leaf("scm.more.fetch", C::Fetch),
        N::Separator,
        N::submenu(
            "scm.more.commit",
            vec![
                N::leaf("scm.more.commit", C::Commit(CommitOptions::PLAIN)),
                N::leaf("scm.more.commit_staged", commit_options(CommitScope::Staged, false, false)),
                N::leaf("scm.more.commit_all", commit_options(CommitScope::All, false, false)),
                N::leaf("scm.more.undo_last_commit", C::UndoLastCommit),
                N::leaf("scm.more.abort_operation", C::AbortOperation),
                N::Separator,
                N::leaf("scm.more.commit_amend", commit_options(CommitScope::Smart, true, false)),
                N::leaf("scm.more.commit_staged_amend", commit_options(CommitScope::Staged, true, false)),
                N::leaf("scm.more.commit_all_amend", commit_options(CommitScope::All, true, false)),
                N::Separator,
                N::leaf("scm.more.commit_signed", commit_options(CommitScope::Smart, false, true)),
                N::leaf("scm.more.commit_staged_signed", commit_options(CommitScope::Staged, false, true)),
                N::leaf("scm.more.commit_all_signed", commit_options(CommitScope::All, false, true)),
            ],
        ),
        N::submenu(
            "scm.more.changes",
            vec![
                N::leaf("scm.more.stage_all", C::StageAll),
                N::leaf("scm.more.unstage_all", C::UnstageAll),
                N::Separator,
                N::leaf("scm.more.discard_all", C::DiscardAll),
            ],
        ),
        N::submenu(
            "scm.more.pull_push",
            vec![
                N::leaf("scm.more.sync", C::Sync),
                N::Separator,
                N::leaf("scm.more.pull", C::Pull),
                N::leaf("scm.more.pull_rebase", C::PullRebase),
                N::Separator,
                N::leaf("scm.more.push", C::Push),
                N::Separator,
                N::leaf("scm.more.fetch", C::Fetch),
            ],
        ),
        N::submenu(
            "scm.more.branch",
            vec![
                N::leaf("scm.more.merge", C::Merge),
                N::leaf("scm.more.rebase", C::Rebase),
                N::Separator,
                N::leaf("scm.more.create_branch", C::CreateBranch),
                N::leaf("scm.more.rename_branch", C::RenameBranch),
                N::leaf("scm.more.delete_branch", C::DeleteBranch),
                N::leaf("scm.more.delete_branch_force", C::DeleteBranchForce),
            ],
        ),
        N::submenu(
            "scm.more.stash",
            vec![
                N::leaf("scm.more.stash", C::Stash),
                N::leaf("scm.more.stash_untracked", C::StashUntracked),
                N::leaf("scm.more.stash_staged", C::StashStaged),
                N::Separator,
                N::leaf("scm.more.stash_apply_latest", C::StashApplyLatest),
                N::leaf("scm.more.stash_pop_latest", C::StashPopLatest),
            ],
        ),
        N::submenu(
            "scm.more.tags",
            vec![N::leaf("scm.more.create_tag", C::CreateTag)],
        ),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PanelRect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

fn rows_height(rows: &[MoreNode]) -> f32 {
    PADDING * 2.0 + rows.iter().map(MoreNode::height).sum::<f32>()
}

/// The rows shown by the panel opened through the first `levels` entries of `open`.
fn rows_at<'a>(tree: &'a [MoreNode], open: &[usize]) -> &'a [MoreNode] {
    let mut rows = tree;
    for &index in open {
        match rows.get(index) {
            Some(MoreNode::Item { children, .. }) => rows = children,
            _ => break,
        }
    }
    rows
}

#[derive(Clone, Debug)]
pub(crate) struct MoreMenu {
    anchor: (f32, f32),
    window: (f32, f32),
    bottom_inset: f32,
    /// The row that is open as a submenu at each level except the last.
    pub open: Vec<usize>,
    /// The highlighted row at each open level.
    pub selected: Vec<usize>,
    /// Label width per locale key, so layout does not need the translator.
    widths: Vec<(&'static str, f32)>,
    pub kind: MoreMenuKind,
}

impl MoreMenu {
    pub(crate) fn new(
        tree: &[MoreNode],
        anchor: (f32, f32),
        window: (f32, f32),
        bottom_inset: f32,
        label_width: impl Fn(&'static str) -> f32,
    ) -> Self {
        let mut widths = Vec::new();
        collect_widths(tree, &label_width, &mut widths);
        let first = tree.iter().position(MoreNode::is_selectable).unwrap_or(0);
        Self {
            anchor,
            window,
            bottom_inset,
            open: Vec::new(),
            selected: vec![first],
            widths,
            kind: MoreMenuKind::Actions,
        }
    }

    fn width_of(&self, rows: &[MoreNode]) -> f32 {
        let widest = rows
            .iter()
            .filter_map(|row| match row {
                MoreNode::Item { label, .. } => self
                    .widths
                    .iter()
                    .find(|(key, _)| key == label)
                    .map(|(_, width)| *width),
                MoreNode::Separator => None,
            })
            .fold(0.0_f32, f32::max);
        (widest + ARROW_ROOM + 20.0).max(MIN_WIDTH)
    }

    /// The panels from the root to the deepest open submenu.
    pub(crate) fn panels(&self, tree: &[MoreNode]) -> Vec<PanelRect> {
        let mut panels = Vec::new();
        let mut rows = tree;
        for level in 0..=self.open.len() {
            let width = self.width_of(rows);
            let height = rows_height(rows);
            let rect = if level == 0 {
                let (x, y) = place_context_menu(
                    self.anchor,
                    (width, height),
                    self.window,
                    self.bottom_inset,
                );
                PanelRect {
                    x,
                    y,
                    width,
                    height,
                }
            } else {
                let parent: PanelRect = panels[level - 1];
                let parent_rows = rows_at(tree, &self.open[..level - 1]);
                let row_top = parent.y
                    + PADDING
                    + parent_rows[..self.open[level - 1]]
                        .iter()
                        .map(MoreNode::height)
                        .sum::<f32>();
                let mut x = parent.x + parent.width - 2.0;
                if x + width > self.window.0 - 4.0 {
                    x = (parent.x - width + 2.0).max(4.0);
                }
                let max_bottom = self.window.1 - self.bottom_inset - 4.0;
                let y = (row_top - PADDING).min(max_bottom - height).max(4.0);
                PanelRect {
                    x,
                    y,
                    width,
                    height,
                }
            };
            panels.push(rect);
            if level < self.open.len() {
                rows = rows_at(tree, &self.open[..=level]);
            }
        }
        panels
    }

    /// The panel level and row under a point.
    fn hit(&self, tree: &[MoreNode], x: f32, y: f32) -> Option<(usize, usize)> {
        let panels = self.panels(tree);
        for level in (0..panels.len()).rev() {
            let panel = panels[level];
            if !panel.contains(x, y) {
                continue;
            }
            let rows = rows_at(tree, &self.open[..level]);
            let mut top = panel.y + PADDING;
            for (index, row) in rows.iter().enumerate() {
                if y >= top && y < top + row.height() {
                    return Some((level, index));
                }
                top += row.height();
            }
            return None;
        }
        None
    }

    pub(crate) fn contains(&self, tree: &[MoreNode], x: f32, y: f32) -> bool {
        self.panels(tree).iter().any(|panel| panel.contains(x, y))
    }

    /// Highlights `index` at `level` and opens or closes submenus to match.
    fn focus(&mut self, tree: &[MoreNode], level: usize, index: usize) {
        let rows = rows_at(tree, &self.open[..level]);
        let Some(row) = rows.get(index).filter(|row| row.is_selectable()) else {
            return;
        };
        self.selected.truncate(level + 1);
        self.selected[level] = index;
        if row.has_children() {
            if self.open.get(level) != Some(&index) {
                self.open.truncate(level);
                self.open.push(index);
                let child_rows = rows_at(tree, &self.open);
                self.selected
                    .push(child_rows.iter().position(MoreNode::is_selectable).unwrap_or(0));
            }
        } else {
            self.open.truncate(level);
        }
    }

    pub(crate) fn pointer_moved(&mut self, tree: &[MoreNode], x: f32, y: f32) {
        if let Some((level, index)) = self.hit(tree, x, y) {
            self.focus(tree, level, index);
        }
    }

    /// A click: the command of a leaf row, or `None` when it opened a submenu or hit nothing.
    pub(crate) fn click(&mut self, tree: &[MoreNode], x: f32, y: f32) -> Option<MoreCommand> {
        let (level, index) = self.hit(tree, x, y)?;
        self.focus(tree, level, index);
        match rows_at(tree, &self.open[..level]).get(index) {
            Some(MoreNode::Item {
                command: Some(command),
                ..
            }) => Some(*command),
            _ => None,
        }
    }

    pub(crate) fn step(&mut self, tree: &[MoreNode], forward: bool) {
        let level = self.open.len();
        let rows = rows_at(tree, &self.open);
        let len = rows.len();
        if len == 0 {
            return;
        }
        let current = self.selected[level];
        for offset in 1..=len {
            let candidate = if forward {
                (current + offset) % len
            } else {
                (current + len - offset) % len
            };
            if rows[candidate].is_selectable() {
                self.focus(tree, level, candidate);
                // Moving over a submenu row opens it; the keyboard stays on this level.
                self.open.truncate(level);
                self.selected.truncate(level + 1);
                self.selected[level] = candidate;
                return;
            }
        }
    }

    /// Right arrow: opens the highlighted submenu.
    pub(crate) fn enter_submenu(&mut self, tree: &[MoreNode]) -> bool {
        let level = self.open.len();
        let index = self.selected[level];
        let opens = rows_at(tree, &self.open)
            .get(index)
            .is_some_and(MoreNode::has_children);
        if opens {
            self.focus(tree, level, index);
        }
        opens
    }

    /// Left arrow: closes the deepest submenu. Returns whether there was one.
    pub(crate) fn leave_submenu(&mut self) -> bool {
        if self.open.pop().is_some() {
            self.selected.pop();
            true
        } else {
            false
        }
    }

    /// Enter: the highlighted leaf's command, or opens the highlighted submenu.
    pub(crate) fn activate(&mut self, tree: &[MoreNode]) -> Option<MoreCommand> {
        let level = self.open.len();
        let index = self.selected[level];
        match rows_at(tree, &self.open).get(index) {
            Some(MoreNode::Item {
                command: Some(command),
                ..
            }) => Some(*command),
            Some(row) if row.has_children() => {
                self.focus(tree, level, index);
                None
            }
            _ => None,
        }
    }
}

fn collect_widths(
    rows: &[MoreNode],
    label_width: &impl Fn(&'static str) -> f32,
    out: &mut Vec<(&'static str, f32)>,
) {
    for row in rows {
        if let MoreNode::Item {
            label, children, ..
        } = row
        {
            if !out.iter().any(|(key, _)| key == label) {
                out.push((label, label_width(label)));
            }
            collect_widths(children, label_width, out);
        }
    }
}

impl App {
    /// Right click on a file of the changes list.
    pub(crate) fn scm_file_right_click(&mut self, x: f32, y: f32) -> bool {
        use super::scm_ui::ScmListRowKind;
        let (window_width, window_height, lh, cw) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (w, h) = gpu.renderer.size();
                (w, h, gpu.renderer.line_height(), gpu.renderer.char_width())
            })
            .unwrap_or((1200.0, 760.0, 18.0, 8.0));
        if self.zen_mode
            || !self.view_preferences.primary_sidebar_visible
            || !self.scm_visible
            || self.active_plugin_activity.is_some()
            || self.shows_scm_welcome_view()
            || !self.primary_sidebar_contains(x, y, window_width, window_height, lh)
        {
            return false;
        }
        let local_x = self.sidebar_local_x(x, window_width);
        let local_y = self.sidebar_local_y(y);
        let layout = self.scm_sidebar_layout_for(window_height, lh, cw);
        let hit = layout.list_rows.iter().find_map(|row| match row.kind {
            ScmListRowKind::File { index, staged } if row.bounds.contains(local_x, local_y) => {
                Some((index, staged))
            }
            _ => None,
        });
        let Some((index, staged)) = hit else {
            return false;
        };
        self.scm_selected = index;
        self.workbench_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.terminal_toolbar_menu = None;
        self.activity_bar_menu = None;
        self.explorer_context_menu = None;
        self.scm_graph_menu = None;
        let kind = MoreMenuKind::File { index, staged };
        self.open_more_menu(kind, (x, y), (window_width, window_height), lh, cw);
        true
    }

    fn open_more_menu(
        &mut self,
        kind: MoreMenuKind,
        anchor: (f32, f32),
        window: (f32, f32),
        lh: f32,
        cw: f32,
    ) {
        let bottom_inset = if self.view_preferences.status_bar_visible {
            lh
        } else {
            0.0
        };
        let tree = menu_tree(kind);
        let char_width = cw.max(6.0);
        let mut menu = MoreMenu::new(&tree, anchor, window, bottom_inset, |label| {
            self.t(label).chars().count() as f32 * char_width
        });
        menu.kind = kind;
        self.scm_more_menu = Some(menu);
    }

    /// The arrow of the commit button: opens its menu under the button, `anchor` being the
    /// bottom left corner of the button.
    pub(crate) fn open_scm_commit_menu(&mut self, anchor: (f32, f32)) {
        let (window_width, window_height, lh, cw) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (w, h) = gpu.renderer.size();
                (w, h, gpu.renderer.line_height(), gpu.renderer.char_width())
            })
            .unwrap_or((1200.0, 760.0, 18.0, 8.0));
        self.open_more_menu(
            MoreMenuKind::CommitOptions,
            anchor,
            (window_width, window_height),
            lh,
            cw,
        );
    }

    /// The "…" button: opens the menu below the pointer.
    pub(crate) fn open_scm_more_menu(&mut self) {
        let (window_width, window_height, lh, cw) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (w, h) = gpu.renderer.size();
                (w, h, gpu.renderer.line_height(), gpu.renderer.char_width())
            })
            .unwrap_or((1200.0, 760.0, 18.0, 8.0));
        let bottom_inset = if self.view_preferences.status_bar_visible {
            lh
        } else {
            0.0
        };
        let _ = bottom_inset;
        let anchor = (self.mouse.x as f32, self.mouse.y as f32 + lh * 0.6);
        self.open_more_menu(
            MoreMenuKind::Actions,
            anchor,
            (window_width, window_height),
            lh,
            cw,
        );
    }

    /// The tree of the open menu.
    fn open_more_tree(&self) -> Vec<MoreNode> {
        menu_tree(self.scm_more_menu.as_ref().map(|menu| menu.kind).unwrap_or_default())
    }

    /// The old "…" behavior: the Git commands in the command palette.
    pub(crate) fn open_scm_git_commands(&mut self) {
        self.open_command_palette();
        if let Some(palette) = self.command_palette.as_mut() {
            palette.query = "Git: ".into();
        }
    }

    pub(crate) fn scm_more_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let tree = self.open_more_tree();
        if let Some(menu) = self.scm_more_menu.as_mut() {
            menu.pointer_moved(&tree, x, y);
        }
    }

    /// A left click while the menu is open: runs the row under it. The menu closes unless the
    /// click opened a submenu.
    pub(crate) fn scm_more_menu_click(&mut self, x: f32, y: f32) {
        let tree = self.open_more_tree();
        let Some(menu) = self.scm_more_menu.as_mut() else {
            return;
        };
        if !menu.contains(&tree, x, y) {
            self.scm_more_menu = None;
            return;
        }
        let kind = menu.kind;
        if let Some(command) = menu.click(&tree, x, y) {
            self.scm_more_menu = None;
            self.run_more_command_for(command, kind);
        }
    }

    pub(crate) fn scm_more_menu_key(&mut self, key: &Key) -> bool {
        let tree = self.open_more_tree();
        let Some(menu) = self.scm_more_menu.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => self.scm_more_menu = None,
            Key::Named(NamedKey::ArrowDown) => menu.step(&tree, true),
            Key::Named(NamedKey::ArrowUp) => menu.step(&tree, false),
            Key::Named(NamedKey::ArrowRight) => {
                menu.enter_submenu(&tree);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                menu.leave_submenu();
            }
            Key::Named(NamedKey::Enter) => {
                let kind = menu.kind;
                if let Some(command) = menu.activate(&tree) {
                    self.scm_more_menu = None;
                    self.run_more_command_for(command, kind);
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn run_more_command(&mut self, command: MoreCommand) {
        self.run_more_command_for(command, MoreMenuKind::Actions);
    }

    /// The file a file menu command is about.
    fn menu_file(&self, kind: MoreMenuKind) -> Option<crate::scm::FileStatus> {
        match kind {
            MoreMenuKind::File { index, .. } => self.scm_snapshot.files.get(index).cloned(),
            MoreMenuKind::Actions | MoreMenuKind::CommitOptions => None,
        }
    }

    fn run_file_command(&mut self, command: MoreCommand, kind: MoreMenuKind) {
        use crate::app::clipboard::{ClipboardAccess, SystemClipboard};
        use MoreCommand as C;
        let (Some(file), MoreMenuKind::File { index, .. }) = (self.menu_file(kind), kind) else {
            return;
        };
        self.scm_selected = index;
        match command {
            C::FileOpen => {
                let path = self.tree.root().join(&file.path);
                self.open_file(path);
            }
            C::FileStage => self.perform_scm_action(ScmAction::Stage),
            C::FileUnstage => self.perform_scm_action(ScmAction::Unstage),
            C::FileDiscard => {
                if let Some(target) = self.discard_target_at(index) {
                    self.confirm_discard(vec![target]);
                }
            }
            C::FileAddToGitignore => {
                let root = self.scm_workspace_root();
                self.status = match crate::scm_ignore::add_to_gitignore(&root, &file.path) {
                    Ok(true) => self.t("scm.file.ignored").into(),
                    Ok(false) => self.t("scm.file.already_ignored").into(),
                    Err(error) => error,
                };
                self.refresh_scm_async();
            }
            C::FileCopyPath | C::FileCopyRelativePath => {
                let text = if command == C::FileCopyPath {
                    self.tree.root().join(&file.path).to_string_lossy().into_owned()
                } else {
                    file.path.to_string_lossy().replace('\\', "/")
                };
                self.status = if SystemClipboard.set_text(&text).is_ok() {
                    self.t("misc.path_copied").into()
                } else {
                    self.t("misc.clipboard_failed").into()
                };
            }
            _ => {}
        }
    }

    pub(crate) fn run_more_command_for(&mut self, command: MoreCommand, kind: MoreMenuKind) {
        use MoreCommand as C;
        match command {
            C::FileOpen
            | C::FileStage
            | C::FileUnstage
            | C::FileDiscard
            | C::FileAddToGitignore
            | C::FileCopyPath
            | C::FileCopyRelativePath => return self.run_file_command(command, kind),
            _ => {}
        }
        match command {
            C::Commit(options) => self.perform_scm_action(ScmAction::Commit(options)),
            C::Pull => self.run_scm_remote(RemoteOp::Pull),
            C::PullRebase => self.run_scm_remote(RemoteOp::PullRebase),
            C::Push => self.run_scm_remote(RemoteOp::Push),
            C::Fetch => self.run_scm_remote(RemoteOp::Fetch),
            C::Sync => self.run_scm_remote(RemoteOp::Sync),
            C::CheckoutTo => self.open_branch_picker(),
            C::UndoLastCommit => self.run_scm_remote(RemoteOp::UndoLastCommit),
            C::AbortOperation => self.abort_git_operation(),
            C::StageAll => self.perform_scm_action(ScmAction::StageAll),
            C::UnstageAll => self.perform_scm_action(ScmAction::UnstageAll),
            C::DiscardAll => self.confirm_discard(self.discard_all_targets()),
            C::Merge => self.open_branch_picker_for(BranchPickerAction::Merge),
            C::Rebase => self.open_branch_picker_for(BranchPickerAction::Rebase),
            C::CreateBranch => self.open_create_branch_prompt(),
            C::RenameBranch => self.open_name_prompt(NameTarget::RenameBranch),
            C::DeleteBranch => self.open_branch_picker_for(BranchPickerAction::Delete),
            C::DeleteBranchForce => self.open_branch_picker_for(BranchPickerAction::DeleteForce),
            C::Stash => self.run_scm_remote(RemoteOp::StashWith(StashMode::Tracked)),
            C::StashUntracked => {
                self.run_scm_remote(RemoteOp::StashWith(StashMode::WithUntracked))
            }
            C::StashStaged => self.run_scm_remote(RemoteOp::StashWith(StashMode::Staged)),
            C::StashApplyLatest => self.run_scm_remote(RemoteOp::StashApplyLatest),
            C::StashPopLatest => self.run_scm_remote(RemoteOp::StashPopLatest),
            C::CreateTag => self.open_name_prompt(NameTarget::TagHead),
            C::FileOpen
            | C::FileStage
            | C::FileUnstage
            | C::FileDiscard
            | C::FileAddToGitignore
            | C::FileCopyPath
            | C::FileCopyRelativePath => {}
        }
    }

    pub(crate) fn draw_scm_more_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.scm_more_menu else {
            return;
        };
        let tree = self.open_more_tree();
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let highlight = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let panels = menu.panels(&tree);
        for (level, panel) in panels.iter().enumerate() {
            renderer.draw_rect(panel.x, panel.y, panel.width, panel.height, border);
            renderer.draw_rect(
                panel.x + 1.0,
                panel.y + 1.0,
                panel.width - 2.0,
                panel.height - 2.0,
                bg,
            );
            let rows = rows_at(&tree, &menu.open[..level]);
            let mut top = panel.y + PADDING;
            for (index, row) in rows.iter().enumerate() {
                match row {
                    MoreNode::Separator => {
                        renderer.draw_rect(panel.x + 1.0, top + 4.0, panel.width - 2.0, 1.0, border);
                    }
                    MoreNode::Item { label, .. } => {
                        let focused = menu.selected.get(level) == Some(&index);
                        if focused {
                            renderer.draw_rect(
                                panel.x + 2.0,
                                top,
                                panel.width - 4.0,
                                ROW_HEIGHT,
                                highlight,
                            );
                        }
                        renderer.draw_text(self.t(label), panel.x + 14.0, top + 4.0, text);
                        if row.has_children() {
                            let size = 14.0;
                            renderer.draw_icon(
                                crate::file_icons::CHEVRON_RIGHT,
                                panel.x + panel.width - size - 10.0,
                                top + (ROW_HEIGHT - size) * 0.5,
                                size,
                                text,
                            );
                        }
                    }
                }
                top += row.height();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn menu(tree: &[MoreNode]) -> MoreMenu {
        MoreMenu::new(tree, (50.0, 50.0), (1200.0, 800.0), 0.0, |label| {
            label.len() as f32 * 8.0
        })
    }

    /// A point inside row `index` of panel `level`.
    fn point(menu: &MoreMenu, tree: &[MoreNode], level: usize, index: usize) -> (f32, f32) {
        let panel = menu.panels(tree)[level];
        let rows = rows_at(tree, &menu.open[..level]);
        let top = panel.y + PADDING + rows[..index].iter().map(MoreNode::height).sum::<f32>();
        (panel.x + 20.0, top + ROW_HEIGHT * 0.5)
    }

    fn index_of(rows: &[MoreNode], label: &str) -> usize {
        rows.iter()
            .position(|row| matches!(row, MoreNode::Item { label: l, .. } if *l == label))
            .unwrap_or_else(|| panic!("no row {label}"))
    }

    #[test]
    fn every_label_of_the_menu_is_translated_in_both_languages() {
        fn walk(rows: &[MoreNode], keys: &mut Vec<&'static str>) {
            for row in rows {
                if let MoreNode::Item {
                    label, children, ..
                } = row
                {
                    keys.push(label);
                    walk(children, keys);
                }
            }
        }
        let mut keys = Vec::new();
        walk(&more_menu_tree(), &mut keys);
        walk(&file_menu_tree(true), &mut keys);
        walk(&file_menu_tree(false), &mut keys);
        walk(&commit_button_tree(), &mut keys);
        let english = graphite_i18n::Translator::english();
        for key in keys {
            assert!(english.has(key), "{key} is missing in English");
        }
        let pt: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../locales/pt-BR/scm.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let mut keys = Vec::new();
        walk(&more_menu_tree(), &mut keys);
        walk(&file_menu_tree(true), &mut keys);
        walk(&file_menu_tree(false), &mut keys);
        walk(&commit_button_tree(), &mut keys);
        for key in keys {
            assert!(pt.get(key).is_some(), "{key} is missing in pt-BR");
        }
    }

    #[test]
    fn hovering_a_group_opens_its_submenu_beside_it_and_hovering_a_leaf_closes_it() {
        let tree = more_menu_tree();
        let mut menu = menu(&tree);
        assert_eq!(menu.panels(&tree).len(), 1);
        let branch = index_of(&tree, "scm.more.branch");
        let (x, y) = point(&menu, &tree, 0, branch);
        menu.pointer_moved(&tree, x, y);
        let panels = menu.panels(&tree);
        assert_eq!(panels.len(), 2);
        assert!(panels[1].x >= panels[0].x + panels[0].width - 3.0, "to the right");
        assert!(menu.contains(&tree, panels[1].x + 5.0, panels[1].y + 10.0));

        let (x, y) = point(&menu, &tree, 0, index_of(&tree, "scm.more.pull"));
        menu.pointer_moved(&tree, x, y);
        assert_eq!(menu.panels(&tree).len(), 1);
    }

    #[test]
    fn a_submenu_that_does_not_fit_on_the_right_opens_on_the_left() {
        let tree = more_menu_tree();
        let mut menu = MoreMenu::new(&tree, (700.0, 50.0), (800.0, 800.0), 0.0, |label| {
            label.len() as f32 * 8.0
        });
        let (x, y) = point(&menu, &tree, 0, index_of(&tree, "scm.more.branch"));
        menu.pointer_moved(&tree, x, y);
        let panels = menu.panels(&tree);
        assert!(panels[1].x < panels[0].x, "flipped to the left");
        for panel in &panels {
            assert!(panel.x >= 0.0 && panel.x + panel.width <= 800.0);
            assert!(panel.y >= 0.0 && panel.y + panel.height <= 800.0);
        }
    }

    #[test]
    fn clicking_a_leaf_returns_its_command_and_a_group_only_opens() {
        let tree = more_menu_tree();
        let mut menu = menu(&tree);
        let (x, y) = point(&menu, &tree, 0, index_of(&tree, "scm.more.stash"));
        assert_eq!(menu.click(&tree, x, y), None);
        assert_eq!(menu.panels(&tree).len(), 2);
        let stash = match &tree[index_of(&tree, "scm.more.stash")] {
            MoreNode::Item { children, .. } => children.clone(),
            MoreNode::Separator => unreachable!(),
        };
        let (x, y) = point(&menu, &tree, 1, index_of(&stash, "scm.more.stash_staged"));
        assert_eq!(menu.click(&tree, x, y), Some(MoreCommand::StashStaged));

        let (x, y) = point(&menu, &tree, 0, index_of(&tree, "scm.more.fetch"));
        assert_eq!(menu.click(&tree, x, y), Some(MoreCommand::Fetch));
    }

    #[test]
    fn the_keyboard_walks_levels_with_the_arrows_and_skips_separators() {
        let tree = more_menu_tree();
        let mut menu = menu(&tree);
        // Pull, Push, Checkout to, Fetch, (separator), Commit...
        for _ in 0..4 {
            menu.step(&tree, true);
        }
        assert_eq!(
            menu.selected[0],
            index_of(&tree, "scm.more.commit"),
            "the separator is skipped"
        );
        assert!(menu.enter_submenu(&tree));
        assert_eq!(menu.open.len(), 1);
        for _ in 0..4 {
            menu.step(&tree, true);
        }
        assert_eq!(menu.selected[1], 4);
        assert_eq!(menu.activate(&tree), Some(MoreCommand::AbortOperation));
        assert!(menu.leave_submenu());
        assert!(!menu.leave_submenu());
        // Up from the first row wraps to the last one.
        let mut menu = self::menu(&tree);
        menu.step(&tree, false);
        assert_eq!(menu.selected[0], index_of(&tree, "scm.more.tags"));
        assert_eq!(menu.activate(&tree), None, "Enter on a group opens it");
        assert_eq!(menu.open.len(), 1);
    }

    #[test]
    fn clicks_outside_every_panel_close_the_menu() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_scm_more_menu();
        assert!(app.scm_more_menu.is_some());
        app.scm_more_menu_click(5000.0, 5000.0);
        assert!(app.scm_more_menu.is_none());
        app.open_scm_more_menu();
        assert!(app.scm_more_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.scm_more_menu.is_none());
        assert!(!app.scm_more_menu_key(&Key::Named(NamedKey::Escape)));
    }

    #[test]
    fn commands_open_pickers_and_prompts_or_start_the_operation() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.run_more_command(MoreCommand::Merge);
        assert_eq!(app.branch_picker_action, BranchPickerAction::Merge);
        app.close_branch_picker();
        app.run_more_command(MoreCommand::DeleteBranchForce);
        assert_eq!(app.branch_picker_action, BranchPickerAction::DeleteForce);
        app.close_branch_picker();
        app.run_more_command(MoreCommand::CheckoutTo);
        assert!(app.branch_picker);
        app.close_branch_picker();

        app.run_more_command(MoreCommand::RenameBranch);
        assert_eq!(
            app.branch_name_prompt.as_ref().unwrap().target,
            NameTarget::RenameBranch
        );
        app.run_more_command(MoreCommand::CreateTag);
        assert_eq!(
            app.branch_name_prompt.as_ref().unwrap().target,
            NameTarget::TagHead
        );

        // Without an event loop an operation stops at this message, which shows it was started.
        for command in [
            MoreCommand::Pull,
            MoreCommand::PullRebase,
            MoreCommand::UndoLastCommit,
            MoreCommand::StashUntracked,
            MoreCommand::StashPopLatest,
        ] {
            app.status.clear();
            app.run_more_command(command);
            assert_eq!(app.status, app.t("scm.error.actions_need_loop"), "{command:?}");
        }
    }

    fn labels(rows: &[MoreNode]) -> Vec<&'static str> {
        rows.iter()
            .filter_map(|row| match row {
                MoreNode::Item { label, .. } => Some(*label),
                MoreNode::Separator => None,
            })
            .collect()
    }

    #[test]
    fn the_file_menu_offers_stage_and_discard_for_changes_and_only_unstage_for_staged_files() {
        let unstaged = labels(&file_menu_tree(false));
        assert!(unstaged.contains(&"scm.file.stage"));
        assert!(unstaged.contains(&"scm.file.discard"));
        assert!(!unstaged.contains(&"scm.file.unstage"));
        let staged = labels(&file_menu_tree(true));
        assert!(staged.contains(&"scm.file.unstage"));
        assert!(!staged.contains(&"scm.file.discard"), "staged changes are unstaged first");
        assert!(!staged.contains(&"scm.file.stage"));
    }

    fn app_with_file(dir: &TempDir, staged: bool) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm_snapshot.files = vec![crate::scm::FileStatus {
            path: "src/a.txt".into(),
            staged,
            unstaged: !staged,
            ..Default::default()
        }];
        app
    }

    #[test]
    fn choosing_discard_in_the_file_menu_asks_before_throwing_changes_away() {
        let dir = TempDir::new();
        let mut app = app_with_file(&dir, false);
        app.open_more_menu(
            MoreMenuKind::File {
                index: 0,
                staged: false,
            },
            (50.0, 50.0),
            (1200.0, 800.0),
            18.0,
            8.0,
        );
        let tree = app.open_more_tree();
        let menu = app.scm_more_menu.as_ref().unwrap();
        let (x, y) = point(menu, &tree, 0, index_of(&tree, "scm.file.discard"));
        app.scm_more_menu_click(x, y);
        assert!(app.scm_more_menu.is_none());
        let confirm = app.scm_confirm.as_ref().expect("a confirmation is shown first");
        assert!(confirm.message.contains("a.txt"));
    }

    #[test]
    fn adding_to_gitignore_writes_the_workspace_file_and_copy_path_uses_the_workspace() {
        let dir = TempDir::new();
        let mut app = app_with_file(&dir, false);
        let kind = MoreMenuKind::File {
            index: 0,
            staged: false,
        };
        app.run_more_command_for(MoreCommand::FileAddToGitignore, kind);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "/src/a.txt
"
        );
        assert_eq!(app.status, app.t("scm.file.ignored"));
        // Without an event loop the refresh replaces the snapshot, so list the file again.
        let mut app = app_with_file(&dir, false);
        app.run_more_command_for(MoreCommand::FileAddToGitignore, kind);
        assert_eq!(app.status, app.t("scm.file.already_ignored"));
        // A command for a file that is no longer listed does nothing.
        let mut app = app_with_file(&dir, false);
        app.scm_snapshot.files.clear();
        app.status.clear();
        app.run_more_command_for(MoreCommand::FileAddToGitignore, kind);
        assert!(app.status.is_empty());
    }

    #[test]
    fn staging_from_the_file_menu_selects_that_file_and_starts_the_operation() {
        let dir = TempDir::new();
        let mut app = app_with_file(&dir, false);
        app.scm_selected = 7;
        app.status.clear();
        app.run_more_command_for(
            MoreCommand::FileStage,
            MoreMenuKind::File {
                index: 0,
                staged: false,
            },
        );
        assert_eq!(app.scm_selected, 0);
        // Without an event loop the operation stops at this message, which shows it was started.
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
    }

    fn commit_commands(rows: &[MoreNode]) -> Vec<CommitOptions> {
        fn walk(rows: &[MoreNode], out: &mut Vec<CommitOptions>) {
            for row in rows {
                if let MoreNode::Item {
                    command, children, ..
                } = row
                {
                    if let Some(MoreCommand::Commit(options)) = command {
                        out.push(*options);
                    }
                    walk(children, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(rows, &mut out);
        out
    }

    #[test]
    fn the_commit_submenu_offers_every_scope_with_amend_and_sign_off() {
        let options = commit_commands(&more_menu_tree());
        assert_eq!(options.len(), 9, "{options:?}");
        for scope in [CommitScope::Smart, CommitScope::Staged, CommitScope::All] {
            for (amend, signoff) in [(false, false), (true, false), (false, true)] {
                assert_eq!(
                    options
                        .iter()
                        .filter(|o| o.scope == scope && o.amend == amend && o.signoff == signoff)
                        .count(),
                    1,
                    "{scope:?} amend={amend} signoff={signoff}"
                );
            }
        }
        assert!(options.iter().all(|o| o.post == PostCommit::Nothing));
    }

    #[test]
    fn the_commit_button_menu_commits_and_then_pushes_or_syncs() {
        let options = commit_commands(&commit_button_tree());
        assert_eq!(
            options.iter().map(|o| o.post).collect::<Vec<_>>(),
            [PostCommit::Nothing, PostCommit::Push, PostCommit::Sync]
        );
        assert!(options.iter().all(|o| !o.amend && !o.signoff));
    }

    #[test]
    fn the_commit_arrow_sits_at_the_right_end_of_the_button() {
        use super::super::scm_ui::{commit_arrow, ScmRect};
        let button = ScmRect {
            x: 40.0,
            y: 100.0,
            width: 240.0,
            height: 22.0,
        };
        let arrow = commit_arrow(button);
        assert_eq!(arrow.x + arrow.width, button.x + button.width);
        assert_eq!((arrow.y, arrow.height), (button.y, button.height));
        assert!(arrow.contains(button.x + button.width - 3.0, button.y + 5.0));
        assert!(!arrow.contains(button.x + 10.0, button.y + 5.0));
    }

    #[test]
    fn picking_a_commit_in_the_menu_commits_with_those_options() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_scm_commit_menu((40.0, 40.0));
        assert_eq!(
            app.scm_more_menu.as_ref().unwrap().kind,
            MoreMenuKind::CommitOptions
        );
        let tree = app.open_more_tree();
        let menu = app.scm_more_menu.as_ref().unwrap();
        let (x, y) = point(menu, &tree, 0, index_of(&tree, "scm.more.commit_push"));
        app.status.clear();
        app.scm_commit_message.clear();
        app.scm_more_menu_click(x, y);
        assert!(app.scm_more_menu.is_none());
        // The choice reached the commit action; without an event loop it stops at this message.
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
    }

    #[test]
    fn the_git_commands_are_still_listed_in_the_palette() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_scm_git_commands();
        assert_eq!(app.command_palette.as_ref().unwrap().query, "Git: ");
    }
}
