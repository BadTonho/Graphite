use super::*;
use regex::Regex;

impl Workspace {
    /// Searches text files using the same ignore rules as the file tree.
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }
        let Ok(pattern) = Regex::new(&regex::escape(query)) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        for entry in WalkBuilder::new(&self.root)
            .standard_filters(true)
            .hidden(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.into_path();
            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (line_idx, line) in contents.lines().enumerate() {
                for found in pattern.find_iter(line) {
                    let start = found.start();
                    let end = found.end();
                    results.push(SearchResult {
                        path: path.clone(),
                        line: line_idx,
                        start,
                        end,
                        text: line.to_owned(),
                    });
                }
            }
        }
        results
    }
}
