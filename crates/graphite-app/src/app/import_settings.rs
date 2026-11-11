use super::*;
impl App {
    pub(super) fn apply_imported_settings(
        &mut self,
        settings: super::command_handlers::configuration::ImportedSettings,
    ) {
        let super::command_handlers::configuration::ImportedSettings {
            report,
            prefs,
            colors,
            dark,
            snippets,
            warnings,
            keymap,
        } = settings;
        DARK_THEME.store(dark, Ordering::Relaxed);
        Theme::set_imported_colors(colors);
        self.font_size = prefs.font_size;
        self.tab_size = prefs.tab_size;
        self.insert_spaces = prefs.insert_spaces;
        self.read_only_patterns = prefs.read_only_patterns;
        self.detect_indentation = prefs.detect_indentation;
        self.trim_trailing_whitespace = prefs.trim_trailing_whitespace;
        self.format_on_save = prefs.format_on_save;
        self.format_on_paste = prefs.format_on_paste;
        self.auto_save_setting = prefs.auto_save;
        self.auto_save_delay_ms = prefs.auto_save_delay_ms;
        self.rulers = prefs.rulers;
        self.reconfigure_wrapping(prefs.word_wrap, prefs.wrapping_indent);
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer.set_font_size(
                self.font_size
                    * gpu.window.scale_factor() as f32
                    * f32::from(self.view_preferences.zoom_percent)
                    / 100.0,
            );
        }
        self.snippets = snippets;
        self.snippet_load_warnings = warnings;
        self.keymap = keymap;
        self.status = if let Some(warning) = self.snippet_load_warnings.first() {
            self.tf(
                "import.complete_with_snippet_warning",
                &[("count", &report.imported), ("warning", warning)],
            )
        } else if let Some(warning) = report.warnings.first() {
            self.tf(
                "import.complete_with_warnings",
                &[
                    ("count", &report.imported),
                    ("warnings", &report.warnings.len()),
                    ("warning", warning),
                ],
            )
        } else {
            self.tf("import.complete", &[("count", &report.imported)])
        };
    }
}
