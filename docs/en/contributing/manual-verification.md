# Manual verification

Run `cargo run -p graphite-xtask -- verify` first and record any failures. Use a disposable workspace and an isolated Graphite profile (a disposable OS account for package checks); never use personal files or credentials as fixtures. These checks are pending until someone records the OS, app revision, package/source build, scenario and observed result. Do not treat an unchecked item as a pass.

## Essential smoke checks

- [ ] Start the application, open a temporary folder, resize the window and use keyboard and mouse. Confirm no rendering corruption or startup errors.
- [ ] Create a file containing accents, emoji and multiple lines; edit, undo/redo, save, close and reopen. Confirm exact saved contents.
- [ ] Find and replace text, including a regex and a no-match search; confirm the changes and undo result.
- [ ] Open split views, type with an IME, scroll independently, and copy/paste through the OS clipboard. Confirm the other view stays consistent.
- [ ] Close a dirty document using Save, Discard and Cancel in separate trials. Confirm saved contents, preserved edits on Cancel and a responsive window.
- [ ] Change a setting and restart. Confirm persistence. In the disposable profile, introduce malformed JSON and confirm an understandable error and preservation of the file; repair it and confirm recovery.
- [ ] Open a terminal, run a harmless command, stop it and close the terminal. Confirm input, output, resize and shell process cleanup.
- [ ] Trigger a safe failure, such as opening a missing temporary file. Inspect the applicable severity file under `<app_config_dir>/logs/errors/` for valid JSONL and an appropriate error. Never deliberately crash the personal profile or insert real secrets.

## Feature checks after relevant changes

- [ ] Explorer create/rename/copy/move/delete; cancel deletion of a dirty file. Verify disk and tabs agree.
- [ ] Themes, icons, folding, menu navigation, shortcuts, focus and accessibility: confirm visible output, pointer targets and screen-reader/IME behavior.
- [ ] Git in a disposable repository: modify, stage, commit, switch branch and resolve a conflict. Confirm status, diff and disk agree.
- [ ] Language tools with a configured local server: completion, navigation, rename and formatting; terminate the server and confirm editor recovery.
- [ ] Debug a disposable program with a configured adapter: breakpoint, step, stop, prelaunch failure and cancellation.
- [ ] Packaged extensions: install a known local fixture VSIX, trust, activate, invoke, disable and restart. Check language providers, terminal events, webviews and Test Explorer using the detailed [Windows checklist](windows-manual-testing.md).
- [ ] Notebooks: open/save, edit cells, run/cancel with a prepared disposable kernel, and confirm outputs and kernel cleanup.
- [ ] Chat, sign-in, sync and remote: test only with explicitly prepared test accounts/services; confirm cancellation, offline errors and conflict recovery. Local mocked tests do not establish live interoperability.

## Before a release

- [ ] Run the existing manual CI for Windows and Linux at the release revision; record its results.
- [ ] Build and launch the actual distributable on each OS following [building](building.md), then repeat essential smoke checks.
- [ ] With disposable installations and controlled update artifacts, verify update, restart, version, rollback and refusal of an invalid package. Do not use the developer's running executable as a test target.
- [ ] On Linux, explicitly check X11/Wayland and GTK/WebKit lifecycle; on Windows, WebView2 and shell integration. Automated headless tests do not validate these.

Record each result as PASS, FAIL or PENDING, including the reason and follow-up for failures or unavailable environments. Link any defect to a regression test when fixed.
