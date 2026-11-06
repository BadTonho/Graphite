# Graphite

A native **Rust** code editor aiming to recreate the VS Code experience — with a fraction of the RAM — and, long term, to run **VS Code extensions** (`.vsix`).

> Status: **v0.3 implementation in place** — workspace tests and the Windows build pass; full Windows/Linux UI verification has not been completed.

## Why

VS Code runs on Electron (a full Chromium + Node.js per window). Graphite renders directly on the GPU with `wgpu` and `cosmic-text`, keeping the editor fast and light.

## Quick start

```bash
cargo run -p graphite-app -- <folder>   # omit <folder> to open the current directory
cargo test --workspace
```

Requires a recent stable Rust toolchain. Supported platforms: Windows and Linux.

## Controls (v0.3)

| Action | Input |
|---|---|
| Open file / expand folder | Click in the sidebar |
| Place cursor | Click in the editor |
| Save | `Ctrl+S` |
| Select all / extend selection | `Ctrl+A` / `Shift` + navigation keys |
| Copy / cut / paste | `Ctrl+C` / `Ctrl+X` / `Ctrl+V` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Search project | `Ctrl+F`, type, `Up` / `Down`, `Enter` |
| Switch / close tab | `Ctrl+Tab` / `Ctrl+W` |
| Toggle light/dark theme | `Ctrl+Alt+T` |
| Import VS Code user settings, snippets and themes | `Ctrl+Shift+I` |
| Find and insert an imported snippet | `Ctrl+Shift+P`, type its prefix, `Enter` |
| Completion / go to definition | `Ctrl+Space` / `F12` |
| Hover / next or previous diagnostic | `Ctrl+K`, `Ctrl+I` / `F8` / `Shift+F8` |
| Show diagnostics / toggle terminal | `Ctrl+Shift+M` / `Ctrl+grave` |
| Move / edit | Arrow keys, Home, End, Backspace, Delete, Enter, Tab |

LSP servers are user-installed executables configured in `lsp.json` in the app configuration directory. The terminal uses the system shell by default and can be changed in `terminal.json` or with `GRAPHITE_SHELL`.

## Workspace layout

| Crate | Responsibility |
|---|---|
| `graphite-text` | Rope-based text buffer, cursor, file I/O |
| `graphite-workspace` | Folder model, `.gitignore`-aware directory listing |
| `graphite-syntax` | tree-sitter syntax highlighting |
| `graphite-render` | `wgpu` renderer + `cosmic-text` glyph atlas |
| `graphite-app` | `winit` window, file tree, editor UI |

The app also includes a generic Language Server Protocol client and an ANSI terminal backed by a cross-platform PTY.

## Documentation

- English: [docs/en](docs/en/README.md)
- Português (pt-BR): [docs/pt-BR](docs/pt-BR/README.md)

## License

MIT

