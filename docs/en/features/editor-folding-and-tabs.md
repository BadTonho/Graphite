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

### More fold commands

Regions are nested: a region inside another is one level deeper. Besides indentation, a region can come from `#region` / `#endregion` markers after a comment leader (`// #region`, `# region`, `<!-- #region`), which also work where indentation does not change.

| Command | Shortcut | What it does |
| --- | --- | --- |
| Toggle Fold (`editor.toggleFold`) | none | Folds the region at the cursor, or unfolds it if it is folded. |
| Fold Recursively / Unfold Recursively | `Ctrl+K Ctrl+[` / `Ctrl+K Ctrl+]` | Folds or unfolds the region at the cursor and every region inside it. |
| Toggle Fold Recursively | `Ctrl+K Ctrl+Shift+L` | Flips the region at the cursor and sets everything inside it to the new state. |
| Fold Level 1 to 7 | `Ctrl+K Ctrl+1` to `7` | Folds every region of that depth, except the ones that contain a cursor. |
| Fold / Unfold All Regions Except Selected | `Ctrl+K Ctrl+-` / `Ctrl+K Ctrl+=` | Applies to every region except the one at the cursor and the regions around or inside it. |
| Fold All Block Comments | `Ctrl+K Ctrl+/` | Folds every region that starts with the language's block comment token. |
| Fold All Regions / Unfold All Regions | `Ctrl+K Ctrl+8` / `Ctrl+K Ctrl+9` | Folds or unfolds every region that starts on a `#region` marker line. |
| Go to Next / Previous Folding Range | none | On a region's first line, moves to the next or previous region at the same level inside the same parent. Elsewhere, moves to the nearest region start below or above. |
| Go to Parent Fold | none | Moves to the first line of the region around the cursor, or of the parent when already on a first line. |
| Create Folding Range from Selection | `Ctrl+K Ctrl+,` | Makes a region of the selected lines and folds it. |
| Remove Manual Folding Ranges | `Ctrl+K Ctrl+.` | Removes the manual ranges at the cursor or touched by the selection; a cursor in no manual range removes all of them. |
| Toggle Import Fold | none | Folds or unfolds the first block of two or more `import` / `use` / `#include` lines. |

Ranges made by hand and the import fold last until the document changes, like the folds themselves. `Ctrl+K Ctrl+L` stays on Unfold.

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
