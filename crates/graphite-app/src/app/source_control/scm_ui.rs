use super::command_execution::{deliver_command_event, CommandError};
use super::scm_icons::{draw_scm_icon, ScmIcon};
use super::*;

pub(super) fn scm_centered_text_x(x: f32, width: f32, text: &str, cell_width: f32) -> f32 {
    (x + (width - text.chars().count() as f32 * cell_width) * 0.5).max(x)
}

pub(super) fn scm_file_label_x(icon_x: f32, icon_size: f32, icon_gap: f32) -> f32 {
    icon_x + icon_size + icon_gap
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScmRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ScmRect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScmListRowKind {
    Section {
        staged: bool,
        count: usize,
        collapsed: bool,
    },
    File {
        index: usize,
        staged: bool,
    },
    /// A folder of the tree view of a changes list. `first` is the snapshot index of its first
    /// file; the folder is the first `depth + 1` parts of that file's path.
    Folder {
        staged: bool,
        first: usize,
        depth: usize,
        count: usize,
        collapsed: bool,
    },
    /// A row of a source control an extension registered.
    Extension(crate::app::plugins::vsix::ExtensionRow),
    GraphHeader {
        collapsed: bool,
        count: usize,
    },
    GraphCommit {
        index: usize,
    },
    GraphCommitFile {
        commit_index: usize,
        file_index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScmListRow {
    pub bounds: ScmRect,
    pub chevron: ScmRect,
    pub action: ScmRect,
    pub kind: ScmListRowKind,
}

#[derive(Clone, Debug)]
pub(crate) struct ScmPanelLayout {
    pub repository_header: ScmRect,
    pub repository_chevron: ScmRect,
    pub repository_icon: ScmRect,
    pub repository_label: ScmRect,
    pub branch_row: ScmRect,
    pub branch_icon: ScmRect,
    pub branch_label: ScmRect,
    pub branch_picker: ScmRect,
    pub refresh: ScmRect,
    pub repo_sync: ScmRect,
    pub repo_pull_request: ScmRect,
    pub repo_more: ScmRect,
    pub commit_field: ScmRect,
    pub commit_button: ScmRect,
    pub list_top: f32,
    pub graph_divider: Option<ScmRect>,
    pub list_rows: Vec<ScmListRow>,
}

/// How the changes lists are shown.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScmListView<'a> {
    /// Folders instead of a flat list.
    pub tree: bool,
    pub collapsed_folders: Option<&'a super::scm_tree::Collapsed>,
}

impl ScmListView<'static> {
    #[cfg(test)]
    /// The flat list.
    pub(crate) const LIST: Self = Self {
        tree: false,
        collapsed_folders: None,
    };
}

/// The folder a [`ScmListRowKind::Folder`] row stands for.
pub(crate) fn folder_path(file: &Path, depth: usize) -> PathBuf {
    file.components().take(depth + 1).collect()
}

/// History and stash buttons of the Changes header, placed left of its stage-all button.
pub(crate) fn changes_header_buttons(action: ScmRect, cell_width: f32) -> (ScmRect, ScmRect) {
    let gap = cell_width * 0.5;
    let stash = ScmRect {
        x: action.x - gap - action.width,
        ..action
    };
    let history = ScmRect {
        x: stash.x - gap - stash.width,
        ..action
    };
    (history, stash)
}

/// The arrow at the right end of the commit button.
pub(crate) fn commit_arrow(commit_button: ScmRect) -> ScmRect {
    ScmRect {
        x: commit_button.x + commit_button.width - commit_button.height,
        width: commit_button.height,
        ..commit_button
    }
}

/// The discard button of a file row, left of its stage button.
pub(crate) fn discard_button(action: ScmRect, cell_width: f32) -> ScmRect {
    ScmRect {
        x: action.x - cell_width * 0.5 - action.width,
        ..action
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn scm_panel_layout(
    snapshot: &crate::scm::Snapshot,
    sidebar_width: f32,
    height: f32,
    row_height: f32,
    cell_width: f32,
    scroll: usize,
    repositories_collapsed: bool,
    staged_collapsed: bool,
    changes_collapsed: bool,
    extension_rows: &[crate::app::plugins::vsix::ExtensionRow],
) -> ScmPanelLayout {
    scm_panel_layout_with_graph(
        snapshot,
        sidebar_width,
        height,
        row_height,
        cell_width,
        scroll,
        repositories_collapsed,
        staged_collapsed,
        changes_collapsed,
        extension_rows,
        false,
        false,
        0,
        0,
        None,
        &ScmListView::LIST,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn scm_panel_layout_with_graph(
    snapshot: &crate::scm::Snapshot,
    sidebar_width: f32,
    height: f32,
    row_height: f32,
    cell_width: f32,
    scroll: usize,
    repositories_collapsed: bool,
    staged_collapsed: bool,
    changes_collapsed: bool,
    extension_rows: &[crate::app::plugins::vsix::ExtensionRow],
    show_graph: bool,
    graph_collapsed: bool,
    graph_count: usize,
    graph_scroll: usize,
    graph_height: Option<f32>,
    view: &ScmListView<'_>,
) -> ScmPanelLayout {
    scm_panel_layout_with_graph_entries(
        snapshot,
        sidebar_width,
        height,
        row_height,
        cell_width,
        scroll,
        repositories_collapsed,
        staged_collapsed,
        changes_collapsed,
        extension_rows,
        show_graph,
        graph_collapsed,
        graph_count,
        graph_scroll,
        graph_height,
        view,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn scm_panel_layout_with_graph_entries(
    snapshot: &crate::scm::Snapshot,
    sidebar_width: f32,
    height: f32,
    row_height: f32,
    cell_width: f32,
    scroll: usize,
    repositories_collapsed: bool,
    staged_collapsed: bool,
    changes_collapsed: bool,
    extension_rows: &[crate::app::plugins::vsix::ExtensionRow],
    show_graph: bool,
    graph_collapsed: bool,
    graph_count: usize,
    graph_scroll: usize,
    graph_height: Option<f32>,
    view: &ScmListView<'_>,
    graph_entries: Option<&[ScmListRowKind]>,
) -> ScmPanelLayout {
    let left = ACTIVITY_BAR_WIDTH + PAD;
    let right = sidebar_width - PAD;
    let full_row = |y| ScmRect {
        x: ACTIVITY_BAR_WIDTH,
        y,
        width: sidebar_width - ACTIVITY_BAR_WIDTH,
        height: row_height,
    };
    let icon_size = (row_height * 0.70).round();
    let icon_x = left + cell_width;
    let icon_gap = cell_width * 0.55;
    let repo_y = row_height * 2.0;
    let branch_y = row_height * 3.0;
    let repo_offset = if repositories_collapsed { -2.0 } else { 0.0 };
    let commit_y = row_height * (4.0 + repo_offset);
    let button_width = (cell_width * 2.2).max(20.0);
    let button_gap = (cell_width * 0.75).max(7.0);
    let refresh_x = right - button_width;
    let picker_x = refresh_x - button_gap - button_width;
    let repo_more_x = right - button_width;
    let repo_pr_x = repo_more_x - button_gap - button_width;
    let repo_sync_x = repo_pr_x - button_gap - button_width;
    let action_width = cell_width * 2.0;
    let chevron_slot = cell_width * 1.6;
    let action_rect = |y| ScmRect {
        x: right - cell_width * 3.1,
        y,
        width: action_width,
        height: row_height,
    };
    let mut entries = Vec::new();
    let staged_count = snapshot.files.iter().filter(|file| file.staged).count();
    let changes_count = snapshot
        .files
        .iter()
        .filter(|file| file.unstaged || file.untracked || file.conflicted)
        .count();
    // Without a repository there is no Git list; extension providers are still listed.
    if snapshot.error.is_some() {
        entries.extend(
            extension_rows
                .iter()
                .map(|row| ScmListRowKind::Extension(*row)),
        );
    } else {
        let list_entries = |staged: bool, entries: &mut Vec<ScmListRowKind>| {
            use super::scm_tree::{tree_rows, Collapsed, TreeRow};
            let files: Vec<(usize, &Path)> = snapshot
                .files
                .iter()
                .enumerate()
                .filter(|(_, file)| {
                    if staged {
                        file.staged
                    } else {
                        file.unstaged || file.untracked || file.conflicted
                    }
                })
                .map(|(index, file)| (index, file.path.as_path()))
                .collect();
            if view.tree {
                let none = Collapsed::new();
                let collapsed = view.collapsed_folders.unwrap_or(&none);
                for row in tree_rows(&files, staged, collapsed) {
                    entries.push(match row {
                        TreeRow::Folder {
                            path,
                            depth,
                            first,
                            count,
                        } => ScmListRowKind::Folder {
                            staged,
                            first,
                            depth,
                            count,
                            collapsed: collapsed.contains(&(staged, path)),
                        },
                        TreeRow::File { index, .. } => ScmListRowKind::File { index, staged },
                    });
                }
            } else {
                entries.extend(files.iter().map(|(index, _)| ScmListRowKind::File {
                    index: *index,
                    staged,
                }));
            }
        };
        if staged_count > 0 {
            entries.push(ScmListRowKind::Section {
                staged: true,
                count: staged_count,
                collapsed: staged_collapsed,
            });
            if !staged_collapsed {
                list_entries(true, &mut entries);
            }
        }
        entries.push(ScmListRowKind::Section {
            staged: false,
            count: changes_count,
            collapsed: changes_collapsed,
        });
        if !changes_collapsed {
            list_entries(false, &mut entries);
        }
        entries.extend(
            extension_rows
                .iter()
                .map(|row| ScmListRowKind::Extension(*row)),
        );
    }
    let list_top = commit_y + row_height * 2.50;
    let list_start_y = list_top + row_height;
    let list_end_y = (height - row_height * 0.25).max(list_start_y);
    let make_row = |y: f32, kind: ScmListRowKind| ScmListRow {
        bounds: full_row(y),
        chevron: ScmRect {
            x: left,
            y,
            width: chevron_slot,
            height: row_height,
        },
        action: action_rect(y),
        kind,
    };

    let (list_rows, graph_divider) = if show_graph && snapshot.error.is_none() {
        let total_h = (list_end_y - list_start_y).max(row_height * 2.0);
        let graph_area_h = if graph_collapsed {
            row_height
        } else {
            let max_graph_h = (total_h - row_height * 2.0).max(row_height * 2.0);
            graph_height
                .unwrap_or(total_h * 0.44)
                .clamp(row_height * 2.0, max_graph_h)
        };
        let graph_header_y = (list_end_y - graph_area_h).max(list_start_y);
        let divider = ScmRect {
            x: ACTIVITY_BAR_WIDTH,
            y: graph_header_y - 3.0,
            width: sidebar_width - ACTIVITY_BAR_WIDTH,
            height: 6.0,
        };
        let top_cap = ((graph_header_y - list_start_y) / row_height)
            .floor()
            .max(0.0) as usize;
        let mut rows: Vec<ScmListRow> = entries
            .into_iter()
            .skip(scroll)
            .take(top_cap)
            .enumerate()
            .map(|(visible, kind)| make_row(list_start_y + row_height * visible as f32, kind))
            .collect();
        rows.push(make_row(
            graph_header_y,
            ScmListRowKind::GraphHeader {
                collapsed: graph_collapsed,
                count: graph_count,
            },
        ));
        if !graph_collapsed {
            let commit_cap = ((list_end_y - (graph_header_y + row_height)) / row_height)
                .floor()
                .max(0.0) as usize;
            if let Some(entries) = graph_entries {
                rows.extend(
                    entries
                        .iter()
                        .copied()
                        .skip(graph_scroll)
                        .take(commit_cap)
                        .enumerate()
                        .map(|(visible, kind)| {
                            make_row(graph_header_y + row_height * (visible as f32 + 1.0), kind)
                        }),
                );
            } else {
                rows.extend(
                    (0..graph_count)
                        .skip(graph_scroll)
                        .take(commit_cap)
                        .enumerate()
                        .map(|(visible, index)| {
                            make_row(
                                graph_header_y + row_height * (visible as f32 + 1.0),
                                ScmListRowKind::GraphCommit { index },
                            )
                        }),
                );
            }
        }
        (rows, Some(divider))
    } else {
        let available = ((height - list_top - row_height * 1.2) / row_height).max(0.0) as usize;
        let rows = entries
            .into_iter()
            .skip(scroll)
            .take(available)
            .enumerate()
            .map(|(visible, kind)| make_row(list_start_y + row_height * visible as f32, kind))
            .collect();
        (rows, None)
    };

    ScmPanelLayout {
        repository_header: full_row(row_height),
        repository_chevron: ScmRect {
            x: left,
            y: row_height,
            width: chevron_slot,
            height: row_height,
        },
        repository_icon: ScmRect {
            x: icon_x,
            y: repo_y + (row_height - icon_size) * 0.5,
            width: icon_size,
            height: icon_size,
        },
        repository_label: ScmRect {
            x: scm_file_label_x(icon_x, icon_size, icon_gap),
            y: repo_y,
            width: repo_sync_x - button_gap * 0.5 - scm_file_label_x(icon_x, icon_size, icon_gap),
            height: row_height,
        },
        branch_row: full_row(branch_y),
        branch_icon: ScmRect {
            x: icon_x,
            y: branch_y + (row_height - icon_size) * 0.5,
            width: icon_size,
            height: icon_size,
        },
        branch_label: ScmRect {
            x: scm_file_label_x(icon_x, icon_size, icon_gap),
            y: branch_y,
            width: picker_x - button_gap * 0.5 - scm_file_label_x(icon_x, icon_size, icon_gap),
            height: row_height,
        },
        branch_picker: ScmRect {
            x: picker_x,
            y: branch_y,
            width: button_width,
            height: row_height,
        },
        refresh: ScmRect {
            x: refresh_x,
            y: branch_y,
            width: button_width,
            height: row_height,
        },
        repo_sync: ScmRect {
            x: repo_sync_x,
            y: repo_y,
            width: button_width,
            height: row_height,
        },
        repo_pull_request: ScmRect {
            x: repo_pr_x,
            y: repo_y,
            width: button_width,
            height: row_height,
        },
        repo_more: ScmRect {
            x: repo_more_x,
            y: repo_y,
            width: button_width,
            height: row_height,
        },
        commit_field: ScmRect {
            x: left,
            y: commit_y,
            width: right - left,
            height: row_height,
        },
        commit_button: ScmRect {
            x: left,
            y: commit_y + row_height * 1.30,
            width: right - left,
            height: row_height,
        },
        list_top,
        graph_divider,
        list_rows,
    }
}

pub(super) fn scm_fit_label(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_owned();
    }
    match max_chars {
        0 => String::new(),
        1 => "…".to_owned(),
        _ => format!("{}…", text.chars().take(max_chars - 1).collect::<String>()),
    }
}

pub(super) fn scm_visible_tail(text: &str, max_chars: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    chars[chars.len().saturating_sub(max_chars)..]
        .iter()
        .collect()
}

pub(super) fn scm_icon_color(color: u32) -> u32 {
    let dark = Theme::color(0x000000, 0xffffff) == 0x000000;
    let channels = [
        ((color >> 16) & 0xff),
        ((color >> 8) & 0xff),
        (color & 0xff),
    ];
    let limit = if dark { 128 } else { 190 };
    if channels.iter().copied().max().unwrap_or_default() >= limit {
        return color;
    }
    let target = if dark { 0xff } else { 0x20 };
    let blend = |channel: u32| (channel * 55 + target * 45) / 100;
    (blend(channels[0]) << 16) | (blend(channels[1]) << 8) | blend(channels[2])
}

/// Hover background of an icon button. Idle buttons draw no frame, only their icon.
pub(super) fn draw_scm_control_slot(renderer: &mut Renderer, bounds: ScmRect, hovered: bool) {
    if !hovered {
        return;
    }
    let inset = 2.0;
    renderer.draw_rect(
        bounds.x + inset,
        bounds.y + inset,
        (bounds.width - inset * 2.0).max(1.0),
        (bounds.height - inset * 2.0).max(1.0),
        rgb(Theme::color(0x45474c, 0xdcdde0)),
    );
}

/// Icon color of a button: dim at rest, full strength under the pointer.
pub(super) fn scm_button_color(hovered: bool) -> graphite_render::Color {
    rgb(if hovered {
        Theme::color(Theme::TEXT, 0x1e1e1e)
    } else {
        Theme::color(Theme::DIM, 0x616161)
    })
}

#[cfg(test)]
mod scm_sidebar_layout_tests {
    use super::*;

    #[test]
    fn commit_label_is_centered_in_the_button() {
        let x = scm_centered_text_x(56.0, 244.0, "✓ Commit", 9.0);
        assert_eq!(x, 142.0);
    }

    #[test]
    fn primary_button_follows_the_branch_state() {
        use crate::scm::{FileStatus, Snapshot};
        let base = Snapshot {
            branch: Some("main".into()),
            has_commits: true,
            has_remote: true,
            upstream: Some("origin/main".into()),
            ..Default::default()
        };
        assert_eq!(scm_primary_action_for(&base), ScmPrimaryAction::Commit);

        let ahead = Snapshot {
            ahead: 1,
            ..base.clone()
        };
        assert_eq!(
            scm_primary_action_for(&ahead),
            ScmPrimaryAction::Sync {
                ahead: 1,
                behind: 0
            }
        );
        let behind = Snapshot {
            behind: 2,
            ..base.clone()
        };
        assert_eq!(
            scm_primary_action_for(&behind),
            ScmPrimaryAction::Sync {
                ahead: 0,
                behind: 2
            }
        );

        let dirty = Snapshot {
            ahead: 1,
            files: vec![FileStatus::default()],
            ..base.clone()
        };
        assert_eq!(scm_primary_action_for(&dirty), ScmPrimaryAction::Commit);

        let untracked_branch = Snapshot {
            upstream: None,
            ..base.clone()
        };
        assert_eq!(
            scm_primary_action_for(&untracked_branch),
            ScmPrimaryAction::PublishBranch
        );

        let no_remote = Snapshot {
            has_remote: false,
            upstream: None,
            ..base.clone()
        };
        assert_eq!(
            scm_primary_action_for(&no_remote),
            ScmPrimaryAction::PublishRepository
        );

        let detached = Snapshot {
            branch: None,
            upstream: None,
            ..base
        };
        assert_eq!(scm_primary_action_for(&detached), ScmPrimaryAction::Commit);
    }

    #[test]
    fn file_name_starts_after_the_icon_and_its_gap() {
        let icon_x = 64.0;
        let icon_size = 14.0;
        let gap = 5.0;
        let label_x = scm_file_label_x(icon_x, icon_size, gap);
        assert_eq!(label_x - (icon_x + icon_size), gap);
    }

    #[test]
    fn without_a_repository_only_extension_rows_are_listed() {
        use crate::app::plugins::vsix::ExtensionRow;
        let snapshot = crate::scm::Snapshot {
            error: Some("not a repository".into()),
            ..Default::default()
        };
        let extension = [ExtensionRow::Provider(1), ExtensionRow::Input(1)];
        let layout = scm_panel_layout(
            &snapshot,
            SIDEBAR_WIDTH,
            700.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
            &extension,
        );
        assert_eq!(layout.list_rows.len(), 2);
        assert!(layout
            .list_rows
            .iter()
            .all(|row| matches!(row.kind, ScmListRowKind::Extension(_))));
        let with_git = scm_panel_layout(
            &crate::scm::Snapshot::default(),
            SIDEBAR_WIDTH,
            700.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
            &extension,
        );
        assert!(matches!(
            with_git.list_rows[0].kind,
            ScmListRowKind::Section { .. }
        ));
        assert!(matches!(
            with_git.list_rows.last().unwrap().kind,
            ScmListRowKind::Extension(ExtensionRow::Input(1))
        ));
    }

    #[test]
    fn branch_buttons_have_separate_hit_areas_and_a_visible_gap() {
        let layout = scm_panel_layout(
            &crate::scm::Snapshot::default(),
            SIDEBAR_WIDTH,
            700.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
            &[],
        );
        assert!(layout.branch_picker.x + layout.branch_picker.width < layout.refresh.x);
        assert!(layout.branch_picker.contains(
            layout.branch_picker.x + layout.branch_picker.width * 0.5,
            layout.branch_picker.y + layout.branch_picker.height * 0.5,
        ));
        assert!(!layout.branch_picker.contains(
            layout.refresh.x + layout.refresh.width * 0.5,
            layout.refresh.y + layout.refresh.height * 0.5,
        ));
    }

    #[test]
    fn repository_and_changes_chevrons_share_a_column_and_action_button_is_separate() {
        let mut snapshot = crate::scm::Snapshot::default();
        snapshot.files.push(crate::scm::FileStatus {
            path: PathBuf::from("a.rs"),
            unstaged: true,
            ..Default::default()
        });
        let layout = scm_panel_layout(
            &snapshot,
            SIDEBAR_WIDTH,
            700.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
            &[],
        );
        let changes = layout
            .list_rows
            .iter()
            .find(|row| matches!(row.kind, ScmListRowKind::Section { staged: false, .. }))
            .unwrap();
        assert_eq!(layout.repository_chevron.x, changes.chevron.x);
        assert_eq!(layout.repository_chevron.width, changes.chevron.width);
        assert!(changes.action.x > layout.repository_header.x);
        assert!(changes.action.x + changes.action.width <= SIDEBAR_WIDTH - PAD - 9.0);
    }

    #[test]
    fn long_branch_names_are_truncated_to_the_available_slot() {
        assert_eq!(
            scm_fit_label("feature/a-very-long-branch", 10),
            "feature/a…"
        );
        assert_eq!(scm_fit_label("main", 10), "main");
    }

    #[test]
    fn long_commit_message_keeps_its_end_visible_without_splitting_unicode() {
        assert_eq!(scm_visible_tail("mensagem longa 🌎 final", 8), " 🌎 final");
        assert_eq!(scm_visible_tail("texto", 20), "texto");
        assert_eq!(scm_visible_tail("texto", 0), "");
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DiffView {
    pub review: Option<(String, u64)>,
    pub title: String,
    pub left_name: String,
    pub right_name: String,
    pub left: String,
    pub right: String,
    pub inline: bool,
    pub collapse_unchanged: bool,
    pub selected_change: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ConflictView {
    pub path: PathBuf,
    pub sides: [Option<String>; 3],
}

impl App {
    pub(crate) fn scm_workspace_root(&self) -> PathBuf {
        self.documents[self.active_document()]
            .buffer
            .path()
            .map(|path| self.tree.root_for(path).to_path_buf())
            .unwrap_or_else(|| self.tree.root().to_path_buf())
    }

    pub(crate) fn refresh_scm_async(&mut self) {
        let root = self.scm_workspace_root();
        let graph_limit = self.scm.graph_limit;
        if let Some(proxy) = self.event_proxy.clone() {
            if !self.scm.refresh_gate.try_start() {
                return;
            }
            let graph_root = root.clone();
            let graph_proxy = proxy.clone();
            if std::thread::Builder::new()
                .name("graphite-scm-status".into())
                .spawn(move || {
                    let snapshot = crate::scm::snapshot(&root);
                    let _ = proxy.send_event(UserEvent::ScmSnapshot(root, snapshot));
                })
                .is_ok()
            {
                self.scm.refresh_gate.started();
            }
            if std::thread::Builder::new()
                .name("graphite-scm-graph".into())
                .spawn(move || {
                    let rows = crate::vcs::scm_graph::load_graph_commits(&graph_root, graph_limit);
                    let _ = graph_proxy.send_event(UserEvent::ScmGraph(graph_root, rows));
                })
                .is_ok()
            {
                self.scm.refresh_gate.started();
            }
        } else {
            self.scm.snapshot = crate::scm::snapshot(&root);
            self.scm.graph_rows =
                crate::vcs::scm_graph::load_graph_commits(&root, graph_limit).unwrap_or_default();
        }
    }

    pub(crate) fn refresh_scm_graph_async(&mut self) {
        let root = self.scm_workspace_root();
        let graph_limit = self.scm.graph_limit;
        if let Some(proxy) = self.event_proxy.clone() {
            std::thread::Builder::new()
                .name("graphite-scm-graph".into())
                .spawn(move || {
                    let rows = crate::vcs::scm_graph::load_graph_commits(&root, graph_limit);
                    let _ = proxy.send_event(UserEvent::ScmGraph(root, rows));
                })
                .ok();
        } else {
            self.scm.graph_rows =
                crate::vcs::scm_graph::load_graph_commits(&root, graph_limit).unwrap_or_default();
        }
    }

    pub(crate) fn accept_scm_snapshot(&mut self, snapshot: crate::scm::Snapshot) {
        self.scm.snapshot = snapshot;
        self.scm.selected = self
            .scm
            .selected
            .min(self.scm.snapshot.files.len().saturating_sub(1));
    }

    pub(crate) fn toggle_scm(&mut self) {
        self.scm.visible = !self.scm.visible;
        self.scm.focused = self.scm.visible;
        if self.scm.visible {
            self.close_plugins();
            self.refresh_scm_async();
        }
    }

    pub(crate) fn scm_selected_path(&self) -> Option<PathBuf> {
        self.scm
            .snapshot
            .files
            .get(self.scm.selected)
            .map(|item| self.scm_workspace_root().join(&item.path))
    }

    pub(crate) fn open_branch_picker(&mut self) {
        self.branch_picker = true;
        self.branch_selected = self
            .scm
            .snapshot
            .branches
            .iter()
            .position(|branch| Some(branch) == self.scm.snapshot.branch.as_ref())
            .unwrap_or(0);
        self.status = self.t("scm.branch.select_hint").into();
    }

    pub(crate) fn checkout_selected_branch(&mut self) {
        if self.documents.iter().any(|tab| tab.is_dirty()) {
            self.fail_current_command(CommandError::Failed(self.t("scm.checkout.unsaved").into()));
            return;
        }
        let Some(branch) = self
            .branch_picker_entries()
            .get(self.branch_selected)
            .cloned()
        else {
            return;
        };
        self.branch_picker = false;
        let root = self.scm_workspace_root();
        let open_paths = self
            .documents
            .iter()
            .filter_map(|tab| tab.buffer.path().map(PathBuf::from))
            .collect::<Vec<_>>();
        let Some(proxy) = self.event_proxy.clone() else {
            self.fail_current_command(CommandError::Failed(
                self.t("scm.error.no_event_loop").into(),
            ));
            return;
        };
        let translator = self.translator;
        let job = self.command_job(false);
        let spawned = std::thread::Builder::new()
            .name("graphite-git-checkout".into())
            .spawn(move || {
                if job
                    .as_ref()
                    .is_some_and(|job| job.cancellation.is_cancelled())
                {
                    return;
                }
                let result = crate::scm::checkout_branch(&root, &branch, &translator);
                let outcome = result
                    .as_ref()
                    .map(|_| serde_json::Value::Null)
                    .map_err(|error| CommandError::Failed(error.clone()));
                let documents = if result.is_ok() {
                    open_paths
                        .into_iter()
                        .map(|path| {
                            let decoded = crate::file_format::read(&path, &translator);
                            (path, decoded)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let message = result
                    .map(|_| translator.format("scm.checkout.success", &[("branch", &branch)]))
                    .unwrap_or_else(|error| {
                        translator.format("scm.checkout.failed", &[("error", &error)])
                    });
                deliver_command_event(
                    &proxy,
                    job.as_ref(),
                    UserEvent::BranchCheckoutCompleted {
                        message,
                        snapshot: crate::scm::snapshot(&root),
                        documents,
                    },
                    outcome,
                );
            });
        if let Err(error) = spawned {
            self.end_scm_busy();
            self.fail_current_command(CommandError::Failed(error.to_string()));
        }
    }

    /// Stages or unstages every file of a list that is at or below `folder`.
    pub(crate) fn stage_folder(&mut self, staged: bool, folder: &Path) {
        self.scm.batch_paths = self
            .scm
            .snapshot
            .files
            .iter()
            .filter(|file| {
                file.path.starts_with(folder)
                    && if staged {
                        file.staged
                    } else {
                        file.unstaged || file.untracked
                    }
            })
            .map(|file| file.path.clone())
            .collect();
        self.perform_scm_action(if staged {
            ScmAction::UnstageFolder
        } else {
            ScmAction::StageFolder
        });
    }

    /// Opens or closes a folder of the tree view.
    pub(crate) fn toggle_scm_folder(&mut self, staged: bool, folder: PathBuf) {
        let key = (staged, folder);
        if !self.scm.collapsed_folders.remove(&key) {
            self.scm.collapsed_folders.insert(key);
        }
    }

    /// What the big button under the commit message does right now.
    pub(crate) fn scm_primary_action(&self) -> ScmPrimaryAction {
        scm_primary_action_for(&self.scm.snapshot)
    }

    pub(crate) fn perform_scm_action(&mut self, action: ScmAction) {
        if let ScmAction::Commit(options) = action {
            if self.defer_commit_for_unsaved(options) {
                return;
            }
        }
        let root = self.scm_workspace_root();
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = self.t("scm.error.actions_need_loop").into();
            self.fail_current_command(CommandError::Failed(self.status.clone()));
            return;
        };
        let active_path = self.documents[self.command_document()]
            .buffer
            .path()
            .map(PathBuf::from);
        let selected = match action {
            ScmAction::StageCursorLine
            | ScmAction::RevertCursorLine
            | ScmAction::RevertFirstHunk => active_path,
            ScmAction::StageHunk(_) if !self.scm.visible => active_path,
            _ if self.scm.visible => self.scm_selected_path(),
            _ => active_path,
        };
        let batch_paths = match action {
            ScmAction::StageAll => self
                .scm
                .snapshot
                .files
                .iter()
                .filter(|file| file.unstaged || file.untracked)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            ScmAction::StageFolder | ScmAction::UnstageFolder => {
                std::mem::take(&mut self.scm.batch_paths)
            }
            ScmAction::UnstageAll => self
                .scm
                .snapshot
                .files
                .iter()
                .filter(|file| file.staged)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let commit_request = match action {
            ScmAction::Commit(options) => match self.build_commit_request(options) {
                Ok(request) => {
                    self.scm.commit_pending = true;
                    self.begin_scm_busy(super::scm_progress::ScmBusyKind::Commit);
                    Some(request)
                }
                Err(error) => {
                    self.status = error;
                    self.fail_current_command(CommandError::Failed(self.status.clone()));
                    return;
                }
            },
            _ => None,
        };
        let line = self.editor().cursor_line_col().0;
        self.status = self.t("scm.status.running").into();
        let translator = self.translator;
        let job = self.command_job(false);
        let spawned = std::thread::Builder::new()
            .name("graphite-scm-operation".into())
            .spawn(move || {
                if job
                    .as_ref()
                    .is_some_and(|job| job.cancellation.is_cancelled())
                {
                    return;
                }
                let result = match action {
                    ScmAction::StageAll => {
                        crate::scm::stage_files(&root, &batch_paths, &translator)
                            .map(|_| translator.text("scm.status.all_staged").to_owned())
                    }
                    ScmAction::StageFolder => {
                        crate::scm::stage_files(&root, &batch_paths, &translator)
                            .map(|_| translator.text("scm.status.all_staged").to_owned())
                    }
                    ScmAction::UnstageFolder => {
                        crate::scm::unstage_files(&root, &batch_paths, &translator)
                            .map(|_| translator.text("scm.status.all_unstaged").to_owned())
                    }
                    ScmAction::UnstageAll => {
                        crate::scm::unstage_files(&root, &batch_paths, &translator)
                            .map(|_| translator.text("scm.status.all_unstaged").to_owned())
                    }
                    ScmAction::Stage => selected
                        .ok_or_else(|| translator.text("scm.error.select_changed").to_owned())
                        .and_then(|path| {
                            crate::scm::stage_file(&root, &path, &translator)
                                .map(|_| translator.text("scm.status.file_staged").to_owned())
                        }),
                    ScmAction::Unstage => selected
                        .ok_or_else(|| translator.text("scm.error.select_staged").to_owned())
                        .and_then(|path| {
                            crate::scm::unstage_file(&root, &path, &translator)
                                .map(|_| translator.text("scm.status.file_unstaged").to_owned())
                        }),
                    ScmAction::Commit(_) => commit_request
                        .as_ref()
                        .ok_or_else(|| translator.text("scm.error.nothing_to_commit").to_owned())
                        .and_then(|request| crate::scm_commit::commit(&root, request)),
                    ScmAction::StageCursorLine => selected
                        .ok_or_else(|| translator.text("scm.error.select_changed").to_owned())
                        .and_then(|path| {
                            crate::scm::stage_line(&root, &path, line, &translator)
                                .map(|_| translator.text("scm.status.line_staged").to_owned())
                        }),
                    ScmAction::RevertCursorLine => selected
                        .ok_or_else(|| translator.text("scm.error.select_changed").to_owned())
                        .and_then(|path| {
                            crate::scm::revert_line(&root, &path, line, &translator)
                                .map(|_| translator.text("scm.status.line_reverted").to_owned())
                        }),
                    ScmAction::StageHunk(index) => selected
                        .ok_or_else(|| translator.text("scm.error.select_changed").to_owned())
                        .and_then(|path| {
                            crate::scm::stage_hunk(&root, &path, index, &translator)
                                .map(|_| translator.text("scm.status.hunk_staged").to_owned())
                        }),
                    ScmAction::RevertFirstHunk => selected
                        .ok_or_else(|| translator.text("scm.error.select_changed").to_owned())
                        .and_then(|path| {
                            crate::scm::revert_hunk(&root, &path, 0, &translator).map(|_| {
                                translator.text("scm.status.first_hunk_reverted").to_owned()
                            })
                        }),
                };
                let outcome = result
                    .as_ref()
                    .map(|_| serde_json::Value::Null)
                    .map_err(|error| CommandError::Failed(error.clone()));
                let snapshot = crate::scm::snapshot(&root);
                let message = result.unwrap_or_else(|error| {
                    translator.format("scm.error.operation_failed", &[("error", &error)])
                });
                deliver_command_event(
                    &proxy,
                    job.as_ref(),
                    UserEvent::ScmOperationCompleted {
                        message,
                        snapshot: Some(snapshot),
                    },
                    outcome,
                );
            });
        if let Err(error) = spawned {
            self.fail_current_command(CommandError::Failed(error.to_string()));
        }
    }

    pub(crate) fn open_diff_paths(&mut self, left: PathBuf, right: PathBuf) {
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = self.t("scm.error.diff_needs_loop").into();
            return;
        };
        let translator = self.translator;
        self.diff_loading = true;
        std::thread::Builder::new()
            .name("graphite-diff-load".into())
            .spawn(move || {
                let read = |path: &PathBuf| {
                    std::fs::read_to_string(path).map_err(|error| {
                        translator.format(
                            "scm.error.cannot_read",
                            &[("path", &path.display()), ("error", &error)],
                        )
                    })
                };
                match (read(&left), read(&right)) {
                    (Ok(left_text), Ok(right_text)) => {
                        let _ = proxy.send_event(UserEvent::DiffReady(DiffView {
                            review: None,
                            title: translator.text("scm.diff.file_comparison").to_owned(),
                            left_name: left.display().to_string(),
                            right_name: right.display().to_string(),
                            left: left_text,
                            right: right_text,
                            inline: false,
                            collapse_unchanged: false,
                            selected_change: 0,
                        }));
                    }
                    (left_result, right_result) => {
                        let message = left_result
                            .err()
                            .or_else(|| right_result.err())
                            .unwrap_or_else(|| translator.text("scm.diff.load_failed").to_owned());
                        let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                            message,
                            snapshot: None,
                        });
                    }
                }
            })
            .ok();
    }

    pub(crate) fn open_active_file_diff(&mut self) {
        let Some(path) = self.documents[self.command_document()]
            .buffer
            .path()
            .map(PathBuf::from)
        else {
            self.status = self.t("scm.diff.save_first").into();
            return;
        };
        let root = self.scm_workspace_root();
        let Some(proxy) = self.event_proxy.clone() else {
            self.fail_current_command(CommandError::Failed(
                self.t("scm.error.no_event_loop").into(),
            ));
            return;
        };
        let translator = self.translator;
        self.diff_loading = true;
        let current = self.documents[self.command_document()].buffer.text();
        let job = self.command_job(true);
        let spawned = std::thread::Builder::new()
            .name("graphite-head-diff".into())
            .spawn(move || {
                if job
                    .as_ref()
                    .is_some_and(|job| job.cancellation.is_cancelled())
                {
                    return;
                }
                let result = (|| {
                    let previous = crate::scm::head_file_text(&root, &path, &translator)?;
                    Ok::<_, String>(DiffView {
                        review: None,
                        title: translator.text("scm.diff.working_tree_head").to_owned(),
                        left_name: translator
                            .format("scm.diff.head_label", &[("path", &path.display())]),
                        right_name: translator
                            .format("scm.diff.working_tree_label", &[("path", &path.display())]),
                        left: previous,
                        right: current,
                        inline: false,
                        collapse_unchanged: false,
                        selected_change: 0,
                    })
                })();
                match result {
                    Ok(view) => {
                        deliver_command_event(
                            &proxy,
                            job.as_ref(),
                            UserEvent::DiffReady(view),
                            Ok(serde_json::Value::Null),
                        );
                    }
                    Err(error) => {
                        deliver_command_event(
                            &proxy,
                            job.as_ref(),
                            UserEvent::ScmOperationCompleted {
                                message: translator
                                    .format("scm.error.compare_head", &[("error", &error)]),
                                snapshot: None,
                            },
                            Err(CommandError::Failed(error)),
                        );
                    }
                }
            });
        if let Err(error) = spawned {
            self.fail_current_command(CommandError::Failed(error.to_string()));
        }
    }

    pub(crate) fn stage_saved_conflict(&mut self, path: &Path) {
        let root = self.scm_workspace_root();
        let is_conflict = self
            .scm
            .snapshot
            .files
            .iter()
            .any(|item| item.conflicted && crate::same_file_path(&root.join(&item.path), path));
        if !is_conflict {
            return;
        }
        let path = path.to_path_buf();
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let translator = self.translator;
        std::thread::Builder::new()
            .name("graphite-conflict-save".into())
            .spawn(move || {
                let result = crate::scm::stage_file(&root, &path, &translator);
                let message = result
                    .map(|_| translator.text("scm.conflict.saved_staged").to_owned())
                    .unwrap_or_else(|error| {
                        translator.format("scm.conflict.save_stage_failed", &[("error", &error)])
                    });
                let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                    message,
                    snapshot: Some(crate::scm::snapshot(&root)),
                });
            })
            .ok();
    }

    pub(crate) fn open_selected_conflict(&mut self) {
        let in_conflict = self
            .scm
            .snapshot
            .files
            .get(self.scm.selected)
            .is_some_and(|item| item.conflicted);
        if in_conflict {
            if let Some(file) = self.scm_selected_path() {
                // A file with conflict markers is resolved in the merge editor; anything else
                // (a deleted side, for example) falls back to the three-way view below.
                if self.open_merge_editor(file).is_ok() {
                    return;
                }
            }
        }
        let Some(path) = self.scm_selected_path() else {
            return;
        };
        let root = self.scm_workspace_root();
        let Some(proxy) = self.event_proxy.clone() else {
            self.fail_current_command(CommandError::Failed(
                self.t("scm.error.no_event_loop").into(),
            ));
            return;
        };
        let translator = self.translator;
        let job = self.command_job(false);
        let spawned = std::thread::Builder::new()
            .name("graphite-conflict-load".into())
            .spawn(move || {
                if job
                    .as_ref()
                    .is_some_and(|job| job.cancellation.is_cancelled())
                {
                    return;
                }
                match crate::scm::merge_conflict(&root, &path, &translator) {
                    Ok(sides) => {
                        deliver_command_event(
                            &proxy,
                            job.as_ref(),
                            UserEvent::ConflictReady(ConflictView { path, sides }),
                            Ok(serde_json::Value::Null),
                        );
                    }
                    Err(error) => {
                        deliver_command_event(
                            &proxy,
                            job.as_ref(),
                            UserEvent::ScmOperationCompleted {
                                message: translator
                                    .format("scm.error.open_conflict", &[("error", &error)]),
                                snapshot: None,
                            },
                            Err(CommandError::Failed(error)),
                        );
                    }
                }
            });
        if let Err(error) = spawned {
            self.fail_current_command(CommandError::Failed(error.to_string()));
        }
    }

    pub(crate) fn resolve_conflict_choice(&mut self, side: usize) {
        let Some(conflict) = self.conflict_view.take() else {
            return;
        };
        if side == 3 {
            self.open_file(conflict.path);
            self.status = self.t("scm.conflict.edit_hint").into();
            return;
        }
        let Some(contents) = conflict.sides.get(side).cloned().flatten() else {
            self.status = self.t("scm.conflict.side_unavailable").into();
            self.conflict_view = Some(conflict);
            return;
        };
        let root = self.scm_workspace_root();
        let Some(proxy) = self.event_proxy.clone() else {
            self.fail_current_command(CommandError::Failed(
                self.t("scm.error.no_event_loop").into(),
            ));
            return;
        };
        let translator = self.translator;
        let path = conflict.path;
        let job = self.command_job(false);
        let spawned = std::thread::Builder::new()
            .name("graphite-conflict-resolve".into())
            .spawn(move || {
                if job
                    .as_ref()
                    .is_some_and(|job| job.cancellation.is_cancelled())
                {
                    return;
                }
                let result = crate::scm::resolve_conflict(&root, &path, &contents, &translator);
                let outcome = result
                    .as_ref()
                    .map(|_| serde_json::Value::Null)
                    .map_err(|error| CommandError::Failed(error.clone()));
                let message = result
                    .map(|_| translator.text("scm.conflict.resolved_staged").to_owned())
                    .unwrap_or_else(|error| {
                        translator.format("scm.error.conflict_failed", &[("error", &error)])
                    });
                deliver_command_event(
                    &proxy,
                    job.as_ref(),
                    UserEvent::ScmOperationCompleted {
                        message,
                        snapshot: Some(crate::scm::snapshot(&root)),
                    },
                    outcome,
                );
            });
        if let Err(error) = spawned {
            self.fail_current_command(CommandError::Failed(error.to_string()));
        }
    }

    pub(crate) fn draw_scm_sidebar(
        &self,
        renderer: &mut Renderer,
        height: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        let sidebar_width = self.sidebar_layout_width_px();
        let row_h = line_height.max(18.0);
        if self.shows_scm_welcome_view() {
            self.draw_scm_welcome_view(renderer, sidebar_width, height, row_h, cell_width);
            return;
        }
        renderer.draw_rect(
            ACTIVITY_BAR_WIDTH,
            0.0,
            sidebar_width - ACTIVITY_BAR_WIDTH,
            height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        let left = ACTIVITY_BAR_WIDTH + PAD;
        let right = sidebar_width - PAD;
        let row_h = line_height.max(18.0);
        let layout = self.scm_sidebar_layout_for(height, line_height, cell_width);
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        renderer.draw_text(self.t("scm.sidebar.title"), left, row_h * 0.2, dim);
        let repo_header_y = layout.repository_header.y;
        let repo_chevron = if self.scm.repositories_collapsed {
            crate::file_icons::CHEVRON_RIGHT
        } else {
            crate::file_icons::CHEVRON_DOWN
        };
        let header_icon_size = (row_h * 0.65).round();
        renderer.draw_icon(
            repo_chevron,
            layout.repository_chevron.x
                + (layout.repository_chevron.width - header_icon_size) * 0.5,
            repo_header_y + (row_h - header_icon_size) * 0.5,
            header_icon_size,
            dim,
        );
        renderer.draw_text(
            self.t("scm.sidebar.repositories"),
            left + layout.repository_chevron.width,
            repo_header_y,
            text,
        );
        let repo_root = self.scm_workspace_root();
        let repo_name = repo_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| repo_root.to_str().unwrap_or("Workspace"));
        if !self.scm.repositories_collapsed {
            let icon_size = layout.repository_icon.width;
            renderer.draw_icon(
                crate::file_icons::GIT.glyph,
                layout.repository_icon.x,
                layout.repository_icon.y,
                icon_size,
                rgb(scm_icon_color(crate::file_icons::GIT.color)),
            );
            let available_chars = (layout.repository_label.width / cell_width)
                .floor()
                .max(0.0) as usize;
            renderer.draw_text(
                &scm_fit_label(repo_name, available_chars),
                layout.repository_label.x,
                layout.repository_label.y,
                text,
            );
        }
        if !self.scm.repositories_collapsed {
            for (rect, icon) in [
                (layout.repo_sync, ScmIcon::Sync),
                (layout.repo_pull_request, ScmIcon::PullRequest),
                (layout.repo_more, ScmIcon::More),
            ] {
                let hovered = rect.contains(self.mouse.x as f32, self.mouse.y as f32);
                draw_scm_control_slot(renderer, rect, hovered);
                draw_scm_icon(renderer, icon, rect, scm_button_color(hovered));
            }
        }
        let branch = self
            .scm
            .snapshot
            .branch
            .as_deref()
            .unwrap_or_else(|| self.t("scm.sidebar.no_repository"));
        if !self.scm.repositories_collapsed {
            let icon_size = layout.branch_icon.width;
            renderer.draw_icon(
                crate::file_icons::GIT.glyph,
                layout.branch_icon.x,
                layout.branch_icon.y,
                icon_size,
                rgb(Theme::color(0x9da5b4, 0x5f6672)),
            );
            let branch_chars = (layout.branch_label.width / cell_width).floor().max(0.0) as usize;
            renderer.draw_text(
                &scm_fit_label(branch, branch_chars),
                layout.branch_label.x,
                layout.branch_label.y,
                text,
            );
            let control_size = (row_h * 0.62).round();
            let picker_hovered = layout
                .branch_picker
                .contains(self.mouse.x as f32, self.mouse.y as f32);
            draw_scm_control_slot(renderer, layout.branch_picker, picker_hovered);
            renderer.draw_icon(
                crate::file_icons::CHEVRON_DOWN,
                layout.branch_picker.x + (layout.branch_picker.width - control_size) * 0.5,
                layout.branch_picker.y + (row_h - control_size) * 0.5,
                control_size,
                scm_button_color(picker_hovered),
            );
            let refresh_hovered = layout
                .refresh
                .contains(self.mouse.x as f32, self.mouse.y as f32);
            draw_scm_control_slot(renderer, layout.refresh, refresh_hovered);
            draw_scm_icon(
                renderer,
                ScmIcon::Refresh,
                layout.refresh,
                scm_button_color(refresh_hovered),
            );
        }
        let commit_top = layout.commit_field.y;
        let field_width = right - left;
        let focused = self.scm.commit_prompt;
        let field_background = Theme::color(0x1f1f1f, 0xffffff);
        let field_border = if focused {
            Theme::color(Theme::STATUS, 0x007acc)
        } else {
            Theme::color(0x454545, 0xc8c8c8)
        };
        renderer.draw_rect(left, commit_top, field_width, row_h, rgb(field_background));
        renderer.draw_rect(left, commit_top, field_width, 1.0, rgb(field_border));
        renderer.draw_rect(
            left,
            commit_top + row_h - 1.0,
            field_width,
            1.0,
            rgb(field_border),
        );
        renderer.draw_rect(left, commit_top, 1.0, row_h, rgb(field_border));
        renderer.draw_rect(
            left + field_width - 1.0,
            commit_top,
            1.0,
            row_h,
            rgb(field_border),
        );
        let field_padding = cell_width * 0.55;
        let visible_chars = ((field_width - field_padding * 2.0) / cell_width)
            .floor()
            .max(0.0) as usize;
        let placeholder = match &self.scm.snapshot.branch {
            Some(branch) => self.tf(
                "scm.sidebar.commit_placeholder_branch",
                &[("branch", branch)],
            ),
            None => self.t("scm.sidebar.commit_placeholder").to_owned(),
        };
        let field_text = if self.scm.commit_message.is_empty() {
            scm_fit_label(&placeholder, visible_chars)
        } else {
            scm_visible_tail(&self.scm.commit_message, visible_chars)
        };
        renderer.draw_text(
            &field_text,
            left + field_padding,
            commit_top + (row_h - line_height) * 0.5 + row_h * 0.08,
            if self.scm.commit_message.is_empty() {
                rgb(Theme::color(0xb0b0b0, 0x666666))
            } else {
                text
            },
        );
        if focused {
            let caret_chars = if self.scm.commit_message.is_empty() {
                0
            } else {
                field_text.chars().count()
            };
            let caret_x = left + field_padding + caret_chars as f32 * cell_width;
            renderer.draw_rect(
                caret_x,
                commit_top + row_h * 0.22,
                1.5,
                row_h * 0.56,
                rgb(Theme::color(Theme::STATUS, 0x007acc)),
            );
        }
        let commit_y = layout.commit_button.y;
        let primary = self.scm_primary_action();
        let commit_enabled =
            primary != ScmPrimaryAction::Commit || !self.scm.commit_message.trim().is_empty();
        renderer.draw_rect(
            left,
            commit_y,
            right - left,
            row_h,
            if commit_enabled {
                rgb(Theme::color(Theme::STATUS, 0x0e639c))
            } else {
                rgb(Theme::color(0x3c3c3c, 0xe5e5e5))
            },
        );
        // The icon and the label are centered together as one group.
        if primary == ScmPrimaryAction::Commit {
            let arrow = commit_arrow(layout.commit_button);
            renderer.draw_rect(
                arrow.x,
                arrow.y + row_h * 0.2,
                1.0,
                row_h * 0.6,
                if commit_enabled { rgb(0x7fb4d8) } else { dim },
            );
            let size = (row_h * 0.65).round();
            renderer.draw_icon(
                crate::file_icons::CHEVRON_DOWN,
                arrow.x + (arrow.width - size) * 0.5,
                arrow.y + (arrow.height - size) * 0.5,
                size,
                if commit_enabled { rgb(0xffffff) } else { dim },
            );
        }
        let commit_label = match primary {
            ScmPrimaryAction::Commit => self.t("scm.sidebar.commit").to_owned(),
            ScmPrimaryAction::PublishRepository | ScmPrimaryAction::PublishBranch => {
                self.t("scm.sidebar.publish").to_owned()
            }
            ScmPrimaryAction::Sync { ahead, behind } => {
                let mut label = self.t("scm.sidebar.sync").to_owned();
                if behind > 0 {
                    label.push_str(&format!(" {behind}↓"));
                }
                if ahead > 0 {
                    label.push_str(&format!(" {ahead}↑"));
                }
                label
            }
        };
        let busy = self.scm_busy_state();
        let commit_label = match busy {
            Some((kind, _)) => self.scm_busy_label(kind).to_owned(),
            None => commit_label,
        };
        let commit_color = if commit_enabled || busy.is_some() {
            rgb(0xffffff)
        } else {
            dim
        };
        let check_size = (row_h * 0.72).round().max(12.0);
        let label_gap = cell_width * 0.4;
        let label_width = commit_label.chars().count() as f32 * cell_width;
        let group_x = left + ((field_width - check_size - label_gap - label_width) * 0.5).max(0.0);
        if let Some((_, elapsed)) = busy {
            super::scm_progress::draw_spinner(
                renderer,
                group_x,
                commit_y + (row_h - check_size) * 0.5,
                check_size,
                0xffffff,
                Theme::color(Theme::STATUS, 0x0e639c),
                super::scm_progress::spinner_head(elapsed),
            );
        } else {
            draw_scm_icon(
                renderer,
                match primary {
                    ScmPrimaryAction::Commit => ScmIcon::Check,
                    ScmPrimaryAction::PublishRepository | ScmPrimaryAction::PublishBranch => {
                        ScmIcon::Push
                    }
                    ScmPrimaryAction::Sync { .. } => ScmIcon::Sync,
                },
                ScmRect {
                    x: group_x,
                    y: commit_y,
                    width: check_size,
                    height: row_h,
                },
                commit_color,
            );
        }
        renderer.draw_text(
            &commit_label,
            group_x + check_size + label_gap,
            commit_y + (row_h - line_height) * 0.5 + row_h * 0.08,
            commit_color,
        );
        let list_top = layout.list_top;
        if let Some(error) = &self.scm.snapshot.error {
            renderer.draw_text(
                error,
                left,
                list_top,
                rgb(Theme::color(Theme::TEXT, 0x9b2226)),
            );
        }
        for row in &layout.list_rows {
            let y = row.bounds.y;
            match row.kind {
                ScmListRowKind::Extension(extension) => self.draw_extension_scm_row(
                    renderer,
                    row,
                    extension,
                    [row_h, line_height, cell_width, sidebar_width],
                ),
                ScmListRowKind::GraphHeader { collapsed, .. } => {
                    self.draw_scm_graph_header(
                        renderer, row, collapsed, row_h, cell_width, text, dim,
                    );
                }
                ScmListRowKind::GraphCommit { index } => {
                    self.draw_scm_graph_commit_row(
                        renderer,
                        row,
                        index,
                        row_h,
                        cell_width,
                        sidebar_width,
                        text,
                        dim,
                    );
                }
                ScmListRowKind::GraphCommitFile {
                    commit_index,
                    file_index,
                } => {
                    self.draw_scm_graph_commit_file_row(
                        renderer,
                        row,
                        commit_index,
                        file_index,
                        row_h,
                        cell_width,
                        sidebar_width,
                        text,
                        dim,
                    );
                }
                ScmListRowKind::Section {
                    staged,
                    count,
                    collapsed,
                } => {
                    let label = if staged {
                        self.t("scm.sidebar.staged")
                    } else {
                        self.t("scm.sidebar.changes")
                    };
                    let chevron = if collapsed {
                        crate::file_icons::CHEVRON_RIGHT
                    } else {
                        crate::file_icons::CHEVRON_DOWN
                    };
                    let icon_size = (row_h * 0.65).round();
                    renderer.draw_icon(
                        chevron,
                        row.chevron.x + (row.chevron.width - icon_size) * 0.5,
                        y + (row.chevron.height - icon_size) * 0.5,
                        icon_size,
                        dim,
                    );
                    renderer.draw_text(label, row.chevron.x + row.chevron.width, y, text);
                    renderer.draw_text(&count.to_string(), right - cell_width, y, accent);
                    let action_icon = if staged {
                        ScmIcon::Minus
                    } else {
                        ScmIcon::Plus
                    };
                    let action_hovered = row
                        .action
                        .contains(self.mouse.x as f32, self.mouse.y as f32);
                    draw_scm_control_slot(renderer, row.action, action_hovered);
                    draw_scm_icon(
                        renderer,
                        action_icon,
                        row.action,
                        scm_button_color(action_hovered),
                    );
                    if !staged {
                        let (history, stash) = changes_header_buttons(row.action, cell_width);
                        for (rect, icon) in [(history, ScmIcon::History), (stash, ScmIcon::Stash)] {
                            let hovered = rect.contains(self.mouse.x as f32, self.mouse.y as f32);
                            draw_scm_control_slot(renderer, rect, hovered);
                            draw_scm_icon(renderer, icon, rect, scm_button_color(hovered));
                        }
                    }
                }
                ScmListRowKind::Folder {
                    staged,
                    first,
                    depth,
                    count,
                    collapsed,
                } => {
                    let name = self
                        .scm
                        .snapshot
                        .files
                        .get(first)
                        .map(|file| folder_path(&file.path, depth))
                        .and_then(|path| {
                            path.file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                        })
                        .unwrap_or_default();
                    let indent = depth as f32 * cell_width * 1.4;
                    let chevron = if collapsed {
                        crate::file_icons::CHEVRON_RIGHT
                    } else {
                        crate::file_icons::CHEVRON_DOWN
                    };
                    let icon_size = (row_h * 0.65).round();
                    renderer.draw_icon(
                        chevron,
                        row.chevron.x + indent + (row.chevron.width - icon_size) * 0.5,
                        y + (row.chevron.height - icon_size) * 0.5,
                        icon_size,
                        dim,
                    );
                    let name_x = row.chevron.x + indent + row.chevron.width;
                    let max = ((right - name_x - cell_width * 6.0) / cell_width).max(1.0) as usize;
                    renderer.draw_text(
                        &name.chars().take(max).collect::<String>(),
                        name_x,
                        y,
                        text,
                    );
                    renderer.draw_text(&count.to_string(), right - cell_width, y, accent);
                    let action_hovered = row
                        .action
                        .contains(self.mouse.x as f32, self.mouse.y as f32);
                    draw_scm_control_slot(renderer, row.action, action_hovered);
                    draw_scm_icon(
                        renderer,
                        if staged {
                            ScmIcon::Minus
                        } else {
                            ScmIcon::Plus
                        },
                        row.action,
                        scm_button_color(action_hovered),
                    );
                }
                ScmListRowKind::File { index, staged } => {
                    let item = &self.scm.snapshot.files[index];
                    if index == self.scm.selected {
                        renderer.draw_rect(
                            ACTIVITY_BAR_WIDTH,
                            y - row_h * 0.1,
                            sidebar_width - ACTIVITY_BAR_WIDTH,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    let icon = crate::icon_theme::for_path(&item.path);
                    let icon_size = (row_h * 0.72).round();
                    // In the tree the folders above a file already say where it is.
                    let tree_depth = if self.scm.view_as_tree {
                        item.path.components().count().saturating_sub(1)
                    } else {
                        0
                    };
                    let icon_x = left + cell_width + tree_depth as f32 * cell_width * 1.4;
                    crate::icon_theme::draw(
                        renderer,
                        &icon,
                        icon_x,
                        y + (row_h - icon_size) * 0.5,
                        icon_size,
                        scm_icon_color,
                    );
                    let full = item.path.to_string_lossy();
                    let name = item
                        .path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&full);
                    let dir = item
                        .path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty() && !self.scm.view_as_tree)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let kind = item.kind(staged);
                    let status = kind.letter();
                    let status_color = kind.color();
                    let discardable = !staged && !item.conflicted;
                    let reserved = cell_width
                        * (status.chars().count() as f32
                            + 3.0
                            + if discardable { 2.5 } else { 0.0 });
                    let file_x = scm_file_label_x(icon_x, icon_size, cell_width * 0.65);
                    let max = ((right - file_x - reserved) / cell_width).max(1.0) as usize;
                    let name = name.chars().take(max).collect::<String>();
                    renderer.draw_text(&name, file_x, y, text);
                    let dir_x =
                        file_x + name.chars().count() as f32 * cell_width + cell_width * 0.4;
                    let dir_max = ((right - dir_x - reserved) / cell_width).max(0.0) as usize;
                    renderer.draw_text(
                        &dir.chars().take(dir_max).collect::<String>(),
                        dir_x,
                        y,
                        dim,
                    );
                    renderer.draw_text(status, right - cell_width, y, rgb(status_color));
                    let action_icon = if staged {
                        ScmIcon::Minus
                    } else {
                        ScmIcon::Plus
                    };
                    let action_hovered = row
                        .action
                        .contains(self.mouse.x as f32, self.mouse.y as f32);
                    draw_scm_control_slot(renderer, row.action, action_hovered);
                    draw_scm_icon(
                        renderer,
                        action_icon,
                        row.action,
                        scm_button_color(action_hovered),
                    );
                    if discardable {
                        let discard = discard_button(row.action, cell_width);
                        let hovered = discard.contains(self.mouse.x as f32, self.mouse.y as f32);
                        draw_scm_control_slot(renderer, discard, hovered);
                        draw_scm_icon(
                            renderer,
                            ScmIcon::Discard,
                            discard,
                            scm_button_color(hovered),
                        );
                    }
                }
            }
        }
        if let Some(divider) = layout.graph_divider {
            let line_y = divider.y + divider.height * 0.5;
            renderer.draw_rect(
                divider.x,
                line_y - 0.5,
                divider.width,
                1.0,
                rgb(Theme::color(0x3c3c3c, 0xd0d0d0)),
            );
            let hovered = !self.scm.graph_resize_dragging
                && !self.scm.graph_collapsed
                && self.is_over_scm_graph_resize_handle(self.mouse.x as f32, self.mouse.y as f32);
            if hovered || self.scm.graph_resize_dragging {
                renderer.draw_rect(divider.x, line_y - 1.5, divider.width, 3.0, accent);
            }
        }
    }

    pub(crate) fn draw_phase4_overlays(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
    ) {
        let sidebar_width = self.sidebar_width_px();
        if self.outline_visible
            && self.view_preferences.secondary_sidebar_visible
            && !self.zen_mode
            && !(self.chat.visible && self.chat.surface == super::chat::ChatSurface::SideBar)
            && self.secondary_plugin.is_none()
        {
            let outline_width = self.secondary_sidebar_width();
            let x = self.secondary_sidebar_x(width);
            let top = self.secondary_sidebar_top(line_height);
            renderer.draw_rect(
                x,
                top,
                outline_width,
                (height - top - line_height).max(0.0),
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            renderer.draw_text(
                self.t("scm.outline.title"),
                x + PAD,
                top + PAD,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            let char_width = renderer.char_width();
            if self.outline.filter_on_type {
                renderer.draw_text(
                    &format!("\u{2315} {}\u{258f}", self.outline.filter),
                    x + PAD,
                    top + line_height * 1.5,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
            let first_row = if self.outline.filter_on_type {
                2.5
            } else {
                1.5
            };
            let chars = ((outline_width - PAD * 2.0) / char_width.max(1.0)).max(0.0) as usize;
            for (i, row) in self.outline_rows().iter().enumerate() {
                let y = top + line_height * (i as f32 + first_row);
                if y + line_height > height - line_height {
                    break;
                }
                if row.current {
                    renderer.draw_rect(
                        x,
                        y,
                        outline_width,
                        line_height,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let chevron = if !row.expandable {
                    "  "
                } else if row.collapsed {
                    "\u{25b8} "
                } else {
                    "\u{25be} "
                };
                let label = format!(
                    "{}{}{}  {}",
                    "  ".repeat(row.depth),
                    chevron,
                    row.line + 1,
                    row.text
                );
                renderer.draw_text(
                    &label.chars().take(chars).collect::<String>(),
                    x + PAD,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if !self.chat_review_open() {
            self.draw_diff_view(renderer, width, height, line_height);
        }
        if self.process_panel_visible {
            renderer.draw_rect(
                sidebar_width,
                line_height,
                width - sidebar_width,
                height - line_height * 2.0,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            renderer.draw_text(
                self.t("scm.process.title"),
                sidebar_width + PAD,
                line_height * 1.2,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let process_range = crate::process_explorer::visible_process_range(
                self.process_metrics.len(),
                self.process_scroll,
                9,
            );
            for (index, process) in self
                .process_metrics
                .get(process_range)
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                let row = format!(
                    "{}  pid {}   {:>5.1}%   {}",
                    process.name,
                    process.pid,
                    process.cpu_percent,
                    crate::process_explorer::format_memory(process.memory_bytes)
                );
                renderer.draw_text(
                    &row,
                    sidebar_width + PAD,
                    line_height * (index as f32 + 2.2),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
            let start = self.process_metrics.len().min(9) as f32 + 2.4;
            renderer.draw_text(
                self.t("scm.process.startup"),
                sidebar_width + PAD,
                line_height * start,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            for (index, metric) in self.startup_metrics.iter().enumerate() {
                renderer.draw_text(
                    metric,
                    sidebar_width + PAD,
                    line_height * (start + index as f32 + 1.0),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if self.branch_picker {
            let box_width = 420.0f32.min(width - PAD * 2.0);
            let entries = self.branch_picker_entries();
            let box_height = (entries.len().min(12) as f32 + 1.5) * line_height;
            let x = (width - box_width) / 2.0;
            let y = (height - box_height) / 2.0;
            renderer.draw_rect(
                x,
                y,
                box_width,
                box_height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            renderer.draw_text(
                self.t(self.branch_picker_action.prompt_key()),
                x + PAD,
                y + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            // Show the 12 entries around the selection, so a long list can be scrolled with the
            // arrow keys.
            let first = self
                .branch_selected
                .saturating_sub(11)
                .min(entries.len().saturating_sub(12));
            for (index, branch) in entries.iter().enumerate().skip(first).take(12) {
                let row_y = y + line_height * ((index - first) as f32 + 1.2);
                if index == self.branch_selected {
                    renderer.draw_rect(
                        x,
                        row_y,
                        box_width,
                        line_height,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                renderer.draw_text(
                    branch,
                    x + PAD,
                    row_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(conflict) = &self.conflict_view {
            renderer.draw_rect(
                0.0,
                0.0,
                width,
                height,
                rgb(Theme::color(Theme::BG, 0xffffff)),
            );
            renderer.draw_text(
                self.t("scm.conflict.shortcuts"),
                PAD,
                PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let pane_width = width / 3.0;
            for (index, (label, side)) in [
                (self.t("scm.conflict.base"), conflict.sides[0].as_deref()),
                (self.t("scm.conflict.ours"), conflict.sides[1].as_deref()),
                (self.t("scm.conflict.theirs"), conflict.sides[2].as_deref()),
            ]
            .into_iter()
            .enumerate()
            {
                let x = index as f32 * pane_width;
                renderer.draw_text(
                    &format!("{label} — {}", conflict.path.display()),
                    x + PAD,
                    line_height * 1.5,
                    rgb(Theme::color(Theme::DIM, 0x616161)),
                );
                if let Some(text) = side {
                    for (row, line) in text
                        .lines()
                        .take(((height - line_height * 2.0) / line_height) as usize)
                        .enumerate()
                    {
                        renderer.draw_text(
                            line,
                            x + PAD,
                            line_height * (row as f32 + 2.5),
                            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                        );
                    }
                } else {
                    renderer.draw_text(
                        self.t("scm.conflict.no_base"),
                        x + PAD,
                        line_height * 2.5,
                        rgb(Theme::color(Theme::DIM, 0x616161)),
                    );
                }
            }
        }
        if self.screencast_mode {
            let (x, y) = (self.mouse.x as f32, self.mouse.y as f32);
            let phase = (self
                .screencast_shortcut
                .as_ref()
                .map(|(_, at)| at.elapsed().as_millis() as usize / 100 % 4)
                .unwrap_or(0)) as f32;
            let radius = 12.0 + phase * 2.0;
            let color = rgb(Theme::color(Theme::CURSOR, 0x007acc));
            renderer.draw_rect(x - radius, y - 1.0, radius * 2.0, 2.0, color);
            renderer.draw_rect(x - 1.0, y - radius, 2.0, radius * 2.0, color);
            if let Some((shortcut, at)) = &self.screencast_shortcut {
                if at.elapsed().as_secs_f32() < 1.5 {
                    let label_width = shortcut.chars().count() as f32 * 9.0 + PAD * 2.0;
                    renderer.draw_rect(
                        x + 18.0,
                        y + 18.0,
                        label_width,
                        line_height,
                        rgb(Theme::color(Theme::STATUS, 0x005a9e)),
                    );
                    renderer.draw_text(shortcut, x + 18.0 + PAD, y + 18.0, rgb(0xffffff));
                }
            }
        }
    }
}

/// The state of the main button of the source control view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScmPrimaryAction {
    Commit,
    /// There is no remote yet: create one and publish.
    PublishRepository,
    /// A remote exists but the branch does not track anything yet.
    PublishBranch,
    /// Nothing to commit and the branch differs from its upstream.
    Sync {
        ahead: usize,
        behind: usize,
    },
}

pub(crate) fn scm_primary_action_for(snapshot: &crate::scm::Snapshot) -> ScmPrimaryAction {
    if snapshot.error.is_some() || !snapshot.has_commits || !snapshot.files.is_empty() {
        return ScmPrimaryAction::Commit;
    }
    if !snapshot.has_remote {
        return ScmPrimaryAction::PublishRepository;
    }
    if snapshot.branch.is_none() {
        return ScmPrimaryAction::Commit;
    }
    if snapshot.upstream.is_none() {
        return ScmPrimaryAction::PublishBranch;
    }
    if snapshot.ahead > 0 || snapshot.behind > 0 {
        return ScmPrimaryAction::Sync {
            ahead: snapshot.ahead,
            behind: snapshot.behind,
        };
    }
    ScmPrimaryAction::Commit
}

#[derive(Clone, Copy)]
pub(crate) enum ScmAction {
    Stage,
    Unstage,
    StageAll,
    UnstageAll,
    /// Stage or unstage the files in `App::scm_batch_paths` (a folder of the tree view).
    StageFolder,
    UnstageFolder,
    Commit(crate::scm_commit::CommitOptions),
    StageCursorLine,
    RevertCursorLine,
    StageHunk(usize),
    RevertFirstHunk,
}
