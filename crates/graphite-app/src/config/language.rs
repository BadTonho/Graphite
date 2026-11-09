use super::*;
use graphite_i18n::{Language as UiLanguage, Translator};
use serde_json::Value;

/// The interface language selected at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSelection {
    pub language: UiLanguage,
    pub warning: Option<LanguageWarning>,
}

/// A recoverable problem while resolving the interface language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageWarning {
    UnknownEnvironmentValue { value: String },
    UnknownSettingValue { value: String },
    CannotReadSettings { path: String, error: String },
    InvalidSettings { path: String, error: String },
    SettingsNotObject { path: String },
    InvalidSettingType,
}

impl LanguageWarning {
    pub fn localized(&self, translator: &Translator) -> String {
        match self {
            Self::UnknownEnvironmentValue { value } => {
                translator.format("language.warning.environment", &[("value", value)])
            }
            Self::UnknownSettingValue { value } => {
                translator.format("language.warning.setting", &[("value", value)])
            }
            Self::CannotReadSettings { path, error } => translator.format(
                "language.warning.settings_read",
                &[("path", path), ("error", error)],
            ),
            Self::InvalidSettings { path, error } => translator.format(
                "language.warning.settings_invalid",
                &[("path", path), ("error", error)],
            ),
            Self::SettingsNotObject { path } => {
                translator.format("language.warning.settings_not_object", &[("path", path)])
            }
            Self::InvalidSettingType => translator.text("language.warning.settings_type").into(),
        }
    }
}

/// Resolves the interface language with this precedence:
/// `GRAPHITE_LANG`, user `workbench.language`, operating-system locale, English.
pub fn language_selection(workspace: &Path) -> LanguageSelection {
    let detected = UiLanguage::detect();
    let environment = std::env::var("GRAPHITE_LANG").ok();
    let setting = match user_language_setting(workspace) {
        Ok(setting) => setting,
        Err(settings_warning) => {
            let (language, warning) = resolve_language(environment.as_deref(), None, detected);
            return LanguageSelection {
                language,
                warning: warning.or(Some(settings_warning)),
            };
        }
    };
    let (language, warning) =
        resolve_language(environment.as_deref(), setting.as_deref(), detected);
    LanguageSelection { language, warning }
}

fn user_language_setting(workspace: &Path) -> Result<Option<String>, LanguageWarning> {
    let paths = settings::settings_paths(workspace);
    let mut language = None;
    for path in &paths[..2] {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(LanguageWarning::CannotReadSettings {
                    path: path.display().to_string(),
                    error: error.to_string(),
                })
            }
        };
        let value = parse_jsonc(&raw).map_err(|error| LanguageWarning::InvalidSettings {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;
        let values = value
            .as_object()
            .ok_or_else(|| LanguageWarning::SettingsNotObject {
                path: path.display().to_string(),
            })?;
        match values.get("workbench.language") {
            None => {}
            Some(Value::String(value)) => language = Some(value.clone()),
            Some(_) => return Err(LanguageWarning::InvalidSettingType),
        }
    }
    Ok(language)
}

fn resolve_language(
    environment: Option<&str>,
    setting: Option<&str>,
    detected: UiLanguage,
) -> (UiLanguage, Option<LanguageWarning>) {
    let mut warning = None;
    if let Some(value) = environment.filter(|value| !value.trim().is_empty()) {
        if let Some(language) = UiLanguage::parse(value) {
            return (language, warning);
        }
        warning = Some(LanguageWarning::UnknownEnvironmentValue {
            value: value.to_owned(),
        });
    }
    if let Some(value) = setting.filter(|value| !value.trim().is_empty()) {
        if value.eq_ignore_ascii_case("auto") || value.eq_ignore_ascii_case("system") {
            return (detected, warning);
        }
        if let Some(language) = UiLanguage::parse(value) {
            return (language, warning);
        }
        warning = Some(LanguageWarning::UnknownSettingValue {
            value: value.to_owned(),
        });
    }
    (detected, warning)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english() -> UiLanguage {
        UiLanguage::fallback()
    }

    fn portuguese() -> UiLanguage {
        UiLanguage::parse("pt-BR").unwrap()
    }

    #[test]
    fn environment_has_priority_over_settings_and_detection() {
        let (language, warning) = resolve_language(Some("pt-BR"), Some("en"), english());
        assert_eq!(language, portuguese());
        assert_eq!(warning, None);
    }

    #[test]
    fn setting_is_used_when_environment_is_absent() {
        let (language, warning) = resolve_language(None, Some("PT_br"), english());
        assert_eq!(language, portuguese());
        assert_eq!(warning, None);
    }

    #[test]
    fn unknown_values_warn_and_fall_back_to_detection() {
        let (language, warning) = resolve_language(Some("xx"), Some("yy"), english());
        assert_eq!(language, english());
        assert_eq!(
            warning,
            Some(LanguageWarning::UnknownSettingValue { value: "yy".into() })
        );
    }

    #[test]
    fn auto_setting_uses_the_detected_language() {
        let (language, warning) = resolve_language(None, Some("auto"), portuguese());
        assert_eq!(language, portuguese());
        assert_eq!(warning, None);
    }

    #[test]
    fn language_warnings_are_localized() {
        let translator = Translator::new(portuguese());
        let warning = LanguageWarning::UnknownSettingValue { value: "xx".into() };
        assert!(warning
            .localized(&translator)
            .contains("workbench.language"));
    }
}
