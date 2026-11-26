//! Quick Open and project search over a generated workspace: walking the disk per query versus
//! ranking from the file index, and searching file contents.
//!
//! Run with `cargo bench -p graphite-workspace`. Each case prints the median of several runs.
//! The workspace is generated in the system temporary folder and removed afterwards.

use graphite_workspace::{SearchOptions, Workspace};
use std::collections::HashMap;
use std::fs;
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
    let root = std::env::temp_dir().join(format!("graphite-bench-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let body =
        "fn handler(request: Request) -> Response {\n    respond(request.path())\n}\n".repeat(40);
    for folder in 0..40 {
        let dir = root.join(format!("src/module_{folder:02}/nested"));
        fs::create_dir_all(&dir).unwrap();
        for file in 0..50 {
            fs::write(dir.join(format!("file_{file:02}.rs")), &body).unwrap();
        }
    }
    let workspace = Workspace::open(&root).unwrap();
    let index = workspace.file_index(|| false).unwrap();
    println!("Workspace of {} files", index.len());

    measure("Quick Open query walking the disk (before)", 10, || {
        workspace.fuzzy_files("modnestfile", 100, || false);
    });
    measure("Quick Open query from the file index", 50, || {
        index.fuzzy("modnestfile", 100, || false);
    });
    measure("building the file index", 10, || {
        workspace.file_index(|| false);
    });
    measure("project search for a rare word", 10, || {
        workspace.search_with_options(
            "nowhere_to_be_found",
            &SearchOptions::default(),
            &HashMap::new(),
            || false,
        );
    });
    measure("project search for a word with 80,000 matches", 10, || {
        workspace.search_with_options(
            "respond",
            &SearchOptions::default(),
            &HashMap::new(),
            || false,
        );
    });

    let _ = fs::remove_dir_all(&root);
}
