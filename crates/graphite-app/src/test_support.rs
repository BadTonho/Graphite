use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let root = canonical_root(std::env::temp_dir());
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("graphite-test-{}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create isolated test directory: {e}"),
            }
        }
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Finds a helper binary built alongside the test executable. The test executable can
/// live under `target/<profile>/deps` or deeper (for example under `build/`), so the
/// search walks up the ancestors instead of assuming a fixed depth.
pub fn test_binary(name: &str) -> PathBuf {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let exe = std::env::current_exe().expect("current test executable");
    match find_in_ancestors(&exe, &file) {
        Ok(path) => path,
        Err(searched) => panic!(
            "build {name} before targeted tests; searched: {}",
            searched
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn find_in_ancestors(exe: &Path, file: &str) -> Result<PathBuf, Vec<PathBuf>> {
    let candidates: Vec<PathBuf> = exe
        .ancestors()
        .skip(1)
        .take(6)
        .map(|dir| dir.join(file))
        .collect();
    match candidates.iter().find(|path| path.is_file()) {
        Some(found) => Ok(found.clone()),
        None => Err(candidates),
    }
}

/// Resolves the temp root to its long form. On Windows CI runners `TEMP` uses the
/// 8.3 short name (`RUNNER~1`), while git and URI conversions report the long name,
/// so tests comparing the two would fail on paths that are otherwise identical.
fn canonical_root(path: PathBuf) -> PathBuf {
    match std::fs::canonicalize(&path) {
        Ok(canonical) => strip_verbatim(canonical),
        Err(_) => path,
    }
}

/// Removes the `\\?\` verbatim prefix that `canonicalize` adds on Windows, keeping
/// UNC paths intact. Non-Windows paths never carry the prefix and pass through unchanged.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with(r"UNC\") => PathBuf::from(rest),
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dirs_use_the_long_canonical_form() {
        let dir = TempDir::new();
        let canonical = strip_verbatim(std::fs::canonicalize(dir.path()).unwrap());
        assert_eq!(dir.path(), canonical);
    }

    #[test]
    fn helper_binary_is_found_from_a_deeply_nested_test_executable() {
        let dir = TempDir::new();
        let debug = dir.path().join("target").join("debug");
        std::fs::create_dir_all(debug.join("build").join("graphite-app").join("abc123")).unwrap();
        let helper = debug.join("helper-tool");
        std::fs::write(&helper, b"").unwrap();
        let exe = debug
            .join("build")
            .join("graphite-app")
            .join("abc123")
            .join("graphite-app-test.exe");
        assert_eq!(find_in_ancestors(&exe, "helper-tool"), Ok(helper));
    }

    #[test]
    fn missing_helper_reports_every_searched_location() {
        let dir = TempDir::new();
        let exe = dir.path().join("deps").join("graphite-test");
        let searched = find_in_ancestors(&exe, "absent-tool").unwrap_err();
        assert!(searched.contains(&dir.path().join("deps").join("absent-tool")));
        assert!(searched.len() >= 2);
    }

    #[test]
    fn verbatim_prefix_is_removed_but_unc_paths_are_kept() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\Temp\x")),
            PathBuf::from(r"C:\Temp\x")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\server\share")),
            PathBuf::from(r"\\?\UNC\server\share")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from("/tmp/x")),
            PathBuf::from("/tmp/x")
        );
    }
}

/// Initialises a git repository whose `.git/config` carries a test author identity.
/// Commits need an identity, and CI runners (unlike developer machines) have no global one.
pub fn init_git_repo(path: &Path) {
    use std::io::Write;
    gix::init(path).unwrap();
    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(path.join(".git").join("config"))
        .unwrap();
    config
        .write_all(b"[user]\n\tname = Graphite Test\n\temail = test@example.invalid\n")
        .unwrap();
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
