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
    let row = layout
        .list_rows
        .iter()
        .find(|row| row.bounds.contains(x, y))?;
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
            if !staged
                && discardable(index)
                && discard_button(row.action, cell_width).contains(x, y)
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
        ScmListRowKind::GraphCommitFile { .. } | ScmListRowKind::Extension(_) => None,
    }
}

/// A screen-space rectangle for the commit hover card and its clickable footer links.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CardRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl CardRect {
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Active commit hover card bounds and footer action hitboxes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CommitHoverCard {
    pub commit_index: usize,
    pub hash: String,
    pub short_hash: String,
    pub github_url: Option<String>,
    pub bounds: CardRect,
    pub copy_sha_rect: CardRect,
    pub open_github_rect: Option<CardRect>,
}

fn author_initials(author: &str) -> String {
    let initials: String = author
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    if initials.is_empty() {
        "?".to_owned()
    } else {
        initials
    }
}

fn author_badge_color(author: &str) -> u32 {
    const PALETTE: [u32; 6] = [0x2b6cb0, 0x2f855a, 0x6b46c1, 0xb7791f, 0xc53030, 0x0987a0];
    let hash = author.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    });
    PALETTE[hash % PALETTE.len()]
}

fn wrap_text(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let limit = max_chars.max(8);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word_len <= limit {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            if lines.len() >= max_lines {
                return lines;
            }
            current = word.to_owned();
        }
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn ref_pill_style(reference: &scm_graph::GraphRef) -> (&str, u32, u32) {
    match reference {
        scm_graph::GraphRef::HeadBranch(name) => (name.as_str(), 0x2b6cb0, 0xffffff),
        scm_graph::GraphRef::LocalBranch(name) => (name.as_str(), 0x3a3d41, 0xd4d4d4),
        scm_graph::GraphRef::RemoteBranch(name) => (name.as_str(), 0x6b46c1, 0xffffff),
        scm_graph::GraphRef::Tag(name) => (name.as_str(), 0x7c5800, 0xffeb99),
    }
}

fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
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
        {
            return None;
        }
        if let Some(card) = self.scm.hover_card.borrow().as_ref() {
            if card.bounds.contains(x, y) {
                return Some(Tip::Commit(card.commit_index));
            }
        }
        if !self.primary_sidebar_contains(x, y, window.0, window.1, lh) {
            return None;
        }
        let local = (self.sidebar_local_x(x, window.0), self.sidebar_local_y(y));
        let layout = self.scm_sidebar_layout_for(window.1, lh, cw);
        tip_at(
            &layout,
            local,
            cw,
            self.scm.repositories_collapsed,
            self.scm_primary_action(),
            |index| self.discard_target_at(index).is_some(),
        )
    }

    /// Whether the pointer is hovering an interactive link inside the commit hover card.
    pub(crate) fn commit_hover_link_at(&self, x: f32, y: f32) -> bool {
        self.scm.hover_card.borrow().as_ref().is_some_and(|card| {
            card.copy_sha_rect.contains(x, y)
                || card
                    .open_github_rect
                    .is_some_and(|rect| rect.contains(x, y))
        })
    }

    /// Handles a left click on the active commit hover card's footer actions.
    pub(crate) fn handle_commit_hover_click(&mut self, x: f32, y: f32) -> bool {
        let Some(card) = self.scm.hover_card.borrow().clone() else {
            return false;
        };
        if card.copy_sha_rect.contains(x, y) {
            use crate::app::clipboard::{ClipboardAccess, SystemClipboard};
            let mut clipboard = SystemClipboard;
            let _ = clipboard.set_text(&card.hash);
            self.status = self.tf("scm.tip.commit.copied_sha", &[("hash", &card.short_hash)]);
            return true;
        }
        if card
            .open_github_rect
            .is_some_and(|rect| rect.contains(x, y))
        {
            if let Some(url) = card.github_url {
                crate::app::plugins::drawing::open_external_https(&url);
            }
            return true;
        }
        card.bounds.contains(x, y)
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
        self.scm.visible
            && (self.scm_tip_at(x, y, window, lh, cw) != self.scm_tip_at(px, py, window, lh, cw)
                || self.commit_hover_link_at(x, y) != self.commit_hover_link_at(px, py))
    }

    fn cached_commit_stats(&self, hash: &str) -> Option<scm_graph::CommitDiffStats> {
        let mut cache = self.scm.commit_stats.borrow_mut();
        if let Some(entry) = cache.get(hash) {
            return *entry;
        }
        let root = self.scm_workspace_root();
        let stats = scm_graph::load_commit_shortstat(&root, hash);
        cache.insert(hash.to_owned(), stats);
        stats
    }

    fn cached_github_base_url(&self) -> Option<String> {
        let mut base = self.scm.commit_github_base.borrow_mut();
        if let Some(cached) = base.as_ref() {
            return cached.clone();
        }
        let root = self.scm_workspace_root();
        let resolved = scm_graph::github_repo_base_url(&root);
        *base = Some(resolved.clone());
        resolved
    }

    /// The hover card under the pointer.
    pub(crate) fn draw_scm_tooltip(&self, renderer: &mut Renderer) {
        if self.scm.graph_menu.is_some()
            || self.scm.more_menu.is_some()
            || self.scm.confirm.is_some()
            || self.explorer_context_menu.is_some()
            || self.scm.graph_resize_dragging
        {
            *self.scm.hover_card.borrow_mut() = None;
            return;
        }
        let (width, height) = renderer.size();
        let (cw, lh) = (renderer.char_width().max(4.0), renderer.line_height());
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        match self.scm_tip_at(mx, my, (width, height), lh, cw) {
            Some(Tip::Button(key)) => {
                *self.scm.hover_card.borrow_mut() = None;
                let line = self.t(key);
                let box_width =
                    ((line.chars().count() as f32 * cw) + 20.0).min((width - 16.0).max(60.0));
                let box_height = lh + 12.0;
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
                renderer.draw_rect(x, y, box_width, box_height, border);
                renderer.draw_rect(x + 1.0, y + 1.0, box_width - 2.0, box_height - 2.0, bg);
                let clipped = Self::help_menu_visible_label(line, cw, (box_width - 20.0).max(0.0));
                renderer.draw_text(&clipped, x + 10.0, y + 6.0, text);
            }
            Some(Tip::Commit(index)) => {
                let Some(row) = self.scm.graph_rows.get(index) else {
                    *self.scm.hover_card.borrow_mut() = None;
                    return;
                };
                self.draw_commit_hover_card(
                    renderer,
                    index,
                    &row.commit,
                    (width, height),
                    (lh, cw),
                    (mx, my),
                );
            }
            None => {
                *self.scm.hover_card.borrow_mut() = None;
            }
        }
    }

    fn draw_commit_hover_card(
        &self,
        renderer: &mut Renderer,
        commit_index: usize,
        commit: &scm_graph::GraphCommit,
        (win_w, win_h): (f32, f32),
        (lh, cw): (f32, f32),
        (mx, my): (f32, f32),
    ) {
        let pad_x = 12.0;
        let pad_y = 10.0;
        let max_card_w = (win_w - 24.0).clamp(260.0, 560.0);
        let max_inner_chars = ((max_card_w - pad_x * 2.0 - cw * 2.0) / cw)
            .floor()
            .max(24.0) as usize;

        let when =
            scm_graph::format_commit_when(&commit.date, now_unix_seconds(), &self.translator);
        let header_author = if commit.author.is_empty() {
            "Unknown".to_owned()
        } else {
            commit.author.clone()
        };
        let header_when = if when.is_empty() {
            String::new()
        } else {
            format!(", {when}")
        };
        let avatar_w = 20.0;
        let header_chars = 3 + header_author.chars().count() + header_when.chars().count();

        let subject_lines = wrap_text(&commit.subject, max_inner_chars, 3);
        let stats = self.cached_commit_stats(&commit.hash);

        let files_text = stats.map(|s| {
            if s.files_changed == 1 {
                self.t("scm.tip.stat.file_changed").to_owned()
            } else {
                self.tf("scm.tip.stat.files_changed", &[("count", &s.files_changed)])
            }
        });
        let ins_text = stats.filter(|s| s.insertions > 0).map(|s| {
            if s.insertions == 1 {
                self.t("scm.tip.stat.insertion").to_owned()
            } else {
                self.tf("scm.tip.stat.insertions", &[("count", &s.insertions)])
            }
        });
        let del_text = stats.filter(|s| s.deletions > 0).map(|s| {
            if s.deletions == 1 {
                self.t("scm.tip.stat.deletion").to_owned()
            } else {
                self.tf("scm.tip.stat.deletions", &[("count", &s.deletions)])
            }
        });
        let stats_chars = files_text.as_ref().map_or(0, |t| t.chars().count())
            + ins_text.as_ref().map_or(0, |t| 2 + t.chars().count())
            + del_text.as_ref().map_or(0, |t| 2 + t.chars().count());

        let github_url = self
            .cached_github_base_url()
            .map(|base| scm_graph::commit_github_url(&base, &commit.hash));
        let open_github_label = self.t("scm.tip.commit.open_github");
        let copy_label = format!("{} [copy]", commit.short_hash);
        let footer_chars = copy_label.chars().count()
            + github_url
                .as_ref()
                .map_or(0, |_| 3 + open_github_label.chars().count());

        let longest_subject = subject_lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0);
        let needed_chars = header_chars
            .max(longest_subject)
            .max(stats_chars)
            .max(footer_chars)
            .min(max_inner_chars);
        let box_width =
            (needed_chars as f32 * cw + pad_x * 2.0 + cw * 2.0).clamp(280.0, max_card_w);
        let inner_w = box_width - pad_x * 2.0;

        // Compute ref pill rows to know exact card height.
        let pill_h = (lh * 0.9).max(16.0);
        let pill_gap = 6.0;
        let mut pill_rows = 0usize;
        if !commit.refs.is_empty() {
            pill_rows = 1;
            let mut row_x = 0.0f32;
            for reference in &commit.refs {
                let (label, _, _) = ref_pill_style(reference);
                let pw = label.chars().count() as f32 * cw + 14.0;
                if row_x > 0.0 && row_x + pw > inner_w {
                    pill_rows += 1;
                    row_x = pw + pill_gap;
                } else {
                    row_x += pw + pill_gap;
                }
            }
        }

        let stats_rows = if let Some(files) = &files_text {
            let mut rows = 1usize;
            let mut row_w = files.chars().count() as f32 * cw;
            for part in [ins_text.as_deref(), del_text.as_deref()]
                .into_iter()
                .flatten()
            {
                let part_w = (2 + part.chars().count()) as f32 * cw;
                if row_w > 0.0 && row_w + part_w > inner_w {
                    rows += 1;
                    row_w = part.chars().count() as f32 * cw;
                } else {
                    row_w += part_w;
                }
            }
            rows
        } else {
            0
        };
        let divider_gap = 11.0;
        let stats_height = if stats_rows > 0 {
            stats_rows as f32 * lh + 4.0
        } else {
            0.0
        };
        let pills_height = if pill_rows > 0 {
            pill_rows as f32 * (pill_h + 4.0) + 2.0
        } else {
            0.0
        };
        let mid_section_height = if stats.is_some() || pill_rows > 0 {
            divider_gap + stats_height + pills_height
        } else {
            0.0
        };
        let box_height = pad_y * 2.0
            + lh // header
            + 4.0
            + subject_lines.len() as f32 * lh
            + mid_section_height
            + divider_gap
            + lh; // footer

        let existing_pos = self
            .scm
            .hover_card
            .borrow()
            .as_ref()
            .filter(|card| card.commit_index == commit_index && card.bounds.contains(mx, my))
            .map(|card| (card.bounds.x, card.bounds.y));
        let (x, y) = existing_pos.unwrap_or_else(|| {
            let x = if mx + 10.0 + box_width <= win_w - 4.0 {
                mx + 10.0
            } else {
                (win_w - box_width - 8.0).max(4.0)
            };
            let y = if my + 12.0 + box_height <= win_h - 4.0 {
                my + 12.0
            } else {
                (my - box_height - 6.0).max(4.0)
            };
            (x, y)
        });

        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x1f1f1f));
        let divider = rgb(Theme::color(Theme::DIM, 0x383838));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x9e9e9e));
        let link = rgb(0x4ea1ff);
        let green = rgb(0x73c991);
        let red = rgb(0xf48771);

        renderer.draw_rect(x, y, box_width, box_height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, box_width - 2.0, box_height - 2.0, bg);

        let mut cy = y + pad_y;
        let cx = x + pad_x;

        // 1. Header: initials badge + author + relative time + date
        let badge_bg = rgb(author_badge_color(&header_author));
        let badge_y = cy + ((lh - avatar_w) * 0.5).max(0.0);
        renderer.draw_rect(cx, badge_y, avatar_w, avatar_w.min(lh), badge_bg);
        let initials = author_initials(&header_author);
        let init_x = cx + ((avatar_w - initials.chars().count() as f32 * cw) * 0.5).max(1.0);
        renderer.draw_text(&initials, init_x, cy, rgb(0xffffff));

        let author_x = cx + avatar_w + 6.0;
        let author_clipped =
            Self::help_menu_visible_label(&header_author, cw, (inner_w - avatar_w - 6.0).max(0.0));
        renderer.draw_text(&author_clipped, author_x, cy, link);
        let when_x = author_x + author_clipped.chars().count() as f32 * cw;
        let when_avail = (x + box_width - pad_x - when_x).max(0.0);
        if !header_when.is_empty() && when_avail > cw * 2.0 {
            let when_clipped = Self::help_menu_visible_label(&header_when, cw, when_avail);
            renderer.draw_text(&when_clipped, when_x, cy, dim);
        }
        cy += lh + 4.0;

        // 2. Commit subject
        for line in &subject_lines {
            let clipped = Self::help_menu_visible_label(line, cw, inner_w);
            renderer.draw_text(&clipped, cx, cy, text);
            cy += lh;
        }

        // 3. Divider + Stats + Ref pills
        if stats.is_some() || pill_rows > 0 {
            cy += 5.0;
            renderer.draw_rect(x + 1.0, cy, box_width - 2.0, 1.0, divider);
            cy += 6.0;

            if let Some(files) = &files_text {
                let mut sx = cx;
                renderer.draw_text(files, sx, cy, text);
                sx += files.chars().count() as f32 * cw;
                for (part, color) in [(ins_text.as_deref(), green), (del_text.as_deref(), red)] {
                    let Some(part) = part else { continue };
                    let part_w = part.chars().count() as f32 * cw;
                    if sx > cx && sx + 2.0 * cw + part_w > cx + inner_w {
                        cy += lh;
                        sx = cx;
                    } else {
                        renderer.draw_text(", ", sx, cy, dim);
                        sx += 2.0 * cw;
                    }
                    let clipped =
                        Self::help_menu_visible_label(part, cw, (cx + inner_w - sx).max(0.0));
                    renderer.draw_text(&clipped, sx, cy, color);
                    sx += clipped.chars().count() as f32 * cw;
                }
                cy += lh + 4.0;
            }

            if !commit.refs.is_empty() {
                let mut px = cx;
                for reference in &commit.refs {
                    let (label, bg_hex, fg_hex) = ref_pill_style(reference);
                    let pw = label.chars().count() as f32 * cw + 14.0;
                    if px > cx && px + pw > cx + inner_w {
                        px = cx;
                        cy += pill_h + 4.0;
                    }
                    renderer.draw_rect(px, cy, pw, pill_h, rgb(bg_hex));
                    renderer.draw_text(label, px + 7.0, cy + (pill_h - lh) * 0.5, rgb(fg_hex));
                    px += pw + pill_gap;
                }
                cy += pill_h + 6.0;
            }
        }

        // 4. Footer divider + interactive actions
        cy += 5.0;
        renderer.draw_rect(x + 1.0, cy, box_width - 2.0, 1.0, divider);
        cy += 6.0;

        let copy_w = copy_label.chars().count() as f32 * cw;
        let copy_sha_rect = CardRect {
            x: cx,
            y: cy,
            width: copy_w,
            height: lh,
        };
        let copy_color = if copy_sha_rect.contains(mx, my) {
            rgb(0x7cc0ff)
        } else {
            link
        };
        renderer.draw_text(&copy_label, cx, cy, copy_color);

        let open_github_rect = github_url.as_ref().map(|_| {
            let sep_x = cx + copy_w;
            renderer.draw_text(" | ", sep_x, cy, dim);
            let gh_x = sep_x + 3.0 * cw;
            let gh_w = open_github_label.chars().count() as f32 * cw;
            let rect = CardRect {
                x: gh_x,
                y: cy,
                width: gh_w,
                height: lh,
            };
            let gh_color = if rect.contains(mx, my) {
                rgb(0x7cc0ff)
            } else {
                link
            };
            renderer.draw_text(open_github_label, gh_x, cy, gh_color);
            rect
        });

        *self.scm.hover_card.borrow_mut() = Some(CommitHoverCard {
            commit_index,
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
            github_url,
            bounds: CardRect {
                x,
                y,
                width: box_width,
                height: box_height,
            },
            copy_sha_rect,
            open_github_rect,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::scm_ui::{scm_panel_layout_with_graph, ScmListView};
    use super::*;
    use crate::scm::{FileStatus, Snapshot};

    fn layout(snapshot: &Snapshot) -> ScmPanelLayout {
        scm_panel_layout_with_graph(
            snapshot,
            300.0,
            700.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
            &[],
            true,
            false,
            2,
            0,
            None,
            &ScmListView::LIST,
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
            tip_at(
                &layout,
                point,
                9.0,
                collapsed,
                ScmPrimaryAction::Commit,
                |_| true,
            )
        };
        assert_eq!(
            tip(centre(layout.repo_sync), false),
            Some(Tip::Button("scm.tip.sync"))
        );
        assert_eq!(
            tip(centre(layout.repo_pull_request), false),
            Some(Tip::Button("scm.tip.pull_request"))
        );
        assert_eq!(
            tip(centre(layout.repo_more), false),
            Some(Tip::Button("scm.tip.more"))
        );
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
            tip_at(&layout, label, 9.0, false, ScmPrimaryAction::Commit, |_| {
                true
            }),
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
            tip_at(&layout, point, 9.0, false, ScmPrimaryAction::Commit, |_| {
                can_discard
            })
        };
        assert_eq!(tip(stage, true), Some(Tip::Button("scm.tip.stage")));
        assert_eq!(tip(discard, true), Some(Tip::Button("scm.tip.discard")));
        assert_eq!(
            tip(discard, false),
            None,
            "no discard button on a conflicted file"
        );
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
                tip_at(
                    &layout,
                    centre(rect),
                    9.0,
                    false,
                    ScmPrimaryAction::Commit,
                    |_| true
                ),
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
            tip_at(
                &layout,
                centre(commit.bounds),
                9.0,
                false,
                ScmPrimaryAction::Commit,
                |_| true
            ),
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

    #[test]
    fn commit_hover_card_helpers_and_footer_clicks_work() {
        assert_eq!(author_initials("Tonho"), "T");
        assert_eq!(author_initials("Jane Doe"), "JD");
        assert_eq!(author_initials(""), "?");

        let wrapped = wrap_text(
            "feat: add commands for previewing and trying new color themes",
            24,
            3,
        );
        assert!(wrapped.len() >= 2);
        assert!(wrapped.iter().all(|line| line.chars().count() <= 24));

        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm.visible = true;
        app.view_preferences.primary_sidebar_visible = true;
        *app.scm.hover_card.borrow_mut() = Some(CommitHoverCard {
            commit_index: 0,
            hash: "cdc385ed072c3ece00fad8ae978e4231a73d4d14".into(),
            short_hash: "cdc385e".into(),
            github_url: Some("https://github.com/owner/repo/commit/cdc385e".into()),
            bounds: CardRect {
                x: 100.0,
                y: 100.0,
                width: 300.0,
                height: 120.0,
            },
            copy_sha_rect: CardRect {
                x: 112.0,
                y: 196.0,
                width: 80.0,
                height: 18.0,
            },
            open_github_rect: Some(CardRect {
                x: 210.0,
                y: 196.0,
                width: 110.0,
                height: 18.0,
            }),
        });

        assert!(app.commit_hover_link_at(120.0, 200.0));
        assert!(app.commit_hover_link_at(220.0, 200.0));
        assert!(!app.commit_hover_link_at(120.0, 120.0));

        assert_eq!(
            app.scm_tip_at(150.0, 150.0, (1280.0, 720.0), 20.0, 9.0),
            Some(Tip::Commit(0))
        );
        assert!(app.handle_commit_hover_click(120.0, 200.0));
        assert!(app.status.contains("cdc385e"));
    }
}
