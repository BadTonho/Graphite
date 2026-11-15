use super::command_execution::{deliver_command_event, CommandError};
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
    /// A row of a source control an extension registered.
    Extension(crate::app::plugins::vsix::ExtensionRow),
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
    pub list_rows: Vec<ScmListRow>,
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
        if staged_count > 0 {
            entries.push(ScmListRowKind::Section {
                staged: true,
                count: staged_count,
                collapsed: staged_collapsed,
            });
            if !staged_collapsed {
                entries.extend(
                    snapshot
                        .files
                        .iter()
                        .enumerate()
                        .filter(|(_, file)| file.staged)
                        .map(|(index, _)| ScmListRowKind::File {
                            index,
                            staged: true,
                        }),
                );
            }
        }
        entries.push(ScmListRowKind::Section {
            staged: false,
            count: changes_count,
            collapsed: changes_collapsed,
        });
        if !changes_collapsed {
            entries.extend(
                snapshot
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, file)| file.unstaged || file.untracked || file.conflicted)
                    .map(|(index, _)| ScmListRowKind::File {
                        index,
                        staged: false,
                    }),
            );
        }
        entries.extend(
            extension_rows
                .iter()
                .map(|row| ScmListRowKind::Extension(*row)),
        );
    }
    let list_top = commit_y + row_height * 2.50;
    let available = ((height - list_top - row_height * 1.2) / row_height).max(0.0) as usize;
    let list_rows = entries
        .into_iter()
        .skip(scroll)
        .take(available)
        .enumerate()
        .map(|(visible, kind)| {
            let y = list_top + row_height * (visible as f32 + 1.0);
            ScmListRow {
                bounds: full_row(y),
                chevron: ScmRect {
                    x: left,
                    y,
                    width: chevron_slot,
                    height: row_height,
                },
                action: action_rect(y),
                kind,
            }
        })
        .collect();

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

pub(super) fn draw_scm_control_slot(renderer: &mut Renderer, bounds: ScmRect, hovered: bool) {
    let inset = 2.0;
    let background = if hovered {
        Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
    } else {
        Theme::color(0x303033, 0xeeeeee)
    };
    let border = Theme::color(0x4d4d50, 0xc7c7c7);
    let x = bounds.x + inset;
    let y = bounds.y + inset;
    let width = (bounds.width - inset * 2.0).max(1.0);
    let height = (bounds.height - inset * 2.0).max(1.0);
    renderer.draw_rect(x, y, width, height, rgb(background));
    renderer.draw_rect(x, y, width, 1.0, rgb(border));
    renderer.draw_rect(x, y + height - 1.0, width, 1.0, rgb(border));
    renderer.draw_rect(x, y, 1.0, height, rgb(border));
    renderer.draw_rect(x + width - 1.0, y, 1.0, height, rgb(border));
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
        if let Some(proxy) = self.event_proxy.clone() {
            std::thread::Builder::new()
                .name("graphite-scm-status".into())
                .spawn(move || {
                    let _ = proxy.send_event(UserEvent::ScmSnapshot(crate::scm::snapshot(&root)));
                })
                .ok();
        } else {
            self.scm_snapshot = crate::scm::snapshot(&root);
        }
    }

    pub(crate) fn accept_scm_snapshot(&mut self, snapshot: crate::scm::Snapshot) {
        self.scm_snapshot = snapshot;
        self.scm_selected = self
            .scm_selected
            .min(self.scm_snapshot.files.len().saturating_sub(1));
    }

    pub(crate) fn toggle_scm(&mut self) {
        self.scm_visible = !self.scm_visible;
        if self.scm_visible {
            self.close_plugins();
            self.refresh_scm_async();
        }
    }

    pub(crate) fn scm_selected_path(&self) -> Option<PathBuf> {
        self.scm_snapshot
            .files
            .get(self.scm_selected)
            .map(|item| self.scm_workspace_root().join(&item.path))
    }

    pub(crate) fn open_branch_picker(&mut self) {
        self.branch_picker = true;
        self.branch_selected = self
            .scm_snapshot
            .branches
            .iter()
            .position(|branch| Some(branch) == self.scm_snapshot.branch.as_ref())
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
            self.fail_current_command(CommandError::Failed(error.to_string()));
        }
    }

    pub(crate) fn perform_scm_action(&mut self, action: ScmAction) {
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
            ScmAction::StageHunk(_) if !self.scm_visible => active_path,
            _ if self.scm_visible => self.scm_selected_path(),
            _ => active_path,
        };
        let batch_paths = match action {
            ScmAction::StageAll => self
                .scm_snapshot
                .files
                .iter()
                .filter(|file| file.unstaged || file.untracked)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            ScmAction::UnstageAll => self
                .scm_snapshot
                .files
                .iter()
                .filter(|file| file.staged)
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let message = self.scm_commit_message.clone();
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
                    ScmAction::Commit => {
                        crate::scm::commit(&root, &message, &translator).map(|id| {
                            translator.format(
                                "scm.status.commit_created",
                                &[("id", &&id[..id.len().min(8)])],
                            )
                        })
                    }
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
            .scm_snapshot
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
            .scm_snapshot
            .files
            .get(self.scm_selected)
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
        let layout = scm_panel_layout(
            &self.scm_snapshot,
            sidebar_width,
            height,
            row_h,
            cell_width,
            self.scm_scroll,
            self.scm_repositories_collapsed,
            self.scm_staged_collapsed,
            self.scm_changes_collapsed,
            &self.vsix_scm_rows(),
        );
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        renderer.draw_text(self.t("scm.sidebar.title"), left, row_h * 0.2, dim);
        let repo_header_y = layout.repository_header.y;
        let repo_chevron = if self.scm_repositories_collapsed {
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
        if !self.scm_repositories_collapsed {
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
        if !self.scm_repositories_collapsed {
            for (rect, glyph) in [
                (layout.repo_sync, "⇅"),
                (layout.repo_pull_request, "⇄"),
                (layout.repo_more, "⋯"),
            ] {
                draw_scm_control_slot(
                    renderer,
                    rect,
                    rect.contains(self.mouse.x as f32, self.mouse.y as f32),
                );
                renderer.draw_text(
                    glyph,
                    scm_centered_text_x(rect.x, rect.width, glyph, cell_width),
                    rect.y + (row_h - line_height) * 0.5 + row_h * 0.08,
                    dim,
                );
            }
        }
        let branch = self
            .scm_snapshot
            .branch
            .as_deref()
            .unwrap_or_else(|| self.t("scm.sidebar.no_repository"));
        if !self.scm_repositories_collapsed {
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
            draw_scm_control_slot(
                renderer,
                layout.branch_picker,
                layout
                    .branch_picker
                    .contains(self.mouse.x as f32, self.mouse.y as f32),
            );
            renderer.draw_icon(
                crate::file_icons::CHEVRON_DOWN,
                layout.branch_picker.x + (layout.branch_picker.width - control_size) * 0.5,
                layout.branch_picker.y + (row_h - control_size) * 0.5,
                control_size,
                dim,
            );
            let refresh = "⟳";
            draw_scm_control_slot(
                renderer,
                layout.refresh,
                layout
                    .refresh
                    .contains(self.mouse.x as f32, self.mouse.y as f32),
            );
            renderer.draw_text(
                refresh,
                scm_centered_text_x(layout.refresh.x, layout.refresh.width, refresh, cell_width),
                layout.refresh.y + (row_h - line_height) * 0.5 + row_h * 0.08,
                dim,
            );
        }
        let commit_top = layout.commit_field.y;
        let field_width = right - left;
        let focused = self.scm_commit_prompt;
        let field_background = Theme::color(0x1f1f1f, 0xffffff);
        let field_border = if focused {
            Theme::color(Theme::STATUS, 0x007acc)
        } else {
            Theme::color(0x606060, 0x8a8a8a)
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
        let placeholder = self.t("scm.sidebar.commit_placeholder");
        let field_text = if self.scm_commit_message.is_empty() {
            placeholder.chars().take(visible_chars).collect::<String>()
        } else {
            scm_visible_tail(&self.scm_commit_message, visible_chars)
        };
        renderer.draw_text(
            &field_text,
            left + field_padding,
            commit_top + (row_h - line_height) * 0.5 + row_h * 0.08,
            if self.scm_commit_message.is_empty() {
                rgb(Theme::color(0xb0b0b0, 0x666666))
            } else {
                text
            },
        );
        if focused {
            let caret_chars = if self.scm_commit_message.is_empty() {
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
        let commit_enabled = !self.scm_commit_message.trim().is_empty();
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
        let commit_label = self.t("scm.sidebar.commit");
        renderer.draw_text(
            commit_label,
            scm_centered_text_x(left, field_width, commit_label, cell_width),
            commit_y + (row_h - line_height) * 0.5 + row_h * 0.08,
            if commit_enabled { rgb(0xffffff) } else { dim },
        );
        let list_top = layout.list_top;
        if let Some(error) = &self.scm_snapshot.error {
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
                    let action = if staged { "−" } else { "+" };
                    draw_scm_control_slot(
                        renderer,
                        row.action,
                        row.action
                            .contains(self.mouse.x as f32, self.mouse.y as f32),
                    );
                    renderer.draw_text(
                        action,
                        scm_centered_text_x(row.action.x, row.action.width, action, cell_width),
                        y + row_h * 0.12,
                        dim,
                    );
                    if !staged {
                        let (history, stash) = changes_header_buttons(row.action, cell_width);
                        for (rect, glyph) in [(history, "↺"), (stash, "▤")] {
                            draw_scm_control_slot(
                                renderer,
                                rect,
                                rect.contains(self.mouse.x as f32, self.mouse.y as f32),
                            );
                            renderer.draw_text(
                                glyph,
                                scm_centered_text_x(rect.x, rect.width, glyph, cell_width),
                                y + row_h * 0.12,
                                dim,
                            );
                        }
                    }
                }
                ScmListRowKind::File { index, staged } => {
                    let item = &self.scm_snapshot.files[index];
                    if index == self.scm_selected {
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
                    let icon_x = left + cell_width;
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
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let status = if item.conflicted {
                        "!"
                    } else if item.untracked {
                        "U"
                    } else if staged {
                        "M"
                    } else {
                        "M"
                    };
                    let status_color = if status == "U" {
                        0xff73c991
                    } else if status == "!" {
                        0xfff14c4c
                    } else {
                        0xffcca700
                    };
                    let reserved = cell_width * (status.chars().count() as f32 + 3.0);
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
                    let action = if staged { "−" } else { "+" };
                    draw_scm_control_slot(
                        renderer,
                        row.action,
                        row.action
                            .contains(self.mouse.x as f32, self.mouse.y as f32),
                    );
                    renderer.draw_text(
                        action,
                        scm_centered_text_x(row.action.x, row.action.width, action, cell_width),
                        y + row_h * 0.12,
                        dim,
                    );
                }
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
        {
            let outline_width = 240.0;
            let x = width - outline_width;
            renderer.draw_rect(
                x,
                0.0,
                outline_width,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            renderer.draw_text(
                self.t("scm.outline.title"),
                x + PAD,
                PAD,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            let char_width = renderer.char_width();
            if self.outline.filter_on_type {
                renderer.draw_text(
                    &format!("\u{2315} {}\u{258f}", self.outline.filter),
                    x + PAD,
                    line_height * 1.5,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
            let first_row = if self.outline.filter_on_type {
                2.5
            } else {
                1.5
            };
            let chars = ((outline_width - PAD * 2.0) / char_width.max(1.0)) as usize;
            for (i, row) in self.outline_rows().iter().enumerate() {
                let y = line_height * (i as f32 + first_row);
                if y + line_height > height {
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
        if let Some(diff) = &self.diff_view {
            renderer.draw_rect(
                0.0,
                0.0,
                width,
                height,
                rgb(Theme::color(Theme::BG, 0xffffff)),
            );
            renderer.draw_text(
                &self.tf("scm.diff.escape_hint", &[("title", &diff.title)]),
                PAD,
                PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let content_top = line_height * 1.5;
            let pane_width = width / 2.0;
            renderer.draw_rect(
                0.0,
                content_top,
                pane_width,
                height - content_top,
                rgb(Theme::color(Theme::BG, 0xffffff)),
            );
            renderer.draw_rect(
                pane_width,
                content_top,
                pane_width,
                height - content_top,
                rgb(Theme::color(Theme::BG, 0xffffff)),
            );
            renderer.draw_text(
                &diff.left_name,
                PAD,
                content_top,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            renderer.draw_text(
                &diff.right_name,
                pane_width + PAD,
                content_top,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            if diff.inline {
                for (index, line) in crate::scm::diff_text(&diff.left, &diff.right)
                    .lines()
                    .take(((height - content_top) / line_height) as usize)
                    .enumerate()
                {
                    let color = if line.starts_with('+') {
                        0x1a7f37
                    } else if line.starts_with('-') {
                        0xcf222e
                    } else {
                        0x1e1e1e
                    };
                    renderer.draw_text(
                        line,
                        PAD,
                        content_top + line_height * (index as f32 + 1.2),
                        rgb(color),
                    );
                }
            } else {
                for (index, line) in diff
                    .left
                    .lines()
                    .take(((height - content_top) / line_height) as usize)
                    .enumerate()
                {
                    renderer.draw_text(
                        line,
                        PAD,
                        content_top + line_height * (index as f32 + 1.2),
                        rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                    );
                }
                for (index, line) in diff
                    .right
                    .lines()
                    .take(((height - content_top) / line_height) as usize)
                    .enumerate()
                {
                    renderer.draw_text(
                        line,
                        pane_width + PAD,
                        content_top + line_height * (index as f32 + 1.2),
                        rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                    );
                }
            }
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

#[derive(Clone, Copy)]
pub(crate) enum ScmAction {
    Stage,
    Unstage,
    StageAll,
    UnstageAll,
    Commit,
    StageCursorLine,
    RevertCursorLine,
    StageHunk(usize),
    RevertFirstHunk,
}
