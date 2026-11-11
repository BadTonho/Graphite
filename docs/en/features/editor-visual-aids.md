# Editor visual aids

## Bracket pairs

Graphite pairs `()`, `[]` and `{}` by scanning the text. When the caret is on or just after a bracket, the matching bracket is highlighted. Brackets are colored by nesting depth with three colors that repeat every three levels.

Pairing is textual: brackets inside strings and comments are counted too. Scans stop after 200,000 characters, so a bracket with no match in that range is not highlighted.

## Indent guides

A thin vertical line is drawn at each indentation level of a line, based on its leading whitespace. Tabs use the document's tab size. Blank lines get no guide.

## Join lines

The **Join lines** command (`editor.action.joinLines`) joins the line under the caret with the next one. With a selection, it joins every selected line into one. Leading whitespace on joined lines is removed and the pieces are separated by a single space. The command is undoable in one step and respects read-only documents. It has no default shortcut; bind one in `keybindings.json` if you want one.
