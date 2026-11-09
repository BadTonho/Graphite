use super::*;
impl App {
    pub(crate) fn draw_plugins(&self, r: &mut Renderer, w: f32, h: f32, lh: f32, cw: f32) {
        if !self.plugins.visible {
            return;
        }
        r.set_clip(None);
        r.draw_rect(0.0, 0.0, w, h - lh, rgb(Theme::color(Theme::BG, 0xffffff)));
        let max = ((w - PAD * 2.0) / cw).max(1.0) as usize;
        let text = |r: &mut Renderer, line: usize, s: &str| {
            let s: String = s
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(max)
                .collect();
            r.draw_text(
                &s,
                PAD,
                lh * line as f32,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        };
        text(r, 0, self.t("plugins.header"));
        text(
            r,
            2,
            if self.plugins.github {
                self.t("plugins.github_help")
            } else {
                self.t("plugins.local_help")
            },
        );
        if let Some(metadata) = self
            .plugins
            .preview
            .as_ref()
            .map(|(_, m)| m)
            .or(self.plugins.inspecting.as_ref())
        {
            text(
                r,
                4,
                &if self.plugins.preview.is_some() {
                    self.tf(
                        "plugins.install_confirm",
                        &[("id", &metadata.id()), ("version", &metadata.version())],
                    )
                } else {
                    self.tf(
                        "plugins.details_title",
                        &[("id", &metadata.id()), ("version", &metadata.version())],
                    )
                },
            );
            let lines = match metadata {
                Metadata::Native(m) => vec![
                    self.tf("plugins.description", &[("description", &m.description)]),
                    self.tf(
                        "plugins.api_entry",
                        &[("api", &m.api_version), ("entry", &m.main)],
                    ),
                    self.tf(
                        "plugins.commands",
                        &[(
                            "commands",
                            &m.commands
                                .iter()
                                .map(|c| c.id.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                        )],
                    ),
                ],
                Metadata::Vsix(m) => vec![
                    self.tf("plugins.description", &[("description", &m.description)]),
                    self.tf("plugins.engines", &[("engines", &m.engines)]),
                    self.tf(
                        "plugins.entry",
                        &[("entry", &m.main.as_deref().unwrap_or(self.t("common.none")))],
                    ),
                    self.tf(
                        "plugins.contributions",
                        &[("contributions", &m.contributions.join(", "))],
                    ),
                ],
            };
            for (i, line) in lines.iter().enumerate() {
                text(r, 6 + i * 2, line);
            }
            text(r, 15, self.t("plugins.native_disabled"));
            return;
        }
        if let Some(id) = &self.plugins.trust {
            text(r, 4, &self.tf("plugins.trust_confirm", &[("id", id)]));
            text(r, 6, self.t("plugins.trust_warning"));
            return;
        }
        if self.plugins.editing {
            text(
                r,
                4,
                &self.tf(
                    "plugins.input_prompt",
                    &[
                        (
                            "label",
                            &self.t(if self.plugins.path_input {
                                "plugins.package_path"
                            } else {
                                "plugins.github_search"
                            }),
                        ),
                        ("query", &self.plugins.query),
                    ],
                ),
            );
            return;
        }
        text(r, 3, &self.plugins.message);
        let rows: Vec<(String, String, String)> = if self.plugins.github {
            if let Some(assets) = &self.plugins.assets {
                assets
                    .iter()
                    .map(|a| {
                        (
                            a.name.clone(),
                            self.tf("plugins.bytes", &[("bytes", &a.size)]),
                            a.browser_download_url.clone(),
                        )
                    })
                    .collect()
            } else {
                self.plugins
                    .repositories
                    .iter()
                    .map(|p| {
                        (
                            self.tf(
                                "plugins.repository",
                                &[("name", &p.full_name), ("stars", &p.stargazers_count)],
                            ),
                            p.description.clone().unwrap_or_default(),
                            p.html_url.clone(),
                        )
                    })
                    .collect()
            }
        } else {
            self.plugins
                .installed
                .iter()
                .map(|p| {
                    let status = match &p.metadata {
                        Metadata::Vsix(_) => self.t("plugins.status.vsix_imported"),
                        Metadata::Native(_) if !p.enabled => self.t("plugins.status.disabled"),
                        Metadata::Native(_) => match self
                            .plugins
                            .hosts
                            .get(p.metadata.id())
                            .and_then(|h| h.supervisor.as_ref())
                            .map(|s| s.state)
                        {
                            Some(graphite_extension_host::HostState::Ready)
                                if self
                                    .plugins
                                    .hosts
                                    .get(p.metadata.id())
                                    .is_some_and(|h| h.native_active) =>
                            {
                                self.t("plugins.status.running")
                            }
                            Some(graphite_extension_host::HostState::Ready) => {
                                self.t("plugins.status.activating")
                            }
                            Some(graphite_extension_host::HostState::Starting) => {
                                self.t("plugins.status.starting")
                            }
                            _ => self.t("plugins.status.stopped"),
                        },
                    };
                    let details = match &p.metadata {
                        Metadata::Native(m) => m.description.clone(),
                        Metadata::Vsix(m) => self.tf(
                            "plugins.details_vsix",
                            &[
                                ("description", &m.description),
                                ("engines", &m.engines),
                                ("entry", &format!("{:?}", m.main)),
                                ("contributions", &m.contributions.join(", ")),
                            ],
                        ),
                    };
                    (
                        self.tf(
                            "plugins.row",
                            &[
                                ("id", &p.metadata.id()),
                                ("version", &p.metadata.version()),
                                ("status", &status),
                            ],
                        ),
                        details,
                        self.plugins
                            .errors
                            .get(p.metadata.id())
                            .cloned()
                            .unwrap_or_else(|| match &p.metadata {
                                Metadata::Native(m) => self.tf(
                                    "plugins.commands",
                                    &[(
                                        "commands",
                                        &m.commands
                                            .iter()
                                            .map(|c| c.id.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                    )],
                                ),
                                _ => self.t("plugins.vsix_prototype").into(),
                            }),
                    )
                })
                .collect()
        };
        if rows.is_empty() {
            text(r, 5, self.t("plugins.empty"));
        }
        let offset = self.plugins.index.saturating_sub(5);
        for (i, (title, detail, extra)) in rows.iter().enumerate().skip(offset) {
            let row = 5 + (i - offset) * 3;
            if (row + 3) as f32 * lh > h - lh {
                break;
            }
            if i == self.plugins.index {
                r.draw_rect(
                    0.0,
                    row as f32 * lh,
                    w,
                    lh * 3.0,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            text(r, row, title);
            text(r, row + 1, detail);
            text(r, row + 2, extra);
        }
    }
}
