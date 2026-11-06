use super::*;

impl App {
    pub(crate) fn handle_terminal_key(&mut self, event: &winit::event::KeyEvent) {
        let mut bytes = Vec::new();
        let ctrl = self.modifiers.control_key();
        match &event.logical_key {
            Key::Named(NamedKey::Enter) => bytes.extend_from_slice(b"\r"),
            Key::Named(NamedKey::Backspace) => bytes.push(0x7f),
            Key::Named(NamedKey::Tab) => bytes.push(b'\t'),
            Key::Named(NamedKey::Escape) => bytes.push(0x1b),
            Key::Named(NamedKey::ArrowUp) => bytes.extend_from_slice(b"\x1b[A"),
            Key::Named(NamedKey::ArrowDown) => bytes.extend_from_slice(b"\x1b[B"),
            Key::Named(NamedKey::ArrowRight) => bytes.extend_from_slice(b"\x1b[C"),
            Key::Named(NamedKey::ArrowLeft) => bytes.extend_from_slice(b"\x1b[D"),
            Key::Named(NamedKey::Home) => bytes.extend_from_slice(b"\x1b[H"),
            Key::Named(NamedKey::End) => bytes.extend_from_slice(b"\x1b[F"),
            Key::Named(NamedKey::Delete) => bytes.extend_from_slice(b"\x1b[3~"),
            Key::Named(NamedKey::PageUp) => bytes.extend_from_slice(b"\x1b[5~"),
            Key::Named(NamedKey::PageDown) => bytes.extend_from_slice(b"\x1b[6~"),
            Key::Character(c) if ctrl => {
                if let Some(ch) = c.chars().next() {
                    let upper = ch.to_ascii_uppercase();
                    if upper.is_ascii_alphabetic() {
                        bytes.push(upper as u8 - b'@');
                    }
                }
            }
            _ => {
                if let Some(text) = &event.text {
                    bytes.extend_from_slice(text.as_bytes());
                }
            }
        }
        if !bytes.is_empty() {
            if let Some(terminal) = &self.terminal {
                terminal.input(bytes);
            }
        }
    }
}
