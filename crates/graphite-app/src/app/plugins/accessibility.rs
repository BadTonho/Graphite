use super::*;
impl App {
    pub(crate) fn plugin_accessibility(&self) -> TreeUpdate {
        let root = NodeId(0);
        let content = NodeId(0x6000000000000000);
        let mut window = Node::new(Role::Window);
        window.set_label(self.t("plugins.window_title"));
        window.set_children(vec![content]);
        let mut panel = Node::new(Role::Label);
        let selected = if self.plugins.marketplace {
            self.plugins
                .open_vsx
                .get(self.plugins.index)
                .map(|extension| {
                    self.tf(
                        "plugins.accessibility.open_vsx",
                        &[
                            ("name", &extension.display_name),
                            ("version", &extension.version),
                        ],
                    )
                })
        } else {
            self.plugins.installed.get(self.plugins.index).map(|r| {
                self.tf(
                    "plugins.accessibility.installed",
                    &[
                        ("id", &r.metadata.id()),
                        ("version", &r.metadata.version()),
                        (
                            "state",
                            &self.t(if r.enabled {
                                "plugins.accessibility.enabled"
                            } else {
                                "plugins.accessibility.disabled"
                            }),
                        ),
                    ],
                )
            })
        };
        let prompt = if let Some((_, m)) = &self.plugins.preview {
            self.tf(
                "plugins.accessibility.install_prompt",
                &[("id", &m.id()), ("version", &m.version())],
            )
        } else if let Some(m) = &self.plugins.inspecting {
            self.tf(
                "plugins.accessibility.details",
                &[("details", &serde_json::to_string(m).unwrap_or_default())],
            )
        } else if let Some(id) = &self.plugins.trust {
            self.tf("plugins.accessibility.trust", &[("id", id)])
        } else if self.plugins.editing {
            self.tf(
                "plugins.accessibility.input",
                &[
                    (
                        "label",
                        &self.t(if self.plugins.path_input {
                            "plugins.package_path"
                        } else {
                            "plugins.open_vsx_search"
                        }),
                    ),
                    ("query", &self.plugins.query),
                ],
            )
        } else {
            selected.unwrap_or_else(|| self.t("plugins.none").into())
        };
        panel.set_label(self.tf(
            "plugins.accessibility.panel",
            &[("prompt", &prompt), ("message", &self.plugins.message)],
        ));
        TreeUpdate {
            nodes: vec![(root, window), (content, panel)],
            tree: Some(Tree::new(root)),
            tree_id: accesskit::TreeId::ROOT,
            focus: content,
        }
    }
}
