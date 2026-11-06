use super::*;

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
    base.join("VsCodeShoppe")
}

pub fn lsp_config_path() -> PathBuf {
    app_config_dir().join("lsp.json")
}

pub fn terminal_shell() -> Option<String> {
    std::env::var("VSCODESHOPPE_SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            fs::read_to_string(app_config_dir().join("terminal.json"))
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .and_then(|value| value.get("shell")?.as_str().map(str::to_owned))
                .filter(|s| !s.trim().is_empty())
        })
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
