# Activity bar

Graphite has an activity bar on the far left edge of the window.
It switches the primary workbench views without reducing the Explorer panel:
the Explorer keeps its 260 px width and the bar adds a 48 px column next to it.

## Items

| Item | Icon | Behavior |
| --- | --- | --- |
| **Explorer** | `cod-files` | Shows the file tree and closes Source Control and project search. |
| **Search** | `cod-search` | Opens the project search overlay (`Ctrl+Shift+F`); clicking it again closes the overlay. |
| **Source Control** | `cod-source_control` | Toggles the Git sidebar (`Ctrl+Shift+G`). |
| **Plugins** | `cod-extensions` | Opens the full-screen Plugins panel (`Ctrl+Shift+X`). |

The active item is marked with a 2 px accent bar in `Theme::STATUS` and a
brighter icon. Hovering an inactive item draws a subtle row highlight. The
selected item follows the visible view: Plugins when the panel is open, then
Search when its overlay is open, then Source Control, otherwise Explorer.

## Layout

The geometry lives in `crates/graphite-app/src/app/activity_bar.rs`:

- `ACTIVITY_BAR_WIDTH` is 48 px (defined in `main.rs`).
- `SIDEBAR_WIDTH` is `ACTIVITY_BAR_WIDTH + 260.0`, so the editor, terminal,
  Quick Open and every overlay keep starting after the full left rail.
- Explorer rows and the Source Control sidebar are offset by
  `ACTIVITY_BAR_WIDTH`; the bar itself occupies the remaining column.
- The bar is hidden in Zen Mode and covered by full-screen surfaces such as the
  diff viewer and the Plugins panel.

The background uses `activityBar.background` when an imported theme defines it
and falls back to `#333333` in the dark theme and `#e8e8e8` in the light theme.

## Accessibility

The bar is exposed as an accessibility `Toolbar` with one `Button` per item.
Each button reports its localized label and selected state and supports the
`Click` action, so screen readers can switch views without using the mouse.

## Adding an item

1. Add a variant to `Activity` and to `Activity::ALL` in
   `crates/graphite-app/src/app/activity_bar.rs`.
2. Return its glyph from `Activity::glyph` and its label key from
   `Activity::label_key`.
3. Add the Nerd Fonts codepoint to `AVAILABLE_GLYPHS` in
   `crates/graphite-render/src/icons.rs` and rebuild the subset with
   `python crates/graphite-render/assets/generate-icons.py --source SymbolsNerdFont-Regular.ttf`.
4. Add the label to `locales/en/app.json` and `locales/pt-BR/app.json`.
5. Handle the new variant in `App::activate_activity`.
6. Run `cargo test -p graphite-app --bin graphite activity_bar` and
   `cargo test --workspace`.

## Verification

Automated coverage checks slot hit testing, the active-item priority, toggling
Search and Source Control, the accessibility toolbar, and that every activity
glyph exists in the embedded icon font. Visual verification is done by running
`target/debug/graphite.exe .` and checking Explorer, Source Control, Search and
Plugins in both themes.
