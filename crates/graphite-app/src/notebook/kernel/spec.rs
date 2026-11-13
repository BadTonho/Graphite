//! Kernel specifications: finding the kernels installed on the machine.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A kernel that can be started, read from a `kernel.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelSpec {
    /// Directory name of the kernelspec, which notebooks refer to.
    pub name: String,
    pub display_name: String,
    pub language: String,
    /// Command line; `{connection_file}` stands for the connection file.
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// How to interrupt: `signal` or `message`.
    pub interrupt_mode: String,
}

pub(crate) fn parse_spec(name: &str, json: &str) -> Result<KernelSpec, String> {
    let value: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let argv: Vec<String> = value["argv"]
        .as_array()
        .ok_or("kernel.json has no argv")?
        .iter()
        .filter_map(|part| part.as_str().map(str::to_owned))
        .collect();
    if argv.is_empty() {
        return Err("kernel.json has an empty argv".into());
    }
    let env = value["env"]
        .as_object()
        .map(|env| {
            env.iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Ok(KernelSpec {
        name: name.to_owned(),
        display_name: value["display_name"].as_str().unwrap_or(name).to_owned(),
        language: value["language"].as_str().unwrap_or("").to_owned(),
        argv,
        env,
        interrupt_mode: value["interrupt_mode"].as_str().unwrap_or("signal").to_owned(),
    })
}

/// The directories that can hold kernelspecs, most specific first. `env` reads an environment
/// variable; `home` is the user's home directory.
pub(crate) fn search_dirs(env: &dyn Fn(&str) -> Option<String>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(paths) = env("JUPYTER_PATH") {
        for path in std::env::split_paths(&paths) {
            if !path.as_os_str().is_empty() {
                dirs.push(path.join("kernels"));
            }
        }
    }
    if let Some(data) = env("JUPYTER_DATA_DIR") {
        dirs.push(PathBuf::from(data).join("kernels"));
    }
    if cfg!(windows) {
        if let Some(appdata) = env("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("jupyter").join("kernels"));
        }
        if let Some(program_data) = env("PROGRAMDATA") {
            dirs.push(PathBuf::from(program_data).join("jupyter").join("kernels"));
        }
    } else {
        if cfg!(target_os = "macos") {
            if let Some(home) = home {
                dirs.push(home.join("Library").join("Jupyter").join("kernels"));
            }
        } else if let Some(data) = env("XDG_DATA_HOME").filter(|data| !data.is_empty()) {
            dirs.push(PathBuf::from(data).join("jupyter").join("kernels"));
        } else if let Some(home) = home {
            dirs.push(home.join(".local").join("share").join("jupyter").join("kernels"));
        }
        dirs.push(PathBuf::from("/usr/local/share/jupyter/kernels"));
        dirs.push(PathBuf::from("/usr/share/jupyter/kernels"));
    }
    dirs
}

/// `share/jupyter/kernels` next to each Python found on `path`, where virtual environments and
/// conda environments install the kernel of their `ipykernel`.
pub(crate) fn prefix_dirs(path: &std::ffi::OsStr) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for entry in std::env::split_paths(path) {
        let has_python = ["python", "python3", "python.exe", "python3.exe"]
            .iter()
            .any(|name| entry.join(name).is_file());
        if !has_python {
            continue;
        }
        // The prefix is the directory itself on Windows and its parent on Unix (`bin`).
        let mut candidates = vec![entry.clone()];
        if let Some(parent) = entry.parent() {
            candidates.push(parent.to_path_buf());
        }
        for prefix in candidates {
            let kernels = prefix.join("share").join("jupyter").join("kernels");
            if !dirs.contains(&kernels) {
                dirs.push(kernels);
            }
        }
    }
    dirs
}

/// Reads every kernelspec under `dirs`. The first directory that has a name wins.
pub(crate) fn discover(dirs: &[PathBuf]) -> Vec<KernelSpec> {
    let mut specs: Vec<KernelSpec> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut found: Vec<_> = entries.filter_map(Result::ok).collect();
        found.sort_by_key(|entry| entry.file_name());
        for entry in found {
            let name = entry.file_name().to_string_lossy().into_owned();
            if specs.iter().any(|spec| spec.name == name) {
                continue;
            }
            let Ok(json) = std::fs::read_to_string(entry.path().join("kernel.json")) else {
                continue;
            };
            if let Ok(spec) = parse_spec(&name, &json) {
                specs.push(spec);
            }
        }
    }
    specs
}

/// A kernel made from the first Python on `path`, for machines where `ipykernel` is installed
/// but no kernelspec was registered.
pub(crate) fn python_fallback(path: &std::ffi::OsStr) -> Option<KernelSpec> {
    for entry in std::env::split_paths(path) {
        for name in ["python3", "python", "python3.exe", "python.exe"] {
            let candidate = entry.join(name);
            if candidate.is_file() {
                return Some(KernelSpec {
                    name: "python3".into(),
                    display_name: "Python 3".into(),
                    language: "python".into(),
                    argv: vec![
                        candidate.to_string_lossy().into_owned(),
                        "-m".into(),
                        "ipykernel_launcher".into(),
                        "-f".into(),
                        "{connection_file}".into(),
                    ],
                    env: BTreeMap::new(),
                    interrupt_mode: "signal".into(),
                });
            }
        }
    }
    None
}

/// All kernels available on this machine.
pub(crate) fn installed() -> Vec<KernelSpec> {
    let env = |name: &str| std::env::var(name).ok();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let mut dirs = search_dirs(&env, home.as_deref());
    let path = std::env::var_os("PATH").unwrap_or_default();
    dirs.extend(prefix_dirs(&path));
    let mut specs = discover(&dirs);
    if specs.is_empty() {
        specs.extend(python_fallback(&path));
    }
    specs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::fs;

    fn write_spec(root: &Path, name: &str, json: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("kernel.json"), json).unwrap();
    }

    const PYTHON: &str = r#"{"argv": ["python", "-m", "ipykernel_launcher", "-f", "{connection_file}"],
        "display_name": "Python 3 (ipykernel)", "language": "python", "env": {"A": "1", "B": 2}}"#;

    #[test]
    fn a_kernelspec_is_read_with_defaults() {
        let spec = parse_spec("python3", PYTHON).unwrap();
        assert_eq!(spec.display_name, "Python 3 (ipykernel)");
        assert_eq!(spec.language, "python");
        assert_eq!(spec.argv.len(), 5);
        assert_eq!(spec.interrupt_mode, "signal");
        // Only string values are environment variables.
        assert_eq!(spec.env.len(), 1);
        let bare = parse_spec("x", r#"{"argv": ["a"], "interrupt_mode": "message"}"#).unwrap();
        assert_eq!(bare.display_name, "x");
        assert_eq!(bare.interrupt_mode, "message");
    }

    #[test]
    fn invalid_kernelspecs_are_rejected() {
        assert!(parse_spec("x", "nope").is_err());
        assert!(parse_spec("x", "{}").unwrap_err().contains("argv"));
        assert!(parse_spec("x", r#"{"argv": []}"#).unwrap_err().contains("empty"));
    }

    #[test]
    fn discovery_reads_every_directory_and_the_first_one_wins() {
        let user = TempDir::new();
        let system = TempDir::new();
        write_spec(user.path(), "python3", PYTHON);
        write_spec(system.path(), "python3", r#"{"argv": ["other"], "display_name": "System"}"#);
        write_spec(system.path(), "julia", r#"{"argv": ["julia"], "display_name": "Julia", "language": "julia"}"#);
        write_spec(system.path(), "broken", "{");
        fs::create_dir_all(system.path().join("empty")).unwrap();
        let specs = discover(&[
            user.path().to_path_buf(),
            system.path().to_path_buf(),
            PathBuf::from("does-not-exist"),
        ]);
        let names: Vec<_> = specs.iter().map(|spec| spec.name.as_str()).collect();
        assert_eq!(names, ["python3", "julia"]);
        assert_eq!(specs[0].display_name, "Python 3 (ipykernel)");
    }

    #[test]
    fn search_directories_follow_the_environment() {
        let env = |name: &str| match name {
            "JUPYTER_PATH" => Some(std::env::join_paths(["/opt/a", "/opt/b"]).unwrap().to_string_lossy().into_owned()),
            "JUPYTER_DATA_DIR" => Some("/data".to_owned()),
            "APPDATA" => Some("C:/Users/me/AppData/Roaming".to_owned()),
            "PROGRAMDATA" => Some("C:/ProgramData".to_owned()),
            "XDG_DATA_HOME" => Some("/xdg".to_owned()),
            _ => None,
        };
        let dirs = search_dirs(&env, Some(Path::new("/home/me")));
        assert_eq!(dirs[0], PathBuf::from("/opt/a").join("kernels"));
        assert_eq!(dirs[1], PathBuf::from("/opt/b").join("kernels"));
        assert_eq!(dirs[2], PathBuf::from("/data").join("kernels"));
        if cfg!(windows) {
            assert!(dirs.iter().any(|dir| dir.ends_with("jupyter/kernels") || dir.ends_with("jupyter\\kernels")));
        } else if !cfg!(target_os = "macos") {
            assert!(dirs.contains(&PathBuf::from("/xdg/jupyter/kernels")));
            assert!(dirs.contains(&PathBuf::from("/usr/share/jupyter/kernels")));
        }
        // Without any variable the lists are still built from the home directory.
        let none = |_: &str| None;
        assert!(search_dirs(&none, None).len() <= 3);
    }

    #[test]
    fn a_python_on_the_path_adds_its_prefix_and_is_the_fallback() {
        let root = TempDir::new();
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let python = if cfg!(windows) { "python.exe" } else { "python3" };
        fs::write(bin.join(python), "").unwrap();
        let path = std::env::join_paths([bin.clone(), root.path().join("nothing")]).unwrap();
        let dirs = prefix_dirs(&path);
        assert!(dirs.contains(&root.path().join("share").join("jupyter").join("kernels")));
        let fallback = python_fallback(&path).unwrap();
        assert_eq!(fallback.name, "python3");
        assert!(fallback.argv[0].ends_with(python));
        assert!(fallback.argv.contains(&"ipykernel_launcher".to_owned()));
        assert!(fallback.argv.contains(&"{connection_file}".to_owned()));
        let empty = std::env::join_paths([root.path().join("nothing")]).unwrap();
        assert!(python_fallback(&empty).is_none());
        assert!(prefix_dirs(&empty).is_empty());
    }
}
