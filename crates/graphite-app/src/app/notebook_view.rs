//! Folding, searching and the other ways of looking at a notebook.

use super::go_menu::{GoPicker, GoPickerKind, GoSymbolItem};
use super::notebook_kernel::RunTarget;
use super::*;

/// Lines of a notebook the search list holds.
const FIND_LIST_LIMIT: usize = 20_000;
/// Lines per cell the search list can tell apart.
const LINES_PER_CELL: usize = 1_000_000;

impl App {
    /// Opens the list of the lines of the notebook; typing narrows it to those that hold the text.
    pub(crate) fn open_notebook_find(&mut self) {
        let Some(session) = self.active_notebook() else {
            self.status = self.t("notebook.error.no_notebook").into();
            return;
        };
        let query = session.find_query.clone().unwrap_or_default();
        let items: Vec<GoSymbolItem> = session
            .all_lines(FIND_LIST_LIMIT)
            .into_iter()
            .map(|found| GoSymbolItem {
                path: PathBuf::new(),
                line: found.cell * LINES_PER_CELL + found.line.min(LINES_PER_CELL - 1),
                name: found.text.trim().to_owned(),
                scope: self.tf("notebook.find.cell", &[("number", &(found.cell + 1))]),
            })
            .collect();
        let message = items.is_empty().then(|| self.t("notebook.find.empty").to_owned());
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        self.go_picker = Some(GoPicker {
            kind: GoPickerKind::NotebookFind,
            query,
            items,
            selected: 0,
            loading: false,
            generation: self.go_picker_generation,
            message,
        });
    }

    /// Goes to the line a search list entry stands for, and remembers the text for next and
    /// previous.
    pub(crate) fn notebook_find_goto(&mut self, code: usize, query: &str) {
        let (cell, line) = (code / LINES_PER_CELL, code % LINES_PER_CELL);
        let query = query.to_owned();
        self.with_notebook(|session| {
            if !query.is_empty() {
                session.find_query = Some(query.clone());
            }
            session.goto_match(cell, line, &query);
        });
    }

    /// The next or previous match of the last search.
    pub(crate) fn notebook_find_step(&mut self, forward: bool) {
        let mut found = false;
        self.with_notebook(|session| found = session.find_step(forward));
        if !found {
            self.status = self.t("notebook.find.none").into();
        }
    }

    /// Forgets the search and closes its list.
    pub(crate) fn notebook_hide_find(&mut self) {
        if self
            .go_picker
            .as_ref()
            .is_some_and(|picker| picker.kind == GoPickerKind::NotebookFind)
        {
            self.go_picker = None;
        }
        self.with_notebook(|session| session.find_query = None);
    }

    /// Runs the cells of the section the focus is in, or just the focused cell.
    pub(crate) fn run_notebook_section(&mut self, single: bool) -> Result<usize, String> {
        if single {
            return self.run_notebook_cells(RunTarget::Selected);
        }
        let Some(range) = self.active_notebook().and_then(|session| session.focused_section()) else {
            return Err(self.t("notebook.section.none").into());
        };
        self.run_notebook_range(range)
    }

    /// Runs the cells in `range`.
    fn run_notebook_range(&mut self, range: std::ops::Range<usize>) -> Result<usize, String> {
        self.ensure_kernel()?;
        let document = self.active_document();
        let Some(session) = self.documents[document].notebook.as_deref_mut() else {
            return Ok(0);
        };
        let indices: Vec<usize> = range.collect();
        let sent = session.run_cells(&indices);
        self.sync_notebook_buffer(document);
        sent
    }
}
