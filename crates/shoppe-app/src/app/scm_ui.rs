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
        self.status = "Select a local branch; checkout requires a clean worktree".into();
    }

    pub(crate) fn checkout_selected_branch(&mut self) {
        if self.documents.iter().any(|tab| tab.is_dirty()) {
            self.status = "Checkout refused because an open document has unsaved changes".into();
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
            return;
        };
        std::thread::Builder::new()
            .name("graphite-git-checkout".into())
            .spawn(move || {
                let result = crate::scm::checkout_branch(&root, &branch);
                let documents = if result.is_ok() {
                    open_paths
                        .into_iter()
                        .map(|path| {
                            let decoded = crate::file_format::read(&path);
                            (path, decoded)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                let message = result
                    .map(|_| format!("Checked out {branch}"))
                    .unwrap_or_else(|error| format!("Checkout failed: {error}"));
                let _ = proxy.send_event(UserEvent::BranchCheckoutCompleted {
                    message,
                    snapshot: crate::scm::snapshot(&root),
                    documents,
                });
            })
            .ok();
    }

    pub(crate) fn perform_scm_action(&mut self, action: ScmAction) {
        let root = self.tree.root().to_path_buf();
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = "SCM actions require the native event loop".into();
            return;
        };
        let active_path = self.documents[self.active_document()]
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
        self.status = "Git operation running…".into();
        std::thread::Builder::new()
            .name("graphite-scm-operation".into())
            .spawn(move || {
                let result = match action {
                    ScmAction::Stage => selected
                        .ok_or_else(|| "Select a changed file first".to_owned())
                        .and_then(|path| {
                            crate::scm::stage_file(&root, &path).map(|_| "File staged".into())
                        }),
                    ScmAction::Unstage => selected
                        .ok_or_else(|| "Select a staged file first".to_owned())
                        .and_then(|path| {
                            crate::scm::unstage_file(&root, &path).map(|_| "File unstaged".into())
                        }),
                    ScmAction::Commit => crate::scm::commit(&root, &message)
                        .map(|id| format!("Commit {} created", &id[..id.len().min(8)])),
                    ScmAction::StageCursorLine => selected
                        .ok_or_else(|| "Select a changed file first".to_owned())
                        .and_then(|path| {
                            crate::scm::stage_line(&root, &path, line)
                                .map(|_| "Selected line staged".into())
                        }),
                    ScmAction::RevertCursorLine => selected
                        .ok_or_else(|| "Select a changed file first".to_owned())
                        .and_then(|path| {
                            crate::scm::revert_line(&root, &path, line)
                                .map(|_| "Selected line reverted".into())
                        }),
                    ScmAction::StageHunk(index) => selected
                        .ok_or_else(|| "Select a changed file first".to_owned())
                        .and_then(|path| {
                            crate::scm::stage_hunk(&root, &path, index)
                                .map(|_| "Hunk staged".into())
                        }),
                    ScmAction::RevertFirstHunk => selected
                        .ok_or_else(|| "Select a changed file first".to_owned())
                        .and_then(|path| {
                            crate::scm::revert_hunk(&root, &path, 0)
                                .map(|_| "First hunk reverted".into())
                        }),
                };
                let snapshot = crate::scm::snapshot(&root);
                let message =
                    result.unwrap_or_else(|error| format!("Git operation failed: {error}"));
                let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                    message,
                    snapshot: Some(snapshot),
                });
            })
            .ok();
    }

    pub(crate) fn open_diff_paths(&mut self, left: PathBuf, right: PathBuf) {
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = "Diff view requires the native event loop".into();
            return;
        };
        self.diff_loading = true;
        std::thread::Builder::new()
            .name("graphite-diff-load".into())
            .spawn(move || {
                let read = |path: &PathBuf| {
                    std::fs::read_to_string(path)
                        .map_err(|error| format!("Cannot read {}: {error}", path.display()))
                };
                match (read(&left), read(&right)) {
                    (Ok(left_text), Ok(right_text)) => {
                        let _ = proxy.send_event(UserEvent::DiffReady(DiffView {
                            title: "File comparison".into(),
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
                            .unwrap_or_else(|| "Could not load diff".into());
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
        let Some(path) = self.documents[self.active_document()]
            .buffer
            .path()
            .map(PathBuf::from)
        else {
            self.status = "Save the current file before comparing it".into();
            return;
        };
        let root = self.tree.root().to_path_buf();
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        self.diff_loading = true;
        let current = self.documents[self.active_document()].buffer.text();
        std::thread::Builder::new()
            .name("graphite-head-diff".into())
            .spawn(move || {
                let result = (|| {
                    let previous = crate::scm::head_file_text(&root, &path)?;
                    Ok::<_, String>(DiffView {
                        title: "Working tree ↔ HEAD".into(),
                        left_name: format!("HEAD — {}", path.display()),
                        right_name: format!("Working tree — {}", path.display()),
                        left: previous,
                        right: current,
                        inline: false,
                    })
                })();
                match result {
                    Ok(view) => {
                        let _ = proxy.send_event(UserEvent::DiffReady(view));
                    }
                    Err(error) => {
                        let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                            message: format!("Cannot compare with HEAD: {error}"),
                            snapshot: None,
                        });
                    }
                }
            })
            .ok();
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
        std::thread::Builder::new()
            .name("graphite-conflict-save".into())
            .spawn(move || {
                let result = crate::scm::stage_file(&root, &path);
                let message = result
                    .map(|_| "Saved conflict resolution and staged it".to_owned())
                    .unwrap_or_else(|error| {
                        format!("Saved file, but could not stage conflict resolution: {error}")
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
            return;
        };
        std::thread::Builder::new()
            .name("graphite-conflict-load".into())
            .spawn(move || match crate::scm::merge_conflict(&root, &path) {
                Ok(sides) => {
                    let _ =
                        proxy.send_event(UserEvent::ConflictReady(ConflictView { path, sides }));
                }
                Err(error) => {
                    let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                        message: format!("Cannot open conflict: {error}"),
                        snapshot: None,
                    });
                }
            })
            .ok();
    }

    pub(crate) fn resolve_conflict_choice(&mut self, side: usize) {
        let Some(conflict) = self.conflict_view.take() else {
            return;
        };
        if side == 3 {
            self.open_file(conflict.path);
            self.status = "Edit the conflict resolution and save to update the Git index".into();
            return;
        }
        let Some(contents) = conflict.sides.get(side).cloned().flatten() else {
            self.status = "That conflict side is unavailable".into();
            self.conflict_view = Some(conflict);
            return;
        };
        let root = self.tree.root().to_path_buf();
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let path = conflict.path;
        std::thread::Builder::new()
            .name("graphite-conflict-resolve".into())
            .spawn(move || {
                let result = crate::scm::resolve_conflict(&root, &path, &contents);
                let message = result
                    .map(|_| "Conflict resolution written and staged".into())
                    .unwrap_or_else(|error| format!("Conflict resolution failed: {error}"));
                let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                    message,
                    snapshot: Some(crate::scm::snapshot(&root)),
                });
            })
            .ok();
    }

    pub(crate) fn draw_scm_sidebar(
        &self,
        renderer: &mut Renderer,
        height: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        renderer.draw_rect(
            0.0,
            0.0,
            SIDEBAR_WIDTH,
            height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        renderer.draw_text(
            "SOURCE CONTROL",
            PAD,
            line_height * 0.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        let branch = self
            .scm_snapshot
            .branch
            .as_deref()
            .unwrap_or("No Git repository");
        renderer.draw_text(
            &format!("Branch: {branch}"),
            PAD,
            line_height * 1.25,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
        renderer.draw_text(
            "S stage  U unstage  H hunk  R resolve  C commit",
            PAD,
            line_height * 2.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        if let Some(error) = &self.scm_snapshot.error {
            renderer.draw_text(
                error,
                PAD,
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
                    0.0,
                    y,
                    SIDEBAR_WIDTH,
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
                PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }
        if self.scm_commit_prompt {
            renderer.draw_rect(
                0.0,
                height - line_height * 2.0,
                SIDEBAR_WIDTH,
                line_height * 2.0,
                rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
            );
            renderer.draw_text(
                &format!("Commit: {}_", self.scm_commit_message),
                PAD,
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
                "OUTLINE",
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
                &format!("{}   Esc closes", diff.title),
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
                "PROCESS EXPLORER  (CPU / memory, wheel to scroll)",
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
                "STARTUP PERFORMANCE",
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
                "Checkout local branch (Enter / Esc)",
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
                "Three-way merge   O ours  T theirs  B base  E edit  Esc close",
                PAD,
                PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let pane_width = width / 3.0;
            for (index, (label, side)) in [
                ("BASE", conflict.sides[0].as_deref()),
                ("OURS", conflict.sides[1].as_deref()),
                ("THEIRS", conflict.sides[2].as_deref()),
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
                        "No base version",
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
