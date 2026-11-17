use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("graphite-test-{}-{id}", std::process::id()));
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
