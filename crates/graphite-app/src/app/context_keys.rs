use super::command_execution::CommandError;
use super::command_registry::CommandTarget;
use super::editor_groups::ViewId;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WidgetKind {
    Find,
    QuickOpen,
    Suggest,
    QuickFix,
    Snippet,
    ProjectSearch,
    Terminal,
    Scm,
    Branch,
    Diff,
    Conflict,
    Close,
    WindowClose,
    Commit,
}
pub(crate) struct WidgetScope {
    pub kind: WidgetKind,
    pub parent: Option<ViewId>,
    values: HashMap<String, String>,
}

/// Context values scoped to the editor view, with shared global defaults.
#[derive(Default)]
pub(crate) struct ContextKeyService {
    global: HashMap<String, String>,
    views: HashMap<ViewId, HashMap<String, String>>,
    widgets: HashMap<u64, WidgetScope>,
    next_widget: u64,
}

impl ContextKeyService {
    pub fn close_widget(&mut self, kind: WidgetKind) {
        if let Some(id) = self.widget_id(kind) {
            self.widgets.remove(&id);
        }
    }
    pub fn widget_id(&self, kind: WidgetKind) -> Option<u64> {
        self.widgets
            .iter()
            .find(|(_, w)| w.kind == kind)
            .map(|(id, _)| *id)
    }
    pub fn widget(&self, id: u64) -> Option<&WidgetScope> {
        self.widgets.get(&id)
    }
    pub fn sync_widget(
        &mut self,
        kind: WidgetKind,
        visible: bool,
        parent: Option<ViewId>,
        values: HashMap<String, String>,
    ) {
        let id = self.widget_id(kind);
        if !visible {
            if let Some(id) = id {
                self.widgets.remove(&id);
            }
            return;
        }
        if let Some(id) = id {
            self.widgets.get_mut(&id).unwrap().values = values;
        } else {
            self.next_widget += 1;
            self.widgets.insert(
                self.next_widget,
                WidgetScope {
                    kind,
                    parent,
                    values,
                },
            );
        }
    }
    pub fn resolve_target(&self, target: CommandTarget) -> HashMap<String, String> {
        match target {
            CommandTarget::Global => self.resolve(None),
            CommandTarget::View(id) => self.resolve(Some(ViewId(id))),
            CommandTarget::Widget(id) => {
                if let Some(widget) = self.widgets.get(&id) {
                    let mut values = self.resolve(widget.parent);
                    values.extend(widget.values.clone());
                    values
                } else {
                    self.resolve(None)
                }
            }
        }
    }
    pub(super) fn replace_global(&mut self, values: HashMap<String, String>) {
        self.global = values;
    }

    pub(super) fn replace_view(&mut self, view: ViewId, values: HashMap<String, String>) {
        self.views.insert(view, values);
    }

    pub(super) fn retain_views(&mut self, live: &std::collections::HashSet<ViewId>) {
        self.views.retain(|id, _| live.contains(id));
    }

    pub(super) fn resolve(&self, target: Option<ViewId>) -> HashMap<String, String> {
        let mut resolved = self.global.clone();
        if let Some(values) = target.and_then(|id| self.views.get(&id)) {
            resolved.extend(values.clone());
        }
        resolved
    }
}

impl super::App {
    pub(crate) fn command_target_view(
        &self,
        target: CommandTarget,
    ) -> Result<Option<ViewId>, CommandError> {
        let view = match target {
            CommandTarget::Global => None,
            CommandTarget::View(id) => Some(ViewId(id)),
            CommandTarget::Widget(id) => {
                self.context_keys
                    .widget(id)
                    .ok_or(CommandError::TargetClosed)?
                    .parent
            }
        };
        if view.is_some_and(|id| self.editor_groups.view(id).is_none()) {
            Err(CommandError::TargetClosed)
        } else {
            Ok(view)
        }
    }
    pub(super) fn ui_target_for_command(&self, id: &str) -> CommandTarget {
        if let Some(kind) = super::command_registry::registry()
            .get(id)
            .and_then(|entry| entry.widget)
        {
            return self
                .context_keys
                .widget_id(kind)
                .map(CommandTarget::Widget)
                .unwrap_or(CommandTarget::Global);
        }
        if self.extension_host.commands.get(id).is_some_and(|command| {
            !command
                .metadata
                .targets
                .contains(&super::command_registry::TargetKind::View)
                && command
                    .metadata
                    .targets
                    .contains(&super::command_registry::TargetKind::Global)
        }) {
            return CommandTarget::Global;
        }
        CommandTarget::View(self.editor_groups.active_view().id.0)
    }
    pub(crate) fn refresh_command_context(&mut self) {
        let live = self.editor_groups.view_ids();
        self.context_keys.retain_views(&live);
        // An owner closing also closes its transient widgets, before evaluating context.
        let orphaned: Vec<_> = self
            .context_keys
            .widgets
            .values()
            .filter(|w| w.parent.is_some_and(|id| !live.contains(&id)))
            .map(|w| w.kind)
            .collect();
        for kind in orphaned {
            match kind {
                WidgetKind::Find => self.local_search = None,
                WidgetKind::QuickOpen => self.quick_open = None,
                WidgetKind::Suggest => self.completion_items.clear(),
                WidgetKind::QuickFix => self.quick_fix_visible = false,
                WidgetKind::Snippet => self.snippet_query = None,
                WidgetKind::ProjectSearch => self.search_query = None,
                _ => {}
            }
            if let Some(id) = self.context_keys.widget_id(kind) {
                self.context_keys.widgets.remove(&id);
            }
        }
        let flags = [
            (
                WidgetKind::Find,
                "findWidgetVisible",
                self.local_search.is_some(),
                true,
            ),
            (
                WidgetKind::QuickOpen,
                "quickOpenVisible",
                self.quick_open.is_some(),
                true,
            ),
            (
                WidgetKind::Suggest,
                "suggestWidgetVisible",
                !self.completion_items.is_empty(),
                true,
            ),
            (
                WidgetKind::QuickFix,
                "quickFixVisible",
                self.quick_fix_visible,
                true,
            ),
            (
                WidgetKind::Snippet,
                "snippetVisible",
                self.snippet_query.is_some(),
                true,
            ),
            (
                WidgetKind::ProjectSearch,
                "projectSearchVisible",
                self.search_query.is_some(),
                true,
            ),
            (
                WidgetKind::Terminal,
                "terminalFocus",
                self.terminal_focused,
                false,
            ),
            (WidgetKind::Scm, "scmVisible", self.scm_visible, false),
            (
                WidgetKind::Branch,
                "branchPickerVisible",
                self.branch_picker,
                false,
            ),
            (
                WidgetKind::Diff,
                "diffVisible",
                self.diff_view.is_some(),
                false,
            ),
            (
                WidgetKind::Conflict,
                "conflictVisible",
                self.conflict_view.is_some(),
                false,
            ),
            (
                WidgetKind::Close,
                "closeConfirmation",
                self.close_confirm,
                true,
            ),
            (
                WidgetKind::WindowClose,
                "windowCloseConfirmation",
                self.close_window_confirm,
                false,
            ),
            (
                WidgetKind::Commit,
                "commitPromptVisible",
                self.scm_commit_prompt,
                false,
            ),
        ];
        let mut global = HashMap::new();
        global.insert(
            "extensionHostEnabled".into(),
            self.extension_host.enabled.to_string(),
        );
        global.insert(
            "extensionHostReady".into(),
            self.extension_host
                .supervisor
                .as_ref()
                .is_some_and(|host| host.state == graphite_extension_host::HostState::Ready)
                .to_string(),
        );
        for (_, key, value, _) in flags {
            global.insert(key.into(), value.to_string());
        }
        global.insert("diffLoading".into(), self.diff_loading.to_string());
        global.insert("multiDiffVisible".into(), self.multi_diff.is_some().to_string());
        global.insert(
            "mergeEditorVisible".into(),
            self.merge_editor.is_some().to_string(),
        );
        global.insert("pluginsVisible".into(), self.plugins.visible.to_string());
        global.insert("pluginsFocus".into(), self.plugins.focused.to_string());
        global.insert(
            "findReplace".into(),
            self.local_search
                .as_ref()
                .is_some_and(|s| s.replace)
                .to_string(),
        );
        global.insert(
            "projectSearchReplace".into(),
            self.search_replacement.is_some().to_string(),
        );
        global.insert(
            "projectSearchDetails".into(),
            self.search_filters.details.to_string(),
        );
        global.insert(
            "explorerFocus".into(),
            (self.explorer_focused
                && self.view_preferences.primary_sidebar_visible
                && self.quick_open.is_none()
                && self.command_palette.is_none()
                && self.go_picker.is_none())
            .to_string(),
        );
        global.insert(
            "explorerHasSelection".into(),
            self.explorer_has_selection().to_string(),
        );
        global.insert(
            "quickOpenHasItems".into(),
            self.quick_open
                .as_ref()
                .is_some_and(|s| !s.paths.is_empty())
                .to_string(),
        );
        global.insert(
            "quickFixHasItems".into(),
            (!self.quick_fix_actions.is_empty()).to_string(),
        );
        global.insert(
            "searchHasItems".into(),
            (!self.search_results.is_empty()).to_string(),
        );
        global.insert(
            "hasClosedTabs".into(),
            (!self.closed_tabs.is_empty()).to_string(),
        );
        global.insert(
            "commandPending".into(),
            (!self.command_runtime.pending.is_empty()).to_string(),
        );
        let overlays = self.local_search.is_some()
            || self.quick_open.is_some()
            || self.search_query.is_some()
            || self.snippet_query.is_some();
        let plugin_keyboard = self.plugin_owns_keyboard();
        let focused = self.editor_groups.active_view().id;
        for view in self.editor_groups.all_views() {
            let document = &self.documents[view.document];
            let editor = document.buffer.view(&view.selection_state);
            self.context_keys.replace_view(
                view.id,
                HashMap::from([
                    ("documentAvailable".into(), "true".into()),
                    (
                        "documentSaving".into(),
                        self.saving.document_pending(view.document).to_string(),
                    ),
                    (
                        "editorTextFocus".into(),
                        (view.id == focused
                            && !self.terminal_focused
                            && !self.explorer_focused
                            && !overlays
                            && !plugin_keyboard)
                            .to_string(),
                    ),
                    ("editorReadonly".into(), editor.is_read_only().to_string()),
                    (
                        "editorWritable".into(),
                        (!editor.is_read_only()).to_string(),
                    ),
                    (
                        "editorHasSelection".into(),
                        editor
                            .selections()
                            .iter()
                            .any(|s| s.anchor != s.head)
                            .to_string(),
                    ),
                    (
                        "multipleSelections".into(),
                        (editor.selections().len() > 1).to_string(),
                    ),
                    (
                        "documentHasPath".into(),
                        editor.path().is_some().to_string(),
                    ),
                    ("canUndo".into(), editor.can_undo().to_string()),
                    ("canRedo".into(), editor.can_redo().to_string()),
                    (
                        "canToggleLineComment".into(),
                        self.active_comment_tokens()
                            .is_some_and(|comments| comments.line.is_some())
                            .to_string(),
                    ),
                    (
                        "canToggleBlockComment".into(),
                        self.active_comment_tokens()
                            .is_some_and(|comments| comments.block.is_some())
                            .to_string(),
                    ),
                ]),
            );
        }
        self.context_keys.replace_global(global);
        let owner = self.command_view_ref().id;
        for (kind, _, visible, per_view) in flags {
            let items = match kind {
                WidgetKind::QuickOpen => self
                    .quick_open
                    .as_ref()
                    .map(|state| state.paths.len())
                    .unwrap_or(0),
                WidgetKind::QuickFix => self.quick_fix_actions.len(),
                WidgetKind::Suggest => self.completion_items.len(),
                WidgetKind::ProjectSearch => self.search_results.len(),
                WidgetKind::Snippet => self.snippets.len(),
                WidgetKind::Terminal => self.terminals.len(),
                WidgetKind::Scm => self.scm_snapshot.files.len(),
                WidgetKind::Branch => self.branch_picker_entries().len(),
                _ => 0,
            };
            self.context_keys.sync_widget(
                kind,
                visible,
                per_view.then_some(owner),
                HashMap::from([
                    ("widgetVisible".into(), visible.to_string()),
                    ("widgetHasItems".into(), (items > 0).to_string()),
                    ("widgetItemCount".into(), items.to_string()),
                ]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::editor_groups::EditorGroups;
    use crate::DocumentId;

    #[test]
    fn view_values_override_global_and_do_not_leak_to_other_views() {
        let mut groups = EditorGroups::new(DocumentId(1));
        let first = groups.active_view().id;
        groups.split(super::super::editor_groups::SplitDirection::Vertical);
        let second = groups.active_view().id;
        let mut service = ContextKeyService::default();
        service.replace_global(HashMap::from([("key".into(), "global".into())]));
        service.replace_view(first, HashMap::from([("key".into(), "first".into())]));
        assert_eq!(service.resolve(Some(first))["key"], "first");
        assert_eq!(service.resolve(Some(second))["key"], "global");
        assert_eq!(service.resolve(None)["key"], "global");
        service.retain_views(&std::collections::HashSet::from([second]));
        assert_eq!(service.resolve(Some(first))["key"], "global");
    }
}
