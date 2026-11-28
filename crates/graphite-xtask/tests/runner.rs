use graphite_xtask::{
    run_steps, verification_steps, verification_succeeded, workspace_root, Step, FRAMEWORK_MANIFEST,
};
use std::{ffi::OsString, process::Command};

fn helper(name: &'static str, arg: &str) -> Step {
    Step {
        name,
        program: env!("CARGO_BIN_EXE_graphite-xtask").into(),
        args: vec![arg.into()],
    }
}

#[test]
fn runs_real_processes_and_continues_after_failure() {
    let results = run_steps(
        &workspace_root(),
        &[
            helper("success", "--help"),
            helper("failure", "invalid-command"),
            helper("continued", "--help"),
        ],
    );
    assert_eq!(results.len(), 3);
    assert!(results[0].error.is_none());
    assert!(results[1].error.is_some());
    assert!(results[2].error.is_none());
    assert!(!verification_succeeded(&results));
}

#[test]
fn missing_program_is_failure_and_does_not_skip_later_checks() {
    let results = run_steps(
        &workspace_root(),
        &[
            Step {
                name: "unavailable",
                program: workspace_root()
                    .join("target/nonexistent-verification-helper")
                    .into(),
                args: vec![],
            },
            helper("continued", "--help"),
        ],
    );
    assert!(results[0]
        .error
        .as_ref()
        .unwrap()
        .contains("cannot execute"));
    assert!(results[1].error.is_none());
    assert!(!verification_succeeded(&results));
}

#[test]
fn successful_checks_and_invalid_cli_have_correct_exit_status() {
    let results = run_steps(&workspace_root(), &[helper("success", "--help")]);
    assert!(verification_succeeded(&results));
    assert!(!verification_succeeded(&[]));
    for args in [vec![], vec!["verify", "unexpected"], vec!["unknown"]] {
        let status = Command::new(env!("CARGO_BIN_EXE_graphite-xtask"))
            .args(args)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(2));
    }
}

#[test]
fn verification_builds_process_fixtures_before_running_workspace_tests() {
    let steps = verification_steps();
    let args: Vec<Vec<OsString>> = steps.into_iter().map(|step| step.args).collect();
    assert_eq!(args.len(), 7);
    assert_eq!(
        args[0],
        ["fmt", "--all", "--", "--check"].map(OsString::from)
    );
    assert_eq!(args[2], ["build", "--workspace"].map(OsString::from));
    assert_eq!(
        args[3],
        [
            "test",
            "--workspace",
            "--no-fail-fast",
            "--",
            "--test-threads=4"
        ]
        .map(OsString::from)
    );
}

#[test]
fn verification_also_checks_the_framework_workspace() {
    let steps = verification_steps();
    let framework: Vec<_> = steps
        .iter()
        .filter(|step| step.args.iter().any(|arg| arg == FRAMEWORK_MANIFEST))
        .map(|step| step.args[0].clone())
        .collect();
    // The framework is a git submodule, so the manifest may be missing in a checkout made
    // without it; only the commands are checked here.
    assert_eq!(framework, ["fmt", "clippy", "test"].map(OsString::from));
}
