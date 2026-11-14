//! Built-in fallback Language Server catalog and cross-platform PATH discovery.

use super::ServerConfig;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

pub(super) struct DefaultServerSpec {
    pub extensions: &'static [&'static str],
    pub language_id: &'static str,
    pub candidates: &'static [(&'static str, &'static [&'static str])],
}

pub(super) const DEFAULT_LANGUAGE_SERVERS: &[DefaultServerSpec] = &[
    DefaultServerSpec {
        extensions: &["rs"],
        language_id: "rust",
        candidates: &[("rust-analyzer", &[])],
    },
    DefaultServerSpec {
        extensions: &["py", "pyi"],
        language_id: "python",
        candidates: &[
            ("pyright-langserver", &["--stdio"]),
            ("basedpyright-langserver", &["--stdio"]),
            ("pylsp", &[]),
        ],
    },
    DefaultServerSpec {
        extensions: &["js", "mjs", "cjs"],
        language_id: "javascript",
        candidates: &[("typescript-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["jsx"],
        language_id: "javascriptreact",
        candidates: &[("typescript-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["ts", "mts", "cts"],
        language_id: "typescript",
        candidates: &[("typescript-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["tsx"],
        language_id: "typescriptreact",
        candidates: &[("typescript-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["json"],
        language_id: "json",
        candidates: &[("vscode-json-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["jsonc"],
        language_id: "jsonc",
        candidates: &[("vscode-json-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["html", "htm"],
        language_id: "html",
        candidates: &[("vscode-html-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["css"],
        language_id: "css",
        candidates: &[("vscode-css-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["c", "h"],
        language_id: "c",
        candidates: &[("clangd", &[])],
    },
    DefaultServerSpec {
        extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        language_id: "cpp",
        candidates: &[("clangd", &[])],
    },
    DefaultServerSpec {
        extensions: &["go"],
        language_id: "go",
        candidates: &[("gopls", &[])],
    },
    DefaultServerSpec {
        extensions: &["toml"],
        language_id: "toml",
        candidates: &[("taplo", &["lsp", "stdio"])],
    },
    DefaultServerSpec {
        extensions: &["yml", "yaml"],
        language_id: "yaml",
        candidates: &[("yaml-language-server", &["--stdio"])],
    },
    DefaultServerSpec {
        extensions: &["sh", "bash", "zsh"],
        language_id: "shellscript",
        candidates: &[("bash-language-server", &["start"])],
    },
    DefaultServerSpec {
        extensions: &["md", "markdown"],
        language_id: "markdown",
        candidates: &[("marksman", &["server"])],
    },
    DefaultServerSpec {
        extensions: &["java"],
        language_id: "java",
        candidates: &[("jdtls", &[])],
    },
    DefaultServerSpec {
        extensions: &["cs"],
        language_id: "csharp",
        candidates: &[
            ("csharp-ls", &[]),
            ("OmniSharp", &["-lsp"]),
            ("omnisharp", &["-lsp"]),
        ],
    },
    DefaultServerSpec {
        extensions: &["php", "phtml"],
        language_id: "php",
        candidates: &[
            ("intelephense", &["--stdio"]),
            ("phpactor", &["language-server"]),
        ],
    },
    DefaultServerSpec {
        extensions: &["kt", "kts"],
        language_id: "kotlin",
        candidates: &[("kotlin-language-server", &[])],
    },
    DefaultServerSpec {
        extensions: &["scala", "sc"],
        language_id: "scala",
        candidates: &[("metals", &[])],
    },
    DefaultServerSpec {
        extensions: &["rb", "rake", "gemspec"],
        language_id: "ruby",
        candidates: &[("ruby-lsp", &[]), ("solargraph", &["stdio"])],
    },
    DefaultServerSpec {
        extensions: &["sql"],
        language_id: "sql",
        candidates: &[
            ("sql-language-server", &["up", "--method", "stdio"]),
            ("sqls", &[]),
        ],
    },
    DefaultServerSpec {
        extensions: &["lua"],
        language_id: "lua",
        candidates: &[("lua-language-server", &[])],
    },
    DefaultServerSpec {
        extensions: &["zig"],
        language_id: "zig",
        candidates: &[("zls", &[])],
    },
    DefaultServerSpec {
        extensions: &["ex", "exs"],
        language_id: "elixir",
        candidates: &[("elixir-ls", &[])],
    },
    DefaultServerSpec {
        extensions: &["ps1", "psm1", "psd1"],
        language_id: "powershell",
        candidates: &[("powershell-editor-services", &[])],
    },
    DefaultServerSpec {
        extensions: &["scss"],
        language_id: "scss",
        candidates: &[
            ("vscode-css-language-server", &["--stdio"]),
            ("css-languageserver", &["--stdio"]),
        ],
    },
    DefaultServerSpec {
        extensions: &["java"],
        language_id: "java",
        candidates: &[("jdtls", &[])],
    },
    DefaultServerSpec {
        extensions: &["cs"],
        language_id: "csharp",
        candidates: &[("csharp-ls", &[]), ("OmniSharp", &["-lsp"]), ("omnisharp", &["-lsp"])],
    },
    DefaultServerSpec {
        extensions: &["php", "phtml"],
        language_id: "php",
        candidates: &[("intelephense", &["--stdio"]), ("phpactor", &["language-server"])],
    },
    DefaultServerSpec {
        extensions: &["kt", "kts"],
        language_id: "kotlin",
        candidates: &[("kotlin-language-server", &[])],
    },
    DefaultServerSpec {
        extensions: &["scala", "sc"],
        language_id: "scala",
        candidates: &[("metals", &[])],
    },
    DefaultServerSpec {
        extensions: &["rb", "rake", "gemspec"],
        language_id: "ruby",
        candidates: &[("ruby-lsp", &[]), ("solargraph", &["stdio"])],
    },
    DefaultServerSpec {
        extensions: &["sql"],
        language_id: "sql",
        candidates: &[("sql-language-server", &["up", "--method", "stdio"]), ("sqls", &[])],
    },
    DefaultServerSpec {
        extensions: &["lua"],
        language_id: "lua",
        candidates: &[("lua-language-server", &[])],
    },
    DefaultServerSpec {
        extensions: &["zig"],
        language_id: "zig",
        candidates: &[("zls", &[])],
    },
    DefaultServerSpec {
        extensions: &["ex", "exs"],
        language_id: "elixir",
        candidates: &[("elixir-ls", &[])],
    },
    DefaultServerSpec {
        extensions: &["ps1", "psm1", "psd1"],
        language_id: "powershell",
        candidates: &[("powershell-editor-services", &[])],
    },
    DefaultServerSpec {
        extensions: &["scss"],
        language_id: "scss",
        candidates: &[("vscode-css-language-server", &["--stdio"]), ("css-languageserver", &["--stdio"])],
    },
];

pub(super) fn resolve_default_configs<F>(
    explicit: Vec<ServerConfig>,
    mut exists_on_path: F,
) -> HashMap<String, ServerConfig>
where
    F: FnMut(&str) -> bool,
{
    let mut map: HashMap<String, ServerConfig> = explicit
        .into_iter()
        .map(|mut cfg| {
            cfg.extension = cfg.extension.trim_start_matches('.').to_ascii_lowercase();
            (cfg.extension.clone(), cfg)
        })
        .collect();

    for spec in DEFAULT_LANGUAGE_SERVERS {
        let missing_exts: Vec<&str> = spec
            .extensions
            .iter()
            .copied()
            .filter(|ext| !map.contains_key(*ext))
            .collect();
        if missing_exts.is_empty() {
            continue;
        }
        let Some((cmd, args)) = spec.candidates.iter().find(|(cmd, _)| exists_on_path(cmd)) else {
            continue;
        };
        for ext in missing_exts {
            map.insert(
                ext.to_owned(),
                ServerConfig {
                    extension: ext.to_owned(),
                    language_id: spec.language_id.to_owned(),
                    command: (*cmd).to_owned(),
                    args: args.iter().map(|s| (*s).to_owned()).collect(),
                },
            );
        }
    }

    map
}

pub(crate) fn executable_in_path(command: &str) -> bool {
    let path_var = std::env::var_os("PATH");
    let pathext_var = std::env::var_os("PATHEXT");
    find_in_path(command, path_var.as_deref(), pathext_var.as_deref())
}

fn find_in_path(command: &str, path_var: Option<&OsStr>, pathext_var: Option<&OsStr>) -> bool {
    let cmd_path = Path::new(command);
    if cmd_path.components().count() > 1 {
        return is_executable_file(cmd_path);
    }
    let Some(path_var) = path_var else {
        return false;
    };
    #[cfg(windows)]
    let extensions: Vec<String> = {
        let mut exts: Vec<String> = pathext_var
            .and_then(|v| v.to_str())
            .unwrap_or(".EXE;.CMD;.BAT;.COM")
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if exts.is_empty() {
            exts.push(".exe".into());
        }
        exts
    };
    #[cfg(not(windows))]
    let _ = pathext_var;

    for dir in std::env::split_paths(path_var) {
        let candidate = dir.join(command);
        if is_executable_file(&candidate) {
            return true;
        }
        #[cfg(windows)]
        if Path::new(command).extension().is_none() {
            for ext in &extensions {
                let with_ext = dir.join(format!("{command}{ext}"));
                if is_executable_file(&with_ext) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_overrides_defaults_and_detected_defaults_fill_gaps() {
        let explicit = vec![ServerConfig {
            extension: ".rs".into(),
            language_id: "rust".into(),
            command: "custom-rust-lsp".into(),
            args: vec!["--custom".into()],
        }];
        let detected = resolve_default_configs(explicit, |cmd| {
            matches!(
                cmd,
                "rust-analyzer"
                    | "pylsp"
                    | "typescript-language-server"
                    | "clangd"
                    | "gopls"
                    | "taplo"
                    | "yaml-language-server"
                    | "bash-language-server"
                    | "marksman"
                    | "jdtls"
                    | "csharp-ls"
                    | "intelephense"
                    | "kotlin-language-server"
                    | "metals"
                    | "ruby-lsp"
                    | "sqls"
                    | "lua-language-server"
                    | "zls"
                    | "elixir-ls"
                    | "powershell-editor-services"
                    | "vscode-css-language-server"
            )
        });

        assert_eq!(detected["rs"].command, "custom-rust-lsp");
        assert_eq!(detected["py"].command, "pylsp");
        assert_eq!(detected["ts"].command, "typescript-language-server");
        assert_eq!(detected["tsx"].language_id, "typescriptreact");
        assert_eq!(detected["c"].command, "clangd");
        assert_eq!(detected["cpp"].command, "clangd");
        assert_eq!(detected["go"].command, "gopls");
        assert_eq!(detected["toml"].args, vec!["lsp", "stdio"]);
        assert_eq!(detected["yaml"].command, "yaml-language-server");
        assert_eq!(detected["sh"].language_id, "shellscript");
        assert_eq!(detected["md"].command, "marksman");
        assert_eq!(detected["java"].command, "jdtls");
        assert_eq!(detected["cs"].language_id, "csharp");
        assert_eq!(detected["php"].args, vec!["--stdio"]);
        assert_eq!(detected["kt"].command, "kotlin-language-server");
        assert_eq!(detected["scala"].command, "metals");
        assert_eq!(detected["rb"].command, "ruby-lsp");
        assert_eq!(detected["sql"].command, "sqls");
        assert_eq!(detected["lua"].command, "lua-language-server");
        assert_eq!(detected["zig"].command, "zls");
        assert_eq!(detected["ex"].command, "elixir-ls");
        assert_eq!(detected["ps1"].command, "powershell-editor-services");
        assert_eq!(detected["scss"].language_id, "scss");
        assert!(!detected.contains_key("html"));
    }

    #[test]
    fn path_lookup_finds_executables_in_isolated_temp_dir() {
        let dir = crate::test_support::TempDir::new();
        #[cfg(windows)]
        let bin_name = "mock-gopls.exe";
        #[cfg(not(windows))]
        let bin_name = "mock-gopls";

        let bin_path = dir.path().join(bin_name);
        std::fs::write(&bin_path, b"binary").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin_path, perms).unwrap();
        }

        let joined = std::env::join_paths([dir.path()]).unwrap();
        assert!(find_in_path(
            "mock-gopls",
            Some(joined.as_os_str()),
            Some(OsStr::new(".EXE;.CMD"))
        ));
        assert!(!find_in_path(
            "missing-lsp",
            Some(joined.as_os_str()),
            Some(OsStr::new(".EXE;.CMD"))
        ));
    }
}
