//! The centered quick-input box shared by the command palette, the file picker and the symbol
//! pickers: one layout, one renderer and one mouse handler, so every picker looks and behaves
//! the same (clickable rows, hover highlight, wheel scrolling).

use super::go_menu::GoPickerKind;
use super::*;

/// Most rows the box shows at once.
pub(crate) const MAX_ROWS: usize = 8;

/// Where the box and its parts sit, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct QuickInputLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Height of the input area above the rows.
    pub header: f32,
    pub row: f32,
    pub pad: f32,
}

impl QuickInputLayout {
    pub(crate) fn new(
        window_width: f32,
        top: f32,
        line_height: f32,
        scale: f32,
        rows: usize,
    ) -> Self {
        let scale = scale.max(0.5);
        let pad = 5.0 * scale;
        let width = (600.0 * scale).min((window_width - 16.0).max(0.0));
        let header = line_height * 1.5 + pad * 2.0;
        let row = line_height * 1.35;
        let rows = rows.min(MAX_ROWS);
        let list = if rows == 0 {
            0.0
        } else {
            rows as f32 * row + pad
        };
        Self {
            x: ((window_width - width) * 0.5).max(0.0),
            y: top + 2.0 * scale,
            width,
            height: header + list,
            header,
            row,
            pad,
        }
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// The visible row under a pointer position, if it is over the list.
    pub(crate) fn row_at(&self, x: f32, y: f32, rows: usize) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let offset = y - self.y - self.header;
        if offset < 0.0 {
            return None;
        }
        let index = (offset / self.row).floor() as usize;
        (index < rows.min(MAX_ROWS)).then_some(index)
    }
}

/// First visible item so that `selected` stays on screen.
pub(crate) fn window_start(selected: usize, count: usize) -> usize {
    selected
        .saturating_sub(MAX_ROWS - 1)
        .min(count.saturating_sub(MAX_ROWS))
}

pub(crate) struct QuickInputRow {
    pub title: String,
    /// Dimmed text aligned to the right: a shortcut, a source or a location.
    pub detail: String,
    pub enabled: bool,
    pub icon: Option<std::path::PathBuf>,
    /// The text screen readers announce for the row.
    pub accessible: String,
}

pub(crate) struct QuickInputModel {
    pub prefix: String,
    pub query: String,
    pub placeholder: String,
    pub masked: bool,
    pub rows: Vec<QuickInputRow>,
    /// Selected row among `rows`.
    pub selected: Option<usize>,
    /// Shown instead of the rows when there are none.
    pub message: Option<String>,
}

impl QuickInputModel {
    /// Rows the box must make room for.
    pub(crate) fn layout_rows(&self) -> usize {
        if self.rows.is_empty() {
            usize::from(self.message.is_some())
        } else {
            self.rows.len()
        }
    }
}

impl App {
    pub(crate) fn quick_input_open(&self) -> bool {
        self.command_palette.is_some() || self.go_picker.is_some() || self.quick_open.is_some()
    }

    /// The selected item and the number of items of the open picker.
    fn quick_input_state(&self) -> Option<(usize, usize)> {
        if let Some(palette) = &self.command_palette {
            let count = palette
                .items
                .iter()
                .filter(|item| command_palette::matches(item, &palette.query))
                .count();
            return Some((palette.selected, count));
        }
        if let Some(picker) = &self.go_picker {
            return Some((picker.selected, self.go_picker_rows().len()));
        }
        let quick = self.quick_open.as_ref()?;
        Some((quick.index, quick.paths.len()))
    }

    fn quick_input_set_selected(&mut self, index: usize) {
        if let Some(palette) = self.command_palette.as_mut() {
            palette.selected = index;
        } else if let Some(picker) = self.go_picker.as_mut() {
            picker.selected = index;
        } else if let Some(quick) = self.quick_open.as_mut() {
            quick.index = index;
        }
    }

    pub(crate) fn quick_input_model(&self) -> Option<QuickInputModel> {
        let preedit = self.editor_groups.active_view().ime_preedit.clone();
        if let Some(palette) = &self.command_palette {
            let matching: Vec<_> = palette
                .items
                .iter()
                .filter(|item| command_palette::matches(item, &palette.query))
                .collect();
            let start = window_start(palette.selected, matching.len());
            let rows: Vec<_> = matching
                .iter()
                .skip(start)
                .take(MAX_ROWS)
                .map(|item| QuickInputRow {
                    title: item.title.clone(),
                    detail: self
                        .keymap
                        .configured_shortcut(&item.id)
                        .or_else(|| (item.source != "Graphite").then(|| item.source.clone()))
                        .unwrap_or_default(),
                    enabled: item.enabled || item.id.starts_with("graphite.vsix:"),
                    icon: None,
                    accessible: item.title.clone(),
                })
                .collect();
            let (prefix, placeholder, masked) = match &palette.extension_prompt {
                Some((title, masked)) => (String::new(), title.clone(), *masked),
                None => (
                    ">".to_owned(),
                    self.t("quick_input.placeholder.commands").to_owned(),
                    false,
                ),
            };
            return Some(QuickInputModel {
                prefix,
                query: format!("{}{preedit}", palette.query),
                placeholder,
                masked,
                selected: palette
                    .selected
                    .checked_sub(start)
                    .filter(|i| *i < rows.len()),
                message: rows
                    .is_empty()
                    .then(|| self.t("quick_input.no_results").to_owned()),
                rows,
            });
        }
        if let Some(picker) = &self.go_picker {
            let items = self.go_picker_rows();
            let start = window_start(picker.selected, items.len());
            let rows: Vec<_> = items
                .iter()
                .skip(start)
                .take(MAX_ROWS)
                .map(|item| {
                    let location = format!(
                        "{}:{}",
                        self.tree.relative_path(&item.path).display(),
                        item.line + 1
                    );
                    QuickInputRow {
                        title: item.name.clone(),
                        accessible: format!("{}  {}", item.name, location),
                        detail: location,
                        enabled: true,
                        icon: None,
                    }
                })
                .collect();
            let prefix = match picker.kind {
                GoPickerKind::WorkspaceSymbols => "#",
                GoPickerKind::EditorSymbols => "@",
                GoPickerKind::LineColumn => ":",
                _ => "",
            };
            let prompt_key = match picker.kind {
                GoPickerKind::IconTheme => "icon_theme.prompt",
                _ => "go.symbol_prompt",
            };
            let message = if picker.kind == GoPickerKind::LineColumn {
                Some(self.t("go.line_column_hint").to_owned())
            } else if picker.loading {
                Some(self.t("go.loading_symbols").to_owned())
            } else if rows.is_empty() {
                Some(
                    picker
                        .message
                        .clone()
                        .unwrap_or_else(|| self.t("go.no_symbols").to_owned()),
                )
            } else {
                None
            };
            return Some(QuickInputModel {
                prefix: prefix.to_owned(),
                query: format!("{}{preedit}", picker.query),
                placeholder: self.t(prompt_key).trim_end_matches(':').to_owned(),
                masked: false,
                selected: picker
                    .selected
                    .checked_sub(start)
                    .filter(|i| *i < rows.len()),
                message,
                rows,
            });
        }
        let quick = self.quick_open.as_ref()?;
        let start = window_start(quick.index, quick.paths.len());
        let rows: Vec<_> = quick
            .paths
            .iter()
            .skip(start)
            .take(MAX_ROWS)
            .map(|path| {
                let relative = self.tree.relative_path(path);
                QuickInputRow {
                    title: path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| relative.display().to_string()),
                    detail: relative
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .map(|parent| parent.display().to_string())
                        .unwrap_or_default(),
                    enabled: true,
                    icon: Some(path.clone()),
                    accessible: relative.display().to_string(),
                }
            })
            .collect();
        Some(QuickInputModel {
            prefix: String::new(),
            query: format!("{}{preedit}", quick.query),
            placeholder: self.t("quick_input.placeholder.files").to_owned(),
            masked: false,
            selected: quick.index.checked_sub(start).filter(|i| *i < rows.len()),
            message: (rows.is_empty() && !quick.query.is_empty())
                .then(|| self.t("quick_input.no_results").to_owned()),
            rows,
        })
    }

    pub(crate) fn quick_input_layout(
        &self,
        window_width: f32,
        line_height: f32,
        rows: usize,
    ) -> QuickInputLayout {
        QuickInputLayout::new(
            window_width,
            self.top_menu_height(),
            line_height,
            self.sidebar_scale_factor,
            rows,
        )
    }

    fn quick_input_layout_now(
        &self,
        window_width: f32,
        line_height: f32,
    ) -> Option<QuickInputLayout> {
        let model = self.quick_input_model()?;
        Some(self.quick_input_layout(window_width, line_height, model.layout_rows()))
    }

    /// Runs the open picker's accept action on the selected item.
    pub(crate) fn quick_input_accept(&mut self) {
        if self.command_center.is_some() {
            self.command_center_accept();
        } else if self.command_palette.is_some() {
            self.execute_command_palette_selection();
        } else if self.go_picker.is_some() {
            self.accept_go_picker();
        } else if self.quick_open.is_some() {
            self.execute_quick_open_command("quickOpen.accept");
        }
    }

    /// Selects and accepts the visible row `visible`; false when no item sits there.
    pub(crate) fn quick_input_accept_row(&mut self, visible: usize) -> bool {
        let Some((selected, count)) = self.quick_input_state() else {
            return false;
        };
        let index = window_start(selected, count) + visible;
        if index >= count {
            return false;
        }
        self.quick_input_set_selected(index);
        self.quick_input_accept();
        true
    }

    /// Closes the open picker without running anything.
    pub(crate) fn quick_input_dismiss(&mut self) {
        if self.command_center.is_some() {
            self.close_command_center();
        } else if self.command_palette.is_some() {
            self.command_palette = None;
            self.cancel_extension_prompt();
        } else if self.go_picker.is_some() {
            self.go_picker = None;
        } else if self.quick_open.is_some() {
            self.execute_quick_open_command("quickOpen.close");
        }
    }

    /// A left click while a picker is open. Returns true when the click was inside the box and
    /// is fully handled; a click outside closes the picker and is left to the rest of the UI.
    pub(crate) fn quick_input_click(
        &mut self,
        x: f32,
        y: f32,
        window_width: f32,
        line_height: f32,
    ) -> bool {
        let Some(model) = self.quick_input_model() else {
            if self.command_center.is_some() {
                self.quick_input_dismiss();
            }
            return false;
        };
        let layout = self.quick_input_layout(window_width, line_height, model.layout_rows());
        if !layout.contains(x, y) {
            self.quick_input_dismiss();
            return false;
        }
        if let Some(row) = layout.row_at(x, y, model.rows.len()) {
            self.quick_input_accept_row(row);
        }
        true
    }

    /// Mouse wheel over the box moves the selection. Returns true when it consumed the event.
    pub(crate) fn quick_input_scroll(
        &mut self,
        lines: f32,
        pointer: (f32, f32),
        window_width: f32,
        line_height: f32,
    ) -> bool {
        let Some(layout) = self.quick_input_layout_now(window_width, line_height) else {
            return false;
        };
        if !layout.contains(pointer.0, pointer.1) {
            return false;
        }
        if let Some((selected, count)) = self.quick_input_state() {
            if count > 0 {
                let target = (selected as isize + lines.round() as isize)
                    .clamp(0, count as isize - 1) as usize;
                self.quick_input_set_selected(target);
            }
        }
        true
    }

    pub(crate) fn draw_quick_input(
        &self,
        r: &mut Renderer,
        window_width: f32,
        line_height: f32,
        char_width: f32,
    ) {
        let Some(model) = self.quick_input_model() else {
            return;
        };
        let lh = line_height;
        let layout = self.quick_input_layout(window_width, lh, model.layout_rows());
        let (x, y, width) = (layout.x, layout.y, layout.width);
        let pad = layout.pad;
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        r.draw_rect(
            x - 4.0,
            y,
            width + 8.0,
            layout.height + 6.0,
            [0.0, 0.0, 0.0, 0.18],
        );
        r.draw_rect(
            x - 2.0,
            y,
            width + 4.0,
            layout.height + 3.0,
            [0.0, 0.0, 0.0, 0.22],
        );
        r.draw_rect(
            x - 1.0,
            y - 1.0,
            width + 2.0,
            layout.height + 2.0,
            rgb(Theme::color(0x454545, 0xc8c8c8)),
        );
        r.draw_rect(
            x,
            y,
            width,
            layout.height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );

        // Input box.
        let (box_x, box_y, box_w, box_h) = (x + pad, y + pad, width - pad * 2.0, lh * 1.5);
        r.draw_rect(
            box_x - 1.0,
            box_y - 1.0,
            box_w + 2.0,
            box_h + 2.0,
            rgb(Theme::color(Theme::STATUS, 0x005a9e)),
        );
        r.draw_rect(
            box_x,
            box_y,
            box_w,
            box_h,
            rgb(Theme::color(0x3c3c3c, 0xffffff)),
        );
        let text_y = box_y + (box_h - lh) * 0.5;
        let text_x = box_x + pad;
        let shown = if model.masked {
            "•".repeat(model.query.chars().count())
        } else {
            model.query.clone()
        };
        let typed = format!("{}{}", model.prefix, shown);
        let typed_width = typed.chars().count() as f32 * char_width;
        let available = (box_w - pad * 2.0).max(char_width);
        if typed.is_empty() {
            r.draw_text(
                &Self::help_menu_visible_label(&model.placeholder, char_width, available),
                text_x + 2.0,
                text_y,
                dim,
            );
            r.draw_rect(text_x, text_y + lh * 0.08, 1.5, lh * 0.84, text);
        } else {
            r.draw_text(
                &Self::help_menu_visible_label(&typed, char_width, available),
                text_x,
                text_y,
                text,
            );
            if typed_width < available {
                r.draw_rect(
                    text_x + typed_width,
                    text_y + lh * 0.08,
                    1.5,
                    lh * 0.84,
                    text,
                );
            }
        }

        // Rows.
        let list_y = y + layout.header;
        if model.rows.is_empty() {
            if let Some(message) = &model.message {
                r.draw_text(
                    &Self::help_menu_visible_label(message, char_width, width - pad * 4.0),
                    x + pad * 2.0,
                    list_y + (layout.row - lh) * 0.5,
                    dim,
                );
            }
            return;
        }
        let hovered = layout.row_at(self.mouse.x as f32, self.mouse.y as f32, model.rows.len());
        let query = self.quick_input_highlight_query().to_lowercase();
        for (index, row) in model.rows.iter().enumerate() {
            let row_y = list_y + layout.row * index as f32;
            if Some(index) == model.selected {
                r.draw_rect(
                    x,
                    row_y,
                    width,
                    layout.row,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            } else if Some(index) == hovered {
                let [red, green, blue, _] = rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff));
                r.draw_rect(x, row_y, width, layout.row, [red, green, blue, 0.55]);
            }
            let item_y = row_y + (layout.row - lh) * 0.5;
            let mut left = x + pad * 2.0;
            if let Some(path) = &row.icon {
                let icon_size = (lh * 0.85).round();
                crate::icon_theme::draw(
                    r,
                    &crate::icon_theme::for_path(path),
                    left,
                    row_y + (layout.row - icon_size) * 0.5,
                    icon_size,
                    |color| color,
                );
                left += icon_size + pad;
            }
            let inner_right = x + width - pad * 2.0;
            let detail_chars = row.detail.chars().count();
            let detail_max = ((inner_right - left) * 0.45 / char_width).floor() as usize;
            let detail = if detail_chars > detail_max {
                Self::help_menu_visible_label(
                    &row.detail,
                    char_width,
                    detail_max as f32 * char_width,
                )
            } else {
                row.detail.clone()
            };
            let detail_width = detail.chars().count() as f32 * char_width;
            if !detail.is_empty() {
                r.draw_text(&detail, inner_right - detail_width, item_y, dim);
            }
            let title_room = (inner_right - left - detail_width - pad * 2.0).max(char_width);
            let title = Self::help_menu_visible_label(&row.title, char_width, title_room);
            let base = if row.enabled { text } else { dim };
            let (head, matched, tail) = split_match(&title, &query);
            let mut cursor = left;
            for (segment, color) in [
                (head, base),
                (
                    matched,
                    if row.enabled {
                        rgb(Theme::color(0x2aaaff, 0x0066bf))
                    } else {
                        dim
                    },
                ),
                (tail, base),
            ] {
                if segment.is_empty() {
                    continue;
                }
                r.draw_text(segment, cursor, item_y, color);
                cursor += segment.chars().count() as f32 * char_width;
            }
        }
    }

    /// The part of the typed text that matches against item titles.
    fn quick_input_highlight_query(&self) -> String {
        if let Some(palette) = &self.command_palette {
            palette.query.clone()
        } else if let Some(picker) = &self.go_picker {
            picker.query.clone()
        } else {
            self.quick_open
                .as_ref()
                .map(|quick| quick.query.clone())
                .unwrap_or_default()
        }
    }
}

/// Splits `title` around the first case-insensitive occurrence of `query`.
fn split_match<'a>(title: &'a str, query: &str) -> (&'a str, &'a str, &'a str) {
    if query.is_empty() {
        return (title, "", "");
    }
    let lower = title.to_lowercase();
    // Lowercasing can change byte lengths for a few scripts; only highlight when it did not.
    if lower.len() != title.len() {
        return (title, "", "");
    }
    match lower.find(query) {
        Some(start)
            if title.is_char_boundary(start) && title.is_char_boundary(start + query.len()) =>
        {
            (
                &title[..start],
                &title[start..start + query.len()],
                &title[start + query.len()..],
            )
        }
        _ => (title, "", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_centered_and_rows_map_to_pixels() {
        let layout = QuickInputLayout::new(1000.0, 30.0, 20.0, 1.0, 3);
        assert_eq!(layout.width, 600.0);
        assert_eq!(layout.x, 200.0);
        let first = layout.y + layout.header + 1.0;
        assert_eq!(layout.row_at(500.0, first, 3), Some(0));
        assert_eq!(layout.row_at(500.0, first + layout.row, 3), Some(1));
        assert_eq!(layout.row_at(500.0, first + layout.row * 3.0, 3), None);
        assert_eq!(layout.row_at(500.0, layout.y + 2.0, 3), None);
        assert_eq!(layout.row_at(100.0, first, 3), None);
    }

    #[test]
    fn layout_shrinks_to_narrow_windows_and_caps_rows() {
        let layout = QuickInputLayout::new(300.0, 0.0, 20.0, 1.0, 99);
        assert_eq!(layout.width, 284.0);
        let capped = QuickInputLayout::new(300.0, 0.0, 20.0, 1.0, MAX_ROWS);
        assert_eq!(layout.height, capped.height);
    }

    #[test]
    fn window_keeps_the_selection_visible() {
        assert_eq!(window_start(0, 3), 0);
        assert_eq!(window_start(5, 30), 0);
        assert_eq!(window_start(10, 30), 3);
        assert_eq!(window_start(29, 30), 22);
    }

    #[test]
    fn match_is_split_case_insensitively() {
        assert_eq!(split_match("Quick open", "open"), ("Quick ", "open", ""));
        assert_eq!(
            split_match("Quick open", "QUICK".to_lowercase().as_str()),
            ("", "Quick", " open")
        );
        assert_eq!(split_match("Quick open", "zzz"), ("Quick open", "", ""));
        assert_eq!(split_match("Quick open", ""), ("Quick open", "", ""));
    }

    fn app() -> (crate::test_support::TempDir, App) {
        let dir = crate::test_support::TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        (dir, app)
    }

    #[test]
    fn clicking_a_palette_row_runs_that_command() {
        let (dir, mut app) = app();
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.open_command_palette();
        app.command_palette.as_mut().unwrap().query = "graphite.security.trustWorkspace".into();
        let model = app.quick_input_model().unwrap();
        assert_eq!(model.rows.len(), 1);
        let layout = app.quick_input_layout(1000.0, 20.0, model.layout_rows());
        let y = layout.y + layout.header + layout.row * 0.5;
        assert!(app.quick_input_click(500.0, y, 1000.0, 20.0));
        assert!(app.command_palette.is_none());
        assert!(app
            .workspace_trust
            .as_ref()
            .unwrap()
            .is_trusted(app.tree.root()));
    }

    #[test]
    fn clicking_outside_closes_the_picker_and_is_not_consumed() {
        let (_dir, mut app) = app();
        app.open_command_palette();
        assert!(!app.quick_input_click(5.0, 700.0, 1000.0, 20.0));
        assert!(app.command_palette.is_none());
    }

    #[test]
    fn clicking_the_input_area_keeps_the_picker_open() {
        let (_dir, mut app) = app();
        app.open_command_palette();
        let layout = app.quick_input_layout(1000.0, 20.0, MAX_ROWS);
        assert!(app.quick_input_click(500.0, layout.y + 3.0, 1000.0, 20.0));
        assert!(app.command_palette.is_some());
    }

    #[test]
    fn clicking_a_file_row_opens_it() {
        let (dir, mut app) = app();
        std::fs::write(dir.path().join("hello.rs"), "fn main() {}\n").unwrap();
        app.start_quick_open();
        app.quick_open.as_mut().unwrap().paths = vec![dir.path().join("hello.rs")];
        let model = app.quick_input_model().unwrap();
        assert_eq!(model.rows[0].title, "hello.rs");
        let layout = app.quick_input_layout(1000.0, 20.0, model.layout_rows());
        let y = layout.y + layout.header + layout.row * 0.5;
        assert!(app.quick_input_click(500.0, y, 1000.0, 20.0));
        assert!(app.quick_open.is_none());
    }

    #[test]
    fn empty_results_show_a_message_instead_of_rows() {
        let (_dir, mut app) = app();
        app.open_command_palette();
        app.command_palette.as_mut().unwrap().query = "zzzz-no-such-command".into();
        let model = app.quick_input_model().unwrap();
        assert!(model.rows.is_empty());
        assert!(model.message.is_some());
        assert_eq!(model.layout_rows(), 1);
    }

    #[test]
    fn wheel_moves_the_selection_within_bounds() {
        let (_dir, mut app) = app();
        app.open_command_palette();
        let layout = app.quick_input_layout(1000.0, 20.0, MAX_ROWS);
        let pointer = (500.0, layout.y + layout.header + 5.0);
        assert!(app.quick_input_scroll(3.0, pointer, 1000.0, 20.0));
        assert_eq!(app.command_palette.as_ref().unwrap().selected, 3);
        assert!(app.quick_input_scroll(-10.0, pointer, 1000.0, 20.0));
        assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
        assert!(!app.quick_input_scroll(3.0, (5.0, 900.0), 1000.0, 20.0));
    }
}
