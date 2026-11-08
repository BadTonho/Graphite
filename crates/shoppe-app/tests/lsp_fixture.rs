//! Referencing the binary makes Cargo build the stdio fixture before unit tests.
#[test]
fn stdio_fixture_is_built_for_workspace_tests() {
    assert!(std::path::Path::new(env!("CARGO_BIN_EXE_graphite-test-lsp")).is_file());
}
