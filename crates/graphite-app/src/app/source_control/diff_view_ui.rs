//! Drawing and hit testing of the two-file diff view.
//!
//! The view covers only the editor area, so the menu bar, activity bar, sidebar and status
//! bar stay visible and clickable. Its header carries a layout toggle and a close button.
use super::*;
use crate::vcs::side_by_side::{Cell, DiffModel, InlineItem, Item, RowKind};

type Rect = (f32, f32, f32, f32);

/// What a click on the diff header landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffHeaderHit {
    Close,
    ToggleLayout,
    None,
}

fn inside(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

/// Header strip height of the diff view.
pub(crate) fn diff_header_height(line_height: f32) -> f32 {
    line_height * 1.5
}

/// Close and layout button rectangles for a diff area.
pub(crate) fn diff_header_buttons(
    area: Rect,
    line_height: f32,
    char_width: f32,
    layout_label_chars: usize,
) -> (Rect, Rect) {
    let header = diff_header_height(line_height);
    let close = (area.0 + area.2 - header, area.1, header, header);
    let layout_w = layout_label_chars as f32 * char_width + 16.0;
    let layout = (close.0 - layout_w, area.1, layout_w, header);
    (close, layout)
}

pub(crate) fn diff_header_hit(
    area: Rect,
    line_height: f32,
    char_width: f32,
    layout_label_chars: usize,
    x: f32,
    y: f32,
) -> DiffHeaderHit {
    let (close, layout) = diff_header_buttons(area, line_height, char_width, layout_label_chars);
    if inside(close, x, y) {
        DiffHeaderHit::Close
    } else if inside(layout, x, y) {
        DiffHeaderHit::ToggleLayout
    } else {
        DiffHeaderHit::None
    }
}

fn review_buttons(area: Rect, lh: f32, cw: f32, apply: &str, discard: &str) -> (Rect, Rect) {
    let height = diff_header_height(lh);
    let width = |label: &str| (label.chars().count() as f32 * cw + PAD * 2.0).min(area.2 * 0.45);
    let apply = (area.0 + PAD, area.1 + height, width(apply), height);
    let discard = (apply.0 + apply.2 + PAD, apply.1, width(discard), height);
    (apply, discard)
}

impl App {
    fn ai_hunk_button(&self, area: Rect, lh: f32) -> Option<Rect> {
        (self.ai_mode.is_some() && self.chat_review_open()).then(|| {
            let header = diff_header_height(lh);
            (
                area.0 + PAD,
                area.1 + header * 2.0,
                (area.2 - PAD * 2.0).max(0.0),
                header,
            )
        })
    }

    fn diff_titles_y(&self, area: Rect, lh: f32) -> f32 {
        let count = 1
            + usize::from(self.chat_review_open())
            + usize::from(self.ai_hunk_button(area, lh).is_some());
        area.1 + diff_header_height(lh) * count as f32
    }

    fn diff_visible_row_count(&self, area: Rect, lh: f32) -> usize {
        let rows_top = self.diff_titles_y(area, lh) + lh * 1.2;
        ((area.1 + area.3 - rows_top) / lh).floor().max(0.0) as usize
    }

    pub(crate) fn diff_contains_point(
        &self,
        width: f32,
        height: f32,
        lh: f32,
        x: f32,
        y: f32,
    ) -> bool {
        inside(self.diff_area(width, height, lh), x, y)
    }

    /// The editor area the diff view occupies: right of the activity bar and sidebar, below the
    /// menu bar and above the status bar.
    pub(crate) fn diff_area(&self, width: f32, height: f32, line_height: f32) -> Rect {
        let top = self.top_menu_height();
        if self.ai_mode.is_some() && self.chat_review_open() {
            return self
                .ai_layout(width, height, line_height)
                .review
                .map_or((0.0, 0.0, 0.0, 0.0), |area| {
                    (area.x, area.y, area.width, area.height)
                });
        }
        if self.chat_review_open() || self.ai_mode.is_some() {
            let (left, right) = if self.ai_mode.is_some() {
                (0.0, width)
            } else {
                self.editor_column(width)
            };
            return (
                left,
                top,
                (right - left).max(0.0),
                (height - line_height - top).max(0.0),
            );
        }
        let bottom = if self.view_preferences.status_bar_visible && !self.zen_mode {
            (height - line_height).max(top)
        } else {
            height
        };
        let (mut left, mut right) = (self.activity_bar_width_px(), width);
        if self.view_preferences.primary_sidebar_visible && !self.zen_mode {
            let (sidebar_left, sidebar_width) = self.primary_sidebar_bounds(width);
            if self.activity_bar_preferences.primary_sidebar_right {
                right = sidebar_left;
            } else {
                left = sidebar_left + sidebar_width;
            }
        }
        (left, top, (right - left).max(0.0), (bottom - top).max(0.0))
    }

    pub(crate) fn diff_layout_label(&self) -> &str {
        self.t("scm.diff.toggle_layout")
    }

    pub(crate) fn close_diff_view(&mut self) {
        self.run_ui_command("graphite.action.closeDiffEditor");
        self.status = self.t("diff.closed").into();
        self.check_cli_waiters();
    }

    /// Handles a click while the diff view is open. Returns true when the click was consumed;
    /// a click outside the view returns false so the rest of the window still works.
    pub(crate) fn click_diff_view(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
        line_height: f32,
        char_width: f32,
    ) -> bool {
        if self.diff_view.is_none() {
            return false;
        }
        let area = self.diff_area(width, height, line_height);
        if inside(area, mx, my) {
            if self.chat_review_open() {
                if self.ai_mode.is_some() {
                    self.focus_region(crate::app::focus_regions::FocusRegion::Review);
                }
                if self
                    .ai_hunk_button(area, line_height)
                    .is_some_and(|rect| inside(rect, mx, my))
                {
                    self.open_review_hunk_in_editor();
                    return true;
                }
                let (apply, discard) = review_buttons(
                    area,
                    line_height,
                    char_width,
                    self.t("chat.review.apply"),
                    self.t("chat.review.discard"),
                );
                if inside(apply, mx, my) {
                    self.decide_chat_review(true);
                    return true;
                }
                if inside(discard, mx, my) {
                    self.decide_chat_review(false);
                    return true;
                }
            }
            let label_chars = self.diff_layout_label().chars().count();
            match diff_header_hit(area, line_height, char_width, label_chars, mx, my) {
                DiffHeaderHit::Close => self.close_diff_view(),
                DiffHeaderHit::ToggleLayout => self.run_ui_command("graphite.diff.toggleLayout"),
                DiffHeaderHit::None => {
                    if self.ai_mode.is_some() && self.chat_review_open() {
                        let rows_top = self.diff_titles_y(area, line_height) + line_height * 1.2;
                        let slot = ((my - rows_top) / line_height).floor();
                        if slot >= 0.0
                            && (slot as usize) < self.diff_visible_row_count(area, line_height)
                        {
                            if let Some(diff) = &self.diff_view {
                                let first = self.diff_scroll.min(self.diff_max_scroll());
                                let row = self.diff_model().row_at_slot(
                                    first + slot as usize,
                                    diff.inline,
                                    diff.collapse_unchanged,
                                );
                                self.select_review_row(row);
                            }
                        }
                    }
                }
            }
            return true;
        }
        // Using the sidebar means leaving the comparison; the click is then handled normally.
        if self.ai_mode.is_none()
            && self.primary_sidebar_contains(mx, my, width, height, line_height)
        {
            self.close_diff_view();
        }
        false
    }

    /// Rows of the open diff, cached until the texts change.
    pub(crate) fn diff_model(&self) -> std::sync::Arc<DiffModel> {
        use std::hash::{Hash, Hasher};
        let Some(diff) = &self.diff_view else {
            return std::sync::Arc::new(DiffModel::default());
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        diff.left.hash(&mut hasher);
        diff.right.hash(&mut hasher);
        let key = hasher.finish();
        let mut cache = self.diff_model_cache.borrow_mut();
        if let Some((cached, model)) = cache.as_ref() {
            if *cached == key {
                return model.clone();
            }
        }
        let model = std::sync::Arc::new(DiffModel::build(&diff.left, &diff.right));
        *cache = Some((key, model.clone()));
        model
    }

    fn diff_item_count(&self) -> usize {
        let Some(diff) = &self.diff_view else {
            return 0;
        };
        let model = self.diff_model();
        if diff.inline {
            model.inline_rows(diff.collapse_unchanged).len()
        } else {
            model.items(diff.collapse_unchanged).len()
        }
    }

    fn diff_max_scroll(&self) -> usize {
        self.diff_item_count()
            .saturating_sub(self.diff_visible_rows.get().max(1))
    }

    pub(crate) fn scroll_diff(&mut self, delta: isize) {
        let max = self.diff_max_scroll() as isize;
        self.diff_scroll = (self.diff_scroll as isize)
            .saturating_add(delta)
            .clamp(0, max) as usize;
    }

    /// Scrolls so the change that starts at `row` of the model is near the top.
    pub(crate) fn scroll_diff_to_row(&mut self, row: usize) {
        let Some(diff) = &self.diff_view else {
            return;
        };
        let model = self.diff_model();
        let items = model.items(diff.collapse_unchanged);
        let position = model.slot_for_row(&items, row, diff.inline);
        let max = self.diff_max_scroll();
        self.diff_scroll = position.saturating_sub(2).min(max);
    }

    pub(crate) fn draw_diff_view(
        &self,
        renderer: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
    ) {
        let Some(diff) = &self.diff_view else {
            return;
        };
        let cw = renderer.char_width();
        let (x, y, w, h) = self.diff_area(width, height, line_height);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        renderer.set_clip(Some([x, y, x + w, y + h]));
        let bg = rgb(Theme::color(Theme::BG, 0xffffff));
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        let header_bg = rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3));
        let button_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let header = diff_header_height(line_height);
        let model = self.diff_model();

        renderer.draw_rect(x, y, w, h, bg);
        renderer.draw_rect(x, y, w, header, header_bg);

        let label = self.diff_layout_label();
        let (close, layout) =
            diff_header_buttons((x, y, w, h), line_height, cw, label.chars().count());
        for rect in [close, layout] {
            renderer.draw_rect(
                rect.0 + 1.0,
                rect.1 + 3.0,
                rect.2 - 2.0,
                rect.3 - 6.0,
                button_bg,
            );
        }
        let header_text_y = y + (header - line_height) * 0.5;
        renderer.draw_text(label, layout.0 + 8.0, header_text_y, text);
        renderer.draw_text("×", close.0 + (close.2 - cw) * 0.5, header_text_y, text);

        let added = format!("+{}", model.added);
        let removed = format!("−{}", model.removed);
        let stats_width = (added.chars().count() + removed.chars().count() + 1) as f32 * cw;
        let stats_x = layout.0 - PAD - stats_width;
        renderer.draw_text(
            &added,
            stats_x,
            header_text_y,
            rgb(Theme::color(0x4ec96b, 0x1a7f37)),
        );
        renderer.draw_text(
            &removed,
            stats_x + (added.chars().count() + 1) as f32 * cw,
            header_text_y,
            rgb(Theme::color(0xf2615b, 0xcf222e)),
        );

        let title = self.tf("scm.diff.escape_hint", &[("title", &diff.title)]);
        let title_room = (stats_x - x - PAD * 2.0).max(0.0);
        renderer.draw_text(
            &Self::help_menu_visible_label(&title, cw, title_room),
            x + PAD,
            header_text_y,
            text,
        );

        let titles_y = self.diff_titles_y((x, y, w, h), line_height);
        if self.chat_review_open() {
            renderer.draw_rect(x, y + header, w, header, header_bg);
            let labels = [self.t("chat.review.apply"), self.t("chat.review.discard")];
            let (apply, discard) =
                review_buttons((x, y, w, h), line_height, cw, labels[0], labels[1]);
            let row_y = y + header + (header - line_height) * 0.5;
            for (rect, label) in [apply, discard].into_iter().zip(labels) {
                renderer.draw_rect(rect.0, rect.1 + 2.0, rect.2, rect.3 - 4.0, button_bg);
                renderer.draw_text(
                    &Self::help_menu_visible_label(label, cw, (rect.2 - PAD * 2.0).max(0.0)),
                    rect.0 + PAD,
                    row_y,
                    text,
                );
            }
            if let Some(notice) = self.chat_review_notice() {
                let notice_x = discard.0 + discard.2 + PAD;
                renderer.draw_text(
                    &Self::help_menu_visible_label(notice, cw, (x + w - notice_x - PAD).max(0.0)),
                    notice_x,
                    row_y,
                    text,
                );
            }
        }
        if let Some(rect) = self.ai_hunk_button((x, y, w, h), line_height) {
            let enabled = self.can_open_review_hunk();
            let label = self.t(if enabled {
                "command.graphite.diff.openHunkInEditor"
            } else {
                "workbench.review.new_file"
            });
            renderer.draw_rect(x, rect.1, w, header, header_bg);
            if enabled {
                renderer.draw_rect(rect.0, rect.1 + 2.0, rect.2, rect.3 - 4.0, button_bg);
            }
            renderer.draw_text(
                &Self::help_menu_visible_label(label, cw, (rect.2 - PAD * 2.0).max(0.0)),
                rect.0 + PAD,
                rect.1 + (header - line_height) * 0.5,
                if enabled { text } else { dim },
            );
        }
        let pane_width = w / 2.0;
        renderer.draw_rect(x, titles_y, w, line_height * 1.2, bg);
        if !diff.inline {
            renderer.draw_text(
                &Self::help_menu_visible_label(&diff.left_name, cw, pane_width - PAD * 2.0),
                x + PAD,
                titles_y,
                dim,
            );
            renderer.draw_text(
                &Self::help_menu_visible_label(&diff.right_name, cw, pane_width - PAD * 2.0),
                x + pane_width + PAD,
                titles_y,
                dim,
            );
        } else {
            renderer.draw_text(
                &Self::help_menu_visible_label(&diff.right_name, cw, w - PAD * 2.0),
                x + PAD,
                titles_y,
                dim,
            );
        }

        let rows_top = titles_y + line_height * 1.2;
        let visible = self.diff_visible_row_count((x, y, w, h), line_height);
        self.diff_visible_rows.set(visible);

        if !model.has_changes() {
            renderer.draw_text(
                self.t("scm.diff.no_changes"),
                x + PAD,
                rows_top + line_height,
                dim,
            );
            renderer.set_clip(None);
            return;
        }

        let palette = DiffPalette::current();
        let digits = model
            .rows
            .iter()
            .flat_map(|row| row.left.iter().chain(row.right.iter()))
            .map(|cell| cell.number)
            .max()
            .unwrap_or(1)
            .to_string()
            .len();
        let gutter = (digits as f32 + 1.0) * cw + 6.0;
        let sign_w = cw + 4.0;
        let first = self.diff_scroll.min(self.diff_max_scroll());
        let selected = self.selected_review_row().map(|row| {
            let items = model.items(diff.collapse_unchanged);
            let start = model.slot_for_row(&items, row, diff.inline);
            let lines = if diff.inline
                && model
                    .rows
                    .get(row)
                    .is_some_and(|row| row.kind == RowKind::Modified)
            {
                2
            } else {
                1
            };
            start..start + lines
        });
        let mut painter = DiffPainter {
            renderer,
            cw,
            line_height,
            gutter,
            sign_w,
            text,
            dim,
            palette,
        };

        if diff.inline {
            let items = model.inline_rows(diff.collapse_unchanged);
            for (slot, item) in items.iter().skip(first).take(visible).enumerate() {
                let row_y = rows_top + slot as f32 * line_height;
                match item {
                    InlineItem::Folded(count) => {
                        painter.folded(x, row_y, w, *count, self, header_bg)
                    }
                    InlineItem::Line { sign, cell, .. } => {
                        let side = match sign {
                            '-' => Side::Removed,
                            '+' => Side::Added,
                            _ => Side::Plain,
                        };
                        painter.cell(x, row_y, w, Some(cell), side, *sign);
                    }
                }
            }
        } else {
            let items = model.items(diff.collapse_unchanged);
            for (slot, item) in items.iter().skip(first).take(visible).enumerate() {
                let row_y = rows_top + slot as f32 * line_height;
                match item {
                    Item::Folded(count) => painter.folded(x, row_y, w, *count, self, header_bg),
                    Item::Row(row) => {
                        let (left_side, right_side) = match row.kind {
                            RowKind::Equal => (Side::Plain, Side::Plain),
                            RowKind::Removed => (Side::Removed, Side::Filler),
                            RowKind::Added => (Side::Filler, Side::Added),
                            RowKind::Modified => (Side::Removed, Side::Added),
                        };
                        let left_sign = if left_side == Side::Removed { '-' } else { ' ' };
                        let right_sign = if right_side == Side::Added { '+' } else { ' ' };
                        painter.cell(
                            x,
                            row_y,
                            pane_width,
                            row.left.as_ref(),
                            left_side,
                            left_sign,
                        );
                        painter.cell(
                            x + pane_width,
                            row_y,
                            pane_width,
                            row.right.as_ref(),
                            right_side,
                            right_sign,
                        );
                    }
                }
            }
            painter.renderer.draw_rect(
                x + pane_width - 0.5,
                rows_top,
                1.0,
                (h - (rows_top - y)).max(0.0),
                header_bg,
            );
        }
        if let Some(range) = selected {
            let start = range.start.max(first);
            let end = range.end.min(first + visible);
            if start < end {
                painter.renderer.draw_rect(
                    x,
                    rows_top + (start - first) as f32 * line_height,
                    2.0,
                    (end - start) as f32 * line_height,
                    rgb(Theme::color(Theme::STATUS, 0x007acc)),
                );
            }
        }
        painter.renderer.set_clip(None);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Plain,
    Removed,
    Added,
    /// The other side has a line here and this side has none.
    Filler,
}

#[derive(Clone, Copy)]
struct DiffPalette {
    removed_line: u32,
    removed_word: u32,
    added_line: u32,
    added_word: u32,
    filler: u32,
}

impl DiffPalette {
    fn current() -> Self {
        Self {
            removed_line: Theme::color(0x3b1e1e, 0xffe7e7),
            removed_word: Theme::color(0x72282a, 0xffb8b8),
            added_line: Theme::color(0x1e3a22, 0xe4f8e6),
            added_word: Theme::color(0x2c6a35, 0xaaf0b4),
            filler: Theme::color(0x262628, 0xf0f0f0),
        }
    }
}

struct DiffPainter<'a> {
    renderer: &'a mut Renderer,
    cw: f32,
    line_height: f32,
    gutter: f32,
    sign_w: f32,
    text: Color,
    dim: Color,
    palette: DiffPalette,
}

impl DiffPainter<'_> {
    fn folded(&mut self, x: f32, y: f32, w: f32, count: usize, app: &App, bg: Color) {
        self.renderer.draw_rect(x, y, w, self.line_height, bg);
        let label = app.tf("scm.diff.folded", &[("count", &count)]);
        let room = (w - 16.0).max(0.0);
        let label = App::help_menu_visible_label(&label, self.cw, room);
        let label_x = x + (w - label.chars().count() as f32 * self.cw).max(0.0) * 0.5;
        self.renderer.draw_text(&label, label_x, y, self.dim);
    }

    /// One line in a pane that starts at `x` and is `width` wide.
    fn cell(&mut self, x: f32, y: f32, width: f32, cell: Option<&Cell>, side: Side, sign: char) {
        let (line, word) = match side {
            Side::Removed => (Some(self.palette.removed_line), self.palette.removed_word),
            Side::Added => (Some(self.palette.added_line), self.palette.added_word),
            Side::Filler => (Some(self.palette.filler), self.palette.filler),
            Side::Plain => (None, self.palette.filler),
        };
        if let Some(color) = line {
            self.renderer
                .draw_rect(x, y, width, self.line_height, rgb(color));
        }
        let Some(cell) = cell else {
            return;
        };
        if side != Side::Plain {
            self.renderer
                .draw_rect(x, y, self.gutter, self.line_height, rgb(word));
        }
        let number = cell.number.to_string();
        let number_x = x + self.gutter - 4.0 - number.chars().count() as f32 * self.cw;
        self.renderer.draw_text(&number, number_x, y, self.dim);
        if sign != ' ' {
            self.renderer
                .draw_text(&sign.to_string(), x + self.gutter + 2.0, y, self.text);
        }
        let text_x = x + self.gutter + self.sign_w;
        let chars = ((width - self.gutter - self.sign_w - 6.0) / self.cw).max(0.0) as usize;
        for range in &cell.changed {
            if range.start >= chars {
                continue;
            }
            let end = range.end.min(chars);
            self.renderer.draw_rect(
                text_x + range.start as f32 * self.cw,
                y,
                (end - range.start) as f32 * self.cw,
                self.line_height,
                rgb(word),
            );
        }
        let shown: String = cell.text.chars().take(chars).collect();
        self.renderer.draw_text(&shown, text_x, y, self.text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::scm_ui::DiffView;
    use crate::test_support::TempDir;

    fn view(left: &str, right: &str) -> DiffView {
        DiffView {
            review: None,
            title: "t".into(),
            left_name: "a".into(),
            right_name: "b".into(),
            left: left.into(),
            right: right.into(),
            inline: false,
            collapse_unchanged: false,
            selected_change: 0,
        }
    }

    #[test]
    fn header_hits_are_close_layout_or_nothing() {
        let area = (100.0, 30.0, 800.0, 500.0);
        let (close, layout) = diff_header_buttons(area, 20.0, 8.0, 10);
        assert!(close.0 + close.2 <= area.0 + area.2 && layout.0 + layout.2 <= close.0);
        let at = |r: Rect| diff_header_hit(area, 20.0, 8.0, 10, r.0 + 1.0, r.1 + 1.0);
        assert_eq!(at(close), DiffHeaderHit::Close);
        assert_eq!(at(layout), DiffHeaderHit::ToggleLayout);
        assert_eq!(
            diff_header_hit(area, 20.0, 8.0, 10, area.0 + 5.0, area.1 + 5.0),
            DiffHeaderHit::None
        );
    }

    #[test]
    fn the_area_leaves_the_menu_bar_activity_bar_and_status_bar_free() {
        let dir = TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (x, y, w, h) = app.diff_area(1200.0, 800.0, 20.0);
        assert!(x >= app.activity_bar_width_px());
        assert!(y >= app.top_menu_height());
        assert!(x + w <= 1200.0 && y + h <= 800.0 - 20.0 + 0.01);
    }

    #[test]
    fn clicks_inside_are_consumed_and_the_close_button_closes_the_view() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.diff_view = Some(view("a", "b"));
        let area = app.diff_area(1200.0, 800.0, 20.0);
        let (close, _) =
            diff_header_buttons(area, 20.0, 8.0, app.diff_layout_label().chars().count());
        assert!(app.click_diff_view(area.0 + 40.0, area.1 + 200.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(app.diff_view.is_some());
        assert!(app.click_diff_view(close.0 + 2.0, close.1 + 2.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(app.diff_view.is_none());
    }

    #[test]
    fn clicks_outside_the_area_are_not_consumed() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.diff_view = Some(view("a", "b"));
        // The menu bar (top) and the status bar (bottom) belong to the rest of the window.
        assert!(!app.click_diff_view(300.0, 2.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(!app.click_diff_view(300.0, 795.0, 1200.0, 800.0, 20.0, 8.0));
        assert!(app.diff_view.is_some());
    }

    #[test]
    fn without_a_diff_nothing_is_consumed() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(!app.click_diff_view(300.0, 300.0, 1200.0, 800.0, 20.0, 8.0));
    }

    fn numbered(count: usize, changed: &[usize]) -> (String, String) {
        let old: String = (1..=count).map(|n| format!("line {n}\n")).collect();
        let mut new = old.clone();
        for n in changed {
            new = new.replace(&format!("line {n}\n"), &format!("edited {n}\n"));
        }
        (old, new)
    }

    #[test]
    fn the_model_is_cached_until_the_texts_change() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.diff_view = Some(view("a\nb\n", "a\nc\n"));
        let first = app.diff_model();
        assert!(std::sync::Arc::ptr_eq(&first, &app.diff_model()));
        app.diff_view = Some(view("a\nb\n", "a\nd\n"));
        assert!(!std::sync::Arc::ptr_eq(&first, &app.diff_model()));
    }

    #[test]
    fn scrolling_stays_inside_the_rows() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (old, new) = numbered(40, &[20]);
        app.diff_view = Some(view(&old, &new));
        app.diff_visible_rows.set(10);
        app.scroll_diff(1000);
        assert_eq!(app.diff_scroll, 40 - 10);
        app.scroll_diff(-1000);
        assert_eq!(app.diff_scroll, 0);
    }

    #[test]
    fn next_and_previous_change_scroll_to_each_group() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (old, new) = numbered(60, &[10, 50]);
        app.diff_view = Some(view(&old, &new));
        app.diff_visible_rows.set(10);
        app.run_ui_command("workbench.action.compareEditor.nextChange");
        let first = app.diff_scroll;
        assert!(
            first > 0 && first <= 10,
            "first change at row 10, scroll {first}"
        );
        app.run_ui_command("workbench.action.compareEditor.nextChange");
        assert!(app.diff_scroll > first);
        app.run_ui_command("workbench.action.compareEditor.previousChange");
        assert_eq!(app.diff_scroll, first);
    }

    #[test]
    fn collapsing_unchanged_regions_shortens_the_scrollable_list() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (old, new) = numbered(100, &[50]);
        let mut shown = view(&old, &new);
        app.diff_view = Some(shown.clone());
        let full = app.diff_item_count();
        shown.collapse_unchanged = true;
        app.diff_view = Some(shown);
        assert!(app.diff_item_count() < full);
    }
}
