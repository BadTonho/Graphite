## New features

- Connect Chat to OpenAI, Claude, OpenCode Console, OpenCode Go, Gemini, OpenRouter, or an OpenAI-compatible service. Chats keep their original provider and model, API keys use the system credential store, and editor context can be attached to individual messages.
- Review and recover local drafts for unsaved text files and untitled tabs after an unexpected exit.
- Choose which components to install on Windows. VSIX Plugins, Copilot SDK and Foundry Local SDK can be selected independently and are all enabled by default.

## Improvements

- The Chat panel now has a guided connection setup, labeled conversation controls, and a message field with send, stop, and removable context attachments.
- Simplify the secondary sidebar by moving access to Outline to the command palette.

## Fixes

- On Windows, terminals, tasks, notebook kernels, and Chat commands no longer inherit Graphite's extension webview, notebook kernel, and debug adapter connections. Previously a process started while such a connection was open kept it alive after Graphite closed it, so a webview could wait for a response that had already been sent.
- Saving the active profile, workspace trust, and plugin storage can no longer leave a half-written file when the file system rejects the fast replacement or Graphite stops mid-save. The new file is flushed first and swapped in one step, and a failed save keeps the previous file.
- A theme color written with a non-ASCII character, such as `#abcdeé`, no longer crashes Graphite; malformed colors are ignored. Short `#RGB` and `#RGBA` colors are now accepted too.
- The GitHub sign-in token is now kept in Windows Credential Manager or the Linux Secret Service instead of in plain text in the profile folder. Tokens saved by earlier versions are moved there automatically, and failures to keep the sign-in are recorded in the error log.
- The error log now redacts GitHub sign-in tokens (`gho_`) and other GitHub, Google, GitLab, Slack, and AWS tokens, `Basic` and `x-api-key` credentials, `key=value` secrets, and passwords in URLs, which could previously be written to disk. The error log and the issue reporter now share one set of redaction rules.
- Chat tools can no longer reach files outside the workspace through a symbolic link or junction: creating a file under a linked folder is refused like reading one, and workspace search no longer follows links, which also prevents it from looping.
- Chat conversations and tool approvals that cannot be saved are now recorded in the medium-severity error log instead of being lost silently, Chat commands no longer write to an isolated profile, and **Chat: Open Chat Edit in Diff** reports when its comparison file cannot be created.
- Chat now stops a reply with an error when a model service sends a single event larger than 8 MiB, instead of using ever more memory, and no longer slows down on replies whose lines arrive in many small pieces.
- Chat terminal commands no longer crash the reply when long output ends in the middle of an accented character or emoji, and no longer hang it forever: a command that runs longer than 2 minutes, or is stopped with **Stop**, is ended along with the processes it started. Output is capped while it is read, and quoted arguments now reach `cmd` on Windows exactly as written.
- **Always allow** in Chat now applies only to the current workspace, and for terminal commands only to the exact command that was approved. Previously one approval let the assistant run any command or edit any file in every workspace. Restricted workspaces never remember approvals, and approvals saved by earlier versions are asked for again.
- Chat edits no longer overwrite, discard, undo, or redo a file whose editor has unsaved changes, which previously erased those changes and the editor's undo history. The model is now told whether an edit was really written, and failed writes are reported and logged instead of being shown as applied. Native plugins writing workspace files are protected the same way.
- Failed `graphite plugin` commands now write one entry to the high-severity error log.
- Render plugin-contributed SVG, bitmap, and codicon icons in the Activity Bar and Secondary Side Bar instead of showing only the first letter of the container title.
- Initialize and sync `vscode.window.tabGroups` (`ExtHostEditorTabs`) in the VSIX extension host so extensions reading `activeTabGroup` on activation no longer fail, raise extension-host queue capacity for concurrent startup activations, and record extension activation/runtime errors in the medium-severity error log.
- Serve extension webviews on a loopback `127.0.0.1` origin with `POST /__graphite_ipc` (avoiding read-only `window.ipc` collisions and `frame-ancestors` blocks for nested local iframes), and accept `MainThreadComments` feature/range updates without surfacing an error.
- Scroll the file Explorer with the mouse wheel when hovering the primary sidebar and render its scrollbar when entries exceed the viewport.
- Add horizontal padding between the editor line-number gutter and the start of text.
