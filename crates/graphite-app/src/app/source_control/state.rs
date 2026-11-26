//! The source control state of the window: the lists and their layout, the Graph, the commit
//! box, and the file watcher that keeps them current.

use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) struct ScmState {
    pub(crate) graph_menu: Option<crate::app::scm_graph_menu::GraphContextMenu>,
    pub(crate) more_menu: Option<crate::app::scm_more_menu::MoreMenu>,
    pub(crate) confirm: Option<crate::app::scm_confirm::ScmConfirm>,
    pub(crate) watcher: Option<notify::RecommendedWatcher>,
    pub(crate) refresh_gate: crate::app::scm_workspace::RefreshGate,
    pub(crate) snapshot: crate::scm::Snapshot,
    pub(crate) visible: bool,
    /// Whether the source control list (not the editor) receives plain-key shortcuts.
    pub(crate) focused: bool,
    /// Set once the unsaved-documents check has been answered, so the commit goes through.
    pub(crate) commit_save_checked: bool,
    pub(crate) selected: usize,
    pub(crate) repositories_collapsed: bool,
    pub(crate) staged_collapsed: bool,
    /// The changes lists as a folder tree instead of a flat list.
    pub(crate) view_as_tree: bool,
    pub(crate) collapsed_folders: crate::app::scm_tree::Collapsed,
    /// The files a folder action (stage or unstage everything in it) works on.
    pub(crate) batch_paths: Vec<PathBuf>,
    pub(crate) changes_collapsed: bool,
    pub(crate) graph_collapsed: bool,
    pub(crate) graph_height: Option<f32>,
    pub(crate) graph_scroll: usize,
    pub(crate) graph_resize_dragging: bool,
    pub(crate) graph_rows: Vec<crate::vcs::scm_graph::GraphRow>,
    pub(crate) commit_stats:
        std::cell::RefCell<HashMap<String, Option<crate::vcs::scm_graph::CommitDiffStats>>>,
    pub(crate) commit_github_base: std::cell::RefCell<Option<Option<String>>>,
    pub(crate) hover_card: std::cell::RefCell<Option<crate::app::scm_tooltips::CommitHoverCard>>,
    /// How many commits the Graph asks git for; it grows as the list is scrolled to its end.
    pub(crate) graph_limit: usize,
    pub(crate) graph_selected_hash: Option<String>,
    pub(crate) graph_expanded: std::collections::HashSet<String>,
    pub(crate) graph_commit_files: HashMap<String, Vec<crate::vcs::scm_graph::CommitChangedFile>>,
    pub(crate) graph_selected_file: Option<(String, String)>,
    pub(crate) scroll: usize,
    pub(crate) commit_message: String,
    pub(crate) commit_pending: bool,
    /// Source control operation in progress, for the animated button.
    pub(crate) busy: Option<(crate::app::scm_progress::ScmBusyKind, std::time::Instant)>,
    pub(crate) publish_modal: Option<crate::app::scm_publish::ScmPublishModal>,
    pub(crate) commit_prompt: bool,
    pub(crate) commit_history: Vec<String>,
    pub(crate) commit_history_index: Option<usize>,
    /// The extension source control whose commit box has the keyboard focus.
    pub(crate) extension_input: Option<u64>,
}

impl ScmState {
    pub(crate) fn new(watcher: Option<notify::RecommendedWatcher>) -> Self {
        Self {
            graph_menu: None,
            more_menu: None,
            confirm: None,
            watcher,
            refresh_gate: Default::default(),
            snapshot: crate::scm::Snapshot::default(),
            visible: false,
            focused: false,
            commit_save_checked: false,
            selected: 0,
            repositories_collapsed: false,
            staged_collapsed: false,
            view_as_tree: false,
            collapsed_folders: Default::default(),
            batch_paths: Vec::new(),
            changes_collapsed: false,
            graph_collapsed: false,
            graph_height: None,
            graph_scroll: 0,
            graph_resize_dragging: false,
            graph_rows: Vec::new(),
            commit_stats: std::cell::RefCell::new(HashMap::new()),
            commit_github_base: std::cell::RefCell::new(None),
            hover_card: std::cell::RefCell::new(None),
            graph_limit: crate::app::scm_graph_ui::GRAPH_PAGE,
            graph_selected_hash: None,
            graph_expanded: std::collections::HashSet::new(),
            graph_commit_files: HashMap::new(),
            graph_selected_file: None,
            scroll: 0,
            commit_message: String::new(),
            commit_pending: false,
            busy: None,
            publish_modal: None,
            commit_prompt: false,
            commit_history: Vec::new(),
            commit_history_index: None,
            extension_input: None,
        }
    }
}
