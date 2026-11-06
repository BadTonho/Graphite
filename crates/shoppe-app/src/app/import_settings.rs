use super::*;

impl App {
    pub(super) fn import_vscode_settings(&mut self) {
        let report = config::import_vscode();
        let settings = config::editor_prefs_for_workspace(self.tree.root());
        let theme = config::imported_theme_colors(self.tree.root());
        let settings_error = match (settings, theme) {
            (Ok(prefs), Ok(colors)) => {
                DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
                Theme::set_imported_colors(colors);
                self.font_size = prefs.font_size;
                self.tab_size = prefs.tab_size;
                self.insert_spaces = prefs.insert_spaces;
                self.read_only_patterns = prefs.read_only_patterns;
                self.detect_indentation = prefs.detect_indentation;
                self.trim_trailing_whitespace = prefs.trim_trailing_whitespace;
                self.rulers = prefs.rulers;
                self.reconfigure_wrapping(prefs.word_wrap, prefs.wrapping_indent);
                None
            }
            (Err(error), _) | (_, Err(error)) => Some(error),
        };
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer
                .set_font_size(self.font_size * gpu.window.scale_factor() as f32);
        }
        self.snippets = config::load_snippets();
        let keybindings_error = self.reload_keybindings().err();
        self.status = if let Some(error) = settings_error {
            format!("Settings unchanged: {error}")
        } else if let Some(error) = keybindings_error {
            format!("Keybindings unchanged: {error}")
        } else if report.warnings.is_empty() {
            format!("Imported {} VS Code items", report.imported)
        } else {
            format!(
                "Imported {} items; {} warning(s): {}",
                report.imported,
                report.warnings.len(),
                report.warnings[0]
            )
        };
    }
}
