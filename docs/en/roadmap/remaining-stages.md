# Remaining stages

Plan for the feature parity work that is still to do. Stages 1 to 3 are done as far as the current architecture allows (see [Feature parity inventory](feature-parity.md)); this document lists what is left, in the order proposed. The local checklist `Itens/comandos-faltando.md` (not committed) tracks every command id.

## Where things stand

| Stage | Scope | State |
| --- | --- | --- |
| 1. Editor core | Lines, case, folding, multi-cursor, selection anchor, brackets, text movement, value stepping | Done, with the leftovers below |
| 2. Workbench shell | Editor groups, window parts, quick open, list navigation, activity bar, breadcrumbs | Done, with the leftovers below |
| 3. Daily tools | Explorer, source control, local history, timeline, outline, preferences, tasks, debug, extensions, snippets, output | Done, with the leftovers below |
| 4. Merge and diff editors | Three-way merge editor, multi-file diff | Done, with the leftovers below |
| 5. Notebooks | Notebook editor, interactive window, REPL | Done, with the leftovers below |
| 6. Services | Chat, sync, remote, update and similar | To do, needs decisions |

## Stage 4: merge editor and multi-file diff

About 32 commands, but the work is a new interface, not a set of shortcuts.

1. **Conflict model.** Parse conflict markers (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`) into blocks with the current, incoming and base text. Today the app only has a simple conflict view; the model must support several conflicts per file and keep working while the file is edited.
2. **Three-pane layout.** Incoming and current side by side on top, the result below, each with line numbers, highlighted differences and synchronized scrolling. Reuses the diff drawing that already exists.
3. **Block actions.** Accept current, accept incoming, accept both (in either order), and ignore, per block. Move to the next or previous conflict. Revert a block to its unresolved state.
4. **Resolution flow.** A counter of unresolved conflicts, "complete merge" that writes the result and stages the file, and "accept all" for current or incoming.
5. **Entry points.** Open from the source control list for a file in conflict, from the command palette, and from the existing conflict view.
6. **Multi-file diff.** One scrollable view with the diffs of several files (for example everything staged or everything changed), collapsible per file, with next and previous change across files.
7. **Commands.** Merge editor ids (accept, navigate, toggle layout, show base, reset) and multi-file diff ids, with titles in both languages.
8. **Tests and docs.** Conflict parsing with unusual markers and CRLF, every block action, the result text for each combination, and EN and PT-BR documentation.

Decisions: the base is an optional fourth pane (at the top or in the center) fed by the `|||||||` section of the markers; the result pane is read-only, and **Open result in the editor** hands the text so far to a normal editor to finish by hand. See [Merge editor](../features/merge-editor.md) and [Multi-file diff](../features/multi-file-diff.md).

## Stage 5: notebooks

About 76 commands. A subsystem of its own.

1. **Document model.** Read and write the `.ipynb` JSON format without losing fields it does not understand: cells (code, markdown), source, outputs, execution counts, metadata.
2. **Notebook editor.** A vertical list of cells with an editor in each, cell selection and focus, add, delete, move, split, merge, change type, collapse input and output, and undo across cells.
3. **Execution.** A kernel connection (Jupyter protocol over ZeroMQ or a local process), kernel selection, start, restart, interrupt, run cell, run all, run above and below.
4. **Outputs.** Text and error streams first, then images, HTML and rich display data, with limits on size.
5. **Interactive window and REPL.** A scratch notebook bound to a kernel, with an input box at the bottom.
6. **Commands, tests and docs.** The notebook command ids, EN and PT-BR documentation, tests with a fake kernel.

Decisions: the protocol is Jupyter's, over a built-in ZeroMQ transport, talking to a kernel that the program starts as a local process (no external ZeroMQ dependency). Rich outputs have size limits: images go through a second RGBA atlas of the renderer, and HTML, Markdown, LaTeX and JSON are shown as text. See [Notebook editor](../features/notebook-editor.md) and [Interactive window](../features/interactive-window.md).

## Stage 6: service-dependent features

About 230 commands. None of them can be built without choosing a backend, so each needs a decision first.

| Area | Commands | What has to be decided |
| --- | ---: | --- |
| Chat, inline chat, agents, sessions, voice | ~170 | Which model service, how it authenticates, what the chat can read and change |
| User data sync | ~13 | Where settings are stored and how conflicts are resolved |
| Edit sessions, remote and tunnels | ~14 | Transport and authentication |
| Update | ~14 | Release channel and installer behavior |
| Issue reporting, sharing | ~6 | Destination service |

## Leftovers from stages 1 to 3

Items that were left out because they depend on something the application does not have yet. Each can be picked up once its prerequisite exists.

**Editor core**
- Reindent lines: needs per-language indent rules.
- Overtype mode: typed text and pasted text share one path; the mode needs to apply only to typing.
- Sticky scroll focus actions (focus, select next and previous line, go to line).
- Peek declaration, peek type definition, peek implementation.
- Diff editor commands: need the diff editor interface.
- Editor accessibility commands and inspection commands.

**Workbench shell**
- Keyboard focus for the activity bar, the status bar, the banner and the panel's own views.
- Moving and resizing views, and the notifications position.
- Navigable breadcrumbs (focus, step through, pick items).
- Even editor widths and maximizing a group: groups have no proportions yet.
- Opening a file, folder or workspace in a new window.
- List commands for search results, the source control list and other lists; type to find; multi-selection.
- Cut and paste in the explorer.

**Daily tools**
- Output channels, compound logs.
- Theme generation and preview commands.
- Search editor.
- Testing commands that exist for extensions.
- Source control: input and resource group focus, the Git graph, next and previous commit.
- Debug: data breakpoints, a hover popup over the code, a breakpoint marker in the gutter, checked against a real debug adapter (the client is only tested against a simulated one).
- Extensions: update all, check for updates, workspace-only enable, recommendations, built-in list, open the extensions folder.

**Merge and diff editors**
- Merge editor developer commands (copy, save and load the contents as JSON, open a selection in a temporary merge editor): developer tools, not planned.
- The base pane only has what the conflict markers carry; computing a base for two-way markers needs the index stages.
- Typing directly in the result pane (use Open result in the editor instead).
- Diff editor commands (switch sides, revert, collapse unchanged regions, moved code) and the notebook diff: they need the two-file diff view to become an editor.

**Notebooks**
- Rich outputs: SVG, HTML drawn as a page (today it is read as text), interactive widgets and app outputs.
- Remote or Jupyter server kernels: only local kernels are started.
- Format the notebook or the cell: needs the per-language formatting wired to cells.
- Multi-cursor inside a cell and the notebook cursor commands.
- Select all of a cell's output, show empty outputs, cell toolbar position, MIME type order and the layout and layout troubleshooting commands.
- Notebook diff (next, previous, revert input, outputs and metadata): needs the diff view to become an editor.
- Chat commands on cells: they belong to Stage 6.
- Tests against a real `ipykernel`: one test exists and is ignored by default (set `GRAPHITE_TEST_KERNEL_PYTHON`); the rest use a fake kernel that speaks the real protocol over real sockets.
- Drawing of images and cells needs a window and was not checked visually.

## Working rules for every stage

- One commit per step, made only when the whole test suite passes.
- Documentation in English and Portuguese for every user-visible change.
- No mention of the reference editor in code, texts, documentation or commit messages.
- Anything that needs a window to be seen is reported as not checked visually.
- The local checklist is updated with each commit.
