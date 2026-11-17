//! Chat, inline chat, agent session, and provider dialog state for `App`.

use graphite_chat::{
    load_approval_policy, load_provider, load_sessions, save_approval_policy, save_provider,
    save_session, AgentMode, ApprovalDecision, ChatSession, PendingToolApproval, ProviderConfig,
    ProviderKind, ToolApprovalPolicy,
};
use std::path::Path;
use std::sync::{atomic::AtomicBool, mpsc, Arc};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SessionsFilter {
    #[default]
    All,
    Archived,
    AutomationRuns,
    Scheduled,
}

/// Interactive provider configuration modal state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderDialogState {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    /// Focused field: 0 = kind, 1 = base_url, 2 = model, 3 = api_key.
    pub focus_field: usize,
}

impl ProviderDialogState {
    pub fn from_config(config: Option<&ProviderConfig>) -> Self {
        match config {
            Some(cfg) => Self {
                kind: cfg.kind,
                base_url: cfg.base_url.clone(),
                model: cfg.model.clone(),
                api_key: cfg.api_key.clone().unwrap_or_default(),
                focus_field: 3,
            },
            None => {
                let def = ProviderConfig::default_for(ProviderKind::Anthropic);
                Self {
                    kind: def.kind,
                    base_url: def.base_url,
                    model: def.model,
                    api_key: String::new(),
                    focus_field: 3,
                }
            }
        }
    }

    pub fn to_config(&self) -> ProviderConfig {
        ProviderConfig {
            kind: self.kind,
            base_url: self.base_url.trim().to_owned(),
            model: self.model.trim().to_owned(),
            api_key: (!self.api_key.trim().is_empty()).then(|| self.api_key.trim().to_owned()),
        }
    }
}

/// Inline chat state anchored to the active editor view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineChatState {
    pub document: graphite_core_api::DocumentId,
    pub anchor_line: usize,
    pub original_selection: Option<(usize, usize)>,
    pub prompt: String,
    pub response: String,
    pub proposed_replacement: Option<String>,
    pub running: bool,
    pub error: Option<String>,
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
    pub provider: Option<ProviderConfig>,
    pub provider_dialog: Option<ProviderDialogState>,
    pub policy: ToolApprovalPolicy,
    pub input: String,
    pub input_history_cursor: Option<usize>,
    pub scroll: usize,
    pub selected_code_block: usize,
    pub pending_approval: Option<(PendingToolApproval, mpsc::Sender<ApprovalDecision>)>,
    pub running_turn: Option<(String, u64, Arc<AtomicBool>)>,
    pub inline_chat: Option<InlineChatState>,
    pub inline_cancel: Option<Arc<AtomicBool>>,
    pub last_prompt: Option<String>,
    pub last_copied_text: Option<String>,
    pub default_visible: bool,
    pub next_session_seq: u64,
}

impl Default for ChatState {
    fn default() -> Self {
        let initial = ChatSession::new("session-1", "New Chat", AgentMode::Agent);
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
            provider: None,
            provider_dialog: None,
            policy: ToolApprovalPolicy::default(),
            input: String::new(),
            input_history_cursor: None,
            scroll: 0,
            selected_code_block: 0,
            pending_approval: None,
            running_turn: None,
            inline_chat: None,
            inline_cancel: None,
            last_prompt: None,
            last_copied_text: None,
            default_visible: false,
            next_session_seq: 2,
        }
    }
}

impl ChatState {
    pub fn load_from_profile(profile_root: &Path) -> Self {
        let mut state = Self {
            provider: load_provider(profile_root),
            policy: load_approval_policy(profile_root),
            ..Self::default()
        };
        let loaded = load_sessions(profile_root);
        if !loaded.is_empty() {
            state.active_session_id = loaded[0].id.clone();
            state.next_session_seq = loaded.len() as u64 + 1;
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
        let session = ChatSession::new(id.clone(), "New Chat", mode);
        self.sessions.insert(0, session);
        self.active_session_id = id;
        self.scroll = 0;
        self.selected_code_block = 0;
        self.active_session()
    }

    pub fn persist_active_session(&self, profile_root: &Path, isolated: bool) {
        if isolated {
            return;
        }
        let _ = save_session(profile_root, self.active_session());
    }

    pub fn persist_provider(&self, profile_root: &Path, isolated: bool) -> Result<(), String> {
        if isolated {
            return Ok(());
        }
        if let Some(cfg) = &self.provider {
            save_provider(profile_root, cfg)
        } else {
            Ok(())
        }
    }

    pub fn persist_policy(&self, profile_root: &Path, isolated: bool) -> Result<(), String> {
        if isolated {
            return Ok(());
        }
        save_approval_policy(profile_root, &self.policy)
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
