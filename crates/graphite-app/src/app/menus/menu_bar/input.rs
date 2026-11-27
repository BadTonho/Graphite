//! Pointer and keyboard input of the title bar and its menus.

use super::*;

impl App {
    pub(crate) fn top_menu_height(&self) -> f32 {
        if self.zen_mode && self.ai_mode.is_none() {
            return 0.0;
        }
        self.full_titlebar_height()
    }

    fn full_titlebar_height(&self) -> f32 {
        self.font_size * self.sidebar_scale_factor * 2.1
    }

    pub(super) fn mode_button_rect(&self, mut slot: MenuItemRectLike) -> MenuItemRectLike {
        let height = self.full_titlebar_height();
        let inset = (3.0 * self.sidebar_scale_factor).min(height * 0.5);
        slot.y = inset;
        slot.height = height - 2.0 * inset;
        slot
    }

    pub(crate) fn command_center_rects(
        &self,
        width: f32,
        char_width: f32,
    ) -> (RectTuple, RectTuple, RectTuple) {
        let layout = self.titlebar_layout(width, char_width);
        let rect = |rect: MenuItemRectLike| (rect.x, rect.y, rect.width, rect.height);
        (
            rect(layout.command_center.back),
            rect(layout.command_center.forward),
            rect(layout.command_center.field),
        )
    }

    pub(crate) fn titlebar_layout(&self, width: f32, char_width: f32) -> TitlebarLayout {
        let scale = self.sidebar_scale_factor.max(0.5);
        let localized_widths = WorkbenchMenu::ALL.map(|menu| {
            self.t(menu.label_key()).chars().count() as f32 * char_width
                + MENU_HORIZONTAL_PADDING * 2.0 * scale
        });
        let update_button_width = self.titlebar_update_button().map_or(0.0, |btn| {
            btn.label.chars().count() as f32 * char_width + 16.0 * scale
        });
        let return_button_width = if self.can_return_to_review() {
            self.t("command.graphite.workbench.returnToReview")
                .chars()
                .count() as f32
                * char_width
                + 16.0 * scale
        } else {
            0.0
        };
        TitlebarLayout::new(
            width,
            self.top_menu_height(),
            char_width,
            scale,
            localized_widths,
            update_button_width,
            return_button_width,
        )
    }

    pub(crate) fn titlebar_accessible_menus(
        &self,
        width: f32,
        char_width: f32,
    ) -> Vec<WorkbenchMenu> {
        if !self.view_preferences.menu_bar_visible {
            return Vec::new();
        }
        self.titlebar_layout(width, char_width)
            .menus
            .into_iter()
            .map(|item| item.menu)
            .collect()
    }

    pub(crate) fn view_menu_open(&mut self, x: f32, width: f32, height: f32) {
        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let anchor = self
            .titlebar_layout(width, char_width)
            .menu_anchor(WorkbenchMenu::View);
        self.view_menu_open_at(if anchor > 0.0 { anchor } else { x }, width, height);
    }

    pub(super) fn view_menu_open_at(&mut self, x: f32, width: f32, height: f32) {
        self.open_view_menu(x, self.top_menu_height(), width, height);
    }

    pub(crate) fn close_workbench_menu(&mut self) {
        self.workbench_menu = None;
        self.workbench_menu_overflow = false;
        self.activity_bar_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.view_menu = None;
        self.run_breakpoint_submenu = false;
        self.go_submenu = None;
    }

    pub(crate) fn dismiss_workbench_menu(&mut self) {
        self.close_workbench_menu();
    }

    pub(super) fn activate_menu(
        &mut self,
        menu: WorkbenchMenu,
        anchor_x: f32,
        width: f32,
        height: f32,
    ) {
        if self.command_center.is_some() {
            self.close_command_center();
        }
        self.workbench_menu = Some(menu);
        self.workbench_menu_overflow = false;
        self.activity_bar_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.view_menu = None;
        if menu == WorkbenchMenu::File {
            self.file_menu_submenu = None;
            self.file_menu_nested = None;
            self.file_menu_selection = 0;
        }
        if menu == WorkbenchMenu::Terminal {
            self.terminal_menu_selection = 0;
        }
        if menu == WorkbenchMenu::Selection {
            self.selection_menu_selection = 0;
        }
        if menu == WorkbenchMenu::Go {
            self.go_menu_selection = 0;
            self.go_submenu = None;
        }
        if menu == WorkbenchMenu::Run {
            self.open_run_menu();
        }
        if menu == WorkbenchMenu::Help {
            self.help_menu_selection = 0;
        }
        if menu == WorkbenchMenu::View {
            self.view_menu_open_at(anchor_x, width, height);
        }
    }

    pub(crate) fn titlebar_control_click(&mut self, x: f32, y: f32) -> bool {
        let Some((width, char_width)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.renderer.size().0, gpu.renderer.char_width()))
        else {
            return false;
        };
        let layout = self.titlebar_layout(width, char_width);
        if layout.update_button.is_some_and(|rect| rect.contains(x, y)) {
            self.click_titlebar_update_button();
            return true;
        }
        if self.click_return_button(&layout, x, y)
            || self.click_mode_button(layout.mode_button, x, y)
        {
            return true;
        }
        if let Some(control) = (self.ai_mode.is_none())
            .then(|| {
                super::titlebar_layout_controls::layout_control_at(&layout.layout_controls, x, y)
            })
            .flatten()
        {
            let anchor = layout
                .layout_controls
                .iter()
                .find(|(item, _)| *item == control)
                .map_or(x, |(_, rect)| rect.0);
            self.run_layout_control(control, anchor);
            return true;
        }
        let target = layout.controls.at(x, y);
        let Some(target) = target else { return false };
        self.titlebar_control_action(target);
        true
    }

    fn click_return_button(&mut self, layout: &TitlebarLayout, x: f32, y: f32) -> bool {
        if !layout
            .return_button
            .is_some_and(|slot| self.mode_button_rect(slot).contains(x, y))
        {
            return false;
        }
        self.run_ui_command("graphite.workbench.returnToReview");
        true
    }

    pub(super) fn click_mode_button(&mut self, slot: MenuItemRectLike, x: f32, y: f32) -> bool {
        if !self.mode_button_rect(slot).contains(x, y) {
            return false;
        }
        self.run_ui_command("graphite.workbench.toggleMode");
        true
    }

    pub(crate) fn titlebar_control_action(&mut self, control: WindowControl) {
        match control {
            WindowControl::Minimize => {
                if let Some(gpu) = &self.gpu {
                    gpu.window.set_minimized(true);
                }
            }
            WindowControl::MaximizeRestore => {
                if let Some(gpu) = &self.gpu {
                    gpu.window.set_maximized(!gpu.window.is_maximized());
                }
            }
            WindowControl::Close => self.run_ui_command("workbench.action.quit"),
        }
    }

    pub(crate) fn titlebar_resize_direction(
        &self,
        x: f32,
        y: f32,
    ) -> Option<winit::window::ResizeDirection> {
        let gpu = self.gpu.as_ref()?;
        if gpu.window.is_maximized() || !gpu.window.is_resizable() {
            return None;
        }
        let (width, height) = gpu.renderer.size();
        let edge = TITLEBAR_RESIZE_EDGE * self.sidebar_scale_factor;
        let layout = self.titlebar_layout(width, gpu.renderer.char_width());
        resize_direction_for_titlebar(
            &layout,
            width,
            height,
            edge,
            x,
            y,
            gpu.window.is_maximized(),
        )
    }

    pub(crate) fn titlebar_handle_press(&mut self, x: f32, y: f32) -> bool {
        if let Some(direction) = self.titlebar_resize_direction(x, y) {
            if let Some(gpu) = &self.gpu {
                let _ = gpu.window.drag_resize_window(direction);
            }
            self.titlebar_pointer_down = None;
            return true;
        }
        let Some((width, char_width, window)) = self.gpu.as_ref().map(|gpu| {
            let (width, _) = gpu.renderer.size();
            (width, gpu.renderer.char_width(), gpu.window.clone())
        }) else {
            return false;
        };
        let layout = self.titlebar_layout(width, char_width);
        if self.zen_mode && self.ai_mode.is_none() {
            return self.click_return_button(&layout, x, y)
                || self.click_mode_button(layout.mode_button, x, y);
        }
        if y >= layout.height {
            return false;
        }
        if self.titlebar_control_click(x, y) {
            return true;
        }
        if layout.icon.contains(x, y) {
            return true;
        }
        if self.view_preferences.menu_bar_visible
            && layout
                .menus
                .iter()
                .any(|item| item.contains(x, y, layout.height))
        {
            let _ = self.workbench_menu_click(x, y);
            return true;
        }

        match layout.command_center.at(x, y) {
            Some(CommandCenterAction::Back) => {
                if self.can_navigate_history(-1) {
                    self.run_ui_command("workbench.action.navigateBack");
                }
                return true;
            }
            Some(CommandCenterAction::Forward) => {
                if self.can_navigate_history(1) {
                    self.run_ui_command("workbench.action.navigateForward");
                }
                return true;
            }
            Some(CommandCenterAction::Search) => {
                self.open_command_center();
                return true;
            }
            None => {}
        }

        let now = std::time::Instant::now();
        let double_click = self
            .last_titlebar_click
            .is_some_and(|last| now.duration_since(last) <= std::time::Duration::from_millis(450));
        if double_click {
            window.set_maximized(!window.is_maximized());
            self.last_titlebar_click = None;
            self.titlebar_pointer_down = None;
        } else {
            self.titlebar_pointer_down = Some((x, y));
            self.titlebar_drag_started = false;
        }
        true
    }

    pub(crate) fn titlebar_pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let Some((start_x, start_y)) = self.titlebar_pointer_down else {
            return false;
        };
        if !self.titlebar_drag_started
            && ((x - start_x).powi(2) + (y - start_y).powi(2)).sqrt() >= 4.0
        {
            if let Some(gpu) = &self.gpu {
                let _ = gpu.window.drag_window();
            }
            self.titlebar_drag_started = true;
            self.last_titlebar_click = None;
        }
        true
    }

    pub(crate) fn titlebar_pointer_released(&mut self) -> bool {
        if self.titlebar_pointer_down.take().is_none() {
            return false;
        }
        if !self.titlebar_drag_started {
            self.last_titlebar_click = Some(std::time::Instant::now());
        }
        self.titlebar_drag_started = false;
        true
    }

    pub(crate) fn titlebar_cursor(&self, x: f32, y: f32) -> Option<winit::window::CursorIcon> {
        if let Some(direction) = self.titlebar_resize_direction(x, y) {
            return Some(direction.into());
        }
        let gpu = self.gpu.as_ref()?;
        let layout = self.titlebar_layout(gpu.renderer.size().0, gpu.renderer.char_width());
        if layout.command_center.field.contains(x, y) {
            return Some(winit::window::CursorIcon::Text);
        }
        if layout.command_center.back.contains(x, y) || layout.command_center.forward.contains(x, y)
        {
            return Some(winit::window::CursorIcon::Pointer);
        }
        (y < layout.height).then_some(winit::window::CursorIcon::Default)
    }

    pub(crate) fn can_navigate_history(&self, direction: isize) -> bool {
        let Some(index) = self.navigation_history_index else {
            return false;
        };
        if direction < 0 {
            (0..index).rev().any(|candidate| {
                self.documents
                    .resolve(self.navigation_history[candidate].0)
                    .is_some()
            })
        } else {
            ((index + 1)..self.navigation_history.len()).any(|candidate| {
                self.documents
                    .resolve(self.navigation_history[candidate].0)
                    .is_some()
            })
        }
    }

    pub(crate) fn workbench_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some((width, height, char_width)) = self.gpu.as_ref().map(|gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height, gpu.renderer.char_width())
        }) else {
            return false;
        };
        let layout = self.titlebar_layout(width, char_width);
        if y < layout.height {
            let selected = self
                .view_preferences
                .menu_bar_visible
                .then(|| {
                    layout
                        .menus
                        .iter()
                        .copied()
                        .find(|item| item.contains(x, y, layout.height))
                        .map(|item| item.menu)
                })
                .flatten();
            match selected {
                Some(WorkbenchMenu::More) => {
                    if self.workbench_menu == Some(WorkbenchMenu::More)
                        && self.workbench_menu_overflow
                    {
                        self.close_workbench_menu();
                    } else {
                        self.workbench_menu = Some(WorkbenchMenu::More);
                        self.workbench_menu_overflow = true;
                        self.workbench_overflow_selection = 0;
                        self.activity_bar_menu = None;
                        self.settings_menu = None;
                        self.profile_menu = None;
                    }
                }
                Some(selected) => {
                    if self.workbench_menu == Some(selected) && !self.workbench_menu_overflow {
                        self.close_workbench_menu();
                    } else {
                        self.workbench_menu = Some(selected);
                        self.workbench_menu_overflow = false;
                        self.activity_bar_menu = None;
                        self.settings_menu = None;
                        self.profile_menu = None;
                        if selected == WorkbenchMenu::File {
                            self.file_menu_submenu = None;
                            self.file_menu_nested = None;
                            self.file_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Edit {
                            self.edit_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Selection {
                            self.selection_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Terminal {
                            self.terminal_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Run {
                            self.open_run_menu();
                        } else if selected == WorkbenchMenu::Help {
                            self.help_menu_selection = 0;
                        } else if selected == WorkbenchMenu::View {
                            self.view_menu_open_at(layout.menu_anchor(selected), width, height);
                        }
                    }
                }
                None => self.close_workbench_menu(),
            }
            return true;
        }

        let Some(active) = self.workbench_menu else {
            return false;
        };
        if active == WorkbenchMenu::More && self.workbench_menu_overflow {
            let popup = layout.overflow_popup(char_width, width);
            let index = ((y - popup.y - 4.0) / OVERFLOW_ROW_HEIGHT) as usize;
            if popup.contains(x, y) && index < layout.overflowed.len() && y >= popup.y + 4.0 {
                let selected = layout.overflowed[index];
                self.workbench_menu = Some(selected);
                self.workbench_menu_overflow = false;
                if selected == WorkbenchMenu::Terminal {
                    self.terminal_menu_selection = 0;
                }
                if selected == WorkbenchMenu::Selection {
                    self.selection_menu_selection = 0;
                }
                if selected == WorkbenchMenu::Run {
                    self.open_run_menu();
                }
                if selected == WorkbenchMenu::View {
                    self.view_menu_open_at(popup.x, width, height);
                }
                return true;
            }
            self.close_workbench_menu();
        } else if active == WorkbenchMenu::Selection {
            let popup_width = self.selection_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Selection)
                .clamp(0.0, (width - popup_width).max(0.0));
            let popup_height =
                super::selection_menu::selection_menu_height(&self.selection_menu_rows());
            if x >= popup_x
                && x < popup_x + popup_width
                && y >= layout.height
                && y < layout.height + popup_height
            {
                if let Some((command, true)) =
                    self.selection_menu_command_at(x, y, popup_x, layout.height, popup_width)
                {
                    self.execute_selection_menu_command(command);
                    self.close_workbench_menu();
                }
            } else {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::Edit {
            let popup_width = super::edit_menu::EDIT_MENU_WIDTH.min(width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Edit)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some((command, enabled)) =
                self.edit_menu_command_at(x, y, popup_x, layout.height)
            {
                if enabled {
                    self.execute_edit_menu_command(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::File {
            let popup_width = super::file_menu::FILE_MENU_WIDTH.min(width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::File)
                .clamp(0.0, (width - popup_width).max(0.0));
            let (submenu_x, nested_x, submenu_width) =
                Self::file_panel_xs(popup_x, popup_width, width);
            if let Some((command, enabled)) = self.file_nested_command_at(x, y, nested_x, height) {
                if enabled {
                    self.execute_settings_subcommand(command);
                    self.close_workbench_menu();
                }
                return true;
            }
            if let Some((command, submenu, enabled)) =
                self.file_menu_command_at(x, y, popup_x, submenu_x, nested_x, height)
            {
                if !enabled {
                    return true;
                }
                if let Some(submenu) = submenu {
                    self.open_file_submenu(submenu);
                } else {
                    self.execute_file_menu_command(command);
                    self.close_workbench_menu();
                }
            } else {
                let inside_panel = |app: &Self, open: Option<_>, left: f32, top: f32| {
                    let panel_height = open.map_or(0.0, |open| app.file_panel_height(open));
                    open.is_some()
                        && x >= left
                        && x < left + submenu_width
                        && y >= top
                        && y < top + panel_height.max(250.0)
                };
                let in_submenu = inside_panel(
                    self,
                    self.file_menu_submenu,
                    submenu_x,
                    self.file_submenu_top(height),
                ) || inside_panel(
                    self,
                    self.file_menu_nested,
                    nested_x,
                    self.file_nested_top(height),
                );
                if !in_submenu {
                    self.close_workbench_menu();
                }
            }
        } else if active == WorkbenchMenu::Go {
            let popup_width = self.go_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Go)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some(submenu) = self.go_submenu {
                let sub_width = self.go_submenu_width(submenu, char_width, width);
                let sub_x = if popup_x + popup_width + sub_width <= width {
                    popup_x + popup_width
                } else {
                    (popup_x - sub_width).max(0.0)
                };
                let sub_y = self.go_submenu_anchor_y(layout.height, popup_x, popup_width);
                if let Some((command, true)) =
                    self.go_submenu_command_at(submenu, x, y, sub_x, sub_y, sub_width)
                {
                    self.go_submenu_execute(command);
                } else if x >= popup_x && x < popup_x + popup_width {
                    if let Some((command, enabled, child)) =
                        self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
                    {
                        if let Some(child) = child {
                            self.go_submenu = Some(child);
                            self.go_submenu_selection = 0;
                        } else if enabled {
                            self.go_menu_execute(command);
                            self.close_workbench_menu();
                        }
                    }
                } else if x < sub_x || x >= sub_x + sub_width || y < layout.height {
                    self.close_workbench_menu();
                }
            } else if let Some((command, enabled, submenu)) =
                self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
            {
                if let Some(submenu) = submenu {
                    self.go_submenu = Some(submenu);
                    self.go_submenu_selection = 0;
                } else if enabled {
                    self.go_menu_execute(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::Terminal {
            let popup_width = super::terminal_menu::TERMINAL_MENU_WIDTH.min(width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Terminal)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some((command, enabled)) =
                self.terminal_menu_command_at(x, y, popup_x, layout.height)
            {
                if enabled {
                    self.execute_terminal_menu_command(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::Run {
            let popup_width = self.run_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Run)
                .clamp(0.0, (width - popup_width).max(0.0));
            let submenu_width = self.run_breakpoint_menu_width(char_width, width);
            let submenu_x =
                self.run_breakpoint_submenu_position(popup_x, popup_width, width, submenu_width);
            let submenu_height = self
                .run_breakpoint_rows()
                .iter()
                .map(|row| {
                    if matches!(row, super::run_menu::RunMenuRow::Separator) {
                        9.0
                    } else {
                        28.0
                    }
                })
                .sum::<f32>()
                + 8.0;
            let submenu_top = self.run_breakpoint_submenu_top(layout.height, height);
            let in_submenu = self.run_breakpoint_submenu
                && x >= submenu_x
                && x < submenu_x + submenu_width
                && y >= submenu_top
                && y < submenu_top + submenu_height;
            if in_submenu {
                if let Some((command, enabled)) =
                    self.run_breakpoint_command_at(x, y, submenu_x, submenu_top, submenu_width)
                {
                    if enabled {
                        self.execute_run_menu_command(command);
                        self.close_workbench_menu();
                    } else {
                        self.show_run_menu_unavailable_reason(command, true);
                    }
                }
            } else if let Some((command, enabled, submenu)) =
                self.run_menu_command_at(x, y, popup_x, layout.height, popup_width)
            {
                if submenu {
                    self.run_breakpoint_submenu = true;
                    self.run_breakpoint_selection = 0;
                } else if enabled {
                    self.execute_run_menu_command(command);
                    self.close_workbench_menu();
                } else {
                    self.show_run_menu_unavailable_reason(command, false);
                }
            } else {
                let main_height = self
                    .run_menu_rows()
                    .iter()
                    .map(|row| {
                        if matches!(row, super::run_menu::RunMenuRow::Separator) {
                            9.0
                        } else {
                            28.0
                        }
                    })
                    .sum::<f32>()
                    + 8.0;
                let in_main = x >= popup_x
                    && x < popup_x + popup_width
                    && y >= layout.height
                    && y < layout.height + main_height;
                if !in_main {
                    self.close_workbench_menu();
                }
            }
        } else if active == WorkbenchMenu::Help {
            let popup_width = self.help_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Help)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some((command, enabled)) =
                self.help_menu_command_at(x, y, popup_x, layout.height, popup_width)
            {
                if enabled {
                    self.execute_help_command(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::View {
            let _ = self.view_menu_click(x, y);
            if self.view_menu.is_none() {
                self.close_workbench_menu();
            }
        } else {
            self.close_workbench_menu();
        }
        true
    }

    pub(crate) fn workbench_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let Some((char_width, width, height)) = self.gpu.as_ref().map(|gpu| {
            (
                gpu.renderer.char_width(),
                gpu.renderer.size().0,
                gpu.renderer.size().1,
            )
        }) else {
            return;
        };
        let layout = self.titlebar_layout(width, char_width);
        let Some(active) = self.workbench_menu else {
            return;
        };
        if active == WorkbenchMenu::More && self.workbench_menu_overflow {
            let popup = layout.overflow_popup(char_width, width);
            if popup.contains(x, y) && y >= popup.y + 4.0 {
                self.workbench_overflow_selection = (((y - popup.y - 4.0) / OVERFLOW_ROW_HEIGHT)
                    as usize)
                    .min(layout.overflowed.len().saturating_sub(1));
            }
            return;
        }
        if y >= layout.height {
            if active == WorkbenchMenu::Go {
                let popup_width = self.go_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Go)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let in_main = x >= popup_x && x < popup_x + popup_width;
                if let Some(submenu) = self.go_submenu {
                    let sub_width = self.go_submenu_width(submenu, char_width, width);
                    let sub_x = if popup_x + popup_width + sub_width <= width {
                        popup_x + popup_width
                    } else {
                        (popup_x - sub_width).max(0.0)
                    };
                    let sub_y = self.go_submenu_anchor_y(layout.height, popup_x, popup_width);
                    if x >= sub_x && x < sub_x + sub_width && y >= sub_y {
                        self.go_submenu_pointer_selection(submenu, y, sub_y);
                    } else if in_main {
                        self.go_menu_pointer_selection(y, layout.height);
                        if let Some((_, _, hovered)) =
                            self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
                        {
                            if hovered != self.go_submenu {
                                self.go_submenu = hovered;
                                self.go_submenu_selection = 0;
                            }
                        }
                    }
                } else if in_main {
                    self.go_menu_pointer_selection(y, layout.height);
                    if let Some((_, _, Some(submenu))) =
                        self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
                    {
                        self.go_submenu = Some(submenu);
                        self.go_submenu_selection = 0;
                    }
                }
            } else if active == WorkbenchMenu::Selection {
                let popup_width = self.selection_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Selection)
                    .clamp(0.0, (width - popup_width).max(0.0));
                if x >= popup_x && x < popup_x + popup_width {
                    self.selection_menu_pointer_selection(y, layout.height);
                }
            } else if active == WorkbenchMenu::Edit {
                let popup_width = super::edit_menu::EDIT_MENU_WIDTH.min(width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Edit)
                    .clamp(0.0, (width - popup_width).max(0.0));
                if x >= popup_x && x < popup_x + popup_width {
                    self.edit_menu_pointer_selection(y, layout.height);
                }
            } else if active == WorkbenchMenu::File {
                let popup_width = super::file_menu::FILE_MENU_WIDTH.min(width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::File)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let (submenu_x, nested_x, submenu_width) =
                    Self::file_panel_xs(popup_x, popup_width, width);
                let in_nested = self.file_menu_nested.is_some()
                    && x >= nested_x
                    && x < nested_x + submenu_width;
                let in_submenu = !in_nested
                    && self.file_menu_submenu.is_some()
                    && x >= submenu_x
                    && x < submenu_x + submenu_width;
                let hovered = self
                    .file_menu_command_at(x, y, popup_x, submenu_x, nested_x, height)
                    .and_then(|(_, submenu, enabled)| enabled.then_some(submenu).flatten());
                if in_nested {
                    self.file_menu_pointer_selection(y, self.file_menu_nested, height);
                } else if in_submenu {
                    self.file_menu_pointer_selection(y, self.file_menu_submenu, height);
                    // Hovering Profiles or Themes opens them beside their row; any other row
                    // closes the nested submenu.
                    match hovered {
                        Some(child) if self.file_menu_nested != Some(child) => {
                            self.open_file_submenu(child)
                        }
                        Some(_) => {}
                        None => self.file_menu_nested = None,
                    }
                } else if x >= popup_x && x < popup_x + popup_width {
                    if hovered.is_none() || hovered != self.file_menu_submenu {
                        self.file_menu_pointer_selection(y, None, height);
                    }
                    if hovered != self.file_menu_submenu {
                        self.file_menu_submenu = hovered;
                        self.file_menu_nested = None;
                        if hovered.is_some() {
                            self.file_menu_selection = 0;
                        }
                    }
                }
            } else if active == WorkbenchMenu::Terminal {
                let popup_width = super::terminal_menu::TERMINAL_MENU_WIDTH.min(width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Terminal)
                    .clamp(0.0, (width - popup_width).max(0.0));
                if x >= popup_x && x < popup_x + popup_width {
                    self.terminal_menu_pointer_selection(y, layout.height);
                }
            } else if active == WorkbenchMenu::Run {
                let popup_width = self.run_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Run)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let submenu_width = self.run_breakpoint_menu_width(char_width, width);
                let submenu_x = self.run_breakpoint_submenu_position(
                    popup_x,
                    popup_width,
                    width,
                    submenu_width,
                );
                let submenu_height = self
                    .run_breakpoint_rows()
                    .iter()
                    .map(|row| {
                        if matches!(row, super::run_menu::RunMenuRow::Separator) {
                            9.0
                        } else {
                            28.0
                        }
                    })
                    .sum::<f32>()
                    + 8.0;
                let submenu_top = self.run_breakpoint_submenu_top(layout.height, height);
                if self.run_breakpoint_submenu
                    && x >= submenu_x
                    && x < submenu_x + submenu_width
                    && y < submenu_top + submenu_height
                {
                    self.run_menu_pointer_selection(y, submenu_top, true);
                } else if x >= popup_x && x < popup_x + popup_width {
                    self.run_menu_pointer_selection(y, layout.height, false);
                }
            } else if active == WorkbenchMenu::Help {
                let popup_width = self.help_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Help)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let mut top = layout.height + 4.0;
                let mut item = 0;
                for row in self.help_menu_rows() {
                    match row {
                        super::help_menu::HelpRow::Separator => top += 9.0,
                        super::help_menu::HelpRow::Item { .. } => {
                            if x >= popup_x
                                && x < popup_x + popup_width
                                && y >= top
                                && y < top + 28.0
                            {
                                self.help_menu_selection = item;
                            }
                            top += 28.0;
                            item += 1;
                        }
                    }
                }
            } else if active == WorkbenchMenu::View {
                self.view_menu_pointer_moved(x, y);
            }
            return;
        }
        if let Some(item) = layout
            .menus
            .into_iter()
            .find(|item| item.contains(x, y, layout.height))
        {
            if item.menu == WorkbenchMenu::More {
                if !self.workbench_menu_overflow {
                    self.workbench_menu = Some(WorkbenchMenu::More);
                    self.workbench_menu_overflow = true;
                    self.workbench_overflow_selection = 0;
                    self.activity_bar_menu = None;
                    self.settings_menu = None;
                    self.profile_menu = None;
                }
            } else if item.menu != active || self.workbench_menu_overflow {
                self.workbench_menu = Some(item.menu);
                self.workbench_menu_overflow = false;
                self.activity_bar_menu = None;
                self.settings_menu = None;
                self.profile_menu = None;
                if item.menu == WorkbenchMenu::Run {
                    self.open_run_menu();
                }
                if item.menu == WorkbenchMenu::Terminal {
                    self.terminal_menu_selection = 0;
                }
                if item.menu == WorkbenchMenu::Selection {
                    self.selection_menu_selection = 0;
                }
                if item.menu == WorkbenchMenu::Go {
                    self.go_menu_selection = 0;
                    self.go_submenu = None;
                }
                if item.menu == WorkbenchMenu::View {
                    self.view_menu_open_at(item.x, width, height);
                }
            }
        }
    }

    pub(crate) fn workbench_menu_key(&mut self, key: &Key) -> bool {
        if let Some(active) = self.workbench_menu {
            if active == WorkbenchMenu::View && self.view_menu.is_some() {
                return self.view_menu_key(key);
            }
            if active == WorkbenchMenu::File {
                match key {
                    Key::Named(NamedKey::Escape) => {
                        if !self.close_file_submenu() {
                            self.close_workbench_menu();
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => self.file_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.file_menu_move_selection(-1),
                    Key::Named(NamedKey::ArrowRight) => {
                        if let Some((_, Some(submenu), _)) = self.file_menu_selected() {
                            self.open_file_submenu(submenu);
                        } else if self.file_menu_submenu.is_none() {
                            self.workbench_menu = Some(WorkbenchMenu::Edit);
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if !self.close_file_submenu() {
                            self.workbench_menu = Some(WorkbenchMenu::Help);
                        }
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space)
                        if self.file_menu_nested.is_some() =>
                    {
                        if let Some((command, true)) = self.file_nested_selected() {
                            self.execute_settings_subcommand(command);
                            self.close_workbench_menu();
                        }
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, submenu, enabled)) = self.file_menu_selected() {
                            if enabled {
                                if let Some(submenu) = submenu {
                                    self.open_file_submenu(submenu);
                                } else {
                                    self.execute_file_menu_command(command);
                                    self.close_workbench_menu();
                                }
                            }
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            // Left/Right fall through to the shared titlebar cycling below.
            if active == WorkbenchMenu::Terminal
                && !matches!(
                    key,
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight)
                )
            {
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => self.terminal_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.terminal_menu_move_selection(-1),
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, true)) = self.terminal_menu_selected() {
                            self.execute_terminal_menu_command(command);
                            self.close_workbench_menu();
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::Run
                && (self.run_breakpoint_submenu
                    || !matches!(
                        key,
                        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight)
                    )
                    || (matches!(key, Key::Named(NamedKey::ArrowRight))
                        && self
                            .run_menu_selected(false)
                            .is_some_and(|(_, enabled, submenu)| enabled && submenu)))
            {
                if self.run_breakpoint_submenu {
                    match key {
                        Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                            self.run_breakpoint_submenu = false
                        }
                        Key::Named(NamedKey::ArrowDown) => self.run_menu_move_selection(1, true),
                        Key::Named(NamedKey::ArrowUp) => self.run_menu_move_selection(-1, true),
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, enabled, _)) = self.run_menu_selected(true) {
                                if enabled {
                                    self.execute_run_menu_command(command);
                                    self.close_workbench_menu();
                                } else {
                                    self.show_run_menu_unavailable_reason(command, true);
                                }
                            }
                        }
                        _ => return false,
                    }
                } else {
                    match key {
                        Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                        Key::Named(NamedKey::ArrowDown) => self.run_menu_move_selection(1, false),
                        Key::Named(NamedKey::ArrowUp) => self.run_menu_move_selection(-1, false),
                        Key::Named(NamedKey::ArrowRight) => {
                            if self
                                .run_menu_selected(false)
                                .is_some_and(|(_, enabled, submenu)| enabled && submenu)
                            {
                                self.run_breakpoint_submenu = true;
                                self.run_breakpoint_selection = 0;
                            } else {
                                return false;
                            }
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, enabled, submenu)) = self.run_menu_selected(false)
                            {
                                if enabled && submenu {
                                    self.run_breakpoint_submenu = true;
                                    self.run_breakpoint_selection = 0;
                                } else if enabled {
                                    self.execute_run_menu_command(command);
                                    self.close_workbench_menu();
                                } else {
                                    self.show_run_menu_unavailable_reason(command, false);
                                }
                            }
                        }
                        _ => return false,
                    }
                }
                return true;
            }
            if active == WorkbenchMenu::Help
                && !matches!(
                    key,
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight)
                )
            {
                let count = self
                    .help_menu_rows()
                    .iter()
                    .filter(|row| matches!(row, super::help_menu::HelpRow::Item { .. }))
                    .count();
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => {
                        self.help_menu_selection =
                            (self.help_menu_selection + 1).min(count.saturating_sub(1))
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.help_menu_selection = self.help_menu_selection.saturating_sub(1)
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        self.activate_help_menu_accessibility_item(self.help_menu_selection)
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::Selection {
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => self.selection_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.selection_menu_move_selection(-1),
                    Key::Named(NamedKey::ArrowRight) => {
                        self.workbench_menu = Some(WorkbenchMenu::View)
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.workbench_menu = Some(WorkbenchMenu::Edit)
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, true, _)) = self
                            .selection_menu_items()
                            .get(self.selection_menu_selection)
                            .copied()
                        {
                            self.execute_selection_menu_command(command);
                            self.close_workbench_menu();
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::Go {
                if let Some(submenu) = self.go_submenu {
                    match key {
                        Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                            self.go_submenu = None;
                            self.go_submenu_selection = 0;
                        }
                        Key::Named(NamedKey::ArrowDown) => self.go_submenu_move_selection(1),
                        Key::Named(NamedKey::ArrowUp) => self.go_submenu_move_selection(-1),
                        Key::Named(NamedKey::ArrowRight) => {
                            self.workbench_menu = Some(WorkbenchMenu::Run)
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, true)) = self.go_submenu_selected() {
                                self.go_submenu_execute(command);
                            }
                        }
                        _ => return false,
                    }
                    let _ = submenu;
                } else {
                    match key {
                        Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                        Key::Named(NamedKey::ArrowDown) => self.go_menu_move_selection(1),
                        Key::Named(NamedKey::ArrowUp) => self.go_menu_move_selection(-1),
                        Key::Named(NamedKey::ArrowRight) => {
                            if let Some((_, true, Some(submenu))) = self.go_menu_selected() {
                                self.go_submenu = Some(submenu);
                                self.go_submenu_selection = 0;
                            } else {
                                self.workbench_menu = Some(WorkbenchMenu::Run);
                            }
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.workbench_menu = Some(WorkbenchMenu::Selection)
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, true, submenu)) = self.go_menu_selected() {
                                if let Some(submenu) = submenu {
                                    self.go_submenu = Some(submenu);
                                    self.go_submenu_selection = 0;
                                } else {
                                    self.go_menu_execute(command);
                                    self.close_workbench_menu();
                                }
                            }
                        }
                        _ => return false,
                    }
                }
                return true;
            }
            if active == WorkbenchMenu::Edit {
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => self.edit_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.edit_menu_move_selection(-1),
                    Key::Named(NamedKey::ArrowRight) => {
                        self.workbench_menu = Some(WorkbenchMenu::Selection)
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.workbench_menu = Some(WorkbenchMenu::File)
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, true)) = self.edit_menu_selected() {
                            self.execute_edit_menu_command(command);
                            self.close_workbench_menu();
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::More && self.workbench_menu_overflow {
                let layout = self.gpu.as_ref().map(|gpu| {
                    self.titlebar_layout(gpu.renderer.size().0, gpu.renderer.char_width())
                });
                let entries = layout
                    .as_ref()
                    .map_or(&[][..], |layout| layout.overflowed.as_slice());
                if entries.is_empty() {
                    self.close_workbench_menu();
                    return true;
                }
                match key {
                    Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                        self.close_workbench_menu();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.workbench_overflow_selection =
                            (self.workbench_overflow_selection + 1) % entries.len();
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.workbench_overflow_selection =
                            (self.workbench_overflow_selection + entries.len() - 1) % entries.len();
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        let selected =
                            entries[self.workbench_overflow_selection.min(entries.len() - 1)];
                        let (width, height) = self.gpu.as_ref().map_or((1200.0, 760.0), |gpu| {
                            let (width, height) = gpu.renderer.size();
                            (width, height)
                        });
                        let char_width = self
                            .gpu
                            .as_ref()
                            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
                        let anchor = self
                            .titlebar_layout(width, char_width)
                            .menu_anchor(selected);
                        self.activate_menu(selected, anchor, width, height);
                    }
                    _ => return false,
                }
                return true;
            }
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.close_workbench_menu();
                }
                Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight) => {
                    let entries = self.gpu.as_ref().map_or_else(
                        || WorkbenchMenu::ALL.to_vec(),
                        |gpu| {
                            self.titlebar_layout(gpu.renderer.size().0, gpu.renderer.char_width())
                                .menus
                                .iter()
                                .map(|item| item.menu)
                                .collect()
                        },
                    );
                    let index = entries.iter().position(|menu| *menu == active).unwrap_or(0);
                    let next = if matches!(key, Key::Named(NamedKey::ArrowLeft)) {
                        (index + entries.len() - 1) % entries.len()
                    } else {
                        (index + 1) % entries.len()
                    };
                    self.workbench_menu = Some(entries[next]);
                    self.workbench_menu_overflow = false;
                    self.activity_bar_menu = None;
                    self.settings_menu = None;
                    self.profile_menu = None;
                    let selected = entries[next];
                    if selected == WorkbenchMenu::Selection {
                        self.selection_menu_selection = 0;
                    }
                    if selected == WorkbenchMenu::Terminal {
                        self.terminal_menu_selection = 0;
                    }
                    if selected == WorkbenchMenu::View {
                        let (width, height, char_width) = self.gpu.as_ref().map_or(
                            (1200.0, 760.0, self.font_size * 0.6),
                            |gpu| {
                                let (width, height) = gpu.renderer.size();
                                (width, height, gpu.renderer.char_width())
                            },
                        );
                        let anchor = self
                            .titlebar_layout(width, char_width)
                            .menu_anchor(selected);
                        self.activate_menu(selected, anchor, width, height);
                    }
                }
                Key::Named(NamedKey::Enter) | Key::Named(NamedKey::ArrowDown) => {
                    if active == WorkbenchMenu::More {
                        self.workbench_menu_overflow = true;
                        self.workbench_overflow_selection = 0;
                    } else if active == WorkbenchMenu::View {
                        let (width, height, char_width) = self.gpu.as_ref().map_or(
                            (1200.0, 760.0, self.font_size * 0.6),
                            |gpu| {
                                let (width, height) = gpu.renderer.size();
                                (width, height, gpu.renderer.char_width())
                            },
                        );
                        let anchor = self.titlebar_layout(width, char_width).menu_anchor(active);
                        self.activate_menu(active, anchor, width, height);
                    } else if active == WorkbenchMenu::File {
                        self.file_menu_selection = 0;
                    } else if active == WorkbenchMenu::Edit {
                        self.edit_menu_selection = 0;
                    }
                }
                _ => return false,
            }
            return true;
        }
        if matches!(key, Key::Named(NamedKey::F10)) {
            self.workbench_menu = Some(WorkbenchMenu::File);
            self.workbench_menu_overflow = false;
            self.file_menu_submenu = None;
            self.file_menu_nested = None;
            self.file_menu_selection = 0;
            return true;
        }
        false
    }
}
