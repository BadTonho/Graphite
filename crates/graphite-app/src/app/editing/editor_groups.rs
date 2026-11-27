//! Per-group presentation state referencing shared documents by stable identity.
use super::document_edit::EditorSession;
use super::{App, DocumentId};
use graphite_text::{PositionEdit, Selection};

pub(crate) use graphite_text::ViewId;

fn next_view_id() -> ViewId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_VIEW_ID: AtomicU64 = AtomicU64::new(1);
    ViewId(NEXT_VIEW_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SplitDirection {
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
    /// Columns the text is scrolled to the left, for views that do not wrap their lines.
    pub scroll_col: usize,
    /// What the user chose for this view with Toggle Word Wrap; `None` follows the setting and the
    /// file type's default.
    pub wrap_choice: Option<bool>,
    /// Start lines of collapsed fold regions in this view, sorted ascending.
    pub folded: Vec<usize>,
    /// Document revision the fold list was built against; a change clears the folds.
    pub folded_revision: u64,
    /// Fold ranges made by hand or for the import block, as `(start, end, by_hand)`; a document
    /// revision change clears them like the folds.
    pub manual_folds: Vec<(usize, usize, bool)>,
    /// Regions to use instead of the document's own while the manual ranges apply: the computed
    /// regions with the manual ones added, and the revision they were built for.
    pub fold_regions: Option<(u64, super::folding::Provided)>,
    /// Pinned tabs stay open when a preview opens and sort before unpinned tabs.
    pub pinned: bool,
    /// Set while the view is a preview tab: the document revision when it was opened.
    /// Any later edit makes the tab permanent.
    pub preview_revision: Option<u64>,
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
    maximized_group: Option<u64>,
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
            scroll_col: 0,
            wrap_choice: None,
            folded: Vec::new(),
            folded_revision: 0,
            manual_folds: Vec::new(),
            fold_regions: None,
            pinned: false,
            preview_revision: None,
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
            maximized_group: None,
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
    pub(crate) fn ordered_group_ids(&self) -> Vec<u64> {
        self.rectangles(GroupRect {
            id: 0,
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 700.0,
        })
        .into_iter()
        .map(|rect| rect.id)
        .collect()
    }
    pub(crate) fn ordered_view_ids(&self) -> Vec<ViewId> {
        self.ordered_group_ids()
            .into_iter()
            .filter_map(|id| self.group(id))
            .flat_map(|group| group.tabs.iter().map(|view| view.id))
            .collect()
    }
    pub(crate) fn view_ids_in_group(&self, id: u64) -> Option<Vec<ViewId>> {
        self.group(id)
            .map(|group| group.tabs.iter().map(|view| view.id).collect())
    }
    pub(crate) fn focus_view(&mut self, id: ViewId) -> bool {
        let Some(group_id) = self.group_for_view(id) else {
            return false;
        };
        let group = self.group_mut(group_id).unwrap();
        let Some(index) = group.tabs.iter().position(|view| view.id == id) else {
            return false;
        };
        group.active = index;
        self.focused = group_id;
        true
    }
    pub(crate) fn collapse_to_single_group(&mut self) {
        if self.groups.len() <= 1 {
            return;
        }
        let focused = self.focused;
        let mut retained = self.group(focused).cloned().expect("focused group exists");
        for group in &self.groups {
            if group.id != focused {
                retained.tabs.extend(group.tabs.iter().cloned());
            }
        }
        self.groups = vec![retained];
        self.layout = Layout::Leaf(focused);
        self.focused = focused;
    }
    pub(crate) fn arrange_layout_grid(&mut self, columns: usize, rows: usize) {
        let columns = columns.clamp(1, 3);
        let rows = rows.clamp(1, 3);
        let count = columns * rows;
        if self.groups.len() > count {
            self.collapse_to_single_group();
        }
        while self.groups.len() < count {
            let source = self.active_view().id;
            self.split_view(
                source,
                if columns > 1 {
                    SplitDirection::Vertical
                } else {
                    SplitDirection::Horizontal
                },
            );
        }
        let ids = self
            .groups
            .iter()
            .map(|group| group.id)
            .take(count)
            .collect::<Vec<_>>();
        let columns_layout = (0..columns)
            .map(|column| {
                let ids = (0..rows)
                    .map(|row| ids[row * columns + column])
                    .collect::<Vec<_>>();
                balanced_layout(&ids, SplitDirection::Horizontal)
            })
            .collect::<Vec<_>>();
        self.layout = balanced_layout_nodes(&columns_layout, SplitDirection::Vertical);
    }
    pub(crate) fn arrange_two_rows_right(&mut self) {
        self.arrange_layout_grid(3, 1);
        self.layout = Layout::Split {
            direction: SplitDirection::Vertical,
            first: Box::new(Layout::Leaf(self.groups[0].id)),
            second: Box::new(Layout::Split {
                direction: SplitDirection::Horizontal,
                first: Box::new(Layout::Leaf(self.groups[1].id)),
                second: Box::new(Layout::Leaf(self.groups[2].id)),
            }),
        };
    }
    pub(crate) fn arrange_two_columns_bottom(&mut self) {
        self.arrange_layout_grid(3, 1);
        self.layout = Layout::Split {
            direction: SplitDirection::Horizontal,
            first: Box::new(Layout::Split {
                direction: SplitDirection::Vertical,
                first: Box::new(Layout::Leaf(self.groups[0].id)),
                second: Box::new(Layout::Leaf(self.groups[1].id)),
            }),
            second: Box::new(Layout::Leaf(self.groups[2].id)),
        };
    }
    pub(crate) fn flip_layout(&mut self) {
        self.layout.flip();
    }
    pub(crate) fn view_ids(&self) -> std::collections::HashSet<ViewId> {
        self.groups
            .iter()
            .flat_map(|group| group.tabs.iter().map(|view| view.id))
            .collect()
    }
    pub(crate) fn for_each_view_mut(&mut self, mut callback: impl FnMut(u64, &mut View)) {
        for group in &mut self.groups {
            for view in &mut group.tabs {
                callback(group.id, view);
            }
        }
    }
    pub(crate) fn active_view(&self) -> &View {
        let group = self.group(self.focused).expect("focused group exists");
        &group.tabs[group.active]
    }
    pub(crate) fn active_view_mut(&mut self) -> &mut View {
        let id = self.focused;
        let group = self.group_mut(id).expect("focused group exists");
        &mut group.tabs[group.active]
    }
    /// Add a background document to the focused group without changing focus.
    pub(crate) fn add_background_document(&mut self, document: DocumentId) {
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
                scroll_col: 0,
                wrap_choice: None,
                folded: Vec::new(),
                folded_revision: 0,
                manual_folds: Vec::new(),
                fold_regions: None,
                pinned: false,
                preview_revision: None,
            });
        }
    }
    pub(crate) fn views(&self, id: u64) -> Option<(&[View], usize)> {
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
    /// Moves pinned tabs before unpinned ones, keeping their relative order and the active tab.
    pub(crate) fn order_group_tabs(&mut self, group: u64) {
        let Some(group) = self.group_mut(group) else {
            return;
        };
        let active_id = group.tabs.get(group.active).map(|view| view.id);
        group.tabs.sort_by_key(|view| !view.pinned);
        if let Some(id) = active_id {
            group.active = group
                .tabs
                .iter()
                .position(|view| view.id == id)
                .unwrap_or(0);
        }
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
                scroll_col: 0,
                wrap_choice: None,
                folded: Vec::new(),
                folded_revision: 0,
                manual_folds: Vec::new(),
                fold_regions: None,
                pinned: false,
                preview_revision: None,
            });
            group.active = group.tabs.len() - 1;
        }
    }
    pub(crate) fn restore_closed_view(&mut self, mut view: View) {
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
    pub(crate) fn split(&mut self, direction: SplitDirection) -> u64 {
        self.split_view(self.active_view().id, direction)
    }
    pub(crate) fn split_view(&mut self, source: ViewId, direction: SplitDirection) -> u64 {
        self.split_view_with_placement(source, direction, false)
    }
    pub(crate) fn split_view_before(&mut self, source: ViewId, direction: SplitDirection) -> u64 {
        self.split_view_with_placement(source, direction, true)
    }
    fn split_view_with_placement(
        &mut self,
        source: ViewId,
        direction: SplitDirection,
        before: bool,
    ) -> u64 {
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
        if before {
            assert!(self.layout.place_leaf_first(id));
        }
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
    pub(crate) fn rectangles(&self, bounds: GroupRect) -> Vec<GroupRect> {
        if let Some(id) = self.maximized_group {
            if self.groups.iter().any(|g| g.id == id) {
                return vec![GroupRect { id, ..bounds }];
            }
        }
        let mut out = Vec::new();
        self.layout.rectangles(bounds, &mut out);
        out
    }

    /// Toggles maximizing the currently focused editor group.
    pub(crate) fn toggle_maximize_group(&mut self) -> bool {
        if self.maximized_group == Some(self.focused) {
            self.maximized_group = None;
            false
        } else {
            self.maximized_group = Some(self.focused);
            true
        }
    }

    /// Maximizes the currently focused editor group.
    pub(crate) fn maximize_focused_group(&mut self) {
        self.maximized_group = Some(self.focused);
    }

    /// Returns whether any editor group is currently maximized.
    pub(crate) fn is_group_maximized(&self) -> bool {
        self.maximized_group.is_some()
    }

    /// Restores equal group sizes and exits maximized group mode.
    pub(crate) fn even_group_widths(&mut self) {
        self.maximized_group = None;
    }

    /// Returns the split direction orthogonal to the focused group's parent split.
    pub(crate) fn split_direction_orthogonal(&self) -> SplitDirection {
        match self.layout.parent_direction(self.focused) {
            Some(SplitDirection::Vertical) => SplitDirection::Horizontal,
            Some(SplitDirection::Horizontal) | None => SplitDirection::Vertical,
        }
    }

    /// Swaps the positions of two editor groups in the layout tree.
    pub(crate) fn swap_groups(&mut self, a: u64, b: u64) -> bool {
        if a == b || self.group(a).is_none() || self.group(b).is_none() {
            return false;
        }
        self.layout.swap_leaves(a, b);
        true
    }
}

fn balanced_layout(ids: &[u64], direction: SplitDirection) -> Layout {
    balanced_layout_nodes(
        &ids.iter().copied().map(Layout::Leaf).collect::<Vec<_>>(),
        direction,
    )
}

fn balanced_layout_nodes(nodes: &[Layout], direction: SplitDirection) -> Layout {
    if nodes.len() == 1 {
        return nodes[0].clone();
    }
    let middle = nodes.len().div_ceil(2);
    Layout::Split {
        direction,
        first: Box::new(balanced_layout_nodes(&nodes[..middle], direction)),
        second: Box::new(balanced_layout_nodes(&nodes[middle..], direction)),
    }
}

impl EditorGroups {
    /// Moves a tab to `index` within its group (clamped to the last position). The active tab
    /// stays the active one.
    pub(crate) fn move_view_to_index(&mut self, view: ViewId, index: usize) -> bool {
        let Some(group_id) = self.group_for_view(view) else {
            return false;
        };
        let group = self.group_mut(group_id).expect("group exists");
        let active_id = group.tabs[group.active].id;
        let from = group
            .tabs
            .iter()
            .position(|tab| tab.id == view)
            .expect("view is in its group");
        let tab = group.tabs.remove(from);
        group.tabs.insert(index.min(group.tabs.len()), tab);
        group.active = group
            .tabs
            .iter()
            .position(|tab| tab.id == active_id)
            .unwrap_or(0);
        self.order_group_tabs(group_id);
        true
    }

    /// Puts a view of the same document into `target` (the existing one when the group already
    /// has it) and makes it the active tab there, focusing that group. With `copy` false the tab
    /// leaves its group, which closes if it had no other tab. Returns the view in the target.
    pub(crate) fn transfer_view(
        &mut self,
        view: ViewId,
        target: u64,
        copy: bool,
    ) -> Option<ViewId> {
        let source = self.group_for_view(view)?;
        self.group(target)?;
        if source == target {
            return Some(view);
        }
        let mut tab = self.view(view)?.clone();
        let existing = self
            .group(target)?
            .tabs
            .iter()
            .position(|other| other.document == tab.document);
        let result = match existing {
            Some(index) => {
                let group = self.group_mut(target)?;
                group.active = index;
                group.tabs[index].id
            }
            None => {
                tab.id = next_view_id();
                tab.ime_preedit.clear();
                tab.ime_composing = false;
                tab.smart_selection_history.clear();
                tab.smart_selection_guard = None;
                let id = tab.id;
                let group = self.group_mut(target)?;
                group.tabs.push(tab);
                group.active = group.tabs.len() - 1;
                self.order_group_tabs(target);
                let group = self.group_mut(target)?;
                group.active = group.tabs.iter().position(|other| other.id == id)?;
                id
            }
        };
        self.focused = target;
        if !copy {
            self.remove_view_from_group(view);
        }
        Some(result)
    }

    /// Takes a tab out of its group without any dirty check, removing the group when it was the
    /// last tab and other groups remain. The focus is left alone unless its group disappeared.
    fn remove_view_from_group(&mut self, view: ViewId) {
        let Some(id) = self.group_for_view(view) else {
            return;
        };
        let focused = self.focused;
        let group = self.group_mut(id).expect("group exists");
        if group.tabs.len() > 1 {
            let active_id = group.tabs[group.active].id;
            let index = group.tabs.iter().position(|tab| tab.id == view).unwrap();
            group.tabs.remove(index);
            group.active = group
                .tabs
                .iter()
                .position(|tab| tab.id == active_id)
                .unwrap_or(index.min(group.tabs.len() - 1));
        } else if self.groups.len() > 1 {
            self.layout = self.layout.clone().remove(id).unwrap();
            self.groups.retain(|group| group.id != id);
            if focused == id {
                self.focused = self.groups[0].id;
            }
        }
    }

    /// Moves every tab of `from` into `into` (skipping documents it already shows) and removes
    /// `from`. The result is the focused group.
    pub(crate) fn join_groups(&mut self, from: u64, into: u64) -> bool {
        if from == into || self.group(from).is_none() || self.group(into).is_none() {
            return false;
        }
        let views = self.view_ids_in_group(from).unwrap_or_default();
        for view in views {
            self.transfer_view(view, into, false);
        }
        self.focused = into;
        true
    }

    /// The tab ids of a group in tab order.
    pub(crate) fn group_views(&self, id: u64) -> Vec<ViewId> {
        self.view_ids_in_group(id).unwrap_or_default()
    }

    /// Index of the active tab of a group.
    pub(crate) fn active_index(&self, id: u64) -> Option<usize> {
        self.group(id).map(|group| group.active)
    }
}

impl Layout {
    fn place_leaf_first(&mut self, target: u64) -> bool {
        let Self::Split { first, second, .. } = self else {
            return false;
        };
        if matches!(**second, Self::Leaf(id) if id == target) {
            std::mem::swap(first, second);
            return true;
        }
        first.place_leaf_first(target) || second.place_leaf_first(target)
    }

    fn flip(&mut self) {
        if let Self::Split { first, second, .. } = self {
            std::mem::swap(first, second);
            first.flip();
            second.flip();
        }
    }
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
    fn parent_direction(&self, target: u64) -> Option<SplitDirection> {
        match self {
            Self::Leaf(_) => None,
            Self::Split {
                direction,
                first,
                second,
            } => {
                if matches!(first.as_ref(), Self::Leaf(id) if *id == target)
                    || matches!(second.as_ref(), Self::Leaf(id) if *id == target)
                {
                    return Some(*direction);
                }
                first
                    .parent_direction(target)
                    .or_else(|| second.parent_direction(target))
            }
        }
    }
    fn swap_leaves(&mut self, a: u64, b: u64) {
        match self {
            Self::Leaf(id) => {
                if *id == a {
                    *id = b;
                } else if *id == b {
                    *id = a;
                }
            }
            Self::Split { first, second, .. } => {
                first.swap_leaves(a, b);
                second.swap_leaves(a, b);
            }
        }
    }
}

impl App {
    pub(crate) fn command_group_rect(&self, width: f32, height: f32, lh: f32) -> GroupRect {
        let group = self
            .editor_groups
            .group_for_view(self.command_view_ref().id)
            .unwrap();
        self.group_rectangles(width, height, lh)
            .into_iter()
            .find(|rect| rect.id == group)
            .unwrap_or_else(|| self.focused_group_rect(width, height, lh))
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
    pub(crate) fn resolve_document(&self, id: DocumentId) -> Option<DocumentId> {
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
            language: tab.language.map(|l| l.name().to_owned()),
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
        self.plugins.active_graphite_detail = None;
        self.plugins.active_installed_detail = None;
        if self.command_runtime.current.is_some() {
            self.command_runtime.view = Some(self.editor_groups.active_view().id);
        }
        self.invalidate_lsp_requests();
        self.request_inlay_hints();
    }

    pub(crate) fn active_view_needs_confirmation(&self) -> bool {
        self.documents[self.command_document()].is_dirty()
            && self.editor_groups.view_count(self.command_document()) <= 1
    }

    pub(crate) fn split_editor_group(&mut self, direction: SplitDirection) -> u64 {
        self.editor_groups
            .split_view(self.command_view_ref().id, direction)
    }

    pub(crate) fn split_editor_group_before(&mut self, direction: SplitDirection) -> u64 {
        self.editor_groups
            .split_view_before(self.command_view_ref().id, direction)
    }

    pub(crate) fn focus_editor_group(&mut self, id: u64) -> bool {
        if !self.editor_groups.focus(id) {
            return false;
        }
        if self
            .plugins
            .active_graphite_detail
            .as_ref()
            .is_some_and(|(group, _)| *group != id)
        {
            self.plugins.active_graphite_detail = None;
        }
        if self
            .plugins
            .active_installed_detail
            .as_ref()
            .is_some_and(|(group, _)| *group != id)
        {
            self.plugins.active_installed_detail = None;
        }
        if self.command_runtime.current.is_some() {
            self.command_runtime.view = Some(self.editor_groups.active_view().id);
        }
        true
    }

    pub(crate) fn focus_next_editor_group(&mut self) {
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

    pub(crate) fn remap_document_views(&mut self, document: DocumentId, previous_revision: u64) {
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
    fn layout_presets_create_distinct_non_overlapping_editor_groups() {
        let mut groups = EditorGroups::new(DocumentId(0));
        for (columns, rows) in [(2, 1), (3, 1), (1, 2), (1, 3), (2, 2)] {
            groups.arrange_layout_grid(columns, rows);
            let bounds = GroupRect {
                id: 0,
                x: 0.0,
                y: 0.0,
                width: 1200.0,
                height: 800.0,
            };
            let rects = groups.rectangles(bounds);
            assert_eq!(rects.len(), columns * rows);
            for rect in &rects {
                assert!(rect.width > 0.0 && rect.height > 0.0);
                assert!(rect.x >= bounds.x && rect.y >= bounds.y);
                assert!(rect.x + rect.width <= bounds.x + bounds.width + 0.01);
                assert!(rect.y + rect.height <= bounds.y + bounds.height + 0.01);
            }
        }
        groups.toggle_maximize_group();
        let max_rects = groups.rectangles(GroupRect {
            id: 0,
            x: 0.0,
            y: 0.0,
            width: 1200.0,
            height: 800.0,
        });
        assert_eq!(max_rects.len(), 1);
        assert_eq!(max_rects[0].width, 1200.0);
        groups.even_group_widths();
        assert_eq!(
            groups
                .rectangles(GroupRect {
                    id: 0,
                    x: 0.0,
                    y: 0.0,
                    width: 1200.0,
                    height: 800.0,
                })
                .len(),
            4
        );
    }

    #[test]
    fn directional_splits_place_new_group_on_the_requested_side() {
        for (direction, expect_before) in [
            (SplitDirection::Vertical, true),
            (SplitDirection::Horizontal, true),
        ] {
            let mut groups = EditorGroups::new(DocumentId(0));
            let original_group = groups.focused();
            let new_group = groups.split_view_before(groups.active_view().id, direction);
            let bounds = GroupRect {
                id: 0,
                x: 0.0,
                y: 0.0,
                width: 1200.0,
                height: 800.0,
            };
            let rects = groups.rectangles(bounds);
            let original = rects.iter().find(|rect| rect.id == original_group).unwrap();
            let new = rects.iter().find(|rect| rect.id == new_group).unwrap();
            if expect_before && direction == SplitDirection::Vertical {
                assert!(new.x < original.x);
            } else {
                assert!(new.y < original.y);
            }
        }
    }

    #[test]
    fn single_layout_keeps_tabs_from_removed_groups() {
        let mut groups = EditorGroups::new(DocumentId(0));
        groups.active_view_mut().document = DocumentId(1);
        groups.arrange_layout_grid(2, 2);
        let document_ids = groups
            .all_views()
            .map(|view| view.document)
            .collect::<Vec<_>>();
        assert_eq!(document_ids.len(), 4);
        groups.collapse_to_single_group();
        assert_eq!(groups.group_count(), 1);
        assert_eq!(groups.all_views().count(), 4);
        assert_eq!(
            groups
                .rectangles(GroupRect {
                    id: 0,
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0
                })
                .len(),
            1
        );
    }

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
