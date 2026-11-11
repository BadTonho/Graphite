use super::*;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks::UTF8, BinaryDetection, SearcherBuilder};
use ignore::overrides::{Override, OverrideBuilder};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::io::{self, Read};

/// How a project search matches text and which files it visits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Treat the query as a regular expression instead of literal text.
    pub regex: bool,
    pub case_sensitive: bool,
    /// Only match whole words (`\b` boundaries around the match).
    pub whole_word: bool,
    /// Glob patterns; when non-empty, only matching files are searched.
    pub include: Vec<String>,
    /// Glob patterns of files to skip.
    pub exclude: Vec<String>,
}

/// Splits a comma-separated filter field into trimmed, non-empty glob patterns.
pub fn parse_glob_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|glob| !glob.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Checks the query and filters without searching, so the UI can report mistakes.
pub fn validate_search(query: &str, options: &SearchOptions) -> Result<(), String> {
    search_pattern(query, options).map_err(|error| error.to_string())?;
    let mut builder = OverrideBuilder::new(".");
    for glob in &options.include {
        builder.add(glob).map_err(|error| error.to_string())?;
    }
    for glob in &options.exclude {
        builder
            .add(&format!("!{glob}"))
            .map_err(|error| error.to_string())?;
    }
    builder.build().map_err(|error| error.to_string())?;
    Ok(())
}

/// The regular expression body for a query, honoring the regex and whole-word options.
fn pattern_body(query: &str, options: &SearchOptions) -> String {
    let body = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    if options.whole_word {
        format!(r"\b(?:{body})\b")
    } else {
        body
    }
}

fn search_pattern(query: &str, options: &SearchOptions) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&pattern_body(query, options))
        .case_insensitive(!options.case_sensitive)
        .build()
}

fn build_overrides(root: &Path, options: &SearchOptions) -> Option<Override> {
    if options.include.is_empty() && options.exclude.is_empty() {
        return None;
    }
    let mut builder = OverrideBuilder::new(root);
    for glob in &options.include {
        builder.add(glob).ok()?;
    }
    for glob in &options.exclude {
        builder.add(&format!("!{glob}")).ok()?;
    }
    builder.build().ok()
}

struct CancellableReader<'a, R, F> {
    source: R,
    cancelled: &'a F,
}

impl<R: Read, F: Fn() -> bool> Read for CancellableReader<'_, R, F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if (self.cancelled)() {
            return Ok(0);
        }
        self.source.read(buf)
    }
}

impl Workspace {
    /// Searches text files using the same ignore rules as the file tree.
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        self.search_cancellable(query, || false)
    }

    /// Stops scanning promptly when a newer search supersedes this request.
    pub fn search_cancellable(
        &self,
        query: &str,
        cancelled: impl Fn() -> bool,
    ) -> Vec<SearchResult> {
        self.search_with_overlays(query, &HashMap::new(), cancelled)
    }

    /// Search visible workspace files with unsaved document contents taking precedence.
    pub fn search_with_overlays(
        &self,
        query: &str,
        overlays: &HashMap<PathBuf, String>,
        cancelled: impl Fn() -> bool,
    ) -> Vec<SearchResult> {
        self.search_with_options(query, &SearchOptions::default(), overlays, cancelled)
    }

    /// Search with regex, case, whole-word and include/exclude options.
    /// Invalid queries or globs produce no results; use `validate_search` to report them.
    pub fn search_with_options(
        &self,
        query: &str,
        options: &SearchOptions,
        overlays: &HashMap<PathBuf, String>,
        cancelled: impl Fn() -> bool,
    ) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }
        let body = pattern_body(query, options);
        let Ok(pattern) = search_pattern(query, options) else {
            return Vec::new();
        };
        let Ok(matcher) = RegexMatcherBuilder::new()
            .case_insensitive(!options.case_sensitive)
            .build(&body)
        else {
            return Vec::new();
        };
        let mut searcher = SearcherBuilder::new()
            .line_number(true)
            .binary_detection(BinaryDetection::quit(b'\0'))
            .build();
        let mut results = Vec::new();
        for root in &self.roots {
            let mut walk = WalkBuilder::new(root);
            walk.standard_filters(true)
                .hidden(false)
                .require_git(false)
                .filter_entry(|e| e.file_name() != ".git");
            if let Some(overrides) = build_overrides(root, options) {
                walk.overrides(overrides);
            }
            for entry in walk.build().filter_map(Result::ok) {
                if cancelled() {
                    break;
                }
                if !entry.file_type().is_some_and(|t| t.is_file()) {
                    continue;
                }
                let path = entry.into_path();
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                let mut file_results = Vec::new();
                let mut sink = UTF8(|line_number, line: &str| {
                    if cancelled() {
                        return Ok(false);
                    }
                    let line = line.trim_end_matches(['\r', '\n']);
                    for found in pattern.find_iter(line) {
                        file_results.push(SearchResult {
                            path: path.clone(),
                            line: line_number as usize - 1,
                            start: found.start(),
                            end: found.end(),
                            text: line.to_owned(),
                        });
                    }
                    Ok(true)
                });
                let outcome = match overlays.get(&canonical) {
                    Some(text) => searcher.search_reader(
                        &matcher,
                        CancellableReader {
                            source: text.as_bytes(),
                            cancelled: &cancelled,
                        },
                        &mut sink,
                    ),
                    None => match std::fs::File::open(&path) {
                        Ok(file) => searcher.search_reader(
                            &matcher,
                            CancellableReader {
                                source: file,
                                cancelled: &cancelled,
                            },
                            &mut sink,
                        ),
                        Err(error) => Err(error),
                    },
                };
                if outcome.is_ok() {
                    results.extend(file_results);
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod options_tests {
    use super::*;
    use std::fs;

    /// Temporary workspace directory, removed when dropped.
    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace_with_files() -> (TempDir, Workspace) {
        let dir = TempDir(std::env::temp_dir().join(format!(
            "graphite_search_options_{}_{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("t").replace("::", "_")
        )));
        let _ = fs::remove_dir_all(&dir.0);
        fs::create_dir_all(dir.0.join("gen")).unwrap();
        fs::write(dir.0.join("main.rs"), "let total = 1;\nTotal2 total\n").unwrap();
        fs::write(dir.0.join("notes.md"), "total in notes\n").unwrap();
        fs::write(dir.0.join("gen/out.rs"), "total generated\n").unwrap();
        let workspace = Workspace::open(&dir.0).unwrap();
        (dir, workspace)
    }

    fn files(results: &[SearchResult]) -> Vec<String> {
        let mut names: Vec<String> = results
            .iter()
            .map(|r| r.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn regex_case_and_whole_word_options_change_matches() {
        let (_dir, workspace) = workspace_with_files();
        let overlays = HashMap::new();
        let literal = SearchOptions::default();
        let found = workspace.search_with_options("total", &literal, &overlays, || false);
        assert_eq!(found.len(), 5);

        let case = SearchOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let found = workspace.search_with_options("Total", &case, &overlays, || false);
        assert_eq!(found.len(), 1);

        let word = SearchOptions {
            whole_word: true,
            case_sensitive: true,
            ..Default::default()
        };
        let found = workspace.search_with_options("total", &word, &overlays, || false);
        assert_eq!(found.len(), 4);

        let regex = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let found = workspace.search_with_options(r"tot\w+\d", &regex, &overlays, || false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Total2 total");
        let literal_dot = workspace.search_with_options(r"tot\w+\d", &literal, &overlays, || false);
        assert!(literal_dot.is_empty());
    }

    #[test]
    fn include_and_exclude_globs_limit_the_visited_files() {
        let (_dir, workspace) = workspace_with_files();
        let overlays = HashMap::new();
        let include = SearchOptions {
            include: vec!["*.rs".into()],
            ..Default::default()
        };
        assert_eq!(
            files(&workspace.search_with_options("total", &include, &overlays, || false)),
            vec!["main.rs", "out.rs"]
        );
        let exclude = SearchOptions {
            exclude: vec!["gen/**".into()],
            ..Default::default()
        };
        assert_eq!(
            files(&workspace.search_with_options("total", &exclude, &overlays, || false)),
            vec!["main.rs", "notes.md"]
        );
    }

    #[test]
    fn invalid_regex_or_glob_is_reported_by_validation() {
        assert!(validate_search("(", &SearchOptions::default()).is_ok());
        let regex = SearchOptions {
            regex: true,
            ..Default::default()
        };
        assert!(validate_search("(", &regex).is_err());
        let bad_glob = SearchOptions {
            include: vec!["[".into()],
            ..Default::default()
        };
        assert!(validate_search("x", &bad_glob).is_err());
        assert_eq!(parse_glob_list(" a/** , ,*.rs "), vec!["a/**", "*.rs"]);
    }
}
