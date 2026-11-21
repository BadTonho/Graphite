use super::drawing::{
    fit, markdown_display_rows, plugin_name, plugin_publisher, push_lines, wrap_text,
};
use super::*;
use std::io::Read;

/// Maximum bytes read from a local plugin README file to keep UI opening instant.
const MAX_README_BYTES: u64 = 256 * 1024;

#[derive(Clone)]
pub(crate) struct InstalledPluginDetailTab {
    pub id: String,
    pub group: u64,
    pub metadata: Metadata,
    pub readme: Option<String>,
    pub scroll_line: usize,
}

fn read_plugin_readme(install_dir: &Path) -> Option<String> {
    const CANDIDATES: [&str; 6] = [
        "extension/README.md",
        "extension/readme.md",
        "extension/Readme.md",
        "README.md",
        "readme.md",
        "Readme.md",
    ];
    for candidate in CANDIDATES {
        let path = install_dir.join(candidate);
        if let Ok(file) = std::fs::File::open(&path) {
            let mut bytes = Vec::new();
            if file.take(MAX_README_BYTES).read_to_end(&mut bytes).is_ok() {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn installed_detail_tab_metrics(id: &str, line_height: f32, cell_width: f32) -> (String, f32) {
    let label = format!(" {} ", plugin_name(id));
    let width = (label.chars().count() as f32 * cell_width * 1.15 + cell_width * 4.2)
        .max(line_height * 4.5);
    (label, width)
}

fn hit_test_detail_tab(tab_x: f32, width: f32, click_x: f32, cell_width: f32) -> Option<bool> {
    if click_x < tab_x || click_x >= tab_x + width {
        return None;
    }
    let (close_left, close_right) =
        crate::app::group_drawing::tab_close_area(tab_x, width, cell_width);
    Some(click_x >= close_left && click_x < close_right)
}

impl App {
    /// Opens an installed plugin's details in an editor tab on the right, keeping the Plugins
    /// sidebar list visible.
    pub(crate) fn open_installed_plugin_details(&mut self, record: &Installed) {
        let id = record.metadata.id().to_owned();
        let readme = self
            .plugins
            .store
            .as_ref()
            .and_then(|store| read_plugin_readme(&record.install_dir(&store.root)));
        let existing = self
            .plugins
            .installed_details
            .iter_mut()
            .find(|tab| tab.id == id)
            .map(|tab| {
                tab.metadata = record.metadata.clone();
                if tab.readme.is_none() {
                    tab.readme = readme.clone();
                }
                tab.group
            });
        let group = existing.unwrap_or_else(|| self.editor_groups.focused());
        if existing.is_none() {
            self.plugins
                .installed_details
                .push(InstalledPluginDetailTab {
                    id: id.clone(),
                    group,
                    metadata: record.metadata.clone(),
                    readme,
                    scroll_line: 0,
                });
        }
        self.focus_editor_group(group);
        self.plugins.active_graphite_detail = None;
        self.plugins.active_installed_detail = Some((group, id));
        self.plugins.focused = false;
    }

    pub(crate) fn close_installed_plugin_detail(&mut self, group: u64, id: &str) {
        self.plugins
            .installed_details
            .retain(|tab| !(tab.group == group && tab.id == id));
        if self.plugins.active_installed_detail.as_ref() == Some(&(group, id.to_owned())) {
            self.plugins.active_installed_detail = None;
        }
    }

    pub(crate) fn close_installed_plugin_details_by_id(&mut self, id: &str) {
        self.plugins.installed_details.retain(|tab| tab.id != id);
        if self
            .plugins
            .active_installed_detail
            .as_ref()
            .is_some_and(|(_, active_id)| active_id == id)
        {
            self.plugins.active_installed_detail = None;
        }
    }

    pub(crate) fn draw_installed_detail_tabs(
        &self,
        renderer: &mut Renderer,
        group: u64,
        x: &mut f32,
        y: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        let active = self.plugins.active_installed_detail.as_ref();
        for tab in self
            .plugins
            .installed_details
            .iter()
            .filter(|tab| tab.group == group)
        {
            let (label, width) = installed_detail_tab_metrics(&tab.id, line_height, cell_width);
            let selected = active.is_some_and(|(active_group, active_id)| {
                *active_group == group && active_id == &tab.id
            });
            renderer.draw_rect(
                *x,
                y,
                width,
                line_height * 1.5,
                rgb(if selected {
                    Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
                } else {
                    Theme::color(Theme::SIDEBAR, 0xf3f3f3)
                }),
            );
            let (close_left, close_right) =
                crate::app::group_drawing::tab_close_area(*x, width, cell_width);
            let label_max = (((close_left - (*x + PAD)).max(cell_width)) / cell_width) as usize;
            renderer.draw_text(
                &fit(&label, label_max),
                *x + PAD,
                y + line_height * 0.25,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let hovered = self.mouse.x as f32 >= close_left
                && (self.mouse.x as f32) < close_right
                && self.mouse.y as f32 >= y
                && (self.mouse.y as f32) < y + line_height * 1.5;
            if hovered {
                renderer.draw_rect(
                    close_left + cell_width * 0.2,
                    y + line_height * 0.2,
                    (close_right - close_left - cell_width * 0.4).max(cell_width),
                    line_height * 1.1,
                    rgb(Theme::color(Theme::DIM, 0x5a5a5a)),
                );
            }
            renderer.draw_text(
                "×",
                close_left + cell_width * 0.6,
                y + line_height * 0.25,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            *x += width;
        }
    }

    pub(crate) fn draw_installed_detail_page(
        &self,
        renderer: &mut Renderer,
        rect: super::editor_groups::GroupRect,
        line_height: f32,
        cell_width: f32,
    ) {
        let Some((group, id)) = &self.plugins.active_installed_detail else {
            return;
        };
        if *group != rect.id {
            return;
        }
        let Some(tab) = self
            .plugins
            .installed_details
            .iter()
            .find(|tab| tab.group == *group && tab.id == *id)
        else {
            return;
        };
        let record = self
            .plugins
            .installed
            .iter()
            .find(|record| record.metadata.id() == tab.id);
        let metadata = record.map(|r| &r.metadata).unwrap_or(&tab.metadata);

        let body_y = rect.y + line_height * 1.5;
        let body_height = (rect.height - line_height * 1.5).max(0.0);
        let body_width = rect.width;
        renderer.draw_rect(
            rect.x,
            body_y,
            body_width,
            body_height,
            rgb(Theme::color(Theme::BG, 0xffffff)),
        );
        let sidebar_width = (body_width * 0.28)
            .clamp(220.0, 340.0)
            .min(body_width * 0.45);
        let sidebar_x = rect.x + body_width - sidebar_width;
        renderer.draw_rect(
            sidebar_x,
            body_y,
            sidebar_width,
            body_height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        renderer.draw_rect(
            sidebar_x,
            body_y,
            1.0,
            body_height,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );

        let text_color = Theme::color(Theme::TEXT, 0x1e1e1e);
        let dim_color = Theme::color(Theme::DIM, 0x616161);
        let error_color = 0x9b2226;
        let main_x = rect.x + PAD * 2.0;
        let main_width = (sidebar_x - main_x - PAD * 2.0).max(cell_width * 12.0);
        let max_main = (main_width / cell_width).max(1.0) as usize;
        let mut y = body_y + PAD;

        let title = self.tf(
            "plugins.sidebar.details_title",
            &[("id", &metadata.id()), ("version", &metadata.version())],
        );
        renderer.draw_text(&fit(&title, max_main), main_x, y, rgb(text_color));
        y += line_height * 1.4;

        let format_label = match metadata {
            Metadata::Vsix(_) => self.t("plugins.sidebar.badge_vsix"),
            Metadata::Native(_) => self.t("plugins.sidebar.badge_plugin"),
        };
        let state_label = match record {
            Some(r) if r.enabled => self.t("plugins.accessibility.enabled"),
            Some(_) => self.t("plugins.accessibility.disabled"),
            None => self.t("plugins.sidebar.badge_stopped"),
        };
        let subtitle = format!(
            "{} · {} · {}",
            plugin_publisher(metadata.id()),
            format_label,
            state_label
        );
        renderer.draw_text(&fit(&subtitle, max_main), main_x, y, rgb(dim_color));
        y += line_height * 1.3;

        let description = match metadata {
            Metadata::Native(manifest) => manifest.description.as_str(),
            Metadata::Vsix(vsix) => vsix.description.as_str(),
        };
        if !description.is_empty() {
            for line in wrap_text(description, max_main).into_iter().take(3) {
                renderer.draw_text(&line, main_x, y, rgb(text_color));
                y += line_height;
            }
            y += line_height * 0.5;
        }
        if let Some(message) = self.plugins.errors.get(metadata.id()) {
            for line in wrap_text(message, max_main).into_iter().take(3) {
                renderer.draw_text(&line, main_x, y, rgb(error_color));
                y += line_height;
            }
            y += line_height * 0.5;
        }

        let max_rows = ((body_y + body_height - y) / line_height).max(0.0) as usize;
        if let Some(readme) = &tab.readme {
            renderer.draw_text(self.t("plugins.sidebar.readme"), main_x, y, rgb(text_color));
            y += line_height * 1.35;
            let readme_rows = ((body_y + body_height - y) / line_height).max(0.0) as usize;
            let rows = markdown_display_rows(readme, max_main);
            let start = tab.scroll_line.min(rows.len());
            for (index, (line, color, _)) in rows.iter().skip(start).take(readme_rows).enumerate() {
                renderer.draw_text(line, main_x, y + index as f32 * line_height, rgb(*color));
            }
        } else {
            let mut lines: Vec<(String, u32)> = Vec::new();
            self.plugin_metadata_lines(metadata, max_main, &mut lines);
            let start = tab.scroll_line.min(lines.len());
            for (index, (line, color)) in lines.iter().skip(start).take(max_rows).enumerate() {
                renderer.draw_text(line, main_x, y + index as f32 * line_height, rgb(*color));
            }
        }

        // Right metadata sidebar
        let pad = PAD * 1.5;
        let busy = self.plugins.worker.is_some() || self.plugins.bisect.is_some();
        let (buttons, mut side_y) =
            installed_detail_buttons(sidebar_x, body_y, sidebar_width, line_height, record, busy);
        let max_side = ((sidebar_width - pad * 2.0) / cell_width).max(1.0) as usize;
        let max_btn_text = ((sidebar_width - pad * 4.0) / cell_width).max(1.0) as usize;
        for button in buttons {
            renderer.draw_rect(
                button.x,
                button.y,
                button.w,
                button.h,
                rgb(button.bg),
            );
            renderer.draw_text(
                &fit(self.t(button.label_key), max_btn_text),
                button.x + pad,
                button.y + (button.h - line_height) * 0.5,
                rgb(0xffffff),
            );
        }
        let mut side_lines: Vec<(String, u32)> = Vec::new();
        self.plugin_metadata_lines(metadata, max_side, &mut side_lines);
        if let Some(message) = self.plugins.errors.get(metadata.id()) {
            push_lines(&mut side_lines, message, max_side, error_color);
        }
        for (line, color) in side_lines {
            if side_y + line_height > body_y + body_height {
                break;
            }
            renderer.draw_text(&line, sidebar_x + pad, side_y, rgb(color));
            side_y += line_height;
        }
    }

    pub(crate) fn installed_detail_click(&mut self, x: f32, y: f32) -> bool {
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let line_height = gpu.renderer.line_height();
        let (window_width, window_height) = gpu.renderer.size();
        let Some((group, id)) = self.plugins.active_installed_detail.clone() else {
            return false;
        };
        let Some(rect) = self
            .group_rectangles(window_width, window_height, line_height)
            .into_iter()
            .find(|rect| rect.id == group)
        else {
            return false;
        };
        let body_y = rect.y + line_height * 1.5;
        if x < rect.x || x >= rect.x + rect.width || y < body_y || y >= rect.y + rect.height {
            return false;
        }
        let sidebar_width = (rect.width * 0.28)
            .clamp(220.0, 340.0)
            .min(rect.width * 0.45);
        let sidebar_x = rect.x + rect.width - sidebar_width;
        let record = self
            .plugins
            .installed
            .iter()
            .find(|record| record.metadata.id() == id);
        let busy = self.plugins.worker.is_some() || self.plugins.bisect.is_some();
        let (buttons, _) =
            installed_detail_buttons(sidebar_x, body_y, sidebar_width, line_height, record, busy);
        if let Some(button) = buttons.into_iter().find(|btn| {
            btn.enabled
                && x >= btn.x
                && x < btn.x + btn.w
                && y >= btn.y
                && y < btn.y + btn.h
        }) {
            self.installed_plugin_action_confirmed(&id, button.action);
        }
        true
    }

    pub(crate) fn installed_detail_tab_click(&mut self, group: u64, x: f32) -> bool {
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let cell_width = gpu.renderer.char_width();
        let line_height = gpu.renderer.line_height();
        let (window_width, window_height) = gpu.renderer.size();
        let mut tab_x = self
            .editor_groups
            .views(group)
            .map(|(views, _)| {
                let start_x = self
                    .group_rectangles(window_width, window_height, line_height)
                    .into_iter()
                    .find(|rect| rect.id == group)
                    .map(|rect| rect.x)
                    .unwrap_or(0.0);
                views
                    .iter()
                    .filter_map(|view| self.resolve_document(view.document))
                    .map(|document| {
                        crate::app::group_drawing::tab_width(
                            &self.documents[document],
                            cell_width,
                            &self.translator,
                        )
                    })
                    .sum::<f32>()
                    + start_x
            })
            .unwrap_or(0.0);
        tab_x += self.graphite_detail_tabs_width(group, line_height, cell_width);

        let tabs = self
            .plugins
            .installed_details
            .iter()
            .filter(|tab| tab.group == group)
            .map(|tab| {
                let (_, width) = installed_detail_tab_metrics(&tab.id, line_height, cell_width);
                (tab.id.clone(), width)
            })
            .collect::<Vec<_>>();
        for (id, width) in tabs {
            if let Some(is_close) = hit_test_detail_tab(tab_x, width, x, cell_width) {
                if is_close {
                    self.close_installed_plugin_detail(group, &id);
                } else {
                    self.plugins.active_graphite_detail = None;
                    self.plugins.active_installed_detail = Some((group, id));
                }
                return true;
            }
            tab_x += width;
        }
        false
    }

    pub(crate) fn installed_detail_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let Some((group, id)) = self.plugins.active_installed_detail.clone() else {
            return false;
        };
        if self.editor_groups.focused() != group {
            return false;
        }
        if self.modifiers.control_key() || self.modifiers.alt_key() {
            return false;
        }
        let Some(tab) = self
            .plugins
            .installed_details
            .iter_mut()
            .find(|tab| tab.group == group && tab.id == id)
        else {
            self.plugins.active_installed_detail = None;
            return false;
        };
        match event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.close_installed_plugin_detail(group, &id);
            }
            Key::Named(NamedKey::ArrowDown) => {
                tab.scroll_line = tab.scroll_line.saturating_add(1);
            }
            Key::Named(NamedKey::ArrowUp) => {
                tab.scroll_line = tab.scroll_line.saturating_sub(1);
            }
            Key::Named(NamedKey::PageDown) => {
                tab.scroll_line = tab.scroll_line.saturating_add(15);
            }
            Key::Named(NamedKey::PageUp) => {
                tab.scroll_line = tab.scroll_line.saturating_sub(15);
            }
            _ => {}
        }
        true
    }

    pub(crate) fn scroll_installed_detail(&mut self, lines: i32) -> bool {
        let Some((group, id)) = self.plugins.active_installed_detail.as_ref() else {
            return false;
        };
        if let Some(tab) = self
            .plugins
            .installed_details
            .iter_mut()
            .find(|tab| tab.group == *group && tab.id == *id)
        {
            tab.scroll_line = (tab.scroll_line as i32 + lines).max(0) as usize;
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DetailActionButton {
    action: char,
    label_key: &'static str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bg: u32,
    enabled: bool,
}

fn installed_detail_buttons(
    sidebar_x: f32,
    body_y: f32,
    sidebar_width: f32,
    line_height: f32,
    record: Option<&Installed>,
    busy: bool,
) -> (Vec<DetailActionButton>, f32) {
    let pad = PAD * 1.5;
    let w = (sidebar_width - pad * 2.0).max(0.0);
    let x = sidebar_x + pad;
    let mut y = body_y + PAD;
    let Some(record) = record else {
        return (Vec::new(), y);
    };
    let runnable = !matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_none());
    let is_vsix = matches!(record.metadata, Metadata::Vsix(_));
    let mut specs: Vec<(char, &'static str, u32, f32)> = Vec::new();
    if record.enabled {
        specs.push(('d', "plugins.action.disable", 0x4b5563, line_height * 1.7));
        if runnable {
            specs.push(('r', "plugins.action.restart", 0x3b4252, line_height * 1.5));
        }
    } else if runnable {
        let label_key = if record.trusted_version.as_deref() != Some(record.metadata.version()) {
            "plugins.action.trust_and_enable"
        } else {
            "plugins.action.enable"
        };
        specs.push(('e', label_key, 0x0e639c, line_height * 1.7));
    }
    if is_vsix {
        specs.push(('u', "plugins.action.update", 0x3b4252, line_height * 1.5));
    }
    specs.push(('x', "plugins.action.uninstall", 0x7f1d1d, line_height * 1.5));

    let mut buttons = Vec::with_capacity(specs.len());
    for (action, label_key, color, h) in specs {
        buttons.push(DetailActionButton {
            action,
            label_key,
            x,
            y,
            w,
            h,
            bg: if busy { 0x6e7681 } else { color },
            enabled: !busy,
        });
        y += h + line_height * 0.35;
    }
    if !buttons.is_empty() {
        y += line_height * 0.35;
    }
    (buttons, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_tab_close_hit_test_matches_drawn_close_area() {
        let (_, width) = installed_detail_tab_metrics("anthropic.claude-code", 20.0, 9.0);
        let tab_x = 120.0;
        let (close_left, close_right) =
            crate::app::group_drawing::tab_close_area(tab_x, width, 9.0);

        // Clicking on the label activates the tab (not close).
        assert_eq!(hit_test_detail_tab(tab_x, width, tab_x + 12.0, 9.0), Some(false));
        // Clicking anywhere on the drawn `×` close area closes the tab.
        assert_eq!(
            hit_test_detail_tab(tab_x, width, close_left + 0.6 * 9.0, 9.0),
            Some(true)
        );
        assert_eq!(
            hit_test_detail_tab(tab_x, width, close_right - 1.0, 9.0),
            Some(true)
        );
        // Clicking outside the tab returns None.
        assert_eq!(hit_test_detail_tab(tab_x, width, tab_x - 1.0, 9.0), None);
        assert_eq!(hit_test_detail_tab(tab_x, width, tab_x + width, 9.0), None);
    }

    #[test]
    fn detail_buttons_match_installed_plugin_state() {
        let disabled = Installed {
            directory: "google.google-antigravity".into(),
            enabled: false,
            trusted_version: None,
            dev_path: None,
            metadata: Metadata::Vsix(graphite_plugins::manifest::Vsix {
                id: "google.google-antigravity".into(),
                version: "1.13.0".into(),
                main: Some("./extension.js".into()),
                ..Default::default()
            }),
        };
        let (btns, next_y) =
            installed_detail_buttons(500.0, 40.0, 260.0, 20.0, Some(&disabled), false);
        assert_eq!(
            btns.iter().map(|b| (b.action, b.label_key)).collect::<Vec<_>>(),
            vec![
                ('e', "plugins.action.trust_and_enable"),
                ('u', "plugins.action.update"),
                ('x', "plugins.action.uninstall"),
            ]
        );
        assert!(next_y > btns.last().unwrap().y + btns.last().unwrap().h);

        let enabled = Installed {
            enabled: true,
            trusted_version: Some("1.13.0".into()),
            ..disabled
        };
        let (btns, _) = installed_detail_buttons(500.0, 40.0, 260.0, 20.0, Some(&enabled), false);
        assert_eq!(
            btns.iter().map(|b| (b.action, b.label_key)).collect::<Vec<_>>(),
            vec![
                ('d', "plugins.action.disable"),
                ('r', "plugins.action.restart"),
                ('u', "plugins.action.update"),
                ('x', "plugins.action.uninstall"),
            ]
        );
    }
}
