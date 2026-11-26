# Regression testing

The stage 5 suite consolidates document/view, command, service, host and asynchronous-save contracts. Its fixtures use temporary directories, owned mock data and local Rust processes without a GPU, network access, personal files or an installed language server. The full workspace also includes native-plugin and Emmet integration tests requiring Node.js 24+, plus local process/compiler/Git tests; install the prerequisites in [building](building.md). Code OSS/network and real-ipykernel tests are explicitly ignored by default.

## Running the suite

Run from the repository root:

```sh
cargo run -p graphite-xtask -- verify
```

The local verifier checks formatting, runs Clippy with warnings denied, builds the workspace (including process helpers), and runs workspace tests with four threads. It runs every step even after failure, preserves command output and prints durations and a final PASS/FAIL summary. Exit code 0 means all four automated steps passed; failures or unavailable tools return 1, and invalid arguments return 2. It does not install tools, rewrite source files, launch the app or trigger CI. Cargo may populate its normal build/dependency caches; a fully offline first build is not guaranteed.

Requires the existing Rust/native build prerequisites plus the `rustfmt` and `clippy` toolchain components. Use the same command on Windows and Linux. No Python, PowerShell or additional Rust dependencies are needed by the verifier. Run it after a completed change; use filtered tests during development and the existing manual CI before releases.

CI runs the same formatting check (on Linux) and Clippy step with warnings denied (on Windows and Linux) before the tests, so a change that passes the verifier locally passes those steps too, except for code that only compiles on the other system.

Ignored tests remain visible in Cargo's output and are not passes. A successful run does not validate the other operating system, real service integrations, UI/GPU or the packaged application. See the [feature verification matrix](verification-matrix.md), [manual verification checklist](manual-verification.md) and [recorded local results](verification-results.md).

Workspace tests build the `graphite-test-lsp` stdio fixture through its integration target. Before running only the application unit tests, build both process helpers:

```sh
cargo build -p graphite-extension-host
cargo build -p graphite-app --bin graphite-test-lsp
cargo test -p graphite-app --bin graphite
```

Useful filters for the last command are `document_view_tests`, `accessibility_actions::tests`, `services::tests`, `workspace_edit::tests`, `command_tests`, `lsp::stdio_tests` and `saving::`. Run `cargo test -p graphite-extension-host` for framing, supervisor and real-host process tests, and `cargo test -p graphite-text` for transaction and file-write contracts.

The fixture binary lives under application test infrastructure and is never started by the editor's default configuration. It implements only scripted test commands and standard LSP framing. It is not a language server for end users.

## Stability checks on Windows

- `cargo test -p graphite-app --bin graphite interrupted_save_preserves_last_complete_version_on_reopen` terminates a headless editor process after the save worker syncs its temporary file and before it replaces the document. A new editor instance opens the last completed save, verifies the text and line endings, and saves another edit.
- `cargo test -p graphite-app --bin graphite draft_recovery` terminates a separate editor process with durable unsaved drafts and exercises recovery, conflict handling, profile isolation and cleanup. See [draft recovery](../features/draft-recovery.md).
- `cargo test -p graphite-app --bin graphite window_close_buttons_save_discard_or_keep_edits` exercises the real dialog's click handler and checks the file on disk and whether the window would close for each choice. Run the [manual checklist](manual-verification.md) for physical input, rendering, and the installed application.
- `cargo test -p graphite-app --test startup_update` starts a disposable copy of the executable three times, rejects a staged update whose checksum does not match, and verifies that the original executable is still usable. Set `GRAPHITE_TEST_PACKAGED_EXE` to the absolute path of the `graphite.exe` extracted from the release archive or installed by the setup program to run the same check against that executable.
- In a disposable Windows account, set `GRAPHITE_TEST_WINDOWS_INSTALLER` to the real setup executable and run `python -m unittest discover -s packaging/release/tests -p test_windows_install.py -v`. The test installs into a temporary directory, checks the bundled runtime, shortcut, uninstall registration and first CLI launch, then runs the uninstaller and verifies removal. The real installer writes to that account's Start menu and registry.

## Measuring performance

Benchmarks use only the standard library and print the median time of each case:

```sh
cargo bench -p graphite-syntax     # highlighting from scratch and while typing, scope lookups
cargo bench -p graphite-text       # bracket colors with and without remembered depths
cargo bench -p graphite-workspace  # Quick Open with and without the file index, project search
```

Cases marked "(before)" run the approach the editor used earlier, so each run shows the gain on the same machine. The workspace benchmark generates 2,000 files in the system temporary folder and removes them. Typing benchmarks cover both an edit that keeps the code valid and one that leaves a syntax error, since highlighting re-checks everything after the edit while the code has an error. Times depend on the machine; compare runs made on the same one.

## Measuring code coverage

With `cargo-llvm-cov` and the matching Rust `llvm-tools-preview` component available, run `cargo llvm-cov --workspace --locked --html --output-dir target/graphite-coverage`. Open `target/graphite-coverage/html/index.html` to see which lines ran and inspect gaps per file. Coverage depends on the operating system and the scenarios executed, including which optional tests ran.

## Coverage matrix

Names below identify native test modules or individual regression tests; existing coverage is reused rather than duplicated.

| Requirement | Automated evidence |
|---|---|
| Independent selections, scroll, IME and multicursor | `document_view_tests`, especially `multicursor_transactions_noop_and_history_keep_peer_state_independent`; `accessibility_actions::tests::accessible_multicursor_edit_preserves_focus_scroll_and_peer_ime` |
| Clipboard, search/replace, completion and line edits | `search_completion_clipboard_and_line_commands_remap_peer_views`; command parity tests in `command_tests::parity` |
| History origin, closed origin, close/reopen and retention | `undo_restores_origin_selections_without_changing_focus_or_scroll`, `closed_origin_is_not_resurrected_by_undo_and_reopened_view_has_new_identity`; `workspace_edit::tests` |
| Versions, no-op, redo and saved text/format | `graphite-text` transaction tests; `unchanged_save_keeps_revision_and_redo_history`; `saved_text_and_format_baselines_survive_undo_redo_and_format_roundtrip` |
| Open/closed/untitled atomic UTF-16 edits | `services::tests::mixed_open_closed_untitled_batch_publishes_only_after_success`; `stdio_apply_edit_uses_event_loop_transactions_and_rejects_partial_batches` |
| Rejection preserves text, history, views, events, leases and disk | `rejected_batches_preserve_views_redo_events_leases_and_disk` covers missing reads, overlap, surrogate splitting, stale versions and read-only documents |
| Changes between prepare and commit | `workspace_edit::tests` covers live version changes, changed closed-file contents, permission changes, newly opened targets and cancelled preparation |
| Command inventory, shortcuts, metadata and arguments | `command_tests::inventory_preserves_every_legacy_id_and_default_binding`, duplicate registration and keybinding argument tests |
| Context precedence, lifetime, fresh enablement and separate visibility | `command_tests` scope, enablement and widget tests; `readonly_widget_rejection_preserves_suggestions_selection_and_history` |
| Equivalent keyboard, widget and API edits | `keyboard_widget_and_programmatic_completion_have_identical_document_effects` compares text, versions, both views' selections, focus and scroll |
| Async cancellation, failure and exactly-once completion | `command_tests` worker tests; `lsp::requests::tests`; host pending-request tests |
| Real LSP worker initialization and JSON results | `lsp::stdio_tests::stdio_initialization_execute_command_and_malformed_response` |
| LSP applyEdit, malformed responses, cancel, late replies and EOF | `lsp::stdio_tests`; simulated-channel `lsp::command_tests` also covers stale/replaced server instances and service transactions |
| Participant ordering, timeout, invalid results and typing during preparation | `saving::tests`; `typing_and_format_change_during_host_participant_save_latest_snapshot`; LSP save-participant tests |
| Typing during writing, per-document queues and concurrent documents | `saving::tests`; `saving::view_tests::separate_documents_write_concurrently_without_waiting_for_other_document` |
| Write failures, permissions, encoding/EOL and temporary cleanup | `saving::tests`, `saving::view_tests`, file-format tests and `graphite-text` file I/O tests |
| Save All partial failure | `save_all_keeps_successful_file_and_dirty_untitled_after_partial_failure` verifies successful writes stay confirmed and untitled views remain dirty/open |
| Pending close, workspace decisions, discard and CLI --wait | `saving::lifecycle_tests`; command decision tests |
| Real host crash, cleanup, restart and surviving editor/LSP | `graphite-extension-host/tests/process.rs`; `lsp::command_tests::real_host_crash_cleans_resources_but_editor_and_lsp_continue_then_restart`; save-host crash tests |

## Determinism and failure assertions

Deadline tests pass controlled `Instant` values to request trackers and coordinator polls; they do not wait one or thirty seconds to exercise expiration. Worker tests use channels to pause before/after commit claim. Protocol tests synchronize through replies and stream-ordered acknowledgements. Bounded receive/pump deadlines detect hangs; elapsed time is not proof of an operation completing.

Rejection tests check observable state and subscriptions, then exercise redo to establish that history survived. Successful workspace edits remain in memory until explicitly saved. Save All is intentionally atomic per document: one failure does not undo other successful writes. A cancelled requester cannot reverse a write whose commit was already claimed.

## Platform and manual checks

This stage is validated with native automated tests and builds on Windows. Linux execution and interactive Windows/Linux UI/GPU checks have not been performed in this Windows session. Headless accessibility trees/actions and IME state tests do not replace OS assistive-technology, input-method or rendering checks.

Manual follow-up should verify split-view typing and IME, clipboard integration, accessibility navigation, independent scrolling, visible save warnings, save/discard/cancel dialogs and window responsiveness while saving. Record the operating system, checks run and any failures instead of marking unavailable checks as passed.

Before delivery, review `git status`, the full diff, staged content and `.gitignore` for secrets and temporary artifacts. `sugestoes.md` remains an ignored local checklist; the coverage matrix is tracked documentation. No commit or push is implied by validation.

The real Code OSS/Node host, its pinned version, a real-extension test and `.vsix` compatibility remain mandatory later work. This suite makes no compatibility claim for them.
