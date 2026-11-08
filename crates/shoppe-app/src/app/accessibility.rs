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

fn node_id(kind: u64, group: u64, value: u64) -> NodeId {
    NodeId((kind << KIND_SHIFT) | ((group & GROUP_MASK) << GROUP_SHIFT) | (value & VALUE_MASK))
}
pub(super) fn group_node(group: u64) -> NodeId {
    node_id(GROUP, group, 0)
}
pub(super) fn editor_node(group: u64) -> NodeId {
    node_id(EDITOR, group, 0)
}
pub(super) fn tab_node(group: u64, document_id: u64) -> NodeId {
    node_id(TAB, group, document_id)
}
pub(super) fn line_node(group: u64, line: usize) -> NodeId {
    node_id(LINE, group, line as u64)
}
pub(super) fn node_target(id: NodeId) -> (u64, u64, u64) {
    (
        id.0 >> KIND_SHIFT,
        (id.0 >> GROUP_SHIFT) & GROUP_MASK,
        id.0 & VALUE_MASK,
    )
}
pub(super) fn editor_target(id: NodeId) -> Option<u64> {
    let (kind, group, _) = node_target(id);
    (kind == EDITOR || kind == GROUP).then_some(group)
}
pub(super) fn tab_target(id: NodeId) -> Option<(u64, u64)> {
    let (kind, group, document) = node_target(id);
    (kind == TAB).then_some((group, document))
}
pub(super) fn line_target(id: NodeId) -> Option<(u64, usize)> {
    let (kind, group, line) = node_target(id);
    (kind == LINE).then_some((group, line as usize))
}
pub(super) fn line_content(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

pub(crate) struct AccessibilityActivation;
impl ActivationHandler for AccessibilityActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(accessibility_tree(&[], "VsCodeShoppe editor", None, false))
    }
}
#[derive(Debug)]
pub(crate) enum UserEvent {
    AccessibilityWake,
    LspWake,
    TerminalWake,
    ScmRefresh,
    ScmSnapshot(crate::scm::Snapshot),
    ScmOperationCompleted {
        message: String,
        snapshot: Option<crate::scm::Snapshot>,
    },
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
    root_node.set_label("VsCodeShoppe");
    let mut tabs_node = Node::new(Role::TabList);
    tabs_node.set_children(tab_ids.clone());
    let mut nodes = vec![(root, root_node), (tablist, tabs_node)];
    let mut terminal_button = Node::new(Role::Button);
    terminal_button.set_label("Toggle integrated terminal");
    nodes.push((NodeId(3), terminal_button));
    let mut problems_button = Node::new(Role::Button);
    problems_button.set_label("Show diagnostics");
    nodes.push((NodeId(4), problems_button));
    for (id, title) in tab_ids.iter().zip(titles) {
        let mut tab = Node::new(Role::Tab);
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

impl App {
    /// Build one accessible editor and tab list for each visible group.
    pub(super) fn grouped_accessibility_tree(
        &mut self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> TreeUpdate {
        self.snapshot_editor_view();
        let rects = self.group_rectangles(width, height, line_height);
        let panel = self.accessible_panel_text();
        let panel_focus = panel.is_some() && self.terminal_focused;
        let root = NodeId(0);
        let mut root_node = Node::new(Role::Window);
        root_node.set_label("VsCodeShoppe");
        let mut children: Vec<_> = rects.iter().map(|rect| group_node(rect.id)).collect();
        children.extend([NodeId(3), NodeId(4)]);
        if panel.is_some() {
            children.push(NodeId(5));
        }
        root_node.set_children(children);
        let mut nodes = vec![(root, root_node)];
        for (index, rect) in rects.iter().enumerate() {
            let (views, active) = self
                .editor_groups
                .views(rect.id)
                .expect("layout group exists");
            let view = &views[active];
            let buffer = &self.tabs[view.document].buffer;
            let editor_id = editor_node(rect.id);
            let tab_list_id = node_id(TAB_LIST, rect.id, 0);
            let mut group = Node::new(Role::Group);
            group.set_label(format!("Editor group {}", index + 1));
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
                .map(|view| tab_node(rect.id, self.tabs[view.document].id))
                .collect();
            tab_list.set_children(ids.clone());
            nodes.push((tab_list_id, tab_list));
            for (i, item) in views.iter().enumerate() {
                let tab = &self.tabs[item.document];
                let mut node = Node::new(Role::Tab);
                node.set_label(super::group_drawing::tab_label(tab).trim());
                node.set_selected(i == active);
                node.add_action(Action::Click);
                node.add_action(Action::Focus);
                nodes.push((ids[i], node));
            }
            let mut editor = Node::new(Role::TextInput);
            editor.set_label("Editor");
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
            let selection = view.selections.first().copied().unwrap_or_default();
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
        let mut terminal = Node::new(Role::Button);
        terminal.set_label("Toggle integrated terminal");
        terminal.add_action(Action::Click);
        nodes.push((NodeId(3), terminal));
        let mut problems = Node::new(Role::Button);
        problems.set_label("Show diagnostics");
        problems.add_action(Action::Click);
        nodes.push((NodeId(4), problems));
        if let Some(panel) = panel {
            let mut node = Node::new(Role::Label);
            node.set_label(panel);
            nodes.push((NodeId(5), node));
        }
        TreeUpdate {
            nodes,
            tree: Some(Tree::new(root)),
            tree_id: accesskit::TreeId::ROOT,
            focus: if panel_focus {
                NodeId(5)
            } else {
                editor_node(self.editor_groups.focused())
            },
        }
    }
}
