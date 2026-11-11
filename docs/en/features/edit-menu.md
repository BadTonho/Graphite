# Edit menu

The title bar's **Edit** menu exposes undo/redo, clipboard actions, editor find/replace, workspace search and replacement, comment toggles, and Emmet expansion. Keyboard navigation uses the same menu rows and enabled states as pointer and accessibility input.

## Workspace replacement

**Find in Files** searches every workspace root. **Replace in Files** adds a replacement field; press **Tab** to switch between fields and **Ctrl+Enter** to apply **Replace All**. The result list acts as a preview. A stale result aborts the operation. Open documents receive unsaved editor changes, while closed files are updated through the workspace edit transaction.

## Comments and Emmet

Line and block comment actions use the active language's comment markers. Built-in markers are provided for Rust, Python, JavaScript, and JSONC. Enabled VSIX extensions can contribute markers through `contributes.languages[].configuration` using the standard `comments.lineComment` and `comments.blockComment` fields. Actions without a configured marker are disabled. Python has no built-in block comment marker.

The Emmet menu action expands an abbreviation at the caret when the active file and current text support it. **Tab** retains its normal editor behavior and expands Emmet when possible.
