//! Connection form and its mouse controls. Both use the same layout.

use super::connections::Provider;
use super::drawing::{
    accent_text, draw_control, draw_label, error_color, fill, foreground, muted, wrap_text,
    ControlStyle,
};
use super::layout::Rect;
use super::state::{ConnectionPicker, ProviderDialogState};
use super::ui::mask_api_key;
use super::*;

const PAGE_SIZE: usize = 7;
const FIELDS: [&str; 6] = [
    "chat.saved_connections",
    "chat.name",
    "chat.provider",
    "chat.base_url",
    "chat.model",
    "chat.api_key",
];

pub(super) struct ConnectionLayout {
    card: Rect,
    fields: [(Rect, Rect); 6],
    saved_actions: [Rect; 3],
    form_actions: [Rect; 3],
    message: Rect,
    picker_rows: [Rect; PAGE_SIZE],
    picker_pages: [Rect; 2],
    confirmation: [Rect; 2],
    lh: f32,
}

impl ConnectionLayout {
    pub(super) fn new(w: f32, h: f32, lh: f32) -> Self {
        let width = (w - 16.0).clamp(1.0, 600.0);
        let lh = lh.min((h - 16.0).max(1.0) / 19.5);
        let height = lh * 19.5;
        let card = Rect::new((w - width) * 0.5, (h - height) * 0.5, width, height);
        let content = Rect::new(card.x + 8.0, card.y + 5.0, (width - 16.0).max(0.0), height);
        let row = |line: f32, rows: f32| content.row(content.y + line * lh, rows * lh);
        let fields = std::array::from_fn(|i| {
            let line = 1.8 + i as f32 * 2.2;
            (row(line, 1.0), row(line + 1.0, 1.15))
        });
        Self {
            card,
            fields,
            saved_actions: row(15.2, 1.35).columns(4.0),
            form_actions: row(16.8, 1.35).columns(4.0),
            message: row(18.2, 1.0),
            picker_rows: std::array::from_fn(|i| row(2.0 + i as f32 * 1.4, 1.4)),
            picker_pages: row(12.1, 1.4).columns(4.0),
            confirmation: row(8.0, 1.5).columns(4.0),
            lh,
        }
    }
}

impl App {
    pub(super) fn open_connection_picker(&mut self, field: usize) {
        let Some(dialog) = &mut self.chat.provider_dialog else {
            return;
        };
        dialog.picker = match field {
            0 if !self.chat.connections.items.is_empty() => Some(ConnectionPicker::Saved(
                self.chat
                    .connections
                    .items
                    .iter()
                    .position(|item| item.id == dialog.draft.id)
                    .unwrap_or(0),
            )),
            2 => Some(ConnectionPicker::Providers(
                Provider::ALL
                    .iter()
                    .position(|item| *item == dialog.draft.provider)
                    .unwrap_or(0),
            )),
            _ => None,
        };
    }

    pub(super) fn pick_connection_option(&mut self, picker: ConnectionPicker, index: usize) {
        match picker {
            ConnectionPicker::Saved(_) => {
                if let Some(connection) = self.chat.connections.items.get(index) {
                    let choose_for_chat = self
                        .chat
                        .provider_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.choose_for_chat);
                    self.chat.provider_dialog =
                        Some(ProviderDialogState::from_connection(Some(connection)));
                    if choose_for_chat {
                        self.use_chat_connection();
                    }
                }
            }
            ConnectionPicker::Providers(_) => {
                if let (Some(provider), Some(dialog)) =
                    (Provider::ALL.get(index), &mut self.chat.provider_dialog)
                {
                    dialog.draft.provider = *provider;
                    dialog.picker = None;
                    dialog.field_error = None;
                }
            }
        }
    }

    fn provider_label(&self, provider: Provider) -> &str {
        if provider == Provider::Compatible {
            self.t("chat.compatible_provider")
        } else {
            provider.label()
        }
    }

    pub(super) fn connection_picker_len(&self, picker: ConnectionPicker) -> usize {
        match picker {
            ConnectionPicker::Saved(_) => self.chat.connections.items.len(),
            ConnectionPicker::Providers(_) => Provider::ALL.len(),
        }
    }

    pub(super) fn draw_provider_dialog(
        &self,
        r: &mut Renderer,
        w: f32,
        h: f32,
        lh: f32,
        cw: f32,
        dialog: &ProviderDialogState,
    ) {
        let layout = ConnectionLayout::new(w, h, lh);
        let lh = layout.lh;
        let card = layout.card;
        fill(r, card, Theme::color(0x48484d, 0xb6b9bd));
        fill(
            r,
            Rect::new(card.x + 1.0, card.y + 1.0, card.w - 2.0, card.h - 2.0),
            Theme::color(Theme::SIDEBAR, 0xf3f3f3),
        );
        if dialog.confirmation.is_some() {
            let title = card.row(card.y + lh * 2.0, lh);
            draw_label(r, title, self.t("chat.switch_title"), lh, cw, foreground());
            let width = super::drawing::text_capacity(title, cw);
            for (i, line) in wrap_text(self.t("chat.switch_detail"), width)
                .iter()
                .take(4)
                .enumerate()
            {
                let row = title.row(title.y + (i + 2) as f32 * lh, lh);
                draw_label(r, row, line, lh, cw, muted());
            }
            draw_control(
                r,
                layout.confirmation[0],
                self.t("chat.new"),
                lh,
                cw,
                ControlStyle::Primary,
            );
            draw_control(
                r,
                layout.confirmation[1],
                self.t("chat.cancel"),
                lh,
                cw,
                ControlStyle::Normal,
            );
            return;
        }
        let heading = if dialog.choose_for_chat {
            "chat.choose_connection"
        } else if dialog
            .picker
            .is_some_and(|picker| matches!(picker, ConnectionPicker::Providers(_)))
        {
            "chat.provider"
        } else {
            "chat.configure"
        };
        draw_label(
            r,
            card.row(card.y + 4.0, lh * 1.4),
            self.t(heading),
            lh,
            cw,
            foreground(),
        );
        if let Some(picker) = dialog.picker {
            self.draw_connection_choices(r, &layout, cw, picker, dialog.choose_for_chat);
            return;
        }

        let draft = &dialog.draft;
        let saved = self.chat.connections.get(&draft.id);
        let credentials_saved = saved.is_some_and(|item| {
            item.provider == draft.provider
                && (item.provider != Provider::Compatible || item.base_url == draft.base_url)
                && (item.credential.is_some() || self.chat.session_keys.contains_key(&item.id))
        });
        let key = if !dialog.api_key.is_empty() {
            mask_api_key(&dialog.api_key)
        } else if credentials_saved {
            self.t("chat.key_saved").to_owned()
        } else {
            String::new()
        };
        let choice = saved.map_or_else(
            || self.t("chat.new_connection").to_owned(),
            |item| format!("{} · {}", item.name, item.model),
        );
        let endpoint = if draft.provider == Provider::Compatible
            || !draft.base_url.is_empty() && !draft.provider.has_fixed_endpoint()
        {
            &draft.base_url
        } else {
            draft.provider.kind().default_base_url()
        };
        let values = [
            format!("{}  ▾", choice),
            draft.name.clone(),
            format!("{}  ▾", self.provider_label(draft.provider)),
            endpoint.to_owned(),
            draft.model.clone(),
            key,
        ];
        for (i, (label, control)) in layout.fields.iter().copied().enumerate() {
            let field_error = dialog.field_error.filter(|(field, _)| *field == i);
            let caption = if let Some((_, error)) = field_error {
                format!("{} · {}", self.t(FIELDS[i]), self.t(error))
            } else if i == 3 && draft.provider != Provider::Compatible {
                format!("{} · {}", self.t(FIELDS[i]), self.t("chat.read_only_field"))
            } else {
                self.t(FIELDS[i]).to_owned()
            };
            draw_label(
                r,
                label,
                &caption,
                lh,
                cw,
                if field_error.is_some() {
                    error_color()
                } else {
                    muted()
                },
            );
            let style = if dialog.focus_field == i {
                ControlStyle::Focused
            } else if i == 0 || i == 2 {
                ControlStyle::Normal
            } else if i == 3 && draft.provider != Provider::Compatible {
                ControlStyle::Disabled
            } else {
                ControlStyle::Field
            };
            draw_control(r, control, &values[i], lh, cw, style);
        }
        for (i, (label, enabled)) in [
            ("chat.add", true),
            ("chat.use", saved.is_some()),
            ("chat.remove", saved.is_some()),
        ]
        .into_iter()
        .enumerate()
        {
            draw_control(
                r,
                layout.saved_actions[i],
                self.t(label),
                lh,
                cw,
                ControlStyle::enabled(enabled),
            );
        }
        draw_control(
            r,
            layout.form_actions[0],
            self.t("chat.save"),
            lh,
            cw,
            ControlStyle::Primary,
        );
        if dialog.offer_session_key {
            draw_control(
                r,
                layout.form_actions[1],
                self.t("chat.use_for_session"),
                lh,
                cw,
                ControlStyle::Normal,
            );
        }
        draw_control(
            r,
            layout.form_actions[2],
            self.t("chat.cancel"),
            lh,
            cw,
            ControlStyle::Normal,
        );
        if let Some(message) = &dialog.message {
            draw_label(r, layout.message, message, lh, cw, error_color());
        }
    }

    fn draw_connection_choices(
        &self,
        r: &mut Renderer,
        layout: &ConnectionLayout,
        cw: f32,
        picker: ConnectionPicker,
        choose_for_chat: bool,
    ) {
        let first = picker.page_start();
        let count = self.connection_picker_len(picker);
        let lh = layout.lh;
        for (row, index) in layout.picker_rows.iter().zip(first..count) {
            let label = match picker {
                ConnectionPicker::Saved(_) => {
                    let item = &self.chat.connections.items[index];
                    format!("{} · {}", item.name, item.model)
                }
                ConnectionPicker::Providers(_) => {
                    self.provider_label(Provider::ALL[index]).to_owned()
                }
            };
            let style = if index == picker.index() {
                ControlStyle::Selected
            } else {
                ControlStyle::Normal
            };
            draw_control(r, *row, &label, lh, cw, style);
        }
        if count > PAGE_SIZE {
            draw_control(
                r,
                layout.picker_pages[0],
                self.t("chat.previous"),
                lh,
                cw,
                ControlStyle::enabled(first > 0),
            );
            draw_control(
                r,
                layout.picker_pages[1],
                self.t("chat.next"),
                lh,
                cw,
                ControlStyle::enabled(first + PAGE_SIZE < count),
            );
        }
        if choose_for_chat {
            let manage = layout
                .card
                .row(layout.form_actions[0].y, layout.form_actions[0].h)
                .columns::<2>(4.0)[0];
            draw_control(
                r,
                manage,
                self.t("chat.configure"),
                lh,
                cw,
                ControlStyle::Normal,
            );
        }
        draw_control(
            r,
            layout.form_actions[2],
            self.t("chat.cancel"),
            lh,
            cw,
            ControlStyle::Normal,
        );
        let hint = match picker {
            ConnectionPicker::Saved(_) => "chat.saved_connections",
            ConnectionPicker::Providers(_) => "chat.provider",
        };
        draw_label(r, layout.message, self.t(hint), lh, cw, accent_text());
    }

    pub(super) fn click_connection_dialog(&mut self, x: f32, y: f32, w: f32, h: f32, lh: f32) {
        let layout = ConnectionLayout::new(w, h, lh);
        let Some(dialog) = &mut self.chat.provider_dialog else {
            return;
        };
        if dialog.confirmation.is_some() {
            if layout.confirmation[0].contains(x, y) {
                self.confirm_chat_connection_change();
            } else if layout.confirmation[1].contains(x, y) {
                dialog.confirmation = None;
            }
            return;
        }
        if let Some(picker) = dialog.picker {
            let first = picker.page_start();
            let count = self.connection_picker_len(picker);
            if let Some(index) = layout.picker_rows.iter().position(|r| r.contains(x, y)) {
                self.pick_connection_option(picker, first + index);
            } else if layout.form_actions[2].contains(x, y) {
                self.close_connection_picker();
            } else if self
                .chat
                .provider_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.choose_for_chat)
                && layout
                    .card
                    .row(layout.form_actions[0].y, layout.form_actions[0].h)
                    .columns::<2>(4.0)[0]
                    .contains(x, y)
            {
                self.open_chat_provider_dialog();
            } else if layout.picker_pages[0].contains(x, y) && first > 0 {
                self.chat.provider_dialog.as_mut().unwrap().picker =
                    Some(picker.at(first - PAGE_SIZE));
            } else if layout.picker_pages[1].contains(x, y) && first + PAGE_SIZE < count {
                self.chat.provider_dialog.as_mut().unwrap().picker =
                    Some(picker.at(first + PAGE_SIZE));
            }
            return;
        }
        if let Some(field) = layout
            .fields
            .iter()
            .position(|(a, b)| a.contains(x, y) || b.contains(x, y))
        {
            dialog.focus_field = field;
            self.open_connection_picker(field);
        } else if layout.saved_actions[0].contains(x, y) {
            self.add_chat_connection();
        } else if layout.saved_actions[1].contains(x, y) {
            self.use_chat_connection();
        } else if layout.saved_actions[2].contains(x, y) {
            self.remove_chat_connection();
        } else if layout.form_actions[0].contains(x, y) {
            self.save_chat_provider_from_dialog(false);
        } else if layout.form_actions[1].contains(x, y) && dialog.offer_session_key {
            self.save_chat_provider_from_dialog(true);
        } else if layout.form_actions[2].contains(x, y) {
            self.chat.provider_dialog = None;
        }
    }

    pub(super) fn close_connection_picker(&mut self) {
        if let Some(dialog) = &mut self.chat.provider_dialog {
            if dialog.choose_for_chat {
                self.chat.provider_dialog = None;
            } else {
                dialog.picker = None;
            }
        }
    }
}

impl ConnectionPicker {
    pub(super) fn index(self) -> usize {
        match self {
            Self::Saved(index) | Self::Providers(index) => index,
        }
    }

    pub(super) fn at(self, index: usize) -> Self {
        match self {
            Self::Saved(_) => Self::Saved(index),
            Self::Providers(_) => Self::Providers(index),
        }
    }

    fn page_start(self) -> usize {
        self.index() / PAGE_SIZE * PAGE_SIZE
    }
}
