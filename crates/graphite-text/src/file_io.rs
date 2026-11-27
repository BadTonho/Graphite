use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::BufReader,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

impl Buffer {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BufferError> {
        let path = path.as_ref();
        let rope = Rope::from_reader(BufReader::new(File::open(path)?))?;
        Ok(Self {
            saved_text: rope.clone(),
            rope,
            path: Some(path.to_path_buf()),
            dirty: false,
            revision: 0,
            undo: Default::default(),
            redo: Vec::new(),
            last_change: None,
            recent_changes: Default::default(),
            last_position_edits: Vec::new(),
            read_only: fs::metadata(path)?.permissions().readonly()
                || matches!(OpenOptions::new().write(true).open(path), Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied),
        })
    }

    pub fn save(&mut self) -> Result<(), BufferError> {
        if self.read_only {
            return Err(
                io::Error::new(io::ErrorKind::PermissionDenied, "document is read-only").into(),
            );
        }
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file path"))?
            .clone();
        self.write_to_path(&path)
    }

    /// Atomically write encoded bytes to the buffer path and mark this revision as saved.
    pub fn save_encoded_bytes(&mut self, bytes: &[u8]) -> Result<(), BufferError> {
        if self.read_only {
            return Err(
                io::Error::new(io::ErrorKind::PermissionDenied, "document is read-only").into(),
            );
        }
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file path"))?
            .clone();
        Self::write_bytes_to_path(&path, bytes)?;
        self.saved_text = self.rope.clone();
        self.dirty = false;
        Ok(())
    }

    /// Save pre-encoded trimmed contents, changing the buffer and its undo
    /// history only after the atomic disk write succeeds.
    pub fn save_encoded_bytes_with_trim(&mut self, bytes: &[u8]) -> Result<(), BufferError> {
        if self.read_only {
            return Err(
                io::Error::new(io::ErrorKind::PermissionDenied, "document is read-only").into(),
            );
        }
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file path"))?
            .clone();
        let edits = self.trailing_whitespace_edits();
        Self::write_bytes_to_path(&path, bytes)?;
        self.apply_transaction(&edits)
            .expect("validated trimming edits");
        self.saved_text = self.rope.clone();
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), BufferError> {
        if self.read_only {
            return Err(
                io::Error::new(io::ErrorKind::PermissionDenied, "document is read-only").into(),
            );
        }
        self.write_to_path(path.as_ref())?;
        self.path = Some(path.as_ref().to_path_buf());
        Ok(())
    }

    fn write_to_path(&mut self, path: &Path) -> Result<(), BufferError> {
        Self::write_rope_to_path(path, &self.rope)?;
        self.saved_text = self.rope.clone();
        self.dirty = false;
        Ok(())
    }

    /// Save trimmed content atomically, modifying the buffer and history only after the write succeeds.
    pub fn save_with_trim(&mut self) -> Result<(), BufferError> {
        if self.read_only {
            return Err(
                io::Error::new(io::ErrorKind::PermissionDenied, "document is read-only").into(),
            );
        }
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file path"))?
            .clone();
        let edits = self.trailing_whitespace_edits();
        let mut trimmed = self.rope.clone();
        for edit in edits.iter().rev() {
            trimmed.remove(edit.range.clone());
        }
        Self::write_rope_to_path(&path, &trimmed)?;
        self.apply_transaction(&edits)
            .expect("validated trimming edits");
        self.saved_text = self.rope.clone();
        self.dirty = false;
        Ok(())
    }

    fn write_rope_to_path(path: &Path, rope: &Rope) -> Result<(), BufferError> {
        let mut bytes = Vec::with_capacity(rope.len_bytes());
        rope.write_to(&mut bytes)?;
        Self::write_bytes_to_path(path, &bytes)
    }

    pub fn mark_saved_snapshot(&mut self, text: &str) {
        self.saved_text = Rope::from_str(text);
        self.dirty = self.rope != self.saved_text;
    }

    pub fn write_bytes_to_path(path: &Path, bytes: &[u8]) -> Result<(), BufferError> {
        Self::write_bytes_with_commit(path, bytes, || true)
    }

    /// The commit gate runs after syncing the temporary file, immediately before replacement.
    pub fn write_bytes_with_commit(
        path: &Path,
        bytes: &[u8],
        commit: impl FnOnce() -> bool,
    ) -> Result<(), BufferError> {
        Self::write_bytes_atomically(path, bytes, commit, true)
    }

    /// Installs a complete new file without replacing a file created in the meantime.
    pub fn write_new_bytes_with_commit(
        path: &Path,
        bytes: &[u8],
        commit: impl FnOnce() -> bool,
    ) -> Result<(), BufferError> {
        Self::write_bytes_atomically(path, bytes, commit, false)
    }

    fn write_bytes_atomically(
        path: &Path,
        bytes: &[u8],
        commit: impl FnOnce() -> bool,
        replace: bool,
    ) -> Result<(), BufferError> {
        // Replace the target only after the complete new contents have been written.
        // Resolve existing symlinks so saving does not replace the link itself.
        let target = match fs::symlink_metadata(path) {
            Ok(_) => fs::canonicalize(path)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => path.to_path_buf(),
            Err(e) => return Err(e.into()),
        };
        let permissions = match fs::metadata(&target) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "target is not a file",
                    )
                    .into());
                }
                if metadata.permissions().readonly() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "target is read-only",
                    )
                    .into());
                }
                // A directory may permit replacement even when ACLs deny writes to the file.
                OpenOptions::new().write(true).open(&target)?;
                Some(metadata.permissions())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let (temp_path, mut file) = loop {
            let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let temp = parent.join(format!(".graphite-{}-{id}.tmp", std::process::id()));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&temp) {
                Ok(file) => break (temp, file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        };
        let result = (|| -> io::Result<()> {
            if let Some(permissions) = permissions {
                file.set_permissions(permissions)?;
            }
            use std::io::Write;
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            if !commit() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "save cancelled"));
            }
            if replace {
                fs::rename(&temp_path, &target)?;
            } else {
                fs::hard_link(&temp_path, path)?;
            }
            Ok(())
        })();
        if !replace || result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result?;
        Ok(())
    }
}
