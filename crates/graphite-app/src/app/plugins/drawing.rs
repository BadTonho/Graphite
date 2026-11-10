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
        let active = if self.plugins.marketplace {
            PluginsTab::OpenVsx
        } else {
            PluginsTab::Installed
        };
        let half = layout.width * 0.5;
        for (index, tab) in [PluginsTab::Installed, PluginsTab::OpenVsx]
            .into_iter()
            .enumerate()
        {
            let x = layout.sidebar_x + index as f32 * half;
            let label = match tab {
                PluginsTab::Installed => self.t("plugins.sidebar.installed"),
                PluginsTab::OpenVsx => self.t("plugins.sidebar.open_vsx"),
            };
            let color = if tab == active {
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
            if tab == active {
                renderer.draw_rect(
                    x + PAD,
                    layout.tabs_y + layout.tab_height - 2.0,
                    half - PAD * 2.0,
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
            self.t("plugins.sidebar.search_open_vsx")
        };
        let (label, color) = if self.plugins.editing {
            (
                format!("{}_", self.plugins.query),
                Theme::color(Theme::TEXT, 0x1e1e1e),
            )
        } else if !self.plugins.query.is_empty()
            && (self.plugins.marketplace || self.plugins.path_input)
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
        if self.plugins.marketplace {
            (
                self.tf(
                    "plugins.sidebar.section_open_vsx",
                    &[("count", &self.plugins.open_vsx.len())],
                ),
                "plugins.sidebar.empty_open_vsx",
            )
        } else {
            (
                self.tf(
                    "plugins.sidebar.section_installed",
                    &[("count", &self.plugins.installed.len())],
                ),
                "plugins.sidebar.empty_installed",
            )
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
        if self.plugins.marketplace {
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
        } else if self.plugins.marketplace {
            "plugins.sidebar.hints_open_vsx"
        } else {
            "plugins.sidebar.hints_installed"
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
