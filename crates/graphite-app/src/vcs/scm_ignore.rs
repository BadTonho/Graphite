//! Adding files to the workspace `.gitignore`.

use std::path::Path;

/// Adds `relative` (a path inside the repository) to the `.gitignore` at the repository root.
/// Returns `false` when the exact entry was already there.
pub(crate) fn add_to_gitignore(root: &Path, relative: &Path) -> Result<bool, String> {
    let entry = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    if entry.is_empty() || entry.split('/').any(|part| part == "..") {
        return Err(format!("'{}' is not inside the repository", relative.display()));
    }
    let entry = format!("/{}", escape(&entry));
    let path = root.join(".gitignore");
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    if text.lines().any(|line| line.trim_end() == entry) {
        return Ok(false);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&entry);
    text.push('\n');
    std::fs::write(&path, text).map_err(|error| error.to_string())?;
    Ok(true)
}

/// Escapes what a `.gitignore` line would read as a pattern.
fn escape(entry: &str) -> String {
    let mut out = String::with_capacity(entry.len());
    // The leading `/` keeps `#` and `!` from meaning a comment or a negation.
    for character in entry.chars() {
        if matches!(character, '*' | '?' | '[' | '\\') {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn entries_are_anchored_to_the_root_and_not_added_twice() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        assert!(add_to_gitignore(root, Path::new("src/gen.rs")).unwrap());
        assert_eq!(fs::read_to_string(root.join(".gitignore")).unwrap(), "/src/gen.rs\n");
        assert!(!add_to_gitignore(root, Path::new("src/gen.rs")).unwrap());
        assert!(add_to_gitignore(root, Path::new("notes.txt")).unwrap());
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            "/src/gen.rs\n/notes.txt\n"
        );
    }

    #[test]
    fn a_file_without_a_final_newline_keeps_its_last_line() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        fs::write(root.join(".gitignore"), "target").unwrap();
        add_to_gitignore(root, Path::new("a.txt")).unwrap();
        assert_eq!(fs::read_to_string(root.join(".gitignore")).unwrap(), "target\n/a.txt\n");
    }

    #[test]
    fn special_characters_are_escaped_and_paths_outside_are_refused() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        add_to_gitignore(root, Path::new("#odd[1].txt")).unwrap();
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            "/#odd\\[1].txt\n"
        );
        assert!(add_to_gitignore(root, Path::new("../outside")).is_err());
        assert!(add_to_gitignore(root, Path::new("")).is_err());
    }
}
