use super::*;
use graphite_text::Selection;

impl App {
    pub(crate) fn smart_select(&mut self, expand: bool) {
        let current = self.editor().selection().unwrap_or(Selection {
            anchor: self.editor().cursor().char_idx,
            head: self.editor().cursor().char_idx,
        });
        let guard = (
            self.active_document(),
            self.documents[self.active_document()].buffer.revision(),
            current,
        );
        if self
            .editor_groups
            .active_view()
            .smart_selection_guard
            .is_some_and(|expected| expected != guard)
        {
            self.editor_groups
                .active_view_mut()
                .smart_selection_history
                .clear();
            self.editor_groups.active_view_mut().smart_selection_guard = None;
        }
        if !expand {
            if let Some(selection) = self
                .editor_groups
                .active_view_mut()
                .smart_selection_history
                .pop()
            {
                self.editor_mut()
                    .set_selection(selection.anchor, selection.head);
                self.editor_groups.active_view_mut().smart_selection_guard =
                    Some((self.active_document(), guard.1, selection));
            }
            return;
        }
        self.refresh_highlights();
        let before = self.editor().selection().unwrap_or(Selection {
            anchor: self.editor().cursor().char_idx,
            head: self.editor().cursor().char_idx,
        });
        let start = before.anchor.min(before.head);
        let end = before.anchor.max(before.head);
        let document = self.active_document();
        let tab = &mut self.documents[document];
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
            self.editor_groups
                .active_view_mut()
                .smart_selection_history
                .push(before);
            self.editor_mut().set_selection(next.start, next.end);
            self.editor_groups.active_view_mut().smart_selection_guard = Some((
                self.active_document(),
                guard.1,
                Selection {
                    anchor: next.start,
                    head: next.end,
                },
            ));
        }
    }
}
