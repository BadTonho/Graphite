use super::*;
use winit::keyboard::{Key, NamedKey};

const ROW_HEIGHT: f32 = 28.0;
const MENU_MIN_WIDTH: f32 = 280.0;
const MENU_LEFT_PADDING: f32 = 30.0;
const MENU_RIGHT_PADDING: f32 = 14.0;
const LABEL_SHORTCUT_GAP: f32 = 14.0;
const MODAL_WIDTH: f32 = 720.0;
const MODAL_HEIGHT: f32 = 520.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelpCommand {
    Welcome,
    ShowAllCommands,
    EditorPlayground,
    OpenWalkthrough,
    ShowReleaseNotes,
    KeyboardShortcutsReference,
    ToggleDeveloperTools,
    OpenProcessExplorer,
    RestartToUpdate,
    About,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelpRow {
    Separator,
    Item {
        command: HelpCommand,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelpPage {
    Welcome,
    Playground,
    Walkthrough,
    ReleaseNotes,
    KeyboardShortcuts,
    DeveloperTools,
    About,
}

#[derive(Clone, Debug)]
pub(crate) struct HelpModal {
    pub page: HelpPage,
    pub selected: usize,
    pub scroll: usize,
    pub query: String,
    pub walkthrough_step: usize,
}

impl App {
    pub(crate) fn help_menu_rows(&self) -> Vec<HelpRow> {
        use HelpCommand as C;
        use HelpRow::{Item as I, Separator as S};
        vec![
            I {
                command: C::Welcome,
                label: "help.welcome",
                shortcut: "",
                enabled: true,
            },
            I {
                command: C::ShowAllCommands,
                label: "help.show_all_commands",
                shortcut: "Ctrl+Shift+P",
                enabled: true,
            },
            I {
                command: C::EditorPlayground,
                label: "help.editor_playground",
                shortcut: "",
                enabled: true,
            },
            I {
                command: C::OpenWalkthrough,
                label: "help.open_walkthrough",
                shortcut: "",
                enabled: true,
            },
            I {
                command: C::ShowReleaseNotes,
                label: "help.release_notes",
                shortcut: "",
                enabled: true,
            },
            S,
            I {
                command: C::KeyboardShortcutsReference,
                label: "help.keyboard_shortcuts",
                shortcut: "Ctrl+K Ctrl+R",
                enabled: true,
            },
            S,
            I {
                command: C::ToggleDeveloperTools,
                label: "help.developer_tools",
                shortcut: "",
                enabled: true,
            },
            I {
                command: C::OpenProcessExplorer,
                label: "help.process_explorer",
                shortcut: "",
                enabled: true,
            },
            S,
            I {
                command: C::RestartToUpdate,
                label: "help.up_to_date",
                shortcut: "",
                enabled: false,
            },
            S,
            I {
                command: C::About,
                label: "help.about",
                shortcut: "",
                enabled: true,
            },
        ]
    }

    pub(crate) fn help_menu_width(&self, char_width: f32, available_width: f32) -> f32 {
        let rows = self.help_menu_rows();
        let required = rows
            .iter()
            .filter_map(|row| match row {
                HelpRow::Item {
                    label, shortcut, ..
                } => {
                    let label_width = self.t(label).chars().count() as f32 * char_width;
                    let shortcut_width = shortcut.chars().count() as f32 * char_width;
                    Some(
                        MENU_LEFT_PADDING
                            + label_width
                            + if shortcut.is_empty() {
                                MENU_RIGHT_PADDING
                            } else {
                                LABEL_SHORTCUT_GAP + shortcut_width + MENU_RIGHT_PADDING
                            },
                    )
                }
                HelpRow::Separator => None,
            })
            .fold(MENU_MIN_WIDTH, f32::max);
        required.min(available_width.max(0.0))
    }

    pub(crate) fn help_menu_visible_label(
        label: &str,
        char_width: f32,
        available_width: f32,
    ) -> String {
        let max_chars = (available_width.max(0.0) / char_width.max(1.0)).floor() as usize;
        let chars: Vec<_> = label.chars().collect();
        if chars.len() <= max_chars {
            return label.to_owned();
        }
        if max_chars == 0 {
            return String::new();
        }
        chars
            .into_iter()
            .take(max_chars.saturating_sub(1))
            .chain(std::iter::once('…'))
            .collect()
    }

    pub(crate) fn help_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        popup_y: f32,
        popup_width: f32,
    ) -> Option<(HelpCommand, bool)> {
        if x < popup_x || x >= popup_x + popup_width || y < popup_y + 4.0 {
            return None;
        }
        let mut top = popup_y + 4.0;
        for row in self.help_menu_rows() {
            match row {
                HelpRow::Separator => top += 9.0,
                HelpRow::Item {
                    command, enabled, ..
                } => {
                    if y >= top && y < top + ROW_HEIGHT {
                        return Some((command, enabled));
                    }
                    top += ROW_HEIGHT;
                }
            }
        }
        None
    }

    pub(crate) fn activate_help_menu_accessibility_item(&mut self, index: usize) {
        if let Some((command, true)) = self
            .help_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                HelpRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                _ => None,
            })
            .nth(index)
        {
            self.execute_help_command(command);
        }
    }

    pub(crate) fn execute_help_command(&mut self, command: HelpCommand) {
        use HelpCommand as C;
        match command {
            C::Welcome => self.open_help_modal(HelpPage::Welcome),
            C::ShowAllCommands => self.open_command_palette(),
            C::EditorPlayground => self.open_help_modal(HelpPage::Playground),
            C::OpenWalkthrough => self.open_help_modal(HelpPage::Walkthrough),
            C::ShowReleaseNotes => self.open_help_modal(HelpPage::ReleaseNotes),
            C::KeyboardShortcutsReference => self.open_help_modal(HelpPage::KeyboardShortcuts),
            C::ToggleDeveloperTools => {
                if self
                    .help_modal
                    .as_ref()
                    .is_some_and(|modal| modal.page == HelpPage::DeveloperTools)
                {
                    self.close_help_modal();
                } else {
                    self.open_help_modal(HelpPage::DeveloperTools);
                }
            }
            C::OpenProcessExplorer => self.toggle_process_explorer(),
            C::RestartToUpdate => {}
            C::About => self.open_help_modal(HelpPage::About),
        }
    }

    pub(crate) fn open_help_modal(&mut self, page: HelpPage) {
        self.close_workbench_menu();
        self.help_modal = Some(HelpModal {
            page,
            selected: 0,
            scroll: 0,
            query: String::new(),
            walkthrough_step: 0,
        });
    }

    pub(crate) fn close_help_modal(&mut self) {
        self.help_modal = None;
    }

    fn load_playground_sample(&mut self, javascript: bool) {
        let sample = if javascript {
            "// Welcome to the Graphite editor\nconst greeting = \"Hello, Graphite!\";\nconsole.log(greeting);\n"
        } else {
            "fn main() {\n    println!(\"Hello, Graphite!\");\n}\n"
        };
        self.new_untitled_document();
        self.editor_mut().insert(sample);
    }

    pub(crate) fn activate_help_modal_accessibility_action(&mut self, index: usize) {
        let Some(modal) = self.help_modal.clone() else {
            return;
        };
        if index == 0 {
            self.close_help_modal();
            return;
        }
        match modal.page {
            HelpPage::Welcome => match index {
                1 => {
                    self.new_untitled_document();
                    self.close_help_modal();
                }
                2 => {
                    self.open_file_dialog();
                    self.close_help_modal();
                }
                3 => {
                    self.open_folder_dialog();
                    self.close_help_modal();
                }
                recent => {
                    let recent = recent - 4;
                    if recent < self.recent_workspaces.len() {
                        self.open_recent_workspace(recent);
                        self.close_help_modal();
                    }
                }
            },
            HelpPage::Playground if index == 1 || index == 2 => {
                self.load_playground_sample(index == 2);
                self.close_help_modal();
            }
            HelpPage::Walkthrough if index == 1 => {
                if let Some(state) = self.help_modal.as_mut() {
                    state.walkthrough_step = state.walkthrough_step.saturating_sub(1);
                }
            }
            HelpPage::Walkthrough if index == 2 => {
                if modal.walkthrough_step >= 3 {
                    self.close_help_modal();
                } else if let Some(state) = self.help_modal.as_mut() {
                    state.walkthrough_step += 1;
                }
            }
            _ => {}
        }
    }

    pub(crate) fn help_modal_key(&mut self, key: &Key, text: Option<&str>, ctrl: bool) -> bool {
        let Some(modal) = self.help_modal.as_mut() else {
            return false;
        };
        if matches!(key, Key::Named(NamedKey::Escape)) {
            self.help_modal = None;
            return true;
        }
        match modal.page {
            HelpPage::KeyboardShortcuts => match key {
                Key::Named(NamedKey::Backspace) => {
                    modal.query.pop();
                    modal.selected = 0;
                    modal.scroll = 0;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    modal.selected = modal.selected.saturating_add(1);
                    if modal.selected >= modal.scroll + 11 {
                        modal.scroll = modal.selected - 10;
                    }
                }
                Key::Named(NamedKey::ArrowUp) => {
                    modal.selected = modal.selected.saturating_sub(1);
                    if modal.selected < modal.scroll {
                        modal.scroll = modal.selected;
                    }
                }
                _ if !ctrl => {
                    if let Some(text) = text {
                        if !text.chars().any(char::is_control) {
                            modal.query.push_str(text);
                            modal.selected = 0;
                            modal.scroll = 0;
                        }
                    }
                }
                _ => return false,
            },
            HelpPage::Walkthrough => match key {
                Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::Enter) => {
                    modal.walkthrough_step = (modal.walkthrough_step + 1).min(3)
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    modal.walkthrough_step = modal.walkthrough_step.saturating_sub(1)
                }
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    pub(crate) fn help_modal_click(&mut self, x: f32, y: f32) -> bool {
        let Some(modal) = self.help_modal.clone() else {
            return false;
        };
        let (width, height) = self.gpu.as_ref().map_or((1200.0, 760.0), |g| {
            let (w, h) = g.renderer.size();
            (w, h)
        });
        let rect = help_modal_rect(width, height);
        if x < rect.0 || x >= rect.0 + rect.2 || y < rect.1 || y >= rect.1 + rect.3 {
            self.help_modal = None;
            return true;
        }
        let local_y = y - rect.1;
        if local_y < 54.0 && x >= rect.0 + rect.2 - 126.0 {
            self.help_modal = None;
            return true;
        }
        match modal.page {
            HelpPage::Welcome
                if (128.0..168.0).contains(&local_y)
                    && x >= rect.0 + 28.0
                    && x < rect.0 + rect.2 - 28.0 =>
            {
                let slot = ((x - rect.0 - 28.0) / ((rect.2 - 56.0) / 3.0)).floor() as usize;
                match slot {
                    0 => self.new_untitled_document(),
                    1 => self.open_file_dialog(),
                    _ => self.open_folder_dialog(),
                }
                self.help_modal = None;
            }
            HelpPage::Welcome if local_y >= 214.0 => {
                let index = ((local_y - 214.0) / ROW_HEIGHT) as usize;
                if index < self.recent_workspaces.len() {
                    self.open_recent_workspace(index);
                    self.help_modal = None;
                }
            }
            HelpPage::Playground if (134.0..198.0).contains(&local_y) => {
                self.load_playground_sample(x >= rect.0 + rect.2 * 0.5);
                self.help_modal = None;
            }
            HelpPage::Walkthrough
                if local_y >= rect.3 - 62.0
                    && x >= rect.0 + rect.2 - 340.0
                    && x < rect.0 + rect.2 - 30.0 =>
            {
                if x < rect.0 + rect.2 - 200.0 {
                    if let Some(m) = self.help_modal.as_mut() {
                        m.walkthrough_step = m.walkthrough_step.saturating_sub(1);
                    }
                } else if let Some(m) = self.help_modal.as_mut() {
                    if m.walkthrough_step >= 3 {
                        self.help_modal = None;
                    } else {
                        m.walkthrough_step += 1;
                    }
                }
            }
            _ => {}
        }
        true
    }

    pub(crate) fn draw_help_modal(&self, renderer: &mut Renderer) {
        let Some(modal) = &self.help_modal else {
            return;
        };
        let (width, height) = renderer.size();
        let rect = help_modal_rect(width, height);
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let accent = rgb(Theme::color(Theme::STATUS, 0x007acc));
        renderer.draw_rect(0.0, 0.0, width, height, rgb(0x111111));
        renderer.draw_rect(rect.0, rect.1, rect.2, rect.3, border);
        renderer.draw_rect(rect.0 + 1.0, rect.1 + 1.0, rect.2 - 2.0, rect.3 - 2.0, bg);
        let title = self.help_page_title(modal.page);
        renderer.draw_text(&title, rect.0 + 28.0, rect.1 + 24.0, text);
        renderer.draw_text(
            self.t("help.close_button"),
            rect.0 + rect.2 - 96.0,
            rect.1 + 24.0,
            dim,
        );
        match modal.page {
            HelpPage::Welcome => {
                renderer.draw_text(
                    self.t("help.welcome_title"),
                    rect.0 + 28.0,
                    rect.1 + 68.0,
                    text,
                );
                renderer.draw_text(
                    self.t("help.welcome_body"),
                    rect.0 + 28.0,
                    rect.1 + 98.0,
                    dim,
                );
                for (i, key) in ["help.new_file", "help.open_file", "help.open_folder"]
                    .iter()
                    .enumerate()
                {
                    let slot_width = (rect.2 - 56.0) / 3.0;
                    let x = rect.0 + 28.0 + i as f32 * slot_width;
                    renderer.draw_rect(x, rect.1 + 128.0, slot_width - 8.0, 40.0, accent);
                    renderer.draw_text(self.t(key), x + 12.0, rect.1 + 139.0, text);
                }
                renderer.draw_text(
                    self.t("help.recent_workspaces"),
                    rect.0 + 28.0,
                    rect.1 + 190.0,
                    text,
                );
                if self.recent_workspaces.is_empty() {
                    renderer.draw_text(
                        self.t("help.no_recent_workspaces"),
                        rect.0 + 28.0,
                        rect.1 + 220.0,
                        dim,
                    );
                } else {
                    for (i, path) in self.recent_workspaces.iter().take(8).enumerate() {
                        renderer.draw_text(
                            &path.display().to_string(),
                            rect.0 + 36.0,
                            rect.1 + 214.0 + i as f32 * ROW_HEIGHT,
                            text,
                        );
                    }
                }
            }
            HelpPage::Playground => {
                renderer.draw_text(
                    self.t("help.playground_body"),
                    rect.0 + 28.0,
                    rect.1 + 76.0,
                    dim,
                );
                for (i, key) in ["help.playground_rust", "help.playground_javascript"]
                    .iter()
                    .enumerate()
                {
                    let x = rect.0 + 28.0 + i as f32 * (rect.2 * 0.5);
                    renderer.draw_rect(x, rect.1 + 132.0, rect.2 * 0.5 - 40.0, 52.0, accent);
                    renderer.draw_text(self.t(key), x + 12.0, rect.1 + 150.0, text);
                }
                renderer.draw_text(
                    self.t("help.playground_note"),
                    rect.0 + 28.0,
                    rect.1 + 214.0,
                    dim,
                );
            }
            HelpPage::Walkthrough => {
                let titles = [
                    "help.walkthrough.0.title",
                    "help.walkthrough.1.title",
                    "help.walkthrough.2.title",
                    "help.walkthrough.3.title",
                ];
                let bodies = [
                    "help.walkthrough.0.body",
                    "help.walkthrough.1.body",
                    "help.walkthrough.2.body",
                    "help.walkthrough.3.body",
                ];
                renderer.draw_text(
                    self.t(titles[modal.walkthrough_step]),
                    rect.0 + 28.0,
                    rect.1 + 90.0,
                    text,
                );
                renderer.draw_text(
                    self.t(bodies[modal.walkthrough_step]),
                    rect.0 + 28.0,
                    rect.1 + 128.0,
                    dim,
                );
                renderer.draw_text(
                    &format!("{} / 4", modal.walkthrough_step + 1),
                    rect.0 + 28.0,
                    rect.1 + rect.3 - 54.0,
                    dim,
                );
                renderer.draw_rect(
                    rect.0 + rect.2 - 340.0,
                    rect.1 + rect.3 - 62.0,
                    130.0,
                    36.0,
                    border,
                );
                renderer.draw_text(
                    self.t("help.back"),
                    rect.0 + rect.2 - 326.0,
                    rect.1 + rect.3 - 52.0,
                    text,
                );
                renderer.draw_rect(
                    rect.0 + rect.2 - 180.0,
                    rect.1 + rect.3 - 62.0,
                    150.0,
                    36.0,
                    accent,
                );
                renderer.draw_text(
                    if modal.walkthrough_step == 3 {
                        self.t("help.finish")
                    } else {
                        self.t("help.next")
                    },
                    rect.0 + rect.2 - 166.0,
                    rect.1 + rect.3 - 52.0,
                    text,
                );
            }
            HelpPage::ReleaseNotes => {
                renderer.draw_text(
                    &format!(
                        "{} {}",
                        self.t("help.release_version"),
                        env!("CARGO_PKG_VERSION")
                    ),
                    rect.0 + 28.0,
                    rect.1 + 72.0,
                    text,
                );
                for (i, key) in [
                    "help.release.0",
                    "help.release.1",
                    "help.release.2",
                    "help.release.3",
                ]
                .iter()
                .enumerate()
                {
                    renderer.draw_text(
                        self.t(key),
                        rect.0 + 36.0,
                        rect.1 + 112.0 + i as f32 * 36.0,
                        dim,
                    );
                }
            }
            HelpPage::KeyboardShortcuts => {
                renderer.draw_text(
                    self.t("help.shortcuts_search"),
                    rect.0 + 28.0,
                    rect.1 + 70.0,
                    text,
                );
                renderer.draw_rect(rect.0 + 28.0, rect.1 + 88.0, rect.2 - 56.0, 30.0, border);
                renderer.draw_text(&modal.query, rect.0 + 36.0, rect.1 + 95.0, text);
                let query = modal.query.to_lowercase();
                let entries = self
                    .shortcut_reference()
                    .into_iter()
                    .filter(|(name, shortcut)| {
                        query.is_empty()
                            || name.to_lowercase().contains(&query)
                            || shortcut.to_lowercase().contains(&query)
                    })
                    .collect::<Vec<_>>();
                for (i, (name, shortcut)) in entries.iter().skip(modal.scroll).take(11).enumerate()
                {
                    if i + modal.scroll == modal.selected {
                        renderer.draw_rect(
                            rect.0 + 28.0,
                            rect.1 + 126.0 + i as f32 * 27.0,
                            rect.2 - 56.0,
                            25.0,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46)),
                        );
                    }
                    renderer.draw_text(name, rect.0 + 36.0, rect.1 + 132.0 + i as f32 * 27.0, text);
                    renderer.draw_text(
                        shortcut,
                        rect.0 + rect.2 - 180.0,
                        rect.1 + 132.0 + i as f32 * 27.0,
                        dim,
                    );
                }
            }
            HelpPage::DeveloperTools => {
                let details = self.developer_diagnostics(width as u32, height as u32);
                for (i, line) in details.iter().enumerate() {
                    renderer.draw_text(
                        line,
                        rect.0 + 28.0,
                        rect.1 + 72.0 + i as f32 * 28.0,
                        if i == 0 { text } else { dim },
                    );
                }
            }
            HelpPage::About => {
                renderer.draw_text("Graphite", rect.0 + 28.0, rect.1 + 86.0, text);
                renderer.draw_text(
                    &format!("{} {}", self.t("help.version"), env!("CARGO_PKG_VERSION")),
                    rect.0 + 28.0,
                    rect.1 + 124.0,
                    dim,
                );
                renderer.draw_text(
                    self.t("help.license_mit"),
                    rect.0 + 28.0,
                    rect.1 + 160.0,
                    dim,
                );
            }
        }
    }

    pub(crate) fn help_page_title(&self, page: HelpPage) -> String {
        let key = match page {
            HelpPage::Welcome => "help.welcome",
            HelpPage::Playground => "help.editor_playground",
            HelpPage::Walkthrough => "help.open_walkthrough",
            HelpPage::ReleaseNotes => "help.release_notes",
            HelpPage::KeyboardShortcuts => "help.keyboard_shortcuts",
            HelpPage::DeveloperTools => "help.developer_tools",
            HelpPage::About => "help.about",
        };
        self.t(key).to_owned()
    }
    pub(crate) fn help_modal_accessible_label(&self, modal: &HelpModal) -> String {
        let mut label = format!(
            "{}. {}",
            self.help_page_title(modal.page),
            self.t("help.close_hint")
        );
        match modal.page {
            HelpPage::Welcome => label.push_str(&format!(". {}", self.t("help.welcome_body"))),
            HelpPage::Playground => {
                label.push_str(&format!(". {}", self.t("help.playground_body")))
            }
            HelpPage::Walkthrough => {
                let index = modal.walkthrough_step.min(3);
                label.push_str(&format!(
                    ". {}. {}",
                    self.t([
                        "help.walkthrough.0.title",
                        "help.walkthrough.1.title",
                        "help.walkthrough.2.title",
                        "help.walkthrough.3.title"
                    ][index]),
                    self.t([
                        "help.walkthrough.0.body",
                        "help.walkthrough.1.body",
                        "help.walkthrough.2.body",
                        "help.walkthrough.3.body"
                    ][index])
                ));
            }
            HelpPage::ReleaseNotes => {
                for key in [
                    "help.release.0",
                    "help.release.1",
                    "help.release.2",
                    "help.release.3",
                ] {
                    label.push_str(&format!(". {}", self.t(key)));
                }
            }
            HelpPage::KeyboardShortcuts => {
                label.push_str(&format!(
                    ". {} {}",
                    self.t("help.shortcuts_search"),
                    modal.query
                ));
                for (name, keys) in self
                    .shortcut_reference()
                    .into_iter()
                    .filter(|(name, keys)| {
                        modal.query.is_empty()
                            || name.to_lowercase().contains(&modal.query.to_lowercase())
                            || keys.to_lowercase().contains(&modal.query.to_lowercase())
                    })
                    .take(12)
                {
                    label.push_str(&format!(". {name}, {keys}"));
                }
            }
            HelpPage::DeveloperTools => {
                label.push_str(&format!(". {}", self.t("help.diagnostics.platform")))
            }
            HelpPage::About => label.push_str(&format!(
                ". Graphite {}. {}",
                env!("CARGO_PKG_VERSION"),
                self.t("help.license_mit")
            )),
        }
        label
    }
    pub(crate) fn shortcut_reference(&self) -> Vec<(String, String)> {
        let mut entries = self
            .keymap
            .reference_bindings()
            .into_iter()
            .map(|(command, keys)| {
                let label = match command.as_str() {
                    "workbench.action.showCommands" => self.t("help.shortcut.palette").to_owned(),
                    "workbench.action.files.newUntitledFile" => {
                        self.t("help.shortcut.new_file").to_owned()
                    }
                    "workbench.action.files.openFile" => {
                        self.t("help.shortcut.open_file").to_owned()
                    }
                    "workbench.action.files.save" => self.t("help.shortcut.save").to_owned(),
                    "actions.find" => self.t("help.shortcut.find").to_owned(),
                    "actions.replace" => self.t("help.shortcut.replace").to_owned(),
                    "workbench.action.findInFiles" => self.t("help.shortcut.search").to_owned(),
                    "graphite.plugins.open" => self.t("help.shortcut.extensions").to_owned(),
                    "workbench.action.terminal.toggleTerminal" => {
                        self.t("help.shortcut.terminal").to_owned()
                    }
                    "workbench.action.closeActiveEditor" => {
                        self.t("help.shortcut.close_editor").to_owned()
                    }
                    _ => super::command_registry::registry()
                        .get(&command)
                        .map_or_else(
                            || command.clone(),
                            |descriptor| {
                                self.translator
                                    .text_or(&format!("command.{command}"), descriptor.title)
                                    .into()
                            },
                        ),
                };
                (label, keys)
            })
            .collect::<Vec<_>>();
        for (label, command, fallback) in [
            (
                "help.shortcut.new_file",
                "workbench.action.files.newUntitledFile",
                "Ctrl+N",
            ),
            (
                "help.shortcut.open_file",
                "workbench.action.files.openFile",
                "Ctrl+O",
            ),
            (
                "help.shortcut.open_folder",
                "workbench.action.files.openFolder",
                "Ctrl+K Ctrl+O",
            ),
            (
                "help.shortcut.save",
                "workbench.action.files.save",
                "Ctrl+S",
            ),
            (
                "help.shortcut.save_as",
                "workbench.action.files.saveAs",
                "Ctrl+Shift+S",
            ),
            (
                "help.shortcut.save_all",
                "workbench.action.files.saveAll",
                "Ctrl+K S",
            ),
            (
                "help.shortcut.new_window",
                "workbench.action.newWindow",
                "Ctrl+Shift+N",
            ),
        ] {
            entries.push((
                self.t(label).to_owned(),
                self.keymap
                    .configured_shortcut(command)
                    .unwrap_or_else(|| fallback.to_owned()),
            ));
        }
        entries.sort_by(|left, right| {
            left.0
                .to_lowercase()
                .cmp(&right.0.to_lowercase())
                .then(left.1.cmp(&right.1))
        });
        entries.dedup();
        entries
    }
    fn developer_diagnostics(&self, width: u32, height: u32) -> Vec<String> {
        vec![
            format!("Graphite {}", env!("CARGO_PKG_VERSION")),
            format!(
                "{}: {} {}",
                self.t("help.diagnostics.platform"),
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
            format!(
                "{}: {} × {}",
                self.t("help.diagnostics.window"),
                width,
                height
            ),
            format!(
                "{}: {}",
                self.t("help.diagnostics.workspace"),
                self.tree.root().display()
            ),
            format!(
                "{}: {}",
                self.t("help.diagnostics.open_documents"),
                self.documents.len()
            ),
            format!(
                "{}: {}",
                self.t("help.diagnostics.plugins"),
                self.plugins.hosts.len()
            ),
            format!(
                "{}: {}",
                self.t("help.diagnostics.language_servers"),
                self.lsp_manager.server_count()
            ),
        ]
    }
}

fn help_modal_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let w = MODAL_WIDTH.min((width - 32.0).max(320.0));
    let h = MODAL_HEIGHT.min((height - 32.0).max(240.0));
    ((width - w) * 0.5, (height - h) * 0.5, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn app() -> (Temp, App) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "graphite-help-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        let temp = Temp(path.clone());
        let app = App::new_with_options(path, None, true);
        (temp, app)
    }

    #[test]
    fn help_menu_has_only_requested_actions_and_restart_is_disabled() {
        let (_temp, app) = app();
        let items: Vec<_> = app
            .help_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                HelpRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                HelpRow::Separator => None,
            })
            .collect();
        assert_eq!(
            items
                .iter()
                .map(|(command, _)| *command)
                .collect::<Vec<_>>(),
            vec![
                HelpCommand::Welcome,
                HelpCommand::ShowAllCommands,
                HelpCommand::EditorPlayground,
                HelpCommand::OpenWalkthrough,
                HelpCommand::ShowReleaseNotes,
                HelpCommand::KeyboardShortcutsReference,
                HelpCommand::ToggleDeveloperTools,
                HelpCommand::OpenProcessExplorer,
                HelpCommand::RestartToUpdate,
                HelpCommand::About,
            ]
        );
        assert!(!items[8].1);
    }

    #[test]
    fn modal_pages_are_dismissed_with_escape_and_shortcut_search_accepts_text() {
        let (_temp, mut app) = app();
        app.open_help_modal(HelpPage::KeyboardShortcuts);
        assert!(app.help_modal_key(&Key::Character("s".into()), Some("save"), false));
        assert_eq!(app.help_modal.as_ref().unwrap().query, "save");
        assert!(app.help_modal_key(&Key::Named(NamedKey::Escape), None, false));
        assert!(app.help_modal.is_none());
    }

    #[test]
    fn welcome_creates_an_untitled_document_and_playground_loads_editable_sample() {
        let (_temp, mut app) = app();
        let initial_documents = app.documents.len();
        app.open_help_modal(HelpPage::Welcome);
        let welcome = help_modal_rect(1200.0, 760.0);
        app.help_modal_click(welcome.0 + 60.0, welcome.1 + 145.0);
        assert!(app.help_modal.is_none());
        assert_eq!(app.documents.len(), initial_documents + 1);

        app.open_help_modal(HelpPage::Playground);
        let playground = help_modal_rect(1200.0, 760.0);
        app.help_modal_click(playground.0 + 120.0, playground.1 + 150.0);
        let document = app.active_document();
        assert!(app.documents[document]
            .buffer
            .text()
            .contains("Hello, Graphite!"));
        assert!(app.help_modal.is_none());
    }

    #[test]
    fn modal_accessibility_actions_activate_welcome_and_playground_controls() {
        let (_temp, mut app) = app();
        let count = app.documents.len();
        app.open_help_modal(HelpPage::Welcome);
        app.activate_help_modal_accessibility_action(1);
        assert_eq!(app.documents.len(), count + 1);
        assert!(app.help_modal.is_none());

        app.open_help_modal(HelpPage::Playground);
        app.activate_help_modal_accessibility_action(1);
        let document = app.active_document();
        assert!(app.documents[document].buffer.text().contains("fn main()"));
        assert!(app.help_modal.is_none());
    }

    #[test]
    fn help_popup_hit_testing_respects_separators_and_disabled_items() {
        let (_temp, app) = app();
        let x = 24.0;
        let y = 32.0 + 4.0 + 5.0 * ROW_HEIGHT + 10.0;
        assert_eq!(
            app.help_menu_command_at(x, y, 0.0, 32.0, app.help_menu_width(8.0, 800.0)),
            Some((HelpCommand::KeyboardShortcutsReference, true))
        );
        let disabled_y =
            32.0 + 4.0 + 5.0 * ROW_HEIGHT + 9.0 + ROW_HEIGHT + 9.0 + 2.0 * ROW_HEIGHT + 9.0 + 2.0;
        assert_eq!(
            app.help_menu_command_at(x, disabled_y, 0.0, 32.0, app.help_menu_width(8.0, 800.0)),
            Some((HelpCommand::RestartToUpdate, false))
        );
    }

    #[test]
    fn help_menu_width_reserves_shortcuts_and_clamps_to_narrow_windows() {
        let (_temp, app) = app();
        let width = app.help_menu_width(8.0, 1200.0);
        assert!(width >= MENU_MIN_WIDTH);
        assert!(width > MENU_MIN_WIDTH);
        assert_eq!(app.help_menu_width(8.0, 190.0), 190.0);

        let label = app.t("help.show_all_commands");
        let clipped = App::help_menu_visible_label(label, 8.0, 80.0);
        assert!(clipped.ends_with('…'));
        assert!(clipped.chars().count() <= 10);
    }

    #[test]
    fn help_menu_update_row_uses_localized_up_to_date_placeholder() {
        let (_temp, app) = app();
        let update = app
            .help_menu_rows()
            .into_iter()
            .find_map(|row| match row {
                HelpRow::Item {
                    command: HelpCommand::RestartToUpdate,
                    label,
                    enabled,
                    ..
                } => Some((label, enabled)),
                _ => None,
            })
            .unwrap();
        assert_eq!(update.0, "help.up_to_date");
        assert!(!update.1);
        assert_ne!(app.t("help.up_to_date"), "help.up_to_date");
    }

    #[test]
    fn shortcut_reference_uses_a_profile_override_when_available() {
        let (_temp, mut app) = app();
        app.keymap
            .reload(
                r#"[{"key":"ctrl+alt+p","command":"workbench.action.showCommands"}]"#,
                &graphite_i18n::Translator::english(),
            )
            .unwrap();
        let palette = app.t("help.shortcut.palette").to_owned();
        assert!(app
            .shortcut_reference()
            .iter()
            .any(|(name, keys)| name == &palette && keys == "Ctrl+Alt+P"));
    }
}
