# Color themes and Graphite Simple

**Settings gear → Themes** (and **File → Preferences → Themes**) lists **Dark** and **Light**, followed by every installed color theme, with a check mark on the active one. Picking a theme applies it immediately and stores it as `workbench.colorTheme` in the profile's `settings.json`; picking **Dark** or **Light** clears that setting again. Themes come from imported settings, enabled extensions that contribute themes, native plugins, and the themes bundled with Graphite.

## Graphite Simple

Graphite Simple is a clean, neutral dark theme that ships with the application and is installed into the profile on startup, so it appears in the menu without any setup. It is also published as a standalone color-theme package in [assets/themes/graphite-simple/](../../../assets/themes/graphite-simple/) (`package.json` plus a theme JSON file, no code), which can be installed like any extension that contributes themes.

| Element | Color |
|---|---|
| Editor background | `#1e1e1e` |
| Editor text | `#d4d4d4` |
| Side bar | `#252526` |
| Activity bar | `#333333` |
| Status bar | `#007acc` |
| Keyword / function / type | `#569cd6` / `#dcdcaa` / `#4ec9b0` |
| String / number / comment | `#ce9178` / `#b5cea8` / `#6a9955` |

To select it without the menu, set `"workbench.colorTheme": "Graphite Simple"`. See [Extension API](extension-api.md) for how contributed themes are applied.
