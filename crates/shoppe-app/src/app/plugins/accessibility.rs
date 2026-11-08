use super::*;
impl App {
    pub(crate) fn plugin_accessibility(&self) -> TreeUpdate {
        let root = NodeId(0);
        let content = NodeId(0x6000000000000000);
        let mut window = Node::new(Role::Window);
        window.set_label("Graphite — Plugins");
        window.set_children(vec![content]);
        let mut panel = Node::new(Role::Label);
        let selected = if self.plugins.github {
            if let Some(assets) = &self.plugins.assets {
                assets.get(self.plugins.index).map(|a| a.name.clone())
            } else {
                self.plugins.repositories.get(self.plugins.index).map(|r| {
                    format!(
                        "{}: {}",
                        r.full_name,
                        r.description.as_deref().unwrap_or("")
                    )
                })
            }
        } else {
            self.plugins.installed.get(self.plugins.index).map(|r| {
                format!(
                    "{} v{}, {}",
                    r.metadata.id(),
                    r.metadata.version(),
                    if r.enabled { "enabled" } else { "disabled" }
                )
            })
        };
        let prompt = if let Some((_, m)) = &self.plugins.preview {
            format!(
                "Install {} v{}? Y confirms, Escape cancels",
                m.id(),
                m.version()
            )
        } else if let Some(m) = &self.plugins.inspecting {
            format!(
                "Details: {}. Escape returns",
                serde_json::to_string(m).unwrap_or_default()
            )
        } else if let Some(id) = &self.plugins.trust {
            format!("Trust {id} to run local code? Y confirms, Escape cancels")
        } else if self.plugins.editing {
            format!(
                "{}: {}",
                if self.plugins.path_input {
                    "Package path"
                } else {
                    "Search"
                },
                self.plugins.query
            )
        } else {
            selected.unwrap_or_else(|| "No plugins found".into())
        };
        panel.set_label(format!("Plugins. I installed, G GitHub, L install local file, slash search, arrows select, Enter release assets, E enable, D disable, R restart, X remove, C run first command, Escape back. {prompt}. {}", self.plugins.message));
        TreeUpdate {
            nodes: vec![(root, window), (content, panel)],
            tree: Some(Tree::new(root)),
            tree_id: accesskit::TreeId::ROOT,
            focus: content,
        }
    }
}
