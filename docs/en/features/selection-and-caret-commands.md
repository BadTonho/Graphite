# Selection and caret commands

Commands that act on the selection, the cursor and brackets. All text-changing ones are undone in one step and need a writable editor.

## Selection anchor

| Command | Shortcut | What it does |
| --- | --- | --- |
| Set Selection Anchor | `Ctrl+K Ctrl+B` | Remembers the cursor position and shows `Anchor set at line:column` in the status bar. The anchor follows the text as it is edited. |
| Select from Anchor to Cursor | `Ctrl+K Ctrl+K` | Selects from the anchor to the cursor and drops the anchor. |
| Go to Selection Anchor | none | Moves the cursor to the anchor. |
| Cancel Selection Anchor | none | Forgets the anchor. |

The go-to and select commands report `No selection anchor is set` when there is none. The anchor is not drawn in the editor.

## Brackets

Brackets are `()`, `[]` and `{}`, paired by text, so brackets inside strings and comments count.

| Command | Shortcut | What it does |
| --- | --- | --- |
| Go to Bracket (`editor.action.jumpToBracket`) | `Ctrl+Shift+]` | On a bracket, jumps to its pair. Elsewhere jumps to the closing bracket around the cursor, or to the next bracket. |
| Select to Bracket | none | Selects the pair around the cursor, brackets included (or the next pair if there is none around). On the closing bracket the selection runs backwards. |
| Remove Brackets | none | Deletes the pair at or around the cursor. |

## Moving and swapping text

| Command | Shortcut | What it does |
| --- | --- | --- |
| Move Selected Text Left / Right | none | Moves a single-line selection one character along its line; the character it passes moves to the other side. |
| Transpose Letters | none | Swaps the letters before and after the cursor, or the last two at the end of a line. Never swaps across a line break. |
| Replace with Next Value / Previous Value | `Ctrl+Shift+.` / `Ctrl+Shift+,` | Replaces the selection, or the word or number at the cursor, with the next or previous value. A number moves by 1 (`9` becomes `10`, and it does not go below 0); `true`/`false`, `True`/`False` and `public`/`protected`/`private` cycle. The new text stays selected. |

## Editor font size

**Increase / Decrease / Reset Editor Font Size** (`editor.action.fontZoomIn`, `fontZoomOut`, `fontZoomReset`) use the same zoom as the View menu entries: steps of 10%, up to 200%.

## Other editor commands

- **Announce Cursor Position** shows `Line X, column Y` (and how many characters are selected) in the status bar.
- **Toggle Sticky Scroll** flips the same setting as the View menu entry.
- **Focus Sticky Scroll** (`editor.action.focusStickyScroll`) puts keyboard focus on the innermost header pinned above the text; the focused row is highlighted. **Select the Next / Previous Sticky Scroll Line** (`editor.action.selectNextStickyScrollLine`, `editor.action.selectPreviousStickyScrollLine`) move that focus up and down the pinned headers and stop at the ends. **Go to the Focused Sticky Scroll Line** (`editor.action.goToFocusedStickyScrollLine`) moves the caret to the start of the focused header and returns focus to the text, and **Focus the Editor Text** (`editor.action.selectEditor`) returns focus to the text without moving the caret. The focus is dropped when the view scrolls. With sticky scroll off, or with nothing pinned, Focus Sticky Scroll only says so in the status bar. None of these has a default shortcut.
- **Find All References** (`editor.action.findReferences`, `editor.action.referenceSearch.trigger`) runs the same search as Go to References.
