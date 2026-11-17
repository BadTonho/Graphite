use super::*;
use ignore::overrides::OverrideBuilder;

impl Workspace {
    /// Files under the workspace roots (or only under `folder`) whose path matches `include` and
    /// not `exclude` (glob patterns, gitignore syntax), honoring ignore files, at most `max`.
    pub fn find_files(
        &self,
        folder: Option<&Path>,
        include: &[String],
        exclude: &[String],
        max: usize,
    ) -> Vec<PathBuf> {
        let mut found = Vec::new();
        // Roots are canonical; compare the requested folder in the same form.
        let folder = folder.map(|folder| folder.canonicalize().unwrap_or(folder.to_path_buf()));
        let folder = folder.as_deref();
        for root in &self.roots {
            if found.len() >= max {
                break;
            }
            if folder.is_some_and(|folder| !folder.starts_with(root) && !root.starts_with(folder)) {
                continue;
            }
            let mut overrides = OverrideBuilder::new(root);
            for include in include.iter().filter(|glob| !glob.trim().is_empty()) {
                if overrides.add(include).is_err() {
                    return Vec::new();
                }
            }
            for exclude in exclude.iter().filter(|glob| !glob.trim().is_empty()) {
                if overrides.add(&format!("!{exclude}")).is_err() {
                    return Vec::new();
                }
            }
            let Ok(overrides) = overrides.build() else {
                return Vec::new();
            };
            let mut walk = WalkBuilder::new(root);
            walk.standard_filters(true)
                .hidden(false)
                .require_git(false)
                .filter_entry(|entry| entry.file_name() != ".git")
                .overrides(overrides);
            for entry in walk.build().filter_map(Result::ok) {
                if found.len() >= max {
                    break;
                }
                if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    continue;
                }
                let path = entry.into_path();
                if folder.is_none_or(|folder| path.starts_with(folder)) {
                    found.push(path);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod find_tests {
    use super::*;
    use std::fs;

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace() -> (TempDir, Workspace) {
        let dir = TempDir(std::env::temp_dir().join(format!(
            "graphite_find_files_{}_{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t").replace("::", "_")
        )));
        let _ = fs::remove_dir_all(&dir.0);
        fs::create_dir_all(dir.0.join("src")).unwrap();
        fs::create_dir_all(dir.0.join("gen")).unwrap();
        fs::write(dir.0.join("src/main.rs"), "").unwrap();
        fs::write(dir.0.join("src/lib.rs"), "").unwrap();
        fs::write(dir.0.join("gen/out.rs"), "").unwrap();
        fs::write(dir.0.join("notes.md"), "").unwrap();
        let workspace = Workspace::open(&dir.0).unwrap();
        (dir, workspace)
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        let mut names: Vec<String> = paths
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn include_exclude_folder_and_limit_shape_the_results() {
        let (_dir, workspace) = workspace();
        assert_eq!(
            names(&workspace.find_files(None, &["**/*.rs".into()], &[], 10)),
            vec!["lib.rs", "main.rs", "out.rs"]
        );
        assert_eq!(
            names(&workspace.find_files(None, &["**/*.rs".into()], &["gen/**".into()], 10)),
            vec!["lib.rs", "main.rs"]
        );
        assert_eq!(
            names(&workspace.find_files(Some(&workspace.root().join("src")), &[], &[], 10)),
            vec!["lib.rs", "main.rs"]
        );
        assert_eq!(workspace.find_files(None, &[], &[], 2).len(), 2);
        assert!(workspace
            .find_files(None, &["[".into()], &[], 10)
            .is_empty());
    }
}
