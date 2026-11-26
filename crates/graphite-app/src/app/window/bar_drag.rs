//! Drag-and-drop reordering and cross-bar movement for Activity Bar and Secondary Sidebar items.

use super::*;
use crate::config::{ActivityBarPreferences, SecondaryTabStyle};

pub(crate) const BAR_DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BarKind {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BarDropTarget {
    pub bar: BarKind,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BarDrag {
    pub source_bar: BarKind,
    pub source_key: String,
    pub label: String,
    pub icon: Option<char>,
    pub movable_across_bars: bool,
    pub start_pos: (f32, f32),
    pub current_pos: (f32, f32),
    pub active: bool,
    pub target: Option<BarDropTarget>,
}

/// Orders `items` according to `order`, appending any unseen items in their original order.
pub(crate) fn sort_by_order<T>(
    items: Vec<T>,
    order: &[String],
    key_fn: impl Fn(&T) -> &str,
) -> Vec<T> {
    if order.is_empty() {
        return items;
    }
    let mut indexed: Vec<(usize, T)> = items
        .into_iter()
        .enumerate()
        .map(|(default_idx, item)| {
            let key = key_fn(&item);
            let pos = order
                .iter()
                .position(|k| k == key)
                .unwrap_or(order.len() + default_idx);
            (pos, item)
        })
        .collect();
    indexed.sort_by_key(|(pos, _)| *pos);
    indexed.into_iter().map(|(_, item)| item).collect()
}

/// Computes a new key list with `source_key` moved to `target_index` (an insertion slot in `0..=len`).
pub(crate) fn reorder_keys(
    current_keys: &[String],
    source_key: &str,
    target_index: usize,
) -> Vec<String> {
    let mut keys: Vec<String> = current_keys.to_vec();
    let old_index = keys.iter().position(|k| k == source_key);
    let mut insert_at = target_index.min(keys.len());
    if let Some(old) = old_index {
        keys.remove(old);
        if old < insert_at {
            insert_at = insert_at.saturating_sub(1);
        }
    }
    insert_at = insert_at.min(keys.len());
    keys.insert(insert_at, source_key.to_owned());
    keys
}

/// Moves a plugin container key between `Primary` and `Secondary` bars and inserts it at `target_index`.
pub(crate) fn move_plugin_between_bars(
    source_key: &str,
    to_bar: BarKind,
    target_index: usize,
    target_current_keys: &[String],
    prefs: &mut ActivityBarPreferences,
) {
    prefs.moved_to_primary.retain(|k| k != source_key);
    prefs.moved_to_secondary.retain(|k| k != source_key);
    prefs.item_order.retain(|k| k != source_key);
    prefs.secondary_order.retain(|k| k != source_key);

    match to_bar {
        BarKind::Primary => {
            prefs.moved_to_primary.push(source_key.to_owned());
            prefs.item_order = reorder_keys(target_current_keys, source_key, target_index);
        }
        BarKind::Secondary => {
            prefs.moved_to_secondary.push(source_key.to_owned());
            prefs.secondary_order = reorder_keys(target_current_keys, source_key, target_index);
        }
    }
}

impl App {
    pub(crate) fn bar_drag_mouse_down(&mut self, mx: f32, my: f32) -> bool {
        if self.zen_mode {
            return false;
        }
        if let Some((source_key, movable_across_bars, label, icon)) =
            self.activity_bar_press_target(mx, my)
        {
            self.bar_drag = Some(BarDrag {
                source_bar: BarKind::Primary,
                source_key,
                label,
                icon,
                movable_across_bars,
                start_pos: (mx, my),
                current_pos: (mx, my),
                active: false,
                target: None,
            });
            return true;
        }
        if let Some((source_key, movable_across_bars, label, icon)) =
            self.secondary_tab_press_target(mx, my)
        {
            self.bar_drag = Some(BarDrag {
                source_bar: BarKind::Secondary,
                source_key,
                label,
                icon,
                movable_across_bars,
                start_pos: (mx, my),
                current_pos: (mx, my),
                active: false,
                target: None,
            });
            return true;
        }
        false
    }

    pub(crate) fn bar_drag_pointer_moved(&mut self, mx: f32, my: f32) -> bool {
        let Some(mut drag) = self.bar_drag.clone() else {
            return false;
        };
        drag.current_pos = (mx, my);
        let dx = mx - drag.start_pos.0;
        let dy = my - drag.start_pos.1;
        if !drag.active && (dx * dx + dy * dy).sqrt() >= BAR_DRAG_THRESHOLD {
            drag.active = true;
        }
        if drag.active {
            drag.target = self.compute_bar_drop_target(&drag, mx, my);
        }
        self.bar_drag = Some(drag);
        true
    }

    fn compute_bar_drop_target(&self, drag: &BarDrag, mx: f32, my: f32) -> Option<BarDropTarget> {
        if let Some(index) = self.activity_bar_drop_index(mx, my) {
            if drag.source_bar == BarKind::Primary || drag.movable_across_bars {
                return Some(BarDropTarget {
                    bar: BarKind::Primary,
                    index,
                });
            }
        }
        if let Some(index) = self.secondary_tab_drop_index(mx, my) {
            if drag.source_bar == BarKind::Secondary || drag.movable_across_bars {
                return Some(BarDropTarget {
                    bar: BarKind::Secondary,
                    index,
                });
            }
        }
        None
    }

    pub(crate) fn bar_drag_mouse_up(&mut self, mx: f32, my: f32) -> bool {
        let Some(drag) = self.bar_drag.take() else {
            return false;
        };
        if !drag.active {
            match drag.source_bar {
                BarKind::Primary => {
                    self.activity_bar_click_at(mx, my);
                }
                BarKind::Secondary => {
                    self.activate_secondary_view_by_key(&drag.source_key);
                }
            }
            return true;
        }

        let Some(target) = self.compute_bar_drop_target(&drag, mx, my).or(drag.target) else {
            return true;
        };

        if target.bar == drag.source_bar {
            match target.bar {
                BarKind::Primary => {
                    let keys = self.primary_activity_keys();
                    self.activity_bar_preferences.item_order =
                        reorder_keys(&keys, &drag.source_key, target.index);
                }
                BarKind::Secondary => {
                    let keys = self.secondary_view_keys();
                    self.activity_bar_preferences.secondary_order =
                        reorder_keys(&keys, &drag.source_key, target.index);
                }
            }
            self.save_activity_bar_preferences();
        } else if drag.movable_across_bars {
            let target_keys = match target.bar {
                BarKind::Primary => self.primary_activity_keys(),
                BarKind::Secondary => self.secondary_view_keys(),
            };
            move_plugin_between_bars(
                &drag.source_key,
                target.bar,
                target.index,
                &target_keys,
                &mut self.activity_bar_preferences,
            );
            if drag.source_bar == BarKind::Primary
                && self.active_plugin_activity.as_deref() == Some(drag.source_key.as_str())
            {
                self.active_plugin_activity = None;
                self.secondary_plugin = Some(drag.source_key.clone());
                self.set_secondary_sidebar_visible(true);
            } else if drag.source_bar == BarKind::Secondary
                && self.secondary_plugin.as_deref() == Some(drag.source_key.as_str())
            {
                self.secondary_plugin = None;
                if let Some(entry) = self
                    .plugin_activity_entries()
                    .into_iter()
                    .find(|entry| entry.key == drag.source_key)
                {
                    self.activate_plugin_activity(entry);
                }
            }
            self.save_activity_bar_preferences();
        }
        true
    }

    pub(crate) fn secondary_tab_style(&self) -> SecondaryTabStyle {
        self.activity_bar_preferences.secondary_tab_style
    }

    pub(crate) fn draw_bar_drag_preview(&self, renderer: &mut Renderer) {
        let Some(drag) = &self.bar_drag else {
            return;
        };
        if !drag.active {
            return;
        }
        let lh = renderer.line_height();
        let cw = renderer.char_width();
        let (mx, my) = drag.current_pos;
        let icon_size = (lh * 0.9).clamp(14.0, 20.0);
        let pill_w = (drag.label.chars().count() as f32 * cw + icon_size + PAD * 3.0).max(40.0);
        let pill_h = (lh * 1.6).max(28.0);
        let (win_w, win_h) = renderer.size();
        let x = (mx + 10.0).clamp(4.0, (win_w - pill_w - 4.0).max(4.0));
        let y = (my - pill_h * 0.5).clamp(4.0, (win_h - pill_h - 4.0).max(4.0));

        let border = rgb(Theme::color(Theme::STATUS, 0x007acc));
        let bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff));
        let fg = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));

        renderer.draw_rect(x, y, pill_w, pill_h, border);
        renderer.draw_rect(x + 1.0, y + 1.0, pill_w - 2.0, pill_h - 2.0, bg);

        let icon_x = x + PAD;
        let icon_y = y + (pill_h - icon_size) * 0.5;
        if let Some(glyph) = drag.icon {
            renderer.draw_icon(glyph, icon_x, icon_y, icon_size, fg);
        } else {
            let initial = drag.label.chars().next().unwrap_or('?').to_string();
            renderer.draw_text(&initial, icon_x, y + (pill_h - lh) * 0.5, fg);
        }
        renderer.draw_text(
            &drag.label,
            icon_x + icon_size + PAD * 0.6,
            y + (pill_h - lh) * 0.5,
            fg,
        );
    }
}

/// Applies the live drag preview to a bar's item list before the user releases the mouse.
pub(crate) fn apply_bar_preview<T>(
    mut items: Vec<T>,
    bar: BarKind,
    drag: Option<&BarDrag>,
    key_fn: impl Fn(&T) -> &str,
    cross_bar_item: impl FnOnce(&str) -> Option<T>,
) -> Vec<T> {
    let Some(drag) = drag.filter(|d| d.active) else {
        return items;
    };
    let Some(target) = drag.target else {
        return items;
    };
    let old_idx = items
        .iter()
        .position(|item| key_fn(item) == drag.source_key);
    if target.bar != bar {
        if let Some(idx) = old_idx {
            items.remove(idx);
        }
        return items;
    }
    let item = if let Some(idx) = old_idx {
        Some(items.remove(idx))
    } else if drag.movable_across_bars {
        cross_bar_item(&drag.source_key)
    } else {
        None
    };
    if let Some(item) = item {
        let mut insert_at = target.index.min(items.len());
        if let Some(old) = old_idx {
            if old < target.index {
                insert_at = target.index.saturating_sub(1).min(items.len());
            }
        }
        items.insert(insert_at, item);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_by_order_places_known_keys_first_and_preserves_new_items() {
        let items = vec!["a", "b", "c", "d"];
        let order = vec!["c".to_owned(), "a".to_owned()];
        let sorted = sort_by_order(items, &order, |s| s);
        assert_eq!(sorted, vec!["c", "a", "b", "d"]);
    }

    #[test]
    fn reorder_keys_moves_forward_and_backward_cleanly() {
        let keys: Vec<String> = ["a", "b", "c", "d"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        // Move "a" (index 0) after "c" (insertion slot 3 -> final index 2)
        assert_eq!(reorder_keys(&keys, "a", 3), vec!["b", "c", "a", "d"]);
        // Move "d" (index 3) to front (insertion slot 0)
        assert_eq!(reorder_keys(&keys, "d", 0), vec!["d", "a", "b", "c"]);
    }

    #[test]
    fn move_plugin_between_bars_updates_moved_lists_and_target_order() {
        let mut prefs = ActivityBarPreferences::default();
        let secondary_keys = vec!["chat".to_owned(), "outline".to_owned()];
        move_plugin_between_bars(
            "plugin:anthropic.claude-code:claude",
            BarKind::Secondary,
            1,
            &secondary_keys,
            &mut prefs,
        );
        assert_eq!(
            prefs.moved_to_secondary,
            vec!["plugin:anthropic.claude-code:claude"]
        );
        assert_eq!(
            prefs.secondary_order,
            vec!["chat", "plugin:anthropic.claude-code:claude", "outline"]
        );

        let primary_keys = vec![
            "explorer".to_owned(),
            "search".to_owned(),
            "source_control".to_owned(),
            "plugins".to_owned(),
        ];
        move_plugin_between_bars(
            "plugin:anthropic.claude-code:claude",
            BarKind::Primary,
            2,
            &primary_keys,
            &mut prefs,
        );
        assert!(prefs.moved_to_secondary.is_empty());
        assert_eq!(
            prefs.moved_to_primary,
            vec!["plugin:anthropic.claude-code:claude"]
        );
        assert_eq!(
            prefs.item_order,
            vec![
                "explorer",
                "search",
                "plugin:anthropic.claude-code:claude",
                "source_control",
                "plugins",
            ]
        );
    }

    #[test]
    fn apply_bar_preview_shifts_items_live_within_and_across_bars() {
        let drag = BarDrag {
            source_bar: BarKind::Primary,
            source_key: "p1".into(),
            label: "Plugin 1".into(),
            icon: None,
            movable_across_bars: true,
            start_pos: (0.0, 0.0),
            current_pos: (100.0, 10.0),
            active: true,
            target: Some(BarDropTarget {
                bar: BarKind::Secondary,
                index: 1,
            }),
        };
        let primary = apply_bar_preview(
            vec!["explorer", "p1", "search"],
            BarKind::Primary,
            Some(&drag),
            |s| s,
            |_| None,
        );
        assert_eq!(primary, vec!["explorer", "search"]);

        let secondary = apply_bar_preview(
            vec!["chat", "outline"],
            BarKind::Secondary,
            Some(&drag),
            |s| s,
            |k| (k == "p1").then_some("p1"),
        );
        assert_eq!(secondary, vec!["chat", "p1", "outline"]);
    }
}
