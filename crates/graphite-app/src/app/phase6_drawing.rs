use super::*;
use crate::test_explorer::{TestItem, TestState};

const PANEL_LINES: usize = 9;

impl App {
    pub(crate) fn phase6_panel_visible(&self) -> bool {
        self.debug_panel_visible || self.tests_visible || self.ports_visible
    }

    pub(crate) fn phase6_panel_click(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> bool {
        if !self.view_preferences.panel_visible || !self.phase6_panel_visible() {
            return false;
        }
        let bounds = self.panel_bounds(width, height, line_height);
        if x < bounds.x
            || x >= bounds.x + bounds.width
            || y < bounds.y
            || y >= bounds.y + bounds.height
        {
            return false;
        }
        self.debug_input_focused =
            self.debug_panel_visible && y >= bounds.y + bounds.height - line_height * 1.8;
        if self.debug_panel_visible {
            let geometry = super::debug_panel::DebugPanelGeometry::new(&bounds, line_height);
            let char_width = self
                .gpu
                .as_ref()
                .map_or(8.0, |gpu| gpu.renderer.char_width());
            self.debug_panel_click(&geometry, x, y, char_width);
            return true;
        }
        if self.tests_visible {
            let row = ((y - bounds.y - line_height) / line_height)
                .floor()
                .max(0.0) as usize;
            if let Some(item) = test_items(
                &self
                    .test_explorer
                    .controllers
                    .values()
                    .flat_map(|controller| controller.roots.iter())
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .get(row)
            {
                self.test_explorer.selected = Some(item.1.id.clone());
                return true;
            }
        }
        if self.ports_visible {
            let row = ((y - bounds.y - line_height) / line_height)
                .floor()
                .max(0.0) as usize;
            if let Some(port) = self.ports.listeners.get(row) {
                self.selected_port_process = Some(port.process_id);
                self.status = format!(
                    "{}:{} · process {}",
                    port.address, port.port, port.process_id
                );
                return true;
            }
        }
        let _ = (x, width);
        true
    }

    pub(crate) fn draw_phase6_overlays(
        &self,
        r: &mut Renderer,
        width: f32,
        height: f32,
        line_height: f32,
    ) {
        if !self.view_preferences.panel_visible || !self.phase6_panel_visible() {
            return;
        }
        let bounds = self.panel_bounds(width, height, line_height);
        let top = bounds.y;
        r.set_clip(None);
        r.draw_rect(
            bounds.x,
            top,
            bounds.width,
            bounds.height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        let heading = if self.debug_panel_visible {
            format!(
                "Debug · {} · F5 Continue · F10 Over · F11 Into · Shift+F5 Stop · Enter evaluate · Ctrl+Enter watch",
                debug_state_label(&self.debug.state)
            )
        } else if self.tests_visible {
            format!(
                "Test Explorer · Enter Run · Esc Close · {} controller(s)",
                self.test_explorer.controllers.len()
            )
        } else {
            format!(
                "Local Ports · Ctrl+Shift+P → Refresh Local Ports · {} listener(s)",
                self.ports.listeners.len()
            )
        };
        r.draw_text(
            &heading,
            bounds.x + PAD,
            top + PAD * 0.3,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
        if self.debug_panel_visible {
            self.draw_debug_panel(r, &bounds, line_height);
        } else if self.tests_visible {
            let roots = self
                .test_explorer
                .controllers
                .values()
                .flat_map(|controller| controller.roots.iter().cloned())
                .collect::<Vec<_>>();
            for (index, (depth, item)) in
                test_items(&roots).iter().take(PANEL_LINES - 2).enumerate()
            {
                let y = top + line_height * (index as f32 + 1.0);
                if self.test_explorer.selected.as_deref() == Some(item.id.as_str()) {
                    r.draw_rect(
                        bounds.x,
                        y,
                        bounds.width,
                        line_height,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                r.draw_text(
                    &format!(
                        "{}{} {}",
                        "  ".repeat(*depth),
                        test_state_glyph(&item.state),
                        item.label
                    ),
                    bounds.x + PAD,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        } else {
            for (index, port) in self
                .ports
                .listeners
                .iter()
                .take(PANEL_LINES - 2)
                .enumerate()
            {
                r.draw_text(
                    &format!(
                        "{}:{} · process {} · {}",
                        port.address, port.port, port.process_id, port.state
                    ),
                    bounds.x + PAD,
                    top + line_height * (index as f32 + 1.0),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
    }
}

fn debug_state_label(state: &crate::debugger::DebugState) -> String {
    match state {
        crate::debugger::DebugState::Stopped => "stopped".into(),
        crate::debugger::DebugState::Initializing => "initializing".into(),
        crate::debugger::DebugState::Running => "running".into(),
        crate::debugger::DebugState::Paused { reason, .. } => format!("paused: {reason}"),
        crate::debugger::DebugState::Failed(error) => format!("failed: {error}"),
    }
}

fn test_state_glyph(state: &TestState) -> &'static str {
    match state {
        TestState::Passed => "✓",
        TestState::Failed { .. } | TestState::Errored(_) => "×",
        TestState::Running => "●",
        TestState::Skipped => "–",
        _ => "○",
    }
}

pub(crate) fn test_items(items: &[TestItem]) -> Vec<(usize, TestItem)> {
    fn visit(items: &[TestItem], depth: usize, output: &mut Vec<(usize, TestItem)>) {
        for item in items {
            output.push((depth, item.clone()));
            visit(&item.children, depth + 1, output);
        }
    }
    let mut result = Vec::new();
    visit(items, 0, &mut result);
    result
}
