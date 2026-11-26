//! Drawing the title bar, the open menu frame, and the window controls.

use super::*;

impl App {
    pub(crate) fn draw_workbench_menu(&self, renderer: &mut Renderer) {
        if self.zen_mode {
            return;
        }
        let (width, _) = renderer.size();
        let line_height = renderer.line_height();
        let char_width = renderer.char_width();
        let layout = self.titlebar_layout(width, char_width);
        let menu_height = layout.height;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let hovered = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        renderer.draw_rect(0.0, 0.0, width, menu_height, background);
        renderer.draw_rect(0.0, menu_height - 1.0, width, 1.0, border);
        draw_brand_icon(renderer, layout.icon);

        let active_is_overflowed = self.workbench_menu.is_some_and(|menu| {
            menu != WorkbenchMenu::More
                && layout.overflowed.contains(&menu)
                && !self.workbench_menu_overflow
        });
        for item in layout
            .menus
            .iter()
            .filter(|_| self.view_preferences.menu_bar_visible)
        {
            let selected = self.workbench_menu == Some(item.menu)
                || (item.menu == WorkbenchMenu::More && active_is_overflowed);
            let item_hovered = item.contains(self.mouse.x as f32, self.mouse.y as f32, menu_height);
            if selected || item_hovered {
                renderer.draw_rect(
                    item.x,
                    1.0,
                    item.width,
                    menu_height - 2.0,
                    if item_hovered {
                        hovered
                    } else {
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d))
                    },
                );
            }
            renderer.draw_text(
                self.t(item.menu.label_key()),
                item.x + MENU_HORIZONTAL_PADDING * self.sidebar_scale_factor,
                (menu_height - line_height) * 0.5,
                text,
            );
        }

        if self.workbench_menu == Some(WorkbenchMenu::More) && self.workbench_menu_overflow {
            let popup = layout.overflow_popup(char_width, width);
            renderer.draw_rect(popup.x, popup.y, popup.width, popup.height, border);
            renderer.draw_rect(
                popup.x + 1.0,
                popup.y + 1.0,
                popup.width - 2.0,
                popup.height - 2.0,
                background,
            );
            for (index, menu) in layout.overflowed.iter().copied().enumerate() {
                let y = popup.y + 4.0 + index as f32 * OVERFLOW_ROW_HEIGHT;
                if index == self.workbench_overflow_selection {
                    renderer.draw_rect(
                        popup.x + 2.0,
                        y,
                        popup.width - 4.0,
                        OVERFLOW_ROW_HEIGHT,
                        hovered,
                    );
                }
                renderer.draw_text(self.t(menu.label_key()), popup.x + 14.0, y + 5.0, text);
            }
        }

        if let Some(active) = self.workbench_menu {
            if active == WorkbenchMenu::File {
                self.draw_file_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Edit {
                self.draw_edit_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Selection {
                self.draw_selection_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Go {
                self.draw_go_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Terminal {
                self.draw_terminal_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Run {
                self.draw_run_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Help {
                self.draw_help_menu(renderer, &layout);
            } else if active != WorkbenchMenu::View
                && active != WorkbenchMenu::Go
                && active != WorkbenchMenu::More
                && !self.workbench_menu_overflow
            {
                let popup_width = (self.t("menubar.empty").chars().count() as f32 * char_width
                    + MENU_HORIZONTAL_PADDING * 2.0)
                    .max(EMPTY_MENU_WIDTH)
                    .min((width - 2.0).max(80.0));
                let popup_x = layout
                    .menu_anchor(active)
                    .clamp(0.0, (width - popup_width).max(0.0));
                renderer.draw_rect(popup_x, menu_height, popup_width, EMPTY_MENU_HEIGHT, border);
                renderer.draw_rect(
                    popup_x + 1.0,
                    menu_height + 1.0,
                    popup_width - 2.0,
                    EMPTY_MENU_HEIGHT - 2.0,
                    background,
                );
                renderer.draw_text(
                    self.t("menubar.empty"),
                    popup_x + MENU_HORIZONTAL_PADDING,
                    menu_height + (EMPTY_MENU_HEIGHT - line_height) * 0.5,
                    dim,
                );
            }
        }
        if self.workbench_menu == Some(WorkbenchMenu::View) {
            self.draw_view_menu(renderer);
        }

        self.draw_run_breakpoint_prompt(renderer);
        self.draw_rename_prompt(renderer);
        self.draw_branch_name_prompt(renderer);
        self.draw_explorer_prompt(renderer);
        self.draw_explorer_delete_confirm(renderer);
        self.draw_scm_confirm(renderer);
        self.draw_scm_publish_modal(renderer);
        self.draw_github_dialog(renderer);
        self.draw_command_center_controls(renderer, &layout);
        self.draw_titlebar_controls(renderer, layout.controls);
        self.draw_layout_controls(renderer, &layout.layout_controls, layout.height);
        self.draw_titlebar_update_button(renderer, &layout);
    }

    pub(crate) fn titlebar_update_button(&self) -> Option<TitlebarUpdateButton> {
        use crate::app::updates::state::UpdateStatus;
        match &self.updates.status {
            UpdateStatus::Available(release) => Some(TitlebarUpdateButton {
                label: self.tf("titlebar.update.available", &[("tag", &release.tag_name)]),
                tooltip: self.tf(
                    "titlebar.update.tip_available",
                    &[("tag", &release.tag_name)],
                ),
                bg_hex: 0x007acc,
                hover_hex: 0x1a8ad4,
                action: TitlebarUpdateAction::Download,
            }),
            UpdateStatus::Downloading { .. } => Some(TitlebarUpdateButton {
                label: self.t("titlebar.update.downloading").to_owned(),
                tooltip: self.t("titlebar.update.downloading").to_owned(),
                bg_hex: 0x3a3d41,
                hover_hex: 0x3a3d41,
                action: TitlebarUpdateAction::None,
            }),
            UpdateStatus::ReadyToRestart { release, .. } => Some(TitlebarUpdateButton {
                label: self.tf("titlebar.update.restart", &[("tag", &release.tag_name)]),
                tooltip: self.tf("titlebar.update.tip_restart", &[("tag", &release.tag_name)]),
                bg_hex: 0x2f855a,
                hover_hex: 0x38a169,
                action: TitlebarUpdateAction::Restart,
            }),
            _ => None,
        }
    }

    pub(crate) fn click_titlebar_update_button(&mut self) {
        let Some(button) = self.titlebar_update_button() else {
            return;
        };
        match button.action {
            TitlebarUpdateAction::Download => self.download_update(),
            TitlebarUpdateAction::Restart => self.restart_to_update(),
            TitlebarUpdateAction::None => {}
        }
    }

    pub(super) fn draw_titlebar_update_button(
        &self,
        renderer: &mut Renderer,
        layout: &TitlebarLayout,
    ) {
        let Some(rect) = layout.update_button else {
            return;
        };
        let Some(button) = self.titlebar_update_button() else {
            return;
        };
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        let hovered = rect.contains(mx, my);
        let bg = rgb(if hovered {
            button.hover_hex
        } else {
            button.bg_hex
        });
        renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, bg);
        let cw = renderer.char_width().max(4.0);
        let lh = renderer.line_height();
        let text_w = button.label.chars().count() as f32 * cw;
        let text_x = rect.x + ((rect.width - text_w) * 0.5).max(4.0);
        let text_y = rect.y + ((rect.height - lh) * 0.5).max(0.0);
        renderer.draw_text(&button.label, text_x, text_y, rgb(0xffffff));

        if hovered {
            let win_w = renderer.size().0;
            let tip_w = button.tooltip.chars().count() as f32 * cw + 16.0;
            let tip_h = lh + 10.0;
            let tip_x = rect.x.clamp(4.0, (win_w - tip_w - 4.0).max(4.0));
            let tip_y = layout.height + 4.0;
            renderer.draw_rect(
                tip_x,
                tip_y,
                tip_w,
                tip_h,
                rgb(Theme::color(Theme::DIM, 0x454545)),
            );
            renderer.draw_rect(
                tip_x + 1.0,
                tip_y + 1.0,
                tip_w - 2.0,
                tip_h - 2.0,
                rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
            );
            renderer.draw_text(
                &button.tooltip,
                tip_x + 8.0,
                tip_y + 5.0,
                rgb(Theme::color(Theme::TEXT, 0xcccccc)),
            );
        }
    }

    pub(super) fn draw_command_center_controls(
        &self,
        renderer: &mut Renderer,
        layout: &TitlebarLayout,
    ) {
        let center = layout.command_center;
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let hover = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        for (rect, enabled, glyph) in [
            (center.back, self.can_navigate_history(-1), "←"),
            (center.forward, self.can_navigate_history(1), "→"),
        ] {
            if rect.contains(self.mouse.x as f32, self.mouse.y as f32) {
                renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, hover);
            }
            let color = if enabled { text } else { dim };
            renderer.draw_text(
                glyph,
                rect.x + (rect.width - renderer.char_width()) * 0.5,
                rect.y + (rect.height - renderer.line_height()) * 0.5,
                color,
            );
        }
        let active = self.command_center.is_some();
        let focused = center
            .field
            .contains(self.mouse.x as f32, self.mouse.y as f32);
        let border = if active {
            rgb(0x007acc)
        } else if focused {
            rgb(Theme::color(Theme::DIM, 0x777777))
        } else {
            rgb(Theme::color(Theme::DIM, 0x454545))
        };
        renderer.draw_rect(
            center.field.x,
            center.field.y,
            center.field.width,
            center.field.height,
            border,
        );
        renderer.draw_rect(
            center.field.x + 1.0,
            center.field.y + 1.0,
            (center.field.width - 2.0).max(0.0),
            (center.field.height - 2.0).max(0.0),
            rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
        );
        let value = self
            .command_center
            .as_ref()
            .map(|center| center.query.as_str())
            .filter(|query| !query.is_empty())
            .unwrap_or_else(|| self.t("command_center.placeholder"));
        let value_width = (center.field.width - 18.0).max(0.0);
        let visible = value
            .chars()
            .rev()
            .take((value_width / renderer.char_width().max(1.0)) as usize)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        renderer.draw_text(
            &visible,
            center.field.x + 9.0,
            center.field.y + (center.field.height - renderer.line_height()) * 0.5,
            if self.command_center.is_some() || value != self.t("command_center.placeholder") {
                text
            } else {
                dim
            },
        );
    }

    pub(super) fn draw_titlebar_controls(
        &self,
        renderer: &mut Renderer,
        controls: TitlebarControlRects,
    ) {
        let (x, y) = (self.mouse.x as f32, self.mouse.y as f32);
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let hovered = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        let close_hovered = rgb(0xc42b1c);
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        for (rect, close) in [
            (controls.minimize, false),
            (controls.maximize, false),
            (controls.close, true),
        ] {
            let is_hovered = rect.contains(x, y);
            if is_hovered {
                renderer.draw_rect(
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    if close { close_hovered } else { hovered },
                );
            } else {
                renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, background);
            }
        }
        let cy = controls.minimize.height * 0.5;
        let min_cx = controls.minimize.x + controls.minimize.width * 0.5;
        renderer.draw_rect(min_cx - 5.5, cy + 4.0, 11.0, 1.2, foreground);
        let max_cx = controls.maximize.x + controls.maximize.width * 0.5;
        if self
            .gpu
            .as_ref()
            .is_some_and(|gpu| gpu.window.is_maximized())
        {
            renderer.draw_rect(max_cx - 4.0, cy - 4.0, 8.0, 7.0, foreground);
            renderer.draw_rect(max_cx - 1.0, cy - 1.0, 7.0, 7.0, background);
            renderer.draw_rect(max_cx - 1.0, cy - 1.0, 7.0, 1.0, foreground);
            renderer.draw_rect(max_cx + 5.0, cy - 1.0, 1.0, 7.0, foreground);
            renderer.draw_rect(max_cx - 1.0, cy + 5.0, 7.0, 1.0, foreground);
        } else {
            draw_outline_rect(renderer, max_cx - 5.0, cy - 5.0, 10.0, 10.0, foreground);
        }
        let close = controls.close;
        renderer.draw_text(
            "×",
            close.x + close.width * 0.5 - char_width_center(renderer, '×'),
            cy - renderer.line_height() * 0.5,
            foreground,
        );
    }
}
