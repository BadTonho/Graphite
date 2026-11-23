//! Whether an editor view wraps its lines.
//!
//! Wrapping is decided for each view, from three things, in this order:
//!
//! 1. what the user chose for that view with the Toggle Word Wrap command (`Alt+Z`);
//! 2. the `editor.wordWrap` setting, when the user or the workspace set it, for every file;
//! 3. a default by file type: Markdown and plain text wrap, code does not.
//!
//! So prose reads comfortably without any setup while code keeps its long lines and its folding,
//! and an explicit setting always wins over the default.

use super::editor_groups::View;
use super::*;
use graphite_syntax::Language;
use std::path::Path;

/// Extensionless files that hold prose.
const PROSE_NAMES: &[&str] = &[
    "readme",
    "license",
    "licence",
    "notice",
    "authors",
    "copying",
    "changelog",
    "contributing",
];

/// Whether a file of this kind wraps when the user has not said otherwise: Markdown and plain
/// text do, code does not. A file with no path yet (a new, untitled one) is plain text.
pub(crate) fn wraps_by_default(path: Option<&Path>, language: Option<Language>) -> bool {
    if language == Some(Language::Markdown) {
        return true;
    }
    if language.is_some() {
        return false;
    }
    let Some(path) = path else {
        return true;
    };
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown" | "mdx" | "txt" | "text") => true,
        Some(_) => false,
        None => path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| PROSE_NAMES.contains(&name.to_ascii_lowercase().as_str())),
    }
}

/// The wrapping of one view: its own choice, else the setting, else the default for its file.
pub(crate) fn resolve_wrap(
    setting: Option<bool>,
    view_choice: Option<bool>,
    by_default: bool,
) -> bool {
    view_choice.or(setting).unwrap_or(by_default)
}

impl App {
    /// Whether the document would wrap by default, from its path and language.
    pub(crate) fn document_wraps_by_default(&self, document: DocumentId) -> bool {
        self.documents
            .get(document)
            .is_some_and(|tab| wraps_by_default(tab.buffer.path(), tab.language))
    }

    /// Whether `view` wraps its lines.
    pub(crate) fn view_wraps(&self, view: &View) -> bool {
        resolve_wrap(
            self.word_wrap_setting,
            view.wrap_choice,
            self.document_wraps_by_default(view.document),
        )
    }

    /// Whether the active view wraps its lines.
    pub(crate) fn word_wrap(&self) -> bool {
        self.view_wraps(self.editor_groups.active_view())
    }

    /// Toggle Word Wrap: flips the wrapping of the active view only, keeping the text that is at
    /// the top of the view at the top.
    pub(crate) fn toggle_word_wrap(&mut self) {
        let (width, height, line_height, char_width) = self
            .gpu
            .as_ref()
            .map(|gpu| {
                let (width, height) = gpu.renderer.size();
                (
                    width,
                    height,
                    gpu.renderer.line_height(),
                    gpu.renderer.char_width(),
                )
            })
            .unwrap_or((1200.0, 760.0, 21.0, 9.0));
        let rects = self.group_rectangles(width, height, line_height);
        let indent = self.wrapping_indent;
        let active = self.editor_groups.active_view().id;
        let now_wraps = self.word_wrap();
        let tabs = &self.documents;
        self.editor_groups.for_each_view_mut(|group, view| {
            if view.id != active {
                return;
            }
            if let (Some(rect), Some(tab)) = (
                rects.iter().find(|rect| rect.id == group),
                tabs.get(view.document),
            ) {
                let gutter = App::gutter_width_for_lines(tab.buffer.len_lines(), char_width);
                let columns = (((rect.width - gutter).max(char_width)) / char_width) as usize;
                super::visual_lines::rescroll(
                    view,
                    tab,
                    columns,
                    (now_wraps, indent),
                    (!now_wraps, indent),
                );
            }
            view.wrap_choice = Some(!now_wraps);
            view.scroll_col = 0;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_and_plain_text_wrap_by_default_and_code_does_not() {
        fn at(name: &str) -> Option<&Path> {
            Some(Path::new(name))
        }
        // Markdown, by language or by extension.
        assert!(wraps_by_default(at("docs/a.md"), None));
        assert!(wraps_by_default(at("a.MARKDOWN"), None));
        assert!(wraps_by_default(at("a.mdx"), None));
        assert!(wraps_by_default(at("whatever"), Some(Language::Markdown)));
        // Plain text: untitled, `.txt`, and the usual prose files without an extension.
        assert!(wraps_by_default(None, None));
        assert!(wraps_by_default(at("notes.txt"), None));
        assert!(wraps_by_default(at("LICENSE"), None));
        assert!(wraps_by_default(at("dir/README"), None));
        // Code and data do not, whether the language is known or not.
        assert!(!wraps_by_default(at("main.rs"), Some(Language::Rust)));
        assert!(!wraps_by_default(at("a.json"), Some(Language::Json)));
        assert!(!wraps_by_default(at("page.xml"), None));
        assert!(!wraps_by_default(at("Makefile"), None));
        assert!(!wraps_by_default(at("Dockerfile"), None));
        assert!(!wraps_by_default(at("data.csv"), None));
    }

    #[test]
    fn a_views_own_choice_beats_the_setting_which_beats_the_default() {
        // Default only.
        assert!(resolve_wrap(None, None, true));
        assert!(!resolve_wrap(None, None, false));
        // An explicit setting overrides the default both ways.
        assert!(resolve_wrap(Some(true), None, false));
        assert!(!resolve_wrap(Some(false), None, true));
        // The view's own choice overrides both.
        assert!(resolve_wrap(Some(false), Some(true), false));
        assert!(!resolve_wrap(Some(true), Some(false), true));
    }

    #[test]
    fn toggling_changes_only_the_active_view_and_the_setting_still_applies_elsewhere() {
        let dir = crate::test_support::TempDir::new();
        let notes = dir.path().join("notes.txt");
        let code = dir.path().join("main.rs");
        std::fs::write(&notes, "some prose\n").unwrap();
        std::fs::write(&code, "fn main() {}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        // The isolated test app starts with wrapping off; use the file-type defaults instead.
        app.word_wrap_setting = None;

        app.open_file(notes.clone());
        assert!(app.word_wrap(), "plain text wraps by default");
        app.toggle_word_wrap();
        assert!(!app.word_wrap(), "this view now does not wrap");

        app.open_file(code.clone());
        assert!(!app.word_wrap(), "code does not wrap by default");
        app.toggle_word_wrap();
        assert!(app.word_wrap(), "this view now wraps");
        app.toggle_word_wrap();
        assert!(!app.word_wrap());
    }
}
