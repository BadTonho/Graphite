//! Expand Selection from an extension's selection range provider
//! (`registerSelectionRangeProvider`). The provider answers with the chain of ranges that enclose
//! the cursor, innermost first; Graphite selects the first one that is larger than the current
//! selection, like the syntax-tree based expansion it replaces.

use super::*;
use graphite_text::Selection;
use std::ops::Range;

/// The first range of `chain` (1-based Code OSS ranges, innermost first) that contains
/// `start..end` and is larger than it, as char offsets in `text`.
pub(super) fn enclosing_range(
    text: &str,
    chain: &Value,
    start: usize,
    end: usize,
) -> Option<Range<usize>> {
    chain.as_array()?.iter().find_map(|item| {
        let coordinate = |name: &str| {
            u32::try_from(item["range"][name].as_u64()?)
                .ok()?
                .checked_sub(1)
        };
        let from = documents::char_offset(
            text,
            coordinate("startLineNumber")?,
            coordinate("startColumn")?,
        );
        let to =
            documents::char_offset(text, coordinate("endLineNumber")?, coordinate("endColumn")?);
        (from <= start && to >= end && (from < start || to > end)).then_some(from..to)
    })
}

impl App {
    /// Applies the provider's answer for the cursor of the active document.
    pub(super) fn apply_vsix_selection_ranges(
        &mut self,
        document: &graphite_core_api::DocumentSnapshot,
        result: &Value,
    ) {
        let before = self.editor().selection().unwrap_or(Selection {
            anchor: self.editor().cursor().char_idx,
            head: self.editor().cursor().char_idx,
        });
        let (start, end) = (
            before.anchor.min(before.head),
            before.anchor.max(before.head),
        );
        if let Some(next) = enclosing_range(&document.text, &result[0], start, end) {
            self.apply_smart_range(before, next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;

    fn range(l1: u64, c1: u64, l2: u64, c2: u64) -> Value {
        json!({"range": {"startLineNumber": l1, "startColumn": c1, "endLineNumber": l2, "endColumn": c2}})
    }

    #[test]
    fn char_offsets_invert_positions_and_clamp() {
        let text = "ab\nc\u{1F30E}d\n\nend";
        for offset in 0..=text.chars().count() {
            let at = documents::position(text, offset);
            assert_eq!(documents::char_offset(text, at.line, at.character), offset);
        }
        assert_eq!(
            documents::char_offset(text, 0, 99),
            2,
            "clamped to the line end"
        );
        assert_eq!(
            documents::char_offset(text, 99, 0),
            text.chars().count(),
            "past the end"
        );
        assert_eq!(documents::char_offset("", 0, 0), 0);
    }

    #[test]
    fn the_first_strictly_larger_range_that_contains_the_selection_is_chosen() {
        let text = "let value = compute(a, b);\n";
        // Innermost first: `a`, `a, b`, the call, the statement.
        let chain = json!([
            range(1, 21, 1, 22),
            range(1, 21, 1, 25),
            range(1, 13, 1, 26),
            range(1, 1, 1, 27),
        ]);
        assert_eq!(enclosing_range(text, &chain, 20, 20), Some(20..21));
        assert_eq!(enclosing_range(text, &chain, 20, 21), Some(20..24));
        assert_eq!(enclosing_range(text, &chain, 20, 24), Some(12..25));
        assert_eq!(enclosing_range(text, &chain, 12, 25), Some(0..26));
        assert_eq!(enclosing_range(text, &chain, 0, 26), None, "nothing larger");
    }

    #[test]
    fn ranges_that_do_not_contain_the_selection_or_are_malformed_are_skipped() {
        let text = "one two\n";
        let chain = json!([
            range(1, 5, 1, 8),
            {"range": {"startLineNumber": 0, "startColumn": 1, "endLineNumber": 1, "endColumn": 3}},
            "garbage",
            range(1, 1, 1, 8),
        ]);
        assert_eq!(enclosing_range(text, &chain, 0, 3), Some(0..7));
        assert_eq!(enclosing_range(text, &Value::Null, 0, 0), None);
    }

    fn app_with_provider(dir: &TempDir, text: &str) -> (App, LoopbackHost) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.run_ui_command("graphite.security.trustWorkspace");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, text).unwrap();
        app.open_file(file);
        let (session, host) = Session::loopback();
        let mut runtime = Runtime::for_test(session);
        languages::language_rpc(
            &mut runtime.languages,
            "MainThreadLanguageFeatures",
            "$registerSelectionRangeProvider",
            &[json!(1), json!([{"language": "*"}])],
        )
        .unwrap()
        .unwrap();
        app.plugins.vsix = Some(runtime);
        (app, host)
    }

    fn sent(host: &LoopbackHost) -> Vec<Value> {
        std::iter::from_fn(|| host.next_sent())
            .filter(|message| message["method"] == "$provideSelectionRanges")
            .collect()
    }

    fn bounds(app: &App) -> (usize, usize) {
        let selection = app.editor().selection().unwrap();
        (
            selection.anchor.min(selection.head),
            selection.anchor.max(selection.head),
        )
    }

    #[test]
    fn expand_selection_asks_the_provider_and_applies_its_answer_with_shrink_history() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_provider(&dir, "f(alpha, beta);\n");
        let chain = || json!([[range(1, 3, 1, 8), range(1, 3, 1, 14), range(1, 1, 1, 15)]]);
        app.editor_mut().set_cursor_line_col(0, 3);
        app.smart_select(true);
        let requests = sent(&host);
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["args"][2],
            json!([{"lineNumber": 1, "column": 4}])
        );
        assert_eq!(
            app.editor().selection(),
            None,
            "nothing changes until the answer"
        );

        host.emit(json!({"type": "result", "id": requests[0]["id"], "result": chain()}));
        app.poll_vsix();
        assert_eq!(bounds(&app), (2, 7), "the word under the cursor");

        // Expanding again asks again, from the new selection.
        app.smart_select(true);
        let requests = sent(&host);
        host.emit(json!({"type": "result", "id": requests[0]["id"], "result": chain()}));
        app.poll_vsix();
        assert_eq!(bounds(&app), (2, 13));

        app.smart_select(false);
        assert_eq!(bounds(&app), (2, 7));
    }

    #[test]
    fn without_a_provider_the_syntax_tree_expansion_is_used() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn main() { let value = 1; }\n").unwrap();
        app.open_file(file);
        app.editor_mut().set_cursor_line_col(0, 16);
        app.smart_select(true);
        assert!(
            app.editor().selection().is_some(),
            "the local expansion ran synchronously"
        );
    }
}
