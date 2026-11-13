# VS Code parity inventory

The goal is a Graphite that matches VS Code, interfaces and panels included. This inventory compares the commands registered by Code - OSS (`microsoft/vscode`, MIT license) with the commands Graphite has, so the work can be planned area by area.

## How it was measured

- Source: a shallow clone of `microsoft/vscode` from 2026-11-13, scanning `src/vs/editor`, `src/vs/workbench/contrib`, `src/vs/workbench/browser` and `src/vs/platform/actions`.
- A command counts when it is registered with `registerAction2`, `EditorAction`, `registerCommand` or similar with a literal id.
- Graphite's side is the list of `command.*` keys in `locales/en/commands.json`.

Result: **949** commands found in VS Code, **47** already present in Graphite under the same id, **902** missing.

Limits of the count:

- Ids built from constants are not found, so the real VS Code total is higher.
- Graphite may have a command under another id (for example `graphite.editor.addCursorsToLineEnds` does what `editor.action.insertCursorAtEndOfEachLineSelected` does), so some "missing" entries are only renames.
- The same command can differ in behavior even when the id matches.

The full list, grouped by area, is kept in the local checklist (`Itens/comandos-vscode-faltando.md`, not committed).

## Missing commands by area

| Area | Missing | Kind |
| --- | ---: | --- |
| Workbench shell: `parts` and `actions` (layout, sidebars, panel, editor groups, zoom) | 214 | Interface |
| Chat, inline chat, agents, sessions | ~145 | Needs an AI service |
| Notebooks, interactive window, REPL | ~76 | Large subsystem |
| Debug | 46 | Interface + DAP |
| Extensions view | 40 | Interface |
| Source control | 28 | Interface + Git |
| Merge editor | 26 | Large interface |
| Folding | 15 | Editor logic |
| Preferences, tasks | 30 | Interface |
| Lines operations | 13 | Editor logic |
| Local history, timeline | 18 | Interface |
| Files, output, outline | 25 | Interface |
| Multi-cursor, go to symbol, sticky scroll, anchors, caret | ~30 | Editor logic |
| User data sync, edit sessions, remote, update, issue | ~45 | Needs a service |

## Proposed order

1. **Editor core** (about 90 commands): lines operations, multi-cursor, folding, go to symbol, sticky scroll, caret and bracket operations, font zoom. Mostly pure text logic, so it ports faithfully and tests well.
2. **Workbench shell** (about 214): layout parts and actions. This is what makes the window feel like VS Code.
3. **Daily tooling**: source control, files, local history, timeline, outline, output, preferences, tasks, testing, debug, extensions view, snippets, themes.
4. **Merge editor and multi-file diff.**
5. **Notebooks and the interactive window**: a subsystem of its own.
6. **Service-dependent features**: chat, sync, remote, update. These need a backend or an account, so each one needs a decision on what it connects to.

Each step is implemented by reading the VS Code source for the behavior, rewriting it in Rust, adding tests and EN/PT-BR strings, and committing per group of commands.
