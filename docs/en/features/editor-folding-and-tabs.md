# Code folding and preview/pinned tabs

## Code folding

Graphite folds code by indentation. A line opens a fold when the next non-blank line is indented deeper than it. The fold covers every following line that is blank or indented deeper, and ends at the last non-blank line of that block. Folding is language-independent and works on any file, but it can differ from a syntax-aware fold on unusual layouts.

| Command | Default shortcut | What it does |
|---|---|---|
| Fold (`editor.fold`) | `Ctrl+Shift+[` | Folds the innermost block around the cursor. If that block is already folded, folds the block around it. The cursor moves to the fold's first line. |
| Unfold (`editor.unfold`) | `Ctrl+K Ctrl+L` | Unfolds the fold that starts on the cursor line, or the fold that hides it. |
| Fold All (`editor.foldAll`) | `Ctrl+K Ctrl+0` | Folds every top-level block. |
| Unfold All (`editor.unfoldAll`) | `Ctrl+K Ctrl+J` | Removes every fold in the view. |

`Ctrl+Shift+]` is already bound to "go to bracket", so unfold uses `Ctrl+K Ctrl+L`. Shortcuts can be changed in `keybindings.json`.

A folded line shows ` …` after its text. Folds are stored per view, so the same document can be folded differently in two groups.

Each tab has a close button (×) at its right end. Click it to close that tab, even when it is not the active one. A tab with unsaved changes still shows the confirmation before closing.

Limitations in this version:

- Folds are cleared when the document changes. They are not kept across edits.
- Folding is unavailable while word wrap is on. The fold commands show a status message in that case, and existing folds are not drawn in wrapped mode.
- There is no clickable chevron in the gutter yet; folding is keyboard-driven.
- If the cursor is moved into a folded block by navigation or search, the block is unfolded so the cursor stays visible.

## Preview and pinned tabs

**Preview tabs.** Opening a file from the explorer (single click) or from Quick Open (`Ctrl+P`) shows it in a preview tab. The preview tab is shown with dimmed text. Opening another file from those places replaces the preview tab instead of adding a new one. Editing the document makes the tab permanent.

**Pinned tabs.** The command *Pin or Unpin Tab* (`editor.action.togglePinTab`) pins the active tab. A pinned tab is marked with a blue line along its top edge, stays open when a preview opens, and is moved before unpinned tabs in its group. Pinning a preview tab makes it permanent. The command has no default shortcut; bind one in `keybindings.json` if you want one.

Limitations in this version:

- Files opened from the command line, from search results, from the Problems list, or from commands still open as permanent tabs.
- Preview text is dimmed rather than italic, because the bundled font has no italic style.
- Double-clicking a tab does not pin it yet.
