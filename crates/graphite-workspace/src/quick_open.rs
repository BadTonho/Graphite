use super::*;
use std::sync::Mutex;

/// The files of a workspace with what fuzzy matching needs, so Quick Open can rank them without
/// walking the disk for every keystroke.
#[derive(Clone, Debug, Default)]
pub struct FileIndex {
    files: Vec<IndexedFile>,
}

#[derive(Clone, Debug)]
struct IndexedFile {
    path: PathBuf,
    /// The path relative to its root, with `/` separators.
    relative: String,
    /// `relative` in lowercase, matched against the lowercase query.
    lower: String,
    /// Characters in `relative`; shorter paths rank higher.
    length: i64,
}

impl Workspace {
    /// Every file of the workspace, as [`Self::fuzzy_files`] finds them: ignore files are
    /// respected, hidden files included, `.git` left out. The roots are walked in parallel.
    /// `None` when `cancelled` returned true before the walk finished.
    pub fn file_index(&self, cancelled: impl Fn() -> bool + Sync) -> Option<FileIndex> {
        let files = Mutex::new(Vec::new());
        for root in &self.roots {
            WalkBuilder::new(root)
                .standard_filters(true)
                .hidden(false)
                .require_git(false)
                .filter_entry(|e| e.file_name() != ".git")
                .build_parallel()
                .run(|| {
                    let files = &files;
                    let cancelled = &cancelled;
                    Box::new(move |entry| {
                        if cancelled() {
                            return ignore::WalkState::Quit;
                        }
                        let Ok(entry) = entry else {
                            return ignore::WalkState::Continue;
                        };
                        if entry.file_type().is_some_and(|t| t.is_file()) {
                            let file = IndexedFile::new(root, entry.into_path());
                            if let Ok(mut files) = files.lock() {
                                files.push(file);
                            }
                        }
                        ignore::WalkState::Continue
                    })
                });
            if cancelled() {
                return None;
            }
        }
        let mut files = files
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        files.sort_by(|a, b| a.relative.cmp(&b.relative));
        Some(FileIndex { files })
    }

    /// Return workspace files ranked by a case-insensitive subsequence match.
    pub fn fuzzy_files(
        &self,
        query: &str,
        limit: usize,
        cancelled: impl Fn() -> bool + Sync,
    ) -> Vec<PathBuf> {
        self.file_index(&cancelled)
            .map(|index| index.fuzzy(query, limit, &cancelled))
            .unwrap_or_default()
    }
}

impl IndexedFile {
    fn new(root: &Path, path: PathBuf) -> Self {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        Self {
            lower: relative.to_lowercase(),
            length: relative.chars().count() as i64,
            relative,
            path,
        }
    }
}

impl FileIndex {
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The indexed paths, sorted by their relative path.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().map(|file| file.path.as_path())
    }

    /// The best `limit` files for `query`, best first; ties go to the alphabetically first path.
    pub fn fuzzy(&self, query: &str, limit: usize, cancelled: impl Fn() -> bool) -> Vec<PathBuf> {
        let query = query.to_lowercase();
        let mut results = Vec::new();
        for (position, file) in self.files.iter().enumerate() {
            // Checking every file would cost more than the match itself.
            if position % 1024 == 0 && cancelled() {
                return Vec::new();
            }
            if let Some(score) = fuzzy_score(&file.lower, file.length, &query) {
                results.push((score, file));
            }
        }
        let rank = |a: &(i64, &IndexedFile), b: &(i64, &IndexedFile)| {
            b.0.cmp(&a.0).then_with(|| a.1.relative.cmp(&b.1.relative))
        };
        if results.len() > limit {
            // Only the best `limit` need sorting.
            results.select_nth_unstable_by(limit, rank);
            results.truncate(limit);
        }
        results.sort_by(rank);
        results
            .into_iter()
            .map(|(_, file)| file.path.clone())
            .collect()
    }
}

/// Scores `lower`, a lowercase path of `length` characters, against the lowercase `query`.
fn fuzzy_score(lower: &str, length: i64, query: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let mut wanted = query.chars();
    let mut next = wanted.next()?;
    let mut score = 0i64;
    let mut last = None;
    let mut previous = None;
    for (i, ch) in lower.chars().enumerate() {
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
                None => return Some(score - length),
            }
        }
        previous = Some(ch);
    }
    None
}
