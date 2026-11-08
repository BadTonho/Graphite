use graphite_text::Editor;

pub(super) trait ClipboardAccess {
    fn get_text(&mut self) -> Result<String, ()>;
    fn set_text(&mut self, text: &str) -> Result<(), ()>;
}

pub(super) struct SystemClipboard;

impl ClipboardAccess for SystemClipboard {
    fn get_text(&mut self) -> Result<String, ()> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|_| ())
    }

    fn set_text(&mut self, text: &str) -> Result<(), ()> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text.to_owned()))
            .map_err(|_| ())
    }
}

pub(super) fn copy_or_cut(
    buffer: &mut Editor<'_>,
    clipboard: &mut impl ClipboardAccess,
    cut: bool,
) -> bool {
    let selected = buffer.selected_texts();
    if selected.is_empty() || clipboard.set_text(&selected.join("\n")).is_err() {
        return false;
    }
    if cut {
        buffer.insert("");
    }
    true
}

pub(super) fn paste(buffer: &mut Editor<'_>, clipboard: &mut impl ClipboardAccess) -> bool {
    let Ok(text) = clipboard.get_text() else {
        return false;
    };
    buffer.paste(&text);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_text::{Buffer, Selection};

    #[derive(Default)]
    struct MemoryClipboard {
        text: String,
        fail_get: bool,
        fail_set: bool,
    }

    impl ClipboardAccess for MemoryClipboard {
        fn get_text(&mut self) -> Result<String, ()> {
            (!self.fail_get).then(|| self.text.clone()).ok_or(())
        }

        fn set_text(&mut self, text: &str) -> Result<(), ()> {
            if self.fail_set {
                return Err(());
            }
            self.text = text.to_owned();
            Ok(())
        }
    }

    #[test]
    fn copy_and_cut_preserve_unicode_multi_selections_and_undo_as_one_edit() {
        let mut buffer_document = Buffer::new();
        let mut buffer_state = graphite_text::SelectionState::default();
        let mut buffer = buffer_document.edit(&mut buffer_state, graphite_text::ViewId(1));
        buffer.insert("café 🧭");
        buffer.set_selections(vec![
            Selection { anchor: 0, head: 4 },
            Selection { anchor: 5, head: 6 },
        ]);
        let mut clipboard = MemoryClipboard::default();

        assert!(copy_or_cut(&mut buffer, &mut clipboard, false));
        assert_eq!(clipboard.text, "café\n🧭");
        assert_eq!(buffer.text(), "café 🧭");

        assert!(copy_or_cut(&mut buffer, &mut clipboard, true));
        assert_eq!(buffer.text(), " ");
        buffer.undo();
        assert_eq!(buffer.text(), "café 🧭");
    }

    #[test]
    fn clipboard_failures_and_readonly_buffers_do_not_change_document_text() {
        let mut buffer_document = Buffer::new();
        let mut buffer_state = graphite_text::SelectionState::default();
        let mut buffer = buffer_document.edit(&mut buffer_state, graphite_text::ViewId(1));
        buffer.insert("keep this");
        buffer.set_selection(0, 4);
        let mut clipboard = MemoryClipboard {
            fail_set: true,
            ..Default::default()
        };
        assert!(!copy_or_cut(&mut buffer, &mut clipboard, true));
        assert_eq!(buffer.text(), "keep this");

        clipboard.fail_set = false;
        buffer.set_read_only(true);
        assert!(copy_or_cut(&mut buffer, &mut clipboard, true));
        assert_eq!(buffer.text(), "keep this");

        clipboard.fail_get = true;
        assert!(!paste(&mut buffer, &mut clipboard));
        assert_eq!(buffer.text(), "keep this");
    }

    #[test]
    fn paste_distributes_unicode_lines_to_multiple_cursors_and_respects_readonly() {
        let mut buffer_document = Buffer::new();
        let mut buffer_state = graphite_text::SelectionState::default();
        let mut buffer = buffer_document.edit(&mut buffer_state, graphite_text::ViewId(1));
        buffer.insert("a b");
        buffer.set_selections(vec![
            Selection { anchor: 0, head: 1 },
            Selection { anchor: 2, head: 3 },
        ]);
        let mut clipboard = MemoryClipboard {
            text: "café\n東京".into(),
            ..Default::default()
        };
        assert!(paste(&mut buffer, &mut clipboard));
        assert_eq!(buffer.text(), "café 東京");
        buffer.undo();
        assert_eq!(buffer.text(), "a b");

        buffer.set_read_only(true);
        assert!(paste(&mut buffer, &mut clipboard));
        assert_eq!(buffer.text(), "a b");
    }
}
