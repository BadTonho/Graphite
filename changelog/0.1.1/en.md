## New features

- Review and recover local drafts for unsaved text files and untitled tabs after an unexpected exit.

## Fixes

- Failed `graphite plugin` commands now write one entry to the high-severity error log.
- Render plugin-contributed SVG, bitmap, and codicon icons in the Activity Bar and Secondary Side Bar instead of showing only the first letter of the container title.
- Initialize and sync `vscode.window.tabGroups` (`ExtHostEditorTabs`) in the VSIX extension host so extensions reading `activeTabGroup` on activation no longer fail, raise extension-host queue capacity for concurrent startup activations, and record extension activation/runtime errors in the medium-severity error log.
- Serve extension webviews on a loopback `127.0.0.1` origin with `POST /__graphite_ipc` (avoiding read-only `window.ipc` collisions and `frame-ancestors` blocks for nested local iframes), and accept `MainThreadComments` feature/range updates without surfacing an error.
