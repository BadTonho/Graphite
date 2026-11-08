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
        text(
            r,
            0,
            "PLUGINS    [I] Installed    [G] GitHub    [Esc] Close",
        );
        text(
            r,
            2,
            if self.plugins.github {
                "[/] Search    [P] Previous    [N] Next    [Enter] Release / Download"
            } else {
                "[L] Local file [E] Enable    [D] Disable   [R] Restart   [X] Remove    [C] Run first command"
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
                    format!(
                        "Install {} v{}? [Y] Confirm  [Esc] Cancel",
                        metadata.id(),
                        metadata.version()
                    )
                } else {
                    format!(
                        "{} v{} — Details [Esc] Back",
                        metadata.id(),
                        metadata.version()
                    )
                },
            );
            let lines = match metadata {
                Metadata::Native(m) => vec![
                    format!("Description: {}", m.description),
                    format!("API: {} | Entry: {}", m.api_version, m.main),
                    format!(
                        "Commands: {}",
                        m.commands
                            .iter()
                            .map(|c| c.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ],
                Metadata::Vsix(m) => vec![
                    format!("Description: {}", m.description),
                    format!("Engines: {}", m.engines),
                    format!("Entry: {}", m.main.as_deref().unwrap_or("none")),
                    format!("Contributions: {}", m.contributions.join(", ")),
                ],
            };
            for (i, line) in lines.iter().enumerate() {
                text(r, 6 + i * 2, line);
            }
            text(
                r,
                15,
                "Native plugins install disabled. VSIX packages are inspected only.",
            );
            return;
        }
        if let Some(id) = &self.plugins.trust {
            text(
                r,
                4,
                &format!("Trust and enable {id}? [Y] Enable  [Esc] Cancel"),
            );
            text(r, 6, "This plugin runs local code with your account's access. A separate process is not a security sandbox.");
            return;
        }
        if self.plugins.editing {
            text(
                r,
                4,
                &format!(
                    "{}: {}_",
                    if self.plugins.path_input {
                        "Package path (Enter to inspect)"
                    } else {
                        "GitHub search (Enter to search)"
                    },
                    self.plugins.query
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
                            format!("{} bytes", a.size),
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
                            format!("{}   Stars: {}", p.full_name, p.stargazers_count),
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
                        Metadata::Vsix(_) => "Imported — VS Code execution unavailable",
                        Metadata::Native(_) if !p.enabled => "Disabled",
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
                                "Running"
                            }
                            Some(graphite_extension_host::HostState::Ready) => "Activating",
                            Some(graphite_extension_host::HostState::Starting) => "Starting",
                            _ => "Stopped — restart to retry",
                        },
                    };
                    let details = match &p.metadata {
                        Metadata::Native(m) => m.description.clone(),
                        Metadata::Vsix(m) => format!(
                            "{} | engines: {} | entry: {:?} | contributions: {}",
                            m.description,
                            m.engines,
                            m.main,
                            m.contributions.join(", ")
                        ),
                    };
                    (
                        format!("{} v{} — {status}", p.metadata.id(), p.metadata.version()),
                        details,
                        self.plugins
                            .errors
                            .get(p.metadata.id())
                            .cloned()
                            .unwrap_or_else(|| match &p.metadata {
                                Metadata::Native(m) => format!(
                                    "Commands: {}",
                                    m.commands
                                        .iter()
                                        .map(|c| c.id.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                                _ => {
                                    "VSIX prototype: no code or contributions are activated".into()
                                }
                            }),
                    )
                })
                .collect()
        };
        if rows.is_empty() {
            text(
                r,
                5,
                "No plugins found. Use G to browse GitHub or L to install a local package.",
            );
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
