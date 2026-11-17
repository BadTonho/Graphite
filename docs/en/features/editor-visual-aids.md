# Editor visual aids

## Bracket pairs

Graphite pairs `()`, `[]` and `{}` by scanning the text. When the caret is on or just after a bracket, the matching bracket is highlighted. Brackets are colored by nesting depth with three colors that repeat every three levels.

Pairing is textual: brackets inside strings and comments are counted too. Scans stop after 200,000 characters, so a bracket with no match in that range is not highlighted.

## Indent guides

A thin vertical line is drawn at each indentation level of a line, based on its leading whitespace. Tabs use the document's tab size. Blank lines get no guide.

## Join lines

The **Join lines** command (`editor.action.joinLines`) joins the line under the caret with the next one. With a selection, it joins every selected line into one. Leading whitespace on joined lines is removed and the pieces are separated by a single space. The command is undoable in one step and respects read-only documents. It has no default shortcut; bind one in `keybindings.json` if you want one.

## Telling the file from the background

The text of a file is set apart from the rest of the window:

- The **line-number column** has a slightly different tone from the text area, with a thin line between the two.
- A thin line closes the **tab strip**, so the tabs end where the file begins.
- The **line with the caret** has a soft background. With a selection it is not drawn, because the selection already marks what it covers.

The tones are subtle on purpose, so the text keeps its contrast, and they follow the dark and light themes. In high contrast mode the surfaces stay black and the two lines are drawn in a bright color instead.
