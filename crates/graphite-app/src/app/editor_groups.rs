//! Per-group presentation state referencing shared documents by stable identity.
use super::document_edit::EditorSession;
use super::{App, DocumentId};
use graphite_text::{PositionEdit, Selection};

pub(super) use graphite_text::ViewId;

fn next_view_id() -> ViewId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_VIEW_ID: AtomicU64 = AtomicU64::new(1);
    ViewId(NEXT_VIEW_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct View {
    pub id: ViewId,
    pub document: DocumentId,
    pub selection_state: graphite_text::SelectionState,
    pub ime_composing: bool,
    pub ime_preedit: String,
    pub smart_selection_history: Vec<Selection>,
    pub smart_selection_guard: Option<(DocumentId, u64, Selection)>,
    pub scroll_line: usize,
}

pub(crate) struct ClosedTab {
    pub id: DocumentId,
    pub document: Option<crate::document_store::ClosedDocument>,
    pub view: View,
}

#[derive(Clone, Debug)]
struct Group {
    id: u64,
    tabs: Vec<View>,
    active: usize,
}

#[derive(Clone, Debug)]
enum Layout {
    Leaf(u64),
    Split {
        direction: SplitDirection,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GroupRect {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Each view owns its selections and scroll; multiple views may refer to one document.
pub(crate) struct EditorGroups {
    groups: Vec<Group>,
    layout: Layout,
    focused: u64,
    next_id: u64,
}

impl EditorGroups {
    pub fn new(document: DocumentId) -> Self {
        let view = View {
            id: next_view_id(),
            document,
            selection_state: Default::default(),
            ime_composing: false,
            ime_preedit: String::new(),
            smart_selection_history: Vec::new(),
            smart_selection_guard: None,
            scroll_line: 0,
        };
        Self {
            groups: vec![Group {
                id: 0,
                tabs: vec![view],
                active: 0,
            }],
            layout: Layout::Leaf(0),
            focused: 0,
            next_id: 1,
        }
    }

    fn group(&self, id: u64) -> Option<&Group> {
        self.groups.iter().find(|group| group.id == id)
    }
    fn group_mut(&mut self, id: u64) -> Option<&mut Group> {
        self.groups.iter_mut().find(|group| group.id == id)
    }
    pub(crate) fn group_for_view(&self, id: ViewId) -> Option<u64> {
        self.groups
            .iter()
            .find(|group| group.tabs.iter().any(|view| view.id == id))
            .map(|group| group.id)
    }
    pub(crate) fn view(&self, id: ViewId) -> Option<&View> {
        self.groups
            .iter()
            .flat_map(|g| &g.tabs)
            .find(|v| v.id == id)
    }
    pub(crate) fn view_mut(&mut self, id: ViewId) -> Option<&mut View> {
        self.groups
            .iter_mut()
            .flat_map(|g| &mut g.tabs)
            .find(|v| v.id == id)
    }
    pub(crate) fn all_views(&self) -> impl Iterator<Item = &View> {
        self.groups.iter().flat_map(|g| &g.tabs)
    }
    pub fn focused(&self) -> u64 {
        self.focused
    }
    pub fn view_count(&self, document: DocumentId) -> usize {
        self.groups
            .iter()
            .flat_map(|group| &group.tabs)
            .filter(|view| view.document == document)
            .count()
    }
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
    pub(super) fn view_ids(&self) -> std::collections::HashSet<ViewId> {
        self.groups
            .iter()
            .flat_map(|group| group.tabs.iter().map(|view| view.id))
            .collect()
    }
    pub(super) fn for_each_view_mut(&mut self, mut callback: impl FnMut(u64, &mut View)) {
        for group in &mut self.groups {
            for view in &mut group.tabs {
                callback(group.id, view);
            }
        }
    }
    pub(super) fn active_view(&self) -> &View {
        let group = self.group(self.focused).expect("focused group exists");
        &group.tabs[group.active]
    }
    pub(crate) fn active_view_mut(&mut self) -> &mut View {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        &mut group.tabs[group.active]
    }
    /// Add a background document to the focused group without changing focus.
    pub(super) fn add_background_document(&mut self, document: DocumentId) {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        if !group.tabs.iter().any(|view| view.document == document) {
            group.tabs.push(View {
                id: next_view_id(),
                document,
                selection_state: Default::default(),
                ime_composing: false,
                ime_preedit: String::new(),
                smart_selection_history: Vec::new(),
                smart_selection_guard: None,
                scroll_line: 0,
            });
        }
    }
    pub(super) fn views(&self, id: u64) -> Option<(&[View], usize)> {
        self.group(id)
            .map(|group| (group.tabs.as_slice(), group.active))
    }
    pub fn focus(&mut self, id: u64) -> bool {
        if self.group(id).is_none() {
            return false;
        }
        self.focused = id;
        true
    }
    pub fn activate_document(
        &mut self,
        document: DocumentId,
        selection_state: graphite_text::SelectionState,
        scroll_line: usize,
    ) {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        if let Some(index) = group.tabs.iter().position(|view| view.document == document) {
            group.active = index;
        } else {
            group.tabs.push(View {
                id: next_view_id(),
                document,
                selection_state,
                ime_composing: false,
                ime_preedit: String::new(),
                smart_selection_history: Vec::new(),
                smart_selection_guard: None,
                scroll_line,
            });
            group.active = group.tabs.len() - 1;
        }
    }
    pub(super) fn restore_closed_view(&mut self, mut view: View) {
        view.id = next_view_id();
        view.ime_composing = false;
        view.ime_preedit.clear();
        view.smart_selection_history.clear();
        view.smart_selection_guard = None;
        *self.active_view_mut() = view;
    }
    pub fn select_tab(&mut self, index: usize) -> bool {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        if index >= group.tabs.len() {
            return false;
        }
        group.active = index;
        true
    }
    pub(super) fn split(&mut self, direction: SplitDirection) -> u64 {
        self.split_view(self.active_view().id, direction)
    }
    pub(super) fn split_view(&mut self, source: ViewId, direction: SplitDirection) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let group = self.group_for_view(source).expect("source view exists");
        let mut view = self.view(source).unwrap().clone();
        view.id = next_view_id();
        view.ime_preedit.clear();
        view.ime_composing = false;
        view.smart_selection_history.clear();
        view.smart_selection_guard = None;
        self.groups.push(Group {
            id,
            tabs: vec![view],
            active: 0,
        });
        assert!(self.layout.split_leaf(group, id, direction));
        self.focused = id;
        id
    }
    /// A dirty document may close here only if another view keeps it open.
    pub fn close_active(&mut self, dirty: bool) -> bool {
        self.close_view(self.active_view().id, dirty)
    }
    pub(crate) fn close_view(&mut self, target: ViewId, dirty: bool) -> bool {
        let Some(view) = self.view(target) else {
            return false;
        };
        if dirty && self.view_count(view.document) == 1 {
            return false;
        }
        let id = self.group_for_view(target).unwrap();
        let group = self.group_mut(id).unwrap();
        let index = group
            .tabs
            .iter()
            .position(|view| view.id == target)
            .unwrap();
        if group.tabs.len() > 1 {
            let active_id = group.tabs[group.active].id;
            group.tabs.remove(index);
            group.active = group
                .tabs
                .iter()
                .position(|view| view.id == active_id)
                .unwrap_or(index.min(group.tabs.len() - 1));
        } else if self.groups.len() > 1 {
            self.layout = self.layout.clone().remove(id).unwrap();
            self.groups.retain(|group| group.id != id);
            if self.focused == id {
                self.focused = self.groups[0].id;
            }
        } else {
            return false;
        }
        true
    }
    pub fn remap_other_views(&mut self, document: DocumentId, changes: &[PositionEdit]) {
        let focused = self.focused;
        for group in &mut self.groups {
            for (index, view) in group.tabs.iter_mut().enumerate() {
                if view.document != document || (group.id == focused && group.active == index) {
                    continue;
                }
                view.selection_state.remap(changes, usize::MAX);
            }
        }
    }
    pub(super) fn rectangles(&self, bounds: GroupRect) -> Vec<GroupRect> {
        let mut out = Vec::new();
        self.layout.rectangles(bounds, &mut out);
        out
    }
}

impl Layout {
    fn split_leaf(&mut self, target: u64, id: u64, direction: SplitDirection) -> bool {
        match self {
            Self::Leaf(current) if *current == target => {
                *self = Self::Split {
                    direction,
                    first: Box::new(Self::Leaf(target)),
                    second: Box::new(Self::Leaf(id)),
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split_leaf(target, id, direction) || second.split_leaf(target, id, direction)
            }
        }
    }
    fn remove(self, target: u64) -> Option<Self> {
        match self {
            Self::Leaf(id) => (id != target).then_some(Self::Leaf(id)),
            Self::Split {
                direction,
                first,
                second,
            } => match (first.remove(target), second.remove(target)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    direction,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            },
        }
    }
    fn rectangles(&self, rect: GroupRect, out: &mut Vec<GroupRect>) {
        match self {
            Self::Leaf(id) => out.push(GroupRect { id: *id, ..rect }),
            Self::Split {
                direction,
                first,
                second,
            } => {
                let (mut a, mut b) = (rect, rect);
                match direction {
                    SplitDirection::Vertical => {
                        a.width *= 0.5;
                        b.x += a.width;
                        b.width -= a.width;
                    }
                    SplitDirection::Horizontal => {
                        a.height *= 0.5;
                        b.y += a.height;
                        b.height -= a.height;
                    }
                }
                first.rectangles(a, out);
                second.rectangles(b, out);
            }
        }
    }
}

impl App {
    pub(super) fn command_group_rect(&self, width: f32, height: f32, lh: f32) -> GroupRect {
        let group = self
            .editor_groups
            .group_for_view(self.command_view_ref().id)
            .unwrap();
        self.group_rectangles(width, height, lh)
            .into_iter()
            .find(|rect| rect.id == group)
            .unwrap()
    }
    pub(crate) fn command_view_ref(&self) -> &View {
        self.command_runtime
            .view
            .and_then(|id| self.editor_groups.view(id))
            .unwrap_or_else(|| self.editor_groups.active_view())
    }
    pub(crate) fn command_view_mut(&mut self) -> &mut View {
        let id = self
            .command_runtime
            .view
            .unwrap_or(self.editor_groups.active_view().id);
        self.editor_groups
            .view_mut(id)
            .expect("validated command view exists")
    }
    pub(crate) fn command_document(&self) -> DocumentId {
        self.command_view_ref().document
    }
    pub(crate) fn active_document(&self) -> DocumentId {
        self.editor_groups.active_view().document
    }
    #[cfg(test)]
    pub(crate) fn document_at(&self, index: usize) -> DocumentId {
        self.documents
            .ids()
            .nth(index)
            .expect("test document exists")
    }
    pub(super) fn resolve_document(&self, id: DocumentId) -> Option<DocumentId> {
        self.documents.resolve(id)
    }

    #[cfg(test)]
    pub(crate) fn test_editor(&self, index: usize) -> graphite_text::EditorRead<'_> {
        let document = self.document_at(index);
        let view = if self.active_document() == document {
            self.editor_groups.active_view()
        } else {
            self.editor_groups
                .groups
                .iter()
                .flat_map(|g| &g.tabs)
                .find(|v| v.document == document)
                .unwrap()
        };
        self.documents[document].buffer.view(&view.selection_state)
    }
    #[cfg(test)]
    pub(crate) fn test_editor_mut(&mut self, index: usize) -> EditorSession<'_> {
        let document = self.document_at(index);
        self.activate_editor_document(document);
        self.editor_mut()
    }
    pub(crate) fn editor(&self) -> graphite_text::EditorRead<'_> {
        let view = self.command_view_ref();
        self.documents[view.document]
            .buffer
            .view(&view.selection_state)
    }
    pub(crate) fn editor_mut(&mut self) -> EditorSession<'_> {
        let active = self.command_view_ref().id;
        let document = self.command_document();
        let mut origin = None;
        let mut peers = Vec::new();
        for group in &mut self.editor_groups.groups {
            for view in &mut group.tabs {
                if view.id == active {
                    origin = Some(view);
                } else if view.document == document {
                    peers.push(view);
                }
            }
        }
        let view = origin.expect("active view exists");
        let tab = self
            .documents
            .get_mut(document)
            .expect("active document exists");
        if self.document_events.needs_record(document, tab) {
            if let Ok(snapshot) = super::services::snapshot_document(document, tab) {
                self.document_events.record(snapshot);
            }
        }
        EditorSession {
            document,
            language: tab.language.map(|l| {
                match l {
                    graphite_syntax::Language::Rust => "rust",
                    graphite_syntax::Language::Python => "python",
                    graphite_syntax::Language::JavaScript => "javascript",
                    graphite_syntax::Language::Json => "json",
                }
                .to_owned()
            }),
            format_dirty: tab.saved_format != (tab.encoding, tab.line_ending),
            events: &mut self.document_events,
            editor: tab.buffer.edit(&mut view.selection_state, active),
            peers,
            schema_revision: &mut tab.schema_revision,
            schema_manager: self.json_schema_manager.as_ref(),
            lsp_revision: &mut tab.lsp_revision,
            manager: &mut self.lsp_manager,
            root: self.tree.root(),
            sender: self.lsp_sender.clone(),
            translator: self.translator,
        }
    }
    pub(crate) fn activate_editor_document(&mut self, document: DocumentId) {
        if self.documents.get(document).is_none() {
            return;
        }
        let current_id = self.active_document();
        let target_id = document;
        if current_id != target_id {
            self.recent_documents
                .retain(|id| *id != target_id && *id != current_id);
            self.recent_documents.insert(0, target_id);
            self.recent_documents.insert(1, current_id);
            self.recent_documents.truncate(100);
        }
        self.editor_groups
            .activate_document(document, Default::default(), 0);
        if self.command_runtime.current.is_some() {
            self.command_runtime.view = Some(self.editor_groups.active_view().id);
        }
        self.invalidate_lsp_requests();
        self.request_inlay_hints();
    }

    pub(super) fn active_view_needs_confirmation(&self) -> bool {
        self.documents[self.command_document()].is_dirty()
            && self.editor_groups.view_count(self.command_document()) <= 1
    }

    pub(super) fn split_editor_group(&mut self, direction: SplitDirection) -> u64 {
        self.editor_groups
            .split_view(self.command_view_ref().id, direction)
    }

    pub(super) fn focus_editor_group(&mut self, id: u64) -> bool {
        if !self.editor_groups.focus(id) {
            return false;
        }
        if self.command_runtime.current.is_some() {
            self.command_runtime.view = Some(self.editor_groups.active_view().id);
        }
        true
    }

    pub(super) fn focus_next_editor_group(&mut self) {
        let rects = self.editor_groups.rectangles(GroupRect {
            id: 0,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        let current = rects
            .iter()
            .position(|rect| rect.id == self.editor_groups.focused())
            .unwrap_or(0);
        self.focus_editor_group(rects[(current + 1) % rects.len()].id);
    }

    pub(super) fn remap_document_views(&mut self, document: DocumentId, previous_revision: u64) {
        if let Some(tab) = self.documents.get(document) {
            if tab.buffer.revision() != previous_revision {
                self.editor_groups.for_each_view_mut(|_, view| {
                    if view.document == document {
                        view.selection_state
                            .remap(tab.buffer.last_position_edits(), tab.buffer.len_chars());
                    }
                });
                for closed in &mut self.closed_tabs {
                    if closed.id == document && closed.document.is_none() {
                        closed
                            .view
                            .selection_state
                            .remap(tab.buffer.last_position_edits(), tab.buffer.len_chars());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_ids_are_unique_and_stable_when_focus_changes() {
        let mut groups = EditorGroups::new(DocumentId(0));
        let original = groups.active_view().id;
        let other = groups.split(SplitDirection::Vertical);
        let split = groups.active_view().id;
        assert_ne!(original, split);
        assert!(groups.focus(0));
        assert_eq!(groups.active_view().id, original);
        assert!(groups.focus(other));
        assert_eq!(groups.active_view().id, split);
        let replacement = EditorGroups::new(DocumentId(0)).active_view().id;
        assert_ne!(replacement, original);
        assert_ne!(replacement, split);
    }

    #[test]
    fn document_ids_survive_closing_and_reopening_another_document() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&second, "second").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first);
        let first_id = app.active_document();
        app.open_file(second);
        let second_id = app.active_document();
        assert_ne!(first_id, second_id);
        assert_eq!(app.documents.resolve(first_id), Some(first_id));
        assert_eq!(app.documents.resolve(second_id), Some(second_id));
        app.close_active_tab();
        assert!(app.documents.iter().any(|tab| tab.id == first_id));
        assert!(!app.documents.iter().any(|tab| tab.id == second_id));
        assert_eq!(app.documents.resolve(second_id), None);
        app.reopen_closed_tab();
        assert_eq!(app.active_document(), second_id);
        assert_eq!(
            app.documents.resolve(second_id),
            Some(app.active_document())
        );
        assert!(app.documents.iter().any(|tab| tab.id == first_id));
    }

    #[test]
    fn closing_an_earlier_document_keeps_other_groups_on_the_same_document() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&second, "second").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first);
        app.open_file(second);
        let second_id = app.active_document();
        let other = app.split_editor_group(SplitDirection::Vertical);
        assert!(app.focus_editor_group(0));
        app.activate_editor_document(app.document_at(0));
        app.close_active_tab();
        assert!(app.focus_editor_group(other));
        assert_eq!(app.active_document(), second_id);
        assert_eq!(app.documents.resolve(second_id), Some(second_id));
        assert_eq!(app.documents[app.active_document()].buffer.text(), "second");
    }

    #[test]
    fn shared_document_keeps_independent_views_and_maps_edits() {
        let mut groups = EditorGroups::new(DocumentId(0));
        groups
            .active_view_mut()
            .selection_state
            .set_selections(vec![Selection { anchor: 4, head: 4 }], 10);
        let right = groups.split(SplitDirection::Vertical);
        groups
            .active_view_mut()
            .selection_state
            .set_selections(vec![Selection { anchor: 1, head: 2 }], 10);
        groups.active_view_mut().scroll_line = 10;
        groups.remap_other_views(
            DocumentId(0),
            &[PositionEdit {
                start: 1,
                end: 1,
                inserted_chars: 3,
            }],
        );
        assert!(groups.focus(0));
        assert_eq!(groups.active_view().selection_state.selections()[0].head, 7);
        assert_eq!(groups.active_view().scroll_line, 0);
        assert!(groups.focus(right));
        assert_eq!(groups.active_view().selection_state.selections()[0].head, 2);
        assert_eq!(groups.active_view().scroll_line, 10);
    }

    #[test]
    fn split_geometry_and_dirty_closing_keep_shared_documents() {
        let mut groups = EditorGroups::new(DocumentId(0));
        let right = groups.split(SplitDirection::Vertical);
        let lower = groups.split(SplitDirection::Horizontal);
        let rects = groups.rectangles(GroupRect {
            id: 0,
            x: 10.,
            y: 20.,
            width: 100.,
            height: 80.,
        });
        assert_eq!(
            rects,
            vec![
                GroupRect {
                    id: 0,
                    x: 10.,
                    y: 20.,
                    width: 50.,
                    height: 80.
                },
                GroupRect {
                    id: right,
                    x: 60.,
                    y: 20.,
                    width: 50.,
                    height: 40.
                },
                GroupRect {
                    id: lower,
                    x: 60.,
                    y: 60.,
                    width: 50.,
                    height: 40.
                },
            ]
        );
        assert!(groups.close_active(true));
        assert_eq!(groups.rectangles(rects[0]).len(), 2);
        assert!(groups.focus(right));
        assert!(groups.close_active(true));
        assert!(!groups.close_active(true));
    }

    #[test]
    fn tabs_belong_to_their_group_and_last_dirty_view_cannot_close() {
        let mut groups = EditorGroups::new(DocumentId(4));
        groups.activate_document(
            DocumentId(9),
            graphite_text::SelectionState::from_selections(
                vec![Selection { anchor: 2, head: 4 }],
                10,
            ),
            5,
        );
        let other = groups.split(SplitDirection::Horizontal);
        assert_eq!(groups.views(other).unwrap().0.len(), 1);
        assert_eq!(groups.active_view().document, DocumentId(9));
        assert!(groups.focus(0));
        assert_eq!(groups.views(0).unwrap().0.len(), 2);
        assert!(groups.select_tab(0));
        assert_eq!(groups.active_view().document, DocumentId(4));
        assert!(!groups.close_active(true));
        assert!(groups.select_tab(1));
        assert!(groups.close_active(true));
        assert_eq!(groups.active_view().document, DocumentId(4));
        assert!(groups.focus(other));
        assert!(!groups.close_active(true));
    }

    #[test]
    fn app_views_share_text_and_history_with_independent_selections() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("aé日z");
        app.test_editor_mut(0).set_selection(3, 3);
        let other = app.split_editor_group(SplitDirection::Vertical);
        app.test_editor_mut(0).set_selection(1, 1);
        app.test_editor_mut(0).insert("🙂");

        assert!(app.focus_editor_group(0));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "a🙂é日z");
        assert_eq!(app.test_editor(0).cursor().char_idx, 4);
        app.test_editor_mut(0).undo();

        assert!(app.focus_editor_group(other));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "aé日z");
        assert_eq!(app.test_editor(0).cursor().char_idx, 1);
        app.test_editor_mut(0).redo();
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "a🙂é日z");
    }

    #[test]
    fn app_open_and_close_shared_views_preserve_document_and_other_tabs() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("one.txt");
        let second = dir.path().join("two.txt");
        std::fs::write(&path, "one").unwrap();
        std::fs::write(&second, "two").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.test_editor_mut(0).insert("!");
        let group = app.split_editor_group(SplitDirection::Vertical);
        assert!(!app.active_view_needs_confirmation());
        app.open_file(second);
        assert_eq!(app.documents.len(), 2);
        app.activate_editor_document(app.document_at(0));
        app.close_active_tab();
        assert_eq!(app.documents.len(), 2);
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "!one");
        assert!(app.focus_editor_group(0));
        assert_eq!(app.active_document(), app.document_at(0));
        assert!(app.active_view_needs_confirmation());
        assert!(app.focus_editor_group(group));
        app.close_active_tab();
        assert_eq!(app.documents.len(), 1);
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "!one");
        assert_eq!(app.active_document(), app.document_at(0));
    }

    #[test]
    fn edit_sessions_remap_inactive_shared_views() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("abcdef");
        app.test_editor_mut(0).set_selection(5, 5);
        app.split_editor_group(SplitDirection::Vertical);
        app.test_editor_mut(0).set_selection(1, 1);

        app.test_editor_mut(0).insert("é");

        assert!(app.focus_editor_group(0));
        assert_eq!(app.test_editor(0).cursor().char_idx, 6);
    }
}
