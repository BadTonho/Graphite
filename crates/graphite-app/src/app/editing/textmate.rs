//! Highlighting with the TextMate grammars extensions contribute, for the files the built-in
//! tree-sitter grammars do not cover. A file's language comes from the `contributes.languages`
//! of the enabled extensions, and its grammar from their `contributes.grammars`.

use super::*;
use graphite_plugins::manifest::{GrammarContribution, LanguageContribution, Metadata};
use graphite_syntax::textmate::{parse_grammar_text, DocumentHighlighter, Registry};
use graphite_syntax::Span;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// The grammar and language contributions of one extension, and where it is installed.
pub(crate) struct Source {
    pub directory: PathBuf,
    pub languages: Vec<LanguageContribution>,
    pub grammars: Vec<GrammarContribution>,
}

/// The grammars of the enabled extensions and the highlighting of the open documents that use
/// them.
pub(crate) struct TextMate {
    /// What the engine was built from; a different set of extensions builds a new one.
    pub signature: String,
    registry: Registry,
    /// Language id to the scope of its grammar.
    scopes: HashMap<String, String>,
    /// File name endings (lowercase, with their dot) to language ids.
    extensions: Vec<(String, String)>,
    /// Whole file names (lowercase) to language ids.
    filenames: HashMap<String, String>,
    highlighters: HashMap<DocumentId, DocumentHighlighter>,
}

/// A grammar file inside `directory`; one that points elsewhere is refused.
fn read_grammar(directory: &Path, relative: &str) -> Option<serde_json::Value> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::Prefix(_)))
    {
        return None;
    }
    let path = directory.join(relative);
    let (inside, file) = (directory.canonicalize().ok()?, path.canonicalize().ok()?);
    if !file.starts_with(&inside) {
        return None;
    }
    let text = std::fs::read_to_string(file).ok()?;
    parse_grammar_text(&text).ok()
}

impl TextMate {
    pub(crate) fn new(signature: String, sources: &[Source]) -> Self {
        let mut files: HashMap<String, (PathBuf, String)> = HashMap::new();
        let mut scopes = HashMap::new();
        let mut extensions = Vec::new();
        let mut filenames = HashMap::new();
        for source in sources {
            for grammar in &source.grammars {
                files
                    .entry(grammar.scope_name.clone())
                    .or_insert_with(|| (source.directory.clone(), grammar.path.clone()));
                if let Some(language) = &grammar.language {
                    scopes
                        .entry(language.clone())
                        .or_insert_with(|| grammar.scope_name.clone());
                }
            }
            for language in &source.languages {
                for extension in &language.extensions {
                    extensions.push((extension.to_ascii_lowercase(), language.id.clone()));
                }
                for name in &language.filenames {
                    filenames.insert(name.to_ascii_lowercase(), language.id.clone());
                }
            }
        }
        // The longest ending wins, so `.d.ts` beats `.ts`.
        extensions.sort_by_key(|(extension, _)| std::cmp::Reverse(extension.len()));
        let registry = Registry::new(move |scope| {
            let (directory, relative) = files.get(scope)?;
            read_grammar(directory, relative)
        });
        Self {
            signature,
            registry,
            scopes,
            extensions,
            filenames,
            highlighters: HashMap::new(),
        }
    }

    /// The scope of the grammar for the file at `path`, if an extension contributes one.
    pub(crate) fn scope_for(&self, path: &Path) -> Option<&str> {
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        let language = self.filenames.get(&name).or_else(|| {
            self.extensions
                .iter()
                .find(|(extension, _)| {
                    name.ends_with(extension.as_str()) && name.len() > extension.len()
                })
                .map(|(_, language)| language)
        })?;
        self.scopes.get(language).map(String::as_str)
    }

    /// The highlighting of `text`, the document at `path`, or `None` without a grammar.
    pub(crate) fn highlight(
        &mut self,
        document: DocumentId,
        path: &Path,
        text: &str,
    ) -> Option<Vec<Vec<Span>>> {
        let scope = self.scope_for(path)?.to_owned();
        let base = self.registry.grammar(&scope)?;
        let highlighter = self.highlighters.entry(document).or_default();
        Some(highlighter.highlight(&self.registry, base, text))
    }

    /// Forgets the state of documents that are no longer open.
    pub(crate) fn retain_documents(&mut self, open: impl Fn(DocumentId) -> bool) {
        self.highlighters.retain(|document, _| open(*document));
    }
}

impl App {
    /// The grammars of the enabled extensions, rebuilt when the set of extensions changes.
    fn textmate_engine(&mut self) -> Option<&mut TextMate> {
        let store_root = self
            .plugins
            .store
            .as_ref()
            .map(|store| store.root.clone())?;
        let mut sources = Vec::new();
        let mut signature = String::new();
        for record in &self.plugins.installed {
            let Metadata::Vsix(vsix) = &record.metadata else {
                continue;
            };
            if !record.enabled || vsix.grammars.is_empty() {
                continue;
            }
            signature.push_str(&format!("{}@{};", vsix.id, vsix.version));
            sources.push(Source {
                directory: store_root.join(&record.directory).join("extension"),
                languages: vsix.languages.clone(),
                grammars: vsix.grammars.clone(),
            });
        }
        if sources.is_empty() {
            self.textmate = None;
            return None;
        }
        if self
            .textmate
            .as_ref()
            .is_none_or(|engine| engine.signature != signature)
        {
            self.textmate = Some(TextMate::new(signature, &sources));
            // Documents highlighted by an earlier engine are highlighted again.
            for id in self.documents.ids().collect::<Vec<_>>() {
                if self.documents[id].language.is_none() {
                    self.documents[id].highlights = None;
                }
            }
        }
        self.textmate.as_mut()
    }

    /// Highlights `document` with a contributed grammar when tree-sitter does not know the file.
    pub(crate) fn refresh_textmate_highlights(&mut self, document: DocumentId) {
        let revision = self.documents[document].buffer.revision();
        let Some(path) = self.documents[document]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            self.documents[document].highlights = None;
            return;
        };
        let text = self.documents[document].buffer.text();
        let lines = self
            .textmate_engine()
            .and_then(|engine| engine.highlight(document, &path, &text));
        self.documents[document].highlights = lines.map(|lines| HighlightCache { revision, lines });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_syntax::Class;

    const TOY: &str = r#"{"scopeName":"source.toy","patterns":[
        {"match":"\\b(fn)\\b","name":"keyword.control.toy"},
        {"match":"//.*$","name":"comment.line.toy"},
        {"include":"source.helper"}]}"#;
    const HELPER: &str = r#"{"scopeName":"source.helper","patterns":[{"match":"\\d+","name":"constant.numeric.helper"}]}"#;

    fn source(dir: &Path) -> Source {
        std::fs::create_dir_all(dir.join("syntaxes")).unwrap();
        std::fs::write(dir.join("syntaxes/toy.tmLanguage.json"), TOY).unwrap();
        std::fs::write(dir.join("syntaxes/helper.json"), HELPER).unwrap();
        Source {
            directory: dir.to_path_buf(),
            languages: vec![LanguageContribution {
                id: "toy".into(),
                extensions: vec![".toy".into(), ".D.Toy".into()],
                filenames: vec!["Toyfile".into()],
            }],
            grammars: vec![
                GrammarContribution {
                    language: Some("toy".into()),
                    scope_name: "source.toy".into(),
                    path: "./syntaxes/toy.tmLanguage.json".into(),
                },
                GrammarContribution {
                    language: None,
                    scope_name: "source.helper".into(),
                    path: "syntaxes/helper.json".into(),
                },
            ],
        }
    }

    #[test]
    fn files_get_the_grammar_of_their_language_by_extension_or_name() {
        let dir = TempDir::new();
        let engine = TextMate::new("sig".into(), &[source(dir.path())]);
        assert_eq!(engine.scope_for(Path::new("/w/a.toy")), Some("source.toy"));
        assert_eq!(
            engine.scope_for(Path::new("/w/A.TOY")),
            Some("source.toy"),
            "case does not matter"
        );
        assert_eq!(
            engine.scope_for(Path::new("/w/x.d.toy")),
            Some("source.toy")
        );
        assert_eq!(
            engine.scope_for(Path::new("/w/Toyfile")),
            Some("source.toy")
        );
        assert_eq!(engine.scope_for(Path::new("/w/a.txt")), None);
        assert_eq!(
            engine.scope_for(Path::new("/w/.toy")),
            None,
            "a bare extension is not a file of it"
        );
    }

    #[test]
    fn highlighting_follows_the_grammar_including_the_grammars_it_includes() {
        let dir = TempDir::new();
        let mut engine = TextMate::new("sig".into(), &[source(dir.path())]);
        let text = "fn 12 // c";
        let spans = engine
            .highlight(DocumentId(1), Path::new("/w/a.toy"), text)
            .unwrap();
        let found: Vec<(&str, Class)> = spans[0]
            .iter()
            .map(|s| (&text[s.range.clone()], s.class))
            .collect();
        assert_eq!(
            found,
            [
                ("fn", Class::Keyword),
                ("12", Class::Number),
                ("// c", Class::Comment)
            ]
        );
        assert!(engine
            .highlight(DocumentId(1), Path::new("/w/a.txt"), text)
            .is_none());
        // An edit is highlighted incrementally and gives what a fresh run gives.
        let edited = "fn 12 // c\nfn";
        let again = engine
            .highlight(DocumentId(1), Path::new("/w/a.toy"), edited)
            .unwrap();
        let fresh = TextMate::new("sig".into(), &[source(dir.path())])
            .highlight(DocumentId(9), Path::new("/w/a.toy"), edited)
            .unwrap();
        assert_eq!(again, fresh);
        engine.retain_documents(|_| false);
        assert!(engine.highlighters.is_empty());
    }

    #[test]
    fn open_files_are_highlighted_by_the_grammar_of_an_enabled_extension() {
        use graphite_plugins::manifest::Vsix;
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        let store = graphite_plugins::store::Store::new(app.profile_root.join("plugins"));
        app.plugins.store = Some(store.clone());
        let directory = "test.toy@1.0.0";
        let extension_source = source(&store.root.join(directory).join("extension"));
        let toy = dir.path().join("a.toy");
        std::fs::write(&toy, "fn 7 // x").unwrap();
        let plain = dir.path().join("a.rs");
        std::fs::write(&plain, "fn main() {}").unwrap();
        app.open_file(toy.clone());
        let id = app.documents.id_for_path(&toy).unwrap();
        app.refresh_document_highlights(id);
        assert!(
            app.documents[id].highlights.is_none(),
            "no extension contributes a grammar yet"
        );

        app.plugins
            .installed_plugins_mut()
            .push(graphite_plugins::store::Installed {
                metadata: Metadata::Vsix(Vsix {
                    id: "test.toy".into(),
                    version: "1.0.0".into(),
                    languages: extension_source.languages.clone(),
                    grammars: extension_source.grammars.clone(),
                    ..Default::default()
                }),
                enabled: true,
                trusted_version: Some("1.0.0".into()),
                directory: directory.into(),
                dev_path: None,
            });
        app.refresh_document_highlights(id);
        let lines = &app.documents[id]
            .highlights
            .as_ref()
            .expect("highlighted by the grammar")
            .lines;
        assert_eq!(
            lines[0].iter().map(|span| span.class).collect::<Vec<_>>(),
            [Class::Keyword, Class::Number, Class::Comment]
        );
        // Tree-sitter keeps the languages it knows.
        app.open_file(plain.clone());
        let rust = app.documents.id_for_path(&plain).unwrap();
        assert!(app.documents[rust].language.is_some());
        app.refresh_document_highlights(rust);
        assert!(app.textmate.as_ref().unwrap().scope_for(&plain).is_none());

        // Disabling the extension removes the highlighting again.
        app.plugins.installed_plugins_mut()[0].enabled = false;
        app.refresh_document_highlights(id);
        assert!(app.documents[id].highlights.is_none());
        assert!(app.textmate.is_none());
    }

    #[test]
    fn grammar_files_outside_the_extension_are_refused() {
        let dir = TempDir::new();
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, TOY).unwrap();
        let extension = dir.path().join("ext");
        std::fs::create_dir_all(&extension).unwrap();
        assert!(read_grammar(&extension, "../outside.json").is_none());
        assert!(read_grammar(&extension, &outside.to_string_lossy()).is_none());
        assert!(read_grammar(&extension, "missing.json").is_none());
        std::fs::write(extension.join("ok.json"), TOY).unwrap();
        assert!(read_grammar(&extension, "ok.json").is_some());
        std::fs::write(extension.join("bad.json"), "{not json").unwrap();
        assert!(read_grammar(&extension, "bad.json").is_none());
    }
}
