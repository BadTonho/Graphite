//! VSIX webview views (`MainThreadWebviewViews` / `ExtHostWebviewViews`).
//!
//! An extension registers a provider for a `contributes.views` entry of type `webview`. Graphite
//! asks the provider to resolve the view the first time it becomes visible, tracks its title,
//! description and badge, and hosts its page in a native overlay positioned over the side bar
//! (see [`App::sync_sidebar_webviews`]). Page content, options and messages travel through the
//! shared `MainThreadWebviews` bridge keyed by the view's webview handle.

use super::webviews::{parse_webview_options, uri_value_to_path, VsixWebviewPanel, DEFAULT_HTML};
use super::*;
use serde_json::{json, Value};

/// Replies to webview view resolution carry ids in `VIEW_BASE..VIEW_END`.
pub(super) const VIEW_BASE: u64 = 14_000_000;
const VIEW_END: u64 = 15_000_000;

pub(super) fn is_view_reply(id: u64) -> bool {
    (VIEW_BASE..VIEW_END).contains(&id)
}

/// Height of a view header row, in side bar line heights.
const HEADER_ROWS: f32 = 1.25;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WebviewViewState {
    pub view_type: String,
    /// The webview handle shared with `MainThreadWebviews`.
    pub handle: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub badge: Option<(u64, Option<String>)>,
    pub visible: bool,
    /// Resolution failed; it is not retried until the provider is registered again.
    pub failed: bool,
}

#[derive(Clone, Debug)]
struct Provider {
    extension_root: Option<PathBuf>,
    retain_context: bool,
}

#[derive(Default)]
pub(super) struct WebviewViews {
    next_id: u64,
    next_handle: u64,
    providers: BTreeMap<String, Provider>,
    views: BTreeMap<String, WebviewViewState>,
    resolving: BTreeMap<u64, String>,
    /// A view an extension asked to reveal; handled once the runtime is back in the app.
    pub(super) pending_show: Option<String>,
}

impl WebviewViews {
    fn register(&mut self, view_type: &str, provider: Provider) -> Option<String> {
        self.providers.insert(view_type.to_owned(), provider);
        self.resolving.retain(|_, pending| pending != view_type);
        self.views.remove(view_type).map(|state| state.handle)
    }

    fn unregister(&mut self, view_type: &str) -> Option<String> {
        self.providers.remove(view_type);
        self.resolving.retain(|_, pending| pending != view_type);
        self.views.remove(view_type).map(|state| state.handle)
    }

    pub(super) fn has_provider(&self, view_type: &str) -> bool {
        self.providers.contains_key(view_type)
    }

    pub(super) fn retains_context(&self, view_type: &str) -> bool {
        self.providers
            .get(view_type)
            .is_some_and(|provider| provider.retain_context)
    }

    pub(crate) fn view(&self, view_type: &str) -> Option<&WebviewViewState> {
        self.views.get(view_type)
    }

    fn view_for_handle_mut(&mut self, handle: &str) -> Option<&mut WebviewViewState> {
        self.views.values_mut().find(|state| state.handle == handle)
    }

    #[cfg(test)]
    pub(super) fn is_view_handle(&self, handle: &str) -> bool {
        self.views.values().any(|state| state.handle == handle)
    }

    /// Drops the bookkeeping of a view whose webview the extension disposed.
    pub(super) fn forget_handle(&mut self, handle: &str) {
        self.views.retain(|_, state| state.handle != handle);
    }

    /// Starts resolving `view_type` when it has a provider and no live or failed resolution.
    /// Returns the request id and the new webview handle.
    fn begin_resolve(&mut self, view_type: &str) -> Option<(u64, String)> {
        if !self.providers.contains_key(view_type) || self.views.contains_key(view_type) {
            return None;
        }
        let id = VIEW_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (VIEW_END - VIEW_BASE);
        let handle = format!("graphite-view-{}", self.next_handle);
        self.next_handle += 1;
        self.views.insert(
            view_type.to_owned(),
            WebviewViewState {
                view_type: view_type.to_owned(),
                handle: handle.clone(),
                title: None,
                description: None,
                badge: None,
                visible: false,
                failed: false,
            },
        );
        self.resolving.insert(id, view_type.to_owned());
        Some((id, handle))
    }

    fn finish_resolve(&mut self, id: u64, failed: bool) {
        let Some(view_type) = self.resolving.remove(&id) else {
            return;
        };
        if let Some(state) = self.views.get_mut(&view_type) {
            state.failed = failed;
        }
    }

    pub(super) fn busy(&self) -> bool {
        !self.resolving.is_empty()
    }
}

/// Where one webview view is drawn, in side bar coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WebviewSlot {
    pub view_id: String,
    pub header_y: f32,
    pub top: f32,
    pub height: f32,
}

/// Splits the side bar area below the container header between `view_ids`. When the container also
/// has tree views, the webviews take the lower half.
pub(crate) fn webview_slots(
    view_ids: &[String],
    has_tree: bool,
    line_height: f32,
    area_bottom: f32,
) -> Vec<WebviewSlot> {
    if view_ids.is_empty() {
        return Vec::new();
    }
    let mut area_top = line_height * 4.0;
    if has_tree {
        area_top += (area_bottom - area_top).max(0.0) * 0.5;
    }
    let slot_height = ((area_bottom - area_top) / view_ids.len() as f32).max(0.0);
    let header = line_height * HEADER_ROWS;
    view_ids
        .iter()
        .enumerate()
        .map(|(index, view_id)| {
            let header_y = area_top + slot_height * index as f32;
            WebviewSlot {
                view_id: view_id.clone(),
                header_y,
                top: header_y + header,
                height: (slot_height - header).max(0.0),
            }
        })
        .collect()
}

/// A native overlay hosting the page of one webview view.
pub(crate) struct SidebarWebview {
    pub host: crate::webview::WebviewHost,
    pub html: String,
    pub scripts: bool,
    pub bounds: [f64; 4],
    pub hidden: bool,
}

fn view_header(state: &WebviewViewState, fallback: &str) -> String {
    let mut header = state.title.clone().unwrap_or_else(|| fallback.to_owned());
    if let Some(description) = state.description.as_deref().filter(|text| !text.is_empty()) {
        header.push_str("  ");
        header.push_str(description);
    }
    if let Some((value, _)) = &state.badge {
        header.push_str(&format!("  ({value})"));
    }
    header
}

impl App {
    pub(super) fn handle_vsix_webview_view_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let text = |index: usize| {
            args.get(index)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("Invalid {method} argument"))
        };
        match method {
            "$registerWebviewViewProvider" => {
                let view_type = text(1)?;
                let options = args.get(2).unwrap_or(&Value::Null);
                let provider = Provider {
                    extension_root: args
                        .first()
                        .and_then(|extension| uri_value_to_path(&extension["location"])),
                    retain_context: options["retainContextWhenHidden"]
                        .as_bool()
                        .unwrap_or(false),
                };
                if let Some(old) = runtime.webview_views.register(view_type, provider) {
                    self.discard_webview_view(runtime, &old);
                }
                Ok(Value::Null)
            }
            "$unregisterWebviewViewProvider" => {
                if let Some(old) = runtime.webview_views.unregister(text(0)?) {
                    self.discard_webview_view(runtime, &old);
                }
                Ok(Value::Null)
            }
            "$setWebviewViewTitle" | "$setWebviewViewDescription" => {
                let value = args
                    .get(1)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .filter(|value| !value.is_empty());
                if let Some(state) = runtime.webview_views.view_for_handle_mut(text(0)?) {
                    if method == "$setWebviewViewTitle" {
                        state.title = value;
                    } else {
                        state.description = value;
                    }
                }
                Ok(Value::Null)
            }
            "$setWebviewViewBadge" => {
                let badge = args.get(1).filter(|badge| !badge.is_null()).and_then(|badge| {
                    Some((
                        badge["value"].as_u64()?,
                        badge["tooltip"].as_str().map(str::to_owned),
                    ))
                });
                if let Some(state) = runtime.webview_views.view_for_handle_mut(text(0)?) {
                    state.badge = badge;
                }
                Ok(Value::Null)
            }
            "$show" => {
                let handle = text(0)?;
                runtime.webview_views.pending_show = runtime
                    .webview_views
                    .views
                    .values()
                    .find(|state| state.handle == handle)
                    .map(|state| state.view_type.clone());
                Ok(Value::Null)
            }
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadWebviewViews", method);
                Err(format!(
                    "Graphite does not support MainThreadWebviewViews.{method}"
                ))
            }
        }
    }

    /// Tells the extension a view is gone and releases its page.
    fn discard_webview_view(&mut self, runtime: &mut Runtime, handle: &str) {
        runtime.webviews.panels.remove(handle);
        self.sidebar_webviews.remove(handle);
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostWebviewViews",
            "method": "$disposeWebviewView",
            "args": [handle],
        }));
    }

    pub(super) fn finish_vsix_webview_view(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let error = event["error"].as_str();
        runtime.webview_views.finish_resolve(id, error.is_some());
        if let Some(error) = error {
            self.status = error.to_owned();
        }
    }

    /// Asks the extension to resolve `view_type`: the panel entry makes `$setHtml`, `$setOptions` and
    /// `$postMessage` work for the new handle before the provider runs.
    pub(super) fn resolve_webview_view(&mut self, runtime: &mut Runtime, view_type: &str, title: &str) {
        let extension_root = runtime
            .webview_views
            .providers
            .get(view_type)
            .and_then(|provider| provider.extension_root.clone());
        let Some((id, handle)) = runtime.webview_views.begin_resolve(view_type) else {
            return;
        };
        let (enable_scripts, roots) =
            parse_webview_options(&Value::Null, extension_root.as_deref(), self.tree.root());
        runtime.webviews.panels.insert(
            handle.clone(),
            VsixWebviewPanel {
                handle: handle.clone(),
                view_type: view_type.to_owned(),
                title: title.to_owned(),
                html: DEFAULT_HTML.to_owned(),
                enable_scripts,
                local_resource_roots: roots.unwrap_or_default(),
                extension_root,
                outbound_messages: Vec::new(),
                visible: false,
            },
        );
        let sent = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostWebviewViews",
            "method": "$resolveWebviewView",
            "args": [handle, view_type, title, Value::Null],
            "cancellable": true,
        }));
        if let Err(error) = sent {
            runtime.webview_views.finish_resolve(id, true);
            self.status = error;
        }
    }

    pub(super) fn stop_sidebar_webviews(&mut self) {
        self.sidebar_webviews.clear();
    }

    /// The webview views of the active container with their absolute overlay rectangles.
    fn active_webview_slots(&self) -> Vec<(WebviewSlot, [f64; 4])> {
        if self.zen_mode
            || !self.view_preferences.primary_sidebar_visible
            || self.command_palette.is_some()
        {
            return Vec::new();
        }
        let Some(entry) = self.active_plugin_entry() else {
            return Vec::new();
        };
        let Some((width, height, lh)) = self.gpu.as_ref().map(|gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height, gpu.renderer.line_height())
        }) else {
            return Vec::new();
        };
        let (left, content_width) = self.primary_sidebar_bounds(width);
        let offset = self.activity_bar_vertical_offset();
        self.webview_slots_for(&entry, height, lh)
            .into_iter()
            .map(|slot| {
                let bounds = [
                    f64::from(left),
                    f64::from(offset + slot.top),
                    f64::from(content_width),
                    f64::from(slot.height),
                ];
                (slot, bounds)
            })
            .collect()
    }

    /// Slots of the webview views in `entry`, in side bar coordinates.
    pub(crate) fn webview_slots_for(
        &self,
        entry: &crate::app::window::activity_bar::PluginActivityEntry,
        window_height: f32,
        line_height: f32,
    ) -> Vec<WebviewSlot> {
        let sidebar_lh = self.sidebar_density_line_height(line_height);
        let ids = entry
            .views
            .iter()
            .filter(|view| view.kind == "webview")
            .map(|view| view.id.clone())
            .collect::<Vec<_>>();
        let has_tree = entry.views.iter().any(|view| view.kind != "webview");
        let area_bottom = window_height - self.activity_bar_vertical_offset() - line_height;
        webview_slots(&ids, has_tree, sidebar_lh, area_bottom)
    }

    /// The header text of a webview view: its (possibly extension-set) title, description and badge.
    pub(crate) fn webview_view_header(&self, view_id: &str, fallback: &str) -> String {
        self.plugins
            .vsix
            .as_ref()
            .and_then(|runtime| runtime.webview_views.view(view_id))
            .map_or_else(|| fallback.to_owned(), |state| view_header(state, fallback))
    }

    /// Resolves the visible webview views, hosts their pages over the side bar, and reports
    /// visibility changes to the extension. Runs every frame of the event loop.
    pub(crate) fn sync_sidebar_webviews(&mut self) {
        if let Some(view_id) = self
            .plugins
            .vsix
            .as_mut()
            .and_then(|runtime| runtime.webview_views.pending_show.take())
        {
            if let Some(entry) = self
                .plugin_activity_entries()
                .into_iter()
                .find(|entry| entry.views.iter().any(|view| view.id == view_id))
            {
                self.view_preferences.primary_sidebar_visible = true;
                self.activate_plugin_activity(entry);
            }
        }
        let Some(mut runtime) = self.plugins.vsix.take() else {
            self.stop_sidebar_webviews();
            return;
        };
        self.sync_sidebar_webviews_with(&mut runtime);
        self.plugins.vsix = Some(runtime);
    }

    fn sync_sidebar_webviews_with(&mut self, runtime: &mut Runtime) {
        let titles: BTreeMap<String, String> = self
            .active_plugin_entry()
            .map(|entry| {
                entry
                    .views
                    .into_iter()
                    .map(|view| (view.id, view.title))
                    .collect()
            })
            .unwrap_or_default();
        let slots = self.active_webview_slots();
        let shown: BTreeSet<String> = slots.iter().map(|(slot, _)| slot.view_id.clone()).collect();

        for (slot, _) in &slots {
            let title = titles.get(&slot.view_id).cloned().unwrap_or_default();
            self.resolve_webview_view_if_needed(runtime, &slot.view_id, &title);
        }

        // Visibility and hosting follow the slots; a hidden view keeps its page only when the
        // provider asked to retain its context.
        let view_types: Vec<String> = runtime.webview_views.views.keys().cloned().collect();
        for view_type in view_types {
            let visible = shown.contains(&view_type);
            let Some(state) = runtime.webview_views.views.get_mut(&view_type) else {
                continue;
            };
            let handle = state.handle.clone();
            if state.visible != visible {
                state.visible = visible;
                if let Some(panel) = runtime.webviews.panels.get_mut(&handle) {
                    panel.visible = visible;
                }
                let _ = runtime.session.send(json!({
                    "type": "call",
                    "actor": "ExtHostWebviewViews",
                    "method": "$onDidChangeWebviewViewVisibility",
                    "args": [handle, visible],
                }));
            }
            if !visible {
                let retain = runtime.webview_views.retains_context(&view_type);
                if retain {
                    if let Some(sidebar) = self.sidebar_webviews.get_mut(&handle) {
                        if !sidebar.hidden {
                            let _ = sidebar.host.set_visible(false);
                            sidebar.hidden = true;
                        }
                    }
                } else {
                    self.sidebar_webviews.remove(&handle);
                }
            }
        }

        let Some(window) = self.gpu.as_ref().map(|gpu| gpu.window.clone()) else {
            return;
        };
        for (slot, bounds) in slots {
            let Some(state) = runtime.webview_views.view(&slot.view_id) else {
                continue;
            };
            let handle = state.handle.clone();
            let Some(panel) = runtime.webviews.panels.get(&handle) else {
                continue;
            };
            let (html, scripts) = (panel.html.clone(), panel.enable_scripts);
            if let Some(sidebar) = self.sidebar_webviews.get_mut(&handle) {
                if sidebar.html == html && sidebar.scripts == scripts {
                    if sidebar.hidden {
                        let _ = sidebar.host.set_visible(true);
                        sidebar.hidden = false;
                    }
                    if sidebar.bounds != bounds || cfg!(target_os = "linux") {
                        let _ = sidebar.host.set_overlay_bounds(&window, bounds);
                        sidebar.bounds = bounds;
                    }
                    sidebar.host.pump();
                    continue;
                }
            }
            match crate::webview::WebviewHost::open_extension_overlay(
                &window, &html, bounds, scripts,
            ) {
                Ok(host) => {
                    self.sidebar_webviews.insert(
                        handle,
                        SidebarWebview {
                            host,
                            html,
                            scripts,
                            bounds,
                            hidden: false,
                        },
                    );
                }
                Err(error) => self.status = error,
            }
        }
        // Pages of views that no longer exist.
        let live: BTreeSet<String> = runtime
            .webview_views
            .views
            .values()
            .map(|state| state.handle.clone())
            .collect();
        self.sidebar_webviews.retain(|handle, _| live.contains(handle));
    }

    fn resolve_webview_view_if_needed(&mut self, runtime: &mut Runtime, view_type: &str, title: &str) {
        if runtime.webview_views.has_provider(view_type) {
            self.resolve_webview_view(runtime, view_type, title);
        }
    }

    /// Finds the active container entry, when a plugin container is selected.
    fn active_plugin_entry(&self) -> Option<crate::app::window::activity_bar::PluginActivityEntry> {
        let key = self.active_plugin_activity.as_deref()?;
        self.plugin_activity_entries()
            .into_iter()
            .find(|entry| entry.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(retain: bool) -> Provider {
        Provider {
            extension_root: None,
            retain_context: retain,
        }
    }

    #[test]
    fn a_view_resolves_once_per_provider_and_only_with_a_provider() {
        let mut views = WebviewViews::default();
        assert!(views.begin_resolve("sample.panel").is_none());
        views.register("sample.panel", provider(false));
        let (id, handle) = views.begin_resolve("sample.panel").unwrap();
        assert!(is_view_reply(id));
        assert!(views.begin_resolve("sample.panel").is_none());
        assert!(views.busy());
        views.finish_resolve(id, false);
        assert!(!views.busy());
        assert_eq!(views.view("sample.panel").unwrap().handle, handle);
    }

    #[test]
    fn a_failed_resolution_is_not_retried_until_the_provider_is_registered_again() {
        let mut views = WebviewViews::default();
        views.register("sample.panel", provider(false));
        let (id, handle) = views.begin_resolve("sample.panel").unwrap();
        views.finish_resolve(id, true);
        assert!(views.view("sample.panel").unwrap().failed);
        assert!(views.begin_resolve("sample.panel").is_none());
        assert_eq!(views.register("sample.panel", provider(false)), Some(handle));
        assert!(views.begin_resolve("sample.panel").is_some());
    }

    #[test]
    fn unregistering_returns_the_handle_to_dispose_and_forgets_the_view() {
        let mut views = WebviewViews::default();
        views.register("sample.panel", provider(true));
        assert!(views.retains_context("sample.panel"));
        let (_, handle) = views.begin_resolve("sample.panel").unwrap();
        assert!(views.is_view_handle(&handle));
        assert_eq!(views.unregister("sample.panel"), Some(handle));
        assert!(!views.has_provider("sample.panel"));
        assert!(views.view("sample.panel").is_none());
        assert!(views.unregister("sample.panel").is_none());
    }

    #[test]
    fn disposing_the_webview_forgets_its_view() {
        let mut views = WebviewViews::default();
        views.register("sample.panel", provider(false));
        let (_, handle) = views.begin_resolve("sample.panel").unwrap();
        views.forget_handle(&handle);
        assert!(!views.is_view_handle(&handle));
        assert!(views.begin_resolve("sample.panel").is_some());
    }

    #[test]
    fn header_combines_title_description_and_badge() {
        let mut state = WebviewViewState {
            view_type: "sample.panel".into(),
            handle: "h".into(),
            title: None,
            description: None,
            badge: None,
            visible: false,
            failed: false,
        };
        assert_eq!(view_header(&state, "Panel"), "Panel");
        state.title = Some("Notes".into());
        state.description = Some("3 open".into());
        state.badge = Some((3, None));
        assert_eq!(view_header(&state, "Panel"), "Notes  3 open  (3)");
    }

    #[test]
    fn slots_split_the_area_and_leave_room_for_trees() {
        let ids = vec!["a".to_owned(), "b".to_owned()];
        let slots = webview_slots(&ids, false, 20.0, 480.0);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].header_y, 80.0);
        assert_eq!(slots[0].top, 105.0);
        assert_eq!(slots[1].header_y, 280.0);
        assert_eq!(slots[1].height, 175.0);
        let mixed = webview_slots(&ids[..1], true, 20.0, 480.0);
        assert_eq!(mixed[0].header_y, 280.0);
        assert!(webview_slots(&[], true, 20.0, 480.0).is_empty());
        assert_eq!(webview_slots(&ids, false, 20.0, 50.0)[0].height, 0.0);
    }
}
