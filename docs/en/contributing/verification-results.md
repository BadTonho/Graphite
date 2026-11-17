# Local verification results

## Execution on 2026-11-16

Windows, base commit `8035ce5` plus this implementation's working-tree changes. Tools: Rust/Cargo 1.97.0, Node.js 24.18.0, Git 2.53.0.windows.1. Command: `cargo run -p graphite-xtask -- verify`. The verifier executes `cargo test --workspace --no-fail-fast -- --test-threads=4` so a failing target does not prevent other targets from being tested.

| Check | Result | Evidence |
|---|---|---|
| Formatting | FAIL | Existing formatting differences in ten files in auth, core-api, plugins, syntax and text |
| Clippy, warnings denied | FAIL | Existing unused code/variables, type complexity, needless lifetimes, loop and allocation lints in workspace crates; compilation stops before every crate is linted |
| Workspace build | PASS | `cargo build --workspace`, with existing warnings |
| Workspace tests | PASS | 1,761 passed, 0 failed, 10 ignored, summed across all test-target summaries |
| Verifier's own Clippy | PASS | `cargo clippy -p graphite-xtask --all-targets -- -D warnings` |
| Overall verifier | FAIL (exit 1) | Formatting and Clippy remain failing checks |
| Windows interactive UI/GPU and packaged build | PENDING | Headless checks only; no interactive app/package trial was performed |
| Linux build/tests/UI | PENDING | This execution environment is Windows |

Raw command output was captured locally in ignored `target/local-verification.txt`. It is a local artifact, not a tracked release report. Rerun the verifier for the current revision; this record is a dated observation, not a continuously updated status badge.

## Findings addressed

The first run found four failing app tests. Isolated `App` instances still pointed at the real configuration directory, causing denied writes and failure to persist the selected theme. Headless isolated apps now own independent temporary profiles for their lifetime, with a regression covering independence and cleanup. The Emmet integration fixture now stages its worker under a temporary profile instead of the user's profile. All four originally failing tests passed in the final workspace run.

Added observable tests for settings persistence/reload, rejection without overwriting corrupted settings, recovery after repair, failed writes preserving existing data, and sanitized on-disk messages/notifications at every severity. Existing save/close/history/search/update success and failure tests were reused. Four verifier tests cover real subprocess success, failure, continuation, unavailable programs, CLI exit codes and build-before-test order.

## Prioritized remaining work

1. Resolve the preexisting formatting and Clippy findings, then rerun strict checks. Do not suppress warnings globally or declare the verifier green until all checks pass.
2. Complete the [manual checks](manual-verification.md) for the essential workflows and real packages on Windows/Linux; run the existing manual CI at the release revision.
3. Prepare Code OSS fixtures and explicitly run the nine ignored VSIX tests; two involve a public sample/network. Prepare `GRAPHITE_TEST_KERNEL_PYTHON` with ipykernel for the tenth ignored test. Run individual test filters with `--ignored`, not an unconditional network-dependent full suite.
4. Extend behavioral coverage beyond the representative tests in the [matrix](verification-matrix.md), starting with `list.*` command dispatch, which has no direct tests at its handler entrypoint. Review live service interoperability and every runtime error producer separately.

No CI triggers, application version or released changelogs were changed. Version 0.1.0 remains the initial release without a changelog.
