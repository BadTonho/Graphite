//! Connection selection and credential management for the built-in chat.

use super::connections::{new_id, Connection, Connections, Provider, SystemVault, Vault};
use super::state::{ConnectionAction, ProviderDialogState};
use super::*;
use graphite_chat::{AgentMode, ProviderConfig};

pub(super) fn record_connection_error(error: &str) {
    crate::app::error_log::record(
        crate::app::error_log::ErrorSeverity::Medium,
        "chat.connections",
        error,
    );
}

impl App {
    pub(crate) fn reload_chat_profile(&mut self, profile: &std::path::Path) {
        let (visible, focused, surface) = (self.chat.visible, self.chat.focused, self.chat.surface);
        self.cancel_chat_turn();
        self.close_inline_chat();
        self.chat = super::ChatState::load_from_profile(profile);
        self.chat.visible = visible;
        self.chat.focused = focused;
        self.chat.surface = surface;
    }

    pub(crate) fn open_chat_provider_dialog(&mut self) {
        if self.chat_connection_busy() {
            return;
        }
        self.chat.composer_menu = None;
        let existing = self
            .chat
            .active_connection()
            .or_else(|| self.chat.connections.items.first());
        self.chat.provider_dialog = Some(ProviderDialogState::from_connection(existing));
    }

    pub(super) fn choose_chat_connection(&mut self) {
        self.open_chat_provider_dialog();
        if self.chat.connections.items.is_empty() {
            return;
        }
        if let Some(dialog) = &mut self.chat.provider_dialog {
            dialog.choose_for_chat = true;
            self.open_connection_picker(0);
        }
    }

    pub(super) fn cycle_chat_connection(&mut self, backwards: bool) {
        let Some(dialog) = &self.chat.provider_dialog else {
            return;
        };
        let items = &self.chat.connections.items;
        let selected = items.iter().position(|item| item.id == dialog.draft.id);
        let next = match (selected, backwards) {
            (Some(index), false) => items.get(index + 1).or_else(|| items.first()),
            (Some(0) | None, true) => items.last(),
            (Some(index), true) => items.get(index - 1),
            (None, false) => items.first(),
        };
        self.chat.provider_dialog = Some(ProviderDialogState::from_connection(next));
    }

    pub(super) fn add_chat_connection(&mut self) {
        self.chat.provider_dialog = Some(ProviderDialogState {
            focus_field: 1,
            ..ProviderDialogState::default()
        });
    }

    pub(super) fn use_chat_connection(&mut self) {
        self.apply_saved_chat_connection(false);
    }

    fn apply_saved_chat_connection(&mut self, confirmed: bool) {
        if self.chat_connection_busy() {
            return;
        }
        let Some(dialog) = &self.chat.provider_dialog else {
            return;
        };
        let id = dialog.draft.id.clone();
        let Some(connection) = self.chat.connections.get(&id) else {
            return;
        };
        if !confirmed && self.chat_switch_starts_new_session(connection) {
            self.chat.provider_dialog.as_mut().unwrap().confirmation = Some(ConnectionAction::Use);
            return;
        }
        let mut next = self.chat.connections.clone();
        next.preferred = Some(id.clone());
        if let Err(error) = self.save_chat_connections(next) {
            self.connection_dialog_error(error);
            return;
        }
        self.bind_chat_connection(&id);
    }

    pub(super) fn save_chat_provider_from_dialog(&mut self, session_only: bool) {
        self.apply_chat_connection_draft(session_only, false);
    }

    pub(super) fn confirm_chat_connection_change(&mut self) {
        let action = self
            .chat
            .provider_dialog
            .as_mut()
            .and_then(|dialog| dialog.confirmation.take());
        match action {
            Some(ConnectionAction::Use) => self.apply_saved_chat_connection(true),
            Some(ConnectionAction::Save { session_only }) => {
                self.apply_chat_connection_draft(session_only, true)
            }
            None => {}
        }
    }

    fn chat_switch_starts_new_session(&self, connection: &Connection) -> bool {
        let session = self.chat.active_session();
        !session.turns.is_empty()
            && (session.connection_id.as_deref() != Some(connection.id.as_str())
                || session.connection_destination.as_deref()
                    != Some(connection.destination().as_str()))
    }

    fn apply_chat_connection_draft(&mut self, session_only: bool, confirmed: bool) {
        if self.chat_connection_busy() {
            return;
        }
        let Some(mut dialog) = self.chat.provider_dialog.take() else {
            return;
        };
        dialog.field_error = None;
        dialog.message = None;
        let result = self.save_chat_connection(&mut dialog, session_only, confirmed);
        match result {
            Ok(Some(id)) => self.bind_chat_connection(&id),
            Ok(None) => self.chat.provider_dialog = Some(dialog),
            Err(error) => {
                if let Some((field, key)) = dialog.field_error {
                    dialog.focus_field = field;
                    record_connection_error(error);
                    self.status = self.t(key).into();
                    self.chat.provider_dialog = Some(dialog);
                } else {
                    self.chat.provider_dialog = Some(dialog);
                    self.connection_dialog_error(error);
                }
            }
        }
    }

    fn save_chat_connection(
        &mut self,
        dialog: &mut ProviderDialogState,
        session_only: bool,
        confirmed: bool,
    ) -> Result<Option<String>, &'static str> {
        let mut connection = dialog.draft.clone();
        let previous = self.chat.connections.get(&connection.id).cloned();
        connection.name = connection.name.trim().to_owned();
        connection.model = connection.model.trim().to_owned();
        connection.base_url = connection.base_url.trim().trim_end_matches('/').to_owned();
        if connection.name.is_empty() {
            dialog.field_error = Some((1, "chat.name_required"));
            return Err("Missing chat connection name");
        }
        if connection.model.is_empty() {
            dialog.field_error = Some((4, "chat.model_required"));
            return Err("Missing chat model ID");
        }
        if connection.provider == Provider::Compatible && connection.base_url.is_empty() {
            dialog.field_error = Some((3, "chat.url_required"));
            return Err("Enter the base URL for this connection");
        }
        // Keep the exact endpoint of a migrated connection unless the protocol changes.
        if connection.provider.has_fixed_endpoint() {
            connection.base_url.clear();
        } else if connection.provider != Provider::Compatible {
            connection.base_url = previous
                .as_ref()
                .filter(|old| old.provider == connection.provider)
                .map(|old| old.base_url.clone())
                .unwrap_or_default();
        }
        let same_endpoint = previous.as_ref().is_some_and(|old| {
            old.provider == connection.provider && old.base_url == connection.base_url
        });
        if !same_endpoint {
            connection.credential = None;
        }
        let keeps_id = previous
            .as_ref()
            .is_some_and(|old| old.same_destination(&connection));
        let preserves_previous = !keeps_id
            && previous.as_ref().is_some_and(|old| {
                self.chat.sessions.iter().any(|session| {
                    !session.turns.is_empty()
                        && session.connection_id.as_deref() == Some(old.id.as_str())
                })
            });
        let entered = dialog.api_key.trim();
        let memory_key = if same_endpoint {
            previous
                .as_ref()
                .and_then(|old| self.chat.session_keys.get(&old.id))
                .cloned()
        } else {
            None
        };
        if connection.provider.requires_key()
            && entered.is_empty()
            && connection.credential.is_none()
            && memory_key.is_none()
        {
            dialog.field_error = Some((5, "chat.key_required"));
            return Err("Enter an API key for this connection");
        }
        // The placeholder only validates the URL and model. It never leaves this function.
        connection
            .config(Some("validation".into()))
            .check()
            .map_err(|_| {
                dialog.field_error = Some((3, "chat.url_invalid"));
                "Enter a valid HTTPS or local HTTP endpoint"
            })?;
        if !confirmed
            && !self.chat.active_session().turns.is_empty()
            && (!keeps_id || self.chat_switch_starts_new_session(&connection))
        {
            dialog.confirmation = Some(ConnectionAction::Save { session_only });
            return Ok(None);
        }
        if !keeps_id {
            connection.id = new_id()?;
        }

        let mut written = None;
        let mut memory = memory_key;
        if !entered.is_empty() {
            if session_only || self.profiles.is_none() {
                memory = Some(entered.to_owned());
            } else {
                let credential = new_id()?;
                if let Err(error) = SystemVault.set(&self.profile_root, &credential, entered) {
                    dialog.offer_session_key = true;
                    return Err(error);
                }
                connection.credential = Some(credential.clone());
                written = Some(credential);
                memory = None;
            }
        }

        let mut next = self.chat.connections.clone();
        let id = connection.id.clone();
        let credential = connection.credential.clone();
        if let Some(index) = next
            .items
            .iter()
            .position(|item| item.id == dialog.draft.id)
            .filter(|_| !preserves_previous)
        {
            next.items[index] = connection;
        } else {
            next.items.push(connection);
        }
        next.preferred = Some(id.clone());
        if let Err(error) = self.save_chat_connections(next) {
            self.remove_chat_credential(written.as_deref());
            return Err(error);
        }
        if let Some(old) = previous.filter(|_| !preserves_previous) {
            self.chat.session_keys.remove(&old.id);
            if old.credential != credential {
                self.remove_chat_credential(old.credential.as_deref());
            }
        }
        if let Some(key) = memory {
            self.chat.session_keys.insert(id.clone(), key);
        }
        Ok(Some(id))
    }

    pub(super) fn remove_chat_connection(&mut self) {
        if self.chat_connection_busy() {
            return;
        }
        let Some(dialog) = &self.chat.provider_dialog else {
            return;
        };
        let Some(connection) = self.chat.connections.get(&dialog.draft.id).cloned() else {
            return;
        };
        let mut next = self.chat.connections.clone();
        next.items.retain(|item| item.id != connection.id);
        if next.preferred.as_deref() == Some(&connection.id) {
            next.preferred = next.items.first().map(|item| item.id.clone());
        }
        if let Err(error) = self.save_chat_connections(next) {
            self.connection_dialog_error(error);
            return;
        }
        self.chat.session_keys.remove(&connection.id);
        self.remove_chat_credential(connection.credential.as_deref());
        let first = self.chat.connections.items.first();
        self.chat.provider_dialog = Some(ProviderDialogState::from_connection(first));
    }

    fn save_chat_connections(&mut self, next: Connections) -> Result<(), &'static str> {
        if self.profiles.is_some() {
            next.save(&self.profile_root)?;
        }
        self.chat.connections = next;
        Ok(())
    }

    fn remove_chat_credential(&self, credential: Option<&str>) {
        if let Some(id) = credential {
            if self
                .chat
                .connections
                .items
                .iter()
                .any(|item| item.credential.as_deref() == Some(id))
            {
                return;
            }
            if let Err(error) = SystemVault.remove(&self.profile_root, id) {
                record_connection_error(error);
            }
        }
    }

    fn bind_chat_connection(&mut self, id: &str) {
        let session = self.chat.active_session();
        let Some(destination) = self.chat.connections.get(id).map(Connection::destination) else {
            return;
        };
        if session.connection_id.as_deref() != Some(id)
            || session.connection_destination.as_deref() != Some(destination.as_str())
        {
            if session.turns.is_empty() {
                self.chat.active_session_mut().connection_id = Some(id.to_owned());
                self.chat.active_session_mut().connection_destination = Some(destination);
                self.chat.active_session_mut().opencode_session = None;
                self.chat.active_session_mut().mode = AgentMode::Ask;
                self.chat.attachments = Default::default();
            } else {
                self.new_chat_session();
            }
        }
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.chat.provider_dialog = None;
        self.chat.focused = self.chat.visible;
        self.status = self.t("chat.connection_selected").into();
    }

    pub(crate) fn chat_connection_busy(&mut self) -> bool {
        if self.chat.running_turn.is_some()
            || self
                .chat
                .inline_chat
                .as_ref()
                .is_some_and(|inline| inline.running)
        {
            self.status = self.t("chat.stop_before_switch").into();
            true
        } else {
            false
        }
    }

    fn connection_dialog_error(&mut self, error: &str) {
        record_connection_error(error);
        if let Some(dialog) = &mut self.chat.provider_dialog {
            dialog.message = Some(error.to_owned());
        }
        self.status = error.to_owned();
    }

    pub(super) fn provider_for_chat(
        &mut self,
        inline: bool,
    ) -> Option<(Connection, ProviderConfig)> {
        let connection = if inline {
            self.chat
                .connections
                .preferred
                .as_deref()
                .and_then(|id| self.chat.connections.get(id))
        } else {
            self.chat.active_connection()
        };
        let Some(connection) = connection.cloned() else {
            let read_only = !inline && !self.chat.active_session().turns.is_empty();
            if !read_only {
                self.open_chat_provider_dialog();
            }
            self.status = self
                .t(if read_only {
                    "chat.read_only"
                } else {
                    "chat.no_connection"
                })
                .into();
            return None;
        };
        match self
            .chat
            .configured_provider(&self.profile_root, &connection)
        {
            Ok(config) => Some((connection, config)),
            Err(error) => {
                self.open_chat_provider_dialog();
                self.connection_dialog_error(&error);
                None
            }
        }
    }
}
