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
        self.rulers = prefs.rulers;
        self.reconfigure_wrapping(prefs.word_wrap, prefs.wrapping_indent);
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer
                .set_font_size(self.font_size * gpu.window.scale_factor() as f32);
        }
        self.snippets = snippets;
        self.snippet_load_warnings = warnings;
        self.keymap = keymap;
        self.status = if let Some(warning) = self.snippet_load_warnings.first() {
            format!(
                "Imported {} items; snippet warning: {warning}",
                report.imported
            )
        } else if let Some(warning) = report.warnings.first() {
            format!(
                "Imported {} items; {} warning(s): {warning}",
                report.imported,
                report.warnings.len()
            )
        } else {
            format!("Imported {} VS Code items", report.imported)
        };
    }
}
