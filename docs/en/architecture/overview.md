# Architecture Overview

## Crates

```
graphite-app ──► graphite-render     (wgpu + cosmic-text + winit Window)
           ├─► graphite-text       (ropey buffer)
           ├─► graphite-workspace  (ignore crate)
           └─► graphite-syntax     (tree-sitter)
```

## Rendering

- **UI stack:** custom UI on `wgpu` (GPU drawing) and `cosmic-text` (shaping, fonts, bidi, fallback), with `winit` for windows and input. Chosen over egui/GPUI/iced for independence, performance control and clean licensing.
- **Pipeline:** one textured-quad pipeline. Rectangles sample a reserved solid 2×2 block of the atlas; glyphs sample an `R8` coverage atlas filled lazily by a shelf packer.
- **Text cache:** shaped lines are cached by `(text, font size)`; glyph images are rasterized once per `CacheKey`.
- **Surface format:** non-sRGB when available, so colors appear as authored.

## Text and highlighting

- Text lives in a `ropey::Rope`; the cursor is a character offset. Every edit bumps a `revision` counter.
- `graphite-syntax` runs `tree-sitter-highlight` and returns per-line byte-range spans with a semantic `Class`. The app maps classes to colors and re-highlights only when the revision changes.

## Platforms

Windows and Linux; platform differences are delegated to `winit`, `wgpu` and `cosmic-text`.

## Known limitations (v0.1)

- Whole-file re-highlight on each edit (no incremental parsing yet).
- No selection, clipboard, undo/redo, or tabs.
- The glyph cache is never evicted.
- Color glyphs (emoji) render as alpha masks.
- Accessibility (`accesskit`) and IME are not integrated yet.

