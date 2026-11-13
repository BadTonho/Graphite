//! File nesting rules for the explorer: companion files such as `foo.js.map` or
//! `package-lock.json` are shown indented under their parent (`foo.ts`, `package.json`).

/// `(parent, children)`. A parent starting with `.` is an extension: a child is the parent's
/// base name followed by one of the suffixes (`.js`). Any other parent is an exact file name,
/// and its children are exact file names.
const RULES: &[(&str, &[&str])] = &[
    (".ts", &[".js", ".d.ts", ".js.map", ".d.ts.map"]),
    (".tsx", &[".css", ".module.css"]),
    (".jsx", &[".css", ".module.css"]),
    (".js", &[".js.map", ".min.js"]),
    (".scss", &[".css", ".css.map"]),
    (".less", &[".css"]),
    (
        "package.json",
        &["package-lock.json", "yarn.lock", "pnpm-lock.yaml"],
    ),
    ("Cargo.toml", &["Cargo.lock"]),
    ("composer.json", &["composer.lock"]),
    ("Gemfile", &["Gemfile.lock"]),
];

/// Whether `child` is a companion file of `parent` according to [`RULES`].
pub fn is_companion(parent: &str, child: &str) -> bool {
    if child == parent {
        return false;
    }
    RULES.iter().any(|(key, suffixes)| {
        if key.starts_with('.') {
            parent.strip_suffix(key).is_some_and(|base| {
                !base.is_empty()
                    && suffixes
                        .iter()
                        .any(|suffix| child.strip_prefix(base).is_some_and(|rest| rest == *suffix))
            })
        } else {
            parent == *key && suffixes.contains(&child)
        }
    })
}

/// For each entry, the index of the parent it nests under, if any.
/// `files` lists `(name, is_dir)`; directories never nest and never act as parents.
/// A companion is nested under the first file that is not itself a companion.
pub fn nesting_parents(files: &[(&str, bool)]) -> Vec<Option<usize>> {
    let is_child: Vec<bool> = (0..files.len())
        .map(|child| {
            files
                .iter()
                .enumerate()
                .any(|(parent, (parent_name, parent_dir))| {
                    parent != child
                        && !parent_dir
                        && !files[child].1
                        && is_companion(parent_name, files[child].0)
                })
        })
        .collect();
    (0..files.len())
        .map(|child| {
            if !is_child[child] || files[child].1 {
                return None;
            }
            files
                .iter()
                .enumerate()
                .find_map(|(parent, (parent_name, parent_dir))| {
                    (parent != child
                        && !parent_dir
                        && !is_child[parent]
                        && is_companion(parent_name, files[child].0))
                    .then_some(parent)
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_rules_nest_companions_under_their_base_file() {
        assert!(is_companion("app.ts", "app.js"));
        assert!(is_companion("app.ts", "app.d.ts"));
        assert!(is_companion("app.ts", "app.js.map"));
        assert!(!is_companion("app.ts", "other.js"));
        assert!(!is_companion("app.ts", "app.rs"));
        assert!(!is_companion(".ts", ".js"));
    }

    #[test]
    fn exact_rules_nest_lock_files_under_manifests() {
        assert!(is_companion("package.json", "package-lock.json"));
        assert!(is_companion("Cargo.toml", "Cargo.lock"));
        assert!(!is_companion("Cargo.toml", "package-lock.json"));
        assert!(!is_companion("cargo.toml", "Cargo.lock"));
    }

    #[test]
    fn parents_are_files_that_are_not_companions_themselves() {
        let files = [
            ("app.ts", false),
            ("app.js", false),
            ("app.d.ts", false),
            ("src", true),
            ("notes.md", false),
        ];
        let parents = nesting_parents(&files);
        assert_eq!(parents, vec![None, Some(0), Some(0), None, None]);
    }

    #[test]
    fn a_companion_never_becomes_the_parent_of_another_companion() {
        // `app.js.map` matches `app.js` by rule, but `app.js` is itself a companion of `app.ts`.
        let files = [("app.ts", false), ("app.js", false), ("app.js.map", false)];
        let parents = nesting_parents(&files);
        assert_eq!(parents, vec![None, Some(0), Some(0)]);
    }
}
