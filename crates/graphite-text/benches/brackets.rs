//! Bracket colors for the visible lines: a scan from the document start versus remembered
//! depths, and the cost after an edit.
//!
//! Run with `cargo bench -p graphite-text`. Each case prints the median of several runs.

use graphite_text::{bracket_levels, BracketDepths, Buffer, SelectionState, ViewId};
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

fn main() {
    let text = "fn f(a: [u8; 4]) -> Option<u8> { if a[0] > 0 { Some(a[1]) } else { None } }\n"
        .repeat(100_000);
    let mut document = Buffer::from_text("bench.rs", &text);
    println!("Document of {} lines", document.rope().len_lines());
    let first = 99_950;
    let end = document.rope().len_chars();

    measure(
        "visible lines at the end, scan from the start (before)",
        10,
        || {
            bracket_levels(document.rope(), end);
        },
    );
    let mut depths = BracketDepths::default();
    measure("visible lines at the end, remembered depths", 50, || {
        depths.levels(&document, first, end);
    });

    let mut view = SelectionState::default();
    measure("after typing near the visible lines", 50, || {
        {
            let mut editor = document.edit(&mut view, ViewId(1));
            let at = editor.rope().line_to_char(first);
            editor.set_selection(at, at);
            editor.insert("(");
        }
        let end = document.rope().len_chars();
        depths.levels(&document, first, end);
    });
}
