# Roadmap

- [x] **v0.1 — Core:** `winit` window, `wgpu` renderer, `cosmic-text`, folder tree, edit/save, tree-sitter highlighting
- [ ] **v0.2 — Editing and customization:** complete selection, clipboard, undo/redo, incremental highlighting, accessibility and IME; add tabs, project search (ripgrep), themes, and import of the supported subset of VS Code themes, snippets and `settings.json`.
- [ ] **v0.3 — Language servers and terminal:** generic stdio LSP client for configured, user-installed servers; per-document sync and diagnostics; completion, definition, hover and diagnostic navigation; cross-platform PTY terminal with ANSI parsing and workspace-root shell.
- [ ] **v0.4 — Extension architecture proof:** Node extension host + RPC bridge with a minimal API subset (`commands`, `workspace`, `window`, `languages`)
- [ ] **v0.5:** full git (SCM API), tree views, decorations
- [ ] **v0.6:** webviews (`wry`), debugging (DAP)
- [ ] **v1.0+:** broaden the API guided by the compatibility suite; GitHub (`octocrab`); WASM plugins where compatibility falls short

v0.1 verification: the Rope buffer loads and saves UTF-8 in chunks, saves through a temporary sibling file, and preserves its path and dirty state on failure. Incremental tree-sitter parsing reads Rope chunks directly. Tests cover large Unicode files, I/O failures, nested ignore rules and incremental highlighting. The Windows workspace build and automated tests pass; interactive window, GPU recovery and Linux verification remain outstanding.

v0.2 progress: multiple cursors, document find and replace, line actions, syntax selection, fuzzy file opening, ripgrep-based project search with unsaved-document overlays, split groups sharing documents, visual wrapping, configurable keybindings, indentation, rulers, read-only patterns and VS Code settings/theme/snippet import have implementations and automated coverage. Windows `cargo test --workspace` passes (139 tests) and `cargo build --workspace` passes. Clipboard behavior now has isolated Unicode, multi-cursor, undo, read-only and backend-failure tests. The implementation review found no other missing phase 2 feature. Interactive window, clipboard, IME, screen-reader and rendering checks remain outstanding because the native Computer Use pipe was unavailable in this session. Linux was not tested in this run. The implementation is complete; keep the phase open until interactive verification is recorded.

v0.2 acceptance: per-tab document/cursor/selection/scroll/dirty state; Unicode-safe editing and history; project search honoring workspace exclusions; source-preserving import with clear errors for invalid or unsupported items; incremental highlighting; accessibility and IME integration. Verify key flows on Windows and Linux and cover buffer, search and import with unit tests. Import applies only supported settings and does not promise full VS Code compatibility.

v0.3 acceptance: configured server processes stay off the UI thread and shut down with the app; document synchronization and diagnostics are version-aware and use UTF-16 positions; completion, definition, hover and diagnostic navigation work per tab; the terminal shell uses a cross-platform PTY, ANSI terminal parsing, keyboard input, resizing and scrollback. Verify builds and basic interaction on Windows and Linux.

