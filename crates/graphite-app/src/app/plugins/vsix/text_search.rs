//! `workspace.findTextInFiles`: the extension host asks Graphite to search the workspace and
//! receives matches through `$handleTextSearchResult` before the request completes.

use super::*;
use graphite_workspace::{SearchOptions, SearchResult};

/// Largest number of matches returned when the extension sets no limit.
const DEFAULT_LIMIT: usize = 5000;
/// A search that takes longer than this stops and reports that the limit was hit, so one
/// request cannot freeze the window.
const TIME_BUDGET: Duration = Duration::from_secs(5);

/// Glob patterns from a pattern expression: `{ "glob": true }`, a string, or a list of either.
fn glob_patterns(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) if !map.contains_key("pattern") => map
            .iter()
            .filter(|(_, enabled)| enabled.as_bool() != Some(false))
            .map(|(glob, _)| glob.clone())
            .collect(),
        other => workbench::patterns(other),
    }
}

/// UTF-16 column of byte offset `byte` in `line`, the unit of extension ranges.
fn utf16_column(line: &str, byte: usize) -> usize {
    line.get(..byte)
        .map_or(byte, |prefix| prefix.encode_utf16().count())
}

/// Groups matches by file as `IRawFileMatch2` values, keeping at most `limit` matches.
/// The flag tells whether further matches were dropped.
pub(super) fn file_matches(results: &[SearchResult], limit: usize) -> (Vec<Value>, bool) {
    let mut files: Vec<(PathBuf, Vec<Value>)> = Vec::new();
    for result in results.iter().take(limit) {
        let start = utf16_column(&result.text, result.start);
        let end = utf16_column(&result.text, result.end);
        let line = result.line;
        let range = |first, last| json!({"startLineNumber": line, "startColumn": first, "endLineNumber": line, "endColumn": last});
        let item = json!({
            "rangeLocations": [{"source": range(start, end), "preview": range(start, end)}],
            "previewText": result.text,
        });
        match files.iter_mut().find(|(path, _)| *path == result.path) {
            Some((_, items)) => items.push(item),
            None => files.push((result.path.clone(), vec![item])),
        }
    }
    let matches = files
        .into_iter()
        .map(|(path, items)| json!({"resource": workbench::file_uri(&path), "results": items}))
        .collect();
    (matches, results.len() > limit)
}

/// Answers `MainThreadWorkspace.$startTextSearch`, or returns `None` for other requests.
pub(super) fn dispatch(
    app: &mut App,
    runtime: &mut Runtime,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    if (actor, method) != ("MainThreadWorkspace", "$startTextSearch") {
        return None;
    }
    Some(search(app, runtime, args))
}

fn search(app: &mut App, runtime: &mut Runtime, args: &[Value]) -> Result<Value> {
    let query = args.first().ok_or("Missing text search query")?;
    let options = args.get(2).unwrap_or(&Value::Null);
    let request = args
        .get(3)
        .and_then(Value::as_u64)
        .ok_or("Missing text search request")?;
    let pattern = query["pattern"].as_str().unwrap_or_default();
    let search_options = SearchOptions {
        regex: query["isRegExp"].as_bool().unwrap_or(false),
        case_sensitive: query["isCaseSensitive"].as_bool().unwrap_or(false),
        whole_word: query["isWordMatch"].as_bool().unwrap_or(false),
        include: glob_patterns(&options["includePattern"]),
        exclude: glob_patterns(&options["excludePattern"]),
    };
    graphite_workspace::validate_search(pattern, &search_options)?;
    let limit = options["maxResults"]
        .as_u64()
        .map_or(DEFAULT_LIMIT, |limit| limit as usize);
    let started = Instant::now();
    let results = app.tree.workspace_snapshot().search_with_options(
        pattern,
        &search_options,
        &Default::default(),
        || started.elapsed() > TIME_BUDGET,
    );
    let timed_out = started.elapsed() > TIME_BUDGET;
    let (matches, truncated) = file_matches(&results, limit);
    for file in matches {
        runtime.session.send(json!({
            "type": "call", "actor": "ExtHostWorkspace", "method": "$handleTextSearchResult",
            "args": [file, request],
        }))?;
    }
    Ok(json!({"limitHit": truncated || timed_out}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(path: &str, line: usize, text: &str, start: usize, end: usize) -> SearchResult {
        SearchResult {
            path: PathBuf::from(path),
            line,
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn matches_are_grouped_by_file_with_utf16_columns() {
        let results = vec![
            found("/w/a.txt", 0, "x🌎 needle", 5, 11),
            found("/w/b.txt", 3, "needle", 0, 6),
            found("/w/a.txt", 7, "needle needle", 7, 13),
        ];
        let (files, truncated) = file_matches(&results, 10);
        assert!(!truncated);
        assert_eq!(files.len(), 2);
        let first = &files[0]["results"];
        assert_eq!(first.as_array().unwrap().len(), 2);
        // The emoji takes four bytes but two UTF-16 units.
        assert_eq!(first[0]["rangeLocations"][0]["source"]["startColumn"], 3);
        assert_eq!(first[0]["rangeLocations"][0]["source"]["endColumn"], 9);
        assert_eq!(first[0]["previewText"], "x🌎 needle");
        assert_eq!(
            files[1]["results"][0]["rangeLocations"][0]["source"]["startLineNumber"],
            3
        );
    }

    #[test]
    fn the_match_limit_truncates_and_is_reported() {
        let results = vec![found("/w/a", 0, "aa", 0, 1), found("/w/a", 1, "aa", 0, 1)];
        let (files, truncated) = file_matches(&results, 1);
        assert!(truncated);
        assert_eq!(files[0]["results"].as_array().unwrap().len(), 1);
        assert!(!file_matches(&results, 2).1);
    }

    #[test]
    fn glob_expressions_accept_maps_strings_and_lists() {
        let mut patterns = glob_patterns(&json!({"**/*.rs": true, "**/*.md": false}));
        patterns.sort();
        assert_eq!(patterns, ["**/*.rs"]);
        assert_eq!(glob_patterns(&json!("**/*.txt")), ["**/*.txt"]);
        assert_eq!(glob_patterns(&json!(["a", "b"])), ["a", "b"]);
        assert!(glob_patterns(&Value::Null).is_empty());
    }
}
