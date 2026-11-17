//! The outline view: the declarations of the active file as a tree that can be sorted, filtered
//! by typing, collapsed, and that follows the cursor.

use super::*;
use std::collections::BTreeSet;
use winit::keyboard::{Key, NamedKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OutlineSort {
    #[default]
    Position,
    Name,
    Kind,
}

/// Settings and state of the outline view.
#[derive(Clone, Debug)]
pub(crate) struct OutlineState {
    pub sort: OutlineSort,
    pub follow_cursor: bool,
    pub filter_on_type: bool,
    pub filter: String,
    /// Declaration lines whose children are hidden.
    pub collapsed: BTreeSet<usize>,
    /// For a notebook: also list its code cells.
    pub notebook_code_cells: bool,
    /// For a notebook: also list the functions and classes declared in code cells.
    pub notebook_symbols: bool,
    /// For a notebook: only the headings of its markdown cells.
    pub notebook_headers_only: bool,
    /// Whether typing goes to the filter (the outline was clicked last).
    pub focused: bool,
}

impl Default for OutlineState {
    fn default() -> Self {
        Self {
            sort: OutlineSort::Position,
            follow_cursor: true,
            filter_on_type: false,
            filter: String::new(),
            collapsed: BTreeSet::new(),
            notebook_code_cells: true,
            notebook_symbols: false,
            notebook_headers_only: false,
            focused: false,
        }
    }
}

/// One declaration shown in the outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutlineRow {
    pub line: usize,
    pub text: String,
    pub depth: usize,
    pub kind: String,
    pub name: String,
    /// Declarations nested under this one (only meaningful when sorted by position).
    pub expandable: bool,
    pub collapsed: bool,
    /// The cursor is inside this declaration.
    pub current: bool,
}

const MODIFIERS: &[&str] = &[
    "pub",
    "pub(crate)",
    "pub(crate)",
    "export",
    "default",
    "async",
    "static",
    "public",
    "private",
    "protected",
    "final",
    "abstract",
    "unsafe",
    "extern",
    "override",
    "declare",
    "readonly",
];

/// Splits a declaration header such as `pub async fn run(&self)` into its kind (`fn`) and name
/// (`run`).
pub(crate) fn split_header(text: &str) -> (String, String) {
    let mut words = text
        .split_whitespace()
        .skip_while(|word| MODIFIERS.contains(word));
    let kind = words.next().unwrap_or_default().to_owned();
    let name: String = words
        .next()
        .unwrap_or_default()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
        .collect();
    (kind, name)
}

/// Rows of the outline for `entries` (line, text, depth), in the requested order. `current` is
/// the line of the cursor.
pub(crate) fn build_rows(
    entries: &[(usize, String, usize)],
    state: &OutlineState,
    current: usize,
) -> Vec<OutlineRow> {
    let filter = state.filter.to_lowercase();
    let filtering = state.filter_on_type && !filter.is_empty();
    // The declaration the cursor is in: the last one that starts at or before it.
    let current_line = entries
        .iter()
        .map(|(line, _, _)| *line)
        .filter(|line| *line <= current)
        .max();
    let mut rows: Vec<OutlineRow> = entries
        .iter()
        .enumerate()
        .map(|(index, (line, text, depth))| {
            let (kind, name) = split_header(text);
            OutlineRow {
                line: *line,
                text: text.clone(),
                depth: *depth,
                kind,
                name,
                expandable: entries
                    .get(index + 1)
                    .is_some_and(|(_, _, next)| next > depth),
                collapsed: state.collapsed.contains(line),
                current: Some(*line) == current_line,
            }
        })
        .collect();
    match state.sort {
        OutlineSort::Position => {
            // Hide what is under a collapsed declaration.
            let mut hidden_below: Option<usize> = None;
            rows.retain(|row| {
                if hidden_below.is_some_and(|depth| row.depth > depth) {
                    return false;
                }
                hidden_below = row.collapsed.then_some(row.depth);
                true
            });
        }
        OutlineSort::Name => {
            for row in &mut rows {
                row.depth = 0;
                row.expandable = false;
            }
            rows.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then(a.line.cmp(&b.line))
            });
        }
        OutlineSort::Kind => {
            for row in &mut rows {
                row.depth = 0;
                row.expandable = false;
            }
            rows.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.line.cmp(&b.line)));
        }
    }
    if filtering {
        rows.retain(|row| row.text.to_lowercase().contains(&filter));
    }
    rows
}

impl App {
    /// The outline of a notebook: the headings of its markdown cells and, as the view is set,
    /// its code cells and what they declare. The "line" of each entry is the index of the cell.
    fn notebook_outline_entries(
        &self,
        session: &crate::notebook::Session,
    ) -> Vec<(usize, String, usize)> {
        use crate::notebook::{sections, CellKind};
        let mut entries = Vec::new();
        // Depth of what is under the heading that was seen last.
        let mut depth = 0;
        for (index, state) in session.cells.iter().enumerate() {
            if let Some(level) = sections::heading_level(session, index) {
                let text = sections::heading_text(session, index).unwrap_or_default();
                entries.push((index, format!("{} {text}", "#".repeat(level)), level - 1));
                depth = level;
                continue;
            }
            if self.outline.notebook_headers_only || state.cell.kind != CellKind::Code {
                continue;
            }
            let source = state.text();
            if self.outline.notebook_code_cells {
                let label = source
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(|line| line.trim().to_owned())
                    .unwrap_or_else(|| "…".to_owned());
                let count = state
                    .cell
                    .execution_count
                    .map_or_else(|| "[ ]".to_owned(), |count| format!("[{count}]"));
                entries.push((index, format!("{count} {label}"), depth));
            }
            if self.outline.notebook_symbols {
                for line in source.lines() {
                    let trimmed = line.trim();
                    let declares = ["def ", "async def ", "class ", "function ", "fn "]
                        .iter()
                        .any(|keyword| trimmed.starts_with(keyword));
                    if declares {
                        let header = trimmed.trim_end_matches([':', '{', ' ']).to_owned();
                        entries.push((
                            index,
                            header,
                            depth + 1 + usize::from(self.outline.notebook_code_cells),
                        ));
                    }
                }
            }
        }
        entries
    }

    /// The declarations of the active file as `(line, text, depth)`.
    fn outline_entries(&self) -> Vec<(usize, String, usize)> {
        if let Some(session) = self.active_notebook() {
            return self.notebook_outline_entries(session);
        }
        let document = &self.documents[self.active_document()];
        let Some(language) = document.language else {
            return Vec::new();
        };
        let text = document.buffer.text();
        let tab = document.indentation.tab_size.max(1);
        graphite_syntax::declaration_headers(language, &text)
            .into_iter()
            .map(|header| {
                let indent: usize = text
                    .lines()
                    .nth(header.line)
                    .unwrap_or_default()
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .map(|c| if c == '\t' { tab } else { 1 })
                    .sum();
                (header.line, header.text, indent / tab)
            })
            .collect()
    }

    pub(crate) fn outline_rows(&self) -> Vec<OutlineRow> {
        let cursor = if self.outline.follow_cursor {
            match self.active_notebook() {
                Some(session) => session.focus,
                None => self.editor().cursor_line_col().0,
            }
        } else {
            usize::MAX
        };
        let mut rows = build_rows(&self.outline_entries(), &self.outline, cursor);
        if !self.outline.follow_cursor {
            for row in &mut rows {
                row.current = false;
            }
        }
        rows
    }

    pub(crate) fn outline_set_sort(&mut self, sort: OutlineSort) {
        self.outline.sort = sort;
    }

    pub(crate) fn outline_toggle_follow_cursor(&mut self) {
        self.outline.follow_cursor = !self.outline.follow_cursor;
    }

    pub(crate) fn outline_toggle_filter_on_type(&mut self) {
        self.outline.filter_on_type = !self.outline.filter_on_type;
        if !self.outline.filter_on_type {
            self.outline.filter.clear();
        } else {
            self.outline_visible = true;
            self.outline.focused = true;
        }
    }

    pub(crate) fn outline_collapse_all(&mut self) {
        let entries = self.outline_entries();
        self.outline.collapsed = entries
            .iter()
            .enumerate()
            .filter(|(index, (_, _, depth))| {
                entries
                    .get(index + 1)
                    .is_some_and(|(_, _, next)| next > depth)
            })
            .map(|(_, (line, _, _))| *line)
            .collect();
    }

    pub(crate) fn outline_expand_all(&mut self) {
        self.outline.collapsed.clear();
    }

    /// Sends typing to the outline filter while filter-on-type is on and the outline was the last
    /// thing clicked. Returns whether the key was used.
    pub(crate) fn handle_outline_key(&mut self, key: &Key, text: Option<&str>) -> bool {
        if !(self.outline_visible && self.outline.filter_on_type && self.outline.focused) {
            return false;
        }
        match key {
            Key::Named(NamedKey::Escape) => self.outline.filter.clear(),
            Key::Named(NamedKey::Backspace) => {
                self.outline.filter.pop();
            }
            _ => match text.filter(|text| !text.chars().any(char::is_control)) {
                Some(text) => self.outline.filter.push_str(text),
                None => return false,
            },
        }
        true
    }

    /// A click on row `row` of the outline: goes to its declaration, or toggles it when the click
    /// is in the left margin of an expandable row.
    pub(crate) fn outline_click(&mut self, row: usize, on_chevron: bool) {
        self.outline.focused = true;
        let rows = self.outline_rows();
        let Some(entry) = rows.get(row) else {
            return;
        };
        if on_chevron && entry.expandable {
            if !self.outline.collapsed.remove(&entry.line) {
                self.outline.collapsed.insert(entry.line);
            }
            return;
        }
        if self.active_notebook().is_some() {
            let cell = entry.line;
            self.with_notebook(|session| {
                session.reveal(cell);
                session.set_focus(cell, false);
                session.exit_edit();
            });
            return;
        }
        self.run_ui_command_args(
            "graphite.editor.setCursor",
            serde_json::json!({"line": entry.line, "column": 0}),
        );
        self.reveal_cursor = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn entries() -> Vec<(usize, String, usize)> {
        vec![
            (0, "pub struct Zeta {".into(), 0),
            (2, "fn make() {".into(), 1),
            (6, "fn alpha() {".into(), 0),
            (9, "impl Zeta {".into(), 0),
            (10, "pub fn beta(&self) {".into(), 1),
        ]
    }

    #[test]
    fn headers_split_into_kind_and_name_skipping_modifiers() {
        assert_eq!(
            split_header("pub async fn run(&self)"),
            ("fn".into(), "run".into())
        );
        assert_eq!(
            split_header("export default class Widget extends Base {"),
            ("class".into(), "Widget".into())
        );
        assert_eq!(
            split_header("pub struct Zeta {"),
            ("struct".into(), "Zeta".into())
        );
        assert_eq!(split_header(""), (String::new(), String::new()));
    }

    #[test]
    fn position_order_nests_and_marks_expandable_rows_and_the_current_one() {
        let rows = build_rows(&entries(), &OutlineState::default(), 7);
        let lines: Vec<_> = rows.iter().map(|r| r.line).collect();
        assert_eq!(lines, vec![0, 2, 6, 9, 10]);
        assert!(rows[0].expandable && !rows[1].expandable && rows[3].expandable);
        let current: Vec<_> = rows.iter().filter(|r| r.current).map(|r| r.line).collect();
        assert_eq!(
            current,
            vec![6],
            "the cursor on line 7 is inside the declaration at line 6"
        );
    }

    #[test]
    fn collapsing_hides_only_what_is_inside() {
        let mut state = OutlineState::default();
        state.collapsed.insert(0);
        let rows = build_rows(&entries(), &state, 0);
        assert_eq!(
            rows.iter().map(|r| r.line).collect::<Vec<_>>(),
            vec![0, 6, 9, 10]
        );
        assert!(rows[0].collapsed);
    }

    #[test]
    fn sorting_by_name_and_kind_flattens_the_tree() {
        let mut state = OutlineState {
            sort: OutlineSort::Name,
            ..OutlineState::default()
        };
        let by_name = build_rows(&entries(), &state, 0);
        assert_eq!(
            by_name.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["alpha", "beta", "make", "Zeta", "Zeta"]
        );
        assert!(by_name.iter().all(|r| r.depth == 0 && !r.expandable));
        state.sort = OutlineSort::Kind;
        let by_kind = build_rows(&entries(), &state, 0);
        let kinds: Vec<_> = by_kind.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, ["fn", "fn", "fn", "impl", "struct"]);
        assert_eq!(by_kind[0].line, 2, "same kind keeps the file order");
    }

    #[test]
    fn filter_on_type_keeps_matching_declarations_only_while_on() {
        let mut state = OutlineState {
            filter: "ALPHA".into(),
            ..OutlineState::default()
        };
        assert_eq!(
            build_rows(&entries(), &state, 0).len(),
            5,
            "off: no filtering"
        );
        state.filter_on_type = true;
        let rows = build_rows(&entries(), &state, 0);
        assert_eq!(rows.iter().map(|r| r.line).collect::<Vec<_>>(), vec![6]);
    }

    fn app_with_rust_file(text: &str) -> (App, TempDir) {
        let dir = TempDir::new();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, text).unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file);
        (app, dir)
    }

    const SOURCE: &str = "struct B;\n\nfn zebra() {\n    fn inner() {}\n}\n\nfn apple() {}\n";

    #[test]
    fn the_commands_change_sorting_collapse_and_follow_cursor() {
        let (mut app, _dir) = app_with_rust_file(SOURCE);
        let position: Vec<_> = app.outline_rows().iter().map(|r| r.name.clone()).collect();
        assert_eq!(position, ["B", "zebra", "inner", "apple"]);

        app.run_ui_command("outline.sortByName");
        let by_name: Vec<_> = app.outline_rows().iter().map(|r| r.name.clone()).collect();
        assert_eq!(by_name, ["apple", "B", "inner", "zebra"]);
        app.run_ui_command("outline.sortByKind");
        assert_eq!(app.outline.sort, OutlineSort::Kind);
        app.run_ui_command("outline.sortByPosition");
        assert_eq!(app.outline.sort, OutlineSort::Position);

        app.run_ui_command("outline.collapse");
        let collapsed: Vec<_> = app.outline_rows().iter().map(|r| r.name.clone()).collect();
        assert_eq!(collapsed, ["B", "zebra", "apple"], "inner is hidden");
        app.run_ui_command("outline.expand");
        assert_eq!(app.outline_rows().len(), 4);

        app.editor_mut().set_cursor_line_col(6, 0);
        assert!(app
            .outline_rows()
            .iter()
            .any(|r| r.current && r.name == "apple"));
        app.run_ui_command("outline.followCursor");
        assert!(!app.outline.follow_cursor);
        assert!(app.outline_rows().iter().all(|r| !r.current));
    }

    #[test]
    fn typing_filters_the_outline_and_escape_clears_it() {
        let (mut app, _dir) = app_with_rust_file(SOURCE);
        app.outline_visible = true;
        app.run_ui_command("outline.filterOnType");
        assert!(app.outline.filter_on_type && app.outline.focused);
        for letter in ["z", "e"] {
            assert!(app.handle_outline_key(&Key::Character(letter.into()), Some(letter)));
        }
        assert_eq!(app.outline.filter, "ze");
        let rows = app.outline_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "zebra");
        assert!(app.handle_outline_key(&Key::Named(NamedKey::Backspace), None));
        assert_eq!(app.outline.filter, "z");
        assert!(app.handle_outline_key(&Key::Named(NamedKey::Escape), None));
        assert!(app.outline.filter.is_empty());
        app.run_ui_command("outline.filterOnType");
        assert!(!app.outline.filter_on_type);
        assert!(
            !app.handle_outline_key(&Key::Character("x".into()), Some("x")),
            "off: keys pass through"
        );
    }

    #[test]
    fn clicking_a_row_goes_to_it_and_clicking_the_chevron_collapses() {
        let (mut app, _dir) = app_with_rust_file(SOURCE);
        app.outline_click(3, false);
        assert_eq!(app.editor().cursor_line_col().0, 6, "apple");
        app.outline_click(1, true);
        assert!(app.outline.collapsed.contains(&2));
        assert_eq!(app.outline_rows().len(), 3);
        app.outline_click(1, true);
        assert!(app.outline.collapsed.is_empty());
    }
}
