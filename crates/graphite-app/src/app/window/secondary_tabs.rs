//! Tab strip layout for the secondary side bar header. Pure geometry, so it can be tested
//! without a renderer.

/// One visible tab: its index among all views and its horizontal span, relative to the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TabSlot {
    pub index: usize,
    pub x: f32,
    pub width: f32,
}

/// Lays out tabs left to right inside `available` pixels. `label_chars[i]` is the length of the
/// label of tab `i`; each tab adds `padding` on both sides. When the tabs do not all fit, the
/// strip scrolls so that the `active` tab is fully visible, and tabs that would be cut off are
/// left out. A single tab wider than the strip is shortened to fit it.
pub(super) fn layout_tabs(
    label_chars: &[usize],
    active: Option<usize>,
    available: f32,
    char_width: f32,
    padding: f32,
) -> Vec<TabSlot> {
    let widths: Vec<f32> = label_chars
        .iter()
        .map(|&chars| chars as f32 * char_width + padding * 2.0)
        .collect();
    let mut first = 0;
    if let Some(active) = active.filter(|&active| active < widths.len()) {
        // Drop leading tabs until the active one ends inside the strip.
        while first < active && widths[first..=active].iter().sum::<f32>() > available {
            first += 1;
        }
    }
    let mut slots = Vec::new();
    let mut x = 0.0;
    for (index, &width) in widths.iter().enumerate().skip(first) {
        if x + width > available {
            if slots.is_empty() {
                slots.push(TabSlot {
                    index,
                    x,
                    width: (available - x).max(0.0),
                });
            }
            break;
        }
        slots.push(TabSlot { index, x, width });
        x += width;
    }
    slots
}

/// Lays out fixed-width icon tabs left to right inside `available` pixels.
pub(super) fn layout_icon_tabs(
    count: usize,
    tab_width: f32,
    active: Option<usize>,
    available: f32,
) -> Vec<TabSlot> {
    if count == 0 || tab_width <= 0.0 || available <= 0.0 {
        return Vec::new();
    }
    let mut first = 0;
    if let Some(active) = active.filter(|&active| active < count) {
        while first < active && (active - first + 1) as f32 * tab_width > available {
            first += 1;
        }
    }
    let mut slots = Vec::new();
    let mut x = 0.0;
    for index in first..count {
        if x + tab_width > available {
            if slots.is_empty() {
                slots.push(TabSlot {
                    index,
                    x,
                    width: (available - x).max(0.0),
                });
            }
            break;
        }
        slots.push(TabSlot {
            index,
            x,
            width: tab_width,
        });
        x += tab_width;
    }
    slots
}

/// The tab under `x` (relative to the strip), if any.
pub(super) fn tab_at(slots: &[TabSlot], x: f32) -> Option<usize> {
    slots
        .iter()
        .find(|slot| x >= slot.x && x < slot.x + slot.width)
        .map(|slot| slot.index)
}

/// The insertion index (`0..=total_count`) for a pointer at `x` (relative to the strip).
pub(super) fn insertion_index_at(slots: &[TabSlot], x: f32, total_count: usize) -> usize {
    for slot in slots {
        if x < slot.x + slot.width * 0.5 {
            return slot.index;
        }
    }
    slots
        .last()
        .map_or(0, |slot| (slot.index + 1).min(total_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tabs_are_laid_out_in_order_when_they_fit() {
        let slots = layout_tabs(&[4, 5, 3], Some(1), 400.0, 10.0, 8.0);
        assert_eq!(
            slots.iter().map(|slot| slot.index).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(
            slots[0],
            TabSlot {
                index: 0,
                x: 0.0,
                width: 56.0
            }
        );
        assert_eq!(slots[1].x, 56.0);
        assert_eq!(slots[2].x, 56.0 + 66.0);
    }

    #[test]
    fn the_strip_scrolls_to_keep_the_active_tab_visible() {
        // Widths 56, 66, 46, 76: only two fit in 130.
        let slots = layout_tabs(&[4, 5, 3, 6], Some(3), 130.0, 10.0, 8.0);
        assert_eq!(
            slots.iter().map(|slot| slot.index).collect::<Vec<_>>(),
            [2, 3]
        );
        let slots = layout_tabs(&[4, 5, 3, 6], Some(2), 130.0, 10.0, 8.0);
        assert_eq!(
            slots.iter().map(|slot| slot.index).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(slots.iter().all(|slot| slot.x + slot.width <= 130.0));
    }

    #[test]
    fn an_oversized_tab_is_shortened_and_nothing_fits_in_an_empty_strip() {
        let slots = layout_tabs(&[40], Some(0), 100.0, 10.0, 8.0);
        assert_eq!(
            slots,
            [TabSlot {
                index: 0,
                x: 0.0,
                width: 100.0
            }]
        );
        assert!(layout_tabs(&[], None, 100.0, 10.0, 8.0).is_empty());
        assert_eq!(layout_tabs(&[3], None, 0.0, 10.0, 8.0)[0].width, 0.0);
    }

    #[test]
    fn hit_testing_finds_the_tab_under_the_pointer() {
        let slots = layout_tabs(&[4, 5], None, 400.0, 10.0, 8.0);
        assert_eq!(tab_at(&slots, 10.0), Some(0));
        assert_eq!(tab_at(&slots, 56.0), Some(1));
        assert_eq!(tab_at(&slots, 122.0), None);
        assert_eq!(tab_at(&slots, -1.0), None);
    }

    #[test]
    fn icon_tabs_and_insertion_index_work() {
        let slots = layout_icon_tabs(4, 30.0, Some(3), 95.0);
        assert_eq!(
            slots.iter().map(|s| s.index).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(insertion_index_at(&slots, 5.0, 4), 1);
        assert_eq!(insertion_index_at(&slots, 25.0, 4), 2);
        assert_eq!(insertion_index_at(&slots, 85.0, 4), 4);
    }
}
