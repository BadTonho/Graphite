use super::layout::PluginsLayout;
use super::*;

pub(super) const ACTION_NODE_BASE: u64 = 0x6000_0000_0000_0100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Install,
    Reload,
    More,
    Views,
    Section(PluginsTab),
    Check,
    UpdateAll,
    AutoUpdate,
    EnableAll,
    DisableAll,
    Running,
    Bisect,
    ProblemPersists,
    ProblemGone,
    StopBisect,
    ConfirmInstall,
    CancelInstall,
    ConfirmTrust,
    SkipTrust,
    CloseRunning,
}

const HEADER: [(Action, &str); 3] = [
    (Action::Install, "+"),
    (Action::Reload, "↻"),
    (Action::More, "⋯"),
];
const MENU: [Action; 9] = [
    Action::Views,
    Action::Check,
    Action::UpdateAll,
    Action::AutoUpdate,
    Action::EnableAll,
    Action::DisableAll,
    Action::Running,
    Action::Bisect,
    Action::Install,
];
const SECTIONS: [Action; 3] = [
    Action::Section(PluginsTab::Installed),
    Action::Section(PluginsTab::OpenVsx),
    Action::Section(PluginsTab::Graphite),
];
const REVIEW: [Action; 2] = [Action::ConfirmInstall, Action::CancelInstall];
const TRUST: [Action; 2] = [Action::ConfirmTrust, Action::SkipTrust];
const TRIAL: [Action; 3] = [
    Action::ProblemPersists,
    Action::ProblemGone,
    Action::StopBisect,
];
const RUNNING: [Action; 1] = [Action::CloseRunning];

#[derive(Default)]
pub(super) struct Menu {
    index: usize,
    views: bool,
}

impl Menu {
    fn actions(&self) -> &'static [Action] {
        if self.views {
            &SECTIONS
        } else {
            &MENU
        }
    }

    fn visible_rows(&self, height: f32, lh: f32) -> std::ops::Range<usize> {
        let count = ((height / lh - 2.15) / 1.65).floor().max(1.0) as usize;
        let count = count.min(self.actions().len());
        let first = self.index.saturating_sub(count - 1);
        first..first + count
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

fn header_rect(layout: &PluginsLayout, lh: f32, index: usize) -> Rect {
    let side = lh * 1.45;
    Rect {
        x: layout.sidebar_x + layout.width - 4.0 - (HEADER.len() - index) as f32 * side,
        y: 0.0,
        w: side,
        h: lh * 1.15,
    }
}

fn menu_rect(layout: &PluginsLayout, lh: f32, index: usize) -> Rect {
    let w = (layout.width - 8.0).min(lh * 23.0).max(0.0);
    Rect {
        x: layout.sidebar_x + layout.width - w - 4.0,
        y: lh * (1.15 + 1.65 * index as f32),
        w,
        h: lh * 1.65,
    }
}

fn detail_rect(layout: &PluginsLayout, lh: f32, index: usize) -> Rect {
    Rect {
        x: layout.sidebar_x + 5.0,
        y: layout.footer_y + lh * 1.5 * index as f32,
        w: (layout.width - 10.0).max(0.0),
        h: lh * 1.4,
    }
}

impl App {
    fn toolbar_details(&self) -> &'static [Action] {
        if self.plugins.preview.is_some() {
            &REVIEW
        } else if self.plugins.trust.is_some() {
            &TRUST
        } else if self.plugins.bisect.is_some() {
            &TRIAL
        } else if self.plugins.running_scroll.is_some() {
            &RUNNING
        } else {
            &[]
        }
    }

    pub(super) fn plugin_toolbar_label(&self, action: Action) -> &str {
        self.t(match action {
            Action::Install => "plugins.toolbar.install",
            Action::Reload => "plugins.toolbar.reload",
            Action::More => "plugins.toolbar.more",
            Action::Views => "plugins.toolbar.views",
            Action::Section(PluginsTab::Installed) => "plugins.sidebar.installed",
            Action::Section(PluginsTab::OpenVsx) => "plugins.sidebar.open_vsx",
            Action::Section(PluginsTab::Graphite) => "plugins.sidebar.graphite",
            Action::Check => "plugins.toolbar.check",
            Action::UpdateAll if self.plugins.update_state.ready.is_some() => {
                "plugins.toolbar.review_updates"
            }
            Action::UpdateAll => "plugins.toolbar.update_all",
            Action::AutoUpdate if self.plugins.update_state.automatic => {
                "plugins.toolbar.auto_disable"
            }
            Action::AutoUpdate => "plugins.toolbar.auto_enable",
            Action::EnableAll => "plugins.toolbar.enable_all",
            Action::DisableAll => "plugins.toolbar.disable_all",
            Action::Running => "plugins.toolbar.running",
            Action::Bisect => "plugins.toolbar.bisect",
            Action::ProblemPersists => "plugins.toolbar.problem_persists",
            Action::ProblemGone => "plugins.toolbar.problem_gone",
            Action::StopBisect => "plugins.toolbar.cancel_bisect",
            Action::ConfirmInstall => "plugins.toolbar.confirm_install",
            Action::CancelInstall => "plugins.toolbar.cancel",
            Action::ConfirmTrust => "plugins.toolbar.confirm_trust",
            Action::SkipTrust => "plugins.toolbar.skip",
            Action::CloseRunning => "plugins.toolbar.close",
        })
    }

    pub(super) fn plugin_toolbar_enabled(&self, action: Action) -> bool {
        match action {
            Action::More
            | Action::Views
            | Action::StopBisect
            | Action::CloseRunning
            | Action::CancelInstall
            | Action::SkipTrust => true,
            Action::ProblemPersists | Action::ProblemGone | Action::ConfirmInstall => {
                self.plugins.worker.is_none() && self.plugins.startup.is_empty()
            }
            Action::ConfirmTrust => self.plugins.worker.is_none(),
            _ => self.plugins.worker.is_none() && !self.plugin_details_open(),
        }
    }

    pub(super) fn plugin_toolbar_action(&mut self, action: Action) {
        if !self.plugin_toolbar_enabled(action) {
            return;
        }
        if action == Action::More {
            self.plugins.toolbar_menu = if self.plugins.toolbar_menu.is_some() {
                None
            } else {
                Some(Menu::default())
            };
            return;
        }
        if action == Action::Views {
            self.plugins.toolbar_menu = Some(Menu {
                index: self.plugins.tab.index(),
                views: true,
            });
            return;
        }
        self.plugins.toolbar_menu = None;
        self.plugins.toolbar_focus = None;
        self.plugins.editing = false;
        match action {
            Action::Install => self.pick_local_vsix(),
            Action::Reload => self.reload_plugin_sidebar(),
            Action::Section(tab) => {
                self.plugin_action(tab.action_key());
                self.plugins.section_collapsed[tab.index()] = false;
            }
            Action::Check => self.plugins_check_updates(),
            Action::UpdateAll => self.plugins_update_all(),
            Action::AutoUpdate => self.toggle_plugin_auto_updates(),
            Action::EnableAll => self.plugins_enable_all(),
            Action::DisableAll => self.plugins_disable_all(),
            Action::Running => self.plugins.running_scroll = Some(0),
            Action::Bisect => self.start_plugin_bisect(),
            Action::ProblemPersists => self.plugin_bisect_answer(true),
            Action::ProblemGone => self.plugin_bisect_answer(false),
            Action::StopBisect => self.cancel_plugin_bisect(),
            Action::ConfirmInstall => self.confirm_plugin_install(),
            Action::CancelInstall => self.cancel_plugin_install(),
            Action::ConfirmTrust => self.confirm_plugin_trust(),
            Action::SkipTrust => {
                self.plugins.trust = None;
            }
            Action::CloseRunning => self.plugins.running_scroll = None,
            Action::More | Action::Views => unreachable!(),
        }
    }

    fn pick_local_vsix(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("VSIX", &["vsix", "VSIX"])
            .pick_file()
        {
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("vsix"))
            {
                self.inspect_local_plugin(path);
            } else {
                self.plugins.message = self.t("plugins.toolbar.expected_vsix").into();
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Medium,
                    "plugins.install.picker",
                    &self.plugins.message,
                );
            }
        }
    }

    pub(super) fn inspect_local_plugin(&mut self, path: PathBuf) {
        self.plugin_job(move |_| {
            let metadata = graphite_plugins::package::inspect(&path)?;
            Ok(Outcome::LocalPreview(path, metadata))
        });
    }

    pub(super) fn cancel_plugin_install(&mut self) {
        self.plugins.preview = None;
        if let Some(update) = self.plugins.downloaded_update.take() {
            super::panel::close_plugin_download(update.download);
        }
        self.cleanup_plugin_download();
    }

    pub(super) fn confirm_plugin_trust(&mut self) {
        if let Some(id) = self.plugins.trust.take() {
            self.enable_plugin(id);
        }
    }

    pub(super) fn confirm_plugin_install(&mut self) {
        let Some((path, metadata)) = self.plugins.preview.take() else {
            return;
        };
        let update = self.plugins.downloaded_update.take();
        let temporary = self.plugins.temporary.take();
        let id = metadata.id().to_owned();
        self.plugins.tab = PluginsTab::Installed;
        self.plugins.section_collapsed[PluginsTab::Installed.index()] = false;
        self.plugins.index = 0;
        self.plugins.scroll = 0;
        self.plugin_job(move |store| {
            let installed = if let Some(update) = update {
                let result = store.install_update(
                    &path,
                    &id,
                    &update.candidate.directory,
                    &update.candidate.version,
                );
                super::panel::close_plugin_download(update.download);
                result
            } else {
                store.install_reviewed(&path, &id, metadata.version())
            };
            if let Some(download) = temporary {
                super::panel::close_plugin_download(download);
            }
            installed?;
            Ok(Outcome::Installed(id, store.list()))
        });
    }

    pub(crate) fn dismiss_plugin_menu(&mut self) {
        self.plugins.toolbar_menu = None;
        self.plugins.toolbar_focus = None;
    }

    pub(super) fn plugin_toolbar_key(&mut self, key: &Key, reverse: bool) -> bool {
        if let Some(menu) = &mut self.plugins.toolbar_menu {
            let count = menu.actions().len();
            match key {
                Key::Named(NamedKey::Escape | NamedKey::ArrowLeft) if menu.views => {
                    self.plugins.toolbar_menu = Some(Menu::default());
                }
                Key::Named(NamedKey::Escape) => self.dismiss_plugin_menu(),
                Key::Named(NamedKey::ArrowUp) => menu.index = (menu.index + count - 1) % count,
                Key::Named(NamedKey::ArrowDown) => menu.index = (menu.index + 1) % count,
                Key::Named(NamedKey::Enter | NamedKey::Space | NamedKey::ArrowRight) => {
                    let action = menu.actions()[menu.index];
                    self.plugin_toolbar_action(action);
                }
                _ => {}
            }
            return true;
        }
        if matches!(key, Key::Named(NamedKey::Tab)) {
            self.plugins.editing = false;
            let count = HEADER.len() + self.toolbar_details().len() + 1;
            let previous = self.plugins.toolbar_focus.map_or(0, |index| index + 1);
            let next = (previous + if reverse { count - 1 } else { 1 }) % count;
            self.plugins.toolbar_focus = next.checked_sub(1);
            return true;
        }
        if let Some(index) = self.plugins.toolbar_focus {
            match key {
                Key::Named(NamedKey::Enter | NamedKey::Space) => {
                    if let Some((action, _)) = HEADER.get(index) {
                        self.plugin_toolbar_action(*action);
                    } else if let Some(action) = self.toolbar_details().get(index - HEADER.len()) {
                        self.plugin_toolbar_action(*action);
                    }
                    return true;
                }
                Key::Named(NamedKey::Escape) => {
                    self.plugins.toolbar_focus = None;
                    return true;
                }
                _ => {}
            }
        }
        if let Some(scroll) = self.plugins.running_scroll {
            match key {
                Key::Named(NamedKey::Escape) => self.plugins.running_scroll = None,
                Key::Named(NamedKey::ArrowUp) => {
                    self.plugins.running_scroll = Some(scroll.saturating_sub(1))
                }
                Key::Named(NamedKey::ArrowDown) => self.scroll_running_plugins(1),
                _ => {}
            }
            return true;
        }
        if self.plugins.bisect.is_some() {
            match key {
                Key::Named(NamedKey::Escape) => self.cancel_plugin_bisect(),
                _ => {}
            }
            return true;
        }
        false
    }

    pub(super) fn plugin_toolbar_click(
        &mut self,
        layout: &PluginsLayout,
        lh: f32,
        height: f32,
        x: f32,
        y: f32,
    ) -> bool {
        for (index, (action, _)) in HEADER.iter().enumerate() {
            if header_rect(layout, lh, index).contains(x, y) {
                self.plugin_toolbar_action(*action);
                return true;
            }
        }
        if let Some(menu) = &self.plugins.toolbar_menu {
            for (row, index) in menu.visible_rows(height, lh).enumerate() {
                if menu_rect(layout, lh, row).contains(x, y) {
                    self.plugin_toolbar_action(menu.actions()[index]);
                    return true;
                }
            }
            self.dismiss_plugin_menu();
            return true;
        }
        for (index, action) in self.toolbar_details().iter().copied().enumerate() {
            if detail_rect(layout, lh, index).contains(x, y) {
                self.plugin_toolbar_action(action);
                return true;
            }
        }
        self.plugins.bisect.is_some() || self.plugins.running_scroll.is_some()
    }

    pub(super) fn scroll_plugin_menu(&mut self, lines: i32) -> bool {
        let Some(menu) = &mut self.plugins.toolbar_menu else {
            return false;
        };
        menu.index = (menu.index as i64 + i64::from(lines))
            .clamp(0, menu.actions().len() as i64 - 1) as usize;
        true
    }

    pub(super) fn plugin_toolbar_nodes(&self) -> Vec<(Action, &str)> {
        HEADER
            .iter()
            .map(|&(action, _)| (action, self.plugin_toolbar_label(action)))
            .chain(
                self.toolbar_details()
                    .iter()
                    .map(|&action| (action, self.plugin_toolbar_label(action))),
            )
            .chain(self.plugins.toolbar_menu.iter().flat_map(|menu| {
                menu.actions()
                    .iter()
                    .map(|&action| (action, self.plugin_toolbar_label(action)))
            }))
            .collect()
    }

    pub(crate) fn plugin_accessibility_click(&mut self, id: u64) -> bool {
        let Some(index) = id.checked_sub(ACTION_NODE_BASE).filter(|index| *index < 32) else {
            return false;
        };
        if self.plugin_owns_keyboard() {
            if let Some((action, _)) = self.plugin_toolbar_nodes().get(index as usize) {
                self.plugin_toolbar_action(*action);
            }
        }
        true
    }

    pub(super) fn plugin_toolbar_accessibility_focus(&self) -> Option<usize> {
        self.plugins
            .toolbar_menu
            .as_ref()
            .map(|menu| HEADER.len() + self.toolbar_details().len() + menu.index)
            .or(self.plugins.toolbar_focus)
    }

    fn running_plugin_labels(&self) -> Vec<String> {
        let mut labels: Vec<_> = self
            .plugins
            .hosts
            .iter()
            .filter(|(_, host)| host.native_active)
            .map(|(id, _)| id.clone())
            .collect();
        if let Some(runtime) = &self.plugins.vsix {
            labels.extend(runtime.active_extensions.iter().cloned());
        }
        labels.sort_unstable();
        labels
    }

    pub(super) fn plugin_running_summary(&self) -> String {
        let labels = self.running_plugin_labels();
        if labels.is_empty() {
            self.t("plugins.toolbar.none_running").into()
        } else {
            labels.join(", ")
        }
    }

    pub(super) fn scroll_running_plugins(&mut self, lines: i32) {
        if let Some(scroll) = self.plugins.running_scroll {
            let last = self.running_plugin_labels().len().saturating_sub(1);
            self.plugins.running_scroll =
                Some((scroll as i64 + i64::from(lines)).clamp(0, last as i64) as usize);
        }
    }

    pub(super) fn draw_plugin_toolbar(
        &self,
        renderer: &mut Renderer,
        layout: &PluginsLayout,
        lh: f32,
        cw: f32,
    ) {
        let (window_width, height) = renderer.size();
        let x = self.sidebar_local_x(self.mouse.x as f32, window_width);
        let y = self.sidebar_local_y(self.mouse.y as f32);
        for (index, &(action, glyph)) in HEADER.iter().enumerate() {
            let rect = header_rect(layout, lh, index);
            let focused = self.plugins.toolbar_focus == Some(index)
                || rect.contains(x, y)
                || (action == Action::More && self.plugins.toolbar_menu.is_some());
            self.draw_plugin_action(renderer, rect, action, glyph, lh, cw, focused, true);
        }
        if let Some(scroll) = self.plugins.running_scroll {
            let mut labels = self.running_plugin_labels();
            if labels.is_empty() {
                labels.push(self.t("plugins.toolbar.none_running").to_owned());
            }
            let max = ((layout.width - 12.0) / cw).max(1.0) as usize;
            for (index, label) in labels.iter().skip(scroll.min(labels.len() - 1)).enumerate() {
                let y = layout.list_y + index as f32 * lh;
                if y + lh > layout.list_bottom {
                    break;
                }
                renderer.draw_text(
                    &super::drawing::fit(label, max),
                    layout.sidebar_x + 6.0,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        for (index, &action) in self.toolbar_details().iter().enumerate() {
            let rect = detail_rect(layout, lh, index);
            let focused =
                self.plugins.toolbar_focus == Some(HEADER.len() + index) || rect.contains(x, y);
            self.draw_plugin_action(
                renderer,
                rect,
                action,
                self.plugin_toolbar_label(action),
                lh,
                cw,
                focused,
                false,
            );
        }
        if let Some(menu) = &self.plugins.toolbar_menu {
            for (row, index) in menu.visible_rows(height, lh).enumerate() {
                let action = menu.actions()[index];
                let rect = menu_rect(layout, lh, row);
                let focused = rect.contains(x, y) || index == menu.index;
                self.draw_plugin_action(
                    renderer,
                    rect,
                    action,
                    self.plugin_toolbar_label(action),
                    lh,
                    cw,
                    focused,
                    false,
                );
            }
        } else if let Some(action) = HEADER.iter().enumerate().find_map(|(index, &(action, _))| {
            header_rect(layout, lh, index)
                .contains(x, y)
                .then_some(action)
        }) {
            let label = self.plugin_toolbar_label(action);
            let right = layout.sidebar_x + layout.width - 4.0;
            let width = (label.chars().count() as f32 * cw + 12.0).min(layout.width - 8.0);
            let rect = Rect {
                x: right - width,
                y: lh * 1.15,
                w: width,
                h: lh * 1.4,
            };
            self.draw_plugin_action(renderer, rect, action, label, lh, cw, false, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_plugin_action(
        &self,
        renderer: &mut Renderer,
        rect: Rect,
        action: Action,
        label: &str,
        lh: f32,
        cw: f32,
        focused: bool,
        icon: bool,
    ) {
        let enabled = self.plugin_toolbar_enabled(action);
        let background = if focused {
            Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
        } else if icon {
            Theme::color(Theme::SIDEBAR, 0xf3f3f3)
        } else {
            Theme::color(Theme::BG, 0xffffff)
        };
        renderer.draw_rect(rect.x, rect.y, rect.w, rect.h, rgb(background));
        let max = ((rect.w - 10.0) / cw).max(1.0) as usize;
        let text = super::drawing::fit(label, max);
        let offset = if icon { (rect.w - cw) / 2.0 } else { 5.0 };
        let color = if enabled {
            Theme::color(Theme::TEXT, 0x1e1e1e)
        } else {
            Theme::color(Theme::DIM, 0x616161)
        };
        renderer.draw_text(
            &text,
            rect.x + offset,
            rect.y + (rect.h - lh) / 2.0,
            rgb(color),
        );
    }
}
