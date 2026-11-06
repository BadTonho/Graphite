# Roadmap

- [x] **v0.1 — Core:** `winit` window, `wgpu` renderer, `cosmic-text`, folder tree, edit/save, tree-sitter highlighting
- [ ] **v0.2 — Editing and customization:** complete selection, clipboard, undo/redo, incremental highlighting, accessibility and IME; add tabs, project search (ripgrep), themes, and import of the supported subset of VS Code themes, snippets and `settings.json`.
- [ ] **v0.3:** native LSP + integrated terminal
- [ ] **v0.4 — Extension architecture proof:** Node extension host + RPC bridge with a minimal API subset (`commands`, `workspace`, `window`, `languages`)
- [ ] **v0.5:** full git (SCM API), tree views, decorations
- [ ] **v0.6:** webviews (`wry`), debugging (DAP)
- [ ] **v1.0+:** broaden the API guided by the compatibility suite; GitHub (`octocrab`); WASM plugins where compatibility falls short

v0.2 acceptance: per-tab document/cursor/selection/scroll/dirty state; Unicode-safe editing and history; project search honoring workspace exclusions; source-preserving import with clear errors for invalid or unsupported items; incremental highlighting; accessibility and IME integration. Verify key flows on Windows and Linux and cover buffer, search and import with unit tests. Import applies only supported settings and does not promise full VS Code compatibility.

