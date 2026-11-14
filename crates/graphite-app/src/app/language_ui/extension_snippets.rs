//! Snippets contributed by enabled VSIX extensions (`contributes.snippets`). They rank below
//! the user's and the project's snippets: a user snippet with the same prefix and scope wins.

use super::*;
use graphite_plugins::manifest::Metadata;

impl App {
    /// Replaces the user and project snippets and rebuilds the combined list.
    pub(crate) fn set_user_snippets(&mut self, snippets: Vec<config::Snippet>) {
        self.user_snippets = snippets;
        self.merge_snippets();
    }

    /// Re-reads the snippet files of every enabled VSIX extension.
    pub(crate) fn reload_extension_snippets(&mut self) {
        let Some(root) = self.plugins.store.as_ref().map(|store| store.root.clone()) else {
            return;
        };
        let translator = self.translator;
        let mut snippets = Vec::new();
        for record in self
            .plugins
            .installed
            .iter()
            .filter(|record| record.enabled)
        {
            let (base_dir, contributions): (
                std::path::PathBuf,
                &[graphite_plugins::manifest::SnippetContribution],
            ) = match &record.metadata {
                Metadata::Vsix(vsix) => (
                    root.join(&record.directory).join("extension"),
                    &vsix.snippets,
                ),
                Metadata::Native(native) => (record.install_dir(&root), &native.snippets),
            };
            for contribution in contributions {
                let path = base_dir.join(&contribution.path);
                let (found, _warnings) = config::parse_snippet_file(&path, &translator);
                snippets.extend(found.into_iter().map(|mut snippet| {
                    // A snippet file without its own scope applies to the contributing language.
                    snippet
                        .scope
                        .get_or_insert_with(|| vec![contribution.language.clone()]);
                    snippet
                }));
            }
        }
        if snippets != self.extension_snippets {
            self.extension_snippets = snippets;
            self.merge_snippets();
        }
    }

    fn merge_snippets(&mut self) {
        let mut merged = merge(&self.extension_snippets, &self.user_snippets);
        merged.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        self.snippets = merged;
    }
}

/// Extension snippets that no user snippet overrides, followed by the user snippets.
fn merge(extension: &[config::Snippet], user: &[config::Snippet]) -> Vec<config::Snippet> {
    let overridden = user
        .iter()
        .map(|snippet| (snippet.prefix.as_str(), snippet.scope.as_ref()))
        .collect::<std::collections::HashSet<_>>();
    extension
        .iter()
        .filter(|snippet| !overridden.contains(&(snippet.prefix.as_str(), snippet.scope.as_ref())))
        .chain(user)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(prefix: &str, body: &str, scope: Option<&str>) -> config::Snippet {
        config::Snippet {
            prefix: prefix.into(),
            body: body.into(),
            scope: scope.map(|scope| vec![scope.to_owned()]),
        }
    }

    #[test]
    fn user_snippets_override_extension_snippets_with_the_same_prefix_and_scope() {
        let extension = [
            snippet("fn", "extension fn", Some("rust")),
            snippet("fn", "extension go fn", Some("go")),
            snippet("only", "extension only", Some("rust")),
        ];
        let user = [snippet("fn", "user fn", Some("rust"))];
        let merged = merge(&extension, &user);
        let body = |prefix: &str, scope: &str| {
            merged
                .iter()
                .find(|snippet| {
                    snippet.prefix == prefix
                        && snippet.scope.as_deref() == Some(&[scope.into()][..])
                })
                .map(|snippet| snippet.body.as_str())
        };
        assert_eq!(body("fn", "rust"), Some("user fn"));
        assert_eq!(body("fn", "go"), Some("extension go fn"));
        assert_eq!(body("only", "rust"), Some("extension only"));
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn extension_snippet_files_are_loaded_for_enabled_extensions_only() {
        use std::io::Write;
        let dir = crate::test_support::TempDir::new();
        let package = dir.path().join("snippets.vsix");
        let manifest = serde_json::json!({
            "publisher": "test", "name": "snips", "version": "1.0.0",
            "engines": {"vscode": "^1.100.0"},
            "contributes": {"snippets": [{"language": "plaintext", "path": "./snippets/a.json"}]},
        });
        let mut archive = zip::ZipWriter::new(std::fs::File::create(&package).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("extension/package.json", options)
            .unwrap();
        serde_json::to_writer(&mut archive, &manifest).unwrap();
        archive
            .start_file("extension/snippets/a.json", options)
            .unwrap();
        archive
            .write_all(br#"{"Hello": {"prefix": "hello", "body": ["Hello ${1:world}"]}}"#)
            .unwrap();
        archive.finish().unwrap();

        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        let store = graphite_plugins::store::Store::new(app.profile_root.join("plugins"));
        store.install(&package).unwrap();
        app.plugins.store = Some(store.clone());
        app.plugins.installed = store.list().unwrap();
        app.reload_extension_snippets();
        assert!(
            app.snippets.is_empty(),
            "disabled extensions contribute nothing"
        );

        store.enable("test.snips", true).unwrap();
        app.plugins.installed = store.list().unwrap();
        app.reload_extension_snippets();
        let hello = app
            .snippets
            .iter()
            .find(|snippet| snippet.prefix == "hello")
            .unwrap();
        assert_eq!(hello.body, "Hello world");
        assert_eq!(hello.scope.as_deref(), Some(&["plaintext".to_owned()][..]));

        app.set_user_snippets(vec![snippet("hello", "user hello", Some("plaintext"))]);
        assert_eq!(app.snippets.len(), 1);
        assert_eq!(app.snippets[0].body, "user hello");
    }
}
