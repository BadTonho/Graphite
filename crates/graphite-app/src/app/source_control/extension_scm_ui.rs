//! Source control providers registered by extensions, drawn and handled below the Git changes.

use super::scm_ui::{
    scm_centered_text_x, scm_file_label_x, scm_fit_label, scm_icon_color, scm_visible_tail,
    ScmListRow,
};
use super::*;
use crate::app::plugins::vsix::ExtensionRow;

impl App {
    /// Draws one row of an extension source control. `metrics` is `[row height, line height,
    /// cell width, sidebar width]`.
    pub(super) fn draw_extension_scm_row(
        &self,
        renderer: &mut Renderer,
        row: &ScmListRow,
        extension: ExtensionRow,
        metrics: [f32; 4],
    ) {
        let [row_h, line_height, cell_width, sidebar_width] = metrics;
        let left = ACTIVITY_BAR_WIDTH + PAD;
        let right = sidebar_width - PAD;
        let y = row.bounds.y;
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        let Some(scm) = self.vsix_scm() else {
            return;
        };
        match extension {
            ExtensionRow::Provider(handle) => {
                let Some(provider) = scm.provider(handle) else {
                    return;
                };
                renderer.draw_rect(
                    ACTIVITY_BAR_WIDTH,
                    y,
                    sidebar_width - ACTIVITY_BAR_WIDTH,
                    1.0,
                    rgb(Theme::color(0x3c3c3c, 0xdddddd)),
                );
                let count = provider.display_count().to_string();
                let chars = ((right - left) / cell_width) as usize;
                let room = chars.saturating_sub(count.chars().count() + 2);
                renderer.draw_text(&scm_fit_label(&provider.label, room), left, y, text);
                renderer.draw_text(
                    &count,
                    right - cell_width * count.chars().count() as f32,
                    y,
                    accent,
                );
            }
            ExtensionRow::Input(handle) => {
                let Some(provider) = scm.provider(handle) else {
                    return;
                };
                let focused = self.scm.extension_input == Some(handle);
                let button_width = row.action.width;
                let field_width = right - left - button_width - cell_width * 0.5;
                let border = if focused {
                    Theme::color(Theme::STATUS, 0x007acc)
                } else {
                    Theme::color(0x606060, 0x8a8a8a)
                };
                renderer.draw_rect(
                    left,
                    y,
                    field_width,
                    row_h,
                    rgb(Theme::color(0x1f1f1f, 0xffffff)),
                );
                for (x, width, height, top) in [
                    (left, field_width, 1.0, y),
                    (left, field_width, 1.0, y + row_h - 1.0),
                    (left, 1.0, row_h, y),
                    (left + field_width - 1.0, 1.0, row_h, y),
                ] {
                    renderer.draw_rect(x, top, width, height, rgb(border));
                }
                let padding = cell_width * 0.55;
                let visible = ((field_width - padding * 2.0) / cell_width)
                    .floor()
                    .max(0.0) as usize;
                let placeholder = provider
                    .placeholder
                    .as_deref()
                    .unwrap_or_else(|| self.t("scm.sidebar.extension_placeholder"));
                let (shown, color) = if provider.input.is_empty() {
                    (
                        placeholder.chars().take(visible).collect::<String>(),
                        rgb(Theme::color(0xb0b0b0, 0x666666)),
                    )
                } else {
                    (scm_visible_tail(&provider.input, visible), text)
                };
                renderer.draw_text(
                    &shown,
                    left + padding,
                    y + (row_h - line_height) * 0.5 + row_h * 0.08,
                    color,
                );
                if focused {
                    let carets = if provider.input.is_empty() {
                        0
                    } else {
                        shown.chars().count()
                    };
                    renderer.draw_rect(
                        left + padding + carets as f32 * cell_width,
                        y + row_h * 0.22,
                        1.5,
                        row_h * 0.56,
                        accent,
                    );
                }
                let enabled = provider.accept_input.is_some() && provider.input_enabled;
                renderer.draw_rect(
                    row.action.x,
                    y,
                    row.action.width,
                    row_h,
                    if enabled {
                        rgb(Theme::color(Theme::STATUS, 0x0e639c))
                    } else {
                        rgb(Theme::color(0x3c3c3c, 0xe5e5e5))
                    },
                );
                renderer.draw_text(
                    "✓",
                    scm_centered_text_x(row.action.x, row.action.width, "✓", cell_width),
                    y + (row_h - line_height) * 0.5 + row_h * 0.08,
                    if enabled { rgb(0xffffff) } else { dim },
                );
            }
            ExtensionRow::Group {
                provider,
                group,
                count,
                collapsed,
            } => {
                let Some(group) = scm.provider(provider).and_then(|item| item.group(group)) else {
                    return;
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
                let count = count.to_string();
                let chars = ((right - row.chevron.x - row.chevron.width) / cell_width) as usize;
                let room = chars.saturating_sub(count.chars().count() + 2);
                renderer.draw_text(
                    &scm_fit_label(&group.label, room),
                    row.chevron.x + row.chevron.width,
                    y,
                    text,
                );
                renderer.draw_text(
                    &count,
                    right - cell_width * count.chars().count() as f32,
                    y,
                    accent,
                );
            }
            ExtensionRow::Resource {
                provider,
                group,
                resource,
            } => {
                let Some(item) = scm
                    .provider(provider)
                    .and_then(|item| item.group(group))
                    .and_then(|group| group.resources.iter().find(|item| item.handle == resource))
                else {
                    return;
                };
                let icon_size = (row_h * 0.72).round();
                let icon_x = left + cell_width;
                let name = item.name();
                let label_x = match item.file_path() {
                    Some(path) => {
                        let icon = crate::icon_theme::for_path(&path);
                        crate::icon_theme::draw(
                            renderer,
                            &icon,
                            icon_x,
                            y + (row_h - icon_size) * 0.5,
                            icon_size,
                            scm_icon_color,
                        );
                        scm_file_label_x(icon_x, icon_size, cell_width * 0.65)
                    }
                    None => icon_x,
                };
                let max = ((right - label_x - cell_width) / cell_width).max(1.0) as usize;
                let name = name.chars().take(max).collect::<String>();
                let color = if item.faded { dim } else { text };
                renderer.draw_text(&name, label_x, y, color);
                let name_width = name.chars().count() as f32 * cell_width;
                if item.strike_through {
                    renderer.draw_rect(label_x, y + row_h * 0.5, name_width, 1.0, color);
                }
                let description_x = label_x + name_width + cell_width * 0.4;
                let description_max =
                    ((right - description_x - cell_width) / cell_width).max(0.0) as usize;
                renderer.draw_text(
                    &item
                        .description()
                        .chars()
                        .take(description_max)
                        .collect::<String>(),
                    description_x,
                    y,
                    dim,
                );
            }
        }
    }

    /// Handles a click on an extension row of the Source Control panel. `x` and `y` are in the
    /// panel's coordinates.
    pub(crate) fn click_extension_scm_row(
        &mut self,
        row: &ScmListRow,
        extension: ExtensionRow,
        x: f32,
        y: f32,
    ) {
        match extension {
            ExtensionRow::Provider(_) => {}
            ExtensionRow::Input(handle) => {
                if row.action.contains(x, y) {
                    self.accept_vsix_scm_input(handle);
                } else {
                    self.scm.extension_input = Some(handle);
                }
            }
            ExtensionRow::Group {
                provider, group, ..
            } => self.toggle_vsix_scm_group(provider, group),
            ExtensionRow::Resource {
                provider,
                group,
                resource,
            } => self.open_vsix_scm_resource(provider, group, resource),
        }
    }

    /// Typing into an extension commit box. Returns true when the key was consumed.
    pub(crate) fn handle_extension_scm_input_key(
        &mut self,
        key: &winit::keyboard::Key,
        text: Option<&str>,
        ctrl: bool,
        alt: bool,
    ) -> bool {
        use winit::keyboard::{Key, NamedKey};
        let Some(provider) = self.scm.extension_input else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => self.scm.extension_input = None,
            Key::Named(NamedKey::Enter) if ctrl => self.accept_vsix_scm_input(provider),
            Key::Named(NamedKey::Enter) => {}
            Key::Named(NamedKey::Backspace) => {
                self.edit_vsix_scm_input(provider, |input| {
                    input.pop();
                });
            }
            _ if !ctrl && !alt => {
                if let Some(text) = text.filter(|text| !text.chars().any(char::is_control)) {
                    self.edit_vsix_scm_input(provider, |input| input.push_str(text));
                }
            }
            _ => {}
        }
        true
    }
}
