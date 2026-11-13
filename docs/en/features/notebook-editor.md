# Notebook editor

Files ending in `.ipynb` (Jupyter notebooks of format 4) open as a notebook: a vertical list of cells, each with its own editor. A file that is not a valid notebook opens as ordinary text instead.

## What is kept

Everything in the file is kept when it is saved, including fields this program does not understand (in the notebook, its metadata, the cells and the outputs). The file is written the way Jupyter writes it: keys sorted, one-space indent, source split into lines, a final newline. A notebook formatted differently is shown as unchanged when opened and is rewritten in that format only when you save it. From format 4.5 on every cell has a unique id; cells without one get one.

## Cells

Each cell shows a header (`[3]` for a code cell that ran as the third execution, `[ ]` for one that did not run, `markdown` or `raw`), its input and, for code cells, its outputs as text (stream, results and errors, with terminal colors removed). The cell with the focus has a bar on its left: blue in command mode, green while editing. Selected cells have a tinted background.

There are two modes, as in Jupyter.

### Command mode (keys act on cells)

| Key | Command |
| --- | --- |
| Enter | Edit the cell. |
| Up / Down, `K` / `J` | Focus the previous or next cell. Shift extends the selection to several cells. |
| Home / End | First or last cell. |
| `A` / `B` | Insert a code cell above or below and edit it. |
| `D` `D` | Delete the selected cells (press `D` twice). Deleting every cell leaves one empty code cell. |
| `M` / `Y` / `R` | Change the cell to markdown, code or raw. Leaving the code type drops its outputs and execution count. |
| Shift+`M` | Join the selected cells, or the cell with the next one. |
| Ctrl+Shift+Down | Move the selected cells down (and the matching command up). |
| `X` / `C` / `V` | Cut, copy and paste cells. Pasted cells get new ids. |
| `O` | Collapse or expand the outputs. |
| `H` | Collapse or expand the input. |
| `L` | Show or hide line numbers. |
| Ctrl+`A` | Select all cells. |

### Edit mode (keys edit the text of the cell)

Typing, Enter, Tab (inserts spaces), Backspace, Delete, the arrow keys, Home and End (with Shift to select), Ctrl+`A` (select the cell's text), Ctrl+`C`, Ctrl+`X` and Ctrl+`V` work as in the text editor. Up on the first line and Down on the last line move on into the neighbouring cell at the same column. Escape returns to command mode. Shift+Enter or Ctrl+Enter leave the cell and focus the next one. Ctrl+Shift+`-` splits the cell at the caret. A click on the input of a cell starts editing at that place; a click elsewhere on a cell selects it.

## Undo

Ctrl+`Z` and Ctrl+`Y` (or Ctrl+Shift+`Z`) undo and redo in both modes, over one history for text typed in any cell and for cell operations (insert, delete, move, split, join, change of type). Undoing back to the saved state leaves the document clean.

## Saving and reloading

The notebook is a normal document: it shows as modified in the tab, saves with the usual commands and is reloaded when the file changes on disk and has no unsaved edits.

Every command is also available from the command palette under **Notebook**.

## Running cells

A strip above the cells names the kernel of the notebook and what it is doing (not started, starting, idle, busy, stopped). Code cells run on a Jupyter kernel that this program starts as a local process and talks to over the Jupyter protocol; nothing needs to be running beforehand, but a kernel has to be installed (for Python, `ipykernel`).

**Choosing the kernel.** The first time a cell runs, the kernel the notebook asks for in its metadata is used. When it does not name one that is installed, the only kernel for its language is used, or the only kernel there is. When the choice is yours the kernel picker opens instead and nothing runs. **Select notebook kernel** opens the picker at any time. The choice is written to the notebook metadata, as Jupyter does, so the notebook is shown as modified. Kernels are found in the Jupyter kernelspec directories (`JUPYTER_PATH`, the user directory and the system directories) and next to the Python programs on `PATH`; with none registered, the first Python on `PATH` is offered.

| Command | Key | What it does |
| --- | --- | --- |
| Run cell | Ctrl+Enter | Runs the selected cells. |
| Run cell and select below | Shift+Enter | Runs, then moves to the next cell; after the last cell it adds a new one. |
| Run cell and insert below | Alt+Enter | Runs, then adds a cell below and edits it. |
| Run all cells | | Runs every code cell in order. |
| Run cells above / Run cell and all below | | Runs the cells before the selection / the selection and everything after it. |
| Interrupt the kernel | | Stops what is running. |
| Restart the kernel | | Ends the kernel and starts it again; cells that were running stop. |
| Restart the kernel and run all cells | | The same, then runs everything. |
| Shut down the kernel | | Asks the kernel to exit; the next run starts a new one. |
| Clear cell outputs / Clear all outputs | | Removes outputs and execution counts. |

Cells are sent to the kernel in order, and it runs them one after the other. A cell shows `[*]` from the moment it is sent until it finishes, then its execution count. Its old outputs are replaced when its run starts. Streams, results, errors, `clear_output` and updatable displays (`update_display_data`) are applied as the kernel reports them. Outputs and counts are part of the notebook, so they are saved with it. If a cell fails, the kernel skips the cells queued behind it. Empty cells and markdown cells are not sent. Editing a cell that is waiting does not change what was sent.

Interrupting uses the interrupt event of the kernel on Windows and SIGINT elsewhere. On Windows, Python only notices an interrupt once a call such as `time.sleep` returns.

If the kernel ends on its own, the strip says it stopped and the next run starts a new one. Closing the notebook ends its kernel. If the file is reloaded from disk while cells are running, the runs in flight are dropped and the kernel is kept.


## Outputs

Each output is drawn under its cell, in the order the kernel produced it.

| Output | How it is shown |
| --- | --- |
| Text streams (`stdout`, `stderr`) | As text; chunks of one stream are joined, and a carriage return rewrites its line (progress bars). |
| Errors | The traceback, with terminal colors removed. |
| Images (`image/png`, `image/jpeg`) | As an image, scaled to fit a 900 x 480 box without enlarging it. A size in the output's metadata is used when there is one. |
| Results and displays with other data | The first of these that exists: the plain text; the HTML read as text (tags dropped, lists bulleted, tables aligned in columns, scripts and styles removed); Markdown or LaTeX source; JSON, formatted. |
| Anything else (for example SVG alone) | A line naming the kinds of data that were not shown. |

Limits keep a runaway output from freezing the window: at most 5000 lines per cell, 10000 characters per line, an image of at most 25 MB of base64 and 40 million pixels, and images are scaled down to at most 2000 pixels on a side for drawing. Output that goes past a limit is cut and says how much was left out; an image past a limit is replaced by a line saying why. The notebook file itself is not cut: everything the kernel sent is saved.

Images are drawn by the renderer's image atlas. Drawing needs a window, so it was not checked visually; the decoding, scaling, sizing and layout are tested.

Outputs can be collapsed (a line says so) and cleared like any other output. A cell that is selected and run again replaces its outputs when the run starts.

## Sections, search and outline

A **section** is a markdown cell that starts with a heading (`#` to `######` and a space) and the cells after it, up to the next heading of the same or a higher level.

| Command | What it does |
| --- | --- |
| Fold / unfold the section, Fold / expand section | Hides the cells under the heading the focus is in (or shows them again). A folded heading says how many cells it hides. Up and Down skip hidden cells. |
| Fold all sections / Unfold all sections | Does it for every heading. |
| Run the cells of the section / Run only the cell | Runs the code cells of the section of the focused cell / only the focused cell. |
| Toggle sticky scroll | Keeps the heading of the section being read pinned at the top once its own cell has scrolled away. |
| Find in the notebook | A list of the lines of all cells that narrows as you type; picking one goes to it, with the match selected. |
| Find next / previous, Hide the search | Goes to the next or previous match of the last search, across cells and around the notebook, opening folded sections that hide it. |
| Set the kernel profile | The same as Select notebook kernel. |

Folding, sticky scroll and line numbers are view settings: they do not modify the file.

The outline view of a notebook lists the headings of its markdown cells as a tree, with the code cells (and, if asked, the functions and classes they declare) under the heading they follow. Clicking an entry goes to its cell and opens a fold that hid it. **Outline: show code cells**, **show code cell symbols** and **show markdown headers only** change what is listed.
