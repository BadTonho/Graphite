//! Syntax highlighting costs: from scratch versus after one edit, and scope lookups.
//!
//! Run with `cargo bench -p graphite-syntax`. Each case prints the median of several runs.

use graphite_syntax::{enclosing_scope_headers, IncrementalHighlighter, Language, TextEdit};
use ropey::Rope;
use std::time::{Duration, Instant};

/// Runs `case` `runs` times (after one warm-up) and prints the median time.
fn measure(name: &str, runs: usize, mut case: impl FnMut()) {
    case();
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            case();
            start.elapsed()
        })
        .collect();
    times.sort();
    println!(
        "{name:<60} {:>10.3} ms",
        times[runs / 2].as_secs_f64() * 1000.0
    );
}

/// A Rust source of about `lines` lines.
fn source(lines: usize) -> String {
    let item = "/// Doc comment.\nfn item(value: u32) -> u32 {\n    let text = \"string\"; // note\n    value + 1\n}\n\n";
    item.repeat(lines / 6)
}

/// Inserts `text` at `column` (in chars, ASCII lines) of `row` and returns the matching edit.
fn insert_at(rope: &mut Rope, row: usize, column: usize, text: &str) -> TextEdit {
    let char_index = rope.line_to_char(row) + column;
    let byte = rope.char_to_byte(char_index);
    rope.insert(char_index, text);
    TextEdit {
        start_byte: byte,
        old_end_byte: byte,
        new_end_byte: byte + text.len(),
        start_position: (row, column),
        old_end_position: (row, column),
        new_end_position: (row, column + text.len()),
    }
}

fn main() {
    let text = source(20_000);
    let rope = Rope::from_str(&text);
    println!("Rust source of {} lines", rope.len_lines());

    measure("highlight from scratch", 5, || {
        IncrementalHighlighter::new().highlight_rope(Language::Rust, &rope, None);
    });

    // Rows 2 mod 6 hold `    let text = "string";`; column 16 is inside the string, so typing
    // there keeps the code valid. Column 0 of a line puts an `x` before `///` or `fn`, a syntax
    // error, for which everything after the edit is queried again.
    for (kind, column) in [("valid code", 16), ("a syntax error", 0)] {
        for (label, row) in [("top", 8), ("middle", 10_004), ("bottom", 19_988)] {
            let mut highlighter = IncrementalHighlighter::new();
            let mut edited = rope.clone();
            highlighter.highlight_rope(Language::Rust, &edited, None);
            measure(
                &format!("typing near the {label}, leaving {kind}"),
                15,
                || {
                    let edit = insert_at(&mut edited, row, column, "x");
                    drop(highlighter.highlight_rope(Language::Rust, &edited, Some(edit)));
                },
            );
        }
    }

    let mut highlighter = IncrementalHighlighter::new();
    highlighter.highlight_rope(Language::Rust, &rope, None);
    measure("breadcrumb scopes from the kept tree", 15, || {
        highlighter.enclosing_scopes(&rope, 15_000);
    });
    measure("breadcrumb scopes with a fresh parse (before)", 5, || {
        enclosing_scope_headers(Language::Rust, &text, 15_000);
    });
}
