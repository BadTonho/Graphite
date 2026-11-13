use super::*;

impl App {
    /// Translates a static UI key.
    pub(crate) fn t<'a>(&self, key: &'a str) -> &'a str {
        self.translator.text(key)
    }

    /// Translates a key and replaces its named placeholders.
    pub(crate) fn tf(&self, key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
        self.translator.format(key, args)
    }

    /// Resolves a command title by stable command ID, falling back to the
    /// English metadata declared at registration.
    pub(crate) fn command_title(
        &self,
        command: &super::command_registry::CommandDescriptor,
    ) -> String {
        let key = format!("command.{}", command.id);
        self.translator.text_or(&key, command.title).to_owned()
    }

    /// Resolves a command category, falling back to the English source.
    pub(crate) fn command_category(&self, category: &'static str) -> String {
        let key = format!("command.category.{}", category.to_ascii_lowercase());
        self.translator.text_or(&key, category).to_owned()
    }

    /// Renders a command failure for the status bar. Messages supplied by
    /// handlers and plugins are already localized or external technical text.
    pub(crate) fn command_error_text(
        &self,
        error: &super::command_execution::CommandError,
    ) -> String {
        use super::command_execution::CommandError;
        match error {
            CommandError::UnknownCommand(id) => self.tf("command.error.unknown", &[("id", id)]),
            CommandError::InvalidArguments(message)
            | CommandError::InvalidTarget(message)
            | CommandError::Failed(message) => message.clone(),
            CommandError::Disabled(id) => self.tf("command.error.disabled", &[("id", id)]),
            CommandError::TargetClosed => self.t("command.error.target_closed").into(),
            CommandError::Cancelled => self.t("command.error.cancelled").into(),
            CommandError::StaleDocument => self.t("command.error.stale_document").into(),
            CommandError::Timeout => self.t("command.error.timeout").into(),
            CommandError::Busy => self.t("command.error.busy").into(),
            CommandError::HostStopped => self.t("command.error.host_stopped").into(),
        }
    }

    /// Returns the active interface language.
    #[cfg(test)]
    pub(crate) fn interface_language(&self) -> graphite_i18n::Language {
        self.translator.language()
    }

    #[cfg(test)]
    pub(crate) fn set_interface_language(&mut self, language: graphite_i18n::Language) {
        self.translator = graphite_i18n::Translator::new(language);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_command_has_titles_and_categories_in_all_locales() {
        let commands = super::super::command_registry::registry();
        for command in commands.iter() {
            // Test-only commands are registered into the same registry and are
            // not part of the shipped interface.
            if command.id.starts_with("graphite.test.") {
                continue;
            }
            let title_key = format!("command.{}", command.id);
            let category_key =
                format!("command.category.{}", command.category.to_ascii_lowercase());
            for language in graphite_i18n::Language::available() {
                let translator = graphite_i18n::Translator::new(language);
                assert!(
                    translator.has(&title_key),
                    "{} is missing {title_key}",
                    language.code()
                );
                assert!(
                    translator.has(&category_key),
                    "{} is missing {category_key}",
                    language.code()
                );
            }
        }
    }

    #[test]
    fn interface_language_can_be_switched_for_tests() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let portuguese = graphite_i18n::Language::parse("pt-BR").unwrap();
        app.set_interface_language(portuguese);
        assert_eq!(app.interface_language(), portuguese);
        assert_eq!(app.t("common.cancel"), "Cancelar");
    }
}
