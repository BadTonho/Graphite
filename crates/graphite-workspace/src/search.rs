use super::*;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks::UTF8, BinaryDetection, SearcherBuilder};
use regex::Regex;
use std::collections::HashMap;
use std::io::{self, Read};

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
        if query.is_empty() {
            return Vec::new();
        }
        let Ok(pattern) = Regex::new(&regex::escape(query)) else {
            return Vec::new();
        };
        let Ok(matcher) = RegexMatcherBuilder::new().fixed_strings(true).build(query) else {
            return Vec::new();
        };
        let mut searcher = SearcherBuilder::new()
            .line_number(true)
            .binary_detection(BinaryDetection::quit(b'\0'))
            .build();
        let mut results = Vec::new();
        for entry in WalkBuilder::new(&self.root)
            .standard_filters(true)
            .hidden(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build()
            .filter_map(Result::ok)
        {
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
        results
    }
}
