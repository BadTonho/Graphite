# Known limitations (0.1.0)

Graphite 0.1.0 is the first release. These are the things to know before using it for important work.

## Platforms and installation

- Windows and Linux on **x86-64** only. There are no macOS, ARM or 32-bit builds.
- Most of the interface was verified on Windows. Linux builds and tests run, but the interface under every GPU, X11, Wayland and WebKit combination has had less testing.
- The Windows installer is **not code-signed** (Authenticode), so SmartScreen may show a warning.
- In-app updates only install assets signed with the project's update key. Anything else must be downloaded and installed by hand. Native Linux packages and the AppImage are updated through their package manager or by downloading again.

## Editor and tools

- **Diff view:** text is not syntax highlighted, lines longer than their column are cut and there is no horizontal scrolling. Unchanged regions can be folded but not expanded with a click.
- **Merge editor:** the result pane is read-only; use **Open result in the editor** to finish by hand.
- **Notebooks:** only local kernels are started. SVG, HTML pages and interactive widgets are shown as text or not at all. Outputs and images were not checked visually on every platform.
- **Debugging:** tested against a simulated adapter, not against every real debug adapter.
- **Remote and tunnels:** use the system OpenSSH client.

## Extensions

- Support for existing VS Code `.vsix` extensions is **partial**. Extensions that need APIs Graphite does not provide will not work or will only partly work. See [Extension compatibility](../architecture/extension-compat.md).
- Voice dictation and the proprietary account and billing commands of other editors are intentionally absent.

## Chat and sync

- Chat panels need a model provider that you configure; Graphite does not include one.
- Settings sync works through a private GitHub Gist or a private repository. The repository option asks for GitHub's broad `repo` permission, because GitHub offers no narrower one for a personal token.

## Feature parity

Graphite does not aim to reproduce every command of other editors. See the [feature parity inventory](feature-parity.md) and the [remaining stages](remaining-stages.md) for what is done and what is planned.
