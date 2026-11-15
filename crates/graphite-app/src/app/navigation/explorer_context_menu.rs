//! Right-click context menu for the Explorer sidebar (files, folders, and empty workspace area).

use super::*;
use crate::app::activity_bar::Activity;
use crate::app::clipboard::{ClipboardAccess, SystemClipboard};
use crate::app::navigation::project_search_filters::SearchFocus;
use winit::keyboard::{Key, NamedKey};

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 6.0;
const MIN_MENU_WIDTH: f32 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExplorerContextAction {
    NewFile,
    NewFolder,
    RevealInFileExplorer,
    OpenInTerminal,
    AddFolderToWorkspace,
    OpenFolderSettings,
    RemoveFolderFromWorkspace,
    FindInFolder,
    OpenTimeline,
    AddToChat,
    Cut,
    Copy,
    Paste,
    CopyPath,
    CopyRelativePath,
    Rename,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExplorerContextMenuRow {
    Action {
        label_key: &'static str,
        shortcut: &'static str,
        action: ExplorerContextAction,
        enabled: bool,
    },
    Separator,
}

impl ExplorerContextMenuRow {
    fn is_selectable(&self) -> bool {
        matches!(self, Self::Action { enabled: true, .. })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExplorerContextMenu {
    pub target: PathBuf,
    pub is_dir: bool,
    pub is_root: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub rows: Vec<ExplorerContextMenuRow>,
}

fn row_height(row: &ExplorerContextMenuRow) -> f32 {
    if matches!(row, ExplorerContextMenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn menu_height(rows: &[ExplorerContextMenuRow]) -> f32 {
    PADDING * 2.0 + rows.iter().map(row_height).sum::<f32>()
}

fn menu_rect(menu: &ExplorerContextMenu) -> (f32, f32, f32, f32) {
    (menu.x, menu.y, menu.width, menu_height(&menu.rows))
}

pub(crate) fn place_context_menu(
    anchor: (f32, f32),
    size: (f32, f32),
    window: (f32, f32),
    bottom_inset: f32,
) -> (f32, f32) {
    let (ax, ay) = anchor;
    let (width, height) = size;
    let (window_width, window_height) = window;
    let max_x = (window_width - width - 4.0).max(4.0);
    let x = ax.clamp(4.0, max_x);
    let max_bottom = (window_height - bottom_inset - 4.0).max(0.0);
    let y = if ay + height <= max_bottom {
        ay
    } else {
        (ay - height).max(4.0)
    };
    (x, y)
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn reveal_in_os_file_manager(target: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("explorer.exe");
        if target.is_file() {
            command.arg(format!("/select,{}", target.display()));
        } else {
            command.arg(target);
        }
        command.spawn().map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let dir = if target.is_dir() {
            target
        } else {
            target.parent().unwrap_or(target)
        };
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

impl App {
    pub(crate) fn explorer_right_click(&mut self, x: f32, y: f32) -> bool {
        let (window_width, window_height, lh, cw) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (w, h) = gpu.renderer.size();
                (w, h, gpu.renderer.line_height(), gpu.renderer.char_width())
            })
            .unwrap_or((1200.0, 760.0, 18.0, 8.0));

        if self.zen_mode
            || !self.view_preferences.primary_sidebar_visible
            || self.active_activity() != Activity::Explorer
            || self.active_plugin_activity.is_some()
            || !self.primary_sidebar_contains(x, y, window_width, window_height, lh)
        {
            self.explorer_context_menu = None;
            return false;
        }

        let target = self.explorer_target_at(y, lh);
        self.explorer_select(target.clone());
        self.workbench_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.terminal_toolbar_menu = None;
        self.activity_bar_menu = None;

        self.open_explorer_context_menu(target, (x, y), (window_width, window_height), lh, cw);
        true
    }

    fn explorer_target_at(&self, y: f32, lh: f32) -> PathBuf {
        let first_row_y = lh * 1.5;
        if y >= first_row_y {
            let row = ((y - first_row_y) / lh) as usize + self.explorer_scroll;
            if let Some(entry) = self.tree.rows.get(row) {
                return entry.entry.path.clone();
            }
        }
        self.tree.root().to_path_buf()
    }

    pub(crate) fn open_explorer_context_menu(
        &mut self,
        target: PathBuf,
        anchor: (f32, f32),
        window: (f32, f32),
        lh: f32,
        cw: f32,
    ) {
        let is_dir = target.is_dir();
        let norm_target = normalize_path(&target);
        let is_root = self
            .tree
            .roots()
            .iter()
            .any(|root| normalize_path(root) == norm_target);
        let can_remove_root = is_root && self.tree.roots().len() > 1;
        let has_clipboard = self.explorer_clipboard.is_some();

        let mut rows = vec![
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.new_file",
                shortcut: "",
                action: ExplorerContextAction::NewFile,
                enabled: true,
            },
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.new_folder",
                shortcut: "",
                action: ExplorerContextAction::NewFolder,
                enabled: true,
            },
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.reveal",
                shortcut: "Shift+Alt+R",
                action: ExplorerContextAction::RevealInFileExplorer,
                enabled: true,
            },
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.open_terminal",
                shortcut: "",
                action: ExplorerContextAction::OpenInTerminal,
                enabled: true,
            },
            ExplorerContextMenuRow::Separator,
        ];

        if is_dir {
            rows.extend([
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.add_folder_workspace",
                    shortcut: "",
                    action: ExplorerContextAction::AddFolderToWorkspace,
                    enabled: true,
                },
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.open_folder_settings",
                    shortcut: "",
                    action: ExplorerContextAction::OpenFolderSettings,
                    enabled: true,
                },
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.remove_folder_workspace",
                    shortcut: "",
                    action: ExplorerContextAction::RemoveFolderFromWorkspace,
                    enabled: can_remove_root,
                },
                ExplorerContextMenuRow::Separator,
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.find_in_folder",
                    shortcut: "Shift+Alt+F",
                    action: ExplorerContextAction::FindInFolder,
                    enabled: true,
                },
                ExplorerContextMenuRow::Separator,
            ]);
        }

        rows.extend([
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.open_timeline",
                shortcut: "",
                action: ExplorerContextAction::OpenTimeline,
                enabled: true,
            },
            ExplorerContextMenuRow::Separator,
            ExplorerContextMenuRow::Action {
                label_key: if is_dir {
                    "explorer.menu.add_folder_chat"
                } else {
                    "explorer.menu.add_file_chat"
                },
                shortcut: "",
                action: ExplorerContextAction::AddToChat,
                enabled: true,
            },
            ExplorerContextMenuRow::Separator,
        ]);

        if !is_root {
            rows.extend([
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.cut",
                    shortcut: "Ctrl+X",
                    action: ExplorerContextAction::Cut,
                    enabled: true,
                },
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.copy",
                    shortcut: "Ctrl+C",
                    action: ExplorerContextAction::Copy,
                    enabled: true,
                },
            ]);
        }

        rows.extend([
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.paste",
                shortcut: "Ctrl+V",
                action: ExplorerContextAction::Paste,
                enabled: has_clipboard,
            },
            ExplorerContextMenuRow::Separator,
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.copy_path",
                shortcut: "Shift+Alt+C",
                action: ExplorerContextAction::CopyPath,
                enabled: true,
            },
            ExplorerContextMenuRow::Action {
                label_key: "explorer.menu.copy_relative_path",
                shortcut: "Ctrl+K Ctrl+Shift+C",
                action: ExplorerContextAction::CopyRelativePath,
                enabled: true,
            },
        ]);

        if !is_root {
            rows.extend([
                ExplorerContextMenuRow::Separator,
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.rename",
                    shortcut: "F2",
                    action: ExplorerContextAction::Rename,
                    enabled: true,
                },
                ExplorerContextMenuRow::Action {
                    label_key: "explorer.menu.delete",
                    shortcut: "Delete",
                    action: ExplorerContextAction::Delete,
                    enabled: true,
                },
            ]);
        }

        let char_w = cw.max(6.0);
        let max_row_chars = rows
            .iter()
            .filter_map(|row| match row {
                ExplorerContextMenuRow::Action {
                    label_key,
                    shortcut,
                    ..
                } => Some(self.t(label_key).chars().count() + shortcut.chars().count() + 5),
                ExplorerContextMenuRow::Separator => None,
            })
            .max()
            .unwrap_or(24);
        let width = (max_row_chars as f32 * char_w + 28.0).max(MIN_MENU_WIDTH);
        let height = menu_height(&rows);
        let bottom_inset = if self.view_preferences.status_bar_visible && !self.zen_mode {
            lh
        } else {
            0.0
        };
        let (x, y) = place_context_menu(anchor, (width, height), window, bottom_inset);
        let selected = rows.iter().position(ExplorerContextMenuRow::is_selectable).unwrap_or(0);
        self.explorer_context_menu = Some(ExplorerContextMenu {
            target,
            is_dir,
            is_root,
            x,
            y,
            width,
            selected,
            rows,
        });
    }

    pub(crate) fn explorer_context_menu_contains(&self, x: f32, y: f32) -> bool {
        let Some(menu) = &self.explorer_context_menu else {
            return false;
        };
        let (mx, my, mw, mh) = menu_rect(menu);
        x >= mx && x < mx + mw && y >= my && y < my + mh
    }

    pub(crate) fn explorer_context_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let Some(menu) = &mut self.explorer_context_menu else {
            return;
        };
        let (mx, my, mw, _) = menu_rect(menu);
        if x < mx || x >= mx + mw {
            return;
        }
        let mut top = my + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            let h = row_height(row);
            if y >= top && y < top + h && row.is_selectable() {
                menu.selected = index;
                break;
            }
            top += h;
        }
    }

    pub(crate) fn explorer_context_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some(menu) = self.explorer_context_menu.clone() else {
            return false;
        };
        let (mx, my, mw, mh) = menu_rect(&menu);
        if x < mx || x >= mx + mw || y < my || y >= my + mh {
            self.explorer_context_menu = None;
            return false;
        }
        let mut top = my + PADDING;
        let mut clicked = None;
        for row in &menu.rows {
            let h = row_height(row);
            if y >= top && y < top + h {
                if let ExplorerContextMenuRow::Action {
                    action,
                    enabled: true,
                    ..
                } = row
                {
                    clicked = Some(*action);
                }
                break;
            }
            top += h;
        }
        self.explorer_context_menu = None;
        if let Some(action) = clicked {
            self.run_explorer_context_action(&menu.target, menu.is_dir, action);
        }
        true
    }

    pub(crate) fn explorer_context_menu_key(&mut self, key: &Key) -> bool {
        let Some(menu) = self.explorer_context_menu.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => {
                self.explorer_context_menu = None;
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                let len = menu.rows.len();
                for offset in 1..=len {
                    let candidate = (menu.selected + offset) % len;
                    if menu.rows[candidate].is_selectable() {
                        menu.selected = candidate;
                        break;
                    }
                }
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                let len = menu.rows.len();
                for offset in 1..=len {
                    let candidate = (menu.selected + len - offset) % len;
                    if menu.rows[candidate].is_selectable() {
                        menu.selected = candidate;
                        break;
                    }
                }
                true
            }
            Key::Named(NamedKey::Enter) => {
                let selected_action = match menu.rows.get(menu.selected) {
                    Some(ExplorerContextMenuRow::Action {
                        action,
                        enabled: true,
                        ..
                    }) => Some(*action),
                    _ => None,
                };
                let target = menu.target.clone();
                let is_dir = menu.is_dir;
                self.explorer_context_menu = None;
                if let Some(action) = selected_action {
                    self.run_explorer_context_action(&target, is_dir, action);
                }
                true
            }
            _ => false,
        }
    }

    fn run_explorer_context_action(
        &mut self,
        target: &Path,
        is_dir: bool,
        action: ExplorerContextAction,
    ) {
        self.explorer_select(target.to_path_buf());
        let target_dir = if is_dir {
            target.to_path_buf()
        } else {
            target
                .parent()
                .unwrap_or_else(|| self.tree.root())
                .to_path_buf()
        };

        match action {
            ExplorerContextAction::NewFile => self.explorer_new_entry(false),
            ExplorerContextAction::NewFolder => self.explorer_new_entry(true),
            ExplorerContextAction::RevealInFileExplorer => {
                if let Err(error) = reveal_in_os_file_manager(target) {
                    self.status = error;
                }
            }
            ExplorerContextAction::OpenInTerminal => {
                let result = if let Some(profile) = self
                    .terminal_profiles
                    .get(self.selected_terminal_profile)
                    .cloned()
                {
                    self.spawn_terminal_profile(&profile, Some(&target_dir), None)
                } else {
                    self.spawn_terminal_session()
                };
                match result {
                    Ok(_) => {
                        self.terminal_visible = true;
                        self.view_preferences.panel_visible = true;
                        self.terminal_focused = true;
                        self.terminal_problems = false;
                        self.resize_terminal();
                    }
                    Err(error) => {
                        self.terminal_status = Some(error.clone());
                        self.status = error;
                    }
                }
            }
            ExplorerContextAction::AddFolderToWorkspace => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    if let Err(error) = self.tree.add_workspace_root(folder) {
                        self.status = error.to_string();
                    }
                }
            }
            ExplorerContextAction::OpenFolderSettings => {
                let settings_path = target_dir.join(".vscode").join("settings.json");
                if !settings_path.exists() {
                    let created = settings_path
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|()| std::fs::write(&settings_path, b"{}\n"));
                    if let Err(error) = created {
                        self.status = error.to_string();
                        return;
                    }
                }
                self.open_file(settings_path);
            }
            ExplorerContextAction::RemoveFolderFromWorkspace => {
                let norm_target = normalize_path(target);
                if let Some(root) = self
                    .tree
                    .roots()
                    .iter()
                    .find(|root| normalize_path(root) == norm_target)
                    .cloned()
                {
                    self.tree.remove_workspace_root(&root);
                }
            }
            ExplorerContextAction::FindInFolder => {
                let rel = self.explorer_relative_display(target);
                self.open_project_search();
                self.search_filters.details = true;
                self.search_filters.include = if rel == "." { String::new() } else { rel };
                self.search_filters.focus = SearchFocus::Query;
            }
            ExplorerContextAction::OpenTimeline => {
                if !is_dir {
                    self.open_file(target.to_path_buf());
                }
                self.open_local_history();
            }
            ExplorerContextAction::AddToChat => {
                self.open_chat_sidebar();
                let rel = self.explorer_relative_display(target);
                let token = format!("@{rel}");
                if !self.chat.input.contains(&token) {
                    if !self.chat.input.is_empty() && !self.chat.input.ends_with(' ') {
                        self.chat.input.push(' ');
                    }
                    self.chat.input.push_str(&token);
                    self.chat.input.push(' ');
                }
            }
            ExplorerContextAction::Cut => self.explorer_cut(),
            ExplorerContextAction::Copy => self.explorer_copy_entry(),
            ExplorerContextAction::Paste => {
                if let Err(error) = self.explorer_paste() {
                    self.status = error;
                }
            }
            ExplorerContextAction::CopyPath => {
                let text = target.to_string_lossy().into_owned();
                self.copy_explorer_path_text(&text);
            }
            ExplorerContextAction::CopyRelativePath => {
                let text = self.explorer_relative_display(target);
                self.copy_explorer_path_text(&text);
            }
            ExplorerContextAction::Rename => self.explorer_rename(),
            ExplorerContextAction::Delete => self.explorer_delete(),
        }
    }

    fn explorer_relative_display(&self, target: &Path) -> String {
        let rel = target.strip_prefix(self.tree.root()).ok().map(Path::to_path_buf).or_else(|| {
            let norm_target = normalize_path(target);
            let norm_root = normalize_path(self.tree.root());
            norm_target
                .strip_prefix(&norm_root)
                .ok()
                .map(Path::to_path_buf)
        });
        if let Some(rel) = rel {
            let s = rel.to_string_lossy().replace('\\', "/");
            if s.is_empty() {
                ".".into()
            } else {
                s
            }
        } else {
            target.to_string_lossy().into_owned()
        }
    }

    fn copy_explorer_path_text(&mut self, text: &str) {
        if SystemClipboard.set_text(text).is_err() {
            self.status = self.t("misc.clipboard_failed").into();
        } else {
            self.status = self.t("misc.path_copied").into();
        }
    }

    pub(crate) fn draw_explorer_context_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.explorer_context_menu else {
            return;
        };
        let (x, y, width, height) = menu_rect(menu);
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let char_width = renderer.char_width().max(4.0);

        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, bg);

        let mut top = y + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            let row_h = row_height(row);
            match row {
                ExplorerContextMenuRow::Separator => {
                    renderer.draw_rect(x + 1.0, top + 4.0, width - 2.0, 1.0, border);
                }
                ExplorerContextMenuRow::Action {
                    label_key,
                    shortcut,
                    enabled,
                    ..
                } => {
                    if *enabled && menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    let fg = if *enabled { text } else { dim };
                    renderer.draw_text(self.t(label_key), x + 14.0, top + 4.0, fg);
                    if !shortcut.is_empty() {
                        let sx = x + width - 14.0 - shortcut.chars().count() as f32 * char_width;
                        renderer.draw_text(shortcut, sx, top + 4.0, dim);
                    }
                }
            }
            top += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn temp_app() -> (TempDir, App) {
        let dir = TempDir::new();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tree.refresh();
        (dir, app)
    }

    #[test]
    fn root_context_menu_includes_folder_and_workspace_actions() {
        let (dir, mut app) = temp_app();
        app.open_explorer_context_menu(
            dir.path().to_path_buf(),
            (40.0, 40.0),
            (1200.0, 760.0),
            18.0,
            8.0,
        );
        let menu = app.explorer_context_menu.as_ref().expect("menu opened");
        assert!(menu.is_root);
        assert!(menu.is_dir);
        let actions: Vec<ExplorerContextAction> = menu
            .rows
            .iter()
            .filter_map(|row| match row {
                ExplorerContextMenuRow::Action { action, .. } => Some(*action),
                ExplorerContextMenuRow::Separator => None,
            })
            .collect();
        assert!(actions.contains(&ExplorerContextAction::NewFile));
        assert!(actions.contains(&ExplorerContextAction::NewFolder));
        assert!(actions.contains(&ExplorerContextAction::RevealInFileExplorer));
        assert!(actions.contains(&ExplorerContextAction::OpenInTerminal));
        assert!(actions.contains(&ExplorerContextAction::AddFolderToWorkspace));
        assert!(actions.contains(&ExplorerContextAction::OpenFolderSettings));
        assert!(actions.contains(&ExplorerContextAction::RemoveFolderFromWorkspace));
        assert!(actions.contains(&ExplorerContextAction::FindInFolder));
        assert!(actions.contains(&ExplorerContextAction::OpenTimeline));
        assert!(actions.contains(&ExplorerContextAction::AddToChat));
        assert!(actions.contains(&ExplorerContextAction::Paste));
        assert!(actions.contains(&ExplorerContextAction::CopyPath));
        assert!(actions.contains(&ExplorerContextAction::CopyRelativePath));
        assert!(!actions.contains(&ExplorerContextAction::Rename));
        assert!(!actions.contains(&ExplorerContextAction::Delete));
    }

    #[test]
    fn file_context_menu_includes_edit_and_file_actions() {
        let (dir, mut app) = temp_app();
        let file = dir.path().join("src/main.rs");
        app.open_explorer_context_menu(file, (40.0, 80.0), (1200.0, 760.0), 18.0, 8.0);
        let menu = app.explorer_context_menu.as_ref().expect("menu opened");
        assert!(!menu.is_root);
        assert!(!menu.is_dir);
        let actions: Vec<ExplorerContextAction> = menu
            .rows
            .iter()
            .filter_map(|row| match row {
                ExplorerContextMenuRow::Action { action, .. } => Some(*action),
                ExplorerContextMenuRow::Separator => None,
            })
            .collect();
        assert!(actions.contains(&ExplorerContextAction::Cut));
        assert!(actions.contains(&ExplorerContextAction::Copy));
        assert!(actions.contains(&ExplorerContextAction::Rename));
        assert!(actions.contains(&ExplorerContextAction::Delete));
    }

    #[test]
    fn find_in_folder_and_add_to_chat_and_folder_settings_work() {
        let (dir, mut app) = temp_app();
        let src = dir.path().join("src");
        app.run_explorer_context_action(&src, true, ExplorerContextAction::FindInFolder);
        assert!(app.search_query.is_some());
        assert!(app.search_filters.details);
        assert_eq!(app.search_filters.include, "src");

        app.run_explorer_context_action(&src, true, ExplorerContextAction::AddToChat);
        assert!(app.chat.visible);
        assert!(app.chat.input.contains("@src"));

        app.run_explorer_context_action(&src, true, ExplorerContextAction::OpenFolderSettings);
        assert!(src.join(".vscode/settings.json").is_file());
    }

    #[test]
    fn escape_dismisses_explorer_context_menu() {
        let (dir, mut app) = temp_app();
        app.open_explorer_context_menu(
            dir.path().to_path_buf(),
            (40.0, 40.0),
            (1200.0, 760.0),
            18.0,
            8.0,
        );
        assert!(app.explorer_context_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.explorer_context_menu.is_none());
    }
}
