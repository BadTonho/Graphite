//! Shared geometry for the Plugins sidebar.
//!
//! Drawing, hit testing and scrolling all read the same measurements so a card
//! drawn on screen is also the card that receives the click.
//!
//! The sidebar is one column: the search box, then one bar per list (Installed, Open VSX,
//! Graphite). The bars above the open list and the open bar sit at the top, its cards follow,
//! and the remaining bars stay under the cards.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PluginsTab {
    Installed,
    OpenVsx,
    Graphite,
}

impl PluginsTab {
    pub(super) const ALL: [PluginsTab; 3] = [Self::Installed, Self::OpenVsx, Self::Graphite];

    pub(super) fn index(self) -> usize {
        match self {
            Self::Installed => 0,
            Self::OpenVsx => 1,
            Self::Graphite => 2,
        }
    }

    pub(super) fn from_index(index: usize) -> Self {
        Self::ALL[index.min(2)]
    }

    /// The key that switches to this list.
    pub(super) fn action_key(self) -> char {
        match self {
            Self::Installed => 'i',
            Self::OpenVsx => 'o',
            Self::Graphite => 'g',
        }
    }
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
    pub(super) search_y: f32,
    pub(super) search_height: f32,
    pub(super) bar_height: f32,
    bars_top: f32,
    /// List whose bar is open (its cards are the ones drawn).
    active: usize,
    /// Top of the bars that come after the open list.
    after_top: f32,
    /// The detail views hide the bars below the open one to use their room.
    after_hidden: bool,
    pub(super) message_y: f32,
    pub(super) list_y: f32,
    pub(super) list_bottom: f32,
    pub(super) card_height: f32,
    pub(super) footer_y: f32,
}

impl PluginsLayout {
    pub(super) fn new(
        height: f32,
        line_height: f32,
        sidebar_width: f32,
        active: usize,
        collapsed: bool,
        details_open: bool,
    ) -> Self {
        let active = active.min(2);
        let sidebar_x = ACTIVITY_BAR_WIDTH;
        let width = sidebar_width - ACTIVITY_BAR_WIDTH;
        let title_y = line_height * 0.2;
        let search_y = line_height * 1.2;
        let search_height = line_height * 1.6;
        let bar_height = line_height * 1.6;
        let bars_top = search_y + search_height + line_height * 0.5;
        let message_y = bars_top + (active + 1) as f32 * bar_height + line_height * 0.15;
        let list_y = message_y + line_height * 1.15;
        let content_bottom = (height - line_height).max(0.0);
        let footer_y = (content_bottom - line_height * 5.4).max(list_y + line_height * 2.0);
        let region_bottom = (footer_y - line_height * 0.25).max(list_y);
        let card_height = line_height * 5.2;
        let after_count = (2 - active) as f32;
        let folded = collapsed && !details_open;
        let after_top = if folded {
            list_y
        } else if details_open {
            region_bottom
        } else {
            (region_bottom - after_count * bar_height).max(list_y + card_height)
        };
        let list_bottom = if folded { list_y } else { after_top };
        Self {
            sidebar_x,
            width,
            title_y,
            search_y,
            search_height,
            bar_height,
            bars_top,
            active,
            after_top,
            after_hidden: details_open,
            message_y,
            list_y,
            list_bottom,
            card_height,
            footer_y,
        }
    }

    pub(super) fn visible_rows(&self) -> usize {
        (((self.list_bottom - self.list_y) / self.card_height).floor() as usize).max(1)
    }

    /// Top of the bar of list `index`.
    pub(super) fn bar_y(&self, index: usize) -> f32 {
        if index <= self.active {
            self.bars_top + index as f32 * self.bar_height
        } else {
            self.after_top + (index - self.active - 1) as f32 * self.bar_height
        }
    }

    /// Whether the bar of list `index` is on screen.
    pub(super) fn bar_visible(&self, index: usize) -> bool {
        index <= self.active || !self.after_hidden
    }

    /// The list whose bar is at the point.
    pub(super) fn bar_at(&self, x: f32, y: f32) -> Option<usize> {
        if x < self.sidebar_x || x >= self.sidebar_x + self.width {
            return None;
        }
        (0..3).find(|&index| {
            self.bar_visible(index)
                && y >= self.bar_y(index)
                && y < self.bar_y(index) + self.bar_height
        })
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

    fn layout(active: usize, collapsed: bool, details: bool) -> PluginsLayout {
        PluginsLayout::new(720.0, 19.0, SIDEBAR_WIDTH, active, collapsed, details)
    }

    #[test]
    fn sidebar_geometry_stays_inside_the_left_column() {
        let layout = layout(0, false, false);
        assert_eq!(layout.sidebar_x, ACTIVITY_BAR_WIDTH);
        assert_eq!(layout.width, 260.0);
        assert!(layout.list_y > layout.search_y + layout.search_height);
        assert!(layout.list_bottom <= layout.footer_y);
        assert!(layout.footer_y < 720.0 - 19.0);
        assert!(layout.visible_rows() >= 4);
    }

    #[test]
    fn the_three_bars_are_stacked_in_order_around_the_open_list() {
        for active in 0..3 {
            let l = layout(active, false, false);
            for index in 0..3 {
                assert!(l.bar_visible(index));
            }
            // Top to bottom: bars up to the open one, the cards, then the rest.
            assert!(l.bar_y(0) >= l.search_y + l.search_height);
            for index in 1..=active {
                assert!(l.bar_y(index) >= l.bar_y(index - 1) + l.bar_height);
            }
            assert!(l.list_y >= l.bar_y(active) + l.bar_height);
            if active < 2 {
                assert!(l.bar_y(active + 1) >= l.list_bottom);
                assert!(l.bar_y(2) + l.bar_height <= l.footer_y);
            }
        }
    }

    #[test]
    fn a_folded_list_lets_the_next_bars_follow_the_open_bar() {
        let open = layout(0, false, false);
        let folded = layout(0, true, false);
        assert!(folded.bar_y(1) < open.bar_y(1));
        assert!(folded.bar_y(1) <= folded.list_y + 0.01);
    }

    #[test]
    fn details_hide_the_bars_below_the_open_one() {
        let l = layout(0, false, true);
        assert!(l.bar_visible(0));
        assert!(!l.bar_visible(1) && !l.bar_visible(2));
        assert_eq!(l.bar_at(ACTIVITY_BAR_WIDTH + 20.0, l.bar_y(1) + 2.0), None);
        assert!(l.list_bottom > layout(0, false, false).list_bottom);
    }

    #[test]
    fn clicks_find_the_bar_search_box_or_card_they_land_on() {
        let l = layout(1, false, false);
        let x = ACTIVITY_BAR_WIDTH + 20.0;
        for index in 0..3 {
            assert_eq!(l.bar_at(x, l.bar_y(index) + 2.0), Some(index));
        }
        assert_eq!(l.bar_at(x, l.search_y + 2.0), None);
        assert_eq!(l.bar_at(x, l.list_y + 2.0), None);
        assert_eq!(l.bar_at(2.0, l.bar_y(0) + 2.0), None);
        assert!(l.search_contains(ACTIVITY_BAR_WIDTH + PAD + 1.0, l.search_y + 2.0));
        assert_eq!(l.card_index(x, l.list_y + 2.0, 0, 5), Some(0));
        assert_eq!(
            l.card_index(x, l.list_y + l.card_height + 2.0, 0, 5),
            Some(1)
        );
        assert_eq!(l.card_index(x, l.list_y + 2.0, 3, 5), Some(3));
        assert_eq!(l.card_index(x, l.list_y + 2.0, 0, 0), None);
        assert_eq!(l.card_index(x, l.list_y + 2.0, 5, 5), None);
    }

    #[test]
    fn every_tab_has_an_index_and_a_key() {
        let indexes: Vec<usize> = PluginsTab::ALL.iter().map(|tab| tab.index()).collect();
        assert_eq!(indexes, [0, 1, 2]);
        let keys: Vec<char> = PluginsTab::ALL.iter().map(|tab| tab.action_key()).collect();
        assert_eq!(keys, ['i', 'o', 'g']);
        assert_eq!(PluginsTab::from_index(1), PluginsTab::OpenVsx);
        assert_eq!(PluginsTab::from_index(9), PluginsTab::Graphite);
    }
}
