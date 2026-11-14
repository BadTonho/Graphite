//! The folders served by extension file system providers, listed below the workspace tree in the
//! explorer. They share the row geometry and scrolling of the tree: virtual row `k` is row
//! `tree.rows.len() + k`.

use super::*;

impl App {
    pub(crate) fn draw_virtual_explorer_rows(
        &self,
        r: &mut Renderer,
        sidebar_lh: f32,
        cw: f32,
        height: f32,
    ) {
        let rows = self.virtual_explorer_rows();
        let first = self.tree.rows.len();
        for (k, row) in rows.iter().enumerate() {
            let Some(visual) = (first + k).checked_sub(self.explorer_scroll) else {
                continue;
            };
            let y = sidebar_lh * 1.5 + visual as f32 * sidebar_lh;
            if y > height {
                break;
            }
            let indent = ACTIVITY_BAR_WIDTH + PAD + row.depth as f32 * cw * 2.0;
            let chevron_slot = cw * 1.8;
            let icon_size = (sidebar_lh * 0.85).round();
            if row.dir && !crate::icon_theme::hides_explorer_arrows() {
                let chevron_size = (sidebar_lh * 0.72).round();
                r.draw_icon(
                    if row.expanded {
                        crate::file_icons::CHEVRON_DOWN
                    } else {
                        crate::file_icons::CHEVRON_RIGHT
                    },
                    indent + (chevron_slot - chevron_size) * 0.5,
                    y + (sidebar_lh - chevron_size) * 0.5,
                    chevron_size,
                    rgb(Theme::color(Theme::DIM, 0x616161)),
                );
            }
            let icon_x = indent + chevron_slot;
            let icon = crate::icon_theme::for_entry(&row.name, row.dir, row.expanded, row.depth == 0);
            crate::icon_theme::draw(
                r,
                &icon,
                icon_x + (sidebar_lh - icon_size) * 0.5,
                y + (sidebar_lh - icon_size) * 0.5,
                icon_size,
                |color| color,
            );
            r.draw_text(
                &row.name,
                icon_x + sidebar_lh + 4.0,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }
    }

    /// Activates virtual explorer row `index` (counted from the first virtual row).
    pub(crate) fn activate_virtual_explorer_row(&mut self, index: usize) -> bool {
        let Some(row) = self.virtual_explorer_rows().into_iter().nth(index) else {
            return false;
        };
        self.activate_virtual_row(&row);
        true
    }

    /// The number of rows the explorer lists, tree rows and virtual rows together.
    pub(crate) fn explorer_row_count(&self) -> usize {
        self.tree.rows.len() + self.virtual_explorer_rows().len()
    }
}
