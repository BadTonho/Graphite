use super::editor_groups::GroupRect;
use super::*;

pub(super) fn tab_label(tab: &Tab) -> String {
    let name = tab
        .buffer
        .path()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into());
    format!(
        " {}{} ",
        name,
        if tab.buffer.is_dirty() { " ●" } else { "" }
    )
}

pub(super) fn tab_width(tab: &Tab, char_width: f32) -> f32 {
    tab_label(tab).chars().count() as f32 * char_width * 1.8
}

impl App {
    pub(super) fn draw_editor_groups(
        &mut self,
        r: &mut Renderer,
        rects: &[GroupRect],
        lh: f32,
        cw: f32,
    ) {
        self.snapshot_editor_view();
        for rect in rects {
            let (views, active) = self
                .editor_groups
                .views(rect.id)
                .expect("layout group exists");
            let views = views.to_vec();
            let view = &views[active];
            self.refresh_document_highlights(view.document);
            let tab = &self.tabs[view.document];
            let buffer = &tab.buffer;
            let gutter = buffer.len_lines().to_string().len().max(3) as f32 * cw + PAD * 2.0;
            let text_x = rect.x + gutter;
            let right = rect.x + rect.width;
            let bottom = rect.y + rect.height;
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
            for (index, view) in views.iter().enumerate() {
                let tab = &self.tabs[view.document];
                let width = tab_width(tab, cw);
                let color = if index == active {
                    Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
                } else {
                    Theme::color(Theme::SIDEBAR, 0xf3f3f3)
                };
                r.draw_rect(x, rect.y, width, lh * 1.5, rgb(color));
                r.draw_text(
                    &tab_label(tab),
                    x + PAD,
                    rect.y + lh * 0.25,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
                x += width;
            }
            let primary = view.selections.first().copied().unwrap_or_default();
            let cursor = primary.head.min(buffer.len_chars());
            let cur_line = buffer.rope().char_to_line(cursor);
            let visible = (((rect.height - lh * 1.5).max(0.0) / lh) as usize).max(1);
            let last = (view.scroll_line + visible).min(buffer.len_lines());
            for x in super::layout::ruler_positions(&self.rulers, text_x, cw, right) {
                r.draw_rect(
                    x,
                    rect.y + lh * 1.5,
                    1.0,
                    (rect.height - lh * 1.5).max(0.0),
                    rgb(Theme::color(0x404040, 0xdddddd)),
                );
            }
            if self.word_wrap {
                self.draw_wrapped_view(r, *rect, view, text_x, gutter, visible, lh, cw);
                continue;
            }
            for line in view.scroll_line..last {
                let y = rect.y + lh * 1.5 + (line - view.scroll_line) as f32 * lh;
                let text = buffer.line(line);
                let line_start = buffer.line_start_char(line);
                let line_end = line_start + text.trim_end_matches(['\r', '\n']).chars().count();
                for selection in &view.selections {
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
                    .any(|d| d.range.start.line as usize == line)
                {
                    r.draw_rect(rect.x + 2.0, y + lh * 0.3, 5.0, 5.0, rgb(0xf14c4c));
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
                draw_code_line(
                    r,
                    &text,
                    spans_for(&tab.highlights, line),
                    text_x,
                    y,
                    cw,
                    tab.indentation.tab_size,
                );
            }
            for selection in &view.selections {
                let head = selection.head.min(buffer.len_chars());
                let line = buffer.rope().char_to_line(head);
                if !(view.scroll_line..last).contains(&line) {
                    continue;
                }
                let prefix: String = buffer
                    .line(line)
                    .chars()
                    .take(head - buffer.line_start_char(line))
                    .collect();
                let column = display_line(&prefix, tab.indentation.tab_size)
                    .chars()
                    .count();
                let x = text_x + column as f32 * cw;
                let y = rect.y + lh * 1.5 + (line - view.scroll_line) as f32 * lh;
                r.draw_rect(x, y, 2.0, lh, rgb(Theme::color(Theme::CURSOR, 0x202020)));
                if rect.id == self.editor_groups.focused()
                    && *selection == primary
                    && !self.ime_preedit.is_empty()
                    && self.local_search.is_none()
                    && self.quick_open.is_none()
                    && self.search_query.is_none()
                    && self.snippet_query.is_none()
                    && !self.terminal_focused
                {
                    r.draw_text(
                        &self.ime_preedit,
                        x,
                        y,
                        rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                    );
                    r.draw_rect(
                        x,
                        y + lh - 2.0,
                        self.ime_preedit.chars().count() as f32 * cw,
                        1.0,
                        rgb(Theme::color(Theme::CURSOR, 0x202020)),
                    );
                }
            }
        }
        r.set_clip(None);
    }
}
