# Embedded icon font

`graphite-icons.ttf` provides the file-type icons drawn by the explorer, quick
open and editor tabs. It is an embedded subset of **Nerd Fonts Symbols Only
v3.5.1** renamed to the family `Graphite Icons`.

- The renderer loads the font once at startup (`graphite-render/src/icons.rs`)
  and draws icons through `Renderer::draw_icon`, which tints the glyph with the
  color supplied by the caller.
- The mapping from file names/extensions to glyph and color lives in
  `crates/graphite-app/src/file_icons.rs`.
- Only the glyphs listed in `AVAILABLE_GLYPHS`
  (`crates/graphite-render/src/icons.rs`) are present in the font. Add the
  codepoint there, then rebuild the font.

## Rebuilding the subset

Download the source font from the Nerd Fonts release page:

```text
https://github.com/ryanoasis/nerd-fonts/releases/latest/download/NerdFontsSymbolsOnly.zip
```

Install `fonttools` and run the generator:

```bash
pip install fonttools
python crates/graphite-render/assets/generate-icons.py \
  --source path/to/SymbolsNerdFont-Regular.ttf
```

The script reads the `'\u{...}'` entries from `AVAILABLE_GLYPHS`, subsets the
source font, renames the family to `Graphite Icons`, and overwrites
`graphite-icons.ttf`. A test in `graphite-render` verifies that every advertised
glyph rasterizes from the embedded font, and a test in `graphite-app` verifies
that every mapped icon exists in `AVAILABLE_GLYPHS`.

## Licensing

See `NOTICE.md` and the `LICENSE-*.txt` files in this directory. The subset is
derived from Nerd Fonts (MIT) and includes glyphs from Codicons (CC BY 4.0),
Devicons (MIT), Font Awesome (CC BY 4.0 / SIL OFL 1.1) and Seti (MIT).
