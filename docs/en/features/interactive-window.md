# Interactive window and REPL

An interactive window is a notebook bound to a kernel with an input box under the cells, for trying code out and keeping what happened. Whatever is run becomes a code cell with its outputs, so the window is a notebook: it uses the same kernel, outputs and cell commands as the [notebook editor](notebook-editor.md).

| Command | What it does |
| --- | --- |
| Open interactive window | Opens a new window in a tab called Interactive. |
| Open REPL | Opens the same thing with the keys of a REPL: Enter runs. |
| Run selection or line in the interactive window | From a text editor: sends the selected text, or the current line when nothing is selected, to the interactive window and runs it. The window is opened the first time and reused after that. The extension of the file (`.py`, `.jl`, `.r`, `.js`) tells which language the code is in, which picks the kernel without asking when only one kernel runs that language. |

## The input box

The box is docked at the bottom, grows with its text up to eight lines, and has the keyboard when the window opens and after every run.

| Key | Window | REPL |
| --- | --- | --- |
| Enter | New line | Runs the input |
| Shift+Enter | Runs the input | New line |
| Ctrl+Enter | Runs the input | Runs the input |
| Up / Down on the first / last line | Previous / next input of the history | Same |
| Escape | Gives the keyboard to the cells above | Same |

Typing, Tab, Backspace, Delete, the arrow keys, Home and End (Shift selects), Ctrl+`A`, Ctrl+`C`, Ctrl+`X`, Ctrl+`V`, Ctrl+`Z` and Ctrl+`Y` work as in the text editor, inside the box. A click in the box puts the caret there.

Running starts the kernel the first time (see **Choosing the kernel** in the notebook editor). If no kernel can be started the input stays in the box and the status bar says why. A blank input runs nothing. Walking the history keeps what you were typing, and coming back past the newest entry restores it. Running the same input twice in a row adds it to the history once.

## The history and the cells

The cells above the box are the history. With the keyboard on them (Escape, or **Focus the interactive history**) all the notebook commands apply: copy, delete, move, change type, collapse, run again. **Focus the last item executed** goes to the newest cell. **Focus interactive input** gives the keyboard back to the box, **Clear interactive input** empties it, and **Scroll to top** and **Scroll to bottom** move the window.

## Saving

The window is not asked about when it is closed: its history is scratch. Saving it (Save, or Save As) writes the history as an `.ipynb` file, and from then on it is a notebook like any other, with the usual modified marker. The next open of that file is an ordinary notebook, without the input box.
