//! VSIX source control providers (`MainThreadSCM` / `ExtHostSCM`).
//!
//! An extension creates a source control with `scm.createSourceControl`, fills resource groups and
//! sets a commit box. The state is kept here and rendered by the Source Control panel below the
//! Git changes (see `source_control::extension_scm_ui`). Clicking a resource asks the extension to
//! run its command, and the commit box forwards its text and its accept command.

use super::*;
use graphite_core_api::DocumentUri;
use serde_json::{json, Value};

/// A command an extension attached to a source control or resource.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScmCommand {
    pub id: String,
    pub title: String,
    pub arguments: Vec<Value>,
}

impl ScmCommand {
    fn parse(value: &Value) -> Option<Self> {
        let id = value["id"].as_str().filter(|id| !id.is_empty())?;
        Some(Self {
            id: id.to_owned(),
            title: value["title"].as_str().unwrap_or_default().to_owned(),
            arguments: value["arguments"].as_array().cloned().unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScmResource {
    pub handle: u64,
    /// The resource URI as the extension sent it (for example `file:///work/a.txt`).
    pub uri: String,
    pub tooltip: Option<String>,
    pub strike_through: bool,
    pub faded: bool,
    pub context_value: Option<String>,
    pub command: Option<ScmCommand>,
}

impl ScmResource {
    /// Parses a `SCMRawResource` tuple.
    fn parse(raw: &Value) -> Option<Self> {
        let items = raw.as_array()?;
        Some(Self {
            handle: items.first()?.as_u64()?,
            uri: uri_text(items.get(1)?)?,
            tooltip: items
                .get(3)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_owned),
            strike_through: items.get(4).and_then(Value::as_bool).unwrap_or(false),
            faded: items.get(5).and_then(Value::as_bool).unwrap_or(false),
            context_value: items
                .get(6)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_owned),
            command: items.get(7).and_then(ScmCommand::parse),
        })
    }

    /// The file name shown for the resource.
    pub(crate) fn name(&self) -> String {
        let (_, path) = split_uri(&self.uri);
        path.rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(&path)
            .to_owned()
    }

    /// The folder shown after the name.
    pub(crate) fn description(&self) -> String {
        let (_, path) = split_uri(&self.uri);
        let trimmed = path.trim_end_matches('/');
        match trimmed.rsplit_once('/') {
            Some((parent, _)) => parent.trim_start_matches('/').to_owned(),
            None => String::new(),
        }
    }

    /// The local path of a `file:` resource.
    pub(crate) fn file_path(&self) -> Option<PathBuf> {
        DocumentUri::parse(&self.uri)
            .ok()
            .filter(|_| split_uri(&self.uri).0 == "file")
            .and_then(|uri| uri.to_path().ok())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScmGroup {
    pub handle: u64,
    pub id: String,
    pub label: String,
    pub hide_when_empty: bool,
    pub collapsed: bool,
    pub resources: Vec<ScmResource>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScmProvider {
    pub handle: u64,
    pub id: String,
    pub label: String,
    pub root: Option<PathBuf>,
    pub hidden: bool,
    pub count: Option<u64>,
    pub accept_input: Option<ScmCommand>,
    pub input: String,
    pub placeholder: Option<String>,
    pub input_visible: bool,
    pub input_enabled: bool,
    pub status_commands: Vec<ScmCommand>,
    pub groups: Vec<ScmGroup>,
}

impl ScmProvider {
    /// The number shown next to the provider: the extension's `count`, or the resources it lists.
    pub(crate) fn display_count(&self) -> u64 {
        self.count.unwrap_or_else(|| {
            self.groups
                .iter()
                .map(|group| group.resources.len() as u64)
                .sum()
        })
    }

    fn group_mut(&mut self, handle: u64) -> Option<&mut ScmGroup> {
        self.groups.iter_mut().find(|group| group.handle == handle)
    }

    pub(crate) fn group(&self, handle: u64) -> Option<&ScmGroup> {
        self.groups.iter().find(|group| group.handle == handle)
    }
}

/// What one row of the extension section of the panel stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtensionRow {
    Provider(u64),
    Input(u64),
    Group {
        provider: u64,
        group: u64,
        count: usize,
        collapsed: bool,
    },
    Resource {
        provider: u64,
        group: u64,
        resource: u64,
    },
}

#[derive(Default)]
pub(crate) struct Scm {
    providers: Vec<ScmProvider>,
}

impl Scm {
    #[cfg(test)]
    pub(super) fn clear_accept_for_test(&mut self, handle: u64) {
        if let Some(provider) = self.provider_mut(handle) {
            provider.accept_input = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn providers(&self) -> &[ScmProvider] {
        &self.providers
    }

    pub(crate) fn provider(&self, handle: u64) -> Option<&ScmProvider> {
        self.providers
            .iter()
            .find(|provider| provider.handle == handle)
    }

    fn provider_mut(&mut self, handle: u64) -> Option<&mut ScmProvider> {
        self.providers
            .iter_mut()
            .find(|provider| provider.handle == handle)
    }

    /// The rows of every visible provider, in panel order.
    pub(crate) fn rows(&self) -> Vec<ExtensionRow> {
        let mut rows = Vec::new();
        for provider in self.providers.iter().filter(|provider| !provider.hidden) {
            rows.push(ExtensionRow::Provider(provider.handle));
            if provider.input_visible {
                rows.push(ExtensionRow::Input(provider.handle));
            }
            for group in &provider.groups {
                if group.hide_when_empty && group.resources.is_empty() {
                    continue;
                }
                rows.push(ExtensionRow::Group {
                    provider: provider.handle,
                    group: group.handle,
                    count: group.resources.len(),
                    collapsed: group.collapsed,
                });
                if !group.collapsed {
                    rows.extend(group.resources.iter().map(|resource| {
                        ExtensionRow::Resource {
                            provider: provider.handle,
                            group: group.handle,
                            resource: resource.handle,
                        }
                    }));
                }
            }
        }
        rows
    }

    fn register(&mut self, provider: ScmProvider) {
        self.providers
            .retain(|existing| existing.handle != provider.handle);
        self.providers.push(provider);
    }

    fn unregister(&mut self, handle: u64) -> bool {
        let before = self.providers.len();
        self.providers.retain(|provider| provider.handle != handle);
        self.providers.len() != before
    }

    fn splice(&mut self, provider: u64, splices: &Value) {
        let Some(provider) = self.provider_mut(provider) else {
            return;
        };
        for entry in splices.as_array().into_iter().flatten() {
            let Some(group) = entry
                .get(0)
                .and_then(Value::as_u64)
                .and_then(|handle| provider.group_mut(handle))
            else {
                continue;
            };
            for splice in entry
                .get(1)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let start = splice.get(0).and_then(Value::as_u64).unwrap_or(0) as usize;
                let delete = splice.get(1).and_then(Value::as_u64).unwrap_or(0) as usize;
                let inserted: Vec<ScmResource> = splice
                    .get(2)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(ScmResource::parse)
                    .collect();
                let start = start.min(group.resources.len());
                let end = (start + delete).min(group.resources.len());
                group.resources.splice(start..end, inserted);
            }
        }
    }
}

/// Splits `scheme:rest` into the scheme and a decoded path without its authority.
fn split_uri(uri: &str) -> (String, String) {
    let Some((scheme, rest)) = uri.split_once(':') else {
        return (String::new(), uri.to_owned());
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let path = match rest.strip_prefix("//") {
        Some(after) => after.find('/').map_or("", |index| &after[index..]),
        None => rest,
    };
    (scheme.to_owned(), super::webviews::percent_decode(path))
}

/// A URI argument as text: either already a string or an object with a `scheme` and `path`.
fn uri_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    let scheme = value["scheme"].as_str()?;
    let path = value["path"].as_str().unwrap_or_default();
    Some(format!("{scheme}://{path}"))
}

fn apply_features(provider: &mut ScmProvider, features: &Value) {
    if let Some(object) = features.as_object() {
        if let Some(count) = object.get("count") {
            provider.count = count.as_u64();
        }
        if let Some(command) = object.get("acceptInputCommand") {
            provider.accept_input = ScmCommand::parse(command);
        }
        if let Some(template) = object.get("commitTemplate").and_then(Value::as_str) {
            if provider.input.is_empty() {
                provider.input = template.to_owned();
            }
        }
        if let Some(commands) = object.get("statusBarCommands").and_then(Value::as_array) {
            provider.status_commands = commands.iter().filter_map(ScmCommand::parse).collect();
        }
    }
}

impl App {
    pub(super) fn handle_vsix_scm_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let handle = args
            .first()
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("Invalid {method} source control handle"))?;
        let text = |index: usize| args.get(index).and_then(Value::as_str);
        let scm = &mut runtime.scm;
        match method {
            "$registerSourceControl" => {
                let root = args
                    .get(4)
                    .and_then(uri_text)
                    .and_then(|uri| DocumentUri::parse(&uri).ok())
                    .and_then(|uri| uri.to_path().ok());
                scm.register(ScmProvider {
                    handle,
                    id: text(2).unwrap_or_default().to_owned(),
                    label: text(3).unwrap_or_default().to_owned(),
                    root,
                    hidden: args.get(6).and_then(Value::as_bool).unwrap_or(false),
                    count: None,
                    accept_input: None,
                    input: String::new(),
                    placeholder: None,
                    input_visible: true,
                    input_enabled: true,
                    status_commands: Vec::new(),
                    groups: Vec::new(),
                });
                Ok(Value::Null)
            }
            "$updateSourceControl" => {
                if let Some(provider) = scm.provider_mut(handle) {
                    apply_features(provider, args.get(1).unwrap_or(&Value::Null));
                }
                Ok(Value::Null)
            }
            "$unregisterSourceControl" => {
                if scm.unregister(handle) && self.scm_extension_input == Some(handle) {
                    self.scm_extension_input = None;
                }
                Ok(Value::Null)
            }
            "$registerGroups" => {
                if let Some(provider) = scm.provider_mut(handle) {
                    for group in args
                        .get(1)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let Some(group_handle) = group.get(0).and_then(Value::as_u64) else {
                            continue;
                        };
                        provider.groups.retain(|existing| existing.handle != group_handle);
                        provider.groups.push(ScmGroup {
                            handle: group_handle,
                            id: group
                                .get(1)
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            label: group
                                .get(2)
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            hide_when_empty: group.get(3).is_some_and(|features| {
                                features["hideWhenEmpty"].as_bool().unwrap_or(false)
                            }),
                            collapsed: false,
                            resources: Vec::new(),
                        });
                    }
                }
                scm.splice(handle, args.get(2).unwrap_or(&Value::Null));
                Ok(Value::Null)
            }
            "$updateGroup" | "$updateGroupLabel" | "$unregisterGroup" => {
                let group_handle = args
                    .get(1)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("Invalid {method} group handle"))?;
                if let Some(provider) = scm.provider_mut(handle) {
                    match method {
                        "$updateGroup" => {
                            if let Some(group) = provider.group_mut(group_handle) {
                                if let Some(hide) = args
                                    .get(2)
                                    .and_then(|features| features["hideWhenEmpty"].as_bool())
                                {
                                    group.hide_when_empty = hide;
                                }
                            }
                        }
                        "$updateGroupLabel" => {
                            if let Some(group) = provider.group_mut(group_handle) {
                                group.label = text(2).unwrap_or_default().to_owned();
                            }
                        }
                        _ => provider.groups.retain(|group| group.handle != group_handle),
                    }
                }
                Ok(Value::Null)
            }
            "$spliceResourceStates" => {
                scm.splice(handle, args.get(1).unwrap_or(&Value::Null));
                Ok(Value::Null)
            }
            "$setInputBoxValue" => {
                if let Some(provider) = scm.provider_mut(handle) {
                    provider.input = text(1).unwrap_or_default().to_owned();
                }
                Ok(Value::Null)
            }
            "$setInputBoxPlaceholder" => {
                if let Some(provider) = scm.provider_mut(handle) {
                    provider.placeholder = text(1).filter(|text| !text.is_empty()).map(str::to_owned);
                }
                Ok(Value::Null)
            }
            "$setInputBoxEnablement" | "$setInputBoxVisibility" => {
                let value = args.get(1).and_then(Value::as_bool).unwrap_or(true);
                if let Some(provider) = scm.provider_mut(handle) {
                    if method == "$setInputBoxEnablement" {
                        provider.input_enabled = value;
                    } else {
                        provider.input_visible = value;
                    }
                }
                Ok(Value::Null)
            }
            "$showValidationMessage" => {
                if let Some(message) = text(1) {
                    self.status = message.to_owned();
                }
                Ok(Value::Null)
            }
            "$setValidationProviderIsEnabled"
            | "$onDidChangeHistoryProviderCurrentHistoryItemRefs"
            | "$onDidChangeHistoryProviderHistoryItemRefs"
            | "$onDidChangeArtifacts" => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadSCM", method);
                Err(format!("Graphite does not support MainThreadSCM.{method}"))
            }
        }
    }

    /// The extension providers the Source Control panel lists.
    pub(crate) fn vsix_scm(&self) -> Option<&Scm> {
        self.plugins.vsix.as_ref().map(|runtime| &runtime.scm)
    }

    /// The rows of the extension section of the panel.
    pub(crate) fn vsix_scm_rows(&self) -> Vec<ExtensionRow> {
        self.vsix_scm().map(Scm::rows).unwrap_or_default()
    }

    pub(crate) fn toggle_vsix_scm_group(&mut self, provider: u64, group: u64) {
        if let Some(group) = self
            .plugins
            .vsix
            .as_mut()
            .and_then(|runtime| runtime.scm.provider_mut(provider))
            .and_then(|provider| provider.group_mut(group))
        {
            group.collapsed = !group.collapsed;
        }
    }

    /// Edits a provider's commit box and tells the extension.
    pub(crate) fn edit_vsix_scm_input(&mut self, provider: u64, edit: impl FnOnce(&mut String)) {
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            return;
        };
        let Some(state) = runtime.scm.provider_mut(provider) else {
            return;
        };
        if !state.input_enabled {
            return;
        }
        edit(&mut state.input);
        let value = state.input.clone();
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostSCM",
            "method": "$onInputBoxValueChange",
            "args": [provider, value],
        }));
    }

    /// Runs the provider's accept-input command (the "commit" action).
    pub(crate) fn accept_vsix_scm_input(&mut self, provider: u64) {
        let command = self
            .vsix_scm()
            .and_then(|scm| scm.provider(provider))
            .and_then(|provider| provider.accept_input.clone());
        match command {
            Some(command) => self.execute_vsix_tree_command(&command.id, command.arguments),
            None => self.status = self.t("scm.sidebar.extension_no_accept").to_owned(),
        }
    }

    /// Opens a resource: the extension runs the command it attached; a `file:` resource without a
    /// command is opened as a file.
    pub(crate) fn open_vsix_scm_resource(&mut self, provider: u64, group: u64, resource: u64) {
        let Some(found) = self
            .vsix_scm()
            .and_then(|scm| scm.provider(provider))
            .and_then(|provider| provider.group(group))
            .and_then(|group| group.resources.iter().find(|item| item.handle == resource))
            .cloned()
        else {
            return;
        };
        if found.command.is_some() {
            if let Some(runtime) = self.plugins.vsix.as_mut() {
                let _ = runtime.session.send(json!({
                    "type": "call",
                    "actor": "ExtHostSCM",
                    "method": "$executeResourceCommand",
                    "args": [provider, group, resource, false],
                }));
            }
        } else if let Some(path) = found.file_path() {
            self.open_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(handle: u64, uri: &str) -> Value {
        json!([handle, uri, [null, null], "tip", false, true, "ctx", null])
    }

    fn provider() -> ScmProvider {
        ScmProvider {
            handle: 1,
            id: "git".into(),
            label: "Sample".into(),
            root: None,
            hidden: false,
            count: None,
            accept_input: None,
            input: String::new(),
            placeholder: None,
            input_visible: true,
            input_enabled: true,
            status_commands: Vec::new(),
            groups: vec![ScmGroup {
                handle: 7,
                id: "changes".into(),
                label: "Changes".into(),
                hide_when_empty: false,
                collapsed: false,
                resources: Vec::new(),
            }],
        }
    }

    #[test]
    fn resources_parse_their_decorations_and_command() {
        let raw = json!([4, "file:///work/src/a.rs", [null, null], "Modified", true, false, "m",
            {"id": "ext.open", "title": "Open", "arguments": [1]}]);
        let resource = ScmResource::parse(&raw).unwrap();
        assert_eq!(resource.handle, 4);
        assert_eq!(resource.name(), "a.rs");
        assert_eq!(resource.description(), "work/src");
        assert_eq!(resource.tooltip.as_deref(), Some("Modified"));
        assert!(resource.strike_through && !resource.faded);
        assert_eq!(resource.context_value.as_deref(), Some("m"));
        assert_eq!(resource.command.unwrap().id, "ext.open");
        assert!(ScmResource::parse(&json!(["x"])).is_none());
    }

    #[test]
    fn splices_insert_replace_and_delete_resources_in_order() {
        let mut scm = Scm::default();
        scm.register(provider());
        let uris = ["file:///a", "file:///b", "file:///c"];
        let insert: Vec<Value> = uris.iter().enumerate().map(|(i, u)| resource(i as u64, u)).collect();
        scm.splice(1, &json!([[7, [[0, 0, insert]]]]));
        let names = |scm: &Scm| -> Vec<u64> {
            scm.provider(1).unwrap().group(7).unwrap().resources.iter().map(|r| r.handle).collect()
        };
        assert_eq!(names(&scm), [0, 1, 2]);
        // Replace the middle one and delete the last, past-the-end ranges are clamped.
        scm.splice(1, &json!([[7, [[1, 1, [resource(9, "file:///z")]], [2, 50, []]]]]));
        assert_eq!(names(&scm), [0, 9]);
        // Unknown groups and providers are ignored.
        scm.splice(1, &json!([[99, [[0, 0, [resource(5, "file:///q")]]]]]));
        scm.splice(2, &json!([[7, [[0, 0, [resource(5, "file:///q")]]]]]));
        assert_eq!(names(&scm), [0, 9]);
    }

    #[test]
    fn rows_list_providers_inputs_groups_and_resources_unless_collapsed_or_hidden() {
        let mut scm = Scm::default();
        let mut state = provider();
        state.groups[0].resources = vec![ScmResource::parse(&resource(3, "file:///a")).unwrap()];
        state.groups.push(ScmGroup {
            handle: 8,
            id: "empty".into(),
            label: "Empty".into(),
            hide_when_empty: true,
            collapsed: false,
            resources: Vec::new(),
        });
        scm.register(state);
        assert_eq!(
            scm.rows(),
            [
                ExtensionRow::Provider(1),
                ExtensionRow::Input(1),
                ExtensionRow::Group { provider: 1, group: 7, count: 1, collapsed: false },
                ExtensionRow::Resource { provider: 1, group: 7, resource: 3 },
            ]
        );
        scm.provider_mut(1).unwrap().groups[0].collapsed = true;
        scm.provider_mut(1).unwrap().input_visible = false;
        assert_eq!(scm.rows().len(), 2);
        scm.provider_mut(1).unwrap().hidden = true;
        assert!(scm.rows().is_empty());
    }

    #[test]
    fn count_falls_back_to_the_listed_resources() {
        let mut state = provider();
        state.groups[0].resources = vec![
            ScmResource::parse(&resource(1, "file:///a")).unwrap(),
            ScmResource::parse(&resource(2, "file:///b")).unwrap(),
        ];
        assert_eq!(state.display_count(), 2);
        apply_features(&mut state, &json!({"count": 5, "commitTemplate": "feat: ",
            "acceptInputCommand": {"id": "ext.commit", "title": "Commit"}}));
        assert_eq!(state.display_count(), 5);
        assert_eq!(state.input, "feat: ");
        assert_eq!(state.accept_input.as_ref().unwrap().id, "ext.commit");
    }

    #[test]
    fn uris_without_a_file_scheme_have_no_local_path() {
        let remote = ScmResource::parse(&resource(1, "memfs:/docs/a.md")).unwrap();
        assert_eq!(remote.name(), "a.md");
        assert!(remote.file_path().is_none());
    }
}
