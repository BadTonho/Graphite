use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Runtime execution engine for a native Graphite plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    #[default]
    Node,
    Native,
}

fn is_default_runtime(runtime: &PluginRuntime) -> bool {
    *runtime == PluginRuntime::Node
}

/// Canonical capability tokens supported in `permissions` of `graphite-plugin.json`.
pub const VALID_PERMISSIONS: &[&str] = &[
    "commands",
    "documents.read",
    "documents.write",
    "ui",
    "languages",
    "storage",
    "workspace.read",
    "workspace.write",
    "terminal",
    "chat",
    "*",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub api_version: u32,
    pub main: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "is_default_runtime")]
    pub runtime: PluginRuntime,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub activity_bar: Vec<ActivityBarContainer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activation_events: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keybindings: Vec<Keybinding>,
    #[serde(default, skip_serializing_if = "Menus::is_empty")]
    pub menus: Menus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<ThemeContribution>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snippets: Vec<SnippetContribution>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageComments>,
}

/// A color theme contributed by a native Graphite plugin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeContribution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub label: String,
    #[serde(default = "default_ui_theme")]
    pub ui_theme: String,
    pub path: String,
}

fn default_ui_theme() -> String {
    "vs-dark".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keybinding {
    pub command: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Menus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_palette: Vec<MenuItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub editor_context: Vec<MenuItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub view_item_context: Vec<MenuItem>,
}

impl Menus {
    pub fn is_empty(&self) -> bool {
        self.command_palette.is_empty()
            && self.editor_context.is_empty()
            && self.view_item_context.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuItem {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBarContainer {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub views: Vec<ActivityBarView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBarView {
    pub id: String,
    pub title: String,
    #[serde(default = "default_view_kind")]
    pub kind: String,
}

fn default_view_kind() -> String {
    "tree".into()
}
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<_> = id.split('.').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        if !valid_id(&self.id) {
            return Err(
                "Plugin ID must be publisher.name (lowercase letters, digits, hyphens)".into(),
            );
        }
        semver::Version::parse(&self.version).map_err(|e| e.to_string())?;
        if self.api_version != 1 {
            return Err("Unsupported Graphite API version".into());
        }
        match self.runtime {
            PluginRuntime::Node => {
                if !crate::package::safe_path(&self.main)
                    || ![".js", ".cjs", ".mjs"]
                        .iter()
                        .any(|s| self.main.ends_with(s))
                {
                    return Err("Plugin main must be a relative JavaScript file".into());
                }
            }
            PluginRuntime::Native => {
                if !crate::package::safe_path(&self.main) || self.main.ends_with('/') {
                    return Err("Native plugin main must be a relative executable path".into());
                }
            }
        }
        for permission in &self.permissions {
            if !VALID_PERMISSIONS.contains(&permission.as_str()) {
                return Err(format!("Unsupported permission in manifest: {permission}"));
            }
        }
        let mut ids = std::collections::HashSet::new();
        for command in &self.commands {
            if !command.id.starts_with(&format!("{}.", self.id))
                || command.title.trim().is_empty()
                || !ids.insert(&command.id)
            {
                return Err(
                    "Commands need unique IDs prefixed by the plugin ID and nonempty titles".into(),
                );
            }
        }
        for container in &self.activity_bar {
            if !container.id.starts_with(&format!("{}.", self.id))
                || container.title.trim().is_empty()
                || !ids.insert(&container.id)
            {
                return Err(
                    "Activity bar containers need unique plugin-prefixed IDs and nonempty titles"
                        .into(),
                );
            }
            for view in &container.views {
                if !view.id.starts_with(&format!("{}.", self.id))
                    || view.title.trim().is_empty()
                    || !["tree", "webview"].contains(&view.kind.as_str())
                    || !ids.insert(&view.id)
                {
                    return Err("Activity bar views need unique plugin-prefixed IDs, titles and valid kinds".into());
                }
            }
        }
        let prefix = format!("{}.", self.id);
        for event in &self.activation_events {
            let valid = event == "*"
                || event == "onStartupFinished"
                || event
                    .strip_prefix("onCommand:")
                    .is_some_and(|v| !v.is_empty())
                || event
                    .strip_prefix("onLanguage:")
                    .is_some_and(|v| !v.is_empty())
                || event.strip_prefix("onView:").is_some_and(|v| !v.is_empty());
            if !valid {
                return Err("Unsupported activation event in manifest".into());
            }
        }
        for binding in &self.keybindings {
            if !binding.command.starts_with(&prefix) || binding.key.trim().is_empty() {
                return Err(
                    "Plugin keybindings need a plugin-prefixed command and nonempty key".into(),
                );
            }
        }
        for item in self
            .menus
            .command_palette
            .iter()
            .chain(&self.menus.editor_context)
            .chain(&self.menus.view_item_context)
        {
            if !item.command.starts_with(&prefix) {
                return Err("Plugin menu items need a plugin-prefixed command".into());
            }
        }
        for theme in &self.themes {
            let path = theme.path.strip_prefix("./").unwrap_or(&theme.path);
            if theme.label.trim().is_empty() || !crate::package::safe_path(path) {
                return Err("Plugin themes need a non-empty label and safe relative path".into());
            }
        }
        for snippet in &self.snippets {
            let path = snippet.path.strip_prefix("./").unwrap_or(&snippet.path);
            if snippet.language.trim().is_empty() || !crate::package::safe_path(path) {
                return Err("Plugin snippets need a language id and safe relative path".into());
            }
        }
        for language in &self.languages {
            if language.id.trim().is_empty() {
                return Err("Plugin language declarations need a non-empty id".into());
            }
        }
        Ok(())
    }

    /// Checks whether this manifest permits `permission`.
    ///
    /// Privileged system capabilities (`workspace.read`, `workspace.write`, `terminal`, `chat`)
    /// always require an explicit declaration (or `"*"`), whereas standard editor capabilities
    /// remain permitted when `permissions` is omitted for backward compatibility.
    pub fn allows_permission(&self, permission: &str) -> bool {
        if self.permissions.iter().any(|p| p == "*" || p == permission) {
            return true;
        }
        let privileged = matches!(
            permission,
            "workspace.read" | "workspace.write" | "terminal" | "chat"
        );
        if privileged {
            return false;
        }
        self.permissions.is_empty()
    }
}

/// Resolves the entrypoint file on disk for `manifest` inside `root`, supporting `.exe` on Windows.
pub fn resolve_entrypoint(root: &Path, manifest: &Manifest) -> Option<PathBuf> {
    let direct = root.join(&manifest.main);
    if direct.is_file() {
        return Some(direct);
    }
    if manifest.runtime == PluginRuntime::Native
        && cfg!(windows)
        && !manifest.main.ends_with(".exe")
    {
        let with_exe = root.join(format!("{}.exe", manifest.main));
        if with_exe.is_file() {
            return Some(with_exe);
        }
    }
    None
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "format", content = "manifest", rename_all = "camelCase")]
pub enum Metadata {
    Native(Manifest),
    Vsix(Vsix),
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Vsix {
    pub id: String,
    pub version: String,
    pub description: String,
    pub engines: Value,
    pub main: Option<String>,
    pub contributions: Vec<String>,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub activity_bar: Vec<ActivityBarContainer>,
    #[serde(default)]
    pub language_comments: Vec<LanguageComments>,
    /// Snippet files the extension contributes (`contributes.snippets`).
    #[serde(default)]
    pub snippets: Vec<SnippetContribution>,
    /// Debug adapters the extension contributes (`contributes.debuggers`).
    #[serde(default)]
    pub debuggers: Vec<DebuggerContribution>,
    /// Activation events from VSIX `activationEvents`.
    #[serde(default)]
    pub activation_events: Vec<String>,
    /// Keybindings contributed via `contributes.keybindings`.
    #[serde(default)]
    pub keybindings: Vec<Keybinding>,
    /// Menu entries contributed via `contributes.menus`.
    #[serde(default)]
    pub menus: Menus,
    /// Custom editors contributed via `contributes.customEditors`.
    #[serde(default)]
    pub custom_editors: Vec<CustomEditorContribution>,
}

/// A custom editor an extension offers for some files (`contributes.customEditors`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEditorContribution {
    pub view_type: String,
    pub display_name: String,
    /// The `filenamePattern` globs of the selector. A pattern with a `/` matches the whole path,
    /// any other matches the file name, ignoring case.
    pub patterns: Vec<String>,
    /// `default` opens matching files automatically; `option` only through Open With.
    pub priority: String,
}

impl CustomEditorContribution {
    /// Whether the editor offers to open `path`.
    pub fn matches(&self, path: &std::path::Path) -> bool {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let full = path.to_string_lossy().replace('\\', "/");
        self.patterns.iter().any(|pattern| {
            let target = if pattern.contains('/') { &full } else { &name };
            globset::GlobBuilder::new(pattern)
                .case_insensitive(true)
                .literal_separator(false)
                .build()
                .is_ok_and(|glob| glob.compile_matcher().is_match(target))
        })
    }

    /// Whether files it matches open with it without asking.
    pub fn is_default(&self) -> bool {
        self.priority == "default"
    }
}

/// How to start a debug adapter: `runtime` (if any) runs `program`, a path inside the extension.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerExecutable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// One entry of `contributes.debuggers`: the debug `type` it serves and how to launch its adapter,
/// with optional per-platform overrides.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DebuggerContribution {
    #[serde(rename = "type")]
    pub debug_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub executable: DebuggerExecutable,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<DebuggerExecutable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<DebuggerExecutable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osx: Option<DebuggerExecutable>,
}

impl DebuggerContribution {
    /// The executable for `platform` (`"windows"`, `"linux"` or `"osx"`): the platform's fields
    /// replace the shared ones, field by field.
    pub fn executable_for(&self, platform: &str) -> DebuggerExecutable {
        let specific = match platform {
            "windows" => &self.windows,
            "linux" => &self.linux,
            "osx" => &self.osx,
            _ => &None,
        };
        let base = &self.executable;
        let Some(over) = specific else {
            return base.clone();
        };
        DebuggerExecutable {
            program: over.program.clone().or_else(|| base.program.clone()),
            runtime: over.runtime.clone().or_else(|| base.runtime.clone()),
            runtime_args: over
                .runtime_args
                .clone()
                .or_else(|| base.runtime_args.clone()),
            args: over.args.clone().or_else(|| base.args.clone()),
        }
    }
}

/// The platform key VS Code uses in `contributes.debuggers` overrides.
pub fn debugger_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Reads `contributes.debuggers`. Entries without a `type`, and `program` paths that leave the
/// extension directory, are dropped.
pub(crate) fn vsix_debuggers(manifest: &Value) -> Vec<DebuggerContribution> {
    fn safe(executable: &mut DebuggerExecutable) -> bool {
        if let Some(program) = executable.program.as_deref() {
            let relative = program.strip_prefix("./").unwrap_or(program);
            if !crate::package::safe_path(relative) {
                return false;
            }
            executable.program = Some(relative.to_owned());
        }
        true
    }
    manifest
        .pointer("/contributes/debuggers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let mut debugger: DebuggerContribution = serde_json::from_value(entry.clone()).ok()?;
            if debugger.debug_type.trim().is_empty() || !safe(&mut debugger.executable) {
                return None;
            }
            for platform in [
                &mut debugger.windows,
                &mut debugger.linux,
                &mut debugger.osx,
            ] {
                if let Some(over) = platform {
                    if !safe(over) {
                        *platform = None;
                    }
                }
            }
            Some(debugger)
        })
        .collect()
}

/// A snippet file inside the extension for one language.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnippetContribution {
    pub language: String,
    /// Path relative to the extension root, validated by [`crate::package::safe_path`].
    pub path: String,
}

pub(crate) fn vsix_snippets(manifest: &Value) -> Vec<SnippetContribution> {
    manifest
        .pointer("/contributes/snippets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let language = entry.get("language")?.as_str()?.trim();
            let path = entry.get("path")?.as_str()?;
            let path = path.strip_prefix("./").unwrap_or(path);
            (!language.is_empty() && crate::package::safe_path(path)).then(|| SnippetContribution {
                language: language.to_owned(),
                path: path.to_owned(),
            })
        })
        .collect()
}

/// A file icon theme the extension contributes (`contributes.iconThemes`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IconThemeContribution {
    pub id: String,
    /// Display name; the id when the extension gives none.
    pub label: String,
    /// Path of the theme file relative to the extension root, validated by
    /// [`crate::package::safe_path`].
    pub path: String,
}

/// Reads `contributes.iconThemes` from an extension's `package.json`. Entries without an id or
/// with a path that leaves the extension are skipped.
pub fn vsix_icon_themes(manifest: &Value) -> Vec<IconThemeContribution> {
    manifest
        .pointer("/contributes/iconThemes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?.trim();
            let path = entry.get("path")?.as_str()?;
            let path = path.strip_prefix("./").unwrap_or(path);
            let label = entry
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .unwrap_or(id);
            (!id.is_empty() && crate::package::safe_path(path)).then(|| IconThemeContribution {
                id: id.to_owned(),
                label: label.to_owned(),
                path: path.to_owned(),
            })
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageComments {
    pub id: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub line_comment: Option<String>,
    #[serde(default)]
    pub block_comment: Option<[String; 2]>,
}

pub(crate) fn vsix_language_comments(
    manifest: &Value,
    mut read_configuration: impl FnMut(&str) -> Option<Value>,
) -> Vec<LanguageComments> {
    let mut result = Vec::new();
    for language in manifest
        .pointer("/contributes/languages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = language.get("id").and_then(Value::as_str) else {
            continue;
        };
        let configuration = language
            .get("configuration")
            .and_then(Value::as_str)
            .map(|path| path.strip_prefix("./").unwrap_or(path))
            .filter(|path| crate::package::safe_path(path))
            .and_then(&mut read_configuration);
        let comments = configuration
            .as_ref()
            .and_then(|configuration| configuration.get("comments"));
        let line_comment = comments
            .and_then(|comments| comments.get("lineComment"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let block_comment = comments
            .and_then(|comments| comments.get("blockComment"))
            .and_then(Value::as_array)
            .filter(|markers| markers.len() == 2)
            .and_then(|markers| {
                Some([
                    markers[0].as_str()?.to_owned(),
                    markers[1].as_str()?.to_owned(),
                ])
            })
            .filter(|markers| !markers[0].is_empty() && !markers[1].is_empty());
        if line_comment.is_none() && block_comment.is_none() {
            continue;
        }
        let extensions = language
            .get("extensions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
            .filter(|extension| !extension.is_empty())
            .collect();
        result.push(LanguageComments {
            id: id.to_owned(),
            extensions,
            line_comment,
            block_comment,
        });
    }
    result
}

pub(crate) fn vsix_activity_bar(manifest: &Value) -> Vec<ActivityBarContainer> {
    let containers = manifest
        .pointer("/contributes/viewsContainers/activitybar")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    containers
        .into_iter()
        .filter_map(|container| {
            let id = container.get("id")?.as_str()?.trim();
            let title = container.get("title")?.as_str()?.trim();
            if id.is_empty() || title.is_empty() {
                return None;
            }
            let views = manifest
                .pointer(&format!(
                    "/contributes/views/{}",
                    id.replace('~', "~0").replace('/', "~1")
                ))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|view| {
                    let view_id = view.get("id")?.as_str()?.trim();
                    let view_title = view.get("name")?.as_str()?.trim();
                    (!view_id.is_empty() && !view_title.is_empty()).then(|| ActivityBarView {
                        id: view_id.into(),
                        title: view_title.into(),
                        kind: match view.get("type").and_then(Value::as_str) {
                            Some("webview") => "webview".into(),
                            _ => "tree".into(),
                        },
                    })
                })
                .collect();
            Some(ActivityBarContainer {
                id: id.into(),
                title: title.into(),
                icon: container
                    .get("icon")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                views,
            })
        })
        .collect()
}

pub(crate) fn vsix_commands(manifest: &Value) -> Vec<Command> {
    let mut ids = std::collections::HashSet::new();
    manifest
        .pointer("/contributes/commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("command")?.as_str()?;
            let title = entry.get("title")?;
            let title = title.as_str().or_else(|| title.get("value")?.as_str())?;
            if id.trim().is_empty() || title.trim().is_empty() || !ids.insert(id) {
                return None;
            }
            let title = match entry.get("category").and_then(Value::as_str) {
                Some(category) if !category.is_empty() => format!("{category}: {title}"),
                _ => title.to_owned(),
            };
            Some(Command {
                id: id.into(),
                title,
            })
        })
        .collect()
}

pub(crate) fn vsix_activation_events(manifest: &Value) -> Vec<String> {
    let mut events: Vec<String> = manifest
        .get("activationEvents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            v.as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
        .collect();
    // Contributions activate their extension without listing the event.
    let mut implicit = |event: String| {
        if !events.contains(&event) {
            events.push(event);
        }
    };
    for editor in vsix_custom_editors(manifest) {
        implicit(format!("onCustomEditor:{}", editor.view_type));
    }
    for views in manifest
        .pointer("/contributes/views")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|containers| containers.values())
    {
        for id in views
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|view| view.get("id").and_then(Value::as_str))
        {
            implicit(format!("onView:{id}"));
        }
    }
    for id in manifest
        .pointer("/contributes/authentication")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|provider| provider.get("id").and_then(Value::as_str))
    {
        implicit(format!("onAuthenticationRequest:{id}"));
    }
    events
}

pub(crate) fn vsix_custom_editors(manifest: &Value) -> Vec<CustomEditorContribution> {
    manifest
        .pointer("/contributes/customEditors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let view_type = entry.get("viewType")?.as_str()?.trim();
            if view_type.is_empty() {
                return None;
            }
            let patterns: Vec<String> = entry
                .get("selector")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|selector| selector.get("filenamePattern")?.as_str())
                .map(str::to_owned)
                .collect();
            Some(CustomEditorContribution {
                view_type: view_type.to_owned(),
                display_name: entry
                    .get("displayName")
                    .and_then(Value::as_str)
                    .unwrap_or(view_type)
                    .to_owned(),
                patterns,
                priority: entry
                    .get("priority")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_owned(),
            })
        })
        .collect()
}

pub(crate) fn vsix_keybindings(manifest: &Value) -> Vec<Keybinding> {
    manifest
        .pointer("/contributes/keybindings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let command = entry.get("command").and_then(Value::as_str)?.trim();
            if command.is_empty() {
                return None;
            }
            #[cfg(target_os = "windows")]
            let raw_key = entry
                .get("win")
                .and_then(Value::as_str)
                .or_else(|| entry.get("key").and_then(Value::as_str));
            #[cfg(not(target_os = "windows"))]
            let raw_key = entry
                .get("linux")
                .and_then(Value::as_str)
                .or_else(|| entry.get("key").and_then(Value::as_str));
            let key = raw_key?.trim();
            if key.is_empty() {
                return None;
            }
            let when = entry
                .get("when")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            Some(Keybinding {
                command: command.to_owned(),
                key: key.to_owned(),
                when,
            })
        })
        .collect()
}

fn parse_vsix_menu_list(manifest: &Value, pointer: &str) -> Vec<MenuItem> {
    manifest
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let command = entry.get("command").and_then(Value::as_str)?.trim();
            if command.is_empty() {
                return None;
            }
            let when = entry
                .get("when")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            let group = entry
                .get("group")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            Some(MenuItem {
                command: command.to_owned(),
                when,
                group,
            })
        })
        .collect()
}

pub(crate) fn vsix_menus(manifest: &Value) -> Menus {
    Menus {
        command_palette: parse_vsix_menu_list(manifest, "/contributes/menus/commandPalette"),
        editor_context: parse_vsix_menu_list(manifest, "/contributes/menus/editor~1context"),
        view_item_context: parse_vsix_menu_list(manifest, "/contributes/menus/view~1item~1context"),
    }
}
impl Metadata {
    pub fn id(&self) -> &str {
        match self {
            Self::Native(m) => &m.id,
            Self::Vsix(m) => &m.id,
        }
    }
    pub fn version(&self) -> &str {
        match self {
            Self::Native(m) => &m.version,
            Self::Vsix(m) => &m.version,
        }
    }
}

#[cfg(test)]
mod activity_bar_tests {
    use super::*;

    #[test]
    fn vsix_activity_bar_keeps_only_valid_containers_and_views() {
        let manifest = serde_json::json!({
            "contributes": {
                "viewsContainers": {"activitybar": [
                    {"id":"sample.tools","title":"Tools","icon":"media/tools.svg"},
                    {"id":"","title":"Invalid"}
                ]},
                "views": {"sample.tools": [
                    {"id":"sample.tree","name":"Tree"},
                    {"id":"sample.bad","name":""}
                ]}
            }
        });
        let containers = vsix_activity_bar(&manifest);
        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].id, "sample.tools");
        assert_eq!(containers[0].title, "Tools");
        assert_eq!(containers[0].icon.as_deref(), Some("media/tools.svg"));
        assert_eq!(containers[0].views.len(), 1);
        assert_eq!(containers[0].views[0].id, "sample.tree");
    }

    #[test]
    fn vsix_views_read_their_declared_type() {
        let manifest = serde_json::json!({
            "contributes": {
                "viewsContainers": {"activitybar": [{"id":"sample.tools","title":"Tools"}]},
                "views": {"sample.tools": [
                    {"id":"sample.panel","name":"Panel","type":"webview"},
                    {"id":"sample.tree","name":"Tree","type":"tree"},
                    {"id":"sample.default","name":"Default"},
                    {"id":"sample.other","name":"Other","type":"unknown"}
                ]}
            }
        });
        let containers = vsix_activity_bar(&manifest);
        let kinds: Vec<_> = containers[0]
            .views
            .iter()
            .map(|view| view.kind.as_str())
            .collect();
        assert_eq!(kinds, ["webview", "tree", "tree", "tree"]);
    }

    #[test]
    fn native_activity_bar_ids_are_scoped_and_kinds_are_checked() {
        let mut manifest: Manifest = serde_json::from_value(serde_json::json!({
            "id":"sample.plugin", "version":"1.0.0", "apiVersion":1,
            "main":"main.js", "description":"",
            "activityBar":[{"id":"sample.plugin.tools","title":"Tools","views":[
                {"id":"sample.plugin.tree","title":"Tree","kind":"tree"},
                {"id":"sample.plugin.panel","title":"Panel","kind":"webview"}
            ]}]
        }))
        .unwrap();
        assert!(manifest.validate().is_ok());
        manifest.activity_bar[0].views[1].kind = "unknown".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn vsix_language_comments_read_safe_configuration_and_ignore_invalid_markers() {
        let manifest = serde_json::json!({
            "contributes": {"languages": [
                {"id":"sample","extensions":[".smp"],"configuration":"./language.json"},
                {"id":"unsafe","extensions":[".bad"],"configuration":"../escape.json"}
            ]}
        });
        let comments = vsix_language_comments(&manifest, |path| {
            (path == "language.json").then(|| {
                serde_json::json!({
                    "comments":{"lineComment":"#","blockComment":["/*","*/"]}
                })
            })
        });
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, "sample");
        assert_eq!(comments[0].extensions, ["smp"]);
        assert_eq!(comments[0].line_comment.as_deref(), Some("#"));
        assert_eq!(comments[0].block_comment.as_ref().unwrap(), &["/*", "*/"]);
    }

    #[test]
    fn vsix_snippets_keep_only_safe_relative_paths_with_a_language() {
        let manifest = serde_json::json!({"contributes": {"snippets": [
            {"language": "rust", "path": "./snippets/rust.json"},
            {"language": "go", "path": "snippets/go.json"},
            {"language": "evil", "path": "../outside.json"},
            {"language": "abs", "path": "/etc/passwd"},
            {"language": "", "path": "snippets/none.json"},
            {"path": "snippets/no-language.json"},
        ]}});
        let snippets = vsix_snippets(&manifest);
        assert_eq!(
            snippets,
            vec![
                SnippetContribution {
                    language: "rust".into(),
                    path: "snippets/rust.json".into()
                },
                SnippetContribution {
                    language: "go".into(),
                    path: "snippets/go.json".into()
                },
            ]
        );
        assert!(vsix_snippets(&serde_json::json!({})).is_empty());
    }

    fn debugger_manifest() -> Value {
        serde_json::json!({"contributes": {"debuggers": [
            {
                "type": "node", "label": "Node Debug", "runtime": "node",
                "program": "./out/adapter.js", "args": ["--serve"],
                "runtimeArgs": ["--no-warnings"],
                "windows": {"program": "./out/adapter-win.js"},
                "osx": {"runtime": "mono", "args": []}
            },
            {"type": "native", "program": "bin/adapter"},
            {"type": "", "program": "bin/blank"},
            {"program": "bin/no-type"},
            {"type": "escape", "program": "../outside"},
            {"type": "absolute", "program": "/usr/bin/adapter"},
            {"type": "partial", "program": "bin/ok", "linux": {"program": "../bad"}}
        ]}})
    }

    #[test]
    fn vsix_debuggers_keep_safe_entries_with_a_type() {
        let debuggers = vsix_debuggers(&debugger_manifest());
        let types: Vec<_> = debuggers.iter().map(|d| d.debug_type.as_str()).collect();
        assert_eq!(types, ["node", "native", "partial"]);
        let node = &debuggers[0];
        assert_eq!(node.label.as_deref(), Some("Node Debug"));
        assert_eq!(node.executable.program.as_deref(), Some("out/adapter.js"));
        assert_eq!(node.executable.runtime.as_deref(), Some("node"));
        assert_eq!(node.executable.args, Some(vec!["--serve".into()]));
        assert_eq!(
            node.executable.runtime_args,
            Some(vec!["--no-warnings".into()])
        );
        assert_eq!(
            debuggers[2].linux, None,
            "an unsafe platform override is dropped, the shared program stays"
        );
        assert_eq!(debuggers[2].executable.program.as_deref(), Some("bin/ok"));
        assert!(vsix_debuggers(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn platform_overrides_replace_shared_fields_one_by_one() {
        let debuggers = vsix_debuggers(&debugger_manifest());
        let node = &debuggers[0];
        let windows = node.executable_for("windows");
        assert_eq!(windows.program.as_deref(), Some("out/adapter-win.js"));
        assert_eq!(windows.runtime.as_deref(), Some("node"), "not overridden");
        assert_eq!(windows.args, Some(vec!["--serve".into()]));
        let osx = node.executable_for("osx");
        assert_eq!(osx.runtime.as_deref(), Some("mono"));
        assert_eq!(
            osx.args,
            Some(Vec::new()),
            "an empty override still replaces"
        );
        assert_eq!(osx.program.as_deref(), Some("out/adapter.js"));
        assert_eq!(node.executable_for("linux"), node.executable);
        assert_eq!(node.executable_for("plan9"), node.executable);
    }

    #[test]
    fn records_saved_before_debuggers_existed_load_with_none() {
        let old = serde_json::json!({
            "id": "a.b", "version": "1.0.0", "description": "", "engines": {},
            "main": null, "contributions": ["debuggers"]
        });
        let vsix: Vsix = serde_json::from_value(old).unwrap();
        assert!(vsix.debuggers.is_empty());
        let round_trip: Vsix =
            serde_json::from_value(serde_json::to_value(&vsix).unwrap()).unwrap();
        assert!(round_trip.debuggers.is_empty());
    }

    #[test]
    fn icon_themes_keep_safe_entries_and_default_the_label() {
        let manifest = serde_json::json!({"contributes": {"iconThemes": [
            {"id": "pack", "label": "Pack Icons", "path": "./icons/pack.json"},
            {"id": "bare", "path": "icons/bare.json"},
            {"id": "escape", "label": "Escape", "path": "../outside.json"},
            {"label": "No id", "path": "icons/x.json"},
        ]}});
        let themes = vsix_icon_themes(&manifest);
        assert_eq!(
            themes,
            vec![
                IconThemeContribution {
                    id: "pack".into(),
                    label: "Pack Icons".into(),
                    path: "icons/pack.json".into(),
                },
                IconThemeContribution {
                    id: "bare".into(),
                    label: "bare".into(),
                    path: "icons/bare.json".into(),
                },
            ]
        );
        assert!(vsix_icon_themes(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn debugger_contributions_survive_serialization() {
        let debuggers = vsix_debuggers(&debugger_manifest());
        let text = serde_json::to_string(&debuggers).unwrap();
        let back: Vec<DebuggerContribution> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, debuggers);
    }

    #[test]
    fn native_manifest_validates_activation_events_keybindings_and_menus() {
        let valid: Manifest = serde_json::from_value(serde_json::json!({
            "id": "local.ui",
            "version": "1.0.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "UI test",
            "commands": [{ "id": "local.ui.open", "title": "Open" }],
            "activationEvents": ["onCommand:local.ui.open", "onView:local.ui.tree"],
            "keybindings": [{ "command": "local.ui.open", "key": "ctrl+alt+u", "when": "editorTextFocus" }],
            "menus": {
                "commandPalette": [{ "command": "local.ui.open", "when": "editorTextFocus" }],
                "editorContext": [{ "command": "local.ui.open", "group": "navigation" }]
            }
        }))
        .unwrap();
        valid.validate().unwrap();

        let mut bad_binding = valid.clone();
        bad_binding.keybindings[0].command = "workbench.action.files.save".into();
        assert!(bad_binding.validate().is_err());

        let mut bad_event = valid.clone();
        bad_event.activation_events.push("onUnknown:foo".into());
        assert!(bad_event.validate().is_err());

        let mut bad_menu = valid;
        bad_menu.menus.editor_context[0].command = "other.plugin.cmd".into();
        assert!(bad_menu.validate().is_err());
    }

    #[test]
    fn native_manifest_validates_themes_snippets_and_languages() {
        let valid: Manifest = serde_json::from_value(serde_json::json!({
            "id": "local.dsl",
            "version": "1.0.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "Language, snippet and theme plugin",
            "themes": [{ "id": "neon", "label": "Native Neon", "uiTheme": "vs-dark", "path": "./themes/neon.json" }],
            "snippets": [{ "language": "dsl", "path": "./snippets/dsl.json" }],
            "languages": [{ "id": "dsl", "extensions": [".dsl"], "lineComment": "///", "blockComment": ["/*", "*/"] }]
        }))
        .unwrap();
        valid.validate().unwrap();

        let mut bad_theme_path = valid.clone();
        bad_theme_path.themes[0].path = "../escape.json".into();
        assert!(bad_theme_path.validate().is_err());

        let mut bad_snippet = valid.clone();
        bad_snippet.snippets[0].path = "../snippets.json".into();
        assert!(bad_snippet.validate().is_err());

        let mut bad_lang = valid;
        bad_lang.languages[0].id = "   ".into();
        assert!(bad_lang.validate().is_err());
    }

    #[test]
    fn native_manifest_validates_runtime_and_granular_permissions() {
        let valid: Manifest = serde_json::from_value(serde_json::json!({
            "id": "local.native",
            "version": "1.0.0",
            "apiVersion": 1,
            "runtime": "native",
            "main": "bin/my-plugin",
            "description": "Compiled native Rust plugin",
            "permissions": ["commands", "workspace.read", "terminal"]
        }))
        .unwrap();
        valid.validate().unwrap();
        assert_eq!(valid.runtime, PluginRuntime::Native);
        assert!(valid.allows_permission("commands"));
        assert!(valid.allows_permission("workspace.read"));
        assert!(valid.allows_permission("terminal"));
        assert!(!valid.allows_permission("workspace.write"));
        assert!(!valid.allows_permission("chat"));
        assert!(!valid.allows_permission("ui"));

        // Empty permissions list remains backward compatible for standard capabilities only.
        let legacy: Manifest = serde_json::from_value(serde_json::json!({
            "id": "local.legacy",
            "version": "1.0.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "Legacy node plugin"
        }))
        .unwrap();
        assert!(legacy.allows_permission("commands"));
        assert!(legacy.allows_permission("ui"));
        assert!(!legacy.allows_permission("terminal"));
        assert!(!legacy.allows_permission("workspace.read"));

        let mut bad_perm = valid.clone();
        bad_perm.permissions.push("root.everything".into());
        assert!(bad_perm.validate().is_err());

        let mut bad_native_main = valid;
        bad_native_main.main = "../outside-bin".into();
        assert!(bad_native_main.validate().is_err());
    }

    #[test]
    fn custom_editors_are_parsed_and_match_files_by_pattern() {
        let manifest = serde_json::json!({
            "contributes": {"customEditors": [
                {"viewType": "sample.paint", "displayName": "Paint",
                 "selector": [{"filenamePattern": "*.PNG"}, {"filenamePattern": "**/icons/*.svg"}],
                 "priority": "option"},
                {"viewType": "sample.notes", "selector": [{"filenamePattern": "*.notes"}]},
                {"viewType": "", "selector": []}
            ]}
        });
        let editors = vsix_custom_editors(&manifest);
        assert_eq!(editors.len(), 2);
        assert_eq!(editors[0].display_name, "Paint");
        assert!(!editors[0].is_default());
        assert!(editors[1].is_default(), "the default priority is `default`");
        assert_eq!(editors[1].display_name, "sample.notes");
        let paint = &editors[0];
        assert!(paint.matches(std::path::Path::new("/work/pic.png")));
        assert!(paint.matches(std::path::Path::new("C:\\work\\Pic.PNG")));
        assert!(paint.matches(std::path::Path::new("/work/icons/a.svg")));
        assert!(!paint.matches(std::path::Path::new("/work/other/a.svg")));
        assert!(!paint.matches(std::path::Path::new("/work/pic.jpg")));
    }

    #[test]
    fn contributions_activate_their_extension_without_listing_the_event() {
        let manifest = serde_json::json!({
            "activationEvents": ["onView:sample.tree"],
            "contributes": {
                "customEditors": [{"viewType": "sample.paint", "selector": []}],
                "views": {"sample.box": [{"id": "sample.tree", "name": "T"}, {"id": "sample.web", "name": "W"}]},
                "authentication": [{"id": "sample.auth", "label": "A"}]
            }
        });
        assert_eq!(
            vsix_activation_events(&manifest),
            [
                "onView:sample.tree",
                "onCustomEditor:sample.paint",
                "onView:sample.web",
                "onAuthenticationRequest:sample.auth"
            ]
        );
    }

    #[test]
    fn vsix_activation_events_keybindings_and_menus_are_parsed() {
        let manifest = serde_json::json!({
            "activationEvents": ["onLanguage:rust", "", "workspaceContains:Cargo.toml"],
            "contributes": {
                "keybindings": [
                    { "command": "ext.fmt", "key": "ctrl+alt+f", "when": "editorTextFocus" },
                    { "command": "", "key": "ctrl+k" }
                ],
                "menus": {
                    "commandPalette": [{ "command": "ext.fmt", "when": "editorTextFocus" }],
                    "editor/context": [{ "command": "ext.fmt", "group": "1_modification" }],
                    "view/item/context": [{ "command": "ext.refresh", "group": "inline" }]
                }
            }
        });
        assert_eq!(
            vsix_activation_events(&manifest),
            vec!["onLanguage:rust", "workspaceContains:Cargo.toml"]
        );
        let keys = vsix_keybindings(&manifest);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].command, "ext.fmt");
        assert_eq!(keys[0].key, "ctrl+alt+f");
        assert_eq!(keys[0].when.as_deref(), Some("editorTextFocus"));

        let menus = vsix_menus(&manifest);
        assert_eq!(menus.command_palette.len(), 1);
        assert_eq!(menus.editor_context.len(), 1);
        assert_eq!(
            menus.editor_context[0].group.as_deref(),
            Some("1_modification")
        );
        assert_eq!(menus.view_item_context.len(), 1);
    }
}
