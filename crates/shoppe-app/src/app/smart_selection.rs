use super::*;
use shoppe_text::Selection;

impl App {
    pub(crate) fn smart_select(&mut self, expand: bool) {
        if !expand {
            if let Some(selection) = self.smart_selection_history.pop() {
                self.tabs[self.active_tab]
                    .buffer
                    .set_selection(selection.anchor, selection.head);
            }
            return;
        }
        self.refresh_highlights();
        let tab = &mut self.tabs[self.active_tab];
        let before = tab.buffer.selection().unwrap_or(Selection {
            anchor: tab.buffer.cursor().char_idx,
            head: tab.buffer.cursor().char_idx,
        });
        let start = before.anchor.min(before.head);
        let end = before.anchor.max(before.head);
        let rope = tab.buffer.rope();
        let byte_start = rope.char_to_byte(start);
        let byte_end = rope.char_to_byte(end);
        let next = tab
            .highlighter
            .selection_ranges(byte_start, byte_end)
            .into_iter()
            .map(|range| rope.byte_to_char(range.start)..rope.byte_to_char(range.end))
            .find(|range| range.start < start || range.end > end);
        if let Some(next) = next {
            self.smart_selection_history.push(before);
            tab.buffer.set_selection(next.start, next.end);
        }
    }
}
