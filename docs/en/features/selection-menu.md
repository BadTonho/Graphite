# Selection menu

The Selection menu groups editor selection and line actions. It provides select all, smart expand and shrink, duplicate or move lines, duplicate selected text, multi-cursor actions, occurrence selection, and column selection.

Ctrl+Click adds a cursor by default. The menu item switches the gesture to Alt+Click and updates its label; the choice is saved in the active profile. Column Selection Mode is a separate toggle, starts off, and is also saved per profile. Dragging while it is active creates aligned selections across visual rows, including wrapped lines.

Duplicate Selection copies the selected text directly after each selection and selects the copies. With no text selected, it duplicates the current line. Cursor-above/below follows visual rows and keeps the column where possible. The menu and command labels are localized in English and Brazilian Portuguese.

## More multi-cursor commands

| Command | Shortcut | What it does |
| --- | --- | --- |
| Add Cursors to Bottom / Top (`editor.action.addCursorsToBottom` / `addCursorsToTop`) | none | Adds a cursor on every line from the primary selection to the last (or first) line, repeating its columns. A line shorter than the column gets the cursor at its end. |
| Change All Occurrences (`editor.action.changeAll`) | `Ctrl+F2` | Selects every occurrence of the selected text, or of the word at the cursor, so one edit changes them all. |
| Move Last Selection to Next / Previous Find Match | `Ctrl+K Ctrl+D` for next | Replaces the last selection (the first one, when going back) with the next (previous) occurrence of its text; the other selections stay. It wraps around the document. |
| Focus Next / Previous Cursor | none | Makes the next or previous selection the primary one, so the view scrolls to it. Cursors do not move. The order is kept until the next edit. |

The commands Add Cursor Above, Add Cursor Below and Add Cursors to Line Ends use the ids `editor.action.insertCursorAbove`, `editor.action.insertCursorBelow` and `editor.action.insertCursorAtEndOfEachLineSelected`. Custom shortcuts that used the old `graphite.editor.*` ids need updating.
