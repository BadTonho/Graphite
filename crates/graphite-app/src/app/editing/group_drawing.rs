use super::editor_groups::GroupRect;
use super::*;

pub(crate) fn tab_label(tab: &Document, translator: &graphite_i18n::Translator) -> String {
    let name = tab
        .buffer
        .path()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            let key = match tab.notebook.as_deref().and_then(crate::notebook::Session::interactive).map(|state| state.kind) {
                Some(crate::notebook::InteractiveKind::Window) => "notebook.interactive.title",
                Some(crate::notebook::InteractiveKind::Repl) => "notebook.repl.title",
                None => "common.untitled",
            };
            translator.text(key).to_owned()
        });
    format!(" {}{} ", name, if tab.is_dirty() { " ●" } else { "" })
}

/// Width of the close (×) area at the right end of a tab, in character cells.
const CLOSE_AREA_CELLS: f32 = 2.2;

pub(crate) fn tab_width(
    tab: &Document,
    char_width: f32,
    translator: &graphite_i18n::Translator,
) -> f32 {
    tab_label(tab, translator).chars().count() as f32 * char_width * 1.8
        + char_width * 2.6
        + char_width * CLOSE_AREA_CELLS
}

/// The close area of a tab that starts at `tab_x` and is `width` wide: `(left, right)`.
pub(crate) fn tab_close_area(tab_x: f32, width: f32, char_width: f32) -> (f32, f32) {
    let right = tab_x + width;
    (right - char_width * CLOSE_AREA_CELLS, right)
}

impl App {
    pub(crate) fn draw_editor_groups(
        &mut self,
        r: &mut Renderer,
        rects: &[GroupRect],
        lh: f32,
        cw: f32,
    ) {
        self.sync_active_folds();
        self.sync_active_preview();
        for rect in rects {
            let (views, active) = self
                .editor_groups
                .views(rect.id)
                .expect("layout group exists");
            let views = views.to_vec();
            let view = &views[active];
            let Some(document) = self.resolve_document(view.document) else {
                continue;
            };
            self.refresh_document_highlights(document);
            self.refresh_notebook(document);
            let sticky_headers = self.sticky_headers_for(document, view.scroll_line);
            let sticky_focus = self.sticky_focus_index(view.id, view.scroll_line);
            let tab = &self.documents[document];
            let buffer = &tab.buffer;
            let gutter = buffer.len_lines().to_string().len().max(3) as f32 * cw + PAD * 2.0;
            let text_x = rect.x + gutter;
            let minimap_width = if self.view_preferences.minimap_visible {
                72.0_f32.min(rect.width * 0.22)
            } else {
                0.0
            };
            let right = rect.x + rect.width - minimap_width;
            let bottom = rect.y + rect.height;
            let sticky_height = sticky_headers.len() as f32 * lh;
            let tab_height = self.editor_tab_strip_height(lh);
            let breadcrumb_height = if self.view_preferences.breadcrumbs_visible && !self.zen_mode {
                lh
            } else {
                0.0
            };
            let body_y = rect.y + tab_height + sticky_height + breadcrumb_height;
            let body_height =
                (rect.height - tab_height - sticky_height - breadcrumb_height).max(0.0);
            r.set_clip(Some([rect.x, rect.y, right, bottom]));
            r.draw_rect(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                rgb(Theme::color(Theme::BG, 0xffffff)),
            );
            r.draw_rect(
                rect.x,
                rect.y,
                1.0,
                rect.height,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            let mut x = rect.x;
            if self.view_preferences.tabs_visible && !self.zen_mode {
                for (index, view) in views.iter().enumerate() {
                    let tab = &self.documents[self
                        .resolve_document(view.document)
                        .expect("view document exists")];
                    let width = tab_width(tab, cw, &self.translator);
                    let color = if index == active {
                        Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
                    } else {
                        Theme::color(Theme::SIDEBAR, 0xf3f3f3)
                    };
                    r.draw_rect(x, rect.y, width, lh * 1.5, rgb(color));
                    let icon = tab
                        .buffer
                        .path()
                        .map(crate::file_icons::for_path)
                        .unwrap_or(crate::file_icons::FILE);
                    let icon_size = (lh * 0.8).round();
                    r.draw_icon(
                        icon.glyph,
                        x + PAD,
                        rect.y + (lh * 1.5 - icon_size) * 0.5,
                        icon_size,
                        rgb(icon.color),
                    );
                    let label_color = if view.preview_revision.is_some() {
                        Theme::color(Theme::DIM, 0x858585)
                    } else {
                        Theme::color(Theme::TEXT, 0x1e1e1e)
                    };
                    r.draw_text(
                        &tab_label(tab, &self.translator),
                        x + PAD + lh,
                        rect.y + lh * 0.25,
                        rgb(label_color),
                    );
                    let (close_left, close_right) = tab_close_area(x, width, cw);
                    let hovered = self.mouse.x as f32 >= close_left
                        && (self.mouse.x as f32) < close_right
                        && self.mouse.y as f32 >= rect.y
                        && (self.mouse.y as f32) < rect.y + lh * 1.5;
                    if hovered {
                        r.draw_rect(
                            close_left + cw * 0.2,
                            rect.y + lh * 0.2,
                            cw * (CLOSE_AREA_CELLS - 0.4),
                            lh * 1.1,
                            rgb(Theme::color(Theme::DIM, 0x5a5a5a)),
                        );
                    }
                    r.draw_text(
                        "×",
                        close_left + cw * 0.6,
                        rect.y + lh * 0.25,
                        rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                    );
                    if view.pinned {
                        r.draw_rect(x, rect.y, width, 2.0, rgb(0x0078d4));
                    }
                    x += width;
                }
                self.draw_graphite_detail_tabs(r, rect.id, &mut x, rect.y, lh, cw);
            }
            if self
                .plugins
                .active_graphite_detail
                .as_ref()
                .is_some_and(|(group, _)| *group == rect.id)
            {
                self.draw_graphite_detail_page(r, *rect, lh, cw);
                r.set_clip(None);
                continue;
            }
            if self.documents[document].notebook.is_some() {
                self.draw_notebook(r, *rect, tab_height, lh, cw, document);
                r.set_clip(None);
                continue;
            }
            if self.view_preferences.breadcrumbs_visible && !self.zen_mode {
                let mut breadcrumb = buffer
                    .path()
                    .map(|path| {
                        path.strip_prefix(self.tree.root_for(path))
                            .unwrap_or(path)
                            .to_string_lossy()
                            .replace('\\', " / ")
                    })
                    .unwrap_or_else(|| self.t("common.untitled").to_owned());
                if let Some(language) = tab.language {
                    let cursor = view
                        .selection_state
                        .selections()
                        .first()
                        .map(|selection| selection.head)
                        .unwrap_or_default()
                        .min(buffer.len_chars());
                    let line = buffer.rope().char_to_line(cursor);
                    let text = buffer.text();
                    if let Some(symbol) =
                        graphite_syntax::enclosing_scope_headers(language, &text, line).last()
                    {
                        breadcrumb.push_str("  ›  ");
                        breadcrumb.push_str(&symbol.text);
                    }
                }
                r.draw_text(
                    &breadcrumb,
                    rect.x + PAD,
                    rect.y + tab_height,
                    rgb(Theme::color(Theme::DIM, 0x858585)),
                );
            }
            let primary = view
                .selection_state
                .selections()
                .first()
                .copied()
                .unwrap_or_default();
            let cursor = primary.head.min(buffer.len_chars());
            let cur_line = buffer.rope().char_to_line(cursor);
            let visible = ((body_height / lh) as usize).max(1);
            let last = (view.scroll_line + visible).min(buffer.len_lines());
            let levels_end = if last < buffer.len_lines() {
                buffer.line_start_char(last)
            } else {
                buffer.len_chars()
            };
            let bracket_levels = graphite_text::bracket_levels(buffer.rope(), levels_end);
            let mut bracket_match_cells = Vec::new();
            if let Some((a, b)) = graphite_text::matching_bracket(buffer.rope(), cursor) {
                for index in [a, b] {
                    let line = buffer.rope().char_to_line(index);
                    bracket_match_cells.push((line, index - buffer.line_start_char(line)));
                }
            }
            for x in super::layout::ruler_positions(&self.rulers, text_x, cw, right) {
                r.draw_rect(
                    x,
                    body_y,
                    1.0,
                    body_height,
                    rgb(Theme::color(0x404040, 0xdddddd)),
                );
            }
            if self.word_wrap {
                let mut body_rect = *rect;
                body_rect.y = body_y;
                body_rect.height = body_height;
                self.draw_wrapped_view(r, body_rect, view, text_x, gutter, visible, lh, cw);
                draw_sticky_headers(r, *rect, &sticky_headers, sticky_focus, text_x, tab_height, lh, cw);
                continue;
            }
            let decoration_uri = buffer
                .path()
                .and_then(|path| graphite_core_api::DocumentUri::from_path(path).ok())
                .map(|uri| uri.as_str().to_owned());
            let folds = super::fold_commands::effective_folds(view, buffer.revision());
            let fold_hidden =
                super::folding::hidden_ranges(
                    buffer,
                    folds,
                    tab.indentation.tab_size,
                    super::fold_commands::region_source(view, tab),
                );
            let rows =
                super::folding::visible_lines(buffer, &fold_hidden, view.scroll_line, visible);
            for (row, &line) in rows.iter().enumerate() {
                let y = body_y + row as f32 * lh;
                let text = buffer.line(line);
                let line_start = buffer.line_start_char(line);
                let line_end = line_start + text.trim_end_matches(['\r', '\n']).chars().count();
                let first_bracket =
                    bracket_levels.partition_point(|(index, _)| *index < line_start);
                let bracket_overlays: Vec<(usize, graphite_syntax::Class)> = bracket_levels
                    [first_bracket..]
                    .iter()
                    .take_while(|(index, _)| *index < line_end)
                    .map(|&(index, level)| {
                        (
                            index - line_start,
                            super::drawing_helpers::bracket_class(level),
                        )
                    })
                    .collect();
                if let Some(uri) = &decoration_uri {
                    self.draw_line_decorations(
                        r,
                        super::decoration_drawing::DecorationLayer::Under,
                        uri,
                        line,
                        &text,
                        tab.indentation.tab_size,
                        (text_x, y, cw, lh, right),
                    );
                }
                for selection in &view.selection_state.selections() {
                    let start = selection.anchor.min(selection.head).max(line_start);
                    let end = selection.anchor.max(selection.head).min(line_end);
                    if start < end {
                        let prefix: String = text.chars().take(start - line_start).collect();
                        let selected: String = text
                            .chars()
                            .skip(start - line_start)
                            .take(end - start)
                            .collect();
                        let a = display_line(&prefix, tab.indentation.tab_size)
                            .chars()
                            .count();
                        let b = display_line(&selected, tab.indentation.tab_size)
                            .chars()
                            .count();
                        r.draw_rect(text_x + a as f32 * cw, y, b as f32 * cw, lh, rgb(0x264f78));
                    }
                }
                let color = if line == cur_line {
                    Theme::color(Theme::TEXT, 0x1e1e1e)
                } else {
                    Theme::color(Theme::DIM, 0x616161)
                };
                if tab
                    .diagnostics
                    .iter()
                    .chain(&tab.schema_diagnostics)
                    .any(|d| d.range.start.line as usize == line)
                {
                    r.draw_rect(rect.x + 10.0, y + lh * 0.3, 5.0, 5.0, rgb(0xf14c4c));
                }
                if let Some(path) = buffer.path() {
                    let relative = path
                        .strip_prefix(self.tree.root_for(path))
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    if let Some(change) = self
                        .scm_snapshot
                        .line_changes
                        .get(Path::new(&relative))
                        .and_then(|lines| {
                            lines.iter().find(|(changed_line, _)| *changed_line == line)
                        })
                        .map(|(_, change)| *change)
                    {
                        let color = match change {
                            crate::scm::LineChange::Added => 0x2da44e,
                            crate::scm::LineChange::Modified => 0x0969da,
                            crate::scm::LineChange::Deleted => 0xcf222e,
                        };
                        r.draw_rect(rect.x + 2.0, y, 4.0, lh, rgb(color));
                    }
                }
                r.draw_text(
                    &format!(
                        "{:>width$}",
                        line + 1,
                        width = ((gutter - PAD * 2.0) / cw) as usize
                    ),
                    rect.x + PAD,
                    y,
                    rgb(color),
                );
                for column in
                    super::drawing_helpers::indent_guide_columns(&text, tab.indentation.tab_size)
                {
                    r.draw_rect(
                        text_x + column as f32 * cw,
                        y,
                        1.0,
                        lh,
                        rgb(Theme::color(0x404040, 0xdddddd)),
                    );
                }
                if folds.binary_search(&line).is_ok() {
                    let width = display_line(
                        text.trim_end_matches(['\r', '\n']),
                        tab.indentation.tab_size,
                    )
                    .chars()
                    .count();
                    r.draw_text(
                        " …",
                        text_x + width as f32 * cw,
                        y,
                        rgb(Theme::color(Theme::DIM, 0x858585)),
                    );
                }
                for &(match_line, column) in &bracket_match_cells {
                    if match_line == line {
                        let prefix: String = text.chars().take(column).collect();
                        let x = display_line(&prefix, tab.indentation.tab_size)
                            .chars()
                            .count();
                        r.draw_rect(
                            text_x + x as f32 * cw,
                            y,
                            cw,
                            lh,
                            rgb(Theme::color(0xc8d8f0, 0x3a3d41)),
                        );
                    }
                }
                let semantic = decoration_uri
                    .as_deref()
                    .map(|uri| self.semantic_spans(uri, buffer.revision(), line, &text))
                    .unwrap_or_default();
                let layered;
                let syntax = if semantic.is_empty() {
                    spans_for(&tab.highlights, line)
                } else {
                    layered = super::drawing_helpers::overlay_spans(
                        &text,
                        spans_for(&tab.highlights, line),
                        &semantic,
                    );
                    &layered
                };
                let merged = super::drawing_helpers::overlay_classes(
                    &text,
                    syntax,
                    &bracket_overlays,
                );
                draw_code_line(r, &text, &merged, text_x, y, cw, tab.indentation.tab_size);
                if let Some(uri) = &decoration_uri {
                    self.draw_line_decorations(
                        r,
                        super::decoration_drawing::DecorationLayer::Over,
                        uri,
                        line,
                        &text,
                        tab.indentation.tab_size,
                        (text_x, y, cw, lh, right),
                    );
                }
                if self.view_preferences.whitespace != config::WhitespaceRendering::None {
                if let Some(uri) = &decoration_uri {
                    self.draw_debug_inline_value(
                        r,
                        uri,
                        buffer.revision(),
                        line,
                        &text,
                        tab.indentation.tab_size,
                        (text_x, y, cw),
                    );
                }
                    draw_whitespace_markers(
                        r,
                        &text,
                        line_start,
                        &view.selection_state.selections(),
                        text_x,
                        y,
                        cw,
                        tab.indentation.tab_size,
                        self.view_preferences.whitespace,
                    );
                }
                if self.view_preferences.control_characters {
                    draw_control_characters(r, &text, text_x, y, cw, tab.indentation.tab_size);
                }
                super::drawing_helpers::draw_line_diagnostic_squiggles(
                    r,
                    &tab.diagnostics,
                    &tab.schema_diagnostics,
                    &text,
                    line,
                    0,
                    text.trim_end_matches(['\r', '\n']).chars().count(),
                    text_x,
                    y,
                    cw,
                    lh,
                    tab.indentation.tab_size,
                );
                if tab.inlay_revision == Some(buffer.revision()) {
                    for hint in tab
                        .inlay_hints
                        .iter()
                        .filter(|hint| hint.position.line as usize == line)
                    {
                        let column = super::drawing_helpers::utf16_column_to_chars(
                            &text,
                            hint.position.character,
                        );
                        let prefix = text.chars().take(column).collect::<String>();
                        let x = text_x
                            + display_line(&prefix, tab.indentation.tab_size)
                                .chars()
                                .count() as f32
                                * cw;
                        let label = match &hint.label {
                            lsp_types::InlayHintLabel::String(value) => value.clone(),
                            lsp_types::InlayHintLabel::LabelParts(parts) => parts
                                .iter()
                                .map(|part| part.value.as_str())
                                .collect::<String>(),
                        };
                        let padding = if hint.padding_left.unwrap_or(false) {
                            " "
                        } else {
                            ""
                        };
                        let right_padding = if hint.padding_right.unwrap_or(false) {
                            " "
                        } else {
                            ""
                        };
                        r.draw_text(
                            &format!("{padding}{label}{right_padding}"),
                            x,
                            y,
                            rgb(Theme::color(Theme::DIM, 0x858585)),
                        );
                    }
                }
            }
            for selection in &view.selection_state.selections() {
                let head = selection.head.min(buffer.len_chars());
                let line = buffer.rope().char_to_line(head);
                let Some(row) = rows.iter().position(|&shown| shown == line) else {
                    continue;
                };
                let prefix: String = buffer
                    .line(line)
                    .chars()
                    .take(head - buffer.line_start_char(line))
                    .collect();
                let column = display_line(&prefix, tab.indentation.tab_size)
                    .chars()
                    .count();
                let x = text_x + column as f32 * cw;
                let y = body_y + row as f32 * lh;
                r.draw_rect(x, y, 2.0, lh, rgb(Theme::color(Theme::CURSOR, 0x202020)));
                if rect.id == self.editor_groups.focused()
                    && *selection == primary
                    && !self.editor_groups.active_view_mut().ime_preedit.is_empty()
                    && self.local_search.is_none()
                    && self.quick_open.is_none()
                    && self.search_query.is_none()
                    && self.snippet_query.is_none()
                    && !self.terminal_focused
                {
                    r.draw_text(
                        &self.editor_groups.active_view_mut().ime_preedit,
                        x,
                        y,
                        rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                    );
                    r.draw_rect(
                        x,
                        y + lh - 2.0,
                        self.editor_groups
                            .active_view_mut()
                            .ime_preedit
                            .chars()
                            .count() as f32
                            * cw,
                        1.0,
                        rgb(Theme::color(Theme::CURSOR, 0x202020)),
                    );
                }
            }
            draw_sticky_headers(r, *rect, &sticky_headers, sticky_focus, text_x, tab_height, lh, cw);
            if minimap_width > 0.0 {
                let x = rect.x + rect.width - minimap_width;
                r.draw_rect(
                    x,
                    rect.y + tab_height,
                    minimap_width,
                    (rect.height - tab_height).max(0.0),
                    rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
                );
                r.draw_rect(
                    x,
                    rect.y + tab_height,
                    1.0,
                    (rect.height - tab_height).max(0.0),
                    rgb(Theme::color(Theme::DIM, 0x454545)),
                );
                let lines = buffer.len_lines().max(1);
                let pixels = (rect.height - tab_height).max(1.0);
                for line in 0..lines.min(600) {
                    let source = buffer.line(line);
                    if source.trim().is_empty() {
                        continue;
                    }
                    let y = rect.y + tab_height + line as f32 / lines as f32 * pixels;
                    let length =
                        source.chars().count().min(30) as f32 / 30.0 * (minimap_width - 8.0);
                    r.draw_rect(
                        x + 4.0,
                        y,
                        length.max(1.0),
                        1.5,
                        rgb(Theme::color(Theme::DIM, 0x858585)),
                    );
                }
                let marker_y =
                    rect.y + tab_height + view.scroll_line as f32 / lines as f32 * pixels;
                r.draw_rect(
                    x + 1.0,
                    marker_y,
                    minimap_width - 2.0,
                    (visible as f32 / lines as f32 * pixels).max(4.0),
                    rgb(Theme::color(Theme::STATUS, 0x007acc)),
                );
            }
        }
        r.set_clip(None);
    }
}

fn draw_whitespace_markers(
    renderer: &mut Renderer,
    text: &str,
    line_start: usize,
    selections: &[graphite_text::Selection],
    x: f32,
    y: f32,
    cw: f32,
    tab_size: usize,
    mode: config::WhitespaceRendering,
) {
    let boundary = text.trim_end_matches(['\r', '\n']).chars().count();
    for (index, ch) in text.trim_end_matches(['\r', '\n']).chars().enumerate() {
        let visible = match mode {
            config::WhitespaceRendering::None => false,
            config::WhitespaceRendering::All => ch == ' ' || ch == '\t',
            config::WhitespaceRendering::Boundary => {
                ch == ' ' && (index == 0 || index + 1 == boundary)
            }
            config::WhitespaceRendering::Selection => {
                ch == ' '
                    && selections.iter().any(|selection| {
                        let start = selection.anchor.min(selection.head);
                        let end = selection.anchor.max(selection.head);
                        (line_start + index) >= start && (line_start + index) < end
                    })
            }
        };
        if visible {
            let display_column =
                display_line(&text.chars().take(index).collect::<String>(), tab_size)
                    .chars()
                    .count();
            renderer.draw_text(
                if ch == '\t' { "→" } else { "·" },
                x + display_column as f32 * cw,
                y,
                rgb(Theme::color(Theme::DIM, 0x858585)),
            );
        }
    }
}

fn draw_control_characters(
    renderer: &mut Renderer,
    text: &str,
    x: f32,
    y: f32,
    cw: f32,
    tab_size: usize,
) {
    for (index, ch) in text.trim_end_matches(['\r', '\n']).chars().enumerate() {
        if ch.is_control() && ch != '\t' {
            let symbol = char::from_u32(0x2400 + ch as u32).unwrap_or('�');
            let column = display_line(&text.chars().take(index).collect::<String>(), tab_size)
                .chars()
                .count();
            renderer.draw_text(
                &symbol.to_string(),
                x + column as f32 * cw,
                y,
                rgb(Theme::color(Theme::DIM, 0x858585)),
            );
        }
    }
}

fn draw_sticky_headers(
    renderer: &mut Renderer,
    rect: GroupRect,
    headers: &[graphite_syntax::ScopeHeader],
    focused: Option<usize>,
    text_x: f32,
    tab_height: f32,
    line_height: f32,
    _cell_width: f32,
) {
    if headers.is_empty() {
        return;
    }
    renderer.draw_rect(
        rect.x,
        rect.y + tab_height,
        rect.width,
        headers.len() as f32 * line_height,
        rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
    );
    for (index, header) in headers.iter().enumerate() {
        let y = rect.y + tab_height + line_height * index as f32;
        if focused == Some(index) {
            renderer.draw_rect(rect.x, y, rect.width, line_height, rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)));
        }
        renderer.draw_text(
            &format!("{:>3}", header.line + 1),
            rect.x + PAD,
            y,
            rgb(Theme::color(Theme::DIM, 0x858585)),
        );
        renderer.draw_text(
            &header.text,
            text_x,
            y,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
    }
}

#[cfg(test)]
mod close_button_tests {
    use super::*;

    #[test]
    fn close_area_sits_at_the_right_end_of_each_tab() {
        let (left, right) = tab_close_area(100.0, 200.0, 10.0);
        assert_eq!(right, 300.0);
        assert_eq!(left, 300.0 - 22.0);
        assert!(left > 100.0);
    }

    #[test]
    fn closing_an_inactive_tab_closes_only_that_tab() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        std::fs::write(&first, "1").unwrap();
        std::fs::write(&second, "2").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first.clone());
        app.open_file(second.clone());
        let group = app.editor_groups.focused();
        let views = app.editor_groups.views(group).unwrap().0.to_vec();
        assert_eq!(views.len(), 2);
        // Close the first tab while the second one is active.
        let first_view = views[0].id;
        app.close_editor_tab(first_view);
        let remaining = app.editor_groups.views(group).unwrap().0.to_vec();
        assert_eq!(remaining.len(), 1);
        assert_eq!(
            app.documents[app.resolve_document(remaining[0].document).unwrap()]
                .buffer
                .path(),
            Some(second.as_path())
        );
    }
}
