//! Per-group view state; document indices refer to the application's shared document store.
use super::App;
use graphite_text::{map_position, PositionEdit, Selection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct View {
    pub document: usize,
    pub selections: Vec<Selection>,
    pub scroll_line: usize,
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
    pub fn new(document: usize) -> Self {
        let view = View {
            document,
            selections: vec![Selection::default()],
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
    pub fn focused(&self) -> u64 {
        self.focused
    }
    pub fn view_count(&self, document: usize) -> usize {
        self.groups
            .iter()
            .flat_map(|group| &group.tabs)
            .filter(|view| view.document == document)
            .count()
    }
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
    pub(super) fn for_each_view_mut(&mut self, mut callback: impl FnMut(u64, &mut View)) {
        for group in &mut self.groups {
            for view in &mut group.tabs {
                callback(group.id, view);
            }
        }
    }
    pub fn forget_document(&mut self, document: usize) {
        for group in &mut self.groups {
            for view in &mut group.tabs {
                assert_ne!(view.document, document, "document still has an open view");
                if view.document > document {
                    view.document -= 1;
                }
            }
        }
    }
    pub(super) fn active_view(&self) -> &View {
        let group = self.group(self.focused).expect("focused group exists");
        &group.tabs[group.active]
    }
    pub(super) fn active_view_mut(&mut self) -> &mut View {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        &mut group.tabs[group.active]
    }
    /// Add a background document to the focused group without changing focus.
    pub(super) fn add_background_document(&mut self, document: usize) {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        if !group.tabs.iter().any(|view| view.document == document) {
            group.tabs.push(View {
                document,
                selections: vec![Selection::default()],
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
        document: usize,
        selections: Vec<Selection>,
        scroll_line: usize,
    ) {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        if let Some(index) = group.tabs.iter().position(|view| view.document == document) {
            group.active = index;
        } else {
            group.tabs.push(View {
                document,
                selections,
                scroll_line,
            });
            group.active = group.tabs.len() - 1;
        }
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
        let id = self.next_id;
        self.next_id += 1;
        let view = self.active_view().clone();
        self.groups.push(Group {
            id,
            tabs: vec![view],
            active: 0,
        });
        assert!(self.layout.split_leaf(self.focused, id, direction));
        self.focused = id;
        id
    }
    /// A dirty document may close here only if another view keeps it open.
    pub fn close_active(&mut self, dirty: bool) -> bool {
        let document = self.active_view().document;
        if dirty
            && self
                .groups
                .iter()
                .flat_map(|g| &g.tabs)
                .filter(|v| v.document == document)
                .count()
                == 1
        {
            return false;
        }
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        if group.tabs.len() > 1 {
            group.tabs.remove(group.active);
            group.active = group.active.min(group.tabs.len() - 1);
        } else if self.groups.len() > 1 {
            self.layout = self
                .layout
                .clone()
                .remove(id)
                .expect("another group remains");
            self.groups.retain(|g| g.id != id);
            self.focused = self.groups[0].id;
        } else {
            // The application replaces the last clean document with an untitled one.
            return false;
        }
        true
    }
    pub fn remap_other_views(&mut self, document: usize, changes: &[PositionEdit]) {
        let focused = self.focused;
        for group in &mut self.groups {
            for (index, view) in group.tabs.iter_mut().enumerate() {
                if view.document != document || (group.id == focused && group.active == index) {
                    continue;
                }
                for selection in &mut view.selections {
                    selection.anchor = map_position(selection.anchor, changes);
                    selection.head = map_position(selection.head, changes);
                }
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
    /// Save the active buffer's cursor state before changing groups or documents.
    pub(super) fn snapshot_editor_view(&mut self) {
        let tab = &self.tabs[self.active_tab];
        if self.editor_groups.active_view().document != self.active_tab {
            self.editor_groups.activate_document(
                self.active_tab,
                tab.buffer.selections(),
                tab.scroll_line,
            );
        } else {
            let view = self.editor_groups.active_view_mut();
            view.selections = tab.buffer.selections();
            view.scroll_line = tab.scroll_line;
        }
    }

    pub(super) fn restore_editor_view(&mut self) {
        let view = self.editor_groups.active_view().clone();
        if let Some(tab) = self.tabs.get_mut(view.document) {
            self.active_tab = view.document;
            tab.buffer.set_selections(view.selections);
            tab.scroll_line = view.scroll_line;
            self.smart_selection_history.clear();
            self.smart_selection_guard = None;
        }
    }

    pub(super) fn activate_editor_document(&mut self, document: usize) {
        if document >= self.tabs.len() {
            return;
        }
        let current_id = self.tabs[self.active_tab].id;
        let target_id = self.tabs[document].id;
        if current_id != target_id {
            self.recent_documents
                .retain(|id| *id != target_id && *id != current_id);
            self.recent_documents.insert(0, target_id);
            self.recent_documents.insert(1, current_id);
            self.recent_documents.truncate(100);
        }
        self.snapshot_editor_view();
        let tab = &self.tabs[document];
        self.editor_groups
            .activate_document(document, tab.buffer.selections(), tab.scroll_line);
        self.restore_editor_view();
        self.invalidate_lsp_requests();
        self.request_inlay_hints();
    }

    pub(super) fn active_view_needs_confirmation(&self) -> bool {
        self.tabs[self.active_tab].buffer.is_dirty()
            && self.editor_groups.view_count(self.active_tab) <= 1
    }

    pub(super) fn split_editor_group(&mut self, direction: SplitDirection) -> u64 {
        self.snapshot_editor_view();
        let id = self.editor_groups.split(direction);
        self.restore_editor_view();
        id
    }

    pub(super) fn focus_editor_group(&mut self, id: u64) -> bool {
        self.snapshot_editor_view();
        if !self.editor_groups.focus(id) {
            return false;
        }
        self.restore_editor_view();
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

    pub(super) fn update_other_editor_views(&mut self, document: usize, previous_revision: u64) {
        if let Some(tab) = self.tabs.get(document) {
            if tab.buffer.revision() == previous_revision + 1 {
                self.editor_groups
                    .remap_other_views(document, tab.buffer.last_position_edits());
            }
        }
        self.snapshot_editor_view();
    }

    pub(super) fn begin_editor_event(&mut self) -> (u64, usize, u64, u64) {
        self.snapshot_editor_view();
        (
            self.editor_groups.focused(),
            self.active_tab,
            self.tabs[self.active_tab].buffer.revision(),
            self.tabs[self.active_tab].id,
        )
    }

    pub(super) fn end_editor_event(&mut self, before: (u64, usize, u64, u64)) {
        if before.3 == self.tabs[self.active_tab].id {
            self.update_other_editor_views(self.active_tab, before.2);
        } else {
            self.snapshot_editor_view();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_document_keeps_independent_views_and_maps_edits() {
        let mut groups = EditorGroups::new(0);
        groups.active_view_mut().selections = vec![Selection { anchor: 4, head: 4 }];
        let right = groups.split(SplitDirection::Vertical);
        groups.active_view_mut().selections = vec![Selection { anchor: 1, head: 2 }];
        groups.active_view_mut().scroll_line = 10;
        groups.remap_other_views(
            0,
            &[PositionEdit {
                start: 1,
                end: 1,
                inserted_chars: 3,
            }],
        );
        assert!(groups.focus(0));
        assert_eq!(groups.active_view().selections[0].head, 7);
        assert_eq!(groups.active_view().scroll_line, 0);
        assert!(groups.focus(right));
        assert_eq!(groups.active_view().selections[0].head, 2);
        assert_eq!(groups.active_view().scroll_line, 10);
    }

    #[test]
    fn split_geometry_and_dirty_closing_keep_shared_documents() {
        let mut groups = EditorGroups::new(0);
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
        let mut groups = EditorGroups::new(4);
        groups.activate_document(9, vec![Selection { anchor: 2, head: 4 }], 5);
        let other = groups.split(SplitDirection::Horizontal);
        assert_eq!(groups.views(other).unwrap().0.len(), 1);
        assert_eq!(groups.active_view().document, 9);
        assert!(groups.focus(0));
        assert_eq!(groups.views(0).unwrap().0.len(), 2);
        assert!(groups.select_tab(0));
        assert_eq!(groups.active_view().document, 4);
        assert!(!groups.close_active(true));
        assert!(groups.select_tab(1));
        assert!(groups.close_active(true));
        assert_eq!(groups.active_view().document, 4);
        assert!(groups.focus(other));
        assert!(!groups.close_active(true));
    }

    #[test]
    fn app_views_share_text_and_history_with_independent_selections() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("aé日z");
        app.tabs[0].buffer.set_selection(3, 3);
        let other = app.split_editor_group(SplitDirection::Vertical);
        app.tabs[0].buffer.set_selection(1, 1);
        let revision = app.tabs[0].buffer.revision();
        app.tabs[0].buffer.insert("🙂");
        app.update_other_editor_views(0, revision);
        assert!(app.focus_editor_group(0));
        assert_eq!(app.tabs[0].buffer.text(), "a🙂é日z");
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 4);
        let revision = app.tabs[0].buffer.revision();
        app.tabs[0].buffer.undo();
        app.update_other_editor_views(0, revision);
        assert!(app.focus_editor_group(other));
        assert_eq!(app.tabs[0].buffer.text(), "aé日z");
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 1);
        app.tabs[0].buffer.redo();
        assert_eq!(app.tabs[0].buffer.text(), "a🙂é日z");
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
        app.tabs[0].buffer.insert("!");
        let group = app.split_editor_group(SplitDirection::Vertical);
        assert!(!app.active_view_needs_confirmation());
        app.open_file(second);
        assert_eq!(app.tabs.len(), 2);
        app.activate_editor_document(0);
        app.close_active_tab();
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs[0].buffer.text(), "!one");
        assert!(app.focus_editor_group(0));
        assert_eq!(app.active_tab, 0);
        assert!(app.active_view_needs_confirmation());
        assert!(app.focus_editor_group(group));
        app.close_active_tab();
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].buffer.text(), "!one");
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn event_boundaries_remap_inactive_shared_views() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("abcdef");
        app.tabs[0].buffer.set_selection(5, 5);
        app.split_editor_group(SplitDirection::Vertical);
        app.tabs[0].buffer.set_selection(1, 1);
        let before = app.begin_editor_event();
        app.tabs[0].buffer.insert("é");
        app.end_editor_event(before);
        assert!(app.focus_editor_group(0));
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 6);
    }
}
