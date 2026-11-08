use super::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProfile {
    pub name: String,
    pub path: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct TerminalSettings {
    pub profiles: Vec<TerminalProfile>,
    pub default_profile: usize,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TerminalConfigFile {
    shell: Option<String>,
    default_profile: Option<String>,
    #[serde(default)]
    profiles: BTreeMap<String, TerminalProfileConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TerminalProfileConfig {
    Path(String),
    Detailed {
        path: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

impl TerminalProfileConfig {
    fn into_profile(self, name: String) -> Result<TerminalProfile, String> {
        let (path, args) = match self {
            Self::Path(path) => (path, Vec::new()),
            Self::Detailed { path, args } => (path, args),
        };
        if path.trim().is_empty() {
            return Err(format!("Terminal profile '{name}' needs a non-empty path"));
        }
        Ok(TerminalProfile { name, path, args })
    }
}

pub fn terminal_settings() -> Result<TerminalSettings, String> {
    if let Some(shell) = shell_override(
        std::env::var("GRAPHITE_SHELL").ok(),
        std::env::var("GRAPHITE_SHELL").ok(),
    ) {
        return Ok(TerminalSettings {
            profiles: vec![TerminalProfile {
                name: "Environment Shell".into(),
                path: shell,
                args: Vec::new(),
            }],
            default_profile: 0,
        });
    }
    let config_path = app_config_dir().join("terminal.json");
    let raw = match fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(default_terminal_settings())
        }
        Err(error) => {
            return Err(format!(
                "Cannot read terminal config {}: {error}",
                config_path.display()
            ))
        }
    };
    parse_terminal_settings(&raw)
        .map_err(|error| format!("Invalid terminal config {}: {error}", config_path.display()))
}

fn parse_terminal_settings(raw: &str) -> Result<TerminalSettings, String> {
    let config = serde_json::from_str::<TerminalConfigFile>(raw)
        .map_err(|error| format!("Invalid terminal JSON: {error}"))?;
    if !config.profiles.is_empty() {
        let profiles = config
            .profiles
            .into_iter()
            .map(|(name, profile)| profile.into_profile(name))
            .collect::<Result<Vec<_>, _>>()?;
        let default_profile = if let Some(name) = config.default_profile {
            profiles
                .iter()
                .position(|profile| profile.name == name)
                .ok_or_else(|| format!("Unknown default terminal profile '{name}'"))?
        } else {
            0
        };
        return Ok(TerminalSettings {
            profiles,
            default_profile,
        });
    }
    if config.default_profile.is_some() {
        return Err("defaultProfile must name an entry in profiles".into());
    }
    if let Some(shell) = config.shell.filter(|shell| !shell.trim().is_empty()) {
        return Ok(TerminalSettings {
            profiles: vec![TerminalProfile {
                name: "Default".into(),
                path: shell,
                args: Vec::new(),
            }],
            default_profile: 0,
        });
    }
    Ok(default_terminal_settings())
}

fn default_terminal_settings() -> TerminalSettings {
    #[cfg(windows)]
    let profiles = vec![
        TerminalProfile {
            name: "PowerShell".into(),
            path: "powershell.exe".into(),
            args: vec!["-NoLogo".into()],
        },
        TerminalProfile {
            name: "Command Prompt".into(),
            path: "cmd.exe".into(),
            args: Vec::new(),
        },
        TerminalProfile {
            name: "WSL".into(),
            path: "wsl.exe".into(),
            args: Vec::new(),
        },
    ];
    #[cfg(not(windows))]
    let profiles = {
        let default = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
        vec![
            TerminalProfile {
                name: "Default Shell".into(),
                path: default,
                args: Vec::new(),
            },
            TerminalProfile {
                name: "Bash".into(),
                path: "bash".into(),
                args: Vec::new(),
            },
            TerminalProfile {
                name: "zsh".into(),
                path: "zsh".into(),
                args: Vec::new(),
            },
        ]
    };
    TerminalSettings {
        profiles,
        default_profile: 0,
    }
}

pub fn app_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    config_dir_for_base(&base)
}

fn shell_override(graphite: Option<String>, legacy: Option<String>) -> Option<String> {
    graphite
        .filter(|value| !value.trim().is_empty())
        .or_else(|| legacy.filter(|value| !value.trim().is_empty()))
}

fn config_dir_for_base(base: &Path) -> PathBuf {
    let current = base.join("Graphite");
    let legacy = base.join("Graphite");
    if current.exists() || !legacy.exists() {
        current
    } else {
        legacy
    }
}

pub fn lsp_config_path() -> PathBuf {
    app_config_dir().join("lsp.json")
}

pub fn vscode_user_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("Code/User"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("Code/User"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphite_config_directory_prefers_existing_settings_and_preserves_legacy() {
        let temp = crate::test_support::TempDir::new();
        let base = temp.path();
        assert_eq!(config_dir_for_base(base), base.join("Graphite"));
        std::fs::create_dir(base.join("Graphite")).unwrap();
        assert_eq!(config_dir_for_base(base), base.join("Graphite"));
        std::fs::create_dir(base.join("Graphite")).unwrap();
        assert_eq!(config_dir_for_base(base), base.join("Graphite"));
    }

    #[test]
    fn graphite_shell_override_takes_precedence_and_falls_back_to_legacy() {
        assert_eq!(
            shell_override(Some("new".into()), Some("old".into())),
            Some("new".into())
        );
        assert_eq!(
            shell_override(Some(" ".into()), Some("old".into())),
            Some("old".into())
        );
        assert_eq!(shell_override(None, Some(" ".into())), None);
    }

    #[test]
    fn terminal_profiles_accept_legacy_shell_and_named_arguments() {
        let legacy = parse_terminal_settings(r#"{"shell":"custom-shell"}"#).unwrap();
        assert_eq!(legacy.profiles[0].path, "custom-shell");

        let configured = parse_terminal_settings(
            r#"{"defaultProfile":"PowerShell","profiles":{"PowerShell":{"path":"pwsh","args":["-NoLogo"]},"cmd":"cmd.exe"}}"#,
        )
        .unwrap();
        let selected = &configured.profiles[configured.default_profile];
        assert_eq!(selected.name, "PowerShell");
        assert_eq!(selected.args, ["-NoLogo"]);
    }

    #[test]
    fn invalid_terminal_settings_report_config_errors() {
        assert!(parse_terminal_settings("{").is_err());
        assert!(
            parse_terminal_settings(r#"{"defaultProfile":"missing","profiles":{"sh":"sh"}}"#)
                .unwrap_err()
                .contains("Unknown default")
        );
        assert!(parse_terminal_settings(r#"{"defaultProfile":"missing"}"#).is_err());
        assert!(parse_terminal_settings(r#"{"profiles":{"empty":" "}}"#).is_err());
    }
}
