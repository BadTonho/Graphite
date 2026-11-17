# Graphite

A native **Rust** code editor built for speed and low memory usage, with long-term support for **existing `.vsix` extensions**.

> Status: **v0.1.0**, the first release, for Windows and Linux (x86-64). Read the [known limitations](docs/en/roadmap/known-limitations.md) before relying on it for important work.

## Why

Electron-based editors run a full Chromium + Node.js per window. Graphite renders directly on the GPU with `wgpu` and `cosmic-text`, keeping the editor fast and light.

## What it does

- **Editor:** multiple cursors, folding, bracket and indentation tools, side-by-side and inline diff, a three-way merge editor, notebooks (`.ipynb`) with local kernels, and a built-in terminal.
- **Language support:** syntax highlighting, TextMate grammars from extensions and a generic Language Server Protocol client.
- **Source control:** Git status, staging, commits, a history graph, branches, stashes and GitHub sign-in, with a progress indicator and a change count.
- **Extensions:** native Graphite plugins and VS Code `.vsix` extensions from Open VSX, run in a separate host process.
- **Chat and agents:** chat panels backed by the model provider you configure.
- **Settings sync:** keep settings, keybindings, snippets and profiles in sync through a private GitHub Gist or a private repository.
- **Interface:** complete English and Portuguese (Brazil), light, dark and high contrast themes, and keyboard-first navigation.

## Install

Download a package from the [Releases](https://github.com/BadTonho/Graphite/releases) page and check it against its `.sha256` file.

| System | Packages |
|---|---|
| Windows (x86-64) | Setup `.exe` (per user, no administrator rights) or portable `.zip` |
| Linux (x86-64) | `.deb`, `.rpm`, Arch `.pkg.tar.zst`, `.AppImage` or portable `.tar.gz` |

The Windows installer is not code-signed yet, so Windows SmartScreen may warn when you run it. Choose **More info** and **Run anyway** if you trust the download. Windows needs up-to-date graphics drivers, and WebView2 for extension webviews.

## Quick start

Use the **Plugins** sidebar (`Ctrl+Shift+X`) to install native Graphite plugins or extensions from Open VSX. The portable x86-64 distribution includes the Code OSS host and Node for supported VSIX extensions. Enable the installed version, trust the workspace, and run its command from the palette (`Ctrl+Shift+P` or `F1`). See [Installing plugins](docs/en/contributing/plugins.md) and [building the portable package](docs/en/contributing/building.md).

```bash
cargo run -p graphite-app -- <folder>   # omit <folder> to open the current directory
cargo test --workspace
```

For a complete local automated check, run `cargo run -p graphite-xtask -- verify`. It checks formatting, Clippy, compilation and tests, then summarizes failures. See the [verification matrix](docs/en/contributing/verification-matrix.md) for coverage and manual checks.

Requires a recent stable Rust toolchain. Supported platforms: Windows and Linux.

The Explorer, Quick Open list and editor tabs show colored file-type icons for programming languages, project files and common formats. See [File icons](docs/en/features/file-icons.md) for the mapping and how to add new icons.

The activity bar on the far left switches between Explorer, Search, Source Control and Plugins while keeping the Explorer at its original width. See [Activity bar](docs/en/features/activity-bar.md) for behavior and layout details.

## Controls

| Action | Input |
|---|---|
| Open file / expand folder | Click in the sidebar |
| Switch sidebar view | Click Explorer, Search, Source Control or Plugins in the activity bar |
| Place cursor | Click in the editor |
| Save | `Ctrl+S` |
| Select all / extend selection | `Ctrl+A` / `Shift` + navigation keys |
| Multiple cursors / occurrences | `Alt+Click` / `Ctrl+D` / `Ctrl+Shift+L` |
| Copy / cut / paste | `Ctrl+C` / `Ctrl+X` / `Ctrl+V` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Find / replace in document | `Ctrl+F` / `Ctrl+H`; Enter finds, Ctrl+Enter replaces all |
| Search project | `Ctrl+Shift+F`, type, `Up` / `Down`, `Enter` |
| Quick open | `Ctrl+P`, type a fuzzy file name, `Up` / `Down`, `Enter` |
| Move / duplicate / delete line | `Alt+Up/Down` / `Shift+Alt+Up/Down` / `Ctrl+Shift+K` |
| Expand / shrink syntax selection | `Shift+Alt+Right/Left` |
| Convert indentation | `Ctrl+Shift+Alt+I` |
| Switch / close tab | `Ctrl+Tab` / `Ctrl+W` |
| Toggle light/dark theme | `Ctrl+Alt+T` |
| Import user settings, snippets and themes | `Ctrl+Shift+I` |
| Find and insert an imported snippet | `Ctrl+Shift+P`, type its prefix, `Enter` |
| Completion / go to definition | `Ctrl+Space` / `F12` |
| Hover / next or previous diagnostic | `Ctrl+K`, `Ctrl+I` / `F8` / `Shift+F8` |
| Show diagnostics / toggle terminal | `Ctrl+Shift+M` / `Ctrl+grave` |
| Move / edit | Arrow keys, Home, End, Backspace, Delete, Enter, Tab |

LSP servers are user-installed executables configured in `lsp.json` in the app configuration directory. The terminal uses the system shell by default and can be changed in `terminal.json` or with `GRAPHITE_SHELL`.

Graphite stores configuration in `%APPDATA%\Graphite` on Windows and `$XDG_CONFIG_HOME/Graphite` (or `~/.config/Graphite`) on Linux. `GRAPHITE_SHELL` selects the terminal shell. Existing settings kept elsewhere must be copied to the Graphite configuration directory to be used.

## Localization

Graphite detects the operating-system language and ships complete English and Portuguese (Brazil) catalogs. Override the choice with `"workbench.language": "pt-BR"` in `%APPDATA%\Graphite\settings.json` or with `GRAPHITE_LANG=pt-BR`. Translation sources live in [`locales/`](locales/README.md); see [Localization](docs/en/features/localization.md) for the full reference.

## Workspace layout

| Crate | Responsibility |
|---|---|
| `graphite-text` | Rope-based text buffer, cursor, file I/O |
| `graphite-workspace` | Folder model, `.gitignore`-aware directory listing |
| `graphite-syntax` | tree-sitter syntax highlighting |
| `graphite-render` | `wgpu` renderer + `cosmic-text` glyph atlas |
| `graphite-core-api` | UI-independent command, document and service contracts |
| `graphite-plugins` | Native plugin manifests, catalogs, VSIX packaging |
| `graphite-extension-host` | Out-of-process extension host protocol |
| `graphite-plugin-sdk` | SDK for writing native Graphite plugins |
| `graphite-auth` | GitHub device-flow sign-in and the stored session |
| `graphite-chat` | Chat providers, agent sessions and tool approval |
| `graphite-i18n` | Embedded translation catalogs and runtime lookup |
| `graphite-xtask` | The repository's verification command |
| `graphite-app` | `winit` window, file tree, editor UI |

The app also includes a generic Language Server Protocol client and an ANSI terminal backed by a cross-platform PTY.

## Documentation

- English: [docs/en](docs/en/README.md)
- Português (pt-BR): [docs/pt-BR](docs/pt-BR/README.md)

## License

Graphite is released under the [MIT license](LICENSE). The distributions also carry `THIRD_PARTY_NOTICES.txt`, with the license of every library they include, and the notices of the bundled Code OSS and Node.js runtimes and of the icon font.

