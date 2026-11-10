//! Shared geometry for the Plugins sidebar.
//!
//! Drawing, hit testing and scrolling all read the same measurements so a card
//! drawn on screen is also the card that receives the click.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PluginsTab {
    Installed,
    OpenVsx,
    Graphite,
}

impl Default for PluginsTab {
    fn default() -> Self {
        Self::Installed
    }
}

pub(super) struct PluginsLayout {
    pub(super) sidebar_x: f32,
    pub(super) width: f32,
    pub(super) title_y: f32,
    pub(super) tabs_y: f32,
    pub(super) tab_height: f32,
    pub(super) search_y: f32,
    pub(super) search_height: f32,
    pub(super) section_y: f32,
    pub(super) message_y: f32,
    pub(super) list_y: f32,
    pub(super) list_bottom: f32,
    pub(super) card_height: f32,
    pub(super) footer_y: f32,
}

impl PluginsLayout {
    pub(super) fn new(height: f32, line_height: f32, sidebar_width: f32) -> Self {
        let sidebar_x = ACTIVITY_BAR_WIDTH;
        let width = sidebar_width - ACTIVITY_BAR_WIDTH;
        let title_y = line_height * 0.2;
        let tabs_y = line_height * 1.1;
        let tab_height = line_height * 1.6;
        let search_y = tabs_y + tab_height + line_height * 0.35;
        let search_height = line_height * 1.6;
        let section_y = search_y + search_height + line_height * 0.5;
        let message_y = section_y + line_height * 1.15;
        let list_y = message_y + line_height * 1.15;
        let content_bottom = (height - line_height).max(0.0);
        let footer_y = (content_bottom - line_height * 5.4).max(list_y + line_height * 2.0);
        let list_bottom = (footer_y - line_height * 0.25).max(list_y);
        Self {
            sidebar_x,
            width,
            title_y,
            tabs_y,
            tab_height,
            search_y,
            search_height,
            section_y,
            message_y,
            list_y,
            list_bottom,
            card_height: line_height * 5.2,
            footer_y,
        }
    }

    pub(super) fn visible_rows(&self) -> usize {
        (((self.list_bottom - self.list_y) / self.card_height).floor() as usize).max(1)
    }

    pub(super) fn tab_at(&self, x: f32, y: f32) -> Option<PluginsTab> {
        if x < self.sidebar_x
            || x >= self.sidebar_x + self.width
            || y < self.tabs_y
            || y >= self.tabs_y + self.tab_height
        {
            return None;
        }
        let relative = x - self.sidebar_x;
        Some(if relative < self.installed_tab_width() {
            PluginsTab::Installed
        } else if relative < self.installed_tab_width() + self.open_vsx_tab_width() {
            PluginsTab::OpenVsx
        } else {
            PluginsTab::Graphite
        })
    }

    pub(super) fn tab_bounds(&self, tab: PluginsTab) -> (f32, f32) {
        let installed = self.installed_tab_width();
        let open_vsx = self.open_vsx_tab_width();
        match tab {
            PluginsTab::Installed => (self.sidebar_x, installed),
            PluginsTab::OpenVsx => (self.sidebar_x + installed, open_vsx),
            PluginsTab::Graphite => (
                self.sidebar_x + installed + open_vsx,
                self.width - installed - open_vsx,
            ),
        }
    }

    fn installed_tab_width(&self) -> f32 {
        self.width * 0.37
    }

    fn open_vsx_tab_width(&self) -> f32 {
        self.width * 0.31
    }

    pub(super) fn search_contains(&self, x: f32, y: f32) -> bool {
        x >= self.sidebar_x + PAD
            && x < self.sidebar_x + self.width - PAD
            && y >= self.search_y
            && y < self.search_y + self.search_height
    }

    pub(super) fn card_index(&self, x: f32, y: f32, scroll: usize, count: usize) -> Option<usize> {
        if x < self.sidebar_x
            || x >= self.sidebar_x + self.width
            || y < self.list_y
            || y >= self.list_bottom
        {
            return None;
        }
        let row = ((y - self.list_y) / self.card_height) as usize;
        let index = scroll + row;
        (index < count).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_geometry_stays_inside_the_left_column() {
        let layout = PluginsLayout::new(720.0, 19.0, SIDEBAR_WIDTH);
        assert_eq!(layout.sidebar_x, ACTIVITY_BAR_WIDTH);
        assert_eq!(layout.width, 260.0);
        assert!(layout.list_y > layout.search_y + layout.search_height);
        assert!(layout.list_bottom <= layout.footer_y);
        assert!(layout.footer_y < 720.0 - 19.0);
        assert!(layout.visible_rows() >= 4);
    }

    #[test]
    fn tabs_search_and_cards_share_the_drawn_geometry() {
        let layout = PluginsLayout::new(720.0, 19.0, SIDEBAR_WIDTH);
        assert_eq!(
            layout.tab_at(ACTIVITY_BAR_WIDTH + 10.0, layout.tabs_y + 2.0),
            Some(PluginsTab::Installed)
        );
        assert_eq!(
            layout.tab_at(ACTIVITY_BAR_WIDTH + layout.width * 0.5, layout.tabs_y + 2.0),
            Some(PluginsTab::OpenVsx)
        );
        assert_eq!(
            layout.tab_at(SIDEBAR_WIDTH - 10.0, layout.tabs_y + 2.0),
            Some(PluginsTab::Graphite)
        );
        assert!(layout.search_contains(ACTIVITY_BAR_WIDTH + PAD + 1.0, layout.search_y + 2.0));
        assert_eq!(
            layout.card_index(ACTIVITY_BAR_WIDTH + PAD, layout.list_y + 2.0, 0, 5),
            Some(0)
        );
        assert_eq!(
            layout.card_index(
                ACTIVITY_BAR_WIDTH + PAD,
                layout.list_y + layout.card_height + 2.0,
                0,
                5
            ),
            Some(1)
        );
        assert_eq!(
            layout.card_index(ACTIVITY_BAR_WIDTH + PAD, layout.list_y + 2.0, 3, 5),
            Some(3)
        );
        assert_eq!(
            layout.card_index(ACTIVITY_BAR_WIDTH + PAD, layout.list_y + 2.0, 0, 0),
            None
        );
        assert_eq!(
            layout.card_index(ACTIVITY_BAR_WIDTH + PAD, layout.list_y + 2.0, 5, 5),
            None
        );
    }
}
