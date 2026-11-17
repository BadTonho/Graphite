use graphite_xtask::{run_steps, verification_steps, verification_succeeded, workspace_root};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("Usage: cargo run -p graphite-xtask -- verify");
        return ExitCode::SUCCESS;
    }
    if args.len() != 1 || args[0] != "verify" {
        eprintln!("Usage: cargo run -p graphite-xtask -- verify");
        return ExitCode::from(2);
    }

    let results = run_steps(&workspace_root(), &verification_steps());
    println!("\n== Local verification summary ==");
    for result in &results {
        let status = if result.error.is_none() {
            "PASS"
        } else {
            "FAIL"
        };
        println!(
            "{status}: {} ({:.1}s)",
            result.name,
            result.elapsed.as_secs_f64()
        );
        if let Some(error) = &result.error {
            println!("  {error}");
        }
    }
    println!("Ignored tests: see Cargo test summaries above; they are not passes.");
    println!("Pending: interactive UI/GPU, packaged application and the other OS.");
    println!("See docs/en/contributing/verification-matrix.md and manual-verification.md.");
    if verification_succeeded(&results) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
