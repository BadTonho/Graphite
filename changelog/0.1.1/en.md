## New features

- Connect Chat to OpenAI, Claude, OpenCode Console, OpenCode Go, Gemini, OpenRouter, or an OpenAI-compatible service. Chats keep their original provider and model, API keys use the system credential store, and editor context can be attached to individual messages.
- Review and recover local drafts for unsaved text files and untitled tabs after an unexpected exit.
- Choose which components to install on Windows. VSIX Plugins, Copilot SDK and Foundry Local SDK can be selected independently and are all enabled by default.

## Improvements

- The Chat panel now has a guided connection setup, labeled conversation controls, and a message field with send, stop, and removable context attachments.
- Simplify the secondary sidebar by moving access to Outline to the command palette.

## Fixes

- Failed `graphite plugin` commands now write one entry to the high-severity error log.
- Render plugin-contributed SVG, bitmap, and codicon icons in the Activity Bar and Secondary Side Bar instead of showing only the first letter of the container title.
- Initialize and sync `vscode.window.tabGroups` (`ExtHostEditorTabs`) in the VSIX extension host so extensions reading `activeTabGroup` on activation no longer fail, raise extension-host queue capacity for concurrent startup activations, and record extension activation/runtime errors in the medium-severity error log.
- Serve extension webviews on a loopback `127.0.0.1` origin with `POST /__graphite_ipc` (avoiding read-only `window.ipc` collisions and `frame-ancestors` blocks for nested local iframes), and accept `MainThreadComments` feature/range updates without surfacing an error.
- Scroll the file Explorer with the mouse wheel when hovering the primary sidebar and render its scrollbar when entries exceed the viewport.
- Add horizontal padding between the editor line-number gutter and the start of text.
