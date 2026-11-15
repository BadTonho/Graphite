//! Source Control welcome view shown when the open folder has no Git repository.

use super::scm_ui::{scm_centered_text_x, ScmRect};
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct ScmWelcomeLayout {
    pub intro_lines: Vec<(f32, String)>,
    pub init_button: ScmRect,
    pub docs_lines: Vec<(f32, String)>,
    pub publish_lines: Vec<(f32, String)>,
    pub publish_button: ScmRect,
}

pub(crate) fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let limit = max_chars.max(8);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word_len <= limit {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub(crate) fn scm_welcome_layout(
    sidebar_width: f32,
    row_height: f32,
    cell_width: f32,
    intro: &str,
    docs: &str,
    publish: &str,
) -> ScmWelcomeLayout {
    let left = ACTIVITY_BAR_WIDTH + PAD * 1.4;
    let content_w = (sidebar_width - left - PAD * 1.4).max(80.0);
    let max_chars = (content_w / cell_width.max(4.0)).floor() as usize;
    let line_step = (row_height * 0.92).max(16.0);
    let btn_h = (row_height * 1.38).max(26.0);

    let mut y = row_height * 1.75;
    let mut layout_paragraph = |text: &str, y: &mut f32| -> Vec<(f32, String)> {
        let mut out = Vec::new();
        for line in wrap_text(text, max_chars) {
            out.push((*y, line));
            *y += line_step;
        }
        out
    };

    let intro_lines = layout_paragraph(intro, &mut y);
    y += row_height * 0.45;
    let init_button = ScmRect {
        x: left,
        y,
        width: content_w,
        height: btn_h,
    };
    y += btn_h + row_height * 0.85;

    let docs_lines = layout_paragraph(docs, &mut y);
    y += row_height * 0.65;

    let publish_lines = layout_paragraph(publish, &mut y);
    y += row_height * 0.45;
    let publish_button = ScmRect {
        x: left,
        y,
        width: content_w,
        height: btn_h,
    };

    ScmWelcomeLayout {
        intro_lines,
        init_button,
        docs_lines,
        publish_lines,
        publish_button,
    }
}

impl App {
    pub(crate) fn shows_scm_welcome_view(&self) -> bool {
        self.scm_snapshot.error.is_some() && self.vsix_scm_rows().is_empty()
    }

    pub(crate) fn initialize_scm_repository(&mut self) {
        if self.event_proxy.is_some() {
            self.run_scm_remote(super::scm_remote_ui::RemoteOp::Init);
        } else {
            let root = self.scm_workspace_root();
            match crate::vcs::scm_remote::init_repository(&root) {
                Ok(_) => self.refresh_scm_async(),
                Err(error) => self.status = error,
            }
        }
    }

    pub(crate) fn publish_scm_to_github(&mut self) {
        if self.github_session.is_none() {
            self.start_github_sign_in_with_scope("repo");
            return;
        }
        if self.scm_snapshot.error.is_some() {
            self.initialize_scm_repository();
        }
        super::plugins::drawing::open_external_https("https://github.com/new");
    }

    pub(crate) fn draw_scm_welcome_view(
        &self,
        renderer: &mut Renderer,
        sidebar_width: f32,
        height: f32,
        row_h: f32,
        cell_width: f32,
    ) {
        renderer.draw_rect(
            ACTIVITY_BAR_WIDTH,
            0.0,
            sidebar_width - ACTIVITY_BAR_WIDTH,
            height,
            rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
        );
        let left = ACTIVITY_BAR_WIDTH + PAD * 1.4;
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let btn_bg = rgb(Theme::color(Theme::STATUS, 0x0e639c));
        let btn_hover = rgb(0x1177bb);
        let btn_fg = rgb(0xffffff);

        renderer.draw_text(self.t("scm.sidebar.title"), left, row_h * 0.25, dim);

        let layout = scm_welcome_layout(
            sidebar_width,
            row_h,
            cell_width,
            self.t("scm.welcome.intro"),
            self.t("scm.welcome.docs"),
            self.t("scm.welcome.publish_intro"),
        );

        for (y, line) in &layout.intro_lines {
            renderer.draw_text(line, left, *y, text);
        }

        let mx = self.gpu.as_ref().map_or(self.mouse.x as f32, |gpu| {
            self.sidebar_local_x(self.mouse.x as f32, gpu.renderer.size().0)
        });
        let my = self.sidebar_local_y(self.mouse.y as f32);

        let draw_button = |renderer: &mut Renderer, rect: ScmRect, label: &str| {
            let hovered = rect.contains(mx, my);
            renderer.draw_rect(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                if hovered { btn_hover } else { btn_bg },
            );
            let label_x = scm_centered_text_x(rect.x, rect.width, label, cell_width);
            let label_y = rect.y + (rect.height - row_h) * 0.5;
            renderer.draw_text(label, label_x, label_y, btn_fg);
        };

        draw_button(renderer, layout.init_button, self.t("scm.welcome.initialize"));

        for (y, line) in &layout.docs_lines {
            renderer.draw_text(line, left, *y, text);
        }
        for (y, line) in &layout.publish_lines {
            renderer.draw_text(line, left, *y, text);
        }

        draw_button(
            renderer,
            layout.publish_button,
            self.t("scm.welcome.publish_github"),
        );
    }

    pub(crate) fn click_scm_welcome_view(
        &mut self,
        local_x: f32,
        local_y: f32,
        sidebar_width: f32,
        row_h: f32,
        cell_width: f32,
    ) -> bool {
        let layout = scm_welcome_layout(
            sidebar_width,
            row_h,
            cell_width,
            self.t("scm.welcome.intro"),
            self.t("scm.welcome.docs"),
            self.t("scm.welcome.publish_intro"),
        );
        if layout.init_button.contains(local_x, local_y) {
            self.initialize_scm_repository();
            return true;
        }
        if layout.publish_button.contains(local_x, local_y) {
            self.publish_scm_to_github();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_layout_places_non_overlapping_buttons_inside_sidebar() {
        let layout = scm_welcome_layout(
            280.0,
            22.0,
            8.0,
            "No git repo here.",
            "Read our docs.",
            "Publish to GitHub.",
        );
        assert!(layout.init_button.x >= ACTIVITY_BAR_WIDTH);
        assert!(layout.init_button.x + layout.init_button.width <= 280.0);
        assert!(layout.init_button.y + layout.init_button.height < layout.publish_button.y);
    }

    #[test]
    fn welcome_view_shows_for_non_git_folder_and_clears_after_init() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.refresh_scm_async();
        assert!(app.shows_scm_welcome_view());

        app.initialize_scm_repository();
        assert!(!app.shows_scm_welcome_view());
    }
}
