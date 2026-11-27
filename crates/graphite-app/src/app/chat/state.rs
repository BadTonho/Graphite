//! Chat, inline chat, agent session, and provider dialog state for `App`.

use super::connections::{Connection, Connections, SystemVault, Vault};
use super::context::ChatAttachments;
use graphite_chat::{
    load_approval_policy, load_sessions, save_approval_policy, save_session, AgentMode,
    ApprovalDecision, ChatSession, PendingToolApproval, ProviderConfig, ToolApprovalPolicy,
};
use std::sync::{atomic::AtomicBool, mpsc, Arc};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn record_storage_error(what: &str, error: &str) {
    crate::app::error_log::record(
        crate::app::error_log::ErrorSeverity::Medium,
        "chat.storage",
        format!("{what}: {error}"),
    );
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SessionsFilter {
    #[default]
    All,
    Archived,
    AutomationRuns,
    Scheduled,
}

/// Draft connection settings. Existing API keys never populate the text field.
#[derive(Default)]
pub(crate) struct ProviderDialogState {
    pub(super) draft: Connection,
    pub(super) api_key: String,
    pub(super) focus_field: usize,
    pub(super) offer_session_key: bool,
    pub(super) message: Option<String>,
    pub(super) field_error: Option<(usize, &'static str)>,
    pub(super) picker: Option<ConnectionPicker>,
    pub(super) choose_for_chat: bool,
    pub(super) confirmation: Option<ConnectionAction>,
}

#[derive(Clone, Copy)]
pub(super) enum ConnectionPicker {
    Saved(usize),
    Providers(usize),
}

#[derive(Clone, Copy)]
pub(super) enum ConnectionAction {
    Use,
    Save { session_only: bool },
}

impl ProviderDialogState {
    pub(super) const ROWS: usize = 6;

    pub(super) fn from_connection(connection: Option<&Connection>) -> Self {
        Self {
            draft: connection.cloned().unwrap_or_default(),
            focus_field: usize::from(connection.is_none()),
            ..Self::default()
        }
    }
}

/// Inline chat state anchored to the active editor view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineChatState {
    pub(crate) connection: Option<(String, String)>,
    pub(crate) opencode_session: Option<String>,
    pub document: graphite_core_api::DocumentId,
    pub anchor_line: usize,
    pub original_selection: Option<(usize, usize)>,
    pub prompt: String,
    pub response: String,
    pub proposed_replacement: Option<String>,
    pub running: bool,
    pub error: Option<String>,
}

impl InlineChatState {
    pub(super) fn opencode_session_for(
        &mut self,
        binding: &(String, String),
    ) -> Result<String, &'static str> {
        if self.connection.as_ref() == Some(binding) {
            if let Some(id) = &self.opencode_session {
                return Ok(id.clone());
            }
        }
        let id = super::connections::new_opencode_session()?;
        self.opencode_session = Some(id.clone());
        Ok(id)
    }
}

/// Which surface displays the chat conversation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ChatSurface {
    #[default]
    SideBar,
    Editor,
    QuickChat,
}

/// Full chat subsystem state owned by `App`.
pub(crate) struct ChatState {
    pub visible: bool,
    pub focused: bool,
    pub surface: ChatSurface,
    pub show_agent_sessions: bool,
    pub sessions_side_by_side: bool,
    pub sessions_filter: SessionsFilter,
    pub sessions_query: String,
    pub show_context_usage: bool,
    pub debug_panel_open: bool,
    pub tips_disabled: bool,
    pub dismissed_tips: Vec<usize>,
    pub tip_index: usize,
    pub sessions: Vec<ChatSession>,
    pub active_session_id: String,
    pub(super) connections: Connections,
    pub(super) session_keys: HashMap<String, String>,
    pub(super) attachments: ChatAttachments,
    pub(super) composer_menu: Option<super::layout::ComposerMenu>,
    pub(super) composer_choice: usize,
    pub provider_dialog: Option<ProviderDialogState>,
    pub policy: ToolApprovalPolicy,
    pub input: String,
    pub input_history_cursor: Option<usize>,
    pub scroll: usize,
    pub(super) review_list: Option<usize>,
    pub(super) reviewed_in_batch: Vec<PathBuf>,
    pub(super) transcript_offset: usize,
    pub selected_code_block: usize,
    pub pending_approval: Option<(PendingToolApproval, mpsc::Sender<ApprovalDecision>)>,
    pub running_turn: Option<(String, u64, Arc<AtomicBool>)>,
    pub inline_chat: Option<InlineChatState>,
    pub inline_cancel: Option<Arc<AtomicBool>>,
    pub last_copied_text: Option<String>,
    pub default_visible: bool,
    pub next_session_seq: u64,
}

impl Default for ChatState {
    fn default() -> Self {
        let initial = ChatSession::new("session-1", "New Chat", AgentMode::Ask);
        Self {
            visible: false,
            focused: false,
            surface: ChatSurface::SideBar,
            show_agent_sessions: false,
            sessions_side_by_side: false,
            sessions_filter: SessionsFilter::All,
            sessions_query: String::new(),
            show_context_usage: true,
            debug_panel_open: false,
            tips_disabled: false,
            dismissed_tips: Vec::new(),
            tip_index: 0,
            sessions: vec![initial],
            active_session_id: "session-1".into(),
            connections: Connections::default(),
            session_keys: HashMap::new(),
            attachments: ChatAttachments::default(),
            composer_menu: None,
            composer_choice: 0,
            provider_dialog: None,
            policy: ToolApprovalPolicy::default(),
            input: String::new(),
            input_history_cursor: None,
            scroll: 0,
            review_list: None,
            reviewed_in_batch: Vec::new(),
            transcript_offset: 0,
            selected_code_block: 0,
            pending_approval: None,
            running_turn: None,
            inline_chat: None,
            inline_cancel: None,
            last_copied_text: None,
            default_visible: false,
            next_session_seq: 2,
        }
    }
}

impl ChatState {
    pub(super) fn opencode_session(
        &mut self,
        connection: &Connection,
        profile: &Path,
        isolated: bool,
    ) -> Result<String, &'static str> {
        let active = self.active_session();
        let destination = connection.destination();
        if active.connection_id.as_deref() == Some(connection.id.as_str())
            && active.connection_destination.as_deref() == Some(destination.as_str())
        {
            if let Some(id) = &active.opencode_session {
                return Ok(id.clone());
            }
        }
        let id = super::connections::new_opencode_session()?;
        let mut pending = active.clone();
        pending.connection_id = Some(connection.id.clone());
        pending.connection_destination = Some(destination);
        pending.opencode_session = Some(id.clone());
        if !isolated {
            save_session(profile, &pending)
                .map_err(|_| "Could not save the OpenCode conversation identifier")?;
        }
        *self.active_session_mut() = pending;
        Ok(id)
    }

    pub fn clear_outgoing_draft(&mut self) {
        self.review_list = None;
        self.reviewed_in_batch.clear();
        self.attachments = ChatAttachments::default();
        self.composer_menu = None;
        self.input.clear();
        self.input_history_cursor = None;
        self.scroll = 0;
        self.transcript_offset = 0;
    }

    pub fn load_from_profile(profile_root: &Path) -> Self {
        let mut state = Self {
            policy: load_approval_policy(profile_root),
            ..Self::default()
        };
        match Connections::load(profile_root) {
            Ok(mut connections) => {
                if let Err(error) = connections.migrate(profile_root, &SystemVault) {
                    super::connection_ui::record_connection_error(error);
                }
                state.connections = connections;
            }
            Err(error) => super::connection_ui::record_connection_error(error),
        }
        let loaded = load_sessions(profile_root);
        if !loaded.is_empty() {
            state.active_session_id = loaded[0].id.clone();
            state.next_session_seq = loaded
                .iter()
                .filter_map(|session| session.id.strip_prefix("session-")?.parse::<u64>().ok())
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            state.sessions = loaded;
        }
        state
    }

    pub fn active_session(&self) -> &ChatSession {
        self.sessions
            .iter()
            .find(|s| s.id == self.active_session_id)
            .unwrap_or(&self.sessions[0])
    }

    pub fn active_session_mut(&mut self) -> &mut ChatSession {
        let id = self.active_session_id.clone();
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            &mut self.sessions[pos]
        } else {
            &mut self.sessions[0]
        }
    }

    pub fn session_by_id_mut(&mut self, id: &str) -> Option<&mut ChatSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn new_session(&mut self, mode: AgentMode) -> &ChatSession {
        let id = format!("session-{}", self.next_session_seq);
        self.next_session_seq += 1;
        let mut session = ChatSession::new(id.clone(), "New Chat", mode);
        if let Some(connection) = self
            .connections
            .preferred
            .as_deref()
            .and_then(|id| self.connections.get(id))
        {
            session.connection_id = Some(connection.id.clone());
            session.connection_destination = Some(connection.destination());
        }
        self.sessions.insert(0, session);
        self.active_session_id = id;
        self.scroll = 0;
        self.transcript_offset = 0;
        self.selected_code_block = 0;
        self.active_session()
    }

    /// Saves `session` unless the profile is isolated. A failure is recorded in the error log;
    /// returns whether the session is saved.
    pub fn persist_session(
        &self,
        profile_root: &Path,
        isolated: bool,
        session: &ChatSession,
    ) -> bool {
        if isolated {
            return true;
        }
        match save_session(profile_root, session) {
            Ok(()) => true,
            Err(error) => {
                record_storage_error("Could not save the chat conversation", &error);
                false
            }
        }
    }

    pub fn persist_active_session(&self, profile_root: &Path, isolated: bool) -> bool {
        self.persist_session(profile_root, isolated, self.active_session())
    }

    /// Saves every session, carrying on past a failure; returns whether all of them are saved.
    pub fn persist_all_sessions(&self, profile_root: &Path, isolated: bool) -> bool {
        let mut all_saved = true;
        for session in &self.sessions {
            all_saved &= self.persist_session(profile_root, isolated, session);
        }
        all_saved
    }

    pub(super) fn active_connection(&self) -> Option<&Connection> {
        let session = self.active_session();
        let id = session
            .connection_id
            .as_ref()
            .or(if session.turns.is_empty() {
                self.connections.preferred.as_ref()
            } else {
                None
            })?;
        let connection = self.connections.get(id)?;
        if session.turns.is_empty()
            || session.connection_destination.as_deref() == Some(connection.destination().as_str())
        {
            Some(connection)
        } else {
            None
        }
    }

    pub(super) fn configured_provider(
        &self,
        profile: &Path,
        connection: &Connection,
    ) -> Result<ProviderConfig, String> {
        let key = match (
            self.session_keys.get(&connection.id),
            connection.credential.as_deref(),
        ) {
            (Some(key), _) => Some(key.clone()),
            (None, Some(id)) => SystemVault.get(profile, id).map_err(str::to_owned)?,
            (None, None) => None,
        };
        if connection.provider.requires_key() && key.is_none() {
            return Err("An API key is required for this connection".into());
        }
        let config = connection.config(key);
        config.check().map_err(|error| error.to_string())?;
        Ok(config)
    }

    /// Saves the tool approvals unless the profile is isolated. A failure is recorded in the
    /// error log; returns whether the approvals are saved.
    pub fn persist_policy(&self, profile_root: &Path, isolated: bool) -> bool {
        if isolated {
            return true;
        }
        match save_approval_policy(profile_root, &self.policy) {
            Ok(()) => true,
            Err(error) => {
                record_storage_error("Could not save the chat tool approvals", &error);
                false
            }
        }
    }

    pub fn filtered_sessions(&self) -> Vec<&ChatSession> {
        let query = self.sessions_query.trim().to_lowercase();
        self.sessions
            .iter()
            .filter(|s| match self.sessions_filter {
                SessionsFilter::All => !s.archived,
                SessionsFilter::Archived => s.archived,
                SessionsFilter::AutomationRuns => {
                    !s.archived
                        && (s.id.starts_with("automation-")
                            || s.title.to_lowercase().contains("automation"))
                }
                SessionsFilter::Scheduled => {
                    !s.archived
                        && (s.id.starts_with("scheduled-")
                            || s.title.to_lowercase().contains("scheduled"))
                }
            })
            .filter(|s| query.is_empty() || s.title.to_lowercase().contains(&query))
            .collect()
    }
}
