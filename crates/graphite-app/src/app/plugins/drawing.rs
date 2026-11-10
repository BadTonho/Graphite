use super::layout::{PluginsLayout, PluginsTab};
use super::*;

const AVATAR_COLORS: [u32; 6] = [0x007acc, 0x8b5cf6, 0x2ea043, 0xd97706, 0xdb2777, 0x0d9488];

struct PluginCard {
    title: String,
    subtitle: String,
    description: String,
    description_color: u32,
    badges: Vec<(String, u32)>,
    avatar: char,
    avatar_color: u32,
}

fn plugin_name(id: &str) -> &str {
    id.split('.').nth(1).unwrap_or(id)
}

fn plugin_publisher(id: &str) -> &str {
    id.split('.').next().unwrap_or("")
}

fn initial(name: &str) -> char {
    name.chars()
        .find(|character| character.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?')
}

fn avatar_color(id: &str) -> u32 {
    let mut hash = 0u32;
    for byte in id.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    AVATAR_COLORS[hash as usize % AVATAR_COLORS.len()]
}

fn fit(text: &str, max: usize) -> String {
    let clean: String = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let max = max.max(1);
    if clean.chars().count() <= max {
        return clean;
    }
    if max <= 3 {
        return clean.chars().take(max).collect();
    }
    let mut result: String = clean.chars().take(max - 3).collect();
    result.push_str("...");
    result
}

fn graphite_resource_label(label: &str, max: usize) -> String {
    fit(&format!("↗ {label}"), max)
}

fn wrap_text(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let clean: String = raw
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect();
        if clean.trim().is_empty() {
            continue;
        }
        let mut current = String::new();
        for word in clean.split_whitespace() {
            let mut word = word;
            while word.chars().count() > max {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                lines.push(word.chars().take(max).collect());
                let cut = word
                    .char_indices()
                    .nth(max)
                    .map(|(index, _)| index)
                    .unwrap_or(word.len());
                word = &word[cut..];
            }
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word.chars().count() <= max {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}

fn push_lines(lines: &mut Vec<(String, u32)>, text: &str, max: usize, color: u32) {
    for line in wrap_text(text, max) {
        lines.push((line, color));
    }
}

impl App {
    pub(crate) fn draw_graphite_detail_tabs(
        &self,
        renderer: &mut Renderer,
        group: u64,
        x: &mut f32,
        y: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        let active = self.plugins.active_graphite_detail.as_ref();
        for tab in self
            .plugins
            .graphite_details
            .iter()
            .filter(|tab| tab.group == group)
        {
            let name = tab.repository.rsplit('/').next().unwrap_or(&tab.repository);
            let label = format!(" {name}  × ");
            let width = (label.chars().count() as f32 * cell_width * 1.4 + cell_width * 2.4)
                .max(line_height * 4.5);
            let selected = active.is_some_and(|(active_group, repository)| {
                *active_group == group && repository == &tab.repository
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
            renderer.draw_text(
                &fit(&label, (width / cell_width) as usize),
                *x + PAD,
                y + line_height * 0.25,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            *x += width;
        }
    }

    pub(crate) fn draw_graphite_detail_page(
        &self,
        renderer: &mut Renderer,
        rect: super::editor_groups::GroupRect,
        line_height: f32,
        cell_width: f32,
    ) {
        let Some((group, repository)) = &self.plugins.active_graphite_detail else {
            return;
        };
        if *group != rect.id {
            return;
        }
        let Some(tab) = self
            .plugins
            .graphite_details
            .iter()
            .find(|tab| tab.group == *group && tab.repository == *repository)
        else {
            return;
        };
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
        let main_x = rect.x + PAD * 2.0;
        let main_width = (sidebar_x - main_x - PAD * 2.0).max(cell_width * 12.0);
        let max_main = (main_width / cell_width).max(1.0) as usize;
        let mut y = body_y + PAD;
        let title = tab
            .details
            .as_ref()
            .map(|details| details.repository.full_name.as_str())
            .unwrap_or(tab.repository.as_str());
        renderer.draw_text(title, main_x, y, rgb(text_color));
        y += line_height * 1.4;
        if let Some(details) = &tab.details {
            if let Some(description) = &details.repository.description {
                for line in wrap_text(description, max_main).into_iter().take(2) {
                    renderer.draw_text(&line, main_x, y, rgb(dim_color));
                    y += line_height;
                }
            }
            let downloads = details
                .release
                .as_ref()
                .map(|release| release.downloads)
                .unwrap_or(0);
            let stats = format!(
                "★ {}    ↓ {} {}",
                details.repository.stargazers_count,
                downloads,
                self.t("plugins.sidebar.downloads")
            );
            renderer.draw_text(&fit(&stats, max_main), main_x, y, rgb(dim_color));
            y += line_height * 1.7;
            renderer.draw_text(self.t("plugins.sidebar.readme"), main_x, y, rgb(text_color));
            y += line_height * 1.35;
            if details.readme_html.is_none() && tab.error.is_some() {
                renderer.draw_text(
                    self.t("plugins.sidebar.readme_render_error"),
                    main_x,
                    y,
                    rgb(0x9b2226),
                );
                y += line_height * 1.2;
            }
            let max_rows = ((body_y + body_height - y) / line_height).max(0.0) as usize;
            if details.readme_html.is_none() {
                let rows = markdown_display_rows(
                    details
                        .readme
                        .as_deref()
                        .unwrap_or_else(|| self.t("plugins.sidebar.no_readme")),
                    max_main,
                );
                let start = tab.scroll_line.min(rows.len());
                for (index, (line, color, _)) in rows.iter().skip(start).take(max_rows).enumerate()
                {
                    renderer.draw_text(line, main_x, y + index as f32 * line_height, rgb(*color));
                }
            }
        } else if tab.loading {
            renderer.draw_text(
                self.t("plugins.sidebar.loading_details"),
                main_x,
                y + line_height,
                rgb(dim_color),
            );
        } else if let Some(error) = &tab.error {
            for (index, line) in wrap_text(error, max_main).iter().take(4).enumerate() {
                renderer.draw_text(line, main_x, y + line_height * index as f32, rgb(0x9b2226));
            }
        }
        self.draw_graphite_detail_sidebar(
            renderer,
            tab,
            sidebar_x,
            body_y,
            sidebar_width,
            line_height,
            cell_width,
        );
    }

    fn draw_graphite_detail_sidebar(
        &self,
        renderer: &mut Renderer,
        tab: &GraphiteDetailTab,
        x: f32,
        body_y: f32,
        width: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        let pad = PAD * 1.5;
        let max = ((width - pad * 2.0) / cell_width).max(1.0) as usize;
        let text = Theme::color(Theme::TEXT, 0x1e1e1e);
        let dim = Theme::color(Theme::DIM, 0x616161);
        let button_y = body_y + PAD;
        let can_install = tab
            .details
            .as_ref()
            .and_then(|details| details.release.as_ref())
            .is_some_and(|release| !release.assets.is_empty());
        renderer.draw_rect(
            x + pad,
            button_y,
            width - pad * 2.0,
            line_height * 1.7,
            rgb(if can_install { 0x0e639c } else { 0x6e7681 }),
        );
        renderer.draw_text(
            self.t("plugins.sidebar.install"),
            x + pad * 2.0,
            button_y + line_height * 0.3,
            rgb(0xffffff),
        );
        let mut rows_y = button_y + line_height * 2.2;
        let Some(details) = &tab.details else {
            if let Some(error) = &tab.error {
                for line in wrap_text(error, max).iter().take(4) {
                    renderer.draw_text(line, x + pad, rows_y, rgb(0x9b2226));
                    rows_y += line_height;
                }
            }
            return;
        };
        if tab.choosing_asset {
            renderer.draw_text(
                self.t("plugins.sidebar.choose_package"),
                x + pad,
                rows_y,
                rgb(text),
            );
            rows_y += line_height * 1.2;
            if let Some(release) = &details.release {
                for (index, asset) in release.assets.iter().enumerate() {
                    let selected = index == tab.selected_asset;
                    if selected {
                        renderer.draw_rect(
                            x + pad,
                            rows_y,
                            width - pad * 2.0,
                            line_height,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                        );
                    }
                    let label = format!("{} · {} ↓", asset.name, asset.download_count);
                    renderer.draw_text(
                        &fit(&label, max),
                        x + pad,
                        rows_y,
                        rgb(if selected { text } else { dim }),
                    );
                    rows_y += line_height;
                }
            }
            renderer.draw_text(
                self.t("plugins.sidebar.package_enter_hint"),
                x + pad,
                rows_y + line_height * 0.4,
                rgb(dim),
            );
            return;
        }
        let Some(release) = &details.release else {
            renderer.draw_text(
                self.t("plugins.sidebar.no_compatible_release"),
                x + pad,
                rows_y,
                rgb(dim),
            );
            rows_y += line_height * 2.0;
            self.draw_graphite_resources(renderer, details, x, rows_y, line_height, max);
            return;
        };
        let heading = self.t("plugins.sidebar.installation");
        renderer.draw_text(heading, x + pad, rows_y, rgb(text));
        rows_y += line_height * 1.3;
        for line in [
            format!(
                "{}: {}",
                self.t("plugins.sidebar.release"),
                release.name.as_deref().unwrap_or(&release.tag_name)
            ),
            format!(
                "{}: {}",
                self.t("plugins.sidebar.published"),
                release
                    .published_at
                    .as_deref()
                    .unwrap_or(self.t("common.none"))
            ),
            format!(
                "{}: {} KB",
                self.t("plugins.sidebar.size"),
                release.size / 1024
            ),
            format!(
                "{}: {}",
                self.t("plugins.sidebar.downloads"),
                release.downloads
            ),
        ] {
            renderer.draw_text(&fit(&line, max), x + pad, rows_y, rgb(dim));
            rows_y += line_height * 1.15;
        }
        if release.assets.is_empty() {
            renderer.draw_text(
                &fit(self.t("plugins.sidebar.no_compatible_release"), max),
                x + pad,
                rows_y,
                rgb(0x9b2226),
            );
            rows_y += line_height * 1.5;
        }
        rows_y += line_height * 0.6;
        if !details.repository.topics.is_empty() {
            renderer.draw_text(
                self.t("plugins.sidebar.categories"),
                x + pad,
                rows_y,
                rgb(text),
            );
            rows_y += line_height * 1.2;
            for topic in details.repository.topics.iter().take(5) {
                renderer.draw_text(&fit(topic, max), x + pad, rows_y, rgb(dim));
                rows_y += line_height;
            }
        }
        self.draw_graphite_resources(
            renderer,
            details,
            x,
            rows_y + line_height * 0.5,
            line_height,
            max,
        );
    }

    fn draw_graphite_resources(
        &self,
        renderer: &mut Renderer,
        details: &RepositoryDetails,
        x: f32,
        mut y: f32,
        line_height: f32,
        max: usize,
    ) {
        let pad = PAD * 1.5;
        renderer.draw_text(
            self.t("plugins.sidebar.resources"),
            x + pad,
            y,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
        y += line_height * 1.25;
        let mut links = vec![(
            self.t("plugins.sidebar.repository").to_owned(),
            details.repository.html_url.clone(),
        )];
        if let Some(homepage) = details
            .repository
            .homepage
            .as_ref()
            .filter(|url| is_https_url(url))
        {
            links.push((
                self.t("plugins.sidebar.homepage").to_owned(),
                homepage.clone(),
            ));
        }
        links.push((
            self.t("plugins.sidebar.issues").to_owned(),
            format!(
                "{}/issues",
                details.repository.html_url.trim_end_matches('/')
            ),
        ));
        if let Some(license) = &details.repository.license {
            let name = license.spdx_id.as_deref().unwrap_or(&license.name);
            links.push((
                format!("{} · {name}", self.t("plugins.sidebar.license")),
                format!(
                    "{}/blob/HEAD/LICENSE",
                    details.repository.html_url.trim_end_matches('/')
                ),
            ));
        }
        for (label, _) in links {
            renderer.draw_text(
                &graphite_resource_label(&label, max),
                x + pad,
                y,
                rgb(0x007acc),
            );
            y += line_height * 1.1;
        }
    }

    pub(crate) fn graphite_detail_click(&mut self, x: f32, y: f32) -> bool {
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let line_height = gpu.renderer.line_height();
        let cell_width = gpu.renderer.char_width();
        let (window_width, window_height) = gpu.renderer.size();
        let Some((group, repository)) = self.plugins.active_graphite_detail.clone() else {
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
        let Some(tab) = self
            .plugins
            .graphite_details
            .iter()
            .find(|tab| tab.group == group && tab.repository == repository)
        else {
            return true;
        };
        if x >= sidebar_x {
            let button_y = body_y + PAD;
            if y >= button_y && y < button_y + line_height * 1.7 {
                self.start_graphite_detail_install();
                return true;
            }
            if tab.choosing_asset {
                let rows_y = button_y + line_height * 2.2 + line_height * 1.2;
                let asset_index = ((y - rows_y) / line_height).floor() as isize;
                if asset_index >= 0 {
                    if let Some(release) = tab
                        .details
                        .as_ref()
                        .and_then(|details| details.release.as_ref())
                    {
                        if (asset_index as usize) < release.assets.len() {
                            if let Some(active) = self
                                .plugins
                                .graphite_details
                                .iter_mut()
                                .find(|tab| tab.group == group && tab.repository == repository)
                            {
                                active.selected_asset = asset_index as usize;
                            }
                            self.install_selected_graphite_asset();
                        }
                    }
                }
                return true;
            }
            if let Some(details) = &tab.details {
                if let Some(url) = self.graphite_resource_url_at(
                    details,
                    y,
                    sidebar_x,
                    body_y,
                    sidebar_width,
                    line_height,
                ) {
                    open_external_https(&url);
                }
            }
            return true;
        }
        if let Some(details) = &tab.details {
            let sidebar_width = (rect.width * 0.28)
                .clamp(220.0, 340.0)
                .min(rect.width * 0.45);
            let main_x = rect.x + PAD * 2.0;
            let main_width =
                (rect.x + rect.width - sidebar_width - main_x - PAD * 2.0).max(cell_width * 12.0);
            let max_main = (main_width / cell_width).max(1.0) as usize;
            let description_rows = details
                .repository
                .description
                .as_deref()
                .map(|description| wrap_text(description, max_main).len().min(2))
                .unwrap_or(0);
            let readme_top = body_y + PAD + line_height * (4.45 + description_rows as f32);
            let display_row =
                ((y - readme_top) / line_height).floor().max(0.0) as usize + tab.scroll_line;
            let rows =
                markdown_display_rows(details.readme.as_deref().unwrap_or_default(), max_main);
            let Some((_, _, source_line)) = rows.get(display_row) else {
                return true;
            };
            if let Some(url) = markdown_url_at(
                details.readme.as_deref().unwrap_or_default(),
                *source_line,
                &details.repository,
            ) {
                open_external_https(&url);
            }
        }
        true
    }

    pub(crate) fn graphite_detail_tab_click(&mut self, group: u64, x: f32) -> bool {
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let cell_width = gpu.renderer.char_width();
        let line_height = gpu.renderer.line_height();
        let window_height = gpu.renderer.size().1;
        let mut tab_x = self
            .editor_groups
            .views(group)
            .map(|(views, _)| {
                let x = self
                    .group_rectangles(gpu.renderer.size().0, window_height, line_height)
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
                    + x
            })
            .unwrap_or(0.0);
        let tabs = self
            .plugins
            .graphite_details
            .iter()
            .filter(|tab| tab.group == group)
            .map(|tab| {
                let label = format!(
                    " {}  × ",
                    tab.repository.rsplit('/').next().unwrap_or(&tab.repository)
                );
                let width = (label.chars().count() as f32 * cell_width * 1.4 + cell_width * 2.4)
                    .max(line_height * 4.5);
                (tab.repository.clone(), width)
            })
            .collect::<Vec<_>>();
        for (repository, width) in tabs {
            if x >= tab_x && x < tab_x + width {
                if x >= tab_x + width - cell_width * 2.0 {
                    self.close_graphite_detail(group, &repository);
                } else {
                    self.plugins.active_graphite_detail = Some((group, repository));
                }
                return true;
            }
            tab_x += width;
        }
        false
    }

    pub(crate) fn graphite_detail_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let Some((group, repository)) = self.plugins.active_graphite_detail.clone() else {
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
            .graphite_details
            .iter_mut()
            .find(|tab| tab.group == group && tab.repository == repository)
        else {
            self.plugins.active_graphite_detail = None;
            return false;
        };
        match event.logical_key {
            Key::Named(NamedKey::Escape) => {
                if tab.choosing_asset {
                    tab.choosing_asset = false;
                } else {
                    self.close_graphite_detail(group, &repository);
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if tab.choosing_asset {
                    let count = tab
                        .details
                        .as_ref()
                        .and_then(|details| details.release.as_ref())
                        .map_or(0, |release| release.assets.len());
                    tab.selected_asset = (tab.selected_asset + 1).min(count.saturating_sub(1));
                } else {
                    tab.scroll_line = tab.scroll_line.saturating_add(1);
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if tab.choosing_asset {
                    tab.selected_asset = tab.selected_asset.saturating_sub(1);
                } else {
                    tab.scroll_line = tab.scroll_line.saturating_sub(1);
                }
            }
            Key::Named(NamedKey::Enter) => {
                if tab.choosing_asset {
                    self.install_selected_graphite_asset();
                } else {
                    self.start_graphite_detail_install();
                }
            }
            _ => {}
        }
        true
    }

    pub(crate) fn scroll_graphite_detail(&mut self, lines: i32) -> bool {
        let Some((group, repository)) = self.plugins.active_graphite_detail.as_ref() else {
            return false;
        };
        if let Some(tab) = self
            .plugins
            .graphite_details
            .iter_mut()
            .find(|tab| tab.group == *group && tab.repository == *repository)
        {
            if tab.choosing_asset {
                let count = tab
                    .details
                    .as_ref()
                    .and_then(|details| details.release.as_ref())
                    .map_or(0, |release| release.assets.len());
                tab.selected_asset = (tab.selected_asset as i32 + lines)
                    .clamp(0, count.saturating_sub(1) as i32)
                    as usize;
            } else {
                tab.scroll_line = (tab.scroll_line as i32 + lines).max(0) as usize;
            }
        }
        true
    }

    pub(crate) fn sync_graphite_detail_webview(&mut self) {
        let Some((group, repository)) = self.plugins.active_graphite_detail.clone() else {
            self.graphite_detail_webview = None;
            self.graphite_detail_webview_target = None;
            self.graphite_detail_webview_html = None;
            self.graphite_detail_webview_bounds = None;
            return;
        };
        let Some(details) = self
            .plugins
            .graphite_details
            .iter()
            .find(|tab| tab.group == group && tab.repository == repository)
            .and_then(|tab| tab.details.as_ref())
        else {
            self.graphite_detail_webview = None;
            self.graphite_detail_webview_target = None;
            self.graphite_detail_webview_html = None;
            self.graphite_detail_webview_bounds = None;
            return;
        };
        let Some(rendered_html) = details.readme_html.as_deref() else {
            self.graphite_detail_webview = None;
            self.graphite_detail_webview_target = None;
            self.graphite_detail_webview_html = None;
            self.graphite_detail_webview_bounds = None;
            return;
        };
        let html = crate::previews::github_readme_document(rendered_html);
        let Some((window, width, height, line_height, cell_width)) = self.gpu.as_ref().map(|gpu| {
            let (width, height) = gpu.renderer.size();
            (
                gpu.window.clone(),
                width,
                height,
                gpu.renderer.line_height(),
                gpu.renderer.char_width(),
            )
        }) else {
            return;
        };
        let Some(rect) = self
            .group_rectangles(width, height, line_height)
            .into_iter()
            .find(|rect| rect.id == group)
        else {
            return;
        };
        let body_y = rect.y + line_height * 1.5;
        let sidebar_width = (rect.width * 0.28)
            .clamp(220.0, 340.0)
            .min(rect.width * 0.45);
        let main_x = rect.x + PAD * 2.0;
        let main_width = (rect.width - sidebar_width - PAD * 4.0).max(cell_width * 12.0);
        let max_main = (main_width / cell_width).max(1.0) as usize;
        let description_rows = details
            .repository
            .description
            .as_deref()
            .map(|description| wrap_text(description, max_main).len().min(2))
            .unwrap_or(0);
        let readme_y = body_y + PAD + line_height * (4.45 + description_rows as f32);
        let bounds = [
            f64::from(main_x),
            f64::from(readme_y),
            f64::from(main_width),
            f64::from((rect.y + rect.height - readme_y).max(0.0)),
        ];
        let target = (group, repository.clone());
        if self.graphite_detail_webview_target.as_ref() == Some(&target)
            && self.graphite_detail_webview_html.as_deref() == Some(html.as_str())
        {
            if self.graphite_detail_webview_bounds != Some(bounds) || cfg!(target_os = "linux") {
                if let Some(view) = &self.graphite_detail_webview {
                    let _ = view.set_overlay_bounds(&window, bounds);
                    self.graphite_detail_webview_bounds = Some(bounds);
                }
            }
            return;
        }
        match crate::webview::WebviewHost::open_overlay(&window, &html, bounds) {
            Ok(view) => {
                self.graphite_detail_webview = Some(view);
                self.graphite_detail_webview_target = Some(target);
                self.graphite_detail_webview_html = Some(html);
                self.graphite_detail_webview_bounds = Some(bounds);
            }
            Err(error) => {
                self.graphite_detail_webview = None;
                self.graphite_detail_webview_target = None;
                self.graphite_detail_webview_html = None;
                self.graphite_detail_webview_bounds = None;
                let message = self.t("plugins.sidebar.readme_render_error").to_owned();
                if let Some(tab) = self
                    .plugins
                    .graphite_details
                    .iter_mut()
                    .find(|tab| tab.group == group && tab.repository == repository)
                {
                    if let Some(details) = &mut tab.details {
                        details.readme_html = None;
                    }
                    tab.error = Some(format!("{message} ({error})"));
                }
            }
        }
    }

    fn graphite_resource_url_at(
        &self,
        details: &RepositoryDetails,
        y: f32,
        sidebar_x: f32,
        body_y: f32,
        sidebar_width: f32,
        line_height: f32,
    ) -> Option<String> {
        let mut rows_y = body_y + PAD + line_height * 2.2;
        if details.release.is_some() {
            rows_y += line_height * 1.3 + line_height * 4.6 + line_height * 0.6;
            if details
                .release
                .as_ref()
                .is_some_and(|release| release.assets.is_empty())
            {
                rows_y += line_height * 1.5;
            }
            if !details.repository.topics.is_empty() {
                rows_y += line_height * (1.2 + details.repository.topics.len().min(5) as f32);
            }
            rows_y += line_height * 0.5;
        } else {
            rows_y += line_height * 2.0;
        }
        rows_y += line_height * 1.25;
        let row = ((y - rows_y) / (line_height * 1.1)).floor() as isize;
        if row < 0 || y < rows_y || sidebar_x + sidebar_width < 0.0 {
            return None;
        }
        let mut links = vec![details.repository.html_url.clone()];
        if let Some(homepage) = details
            .repository
            .homepage
            .as_ref()
            .filter(|url| is_https_url(url))
        {
            links.push(homepage.clone());
        }
        links.push(format!(
            "{}/issues",
            details.repository.html_url.trim_end_matches('/')
        ));
        if details.repository.license.is_some() {
            links.push(format!(
                "{}/blob/HEAD/LICENSE",
                details.repository.html_url.trim_end_matches('/')
            ));
        }
        links.get(row as usize).cloned()
    }

    pub(crate) fn draw_plugins_sidebar(
        &self,
        renderer: &mut Renderer,
        height: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        if !self.plugins.visible || self.zen_mode {
            return;
        }
        let layout = PluginsLayout::new(height, line_height);
        let x = layout.sidebar_x;
        let width = layout.width;
        renderer.set_clip(Some([x, 0.0, x + width, height - line_height]));
        renderer.draw_rect(
            x,
            0.0,
            width,
            height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        renderer.draw_text(
            self.t("plugins.sidebar.title"),
            x + PAD,
            layout.title_y,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        self.draw_plugin_tabs(renderer, &layout, line_height);
        self.draw_plugin_search(renderer, &layout, line_height, cell_width);

        let (section, empty_key) = self.plugin_section();
        renderer.draw_text(
            &section,
            x + PAD,
            layout.section_y,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        if !self.plugins.message.is_empty() {
            let color = if self.plugins.worker.is_some() {
                Theme::color(Theme::DIM, 0x616161)
            } else {
                0x9b2226
            };
            let max = ((width - PAD * 2.0) / cell_width).max(1.0) as usize;
            renderer.draw_text(
                &fit(&self.plugins.message, max),
                x + PAD,
                layout.message_y,
                rgb(color),
            );
        }
        if self.plugins.preview.is_some()
            || self.plugins.trust.is_some()
            || self.plugins.inspecting.is_some()
        {
            self.draw_plugin_details(renderer, &layout, line_height, cell_width);
        } else {
            self.draw_plugin_cards(renderer, &layout, line_height, cell_width, empty_key);
        }
        self.draw_plugin_footer(renderer, &layout, line_height, cell_width);
        renderer.set_clip(None);
    }

    fn draw_plugin_tabs(&self, renderer: &mut Renderer, layout: &PluginsLayout, line_height: f32) {
        for tab in [
            PluginsTab::Installed,
            PluginsTab::OpenVsx,
            PluginsTab::Graphite,
        ]
        .into_iter()
        {
            let (x, tab_width) = layout.tab_bounds(tab);
            let label = match tab {
                PluginsTab::Installed => self.t("plugins.sidebar.installed"),
                PluginsTab::OpenVsx => self.t("plugins.sidebar.open_vsx"),
                PluginsTab::Graphite => self.t("plugins.sidebar.graphite"),
            };
            let color = if tab == self.plugins.tab {
                Theme::color(Theme::TEXT, 0x1e1e1e)
            } else {
                Theme::color(Theme::DIM, 0x616161)
            };
            renderer.draw_text(
                label,
                x + PAD,
                layout.tabs_y + (layout.tab_height - line_height) * 0.5,
                rgb(color),
            );
            if tab == self.plugins.tab {
                renderer.draw_rect(
                    x + PAD,
                    layout.tabs_y + layout.tab_height - 2.0,
                    tab_width - PAD * 2.0,
                    2.0,
                    rgb(Theme::color(Theme::STATUS, 0x005a9e)),
                );
            }
        }
        renderer.draw_rect(
            layout.sidebar_x,
            layout.tabs_y + layout.tab_height,
            layout.width,
            1.0,
            rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
        );
    }

    fn draw_plugin_search(
        &self,
        renderer: &mut Renderer,
        layout: &PluginsLayout,
        line_height: f32,
        cell_width: f32,
    ) {
        let x = layout.sidebar_x + PAD;
        let y = layout.search_y;
        let width = layout.width - PAD * 2.0;
        let height = layout.search_height;
        renderer.draw_rect(x, y, width, height, rgb(Theme::color(Theme::BG, 0xffffff)));
        let border = if self.plugins.editing {
            Theme::color(Theme::STATUS, 0x005a9e)
        } else {
            Theme::color(Theme::DIM, 0x616161)
        };
        renderer.draw_rect(x, y, width, 1.0, rgb(border));
        renderer.draw_rect(x, y + height - 1.0, width, 1.0, rgb(border));
        renderer.draw_rect(x, y, 1.0, height, rgb(border));
        renderer.draw_rect(x + width - 1.0, y, 1.0, height, rgb(border));
        let icon_size = (line_height * 0.8).round();
        renderer.draw_icon(
            super::super::activity_bar::Activity::Search.glyph(),
            x + PAD * 0.75,
            y + (height - icon_size) * 0.5,
            icon_size,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        let text_x = x + PAD * 0.75 + icon_size + PAD * 0.75;
        let max = (((x + width - PAD * 0.75) - text_x) / cell_width).max(1.0) as usize;
        let placeholder = if self.plugins.path_input {
            self.t("plugins.sidebar.search_path")
        } else {
            self.t(match self.plugins.tab {
                PluginsTab::Installed => "plugins.sidebar.search_open_vsx",
                PluginsTab::OpenVsx => "plugins.sidebar.search_open_vsx",
                PluginsTab::Graphite => "plugins.sidebar.search_graphite",
            })
        };
        let (label, color) = if self.plugins.editing {
            (
                format!("{}_", self.plugins.query),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else if !self.plugins.query.is_empty()
            && (self.plugins.tab != PluginsTab::Installed || self.plugins.path_input)
        {
            (
                self.plugins.query.clone(),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else {
            (placeholder.to_string(), Theme::color(Theme::DIM, 0x616161))
        };
        renderer.draw_text(
            &fit(&label, max),
            text_x,
            y + (height - line_height) * 0.5,
            rgb(color),
        );
    }

    fn plugin_section(&self) -> (String, &'static str) {
        match self.plugins.tab {
            PluginsTab::Installed => (
                self.tf(
                    "plugins.sidebar.section_installed",
                    &[("count", &self.plugins.installed.len())],
                ),
                "plugins.sidebar.empty_installed",
            ),
            PluginsTab::OpenVsx => (
                self.tf(
                    "plugins.sidebar.section_open_vsx",
                    &[("count", &self.plugins.open_vsx.len())],
                ),
                "plugins.sidebar.empty_open_vsx",
            ),
            PluginsTab::Graphite => {
                if let Some((_, assets)) = &self.plugins.graphite_assets {
                    (
                        self.tf(
                            "plugins.sidebar.section_assets",
                            &[("count", &assets.len())],
                        ),
                        "plugins.sidebar.empty_assets",
                    )
                } else {
                    (
                        self.tf(
                            "plugins.sidebar.section_graphite",
                            &[("count", &self.plugins.graphite.len())],
                        ),
                        "plugins.sidebar.empty_graphite",
                    )
                }
            }
        }
    }

    fn draw_plugin_cards(
        &self,
        renderer: &mut Renderer,
        layout: &PluginsLayout,
        line_height: f32,
        cell_width: f32,
        empty_key: &'static str,
    ) {
        let count = self.plugin_row_count();
        let max = ((layout.width - PAD * 2.0) / cell_width).max(1.0) as usize;
        if count == 0 {
            if self.plugins.worker.is_none() && self.plugins.message.is_empty() {
                renderer.draw_text(
                    &fit(self.t(empty_key), max),
                    layout.sidebar_x + PAD,
                    layout.list_y,
                    rgb(Theme::color(Theme::DIM, 0x616161)),
                );
            }
            return;
        }
        let rows = layout.visible_rows();
        let offset = self.plugins.scroll.min(count.saturating_sub(rows));
        let hovered = layout.card_index(self.mouse.x as f32, self.mouse.y as f32, offset, count);
        for index in offset..(offset + rows).min(count) {
            let row = index - offset;
            let y = layout.list_y + row as f32 * layout.card_height;
            if y + layout.card_height > layout.list_bottom + 0.5 {
                break;
            }
            self.draw_plugin_card(
                renderer,
                layout,
                line_height,
                cell_width,
                index,
                y,
                index == self.plugins.index,
                hovered == Some(index),
            );
        }
    }

    fn draw_plugin_card(
        &self,
        renderer: &mut Renderer,
        layout: &PluginsLayout,
        line_height: f32,
        cell_width: f32,
        index: usize,
        y: f32,
        selected: bool,
        hovered: bool,
    ) {
        let Some(card) = self.plugin_card_data(index) else {
            return;
        };
        let x = layout.sidebar_x;
        let width = layout.width;
        let active_row = Theme::color(Theme::ACTIVE_ROW, 0xd6ebff);
        if selected || hovered {
            renderer.draw_rect(x, y, width, layout.card_height, rgb(active_row));
        }
        if selected {
            renderer.draw_rect(
                x,
                y,
                2.0,
                layout.card_height,
                rgb(Theme::color(Theme::STATUS, 0x005a9e)),
            );
        } else if !hovered {
            renderer.draw_rect(
                x + PAD,
                y + layout.card_height - 1.0,
                width - PAD * 2.0,
                1.0,
                rgb(active_row),
            );
        }
        let tile = (line_height * 1.6).round();
        let tile_x = x + PAD;
        let tile_y = y + line_height * 0.3;
        renderer.draw_rect(tile_x, tile_y, tile, tile, rgb(card.avatar_color));
        let letter = card.avatar.to_string();
        renderer.draw_text(
            &letter,
            tile_x + (tile - cell_width) * 0.5,
            tile_y + (tile - line_height) * 0.5,
            rgb(0xffffff),
        );
        let text_x = tile_x + tile + PAD;
        let text_max = (((x + width - PAD) - text_x) / cell_width).max(1.0) as usize;
        renderer.draw_text(
            &fit(&card.title, text_max),
            text_x,
            y + line_height * 0.3,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
        renderer.draw_text(
            &fit(&card.subtitle, text_max),
            text_x,
            y + line_height * 1.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        let description_x = x + PAD;
        let description_max = ((width - PAD * 2.0) / cell_width).max(1.0) as usize;
        let description_y = tile_y + tile + line_height * 0.15;
        for (line_index, line) in wrap_text(&card.description, description_max)
            .iter()
            .take(2)
            .enumerate()
        {
            renderer.draw_text(
                line,
                description_x,
                description_y + line_index as f32 * line_height,
                rgb(card.description_color),
            );
        }
        let badge_y = y + layout.card_height - line_height * 1.15;
        let badge_height = line_height * 1.05;
        let mut badge_x = description_x;
        for (label, color) in &card.badges {
            let max = (((x + width - PAD) - badge_x) / cell_width).max(1.0) as usize;
            let label = fit(label, max);
            if label.is_empty() {
                break;
            }
            let badge_width = label.chars().count() as f32 * cell_width + PAD * 1.4;
            if badge_x + badge_width > x + width - PAD {
                break;
            }
            renderer.draw_rect(badge_x, badge_y, badge_width, badge_height, rgb(*color));
            renderer.draw_text(
                &label,
                badge_x + PAD * 0.7,
                badge_y + (badge_height - line_height) * 0.5,
                rgb(0xffffff),
            );
            badge_x += badge_width + PAD * 0.5;
        }
    }

    fn plugin_card_data(&self, index: usize) -> Option<PluginCard> {
        let dim = Theme::color(Theme::DIM, 0x616161);
        let error = 0x9b2226;
        if self.plugins.tab == PluginsTab::OpenVsx {
            let extension = self.plugins.open_vsx.get(index)?;
            let id = format!("{}.{}", extension.namespace, extension.name);
            return Some(PluginCard {
                title: extension.display_name.clone(),
                subtitle: format!("{} · v{}", extension.namespace, extension.version),
                description: extension.description.clone(),
                description_color: dim,
                badges: Vec::new(),
                avatar: initial(&extension.name),
                avatar_color: avatar_color(&id),
            });
        } else if self.plugins.tab == PluginsTab::Graphite {
            if let Some((repository, assets)) = &self.plugins.graphite_assets {
                let asset = assets.get(index)?;
                return Some(PluginCard {
                    title: asset.name.clone(),
                    subtitle: self.tf("plugins.bytes", &[("bytes", &asset.size)]),
                    description: repository.full_name.clone(),
                    description_color: dim,
                    badges: Vec::new(),
                    avatar: initial(&asset.name),
                    avatar_color: avatar_color(&repository.full_name),
                });
            }
            let repository = self.plugins.graphite.get(index)?;
            return Some(PluginCard {
                title: repository.full_name.clone(),
                subtitle: self.tf(
                    "plugins.sidebar.stars",
                    &[("stars", &repository.stargazers_count)],
                ),
                description: repository.description.clone().unwrap_or_default(),
                description_color: dim,
                badges: Vec::new(),
                avatar: initial(
                    repository
                        .full_name
                        .rsplit('/')
                        .next()
                        .unwrap_or(&repository.full_name),
                ),
                avatar_color: avatar_color(&repository.full_name),
            });
        }
        let record = self.plugins.installed.get(index)?;
        let id = record.metadata.id();
        let (status, status_color) = match &record.metadata {
            Metadata::Vsix(_) if record.enabled => {
                (self.t("plugins.sidebar.badge_vsix").to_string(), 0x6f42c1)
            }
            Metadata::Vsix(_) => (
                self.t("plugins.sidebar.badge_disabled").to_string(),
                0x6e7681,
            ),
            Metadata::Native(_) if !record.enabled => (
                self.t("plugins.sidebar.badge_disabled").to_string(),
                0x6e7681,
            ),
            Metadata::Native(_) => match self
                .plugins
                .hosts
                .get(id)
                .and_then(|host| host.supervisor.as_ref())
                .map(|supervisor| supervisor.state)
            {
                Some(graphite_extension_host::HostState::Ready)
                    if self
                        .plugins
                        .hosts
                        .get(id)
                        .is_some_and(|host| host.native_active) =>
                {
                    (
                        self.t("plugins.sidebar.badge_running").to_string(),
                        0x1a7f37,
                    )
                }
                Some(graphite_extension_host::HostState::Ready) => (
                    self.t("plugins.sidebar.badge_activating").to_string(),
                    0x007acc,
                ),
                Some(graphite_extension_host::HostState::Starting) => (
                    self.t("plugins.sidebar.badge_starting").to_string(),
                    0x007acc,
                ),
                _ => (
                    self.t("plugins.sidebar.badge_stopped").to_string(),
                    0x6e7681,
                ),
            },
        };
        let mut badges = vec![(status, status_color)];
        let mut description = match &record.metadata {
            Metadata::Native(manifest) => manifest.description.clone(),
            Metadata::Vsix(vsix) => vsix.description.clone(),
        };
        let mut description_color = dim;
        if let Some(message) = self.plugins.errors.get(id) {
            badges.push((self.t("plugins.sidebar.badge_error").to_string(), error));
            description = message.clone();
            description_color = error;
        }
        let version = self.tf(
            "plugins.sidebar.version",
            &[("version", &record.metadata.version())],
        );
        Some(PluginCard {
            title: plugin_name(id).to_string(),
            subtitle: format!("{} · {version}", plugin_publisher(id)),
            description,
            description_color,
            badges,
            avatar: initial(plugin_name(id)),
            avatar_color: avatar_color(id),
        })
    }

    fn draw_plugin_details(
        &self,
        renderer: &mut Renderer,
        layout: &PluginsLayout,
        line_height: f32,
        cell_width: f32,
    ) {
        let max = ((layout.width - PAD * 2.0) / cell_width).max(1.0) as usize;
        let text = Theme::color(Theme::TEXT, 0x1e1e1e);
        let dim = Theme::color(Theme::DIM, 0x616161);
        let error = 0x9b2226;
        let mut lines: Vec<(String, u32)> = Vec::new();
        if let Some((_, metadata)) = &self.plugins.preview {
            push_lines(
                &mut lines,
                &self.tf(
                    "plugins.install_confirm",
                    &[("id", &metadata.id()), ("version", &metadata.version())],
                ),
                max,
                text,
            );
            self.plugin_metadata_lines(metadata, max, &mut lines);
        } else if let Some(id) = &self.plugins.trust {
            let version = self
                .plugins
                .installed
                .iter()
                .find(|record| record.metadata.id() == id)
                .map(|record| record.metadata.version())
                .unwrap_or_default();
            push_lines(
                &mut lines,
                &self.tf(
                    "plugins.trust_confirm",
                    &[("id", id), ("version", &version)],
                ),
                max,
                text,
            );
            push_lines(&mut lines, self.t("plugins.trust_warning"), max, dim);
        } else if let Some(metadata) = &self.plugins.inspecting {
            push_lines(
                &mut lines,
                &self.tf(
                    "plugins.sidebar.details_title",
                    &[("id", &metadata.id()), ("version", &metadata.version())],
                ),
                max,
                text,
            );
            self.plugin_metadata_lines(metadata, max, &mut lines);
            if let Some(message) = self.plugins.errors.get(metadata.id()) {
                push_lines(&mut lines, message, max, error);
            }
        }
        for (index, (line, color)) in lines.iter().enumerate() {
            let y = layout.list_y + index as f32 * line_height;
            if y + line_height > layout.list_bottom {
                break;
            }
            renderer.draw_text(line, layout.sidebar_x + PAD, y, rgb(*color));
        }
    }

    fn plugin_metadata_lines(
        &self,
        metadata: &Metadata,
        max: usize,
        lines: &mut Vec<(String, u32)>,
    ) {
        let text = Theme::color(Theme::TEXT, 0x1e1e1e);
        let dim = Theme::color(Theme::DIM, 0x616161);
        match metadata {
            Metadata::Native(manifest) => {
                push_lines(
                    lines,
                    &self.tf(
                        "plugins.description",
                        &[("description", &manifest.description)],
                    ),
                    max,
                    text,
                );
                push_lines(
                    lines,
                    &self.tf(
                        "plugins.api_entry",
                        &[("api", &manifest.api_version), ("entry", &manifest.main)],
                    ),
                    max,
                    dim,
                );
                let commands = manifest
                    .commands
                    .iter()
                    .map(|command| command.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                push_lines(
                    lines,
                    &self.tf("plugins.commands", &[("commands", &commands)]),
                    max,
                    dim,
                );
            }
            Metadata::Vsix(vsix) => {
                push_lines(
                    lines,
                    &self.tf("plugins.description", &[("description", &vsix.description)]),
                    max,
                    text,
                );
                push_lines(
                    lines,
                    &self.tf("plugins.engines", &[("engines", &vsix.engines)]),
                    max,
                    dim,
                );
                push_lines(
                    lines,
                    &self.tf(
                        "plugins.entry",
                        &[(
                            "entry",
                            &vsix.main.as_deref().unwrap_or(self.t("common.none")),
                        )],
                    ),
                    max,
                    dim,
                );
                let contributions = vsix.contributions.join(", ");
                push_lines(
                    lines,
                    &self.tf(
                        "plugins.contributions",
                        &[("contributions", &contributions)],
                    ),
                    max,
                    dim,
                );
                if vsix.main.is_some() {
                    push_lines(lines, self.t("plugins.vsix_enable_hint"), max, dim);
                }
            }
        }
    }

    fn draw_plugin_footer(
        &self,
        renderer: &mut Renderer,
        layout: &PluginsLayout,
        line_height: f32,
        cell_width: f32,
    ) {
        let key = if !self.plugins.focused {
            "plugins.sidebar.hints_unfocused"
        } else if self.plugins.preview.is_some() || self.plugins.trust.is_some() {
            "plugins.sidebar.hints_confirm"
        } else if self.plugins.inspecting.is_some() {
            "plugins.sidebar.hints_details"
        } else if self.plugins.editing {
            "plugins.sidebar.hints_editing"
        } else {
            match self.plugins.tab {
                PluginsTab::Installed => "plugins.sidebar.hints_installed",
                PluginsTab::OpenVsx => "plugins.sidebar.hints_open_vsx",
                PluginsTab::Graphite => {
                    if self.plugins.graphite_assets.is_some() {
                        "plugins.sidebar.hints_assets"
                    } else {
                        "plugins.sidebar.hints_graphite"
                    }
                }
            }
        };
        let max = ((layout.width - PAD * 2.0) / cell_width).max(1.0) as usize;
        renderer.draw_rect(
            layout.sidebar_x + PAD,
            layout.footer_y - line_height * 0.35,
            layout.width - PAD * 2.0,
            1.0,
            rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
        );
        for (index, line) in self.t(key).lines().take(5).enumerate() {
            renderer.draw_text(
                &fit(line, max),
                layout.sidebar_x + PAD,
                layout.footer_y + index as f32 * line_height,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
        }
    }
}

fn markdown_display_rows(markdown: &str, width: usize) -> Vec<(String, u32, usize)> {
    let mut rows = Vec::new();
    let mut in_code = false;
    for (source_line, raw) in markdown.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if trimmed.is_empty() {
            rows.push((
                String::new(),
                Theme::color(Theme::DIM, 0x616161),
                source_line,
            ));
            continue;
        }
        if in_code {
            for line in wrap_text(raw, width.saturating_sub(2)).into_iter() {
                rows.push((format!("  {line}"), 0x6a9955, source_line));
            }
            continue;
        }
        let (text, color) = if let Some(heading) = trimmed.strip_prefix("#### ") {
            (format!("  {}", markdown_inline_text(heading)), 0x569cd6)
        } else if let Some(heading) = trimmed.strip_prefix("### ") {
            (format!("▌ {}", markdown_inline_text(heading)), 0x569cd6)
        } else if let Some(heading) = trimmed.strip_prefix("## ") {
            (format!("▌ {}", markdown_inline_text(heading)), 0x569cd6)
        } else if let Some(heading) = trimmed.strip_prefix("# ") {
            (format!("▌ {}", markdown_inline_text(heading)), 0x569cd6)
        } else if let Some(item) = trimmed.strip_prefix("- [ ] ") {
            (
                format!("☐ {}", markdown_inline_text(item)),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else if let Some(item) = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "))
        {
            (
                format!("☑ {}", markdown_inline_text(item)),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            (
                format!("• {}", markdown_inline_text(item)),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else if let Some((number, item)) = trimmed
            .split_once(". ")
            .filter(|(number, _)| number.chars().all(|character| character.is_ascii_digit()))
        {
            (
                format!("{number}. {}", markdown_inline_text(item)),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else if trimmed.starts_with('|') {
            let cells = trimmed
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .filter(|cell| {
                    !cell
                        .chars()
                        .all(|c| c == '-' || c == ':' || c.is_whitespace())
                })
                .map(markdown_inline_text)
                .collect::<Vec<_>>();
            (cells.join("   │   "), Theme::color(Theme::TEXT, 0x1e1e1e))
        } else if let Some(quote) = trimmed.strip_prefix("> ") {
            (format!("│ {}", markdown_inline_text(quote)), 0x9da5b4)
        } else {
            (
                markdown_inline_text(trimmed),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        };
        for line in wrap_text(&text, width) {
            rows.push((line, color, source_line));
        }
    }
    rows
}

fn markdown_inline_text(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '!' && chars.peek() == Some(&'[') {
            chars.next();
            let alt = collect_until(&mut chars, ']');
            if chars.next() == Some('(') {
                let target = collect_until(&mut chars, ')');
                output.push_str("▧ ");
                output.push_str(&alt);
                if !target.is_empty() {
                    output.push_str(" <");
                    output.push_str(&target);
                    output.push('>');
                }
            } else {
                output.push_str(&alt);
            }
        } else if ch == '[' {
            let label = collect_until(&mut chars, ']');
            if chars.next() == Some('(') {
                let target = collect_until(&mut chars, ')');
                output.push_str(&label);
                if !target.is_empty() {
                    output.push_str(" <");
                    output.push_str(&target);
                    output.push('>');
                }
            } else {
                output.push_str(&label);
            }
        } else if matches!(ch, '`' | '*' | '_' | '~') {
            while chars.peek() == Some(&ch) {
                chars.next();
            }
        } else if ch == '<' {
            for next in chars.by_ref() {
                if next == '>' {
                    break;
                }
            }
        } else if !ch.is_control() {
            output.push(ch);
        } else {
            output.push(' ');
        }
    }
    output
}

fn collect_until(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, end: char) -> String {
    let mut text = String::new();
    for ch in chars.by_ref() {
        if ch == end {
            break;
        }
        text.push(ch);
    }
    text
}

fn markdown_url_at(
    markdown: &str,
    line: usize,
    repository: &graphite_plugins::catalog::Repository,
) -> Option<String> {
    let source = markdown.lines().nth(line)?;
    let start = source.find("](")? + 2;
    let end = source[start..].find(')')? + start;
    let url = source[start..end].trim();
    if let Ok(parsed) = reqwest::Url::parse(url) {
        return (parsed.scheme() == "https").then(|| url.to_owned());
    }
    if url.starts_with('#') || url.is_empty() || url.starts_with("//") {
        return None;
    }
    let branch = repository.default_branch.as_deref().unwrap_or("HEAD");
    let path = if url.starts_with('/') {
        url.trim_start_matches('/')
    } else {
        url
    };
    let target = format!(
        "{}/blob/{branch}/{path}",
        repository.html_url.trim_end_matches('/')
    );
    is_https_url(&target).then_some(target)
}

fn is_https_url(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| url.scheme() == "https" && url.host_str().is_some())
}

fn open_external_https(value: &str) {
    if !is_https_url(value) {
        return;
    }
    #[cfg(windows)]
    let mut command = std::process::Command::new("explorer");
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(not(any(windows, target_os = "linux")))]
    let mut command = std::process::Command::new("open");
    let _ = command.arg(value).spawn();
}

#[cfg(test)]
mod graphite_readme_tests {
    use super::*;

    #[test]
    fn readme_blocks_render_common_markdown_as_safe_text() {
        let rows = markdown_display_rows(
            "# Title\n\n- item **bold**\n\n```js\n<script>alert(1)</script>\n```\n\n[Docs](https://example.test)",
            40,
        );
        let rendered = rows
            .iter()
            .map(|(line, _, _)| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Title"));
        assert!(rendered.contains("• item bold"));
        assert!(rendered.contains("alert(1)"));
        assert!(rendered.contains("Docs <https://example.test>"));
        assert_eq!(
            markdown_inline_text("<script>alert(1)</script>"),
            "alert(1)"
        );
    }

    #[test]
    fn resource_links_use_the_available_panel_width_before_truncating() {
        for label in ["Repository", "Homepage", "Issues", "License · MIT"] {
            assert_eq!(graphite_resource_label(label, 28), format!("↗ {label}"));
        }
    }

    #[test]
    fn readme_relative_links_resolve_to_the_repository_default_branch() {
        let repository = graphite_plugins::catalog::Repository {
            full_name: "author/plugin".into(),
            description: None,
            stargazers_count: 0,
            html_url: "https://github.com/author/plugin".into(),
            homepage: None,
            topics: Vec::new(),
            license: None,
            open_issues_count: 0,
            default_branch: Some("main".into()),
        };
        assert_eq!(
            markdown_url_at("[guide](docs/guide.md)", 0, &repository).as_deref(),
            Some("https://github.com/author/plugin/blob/main/docs/guide.md")
        );
        assert!(markdown_url_at("[bad](javascript:alert(1))", 0, &repository).is_none());
    }
}
