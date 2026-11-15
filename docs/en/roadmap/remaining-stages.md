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
| 6. Services | Chat, sync, remote, update and similar | Done, with the exclusions below |

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

Implemented across dedicated modules and commands, with each backend decision documented below.

| Area | Backend and design decision | Reference |
| --- | --- | --- |
| Chat, inline chat, agents, sessions | `graphite-chat` SSE providers (Anthropic & OpenAI-compatible, `chat-provider.json`), explicit `ToolApprovalPolicy`, inline chat diffs, and local session persistence | [Chat, inline chat, and agent sessions](../features/chat.md) |
| User data sync | Private GitHub Gist (`read:user gist`) with three-way JSON key merge, manual conflict acceptance, and timestamped local backups (`sync-backups/`) | [User data sync](../features/user-data-sync.md) |
| Edit sessions, remote and tunnels | System OpenSSH client (`ssh`) with `~/.ssh/config` host parsing, RFC 6455 WebSocket relay (`ws://`), port cleanup, and `<profile>/edit-sessions.json` snapshots | [Remote workspaces, tunnels, and edit sessions](../features/remote-and-edit-sessions.md) |
| Update | GitHub Releases (`stable` / `prerelease`), strict `SemVer` ordering, and `ring::digest::SHA256` verification before staging | [Application updates and redacted issue reporter](../features/updates-and-issue-reporter.md) |
| Issue reporting, troubleshooting | Automatic secret/path redaction, troubleshooting session log, and pre-filled GitHub Issue URLs | [Application updates and redacted issue reporter](../features/updates-and-issue-reporter.md) |

Intentional exclusions and unverified checks:
- Voice dictation and proprietary vendor billing/account commands remain excluded.
- Interactive desktop window rendering of new chat/quick-chat panels requires a live window and was not checked visually.

## Leftovers from stages 1 to 3

Items that were left out because they depend on something the application does not have yet. Each can be picked up once its prerequisite exists.

**Editor core**
- Diff editor commands (`switchSide`, `collapseAllUnchangedRegions`, `showAllUnchangedRegions`, `compareEditor.nextChange`/`previousChange`, `git.revertSelectedRanges`) are implemented.
- Editor accessibility and token inspection (`editor.action.inspectTMScopes`, `editor.action.toggleScreenReaderAccessibilityMode`, `workbench.action.showAccessibilityHelp`) are implemented.

**Workbench shell**
- Keyboard focus for the activity bar (`workbench.action.focusActivityBar`) and status bar (`workbench.action.focusStatusBar`) is implemented; banner and panel sub-views remain.
- Moving and resizing views (`workbench.action.increaseViewSize`, `decreaseViewSize`, `resetViewLocations`) and notifications position (`notifications.togglePosition`) are implemented.
- Navigable breadcrumbs (`breadcrumbs.focusAndSelect` opens symbol navigation; per-segment stepping remains).
- List commands (`list.selectAll`, `list.clear`, `list.expandSelectionDown`, `list.expandSelectionUp`, `list.toggleFilterOnType`, `list.find`) are implemented for explorer lists; search/SCM list parity remains.

**Daily tools**
- Output channels, compound logs (`workbench.output.action.clearOutput` is implemented).
- Theme generation (`workbench.action.generateColorTheme`) is implemented; live theme preview remains.
- Testing commands that exist for extensions.
- Source control: `scm.focusResourceGroup`, commit message history navigation (`ArrowUp`/`ArrowDown` in the commit input), and `git.viewHistory` graph view are implemented.
- Debug: tests against a real debug adapter (the client is only tested against a simulated one). Breakpoint gutter markers, data breakpoints (`setDataBreakpoints`), Test Explorer gutter decorators, and paused debug variable hover popup are implemented.
- Extensions: `checkForUpdates`, `updateAll`, `showBuiltInExtensions`, `openExtensionsFolder`, and `showRecommendedExtensions` are implemented; workspace-only enable remains.

**Merge and diff editors**
- Merge editor developer commands (copy, save and load the contents as JSON, open a selection in a temporary merge editor): developer tools, not planned.
- The base pane only has what the conflict markers carry; computing a base for two-way markers needs the index stages.
- Typing directly in the result pane (use Open result in the editor instead).
- Diff editor commands (`switchSide`, `collapseAllUnchangedRegions`, `showAllUnchangedRegions`, `compareEditor.nextChange`/`previousChange`) are implemented; revert, moved-code detection and notebook diff still need the two-file diff view to become an editor.

**Notebooks**
- Rich outputs: SVG, HTML drawn as a page (today it is read as text), interactive widgets and app outputs.
- Remote or Jupyter server kernels: only local kernels are started.
- Format the notebook or the cell (`notebook.format`, `notebook.formatCell` are implemented for whitespace/indentation cleanup; external per-language formatters remain).
- Multi-cursor inside a cell and the notebook cursor commands.
- Select/copy cell output (`notebook.cell.copyOutput`) and inline chat entry (`notebook.cell.chat.start`) are implemented; show empty outputs, cell toolbar position, MIME type order and layout diagnostics remain.
- Notebook diff (next, previous, revert input, outputs and metadata): needs the diff view to become an editor.
- Chat commands on cells: `notebook.cell.chat.start` opens chat from a cell.
- Tests against a real `ipykernel`: one test exists and is ignored by default (set `GRAPHITE_TEST_KERNEL_PYTHON`); the rest use a fake kernel that speaks the real protocol over real sockets.
- Drawing of images and cells needs a window and was not checked visually.

## Working rules for every stage

- One commit per step, made only when the whole test suite passes.
- Documentation in English and Portuguese for every user-visible change.
- No mention of the reference editor in code, texts, documentation or commit messages.
- Anything that needs a window to be seen is reported as not checked visually.
- The local checklist is updated with each commit.
