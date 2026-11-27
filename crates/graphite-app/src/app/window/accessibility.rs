use super::activity_bar::Activity;
use super::*;

const KIND_SHIFT: u32 = 60;
const GROUP_SHIFT: u32 = 32;
const GROUP_MASK: u64 = (1 << 28) - 1;
const VALUE_MASK: u64 = u32::MAX as u64;
const GROUP: u64 = 1;
const TAB_LIST: u64 = 2;
const EDITOR: u64 = 3;
const TAB: u64 = 4;
const LINE: u64 = 5;
const ACTIVITY_TOOLBAR: u64 = 6;
const ACTIVITY_BUTTON: u64 = 7;
const WORKBENCH_MENU_BAR: u64 = 20;
const WORKBENCH_MENU_BUTTON: u64 = 21;
const WINDOW_CONTROL_BUTTON: u64 = 30;
const SETTINGS_GEAR: u64 = 0xABDE_FFFE;
const PROFILE_BUTTON: u64 = 0xAC10_FFFE;

fn node_id(kind: u64, group: u64, value: u64) -> NodeId {
    NodeId((kind << KIND_SHIFT) | ((group & GROUP_MASK) << GROUP_SHIFT) | (value & VALUE_MASK))
}
pub(crate) fn group_node(group: u64) -> NodeId {
    node_id(GROUP, group, 0)
}
pub(crate) fn editor_node(group: u64) -> NodeId {
    node_id(EDITOR, group, 0)
}
pub(crate) fn tab_node(group: u64, document_id: DocumentId) -> NodeId {
    node_id(TAB, group, document_id.0)
}
pub(crate) fn line_node(group: u64, line: usize) -> NodeId {
    node_id(LINE, group, line as u64)
}
pub(crate) fn node_target(id: NodeId) -> (u64, u64, u64) {
    (
        id.0 >> KIND_SHIFT,
        (id.0 >> GROUP_SHIFT) & GROUP_MASK,
        id.0 & VALUE_MASK,
    )
}
pub(crate) fn editor_target(id: NodeId) -> Option<u64> {
    let (kind, group, _) = node_target(id);
    (kind == EDITOR || kind == GROUP).then_some(group)
}
pub(crate) fn tab_target(id: NodeId) -> Option<(u64, DocumentId)> {
    let (kind, group, document) = node_target(id);
    (kind == TAB).then_some((group, DocumentId(document)))
}
pub(crate) fn line_target(id: NodeId) -> Option<(u64, usize)> {
    let (kind, group, line) = node_target(id);
    (kind == LINE).then_some((group, line as usize))
}
pub(crate) fn activity_toolbar_node() -> NodeId {
    NodeId(ACTIVITY_TOOLBAR)
}
pub(crate) fn settings_gear_node() -> NodeId {
    NodeId(SETTINGS_GEAR)
}
pub(crate) fn profile_button_node() -> NodeId {
    NodeId(PROFILE_BUTTON)
}
pub(crate) fn profile_menu_popup_node() -> NodeId {
    NodeId(0xAC10_FFFF)
}
pub(crate) fn profile_menu_item_node(index: usize) -> NodeId {
    NodeId(0xAC10_0000 + index as u64)
}
pub(crate) fn profile_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xAC10_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn settings_menu_popup_node() -> NodeId {
    NodeId(0xABDE_FFFF)
}
pub(crate) fn settings_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABDE_0000 + index as u64)
}
pub(crate) fn settings_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABDE_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn settings_submenu_popup_node() -> NodeId {
    NodeId(0xABDF_FFFF)
}
pub(crate) fn settings_submenu_item_node(index: usize) -> NodeId {
    NodeId(0xABDF_0000 + index as u64)
}
pub(crate) fn settings_submenu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABDF_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn workbench_menu_bar_node() -> NodeId {
    NodeId(WORKBENCH_MENU_BAR)
}
pub(crate) fn workbench_menu_button_node(index: usize) -> NodeId {
    NodeId(WORKBENCH_MENU_BUTTON + index as u64)
}
pub(crate) fn workbench_menu_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(WORKBENCH_MENU_BUTTON)? as usize;
    (index <= super::menu_bar::WorkbenchMenu::ALL.len()).then_some(index)
}
pub(crate) fn file_menu_popup_node() -> NodeId {
    NodeId(0xABCD_FFFF)
}
pub(crate) fn file_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABCD_0000 + index as u64)
}
pub(crate) fn file_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABCD_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn edit_menu_popup_node() -> NodeId {
    NodeId(0xABCE_FFFF)
}
pub(crate) fn edit_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABCE_0000 + index as u64)
}
pub(crate) fn edit_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABCE_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn selection_menu_popup_node() -> NodeId {
    NodeId(0xABD8_FFFF)
}
pub(crate) fn selection_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABD8_0000 + index as u64)
}
pub(crate) fn selection_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD8_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn go_menu_popup_node() -> NodeId {
    NodeId(0xABD9_FFFF)
}
pub(crate) fn go_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABD9_0000 + index as u64)
}
pub(crate) fn go_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD9_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn go_submenu_node() -> NodeId {
    NodeId(0xABDA_FFFF)
}
pub(crate) fn go_submenu_item_node(index: usize) -> NodeId {
    NodeId(0xABDA_0000 + index as u64)
}
pub(crate) fn go_submenu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABDA_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn go_picker_node() -> NodeId {
    NodeId(0xABDB_FFFF)
}
pub(crate) fn go_picker_query_node() -> NodeId {
    NodeId(0xABDC_FFFF)
}
pub(crate) fn go_picker_item_node(index: usize) -> NodeId {
    NodeId(0xABDD_0000 + index as u64)
}
pub(crate) fn go_picker_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABDD_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn command_center_back_node() -> NodeId {
    NodeId(0xAC20_FFFD)
}
pub(crate) fn command_center_forward_node() -> NodeId {
    NodeId(0xAC20_FFFE)
}
pub(crate) fn command_center_search_node() -> NodeId {
    NodeId(0xAC20_FFFF)
}
pub(crate) fn command_center_popup_node() -> NodeId {
    NodeId(0xAC21_FFFF)
}
pub(crate) fn command_center_item_node(index: usize) -> NodeId {
    NodeId(0xAC21_0000 + index as u64)
}
pub(crate) fn command_center_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xAC21_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn terminal_menu_popup_node() -> NodeId {
    NodeId(0xABD0_FFFF)
}
pub(crate) fn terminal_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABD0_0000 + index as u64)
}
pub(crate) fn terminal_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD0_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn view_menu_popup_node() -> NodeId {
    NodeId(0xABCF_FFFF)
}
pub(crate) fn view_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABCF_0000 + index as u64)
}
pub(crate) fn view_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABCF_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn help_menu_popup_node() -> NodeId {
    NodeId(0xABD1_FFFF)
}
pub(crate) fn help_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABD1_0000 + index as u64)
}
pub(crate) fn help_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD1_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn help_modal_node() -> NodeId {
    NodeId(0xABD2_FFFF)
}
pub(crate) fn help_modal_action_node(index: usize) -> NodeId {
    NodeId(0xABD3_0000 + index as u64)
}
pub(crate) fn help_modal_action_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD3_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn help_modal_search_node() -> NodeId {
    NodeId(0xABD4_FFFF)
}
pub(crate) fn run_menu_popup_node() -> NodeId {
    NodeId(0xABD5_FFFF)
}
pub(crate) fn run_menu_item_node(index: usize) -> NodeId {
    NodeId(0xABD5_0000 + index as u64)
}
pub(crate) fn run_menu_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD5_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn run_breakpoint_menu_node() -> NodeId {
    NodeId(0xABD6_FFFF)
}
pub(crate) fn run_breakpoint_item_node(index: usize) -> NodeId {
    NodeId(0xABD6_0000 + index as u64)
}
pub(crate) fn run_breakpoint_item_target(id: NodeId) -> Option<usize> {
    let index = id.0.checked_sub(0xABD6_0000)? as usize;
    (index < 0xFFFF).then_some(index)
}
pub(crate) fn run_breakpoint_prompt_node() -> NodeId {
    NodeId(0xABD7_FFFF)
}
pub(crate) fn run_breakpoint_prompt_input_node() -> NodeId {
    NodeId(0xABD7_FFFE)
}
pub(crate) fn window_control_node(control: super::menu_bar::WindowControl) -> NodeId {
    let index = match control {
        super::menu_bar::WindowControl::Minimize => 0,
        super::menu_bar::WindowControl::MaximizeRestore => 1,
        super::menu_bar::WindowControl::Close => 2,
    };
    NodeId(WINDOW_CONTROL_BUTTON + index)
}
pub(crate) fn window_control_target(id: NodeId) -> Option<super::menu_bar::WindowControl> {
    match id.0.checked_sub(WINDOW_CONTROL_BUTTON)? {
        0 => Some(super::menu_bar::WindowControl::Minimize),
        1 => Some(super::menu_bar::WindowControl::MaximizeRestore),
        2 => Some(super::menu_bar::WindowControl::Close),
        _ => None,
    }
}
pub(crate) fn activity_button_node(activity: Activity) -> NodeId {
    let index = Activity::ALL
        .iter()
        .position(|candidate| *candidate == activity)
        .expect("activity is part of the bar");
    NodeId(ACTIVITY_BUTTON + index as u64)
}
pub(crate) fn activity_target(id: NodeId) -> Option<Activity> {
    let index = id.0.checked_sub(ACTIVITY_BUTTON)? as usize;
    Activity::ALL.get(index).copied()
}
pub(crate) fn plugin_activity_target(id: NodeId, key: &str) -> bool {
    id == plugin_activity_node(key)
}

fn plugin_activity_node(key: &str) -> NodeId {
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    NodeId((1 << 63) | (hash & !(1 << 63)))
}
pub(crate) fn line_content(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

pub(crate) struct AccessibilityActivation;
impl ActivationHandler for AccessibilityActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        let translator = graphite_i18n::Translator::new(graphite_i18n::Language::detect());
        Some(accessibility_tree(
            &[],
            translator.text("accessibility.editor_initial"),
            None,
            false,
            translator,
        ))
    }
}
#[derive(Debug)]
pub(crate) enum UserEvent {
    /// A background thread left work in a queue the loop reads; see `app::wake`.
    Wake,
    /// The workspace file index finished building; see `app::file_index`.
    FileIndexReady {
        generation: u64,
        index: std::sync::Arc<graphite_workspace::FileIndex>,
    },
    /// Files were created or removed in the workspace.
    FileIndexStale,
    ExtensionHostWake,
    CommandCompleted {
        execution: u64,
        event: Option<Box<UserEvent>>,
        result: super::command_execution::CommandResult,
    },
    SettingsImported(Box<super::command_handlers::configuration::ImportedSettings>),
    SnippetsLoaded {
        snippets: Vec<config::Snippet>,
        warnings: Vec<String>,
    },
    LspCommandEvent(Box<crate::lsp::LspUiEvent>),
    AccessibilityWake,
    LspWake,
    TerminalWake,
    TaskOutput(String),
    TaskProcessStarted(u32),
    TaskFinished {
        result: Result<i32, String>,
        diagnostics: Vec<crate::app::phase5::TaskDiagnostic>,
    },
    ScmRefresh,
    /// Workspace file changes, for the file watchers of extensions.
    FileEvents(Vec<(crate::app::plugins::FileChange, std::path::PathBuf)>),
    /// Result of loading the Git history for the history view.
    ScmHistory(Result<String, String>),
    /// Progress of a GitHub sign-in running on a background thread.
    GitHubAuth(crate::app::github_auth::GitHubAuthEvent),
    /// Streaming and tool-approval events from background chat/agent turns.
    Chat(crate::app::chat::ChatUiEvent),
    /// User Data Sync completion / reset notifications.
    Sync(crate::app::user_data_sync::SyncUiEvent),
    /// Application update check and download notifications.
    Update(crate::app::updates::UpdateUiEvent),
    /// Newly recorded runtime error to display in the bottom-right notification toast.
    ErrorRecorded(crate::app::error_log::ErrorNotificationItem),
    /// Re-checks open documents against disk after recently written files settle.
    ExternalChangeCheck,
    /// A notebook kernel has events to collect.
    KernelWake,
    /// The files of a multi-file diff were read on a background thread.
    MultiDiffReady {
        set: crate::scm_changes::ChangeSet,
        result: Result<Vec<crate::multi_diff::FileInput>, String>,
    },
    /// A refreshed snapshot and the folder it was read from; one for another folder is stale.
    ScmSnapshot(PathBuf, crate::scm::Snapshot),
    ScmGraph(
        PathBuf,
        Result<Vec<crate::vcs::scm_graph::GraphRow>, String>,
    ),
    ScmOperationCompleted {
        message: String,
        snapshot: Option<crate::scm::Snapshot>,
    },
    /// A clone finished: the new folder, or why it failed.
    CloneCompleted(Result<PathBuf, String>),
    BranchCheckoutCompleted {
        message: String,
        snapshot: crate::scm::Snapshot,
        documents: Vec<(PathBuf, Result<crate::file_format::DecodedFile, String>)>,
    },
    DiffReady(crate::app::scm_ui::DiffView),
    ConflictReady(crate::app::scm_ui::ConflictView),
    CliRequest {
        request: crate::cli::CliRequest,
        acknowledge: std::sync::mpsc::Sender<()>,
    },
    ProcessSnapshot(Vec<crate::process_explorer::ProcessMetric>),
    ScreencastTick,
    ProjectSearch {
        query: String,
        generation: u64,
        results: Vec<SearchResult>,
    },
    QuickOpen {
        query: String,
        generation: u64,
        paths: Vec<PathBuf>,
    },
    GoSymbolsReady {
        generation: u64,
        items: Vec<super::go_menu::GoSymbolItem>,
    },
}

pub(crate) struct AccessibilityActionQueue {
    pub(crate) queue: Arc<std::sync::Mutex<Vec<ActionRequest>>>,
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}
impl ActionHandler for AccessibilityActionQueue {
    fn do_action(&mut self, request: ActionRequest) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.push(request);
        }
        let _ = self.proxy.send_event(UserEvent::AccessibilityWake);
    }
}
pub(crate) struct AccessibilityDeactivation;
impl DeactivationHandler for AccessibilityDeactivation {
    fn deactivate_accessibility(&mut self) {}
}
pub(crate) fn accessibility_tree(
    titles: &[String],
    value: &str,
    panel: Option<&str>,
    read_only: bool,
    translator: graphite_i18n::Translator,
) -> TreeUpdate {
    let root = NodeId(0);
    let tablist = NodeId(1);
    let editor = NodeId(2);
    let tab_ids: Vec<_> = (0..titles.len()).map(|i| NodeId(10 + i as u64)).collect();
    let mut root_node = Node::new(Role::Window);
    let mut root_children = vec![tablist, editor, NodeId(3), NodeId(4)];
    if panel.is_some() {
        root_children.push(NodeId(5));
    }
    root_node.set_children(root_children);
    root_node.set_label(translator.text("accessibility.app_name"));
    let mut tabs_node = Node::new(Role::TabList);
    tabs_node.set_children(tab_ids.clone());
    let mut nodes = vec![(root, root_node), (tablist, tabs_node)];
    let mut terminal_button = Node::new(Role::Button);
    terminal_button.set_label(translator.text("accessibility.toggle_terminal"));
    nodes.push((NodeId(3), terminal_button));
    let mut problems_button = Node::new(Role::Button);
    problems_button.set_label(translator.text("accessibility.show_diagnostics"));
    nodes.push((NodeId(4), problems_button));
    for (id, title) in tab_ids.iter().zip(titles) {
        let mut tab = Node::new(Role::Document);
        tab.set_label(title.as_str());
        nodes.push((*id, tab));
    }
    let mut editor_node = Node::new(Role::TextInput);
    if read_only {
        editor_node.set_read_only();
    }
    editor_node.set_value(value);
    let lines: Vec<_> = value.split_inclusive('\n').collect();
    let run_ids: Vec<_> = (0..lines.len()).map(|i| NodeId(1_000 + i as u64)).collect();
    editor_node.set_children(run_ids.clone());
    nodes.push((editor, editor_node));
    for (id, line) in run_ids.iter().zip(lines) {
        let text = line.trim_end_matches(['\n', '\r']);
        let mut run = Node::new(Role::TextRun);
        run.set_value(text);
        run.set_character_lengths(vec![1; text.chars().count()]);
        nodes.push((*id, run));
    }
    if let Some(panel) = panel {
        let mut panel_node = Node::new(Role::Label);
        panel_node.set_label(panel);
        nodes.push((NodeId(5), panel_node));
    }
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(root)),
        tree_id: accesskit::TreeId::ROOT,
        focus: if panel.is_some() { NodeId(5) } else { editor },
    }
}

pub(crate) const MODE_SELECTOR_NODE: NodeId = NodeId(6);
pub(crate) const AI_CHAT_NODE: NodeId = NodeId(7);
pub(crate) const AI_CLOSE_FILE_NODE: NodeId = NodeId(8);

impl App {
    /// Build one accessible editor and tab list for each visible group.
    pub(crate) fn grouped_accessibility_tree(
        &mut self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> TreeUpdate {
        if self.plugin_owns_keyboard() {
            return self.plugin_accessibility();
        }
        let rects = self.group_rectangles(width, height, line_height);
        let panel = self.accessible_panel_text();
        let panel_focus = panel.is_some() && self.terminal_focused;
        let root = NodeId(0);
        let mut root_node = Node::new(Role::Window);
        root_node.set_label(self.t("accessibility.app_name"));
        let toolbar = (self.ai_mode.is_none()
            && !self.zen_mode
            && self.activity_bar_preferences.position
                != crate::config::ActivityBarPosition::Hidden)
            .then(activity_toolbar_node);
        let mut children = Vec::new();
        children.push(workbench_menu_bar_node());
        if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::File) {
            children.push(file_menu_popup_node());
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Edit) {
            children.push(edit_menu_popup_node());
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Selection) {
            children.push(selection_menu_popup_node());
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Go) {
            children.push(go_menu_popup_node());
            if self.go_submenu.is_some() {
                children.push(go_submenu_node());
            }
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Terminal) {
            children.push(terminal_menu_popup_node());
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Help) {
            children.push(help_menu_popup_node());
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Run) {
            children.push(run_menu_popup_node());
            if self.run_breakpoint_submenu {
                children.push(run_breakpoint_menu_node());
            }
        }
        if self.run_breakpoint_prompt.is_some() {
            children.push(run_breakpoint_prompt_node());
        }
        if self.help_modal.is_some() {
            children.push(help_modal_node());
        }
        if self.settings_menu.is_some() {
            children.push(settings_menu_popup_node());
            if self
                .settings_menu
                .as_ref()
                .is_some_and(|menu| menu.submenu.is_some())
            {
                children.push(settings_submenu_popup_node());
            }
        }
        if self.profile_menu.is_some() {
            children.push(profile_menu_popup_node());
        }
        if self.go_picker.is_some() && self.command_center.is_none() {
            children.push(go_picker_node());
        }
        if self.command_center.is_some() {
            children.push(command_center_popup_node());
        }
        children.extend([
            window_control_node(super::menu_bar::WindowControl::Minimize),
            window_control_node(super::menu_bar::WindowControl::MaximizeRestore),
            window_control_node(super::menu_bar::WindowControl::Close),
        ]);
        if let Some(toolbar) = toolbar {
            children.push(toolbar);
        }
        children.extend(rects.iter().map(|rect| group_node(rect.id)));
        children.extend([NodeId(3), MODE_SELECTOR_NODE]);
        if self.ai_mode.is_some() {
            children.push(AI_CHAT_NODE);
            if self.ai_file_group().is_some() {
                children.push(AI_CLOSE_FILE_NODE);
            }
        } else {
            children.push(NodeId(4));
        }
        if panel.is_some() {
            children.push(NodeId(5));
        }
        root_node.set_children(children);
        let mut nodes = vec![(root, root_node)];
        let mut menu_bar = Node::new(Role::Toolbar);
        menu_bar.set_label(self.t("accessibility.menu_bar"));
        let menu_items = self.titlebar_accessible_menus(
            width,
            self.gpu
                .as_ref()
                .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width()),
        );
        let menu_ids = menu_items
            .iter()
            .map(|menu| {
                let index = super::menu_bar::WorkbenchMenu::ALL
                    .iter()
                    .position(|candidate| candidate == menu)
                    .unwrap_or(super::menu_bar::WorkbenchMenu::ALL.len());
                workbench_menu_button_node(index)
            })
            .collect::<Vec<_>>();
        let (back_rect, forward_rect, search_rect) = self.command_center_rects(
            width,
            self.gpu
                .as_ref()
                .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width()),
        );
        for (id, label, rect, enabled) in [
            (
                command_center_back_node(),
                "command_center.back",
                back_rect,
                self.can_navigate_history(-1),
            ),
            (
                command_center_forward_node(),
                "command_center.forward",
                forward_rect,
                self.can_navigate_history(1),
            ),
        ] {
            let mut button = Node::new(Role::Button);
            button.set_label(self.t(label));
            if enabled {
                button.add_action(Action::Click);
            } else {
                button.set_disabled();
            }
            button.set_bounds(accesskit::Rect {
                x0: rect.0 as f64,
                y0: rect.1 as f64,
                x1: (rect.0 + rect.2) as f64,
                y1: (rect.1 + rect.3) as f64,
            });
            nodes.push((id, button));
        }
        let mut search = Node::new(Role::TextInput);
        search.set_label(self.t("command_center.accessible_label"));
        search.set_value(
            self.command_center
                .as_ref()
                .map_or("", |center| center.query.as_str()),
        );
        search.add_action(Action::Focus);
        search.add_action(Action::Click);
        search.set_bounds(accesskit::Rect {
            x0: search_rect.0 as f64,
            y0: search_rect.1 as f64,
            x1: (search_rect.0 + search_rect.2) as f64,
            y1: (search_rect.1 + search_rect.3) as f64,
        });
        nodes.push((command_center_search_node(), search));
        menu_bar.set_children(
            menu_ids
                .iter()
                .copied()
                .chain([
                    command_center_back_node(),
                    command_center_forward_node(),
                    command_center_search_node(),
                ])
                .collect::<Vec<_>>(),
        );
        nodes.push((workbench_menu_bar_node(), menu_bar));
        for menu in menu_items {
            let index = super::menu_bar::WorkbenchMenu::ALL
                .iter()
                .position(|candidate| *candidate == menu)
                .unwrap_or(super::menu_bar::WorkbenchMenu::ALL.len());
            let mut button = Node::new(Role::Button);
            button.set_label(self.t(menu.label_key()));
            button.set_selected(self.workbench_menu == Some(menu));
            button.add_action(Action::Click);
            nodes.push((workbench_menu_button_node(index), button));
        }
        if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::File) {
            use super::file_menu::FileMenuRow;
            use super::settings_menu::SettingsSubmenuRow;
            // (label, shortcut, enabled, checked) of every selectable row of the innermost panel.
            let entries: Vec<(String, String, bool, bool)> = match self.file_menu_nested {
                Some(nested) => self
                    .file_nested_rows(nested)
                    .into_iter()
                    .filter_map(|row| match row {
                        SettingsSubmenuRow::Item {
                            label,
                            shortcut,
                            enabled,
                            checked,
                            ..
                        } => Some((self.t(&label).to_owned(), shortcut, enabled, checked)),
                        SettingsSubmenuRow::Separator => None,
                    })
                    .collect(),
                None => self
                    .file_menu_rows(self.file_menu_submenu)
                    .into_iter()
                    .filter_map(|row| match row {
                        FileMenuRow::Item {
                            command,
                            label,
                            shortcut,
                            enabled,
                            checked,
                            ..
                        } => Some((
                            self.file_menu_label(label, command),
                            shortcut.to_owned(),
                            enabled,
                            checked,
                        )),
                        FileMenuRow::Separator => None,
                    })
                    .collect(),
            };
            let selected_row = match self.file_menu_nested {
                Some(_) => self.file_menu_nested_selection,
                None => self.file_menu_selection,
            };
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.file"));
            let mut item_children = Vec::new();
            for (item_index, (label, shortcut, enabled, checked)) in entries.into_iter().enumerate()
            {
                {
                    let id = file_menu_item_node(item_index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut accessible_label = label;
                    if !shortcut.is_empty() {
                        accessible_label.push_str(&format!(", {shortcut}"));
                    }
                    if checked {
                        accessible_label.push_str(", checked");
                    }
                    if !enabled {
                        accessible_label
                            .push_str(&format!(", {}", self.t("accessibility.unavailable")));
                    }
                    item.set_label(accessible_label);
                    item.set_selected(item_index == selected_row);
                    item.add_action(Action::Click);
                    nodes.push((id, item));
                    item_children.push(id);
                }
            }
            menu.set_children(item_children);
            nodes.push((file_menu_popup_node(), menu));
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Edit) {
            use super::edit_menu::EditMenuRow;
            let rows = self.edit_menu_rows();
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.edit"));
            let mut item_children = Vec::new();
            let mut item_index = 0;
            for row in rows {
                if let EditMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    ..
                } = row
                {
                    let id = edit_menu_item_node(item_index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut accessible_label = self.edit_menu_label(label);
                    if !shortcut.is_empty() {
                        accessible_label.push_str(&format!(", {shortcut}"));
                    }
                    if !enabled {
                        accessible_label.push_str(", unavailable");
                    }
                    item.set_label(accessible_label);
                    item.set_selected(item_index == self.edit_menu_selection);
                    item.add_action(Action::Click);
                    nodes.push((id, item));
                    item_children.push(id);
                    item_index += 1;
                }
            }
            menu.set_children(item_children);
            nodes.push((edit_menu_popup_node(), menu));
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Selection) {
            use super::selection_menu::SelectionMenuRow;
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.selection"));
            let mut children = Vec::new();
            let mut index = 0;
            for row in self.selection_menu_rows() {
                if let SelectionMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    checked,
                    ..
                } = row
                {
                    let id = selection_menu_item_node(index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut name = self.t(label).to_owned();
                    if !shortcut.is_empty() {
                        name.push_str(&format!(", {shortcut}"));
                    }
                    if checked {
                        name.push_str(", checked");
                    }
                    if !enabled {
                        name.push_str(", unavailable");
                    }
                    item.set_label(name);
                    item.set_selected(index == self.selection_menu_selection);
                    if enabled {
                        item.add_action(Action::Click);
                    }
                    nodes.push((id, item));
                    children.push(id);
                    index += 1;
                }
            }
            menu.set_children(children);
            nodes.push((selection_menu_popup_node(), menu));
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Go) {
            use super::go_menu::{GoMenuRow, GoSubmenuRow};
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.go"));
            let mut children = Vec::new();
            let mut index = 0;
            for row in self.go_menu_rows() {
                if let GoMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    submenu,
                    ..
                } = row
                {
                    let id = go_menu_item_node(index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut name = self.t(label).to_owned();
                    if !shortcut.is_empty() {
                        name.push_str(&format!(", {shortcut}"));
                    }
                    if !enabled {
                        name.push_str(", unavailable");
                    }
                    if submenu.is_some() {
                        name.push_str(", submenu");
                    }
                    item.set_label(name);
                    item.set_selected(index == self.go_menu_selection);
                    item.add_action(Action::Click);
                    nodes.push((id, item));
                    children.push(id);
                    index += 1;
                }
            }
            menu.set_children(children);
            nodes.push((go_menu_popup_node(), menu));
            if let Some(submenu_kind) = self.go_submenu {
                let mut submenu = Node::new(Role::Menu);
                submenu.set_label(self.t(match submenu_kind {
                    super::go_menu::GoSubmenu::SwitchEditor => "go.switch_editor",
                    super::go_menu::GoSubmenu::SwitchGroup => "go.switch_group",
                }));
                let mut children = Vec::new();
                let mut index = 0;
                for row in self.go_submenu_rows(submenu_kind) {
                    if let GoSubmenuRow::Item {
                        label,
                        shortcut,
                        enabled,
                        ..
                    } = row
                    {
                        let id = go_submenu_item_node(index);
                        let mut item = Node::new(Role::MenuItem);
                        let mut name = self.t(label).to_owned();
                        if !shortcut.is_empty() {
                            name.push_str(&format!(", {shortcut}"));
                        }
                        if !enabled {
                            name.push_str(", unavailable");
                        }
                        item.set_label(name);
                        item.set_selected(index == self.go_submenu_selection);
                        item.add_action(Action::Click);
                        nodes.push((id, item));
                        children.push(id);
                        index += 1;
                    }
                }
                submenu.set_children(children);
                nodes.push((go_submenu_node(), submenu));
            }
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Terminal) {
            use super::terminal_menu::TerminalMenuRow;
            let rows = self.terminal_menu_rows();
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.terminal"));
            let mut item_children = Vec::new();
            let mut item_index = 0;
            for row in rows {
                if let TerminalMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    ..
                } = row
                {
                    let id = terminal_menu_item_node(item_index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut accessible_label = self.t(label).to_owned();
                    if !shortcut.is_empty() {
                        accessible_label.push_str(&format!(", {shortcut}"));
                    }
                    if !enabled {
                        accessible_label.push_str(", unavailable");
                    }
                    item.set_label(accessible_label);
                    item.set_selected(item_index == self.terminal_menu_selection);
                    item.add_action(Action::Click);
                    nodes.push((id, item));
                    item_children.push(id);
                    item_index += 1;
                }
            }
            menu.set_children(item_children);
            nodes.push((terminal_menu_popup_node(), menu));
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::View) {
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.view"));
            let mut item_children = Vec::new();
            for (index, (label, enabled, checked, separator)) in
                self.view_menu_accessible_rows().into_iter().enumerate()
            {
                if separator {
                    continue;
                }
                let id = view_menu_item_node(index);
                let mut item = Node::new(Role::MenuItem);
                let label = if checked {
                    format!("{label}, checked")
                } else {
                    label
                };
                item.set_label(if enabled {
                    label
                } else {
                    format!("{label}, {}", self.t("accessibility.unavailable"))
                });
                item.set_selected(
                    self.view_menu
                        .as_ref()
                        .is_some_and(|state| state.active_selected() == index),
                );
                if enabled {
                    item.add_action(Action::Click);
                }
                nodes.push((id, item));
                item_children.push(id);
            }
            menu.set_children(item_children);
            nodes.push((view_menu_popup_node(), menu));
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Help) {
            use super::help_menu::HelpRow;
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.help"));
            let mut item_children = Vec::new();
            let mut index = 0;
            for row in self.help_menu_rows() {
                if let HelpRow::Item {
                    label,
                    shortcut,
                    enabled,
                    ..
                } = row
                {
                    let id = help_menu_item_node(index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut name = self.t(label).to_owned();
                    if !shortcut.is_empty() {
                        name.push_str(&format!(", {shortcut}"));
                    }
                    item.set_label(name);
                    item.set_selected(index == self.help_menu_selection);
                    if enabled {
                        item.add_action(Action::Click);
                    }
                    nodes.push((id, item));
                    item_children.push(id);
                    index += 1;
                }
            }
            menu.set_children(item_children);
            nodes.push((help_menu_popup_node(), menu));
        } else if self.workbench_menu == Some(super::menu_bar::WorkbenchMenu::Run) {
            use super::run_menu::RunMenuRow;
            let mut menu = Node::new(Role::Menu);
            menu.set_label(self.t("menubar.run"));
            let mut children = Vec::new();
            let mut index = 0;
            for row in self.run_menu_rows() {
                if let RunMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    unavailable_reason,
                    ..
                } = row
                {
                    let id = run_menu_item_node(index);
                    let mut item = Node::new(Role::MenuItem);
                    let mut name = self.t(label).to_owned();
                    if !shortcut.is_empty() {
                        name.push_str(&format!(", {shortcut}"));
                    }
                    if !enabled {
                        if let Some(reason) = unavailable_reason {
                            name.push_str(&format!(", {}", self.t(reason)));
                        }
                    }
                    item.set_label(name);
                    item.set_selected(index == self.run_menu_selection);
                    if enabled {
                        item.add_action(Action::Click);
                    }
                    nodes.push((id, item));
                    children.push(id);
                    index += 1;
                }
            }
            menu.set_children(children);
            nodes.push((run_menu_popup_node(), menu));
            if self.run_breakpoint_submenu {
                let mut submenu = Node::new(Role::Menu);
                submenu.set_label(self.t("run.new_breakpoint"));
                let mut children = Vec::new();
                let mut index = 0;
                for row in self.run_breakpoint_rows() {
                    if let RunMenuRow::Item {
                        label,
                        shortcut,
                        enabled,
                        unavailable_reason,
                        ..
                    } = row
                    {
                        let id = run_breakpoint_item_node(index);
                        let mut item = Node::new(Role::MenuItem);
                        let mut name = self.t(label).to_owned();
                        if !shortcut.is_empty() {
                            name.push_str(&format!(", {shortcut}"));
                        }
                        if !enabled {
                            if let Some(reason) = unavailable_reason {
                                name.push_str(&format!(", {}", self.t(reason)));
                            }
                        }
                        item.set_label(name);
                        item.set_selected(index == self.run_breakpoint_selection);
                        if enabled {
                            item.add_action(Action::Click);
                        }
                        nodes.push((id, item));
                        children.push(id);
                        index += 1;
                    }
                }
                submenu.set_children(children);
                nodes.push((run_breakpoint_menu_node(), submenu));
            }
        }
        if let Some(prompt) = &self.run_breakpoint_prompt {
            let mut dialog = Node::new(Role::Dialog);
            let title = match prompt.kind {
                super::run_menu::BreakpointPromptKind::Conditional => {
                    self.t("run.prompt.conditional")
                }
                super::run_menu::BreakpointPromptKind::Edit => self.t("run.prompt.edit"),
                super::run_menu::BreakpointPromptKind::Logpoint => self.t("run.prompt.logpoint"),
            };
            dialog.set_label(title);
            let mut input = Node::new(Role::TextInput);
            input.set_label(title);
            input.set_value(prompt.value.as_str());
            input.add_action(Action::Focus);
            dialog.set_children(vec![run_breakpoint_prompt_input_node()]);
            nodes.push((run_breakpoint_prompt_input_node(), input));
            nodes.push((run_breakpoint_prompt_node(), dialog));
        }
        if let Some(modal) = &self.help_modal {
            let mut dialog = Node::new(Role::Dialog);
            dialog.set_label(self.help_modal_accessible_label(modal));
            let mut dialog_children = vec![help_modal_action_node(0)];
            let mut actions = vec![self.t("help.close_button").to_owned()];
            match modal.page {
                super::help_menu::HelpPage::Welcome => {
                    actions.extend([
                        self.t("help.new_file").to_owned(),
                        self.t("help.open_file").to_owned(),
                        self.t("help.open_folder").to_owned(),
                    ]);
                    actions.extend(
                        self.recent_workspaces
                            .iter()
                            .take(12)
                            .map(|path| path.display().to_string()),
                    );
                }
                super::help_menu::HelpPage::Playground => actions.extend([
                    self.t("help.playground_rust").to_owned(),
                    self.t("help.playground_javascript").to_owned(),
                ]),
                super::help_menu::HelpPage::Walkthrough => actions.extend([
                    self.t("help.back").to_owned(),
                    if modal.walkthrough_step >= 3 {
                        self.t("help.finish").to_owned()
                    } else {
                        self.t("help.next").to_owned()
                    },
                ]),
                super::help_menu::HelpPage::KeyboardShortcuts => {
                    let search_id = help_modal_search_node();
                    let mut search = Node::new(Role::TextInput);
                    search.set_label(self.t("help.shortcuts_search"));
                    search.set_value(modal.query.as_str());
                    search.add_action(Action::Focus);
                    nodes.push((search_id, search));
                    dialog_children.push(search_id);
                }
                _ => {}
            }
            for (index, label) in actions.into_iter().enumerate() {
                let id = help_modal_action_node(index);
                let mut button = Node::new(Role::Button);
                button.set_label(label);
                button.add_action(Action::Click);
                nodes.push((id, button));
                dialog_children.push(id);
            }
            dialog.set_children(dialog_children);
            nodes.push((help_modal_node(), dialog));
        }
        if let Some(picker) = self
            .go_picker
            .as_ref()
            .filter(|_| self.command_center.is_none())
        {
            let mut dialog = Node::new(Role::Dialog);
            dialog.set_label(self.t(match picker.kind {
                super::go_menu::GoPickerKind::LineColumn => "go.line_column_prompt",
                _ => "go.symbol_prompt",
            }));
            let mut input = Node::new(Role::TextInput);
            input.set_label(self.t(match picker.kind {
                super::go_menu::GoPickerKind::LineColumn => "go.line_column_prompt",
                _ => "go.symbol_prompt",
            }));
            input.set_value(picker.query.as_str());
            input.add_action(Action::Focus);
            nodes.push((go_picker_query_node(), input));
            let mut children = vec![go_picker_query_node()];
            for (index, item) in self.go_picker_rows().iter().take(100).enumerate() {
                let mut button = Node::new(Role::Button);
                button.set_label(format!(
                    "{}, {}:{}",
                    item.name,
                    self.tree.relative_path(&item.path).display(),
                    item.line + 1
                ));
                button.set_selected(index == picker.selected);
                button.add_action(Action::Click);
                nodes.push((go_picker_item_node(index), button));
                children.push(go_picker_item_node(index));
            }
            dialog.set_children(children);
            nodes.push((go_picker_node(), dialog));
        }
        if self.command_center.is_some() {
            let rows = self.command_center_rows();
            let selected = self.command_center_selected_index();
            let mut children = Vec::new();
            for (index, title) in rows.iter().enumerate() {
                let id = command_center_item_node(index);
                let mut item = Node::new(Role::Button);
                item.set_label(title.as_str());
                item.set_selected(selected == Some(index));
                item.add_action(Action::Click);
                nodes.push((id, item));
                children.push(id);
            }
            let mut dialog = Node::new(Role::Dialog);
            dialog.set_label(self.t("command_center.suggestions"));
            dialog.set_children(children);
            nodes.push((command_center_popup_node(), dialog));
        }
        for (control, label) in [
            (
                super::menu_bar::WindowControl::Minimize,
                "titlebar.minimize",
            ),
            (
                super::menu_bar::WindowControl::MaximizeRestore,
                if self
                    .gpu
                    .as_ref()
                    .is_some_and(|gpu| gpu.window.is_maximized())
                {
                    "titlebar.restore"
                } else {
                    "titlebar.maximize"
                },
            ),
            (super::menu_bar::WindowControl::Close, "titlebar.close"),
        ] {
            let mut button = Node::new(Role::Button);
            button.set_label(self.t(label));
            button.add_action(Action::Click);
            nodes.push((window_control_node(control), button));
        }
        if toolbar.is_some() {
            let mut node = Node::new(Role::Toolbar);
            node.set_label(self.t("activity.bar"));
            let builtins = Activity::ALL
                .iter()
                .copied()
                .filter(|activity| {
                    !self
                        .activity_bar_preferences
                        .hidden_items
                        .contains(match activity {
                            Activity::Explorer => "explorer",
                            Activity::Search => "search",
                            Activity::SourceControl => "source_control",
                            Activity::Plugins => "plugins",
                        })
                })
                .map(activity_button_node);
            let plugins = self.plugin_activity_entries().into_iter().filter(|entry| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(&entry.key)
            });
            node.set_children(
                builtins
                    .chain(plugins.map(|entry| plugin_activity_node(&entry.key)))
                    .chain(std::iter::once(profile_button_node()))
                    .chain(std::iter::once(settings_gear_node()))
                    .collect::<Vec<_>>(),
            );
            nodes.push((activity_toolbar_node(), node));
            let mut profile_button = Node::new(Role::Button);
            profile_button.set_label(self.profile_button_accessible_label());
            profile_button.add_action(Action::Click);
            if let Some((x, y, width, height)) =
                super::activity_bar::activity_bar_profile_button_rect(
                    &self.activity_bar_preferences,
                    width,
                    height,
                    self.top_menu_height(),
                    line_height,
                )
            {
                profile_button.set_bounds(accesskit::Rect {
                    x0: x as f64,
                    y0: y as f64,
                    x1: (x + width) as f64,
                    y1: (y + height) as f64,
                });
            }
            nodes.push((profile_button_node(), profile_button));
            let mut gear = Node::new(Role::Button);
            gear.set_label(self.t("settings_menu.gear_label"));
            gear.add_action(Action::Click);
            if let Some((x, y, width, height)) =
                super::activity_bar::activity_bar_settings_gear_rect(
                    &self.activity_bar_preferences,
                    width,
                    height,
                    self.top_menu_height(),
                    line_height,
                )
            {
                gear.set_bounds(accesskit::Rect {
                    x0: x as f64,
                    y0: y as f64,
                    x1: (x + width) as f64,
                    y1: (y + height) as f64,
                });
            }
            nodes.push((settings_gear_node(), gear));
            let active = self.active_activity();
            for activity in Activity::ALL.into_iter().filter(|activity| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(match activity {
                        Activity::Explorer => "explorer",
                        Activity::Search => "search",
                        Activity::SourceControl => "source_control",
                        Activity::Plugins => "plugins",
                    })
            }) {
                let mut button = Node::new(Role::Button);
                button.set_label(self.t(activity.label_key()));
                button.set_selected(activity == active);
                button.add_action(Action::Click);
                nodes.push((activity_button_node(activity), button));
            }
            for entry in self.plugin_activity_entries().into_iter().filter(|entry| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(&entry.key)
            }) {
                let mut button = Node::new(Role::Button);
                button.set_label(entry.title.clone());
                button.set_selected(self.active_plugin_activity.as_deref() == Some(&entry.key));
                button.add_action(Action::Click);
                nodes.push((plugin_activity_node(&entry.key), button));
            }
        }
        if let Some(menu) = &self.settings_menu {
            let rows = self.settings_menu_rows();
            let item_nodes = rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| match row {
                    super::settings_menu::SettingsMenuRow::Item {
                        label,
                        shortcut,
                        enabled,
                        submenu,
                        ..
                    } => {
                        let mut item = Node::new(Role::MenuItem);
                        let mut accessible_label = self.t(label).to_string();
                        if !shortcut.is_empty() {
                            accessible_label.push_str(&format!(", {shortcut}"));
                        }
                        if submenu.is_some() {
                            accessible_label.push_str(", submenu");
                        }
                        item.set_label(accessible_label);
                        if !enabled {
                            item.set_disabled();
                        }
                        item.add_action(Action::Click);
                        Some((settings_menu_item_node(index), item))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut popup = Node::new(Role::Menu);
            popup.set_label(self.t("settings_menu.gear_label"));
            popup.set_children(item_nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>());
            nodes.push((settings_menu_popup_node(), popup));
            nodes.extend(item_nodes);
            if let Some(submenu) = menu.submenu {
                let rows = self.settings_submenu_rows(submenu, &menu.profiles);
                let item_nodes = rows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, row)| match row {
                        super::settings_menu::SettingsSubmenuRow::Item {
                            label,
                            shortcut,
                            enabled,
                            checked,
                            ..
                        } => {
                            let mut item = Node::new(Role::MenuItem);
                            let mut accessible_label = label.clone();
                            if !shortcut.is_empty() {
                                accessible_label.push_str(&format!(", {shortcut}"));
                            }
                            item.set_label(accessible_label);
                            if !enabled {
                                item.set_disabled();
                            }
                            item.set_selected(*checked);
                            item.add_action(Action::Click);
                            Some((settings_submenu_item_node(index), item))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let mut popup = Node::new(Role::Menu);
                popup.set_label(self.t(match submenu {
                    super::settings_menu::SettingsSubmenu::Profiles => "settings_menu.profiles",
                    super::settings_menu::SettingsSubmenu::Themes => "settings_menu.themes",
                    super::settings_menu::SettingsSubmenu::Density => {
                        "settings_menu.layout_density"
                    }
                }));
                popup.set_children(item_nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>());
                nodes.push((settings_submenu_popup_node(), popup));
                nodes.extend(item_nodes);
            }
        }
        if let Some(menu) = &self.profile_menu {
            let mut item_nodes = Vec::new();
            for (index, row) in menu.rows.iter().enumerate() {
                let (label, selected, disabled) = match row {
                    super::profile_menu::ProfileMenuRow::Profile { name, active } => {
                        (name.clone(), *active, false)
                    }
                    super::profile_menu::ProfileMenuRow::Create => {
                        (self.t("profile_menu.create").to_owned(), false, false)
                    }
                    super::profile_menu::ProfileMenuRow::Unavailable => {
                        (self.t("profile_menu.unavailable").to_owned(), false, true)
                    }
                    super::profile_menu::ProfileMenuRow::GitHubSignIn => {
                        (self.t("github.sign_in").to_owned(), false, false)
                    }
                    super::profile_menu::ProfileMenuRow::GitHubSignOut => {
                        (self.t("github.sign_out").to_owned(), false, false)
                    }
                    super::profile_menu::ProfileMenuRow::GitHubAccount { login } => (
                        self.tf("github.signed_in_as", &[("login", login)]),
                        false,
                        true,
                    ),
                    super::profile_menu::ProfileMenuRow::SyncStatus { text } => {
                        (text.clone(), false, true)
                    }
                    super::profile_menu::ProfileMenuRow::SyncTurnOn => {
                        (self.t("sync_menu.turn_on").to_owned(), false, false)
                    }
                    super::profile_menu::ProfileMenuRow::SyncTurnOnRepository => (
                        self.t("sync_menu.turn_on_repository").to_owned(),
                        false,
                        false,
                    ),
                    super::profile_menu::ProfileMenuRow::SyncNow => {
                        (self.t("sync_menu.sync_now").to_owned(), false, false)
                    }
                    super::profile_menu::ProfileMenuRow::SyncTurnOff => {
                        (self.t("sync_menu.turn_off").to_owned(), false, false)
                    }
                    super::profile_menu::ProfileMenuRow::Separator => continue,
                };
                let mut item = Node::new(Role::MenuItem);
                item.set_label(label);
                item.set_selected(selected);
                if disabled {
                    item.set_disabled();
                } else {
                    item.add_action(Action::Click);
                }
                item_nodes.push((profile_menu_item_node(index), item));
            }
            let mut popup = Node::new(Role::Menu);
            popup.set_label(self.t("profile_menu.title"));
            popup.set_children(item_nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>());
            nodes.push((profile_menu_popup_node(), popup));
            nodes.extend(item_nodes);
        }
        for (index, rect) in rects.iter().enumerate() {
            let (views, active) = self
                .editor_groups
                .views(rect.id)
                .expect("layout group exists");
            let view = &views[active];
            let buffer = &self.documents[self
                .resolve_document(view.document)
                .expect("view document exists")]
            .buffer;
            let editor_id = editor_node(rect.id);
            let tab_list_id = node_id(TAB_LIST, rect.id, 0);
            let mut group = Node::new(Role::Group);
            group.set_label(self.tf("accessibility.editor_group", &[("index", &(index + 1))]));
            group.set_bounds(accesskit::Rect {
                x0: rect.x as f64,
                y0: rect.y as f64,
                x1: (rect.x + rect.width) as f64,
                y1: (rect.y + rect.height) as f64,
            });
            group.set_children([tab_list_id, editor_id]);
            group.add_action(Action::Focus);
            nodes.push((group_node(rect.id), group));
            let mut tab_list = Node::new(Role::TabList);
            let ids: Vec<_> = views
                .iter()
                .map(|view| tab_node(rect.id, view.document))
                .collect();
            tab_list.set_children(ids.clone());
            nodes.push((tab_list_id, tab_list));
            for (i, item) in views.iter().enumerate() {
                let tab = &self.documents[self
                    .resolve_document(item.document)
                    .expect("view document exists")];
                let mut node = Node::new(Role::Document);
                node.set_label(super::group_drawing::tab_label(tab, &self.translator).trim());
                node.set_selected(i == active);
                node.add_action(Action::Click);
                node.add_action(Action::Focus);
                nodes.push((ids[i], node));
            }
            let graphite_detail = self
                .plugins
                .active_graphite_detail
                .as_ref()
                .filter(|(group, _)| *group == rect.id)
                .and_then(|(_, repository)| {
                    self.plugins
                        .graphite_details
                        .iter()
                        .find(|tab| tab.group == rect.id && &tab.repository == repository)
                });
            let installed_detail = self
                .plugins
                .active_installed_detail
                .as_ref()
                .filter(|(group, _)| *group == rect.id)
                .and_then(|(_, id)| {
                    self.plugins
                        .installed_details
                        .iter()
                        .find(|tab| tab.group == rect.id && &tab.id == id)
                });
            let mut editor = Node::new(Role::TextInput);
            if let Some(detail) = graphite_detail {
                editor.set_label(format!("{} — README", detail.repository));
                let readme = detail
                    .details
                    .as_ref()
                    .and_then(|details| details.readme.as_deref())
                    .unwrap_or_else(|| {
                        if detail.loading {
                            self.t("plugins.sidebar.loading_details")
                        } else if let Some(error) = &detail.error {
                            error
                        } else {
                            self.t("plugins.sidebar.no_readme")
                        }
                    });
                editor.set_value(if let Some(error) = &detail.error {
                    format!("{error}\n\n{readme}")
                } else {
                    readme.to_owned()
                });
                editor.set_read_only();
                editor.add_action(Action::Focus);
                nodes.push((editor_id, editor));
            } else if let Some(detail) = installed_detail {
                editor.set_label(self.tf(
                    "plugins.sidebar.details_title",
                    &[
                        ("id", &detail.metadata.id()),
                        ("version", &detail.metadata.version()),
                    ],
                ));
                let body = detail.readme.as_deref().unwrap_or(match &detail.metadata {
                    graphite_plugins::manifest::Metadata::Native(m) => m.description.as_str(),
                    graphite_plugins::manifest::Metadata::Vsix(v) => v.description.as_str(),
                });
                editor.set_value(body);
                editor.set_read_only();
                editor.add_action(Action::Focus);
                nodes.push((editor_id, editor));
            } else {
                editor.set_label(self.t("accessibility.editor"));
                editor.set_value(buffer.text());
                if buffer.is_read_only() {
                    editor.set_read_only();
                }
                editor.add_action(Action::Focus);
                editor.add_action(Action::SetTextSelection);
                if !buffer.is_read_only() {
                    editor.add_action(Action::SetValue);
                    editor.add_action(Action::ReplaceSelectedText);
                }
                let runs: Vec<_> = (0..buffer.len_lines())
                    .map(|line| line_node(rect.id, line))
                    .collect();
                editor.set_children(runs.clone());
                let position = |offset: usize| {
                    let offset = offset.min(buffer.len_chars());
                    let line = buffer.rope().char_to_line(offset);
                    let text = buffer.line(line);
                    let max = line_content(&text).chars().count();
                    accesskit::TextPosition {
                        node: runs[line],
                        character_index: (offset - buffer.line_start_char(line)).min(max),
                    }
                };
                let selection = view
                    .selection_state
                    .selections()
                    .first()
                    .copied()
                    .unwrap_or_default();
                editor.set_text_selection(accesskit::TextSelection {
                    anchor: position(selection.anchor),
                    focus: position(selection.head),
                });
                nodes.push((editor_id, editor));
                for (line, id) in runs.into_iter().enumerate() {
                    let text = buffer.line(line);
                    let mut lengths: Vec<u8> = line_content(&text)
                        .chars()
                        .map(|ch| ch.len_utf8() as u8)
                        .collect();
                    if text.ends_with("\r\n") {
                        lengths.push(2);
                    } else if text.ends_with('\n') {
                        lengths.push(1);
                    }
                    let mut run = Node::new(Role::TextRun);
                    run.set_value(text);
                    run.set_character_lengths(lengths);
                    nodes.push((id, run));
                }
            }
        }
        let mut terminal = Node::new(Role::Button);
        terminal.set_label(self.t("accessibility.toggle_terminal"));
        terminal.add_action(Action::Click);
        nodes.push((NodeId(3), terminal));
        let mut selector = Node::new(Role::Button);
        selector.set_label(self.t("command.graphite.workbench.toggleMode"));
        selector.add_action(Action::Click);
        nodes.push((MODE_SELECTOR_NODE, selector));
        if self.ai_mode.is_some() {
            let mut chat = Node::new(Role::TextInput);
            chat.set_label(self.t("workbench.mode.chat"));
            chat.set_value(self.chat.input.as_str());
            chat.add_action(Action::Focus);
            nodes.push((AI_CHAT_NODE, chat));
            if self.ai_file_group().is_some() {
                let mut close = Node::new(Role::Button);
                close.set_label(self.t("workbench.mode.close_file"));
                close.add_action(Action::Click);
                nodes.push((AI_CLOSE_FILE_NODE, close));
            }
        } else {
            let mut problems = Node::new(Role::Button);
            problems.set_label(self.t("accessibility.show_diagnostics"));
            problems.add_action(Action::Click);
            nodes.push((NodeId(4), problems));
        }
        if let Some(panel) = panel {
            let mut node = Node::new(Role::Label);
            node.set_label(panel);
            nodes.push((NodeId(5), node));
        }
        TreeUpdate {
            nodes,
            tree: Some(Tree::new(root)),
            tree_id: accesskit::TreeId::ROOT,
            focus: if self.command_center.is_some() {
                command_center_search_node()
            } else if self.go_picker.is_some() {
                go_picker_query_node()
            } else if self.help_modal.is_some() {
                help_modal_node()
            } else if self.run_breakpoint_prompt.is_some() {
                run_breakpoint_prompt_input_node()
            } else if panel_focus {
                NodeId(5)
            } else if self.ai_mode.is_some()
                && (self.chat.focused || self.ai_file_group().is_none())
            {
                AI_CHAT_NODE
            } else {
                editor_node(self.editor_groups.focused())
            },
        }
    }
}
