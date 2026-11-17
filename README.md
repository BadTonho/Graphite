<p align="center">
  <img src="assets/branding/graphite.png" alt="Graphite" width="140">
</p>

<h1 align="center">Graphite</h1>

<p align="center">
  A fast, native code editor written in Rust, with room for the extensions you already use.
</p>

<p align="center">
  <img alt="Version 0.1.0" src="https://img.shields.io/badge/version-0.1.0-blue">
  <img alt="MIT license" src="https://img.shields.io/badge/license-MIT-green">
  <img alt="Windows and Linux" src="https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-lightgrey">
  <img alt="Built with Rust" src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust">
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#features">Features</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#keyboard-shortcuts">Shortcuts</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#build-from-source">Build</a> ·
  <a href="docs/en/README.md">Documentation</a>
</p>

---

Graphite draws its whole interface directly on the GPU with [`wgpu`](https://wgpu.rs) instead of embedding a browser, so it starts quickly and stays light. It keeps what you expect from a modern editor (language servers, Git, a terminal, notebooks, themes) and can run existing **`.vsix` extensions** from [Open VSX](https://open-vsx.org) in a separate host process.

> **Status: 0.1.0, the first release.** Graphite is ready to try on Windows and Linux (x86-64), but it is young. Read the [known limitations](docs/en/roadmap/known-limitations.md) before relying on it for important work.

## Install

Download a package from the [Releases](https://github.com/BadTonho/Graphite/releases) page and check it against its `.sha256` file.

| System | Packages |
|---|---|
| **Windows** (x86-64) | Setup `.exe` (per user, no administrator rights) or portable `.zip` |
| **Linux** (x86-64) | `.deb`, `.rpm`, Arch `.pkg.tar.zst`, `.AppImage` or portable `.tar.gz` |

- The Windows installer is not code-signed yet, so Windows SmartScreen may warn when you run it. Choose **More info**, then **Run anyway**, if you trust the download.
- Windows needs up-to-date graphics drivers, and WebView2 for extension webviews.
- The portable packages include the Code OSS host and Node.js that VSIX extensions run on.

## Features

**Editing**
- Multiple cursors, code folding, bracket and indentation tools, line operations and syntax-aware selection.
- Syntax highlighting with tree-sitter, plus TextMate grammars that come with extensions.
- Side-by-side and inline diffs, a three-way merge editor and a multi-file diff.
- Notebooks (`.ipynb`) with local kernels, and an interactive window.

**Language support**
- A generic Language Server Protocol client: completion, hover, go to definition, formatting, refactoring and diagnostics.
- A debugger that talks to debug adapters, with breakpoints, variables and a debug console.
- An integrated terminal on a cross-platform PTY, with shell integration.

**Workflow**
- Git built in: status, staging, commits, a history graph, branches, stashes and GitHub sign-in.
- Quick open, project search, a command palette, outline and timeline views, and local file history.
- Workspaces with several folders, tasks and edit sessions over SSH.

**Extensions and customization**
- Native Graphite plugins and VS Code `.vsix` extensions from Open VSX, in a separate host process.
- Light, dark and high-contrast themes, imported settings, snippets and themes, and file-type icons.
- Chat panels and agents backed by the model provider you configure, with explicit approval for tools.
- Settings sync through a private GitHub Gist or a private repository.
- A complete interface in English and Portuguese (Brazil).

## Quick start

1. Open a folder with **File > Open Folder** (`Ctrl+K`, `Ctrl+O`), or start Graphite from a terminal:
   ```bash
   graphite <folder>      # omit <folder> to open the current directory
   ```
2. Press `Ctrl+P` to jump to a file and `Ctrl+Shift+P` (or `F1`) for the command palette.
3. Open **Plugins** (`Ctrl+Shift+X`) to install a native plugin or an extension from Open VSX. Enable it, trust the workspace and run its commands from the palette. See [Installing plugins](docs/en/contributing/plugins.md).
4. Open **Source Control** in the activity bar to see changes, stage files and commit. Sign in with GitHub from the profile menu to publish and sync.

## Keyboard shortcuts

The essentials:

| Action | Shortcut |
|---|---|
| Command palette | `Ctrl+Shift+P` or `F1` |
| Quick open | `Ctrl+P` |
| Save | `Ctrl+S` |
| Find / replace in document | `Ctrl+F` / `Ctrl+H` |
| Search project | `Ctrl+Shift+F` |
| Plugins | `Ctrl+Shift+X` |
| Toggle terminal | `Ctrl+grave` |
| Completion / go to definition | `Ctrl+Space` / `F12` |

<details>
<summary>More shortcuts</summary>

| Action | Input |
|---|---|
| Multiple cursors / next occurrence / all occurrences | `Alt+Click` / `Ctrl+D` / `Ctrl+Shift+L` |
| Select all / extend selection | `Ctrl+A` / `Shift` + navigation keys |
| Copy / cut / paste | `Ctrl+C` / `Ctrl+X` / `Ctrl+V` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y` |
| Move / duplicate / delete line | `Alt+Up/Down` / `Shift+Alt+Up/Down` / `Ctrl+Shift+K` |
| Expand / shrink syntax selection | `Shift+Alt+Right/Left` |
| Convert indentation | `Ctrl+Shift+Alt+I` |
| Switch / close tab | `Ctrl+Tab` / `Ctrl+W` |
| Toggle light/dark theme | `Ctrl+Alt+T` |
| Import user settings, snippets and themes | `Ctrl+Shift+I` |
| Hover / next or previous diagnostic | `Ctrl+K`, `Ctrl+I` / `F8` / `Shift+F8` |
| Show diagnostics | `Ctrl+Shift+M` |

Every command is in the command palette, where you can also see and change its shortcut.

</details>

## Configuration

Graphite keeps its settings in `%APPDATA%\Graphite` on Windows and in `$XDG_CONFIG_HOME/Graphite` (or `~/.config/Graphite`) on Linux. Settings kept elsewhere must be copied there to be used.

- **Language servers:** user-installed executables, configured in `lsp.json`. See [Languages and LSP](docs/en/features/languages-and-lsp.md).
- **Terminal shell:** the system shell by default; change it in `terminal.json` or with `GRAPHITE_SHELL`.
- **Language of the interface:** detected from the operating system. Override it with `"workbench.language": "pt-BR"` in `settings.json` or with `GRAPHITE_LANG=pt-BR`. See [Localization](docs/en/features/localization.md).
- **Settings sync:** turn it on from the profile menu. See [User data sync](docs/en/features/user-data-sync.md).

## Build from source

You need a recent stable [Rust toolchain](https://rustup.rs). On Linux, also install the desktop libraries the build links against (Debian and Ubuntu package names):

```bash
sudo apt-get install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev \
  libxrandr-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev pkg-config
```

```bash
cargo run -p graphite-app -- <folder>   # run the editor
cargo test --workspace                  # run the tests
cargo run -p graphite-xtask -- verify   # formatting, Clippy, build and tests in one go
```

[Building](docs/en/contributing/building.md) explains how to produce the portable package, [Testing](docs/en/contributing/testing.md) and the [verification matrix](docs/en/contributing/verification-matrix.md) describe what is checked, and [Releases](docs/en/contributing/releases.md) covers packages and signing.

<details>
<summary>Project layout</summary>

| Crate | Responsibility |
|---|---|
| `graphite-app` | `winit` window, file tree and the editor interface |
| `graphite-text` | Rope-based text buffer, cursors and file I/O |
| `graphite-render` | `wgpu` renderer and `cosmic-text` glyph atlas |
| `graphite-syntax` | tree-sitter and TextMate syntax highlighting |
| `graphite-workspace` | Folder model and `.gitignore`-aware directory listing |
| `graphite-core-api` | UI-independent command, document and service contracts |
| `graphite-plugins` | Native plugin manifests, catalogs and VSIX packaging |
| `graphite-plugin-sdk` | SDK for writing native Graphite plugins |
| `graphite-extension-host` | Out-of-process extension host protocol |
| `graphite-auth` | GitHub device-flow sign-in and the stored session |
| `graphite-chat` | Chat providers, agent sessions and tool approval |
| `graphite-i18n` | Embedded translation catalogs and runtime lookup |
| `graphite-xtask` | The repository's verification command |

</details>

## Documentation

- **English:** [docs/en](docs/en/README.md)
- **Português (Brasil):** [docs/pt-BR](docs/pt-BR/README.md)

Good places to start: the [architecture overview](docs/en/architecture/overview.md), the [roadmap](docs/en/roadmap/roadmap.md), the [known limitations](docs/en/roadmap/known-limitations.md) and the [security policy](docs/en/security/policy.md).

## License

Graphite is released under the [MIT license](LICENSE). The distributions also carry `THIRD_PARTY_NOTICES.txt`, with the license of every library they include, and the notices of the bundled Code OSS and Node.js runtimes and of the icon font.
