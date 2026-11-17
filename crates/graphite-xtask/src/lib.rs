//! Local development checks. This tool never launches the editor or installs tools.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

pub struct Step {
    pub name: &'static str,
    pub program: OsString,
    pub args: Vec<OsString>,
}

pub struct StepResult {
    pub name: &'static str,
    pub elapsed: Duration,
    pub error: Option<String>,
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask belongs to the workspace crates directory")
        .to_path_buf()
}

pub fn verification_steps() -> Vec<Step> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    [
        ("Formatting", vec!["fmt", "--all", "--", "--check"]),
        (
            "Clippy",
            vec![
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("Build", vec!["build", "--workspace"]),
        (
            "Tests",
            vec![
                "test",
                "--workspace",
                "--no-fail-fast",
                "--",
                "--test-threads=4",
            ],
        ),
    ]
    .into_iter()
    .map(|(name, args)| Step {
        name,
        program: cargo.clone(),
        args: args.into_iter().map(OsString::from).collect(),
    })
    .collect()
}

/// Run every step even after failure. Child output stays visible, including ignored tests.
pub fn run_steps(root: &Path, steps: &[Step]) -> Vec<StepResult> {
    steps
        .iter()
        .map(|step| {
            println!("\n== {} ==", step.name);
            let start = Instant::now();
            let error = match Command::new(&step.program)
                .args(&step.args)
                .current_dir(root)
                .status()
            {
                Ok(status) if status.success() => None,
                Ok(status) => Some(format!("process {status}")),
                Err(error) => Some(format!("cannot execute {:?}: {error}", step.program)),
            };
            StepResult {
                name: step.name,
                elapsed: start.elapsed(),
                error,
            }
        })
        .collect()
}

pub fn verification_succeeded(results: &[StepResult]) -> bool {
    !results.is_empty() && results.iter().all(|result| result.error.is_none())
}
