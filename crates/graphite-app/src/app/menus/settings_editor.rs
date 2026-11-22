//! Interactive VS Code-style Settings Editor modal with collapsible category tree.

use super::*;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use winit::keyboard::{Key, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsScope {
    User,
    Workspace,
}

#[derive(Debug, Clone, Copy)]
enum SettingKind {
    Bool,
    Enum(&'static [&'static str]),
    Number,
}

#[derive(Debug, Clone, Copy)]
struct SettingDef {
    key: &'static str,
    category: &'static str,
    title: &'static str,
    description: &'static str,
    kind: SettingKind,
    default_value: &'static str,
    commonly_used: bool,
}

#[derive(Debug, Clone, Copy)]
struct SubcategoryDef {
    id: &'static str,
    label: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct CategoryDef {
    id: &'static str,
    label: &'static str,
    subcategories: &'static [SubcategoryDef],
}

#[derive(Debug, Clone, Copy)]
struct SettingsRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl SettingsRect {
    fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SettingsEditor {
    pub scope: SettingsScope,
    pub expanded_categories: BTreeSet<&'static str>,
    pub selected_category: &'static str,
    pub search_query: String,
    pub search_focused: bool,
    pub editing_key: Option<(&'static str, String)>,
    pub sidebar_scroll: usize,
    pub content_scroll: usize,
}

impl SettingsEditor {
    pub(crate) fn new(scope: SettingsScope) -> Self {
        Self {
            scope,
            expanded_categories: BTreeSet::new(),
            selected_category: "commonly_used",
            search_query: String::new(),
            search_focused: false,
            editing_key: None,
            sidebar_scroll: 0,
            content_scroll: 0,
        }
    }
}

const CATEGORIES: &[CategoryDef] = &[
    CategoryDef {
        id: "commonly_used",
        label: "Commonly Used",
        subcategories: &[],
    },
    CategoryDef {
        id: "text_editor",
        label: "Text Editor",
        subcategories: &[
            SubcategoryDef {
                id: "text_editor.cursor",
                label: "Cursor",
            },
            SubcategoryDef {
                id: "text_editor.find",
                label: "Find",
            },
            SubcategoryDef {
                id: "text_editor.font",
                label: "Font",
            },
            SubcategoryDef {
                id: "text_editor.formatting",
                label: "Formatting",
            },
            SubcategoryDef {
                id: "text_editor.diff_editor",
                label: "Diff Editor",
            },
            SubcategoryDef {
                id: "text_editor.multi_diff",
                label: "Multi-File Diff Editor",
            },
            SubcategoryDef {
                id: "text_editor.minimap",
                label: "Minimap",
            },
            SubcategoryDef {
                id: "text_editor.suggestions",
                label: "Suggestions",
            },
            SubcategoryDef {
                id: "text_editor.files",
                label: "Files",
            },
        ],
    },
    CategoryDef {
        id: "workbench",
        label: "Workbench",
        subcategories: &[
            SubcategoryDef {
                id: "workbench.appearance",
                label: "Appearance",
            },
            SubcategoryDef {
                id: "workbench.breadcrumbs",
                label: "Breadcrumbs",
            },
            SubcategoryDef {
                id: "workbench.editor_management",
                label: "Editor Management",
            },
            SubcategoryDef {
                id: "workbench.settings_editor",
                label: "Settings Editor",
            },
            SubcategoryDef {
                id: "workbench.zen_mode",
                label: "Zen Mode",
            },
            SubcategoryDef {
                id: "workbench.screencast",
                label: "Screencast Mode",
            },
            SubcategoryDef {
                id: "workbench.browser",
                label: "Browser",
            },
        ],
    },
    CategoryDef {
        id: "window",
        label: "Window",
        subcategories: &[SubcategoryDef {
            id: "window.new_window",
            label: "New Window",
        }],
    },
    CategoryDef {
        id: "chat",
        label: "Chat",
        subcategories: &[
            SubcategoryDef {
                id: "chat.agent",
                label: "Agent",
            },
            SubcategoryDef {
                id: "chat.appearance",
                label: "Appearance",
            },
            SubcategoryDef {
                id: "chat.sessions",
                label: "Sessions",
            },
            SubcategoryDef {
                id: "chat.tools",
                label: "Tools",
            },
            SubcategoryDef {
                id: "chat.mcp",
                label: "MCP",
            },
            SubcategoryDef {
                id: "chat.context",
                label: "Context",
            },
            SubcategoryDef {
                id: "chat.inline_chat",
                label: "Inline Chat",
            },
            SubcategoryDef {
                id: "chat.misc",
                label: "Miscellaneous",
            },
        ],
    },
    CategoryDef {
        id: "features",
        label: "Features",
        subcategories: &[
            SubcategoryDef {
                id: "features.accessibility_signals",
                label: "Accessibility Signals",
            },
            SubcategoryDef {
                id: "features.accessibility",
                label: "Accessibility",
            },
            SubcategoryDef {
                id: "features.explorer",
                label: "Explorer",
            },
            SubcategoryDef {
                id: "features.search",
                label: "Search",
            },
            SubcategoryDef {
                id: "features.debug",
                label: "Debug",
            },
            SubcategoryDef {
                id: "features.testing",
                label: "Testing",
            },
            SubcategoryDef {
                id: "features.scm",
                label: "Source Control",
            },
            SubcategoryDef {
                id: "features.extensions",
                label: "Extensions",
            },
            SubcategoryDef {
                id: "features.terminal",
                label: "Terminal",
            },
            SubcategoryDef {
                id: "features.task",
                label: "Task",
            },
            SubcategoryDef {
                id: "features.problems",
                label: "Problems",
            },
            SubcategoryDef {
                id: "features.output",
                label: "Output",
            },
            SubcategoryDef {
                id: "features.comments",
                label: "Comments",
            },
            SubcategoryDef {
                id: "features.remote",
                label: "Remote",
            },
            SubcategoryDef {
                id: "features.timeline",
                label: "Timeline",
            },
            SubcategoryDef {
                id: "features.notebook",
                label: "Notebook",
            },
            SubcategoryDef {
                id: "features.merge_editor",
                label: "Merge Editor",
            },
        ],
    },
    CategoryDef {
        id: "application",
        label: "Application",
        subcategories: &[
            SubcategoryDef {
                id: "application.proxy",
                label: "Proxy",
            },
            SubcategoryDef {
                id: "application.keyboard",
                label: "Keyboard",
            },
            SubcategoryDef {
                id: "application.update",
                label: "Update",
            },
            SubcategoryDef {
                id: "application.telemetry",
                label: "Telemetry",
            },
            SubcategoryDef {
                id: "application.sync",
                label: "Settings Sync",
            },
            SubcategoryDef {
                id: "application.network",
                label: "Network",
            },
            SubcategoryDef {
                id: "application.experimental",
                label: "Experimental",
            },
        ],
    },
    CategoryDef {
        id: "security",
        label: "Security",
        subcategories: &[SubcategoryDef {
            id: "security.workspace",
            label: "Workspace",
        }],
    },
    CategoryDef {
        id: "extensions",
        label: "Extensions",
        subcategories: &[SubcategoryDef {
            id: "extensions.installed",
            label: "Installed Extensions",
        }],
    },
];

const SETTINGS: &[SettingDef] = &[
    SettingDef {
        key: "editor.fontSize",
        category: "text_editor.font",
        title: "Editor: Font Size",
        description: "Controls the font size in pixels.",
        kind: SettingKind::Number,
        default_value: "14",
        commonly_used: true,
    },
    SettingDef {
        key: "editor.formatOnSave",
        category: "text_editor.formatting",
        title: "Editor: Format On Save",
        description: "Format a file on save when a formatter is available.",
        kind: SettingKind::Bool,
        default_value: "false",
        commonly_used: true,
    },
    SettingDef {
        key: "files.autoSave",
        category: "text_editor.files",
        title: "Files: Auto Save",
        description: "Controls auto save of editors that have unsaved changes.",
        kind: SettingKind::Enum(&["off", "afterDelay", "onFocusChange", "onWindowChange"]),
        default_value: "off",
        commonly_used: true,
    },
    SettingDef {
        key: crate::app::draft_recovery::SETTING,
        category: "text_editor.files",
        title: "Files: Draft Recovery",
        description: "Keep local copies of unsaved text for recovery after an unexpected exit.",
        kind: SettingKind::Bool,
        default_value: "true",
        commonly_used: true,
    },
    SettingDef {
        key: "files.autoSaveDelay",
        category: "text_editor.files",
        title: "Files: Auto Save Delay",
        description: "Controls the delay in ms after which a dirty editor is saved.",
        kind: SettingKind::Number,
        default_value: "1000",
        commonly_used: false,
    },
    SettingDef {
        key: "editor.tabSize",
        category: "text_editor",
        title: "Editor: Tab Size",
        description: "The number of spaces a tab is equal to.",
        kind: SettingKind::Number,
        default_value: "4",
        commonly_used: true,
    },
    SettingDef {
        key: "editor.insertSpaces",
        category: "text_editor",
        title: "Editor: Insert Spaces",
        description: "Insert spaces when pressing Tab.",
        kind: SettingKind::Bool,
        default_value: "true",
        commonly_used: false,
    },
    SettingDef {
        key: "editor.detectIndentation",
        category: "text_editor",
        title: "Editor: Detect Indentation",
        description: "Detect tabSize and insertSpaces based on file contents.",
        kind: SettingKind::Bool,
        default_value: "true",
        commonly_used: false,
    },
    SettingDef {
        key: "editor.wordWrap",
        category: "text_editor",
        title: "Editor: Word Wrap",
        description: "Controls how lines should wrap.",
        kind: SettingKind::Enum(&["off", "on", "wordWrapColumn", "bounded"]),
        default_value: "off",
        commonly_used: true,
    },
    SettingDef {
        key: "editor.wrappingIndent",
        category: "text_editor",
        title: "Editor: Wrapping Indent",
        description: "Controls the indentation of wrapped lines.",
        kind: SettingKind::Enum(&["none", "same", "indent", "deepIndent"]),
        default_value: "same",
        commonly_used: false,
    },
    SettingDef {
        key: "editor.formatOnPaste",
        category: "text_editor.formatting",
        title: "Editor: Format On Paste",
        description: "Controls whether the editor automatically formats pasted content.",
        kind: SettingKind::Bool,
        default_value: "false",
        commonly_used: false,
    },
    SettingDef {
        key: "editor.formatOnType",
        category: "text_editor.formatting",
        title: "Editor: Format On Type",
        description: "Controls whether the editor formats the line after typing.",
        kind: SettingKind::Bool,
        default_value: "false",
        commonly_used: false,
    },
    SettingDef {
        key: "files.trimTrailingWhitespace",
        category: "text_editor.files",
        title: "Files: Trim Trailing Whitespace",
        description: "When enabled, trims trailing whitespace when saving a file.",
        kind: SettingKind::Bool,
        default_value: "false",
        commonly_used: false,
    },
    SettingDef {
        key: "workbench.colorTheme",
        category: "workbench.appearance",
        title: "Workbench: Color Theme",
        description: "Specifies the color theme used in the workbench.",
        kind: SettingKind::Enum(&["Default Dark+", "Default Light+"]),
        default_value: "Default Dark+",
        commonly_used: true,
    },
    SettingDef {
        key: "workbench.language",
        category: "workbench.appearance",
        title: "Workbench: Language",
        description: "Controls the display language of Graphite.",
        kind: SettingKind::Enum(&["en", "pt-BR"]),
        default_value: "en",
        commonly_used: false,
    },
    SettingDef {
        key: "workbench.secondarySidebar.tabStyle",
        category: "workbench.appearance",
        title: "Workbench > Secondary Side Bar: Tab Style",
        description: "Controls whether Secondary Side Bar header tabs display text, icons, or both.",
        kind: SettingKind::Enum(&["text", "icons", "iconsAndText"]),
        default_value: "text",
        commonly_used: false,
    },
    SettingDef {
        key: "update.channel",
        category: "application.update",
        title: "Update: Channel",
        description: "Configure whether you receive stable or prerelease updates.",
        kind: SettingKind::Enum(&["stable", "prerelease"]),
        default_value: "stable",
        commonly_used: false,
    },
    SettingDef {
        key: "security.workspace.trust.enabled",
        category: "security.workspace",
        title: "Security > Workspace: Trust Enabled",
        description: "Controls whether Workspace Trust is enabled.",
        kind: SettingKind::Bool,
        default_value: "true",
        commonly_used: false,
    },
];

#[derive(Debug, Clone)]
struct SidebarRow {
    id: &'static str,
    label: &'static str,
    is_parent: bool,
    has_children: bool,
    expanded: bool,
}

fn visible_sidebar_rows(expanded: &BTreeSet<&'static str>) -> Vec<SidebarRow> {
    let mut rows = Vec::new();
    for cat in CATEGORIES {
        let is_expanded = expanded.contains(cat.id);
        rows.push(SidebarRow {
            id: cat.id,
            label: cat.label,
            is_parent: true,
            has_children: !cat.subcategories.is_empty(),
            expanded: is_expanded,
        });
        if is_expanded {
            for sub in cat.subcategories {
                rows.push(SidebarRow {
                    id: sub.id,
                    label: sub.label,
                    is_parent: false,
                    has_children: false,
                    expanded: false,
                });
            }
        }
    }
    rows
}

fn filtered_settings(selected_category: &str, search_query: &str) -> Vec<&'static SettingDef> {
    let query = search_query.trim().to_ascii_lowercase();
    SETTINGS
        .iter()
        .filter(|s| {
            if !query.is_empty() {
                return s.key.to_ascii_lowercase().contains(&query)
                    || s.title.to_ascii_lowercase().contains(&query)
                    || s.description.to_ascii_lowercase().contains(&query);
            }
            if selected_category == "commonly_used" {
                s.commonly_used
            } else {
                s.category == selected_category
                    || s.category.starts_with(&format!("{selected_category}."))
            }
        })
        .collect()
}

fn category_heading(id: &str) -> &'static str {
    for cat in CATEGORIES {
        if cat.id == id {
            return cat.label;
        }
        for sub in cat.subcategories {
            if sub.id == id {
                return sub.label;
            }
        }
    }
    "Commonly Used"
}

fn modal_bounds(w: f32, h: f32) -> SettingsRect {
    let mw = (w - 80.0).clamp(520.0, 980.0);
    let mh = (h - 72.0).clamp(400.0, 680.0);
    SettingsRect {
        x: ((w - mw) * 0.5).max(16.0),
        y: ((h - mh) * 0.5).max(16.0),
        w: mw,
        h: mh,
    }
}

impl App {
    pub(crate) fn open_settings_editor(&mut self, scope: SettingsScope) {
        self.settings_menu = None;
        self.workbench_menu = None;
        self.settings_editor = Some(SettingsEditor::new(scope));
        self.status = self.t("settings_editor.opened").to_owned();
    }

    fn settings_file_path(&self, scope: SettingsScope) -> PathBuf {
        match scope {
            SettingsScope::User => self.profile_root.join("settings.json"),
            SettingsScope::Workspace => self.tree.root().join(".vscode").join("settings.json"),
        }
    }

    fn read_scope_settings_map(&self, scope: SettingsScope) -> Map<String, Value> {
        let path = self.settings_file_path(scope);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Map::new();
        };
        crate::config::parse_jsonc(&raw)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    fn setting_display_value(&self, scope: SettingsScope, def: &SettingDef) -> String {
        let map = self.read_scope_settings_map(scope);
        if let Some(v) = map.get(def.key) {
            return match v {
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
        }
        match def.key {
            "editor.fontSize" => format!("{}", self.font_size.round() as u32),
            "editor.tabSize" => format!("{}", self.tab_size),
            "editor.insertSpaces" => self.insert_spaces.to_string(),
            "update.channel" => self.updates.config.channel.label().to_owned(),
            _ => def.default_value.to_owned(),
        }
    }

    fn write_scope_setting(&mut self, scope: SettingsScope, key: &str, value: Value) {
        if scope == SettingsScope::Workspace && key == crate::app::draft_recovery::SETTING {
            return;
        }
        let path = self.settings_file_path(scope);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut map = self.read_scope_settings_map(scope);
        map.insert(key.to_owned(), value.clone());
        if let Ok(pretty) = serde_json::to_string_pretty(&Value::Object(map)) {
            if let Err(err) = std::fs::write(&path, format!("{pretty}\n")) {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::High,
                    "settings_editor.save",
                    err.to_string(),
                );
                self.status = err.to_string();
                return;
            }
        }
        self.apply_live_setting(key, &value);
        self.status = format!("Updated {key}");
    }

    fn apply_live_setting(&mut self, key: &str, value: &Value) {
        match key {
            crate::app::draft_recovery::SETTING => {
                if let Some(enabled) = value.as_bool() {
                    self.drafts.set_enabled(&self.profile_root, enabled);
                }
            }
            "editor.fontSize" => {
                if let Some(n) = value.as_f64() {
                    self.font_size = (n as f32).clamp(8.0, 48.0);
                }
            }
            "editor.tabSize" => {
                if let Some(n) = value.as_u64() {
                    self.tab_size = (n as usize).clamp(1, 16);
                }
            }
            "editor.insertSpaces" => {
                if let Some(b) = value.as_bool() {
                    self.insert_spaces = b;
                }
            }
            "update.channel" => {
                if let Some(s) = value.as_str() {
                    if let Some(ch) = crate::app::updates::state::UpdateChannel::from_str_lossy(s) {
                        self.set_update_channel(ch);
                    }
                }
            }
            "workbench.secondarySidebar.tabStyle" => {
                if let Some(s) = value.as_str() {
                    if let Some(style) = crate::config::SecondaryTabStyle::from_setting_str(s) {
                        self.activity_bar_preferences.secondary_tab_style = style;
                        self.save_activity_bar_preferences();
                    }
                }
            }
            _ => {}
        }
    }

    fn commit_inline_setting_edit(&mut self) {
        let Some(editor) = self.settings_editor.as_mut() else {
            return;
        };
        let Some((key, raw)) = editor.editing_key.take() else {
            return;
        };
        let scope = editor.scope;
        let Some(def) = SETTINGS.iter().find(|s| s.key == key) else {
            return;
        };
        let value = match def.kind {
            SettingKind::Number => match raw.trim().parse::<u64>() {
                Ok(n) => Value::Number(n.into()),
                Err(_) => Value::Number(def.default_value.parse::<u64>().unwrap_or(14).into()),
            },
            _ => Value::String(raw),
        };
        self.write_scope_setting(scope, key, value);
    }

    pub(crate) fn settings_editor_scroll(&mut self, delta_y: f32) -> bool {
        let Some(editor) = self.settings_editor.as_mut() else {
            return false;
        };
        let Some((w, h)) = self.gpu.as_ref().map(|g| {
            let (w, h) = g.renderer.size();
            (w, h)
        }) else {
            return false;
        };
        let modal = modal_bounds(w, h);
        let mx = self.mouse.x as f32;
        let my = self.mouse.y as f32;
        if !modal.contains(mx, my) {
            return true;
        }
        let step = if delta_y > 0.0 { 1 } else { 0 };
        if mx < modal.x + 210.0 {
            if step > 0 {
                editor.sidebar_scroll = editor.sidebar_scroll.saturating_add(1);
            } else {
                editor.sidebar_scroll = editor.sidebar_scroll.saturating_sub(1);
            }
        } else if step > 0 {
            editor.content_scroll = editor.content_scroll.saturating_add(1);
        } else {
            editor.content_scroll = editor.content_scroll.saturating_sub(1);
        }
        true
    }

    pub(crate) fn settings_editor_key(&mut self, key: &Key, text: Option<&str>) -> bool {
        if self.settings_editor.is_none() {
            return false;
        }
        match key {
            Key::Named(NamedKey::Escape) => {
                if self
                    .settings_editor
                    .as_ref()
                    .is_some_and(|e| e.editing_key.is_some())
                {
                    if let Some(e) = self.settings_editor.as_mut() {
                        e.editing_key = None;
                    }
                } else {
                    self.settings_editor = None;
                }
                return true;
            }
            Key::Named(NamedKey::Enter) => {
                self.commit_inline_setting_edit();
                return true;
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(editor) = self.settings_editor.as_mut() {
                    if let Some((_, buf)) = editor.editing_key.as_mut() {
                        buf.pop();
                    } else if editor.search_focused {
                        editor.search_query.pop();
                        editor.content_scroll = 0;
                    }
                }
                return true;
            }
            _ => {}
        }
        if let Some(ch) = text {
            if !ch.chars().any(|c| c.is_control()) {
                if let Some(editor) = self.settings_editor.as_mut() {
                    if let Some((_, buf)) = editor.editing_key.as_mut() {
                        buf.push_str(ch);
                    } else if editor.search_focused {
                        editor.search_query.push_str(ch);
                        editor.content_scroll = 0;
                    }
                }
            }
        }
        true
    }

    pub(crate) fn settings_editor_click(&mut self, x: f32, y: f32) -> bool {
        let Some(editor_snapshot) = self.settings_editor.clone() else {
            return false;
        };
        let Some((w, h, lh)) = self.gpu.as_ref().map(|g| {
            let (w, h) = g.renderer.size();
            (w, h, g.renderer.line_height())
        }) else {
            return false;
        };
        let modal = modal_bounds(w, h);
        if !modal.contains(x, y) {
            self.commit_inline_setting_edit();
            self.settings_editor = None;
            return true;
        }

        let close_rect = SettingsRect {
            x: modal.x + modal.w - 34.0,
            y: modal.y + 10.0,
            w: 24.0,
            h: 24.0,
        };
        if close_rect.contains(x, y) {
            self.commit_inline_setting_edit();
            self.settings_editor = None;
            return true;
        }

        let open_json_rect = SettingsRect {
            x: modal.x + modal.w - 196.0,
            y: modal.y + 10.0,
            w: 152.0,
            h: 24.0,
        };
        if open_json_rect.contains(x, y) {
            self.commit_inline_setting_edit();
            let scope = editor_snapshot.scope;
            self.settings_editor = None;
            match scope {
                SettingsScope::User => self.open_preference_file("settings.json"),
                SettingsScope::Workspace => self.open_workspace_settings_file(),
            }
            return true;
        }

        let search_rect = SettingsRect {
            x: modal.x + 16.0,
            y: modal.y + 42.0,
            w: modal.w - 32.0,
            h: (lh + 10.0).max(28.0),
        };
        if search_rect.contains(x, y) {
            self.commit_inline_setting_edit();
            if let Some(e) = self.settings_editor.as_mut() {
                e.search_focused = true;
            }
            return true;
        }

        let tabs_y = search_rect.y + search_rect.h + 10.0;
        let user_tab = SettingsRect {
            x: modal.x + 16.0,
            y: tabs_y,
            w: 84.0,
            h: 26.0,
        };
        let workspace_tab = SettingsRect {
            x: user_tab.x + user_tab.w + 8.0,
            y: tabs_y,
            w: 112.0,
            h: 26.0,
        };
        if user_tab.contains(x, y) {
            self.commit_inline_setting_edit();
            if let Some(e) = self.settings_editor.as_mut() {
                e.scope = SettingsScope::User;
                e.search_focused = false;
            }
            return true;
        }
        if workspace_tab.contains(x, y) {
            self.commit_inline_setting_edit();
            if let Some(e) = self.settings_editor.as_mut() {
                e.scope = SettingsScope::Workspace;
                e.search_focused = false;
            }
            return true;
        }

        let body_y = tabs_y + 34.0;
        let sidebar_w = 210.0;
        let row_h = (lh + 6.0).max(24.0);

        if x < modal.x + sidebar_w && y >= body_y {
            self.commit_inline_setting_edit();
            let rows = visible_sidebar_rows(&editor_snapshot.expanded_categories);
            let idx = ((y - body_y) / row_h).floor() as usize + editor_snapshot.sidebar_scroll;
            if let Some(row) = rows.get(idx) {
                if let Some(e) = self.settings_editor.as_mut() {
                    e.search_focused = false;
                    e.selected_category = row.id;
                    e.content_scroll = 0;
                    if row.has_children {
                        if e.expanded_categories.contains(row.id) {
                            e.expanded_categories.remove(row.id);
                        } else {
                            e.expanded_categories.insert(row.id);
                        }
                    }
                }
            }
            return true;
        }

        let content_x = modal.x + sidebar_w + 20.0;
        let content_w = modal.w - sidebar_w - 36.0;
        let list_y = body_y + lh + 18.0;
        if x >= content_x && y >= list_y {
            let items = filtered_settings(
                editor_snapshot.selected_category,
                &editor_snapshot.search_query,
            )
            .into_iter()
            .filter(|def| {
                editor_snapshot.scope == SettingsScope::User
                    || def.key != crate::app::draft_recovery::SETTING
            })
            .collect::<Vec<_>>();
            let card_h = (lh * 3.4).max(74.0);
            let idx = ((y - list_y) / card_h).floor() as usize + editor_snapshot.content_scroll;
            if let Some(def) = items.get(idx) {
                let card_y = list_y + (idx - editor_snapshot.content_scroll) as f32 * card_h;
                let control_rect = SettingsRect {
                    x: content_x,
                    y: card_y + lh * 2.0 + 2.0,
                    w: content_w.min(260.0),
                    h: (lh + 6.0).max(24.0),
                };
                if control_rect.contains(x, y) {
                    let current = self.setting_display_value(editor_snapshot.scope, def);
                    match def.kind {
                        SettingKind::Bool => {
                            let next = current != "true";
                            self.write_scope_setting(
                                editor_snapshot.scope,
                                def.key,
                                Value::Bool(next),
                            );
                        }
                        SettingKind::Enum(options) => {
                            let pos = options.iter().position(|o| *o == current).unwrap_or(0);
                            let next = options[(pos + 1) % options.len()];
                            self.write_scope_setting(
                                editor_snapshot.scope,
                                def.key,
                                Value::String(next.to_owned()),
                            );
                        }
                        SettingKind::Number => {
                            if let Some(e) = self.settings_editor.as_mut() {
                                e.search_focused = false;
                                e.editing_key = Some((def.key, current));
                            }
                        }
                    }
                    return true;
                }
            }
        }

        if let Some(e) = self.settings_editor.as_mut() {
            e.search_focused = false;
        }
        true
    }

    pub(crate) fn draw_settings_editor(&self, renderer: &mut Renderer) {
        let Some(editor) = self.settings_editor.as_ref() else {
            return;
        };
        let (w, h) = renderer.size();
        let lh = renderer.line_height();
        let modal = modal_bounds(w, h);

        let bg = rgb(Theme::color(Theme::BG, 0xffffff));
        let sidebar_bg = rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3));
        let border = rgb(Theme::color(Theme::ACTIVE_ROW, 0xd4d4d4));
        let active_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0xe4e6f1));
        let accent = rgb(Theme::color(Theme::STATUS, 0x005a9e));
        let fg = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));

        renderer.draw_rect(modal.x, modal.y, modal.w, modal.h, border);
        renderer.draw_rect(
            modal.x + 1.0,
            modal.y + 1.0,
            modal.w - 2.0,
            modal.h - 2.0,
            bg,
        );

        renderer.draw_text(
            self.t("settings_editor.title"),
            modal.x + 16.0,
            modal.y + 12.0,
            fg,
        );
        renderer.draw_rect(
            modal.x + modal.w - 196.0,
            modal.y + 10.0,
            152.0,
            24.0,
            active_bg,
        );
        renderer.draw_text(
            self.t("settings_editor.open_json"),
            modal.x + modal.w - 188.0,
            modal.y + 14.0,
            fg,
        );
        renderer.draw_text("x", modal.x + modal.w - 26.0, modal.y + 12.0, dim);

        let search_y = modal.y + 42.0;
        let search_h = (lh + 10.0).max(28.0);
        renderer.draw_rect(modal.x + 16.0, search_y, modal.w - 32.0, search_h, border);
        renderer.draw_rect(
            modal.x + 17.0,
            search_y + 1.0,
            modal.w - 34.0,
            search_h - 2.0,
            sidebar_bg,
        );
        let search_text = if editor.search_query.is_empty() {
            self.t("settings_editor.search_placeholder").to_owned()
        } else {
            editor.search_query.clone()
        };
        let search_color = if editor.search_query.is_empty() {
            dim
        } else {
            fg
        };
        renderer.draw_text(&search_text, modal.x + 24.0, search_y + 6.0, search_color);

        let tabs_y = search_y + search_h + 10.0;
        let user_bg = if editor.scope == SettingsScope::User {
            active_bg
        } else {
            bg
        };
        let ws_bg = if editor.scope == SettingsScope::Workspace {
            active_bg
        } else {
            bg
        };
        renderer.draw_rect(modal.x + 16.0, tabs_y, 84.0, 26.0, user_bg);
        renderer.draw_text(
            self.t("settings_editor.scope_user"),
            modal.x + 28.0,
            tabs_y + 5.0,
            fg,
        );
        renderer.draw_rect(modal.x + 108.0, tabs_y, 112.0, 26.0, ws_bg);
        renderer.draw_text(
            self.t("settings_editor.scope_workspace"),
            modal.x + 118.0,
            tabs_y + 5.0,
            fg,
        );
        renderer.draw_rect(modal.x + 1.0, tabs_y + 30.0, modal.w - 2.0, 1.0, border);

        let body_y = tabs_y + 34.0;
        let sidebar_w = 210.0;
        renderer.draw_rect(
            modal.x + 1.0,
            body_y - 3.0,
            sidebar_w,
            modal.y + modal.h - body_y + 2.0,
            sidebar_bg,
        );

        let row_h = (lh + 6.0).max(24.0);
        let max_rows = ((modal.y + modal.h - body_y - 8.0) / row_h)
            .floor()
            .max(1.0) as usize;
        for (i, row) in visible_sidebar_rows(&editor.expanded_categories)
            .into_iter()
            .skip(editor.sidebar_scroll)
            .take(max_rows)
            .enumerate()
        {
            let ry = body_y + i as f32 * row_h;
            if row.id == editor.selected_category {
                renderer.draw_rect(modal.x + 2.0, ry, sidebar_w - 4.0, row_h - 2.0, active_bg);
            }
            let prefix = if !row.is_parent {
                "   "
            } else if !row.has_children {
                "  "
            } else if row.expanded {
                "v "
            } else {
                "> "
            };
            renderer.draw_text(
                &format!("{prefix}{}", row.label),
                modal.x + 10.0,
                ry + 3.0,
                if row.id == editor.selected_category {
                    accent
                } else {
                    fg
                },
            );
        }

        let content_x = modal.x + sidebar_w + 20.0;
        let heading = if editor.search_query.trim().is_empty() {
            category_heading(editor.selected_category)
        } else {
            "Search Results"
        };
        renderer.draw_text(heading, content_x, body_y + 2.0, fg);

        let list_y = body_y + lh + 18.0;
        let card_h = (lh * 3.4).max(74.0);
        let max_cards = ((modal.y + modal.h - list_y - 8.0) / card_h)
            .floor()
            .max(1.0) as usize;
        for (i, def) in filtered_settings(editor.selected_category, &editor.search_query)
            .into_iter()
            .filter(|def| {
                editor.scope == SettingsScope::User
                    || def.key != crate::app::draft_recovery::SETTING
            })
            .skip(editor.content_scroll)
            .take(max_cards)
            .enumerate()
        {
            let cy = list_y + i as f32 * card_h;
            let (title, description) = if def.key == crate::app::draft_recovery::SETTING {
                (
                    self.t("drafts.setting_title"),
                    self.t("drafts.setting_description"),
                )
            } else {
                (def.title, def.description)
            };
            renderer.draw_text(title, content_x, cy, fg);
            renderer.draw_text(description, content_x, cy + lh, dim);

            let val = if let Some((k, buf)) = editor.editing_key.as_ref() {
                if *k == def.key {
                    format!("{buf}|")
                } else {
                    self.setting_display_value(editor.scope, def)
                }
            } else {
                self.setting_display_value(editor.scope, def)
            };
            let box_y = cy + lh * 2.0 + 2.0;
            let box_w = (modal.w - sidebar_w - 36.0).min(260.0);
            let box_h = (lh + 6.0).max(24.0);
            renderer.draw_rect(content_x, box_y, box_w, box_h, active_bg);
            let display = match def.kind {
                SettingKind::Bool => {
                    if val == "true" {
                        "[x] Enabled".to_owned()
                    } else {
                        "[ ] Disabled".to_owned()
                    }
                }
                SettingKind::Enum(_) => format!("{val}  v"),
                SettingKind::Number => val,
            };
            renderer.draw_text(&display, content_x + 8.0, box_y + 3.0, fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_tree_starts_collapsed_and_expands_on_toggle() {
        let mut expanded = BTreeSet::new();
        let collapsed = visible_sidebar_rows(&expanded);
        assert_eq!(collapsed.len(), CATEGORIES.len());
        assert!(!collapsed.iter().any(|r| r.id == "text_editor.font"));

        expanded.insert("text_editor");
        let with_text_editor = visible_sidebar_rows(&expanded);
        assert!(with_text_editor.iter().any(|r| r.id == "text_editor.font"));
        assert!(with_text_editor
            .iter()
            .any(|r| r.id == "text_editor.formatting"));
    }

    #[test]
    fn search_filters_settings_across_all_categories() {
        let results = filtered_settings("commonly_used", "channel");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "update.channel");
    }

    #[test]
    fn writing_setting_persists_to_user_and_workspace_json_and_updates_app() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().join("profile");
        std::fs::create_dir_all(&app.profile_root).unwrap();

        app.write_scope_setting(
            SettingsScope::User,
            "editor.fontSize",
            Value::Number(18.into()),
        );
        assert_eq!(app.font_size, 18.0);
        let user_raw = std::fs::read_to_string(app.profile_root.join("settings.json")).unwrap();
        assert!(user_raw.contains("\"editor.fontSize\": 18"));

        app.write_scope_setting(
            SettingsScope::Workspace,
            "editor.formatOnSave",
            Value::Bool(true),
        );
        let ws_raw =
            std::fs::read_to_string(app.tree.root().join(".vscode").join("settings.json")).unwrap();
        assert!(ws_raw.contains("\"editor.formatOnSave\": true"));
    }
}
