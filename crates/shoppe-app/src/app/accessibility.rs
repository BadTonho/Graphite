use super::*;

pub(crate) struct AccessibilityActivation;
impl ActivationHandler for AccessibilityActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(accessibility_tree(&[], "Graphite editor", None))
    }
}
#[derive(Debug)]
pub(crate) enum UserEvent {
    AccessibilityWake,
    LspWake,
    TerminalWake,
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
    root_node.set_label("Graphite");
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
