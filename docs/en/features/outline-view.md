# Outline view

The outline lists the declarations of the active file in the right side bar (toggle it with **Toggle Outline**). It is now a tree: a declaration indented deeper than the one above it is nested under it. Every row shows the line number and the declaration header. Click a row to go to it; click the arrow at the left of a row that has children to collapse or expand it.

| Command | What it does |
| --- | --- |
| Sort Outline by Position / Name / Kind | Position keeps the file order and the nesting. Name sorts all declarations alphabetically, ignoring case. Kind groups them by `fn`, `struct`, `class` and so on, keeping the file order inside each group. Sorting by name or kind flattens the tree. |
| Follow Cursor in Outline | On by default: the row of the declaration the cursor is in is highlighted. Turning it off removes the highlight. |
| Filter Outline on Type | Turns on a filter line at the top of the outline. After clicking the outline, typing narrows the rows to the declarations that contain the text (ignoring case), Backspace deletes a character and Escape clears the text. Turning it off clears the filter. |
| Collapse All Outline Items / Expand All Outline Items | Collapses or expands every declaration that has nested ones. |

The kind and name come from the first words of the declaration header, skipping modifiers such as `pub`, `export` and `async`. The outline works for the languages with a syntax parser.
