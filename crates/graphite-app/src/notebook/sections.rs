//! Sections of a notebook: a markdown cell that starts with a heading, and the cells after it up
//! to the next heading of the same or a higher level. Sections can be folded.

use super::layout::CellLayout;
use super::model::CellKind;
use super::session::Session;

/// The level (1 to 6) of the heading a markdown cell starts with.
pub(crate) fn heading_level(session: &Session, index: usize) -> Option<usize> {
    let state = session.cells.get(index)?;
    if state.cell.kind != CellKind::Markdown {
        return None;
    }
    let text = state.text();
    let first = text.lines().find(|line| !line.trim().is_empty())?;
    let hashes = first.chars().take_while(|&c| c == '#').count();
    (1..=6)
        .contains(&hashes)
        .then(|| first[hashes..].starts_with(' ').then_some(hashes))
        .flatten()
}

/// The text of the heading of cell `index`, without the hashes.
pub(crate) fn heading_text(session: &Session, index: usize) -> Option<String> {
    heading_level(session, index)?;
    let text = session.cells[index].text();
    let first = text.lines().find(|line| !line.trim().is_empty())?;
    Some(first.trim_start_matches('#').trim().to_owned())
}

/// One past the last cell of the section that starts at heading `index`.
pub(crate) fn section_end(session: &Session, index: usize) -> usize {
    let Some(level) = heading_level(session, index) else {
        return index + 1;
    };
    (index + 1..session.cells.len())
        .find(|&next| heading_level(session, next).is_some_and(|other| other <= level))
        .unwrap_or(session.cells.len())
}

/// The heading of the innermost section that holds cell `index` (the cell itself when it is a
/// heading).
pub(crate) fn enclosing_heading(session: &Session, index: usize) -> Option<usize> {
    if heading_level(session, index).is_some() {
        return Some(index);
    }
    (0..index).rev().find(|&candidate| {
        heading_level(session, candidate).is_some() && section_end(session, candidate) > index
    })
}

/// The cells of the section of `index`, without the heading itself.
pub(crate) fn section_body(session: &Session, index: usize) -> Option<std::ops::Range<usize>> {
    let heading = enclosing_heading(session, index)?;
    Some(heading + 1..section_end(session, heading))
}

/// The heading that stays pinned at the top while the cell at `scroll` is shown: the one whose
/// section holds that cell, once the heading itself has scrolled out of sight.
pub(crate) fn sticky_heading(
    session: &Session,
    layouts: &[CellLayout],
    scroll: f32,
) -> Option<usize> {
    let first = layouts
        .iter()
        .position(|cell| cell.height > 0.0 && cell.bottom() > scroll)?;
    let heading = enclosing_heading(session, first)?;
    (layouts[heading].top < scroll).then_some(heading)
}

#[cfg(test)]
mod tests {
    use super::super::layout::layout;
    use super::super::model::{Cell, Notebook};
    use super::*;

    fn session(cells: &[(&str, CellKind)]) -> Session {
        let mut notebook = Notebook::new();
        notebook.cells = cells
            .iter()
            .map(|(source, kind)| Cell::new(*kind, source))
            .collect();
        Session::from_notebook(notebook)
    }

    use CellKind::{Code, Markdown};

    fn outline() -> Session {
        session(&[
            ("intro text", Markdown),
            ("# One", Markdown),
            ("a = 1", Code),
            ("## One A", Markdown),
            ("b = 2", Code),
            ("### Deep", Markdown),
            ("c = 3", Code),
            ("## One B", Markdown),
            ("d = 4", Code),
            ("# Two", Markdown),
            ("e = 5", Code),
        ])
    }

    #[test]
    fn headings_are_markdown_cells_that_start_with_one_to_six_hashes_and_a_space() {
        let session = session(&[
            ("# H1", Markdown),
            ("\n\n###### H6", Markdown),
            ("####### seven", Markdown),
            ("#nospace", Markdown),
            ("# code heading", Code),
            ("text\n# later", Markdown),
            ("", Markdown),
        ]);
        let levels: Vec<_> = (0..7).map(|index| heading_level(&session, index)).collect();
        assert_eq!(levels, [Some(1), Some(6), None, None, None, None, None]);
        assert_eq!(heading_level(&session, 99), None);
        assert_eq!(heading_text(&session, 0).as_deref(), Some("H1"));
        assert_eq!(heading_text(&session, 3), None);
    }

    #[test]
    fn a_section_runs_to_the_next_heading_of_the_same_or_a_higher_level() {
        let session = outline();
        assert_eq!(section_end(&session, 1), 9);
        assert_eq!(section_end(&session, 3), 7);
        assert_eq!(section_end(&session, 5), 7);
        assert_eq!(section_end(&session, 7), 9);
        assert_eq!(section_end(&session, 9), 11);
        // A cell that is not a heading is a section of its own size.
        assert_eq!(section_end(&session, 2), 3);
    }

    #[test]
    fn the_enclosing_heading_is_the_innermost_one() {
        let session = outline();
        assert_eq!(enclosing_heading(&session, 0), None);
        assert_eq!(enclosing_heading(&session, 2), Some(1));
        assert_eq!(enclosing_heading(&session, 4), Some(3));
        assert_eq!(enclosing_heading(&session, 6), Some(5));
        assert_eq!(enclosing_heading(&session, 8), Some(7));
        assert_eq!(enclosing_heading(&session, 10), Some(9));
        // A heading encloses itself.
        assert_eq!(enclosing_heading(&session, 3), Some(3));
        assert_eq!(section_body(&session, 4), Some(4..7));
        assert_eq!(section_body(&session, 0), None);
    }

    #[test]
    fn the_sticky_heading_appears_once_its_own_cell_has_scrolled_away() {
        let session = outline();
        let layouts = layout(&session, 10.0);
        assert_eq!(sticky_heading(&session, &layouts, 0.0), None);
        // Scrolled into the code cell under "# One": the heading is out of view.
        let scroll = layouts[2].top + 1.0;
        assert_eq!(sticky_heading(&session, &layouts, scroll), Some(1));
        // At the heading itself nothing is pinned.
        assert_eq!(sticky_heading(&session, &layouts, layouts[1].top), None);
        // Deeper in, the innermost heading is the one pinned.
        let deep = layouts[6].top + 1.0;
        assert_eq!(sticky_heading(&session, &layouts, deep), Some(5));
        assert_eq!(sticky_heading(&session, &[], 0.0), None);
    }
}
