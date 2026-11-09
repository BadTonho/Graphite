use super::command_execution::{deliver_command_event, CommandError};
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct DiffView {
    pub title: String,
    pub left_name: String,
    pub right_name: String,
    pub left: String,
    pub right: String,
    pub inline: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ConflictView {
    pub path: PathBuf,
    pub sides: [Option<String>; 3],
}

impl App {
    pub(crate) fn refresh_scm_async(&mut self) {
        let root = self.tree.root().to_path_buf();
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
            self.refresh_scm_async();
        }
    }

    pub(crate) fn scm_selected_path(&self) -> Option<PathBuf> {
        self.scm_snapshot
            .files
            .get(self.scm_selected)
            .map(|item| self.tree.root().join(&item.path))
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
            .scm_snapshot
            .branches
            .get(self.branch_selected)
            .cloned()
        else {
            return;
        };
        self.branch_picker = false;
        let root = self.tree.root().to_path_buf();
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
        let root = self.tree.root().to_path_buf();
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
        let root = self.tree.root().to_path_buf();
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
        let root = self.tree.root().to_path_buf();
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
        let Some(path) = self.scm_selected_path() else {
            return;
        };
        let root = self.tree.root().to_path_buf();
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
        let root = self.tree.root().to_path_buf();
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
        renderer.draw_rect(
            ACTIVITY_BAR_WIDTH,
            0.0,
            SIDEBAR_WIDTH - ACTIVITY_BAR_WIDTH,
            height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        renderer.draw_text(
            self.t("scm.sidebar.title"),
            ACTIVITY_BAR_WIDTH + PAD,
            line_height * 0.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        let branch = self
            .scm_snapshot
            .branch
            .as_deref()
            .unwrap_or_else(|| self.t("scm.sidebar.no_repository"));
        renderer.draw_text(
            &self.tf("scm.sidebar.branch", &[("branch", &branch)]),
            ACTIVITY_BAR_WIDTH + PAD,
            line_height * 1.25,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
        renderer.draw_text(
            self.t("scm.sidebar.shortcuts"),
            ACTIVITY_BAR_WIDTH + PAD,
            line_height * 2.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        if let Some(error) = &self.scm_snapshot.error {
            renderer.draw_text(
                error,
                ACTIVITY_BAR_WIDTH + PAD,
                line_height * 3.25,
                rgb(Theme::color(Theme::TEXT, 0x9b2226)),
            );
            return;
        }
        for (index, item) in self.scm_snapshot.files.iter().enumerate() {
            let y = line_height * 3.25 + index as f32 * line_height;
            if y + line_height > height - line_height {
                break;
            }
            if index == self.scm_selected {
                renderer.draw_rect(
                    ACTIVITY_BAR_WIDTH,
                    y,
                    SIDEBAR_WIDTH - ACTIVITY_BAR_WIDTH,
                    line_height,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            let marker = if item.conflicted {
                "!"
            } else if item.untracked {
                "U"
            } else if item.staged {
                "S"
            } else {
                "M"
            };
            let name = item.path.to_string_lossy();
            let max_chars = ((SIDEBAR_WIDTH - cell_width * 5.0) / cell_width).max(1.0) as usize;
            let label = name.chars().take(max_chars).collect::<String>();
            renderer.draw_text(
                &format!("{marker} {label}"),
                ACTIVITY_BAR_WIDTH + PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }
        if self.scm_commit_prompt {
            renderer.draw_rect(
                ACTIVITY_BAR_WIDTH,
                height - line_height * 2.0,
                SIDEBAR_WIDTH - ACTIVITY_BAR_WIDTH,
                line_height * 2.0,
                rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
            );
            renderer.draw_text(
                &self.tf(
                    "scm.sidebar.commit_prompt",
                    &[("message", &self.scm_commit_message)],
                ),
                ACTIVITY_BAR_WIDTH + PAD,
                height - line_height * 1.25,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }
    }

    pub(crate) fn draw_phase4_overlays(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
    ) {
        if self.outline_visible && !self.zen_mode {
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
            if let Some(language) = self.documents[self.active_document()].language {
                let text = self.documents[self.active_document()].buffer.text();
                let headers = graphite_syntax::declaration_headers(language, &text);
                for (i, header) in headers.iter().take(16).enumerate() {
                    renderer.draw_text(
                        &format!("{}  {}", header.line + 1, header.text),
                        x + PAD,
                        line_height * (i as f32 + 1.5),
                        rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                    );
                }
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
                SIDEBAR_WIDTH,
                line_height,
                width - SIDEBAR_WIDTH,
                height - line_height * 2.0,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            renderer.draw_text(
                self.t("scm.process.title"),
                SIDEBAR_WIDTH + PAD,
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
                    SIDEBAR_WIDTH + PAD,
                    line_height * (index as f32 + 2.2),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
            let start = self.process_metrics.len().min(9) as f32 + 2.4;
            renderer.draw_text(
                self.t("scm.process.startup"),
                SIDEBAR_WIDTH + PAD,
                line_height * start,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            for (index, metric) in self.startup_metrics.iter().enumerate() {
                renderer.draw_text(
                    metric,
                    SIDEBAR_WIDTH + PAD,
                    line_height * (start + index as f32 + 1.0),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if self.branch_picker {
            let box_width = 420.0f32.min(width - PAD * 2.0);
            let box_height = (self.scm_snapshot.branches.len().min(12) as f32 + 1.5) * line_height;
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
                self.t("scm.branch.prompt"),
                x + PAD,
                y + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            for (index, branch) in self.scm_snapshot.branches.iter().take(12).enumerate() {
                let row_y = y + line_height * (index as f32 + 1.2);
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
    Commit,
    StageCursorLine,
    RevertCursorLine,
    StageHunk(usize),
    RevertFirstHunk,
}
