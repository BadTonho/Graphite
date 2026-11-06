# Architecture Overview

## Crates

```
shoppe-app ──► shoppe-render     (wgpu + cosmic-text + winit Window)
           ├─► shoppe-text       (ropey buffer)
           ├─► shoppe-workspace  (ignore crate)
           └─► shoppe-syntax     (tree-sitter)
```

## Module responsibilities

- `shoppe-app` separates window events, editor input, document operations, drawing, accessibility, theme and LSP/terminal integration under `app/`. Configuration modules separate settings, themes, snippets, paths and VS Code import; LSP modules separate the manager, worker, message dispatch and protocol conversion.
- `shoppe-text` separates filesystem I/O, edits and history, and cursor navigation. `shoppe-workspace` separates project search from folder listing.
- `shoppe-render` separates text shaping and glyph rasterization from the GPU surface pipeline. `shoppe-syntax` keeps its incremental and full-document highlighters in separate modules.

## Rendering

- **UI stack:** custom UI on `wgpu` (GPU drawing) and `cosmic-text` (shaping, fonts, bidi, fallback), with `winit` for windows and input. Chosen over egui/GPUI/iced for independence, performance control and clean licensing.
- **Pipeline:** one textured-quad pipeline. Rectangles sample a reserved solid 2×2 block of the atlas; glyphs sample an `R8` coverage atlas filled lazily by a shelf packer.
- **Text cache:** shaped lines are cached by `(text, font size)`; glyph images are rasterized once per `CacheKey`.
- **Surface format:** non-sRGB when available, so colors appear as authored.
- **Clipping:** rectangles and glyphs are clipped to editor bounds, with texture coordinates adjusted for partially visible glyphs. The same primitive can constrain future split panes.

## Text and highlighting

- Text lives in a `ropey::Rope`; the cursor is a character offset. Every edit bumps a `revision` counter.
- Transactions record their constituent character ranges for position mapping across edits, undo and redo. This is groundwork for independent views of a shared document; editor groups are not yet implemented.
- `shoppe-syntax` provides incremental tree-sitter parsing over Rope chunks and a full-document highlighter; both return per-line byte-range spans with a semantic `Class`. The app maps classes to colors and re-highlights only when the revision changes.

## Platforms

Windows and Linux; platform differences are delegated to `winit`, `wgpu` and `cosmic-text`.

## Known limitations

- Incremental highlighting can invalidate the remainder of a document after an edit.
- The glyph cache is never evicted.
- Color glyphs (emoji) render as alpha masks.
- Interactive verification of the window and GPU recovery is still pending on Windows and Linux.

