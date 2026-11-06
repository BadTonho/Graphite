# VsCodeShoppe

A native **Rust** code editor aiming to recreate the VS Code experience — with a fraction of the RAM — and, long term, to run **VS Code extensions** (`.vsix`).

> Status: **v0.2 implementation complete** — automated workspace tests and Windows compilation pass; manual Windows/Linux interaction checks remain unverified.

## Why

VS Code runs on Electron (a full Chromium + Node.js per window). VsCodeShoppe renders directly on the GPU with `wgpu` and `cosmic-text`, keeping the editor fast and light.

## Quick start

```bash
cargo run -p shoppe-app -- <folder>   # omit <folder> to open the current directory
cargo test --workspace
```

Requires a recent stable Rust toolchain. Supported platforms: Windows and Linux.

## Controls (v0.2)

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
| Move / edit | Arrow keys, Home, End, Backspace, Delete, Enter, Tab |

## Workspace layout

| Crate | Responsibility |
|---|---|
| `shoppe-text` | Rope-based text buffer, cursor, file I/O |
| `shoppe-workspace` | Folder model, `.gitignore`-aware directory listing |
| `shoppe-syntax` | tree-sitter syntax highlighting |
| `shoppe-render` | `wgpu` renderer + `cosmic-text` glyph atlas |
| `shoppe-app` | `winit` window, file tree, editor UI |

## Documentation

- English: [docs/en](docs/en/README.md)
- Português (pt-BR): [docs/pt-BR](docs/pt-BR/README.md)

## License

MIT

