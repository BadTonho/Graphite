use super::editor_groups::GroupRect;
use super::*;

pub(crate) fn ruler_positions(
    columns: &[usize],
    text_x: f32,
    char_width: f32,
    width: f32,
) -> Vec<f32> {
    columns
        .iter()
        .map(|column| text_x + *column as f32 * char_width)
        .filter(|x| *x < width)
        .collect()
}

impl App {
    pub(crate) fn activity_bar_width_px(&self) -> f32 {
        match self.activity_bar_preferences.position {
            config::ActivityBarPosition::Top
            | config::ActivityBarPosition::Bottom
            | config::ActivityBarPosition::Hidden => 0.0,
            _ if self.activity_bar_preferences.size == config::ActivityBarSize::Compact => 36.0,
            _ => ACTIVITY_BAR_WIDTH,
        }
    }

    pub(crate) fn sidebar_translation(&self, window_width: f32) -> f32 {
        let bar = self.activity_bar_width_px();
        let content = if self.activity_bar_preferences.primary_sidebar_right {
            (window_width - self.sidebar_width_px()).max(0.0)
        } else {
            (self.sidebar_width_px() - bar).max(0.0)
        };
        if self.activity_bar_preferences.primary_sidebar_right {
            window_width - content - ACTIVITY_BAR_WIDTH
        } else {
            bar - ACTIVITY_BAR_WIDTH
        }
    }

    pub(crate) fn sidebar_layout_width_px(&self) -> f32 {
        let bar = self.activity_bar_width_px();
        let content = if self.activity_bar_preferences.primary_sidebar_right {
            self.sidebar_window_width_px - self.sidebar_width_px()
        } else {
            self.sidebar_width_px() - bar
        };
        ACTIVITY_BAR_WIDTH + content.max(0.0)
    }

    pub(crate) fn primary_sidebar_bounds(&self, window_width: f32) -> (f32, f32) {
        if !self.view_preferences.primary_sidebar_visible {
            return (0.0, 0.0);
        }
        let content = (self.sidebar_layout_width_px() - ACTIVITY_BAR_WIDTH).max(0.0);
        if self.activity_bar_preferences.primary_sidebar_right {
            (window_width - content, content)
        } else {
            (self.activity_bar_width_px(), content)
        }
    }

    pub(crate) fn primary_sidebar_contains(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> bool {
        if !self.view_preferences.primary_sidebar_visible {
            return false;
        }
        let (left, panel_width) = self.primary_sidebar_bounds(width);
        x >= left
            && x < left + panel_width
            && y >= self.activity_bar_vertical_offset()
            && y < height - line_height
    }

    pub(crate) fn sidebar_local_x(&self, x: f32, window_width: f32) -> f32 {
        x - self.sidebar_translation(window_width)
    }

    pub(crate) fn sidebar_local_y(&self, y: f32) -> f32 {
        y - self.activity_bar_vertical_offset()
    }

    pub(crate) fn activity_bar_vertical_offset(&self) -> f32 {
        let menu_bar_height = self.top_menu_height();
        if self.activity_bar_preferences.position == config::ActivityBarPosition::Top {
            if self.activity_bar_preferences.size == config::ActivityBarSize::Compact {
                menu_bar_height + 40.0
            } else {
                menu_bar_height + 48.0
            }
        } else {
            menu_bar_height
        }
    }

    pub(crate) fn sidebar_width_px(&self) -> f32 {
        let bar = self.activity_bar_width_px();
        let content = effective_sidebar_content_width(
            self.sidebar_width,
            self.sidebar_scale_factor,
            self.sidebar_window_width_px,
            self.secondary_sidebar_width(),
            bar,
        );
        if self.activity_bar_preferences.primary_sidebar_right {
            self.sidebar_window_width_px - content * self.sidebar_scale_factor
        } else {
            bar + content * self.sidebar_scale_factor
        }
    }

    pub(crate) fn resize_sidebar_from_pointer(&mut self, pointer_x: f32) {
        let scale = self.sidebar_scale_factor;
        let raw_width = if self.activity_bar_preferences.primary_sidebar_right {
            (self.sidebar_window_width_px - pointer_x) / scale
        } else {
            (pointer_x - self.activity_bar_width_px()) / scale
        };
        let available = max_sidebar_content_width(
            scale,
            self.sidebar_window_width_px,
            self.secondary_sidebar_width(),
        );
        self.sidebar_width = if available >= config::MIN_SIDEBAR_WIDTH {
            config::clamp_sidebar_width(raw_width).min(available)
        } else {
            raw_width.clamp(0.0, available)
        };
    }

    /// The x position of the draggable edge of the secondary side bar, which faces the editor.
    pub(crate) fn secondary_separator_x(&self, window_width: f32) -> f32 {
        if self.activity_bar_preferences.primary_sidebar_right {
            self.secondary_sidebar_x(window_width) + self.secondary_sidebar_width()
        } else {
            self.secondary_sidebar_x(window_width)
        }
    }

    pub(crate) fn is_over_secondary_resize_handle(&self, pointer_x: f32, pointer_y: f32) -> bool {
        self.secondary_sidebar_width() > 0.0
            && pointer_y >= self.activity_bar_vertical_offset()
            && (pointer_x - self.secondary_separator_x(self.sidebar_window_width_px)).abs() <= 4.0
    }

    pub(crate) fn resize_secondary_sidebar_from_pointer(&mut self, pointer_x: f32) {
        let width = if self.activity_bar_preferences.primary_sidebar_right {
            pointer_x - self.secondary_sidebar_x(self.sidebar_window_width_px)
        } else {
            self.sidebar_window_width_px - pointer_x
        };
        self.secondary_sidebar_user_width = Some(width.clamp(
            config::MIN_SECONDARY_SIDEBAR_WIDTH,
            self.secondary_sidebar_max_width(),
        ));
    }

    pub(crate) fn visible_lines(&self, renderer: &Renderer) -> usize {
        let (width, height) = renderer.size();
        let rect = self.focused_group_rect(width, height, renderer.line_height());
        (((rect.height - renderer.line_height() * 1.5).max(0.0) / renderer.line_height()) as usize)
            .max(1)
    }

    pub(crate) fn group_rectangles(
        &self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> Vec<GroupRect> {
        let sidebar_boundary = self.sidebar_width_px();
        let activity_width = self.activity_bar_width_px();
        let primary_width = if !self.view_preferences.primary_sidebar_visible {
            0.0
        } else if self.activity_bar_preferences.primary_sidebar_right {
            (width - sidebar_boundary).max(0.0)
        } else {
            (sidebar_boundary - activity_width).max(0.0)
        };
        let has_panel = self.view_preferences.panel_visible
            && (self.terminal_visible || self.phase6_panel_visible());
        let panel =
            if !has_panel || self.view_preferences.panel_position == config::PanelPosition::Right {
                0.0
            } else if self.terminal_visible {
                terminal_panel_height_with_delta(height, self.view_preferences.panel_height_delta)
            } else if self.phase6_panel_visible() {
                line_height * 9.0
            } else {
                0.0
            };
        let right_panel_width =
            if has_panel && self.view_preferences.panel_position == config::PanelPosition::Right {
                (width * 0.32).min(520.0)
            } else {
                0.0
            };
        let sec_width = self.secondary_sidebar_width();
        let (left_sidebar, right_sidebar) = if self.zen_mode {
            (0.0, 0.0)
        } else {
            let primary_right = self.activity_bar_preferences.primary_sidebar_right;
            (
                if primary_right {
                    activity_width + sec_width
                } else {
                    if self.view_preferences.primary_sidebar_visible {
                        sidebar_boundary
                    } else {
                        activity_width
                    }
                },
                if primary_right && self.view_preferences.primary_sidebar_visible {
                    primary_width + right_panel_width
                } else if primary_right {
                    right_panel_width
                } else {
                    sec_width + right_panel_width
                },
            )
        };
        let editor_left = left_sidebar;
        let editor_right = (width - right_sidebar).max(editor_left);
        let available_editor_width = (editor_right - editor_left).max(0.0);
        let centered = self.centered_editor || self.zen_mode;
        let editor_width = if centered {
            let cw = self
                .gpu
                .as_ref()
                .map(|gpu| gpu.renderer.char_width())
                .unwrap_or(8.0);
            (cw * 80.0 + self.gutter_width(cw)).min(available_editor_width)
        } else {
            available_editor_width
        };
        let x = if centered {
            editor_left + (available_editor_width - editor_width) / 2.0
        } else {
            editor_left
        };
        let vertical_offset = self.activity_bar_vertical_offset();
        let bottom_offset =
            if self.activity_bar_preferences.position == config::ActivityBarPosition::Bottom {
                if self.activity_bar_preferences.size == config::ActivityBarSize::Compact {
                    40.0
                } else {
                    48.0
                }
            } else {
                0.0
            };
        self.editor_groups.rectangles(GroupRect {
            id: 0,
            x,
            y: vertical_offset,
            width: editor_width.max(0.0),
            height: (height
                - panel
                - if self.zen_mode || !self.view_preferences.status_bar_visible {
                    0.0
                } else {
                    line_height
                }
                - vertical_offset
                - bottom_offset)
                .max(0.0),
        })
    }

    pub(crate) fn panel_bounds(&self, width: f32, height: f32, line_height: f32) -> GroupRect {
        if self.view_preferences.panel_position == config::PanelPosition::Right {
            let x = (width - (width * 0.32).min(520.0)).max(0.0);
            let y = self.top_menu_height();
            return GroupRect {
                id: 0,
                x,
                y,
                width: width - x,
                height: (height
                    - y
                    - if self.view_preferences.status_bar_visible {
                        line_height
                    } else {
                        0.0
                    })
                .max(0.0),
            };
        }
        if self.terminal_visible {
            let panel_height =
                terminal_panel_height_with_delta(height, self.view_preferences.panel_height_delta);
            let y = height - line_height - panel_height;
            let available_x = self.sidebar_width_px();
            let available_width = (width - available_x).max(0.0);
            let aligned_width = if self.view_preferences.panel_align_center
                || self.view_preferences.panel_align_right
            {
                available_width * 0.7
            } else {
                available_width
            };
            let x = if self.view_preferences.panel_align_center {
                available_x + (available_width - aligned_width) / 2.0
            } else if self.view_preferences.panel_align_right {
                width - aligned_width
            } else {
                available_x
            };
            GroupRect {
                id: 0,
                x,
                y,
                width: aligned_width,
                height: panel_height,
            }
        } else {
            let panel_height = line_height * 9.0;
            GroupRect {
                id: 0,
                x: 0.0,
                y: height - panel_height,
                width,
                height: panel_height,
            }
        }
    }

    pub(crate) fn editor_tab_strip_height(&self, line_height: f32) -> f32 {
        if self.view_preferences.tabs_visible && !self.zen_mode {
            line_height * 1.5
        } else {
            0.0
        }
    }

    pub(crate) fn focused_group_rect(
        &self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> GroupRect {
        self.group_rectangles(width, height, line_height)
            .into_iter()
            .find(|rect| rect.id == self.editor_groups.focused())
            .expect("focused group has a rectangle")
    }

    pub(crate) fn keep_cursor_visible(
        &mut self,
        visible: usize,
        width: f32,
        height: f32,
        lh: f32,
        cw: f32,
    ) {
        let line = if self.word_wrap {
            let rect = self.focused_group_rect(width, height, lh);
            let columns = (((rect.width - self.gutter_width(cw)).max(cw)) / cw) as usize;
            let tab = &self.documents[self.active_document()];
            super::visual_lines::document_position(
                &tab.buffer,
                self.editor().cursor().char_idx,
                columns,
                tab.indentation.tab_size,
                self.wrapping_indent,
            )
            .0
        } else {
            self.editor().cursor_line_col().0
        };
        if line < self.editor_groups.active_view_mut().scroll_line {
            self.editor_groups.active_view_mut().scroll_line = line;
        } else if line >= self.editor_groups.active_view_mut().scroll_line + visible {
            self.editor_groups.active_view_mut().scroll_line = line + 1 - visible;
        }
        let rect = self.focused_group_rect(width, height, lh);
        self.reveal_cursor_column(rect, cw);
    }

    pub(crate) fn gutter_width(&self, char_w: f32) -> f32 {
        let digits = self.documents[self.active_document()]
            .buffer
            .len_lines()
            .to_string()
            .len()
            .max(3);
        digits as f32 * char_w + PAD * 2.0
    }
}

fn max_sidebar_content_width(scale: f32, window_width_px: f32, right_sidebar_width_px: f32) -> f32 {
    let window_width = window_width_px / scale;
    let activity_width = ACTIVITY_BAR_WIDTH / scale;
    let maximum_by_percentage = window_width * 0.70 - activity_width;
    let maximum_by_editor = window_width - activity_width - right_sidebar_width_px / scale - 320.0;
    maximum_by_percentage
        .min(maximum_by_editor)
        .clamp(0.0, config::MAX_PERSISTED_SIDEBAR_WIDTH)
}

fn effective_sidebar_content_width(
    preferred_logical: f32,
    scale: f32,
    window_width_px: f32,
    right_sidebar_width_px: f32,
    activity_width_px: f32,
) -> f32 {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let mut available = max_sidebar_content_width(scale, window_width_px, right_sidebar_width_px);
    available = (available + ACTIVITY_BAR_WIDTH / scale - activity_width_px / scale).max(0.0);
    let min = if available >= config::MIN_SIDEBAR_WIDTH {
        config::MIN_SIDEBAR_WIDTH
    } else {
        0.0
    };
    preferred_logical.clamp(min, available)
}

#[cfg(test)]
fn effective_sidebar_boundary(
    preferred_logical: f32,
    scale: f32,
    window_width_px: f32,
    right_sidebar_width_px: f32,
) -> f32 {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let available = max_sidebar_content_width(scale, window_width_px, right_sidebar_width_px);
    let min = if available >= config::MIN_SIDEBAR_WIDTH {
        config::MIN_SIDEBAR_WIDTH
    } else {
        0.0
    };
    ACTIVITY_BAR_WIDTH + preferred_logical.clamp(min, available) * scale
}

#[cfg(test)]
mod sidebar_width_tests {
    use super::*;

    #[test]
    fn sidebar_boundary_preserves_default_bounds_and_editor_space() {
        assert_eq!(effective_sidebar_boundary(260.0, 1.0, 1400.0, 0.0), 308.0);
        assert_eq!(effective_sidebar_boundary(500.0, 1.0, 800.0, 0.0), 480.0);
        assert_eq!(effective_sidebar_boundary(450.0, 2.0, 1600.0, 0.0), 948.0);
        assert_eq!(effective_sidebar_boundary(260.0, 1.0, 350.0, 0.0), 48.0);
    }

    #[test]
    fn sidebar_maximum_is_seventy_percent_and_preserves_editor_space() {
        assert_eq!(max_sidebar_content_width(1.0, 1920.0, 0.0), 1296.0);
        assert_eq!(effective_sidebar_boundary(2000.0, 1.0, 1920.0, 0.0), 1344.0);
        assert_eq!(max_sidebar_content_width(2.0, 1600.0, 0.0), 456.0);
        assert_eq!(max_sidebar_content_width(1.0, 1600.0, 240.0), 992.0);
        assert_eq!(max_sidebar_content_width(1.0, 600.0, 0.0), 232.0);
    }

    #[test]
    fn dragging_sidebar_clamps_to_preference_limits_and_available_editor_space() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.resize_sidebar_from_pointer(100.0);
        assert_eq!(app.sidebar_width, config::MIN_SIDEBAR_WIDTH);
        assert_eq!(app.sidebar_width_px(), ACTIVITY_BAR_WIDTH + 180.0);

        app.resize_sidebar_from_pointer(900.0);
        assert_eq!(app.sidebar_width, 792.0);
        assert_eq!(app.sidebar_width_px(), 840.0);

        app.sidebar_window_width_px = 600.0;
        app.resize_sidebar_from_pointer(498.0);
        assert_eq!(app.sidebar_width, 232.0);
        assert_eq!(app.sidebar_width_px(), 280.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_alignment_changes_bottom_panel_bounds_without_changing_workspace_width() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.terminal_visible = true;
        let left = app.panel_bounds(1200.0, 800.0, 20.0);
        app.view_preferences.panel_align_center = true;
        let center = app.panel_bounds(1200.0, 800.0, 20.0);
        app.view_preferences.panel_align_center = false;
        app.view_preferences.panel_align_right = true;
        let right = app.panel_bounds(1200.0, 800.0, 20.0);

        assert_eq!(left.x, app.sidebar_width_px());
        assert!(center.x > left.x);
        assert_eq!(center.width, right.width);
        assert!(right.x > center.x);
        assert!((right.x + right.width - 1200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rulers_only_include_columns_inside_editor_width() {
        assert_eq!(
            ruler_positions(&[80, 100, 120], 100.0, 8.0, 950.0),
            [740.0, 900.0]
        );
    }
}
