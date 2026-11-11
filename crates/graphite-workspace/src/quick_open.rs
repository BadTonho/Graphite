use super::*;

impl Workspace {
    /// Return workspace files ranked by a case-insensitive subsequence match.
    pub fn fuzzy_files(
        &self,
        query: &str,
        limit: usize,
        cancelled: impl Fn() -> bool,
    ) -> Vec<PathBuf> {
        let query = query.to_lowercase();
        let mut results = Vec::new();
        for root in &self.roots {
            for entry in WalkBuilder::new(root)
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
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Some(score) = fuzzy_score(&relative, &query) {
                    results.push((score, relative, path));
                }
            }
        }
        results.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        results.truncate(limit);
        results.into_iter().map(|(_, _, path)| path).collect()
    }
}

fn fuzzy_score(path: &str, query: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let mut wanted = query.chars();
    let mut next = wanted.next()?;
    let mut score = 0i64;
    let mut last = None;
    let mut previous = None;
    for (i, ch) in path.to_lowercase().chars().enumerate() {
        if ch == next {
            score += 10;
            if last == i.checked_sub(1) && last.is_some() {
                score += 8;
            }
            if i == 0 || previous.is_some_and(|c| matches!(c, '/' | '_' | '-')) {
                score += 5;
            }
            last = Some(i);
            match wanted.next() {
                Some(ch) => next = ch,
                None => return Some(score - path.chars().count() as i64),
            }
        }
        previous = Some(ch);
    }
    None
}
