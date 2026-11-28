# Building and Testing

## Requirements

- Recent stable Rust toolchain (`rustup`).
- Node.js 24+ for native plugins, Emmet and their integration tests when running a regular Cargo build. The portable distribution bundles Node.js 24.22.1 and the Code OSS host for VSIX execution.
- A GPU/driver supported by `wgpu` (DirectX 12 or Vulkan on Windows; Vulkan or OpenGL on Linux).
- Linux: development packages for X11/Wayland and fontconfig.
- A C compiler (tree-sitter grammars compile C code): MSVC Build Tools on Windows, `gcc`/`clang` on Linux.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo run -p graphite-app -- <folder>
```

Debug builds optimize the dependencies (`opt-level = 2` for every package outside the workspace) and leave the workspace's own crates unoptimized. Tree-sitter, wgpu, regex, and text shaping then run several times faster in `cargo run` and the tests (the syntax tests take about a third of the time), while edits to Graphite's code still rebuild quickly. The first build after a clean checkout or a dependency update takes longer, since every dependency is optimized once. Stepping into dependency code in a debugger shows optimized code.

## Application icon

The temporary artwork lives in `assets/branding/graphite.png`. Replace this source PNG and rebuild to update every generated icon. The build fits the artwork into square canvases without cropping, preserves alpha, embeds 256×256 RGBA pixels in the window, and generates a multi-size ICO for Windows executables. Generated files stay in Cargo's `OUT_DIR`; no image file is required alongside the executable. Windows builds require the resource compiler from the Windows SDK (or `windres` when targeting GNU Windows).

Linux windows use the `graphite` application identity. For desktop integration, install `packaging/linux/graphite.desktop` under `$XDG_DATA_HOME/applications` and the source PNG as `$XDG_DATA_HOME/icons/graphite.png`, with `$XDG_DATA_HOME` defaulting to `~/.local/share`. Keep the `graphite` executable on `PATH`. Wayland launchers obtain their icon from this desktop entry rather than the window icon. Building the workspace does not install these files into the user's desktop.

`cargo test -p graphite-app --test branding` verifies generated sizes/pixels, malformed input, the Linux desktop identity and (on Windows) the icon resource inside the actual built editor executable. UI/taskbar rendering and Linux execution require separate platform checks.

## Simulated extension host

Build the workspace first, then use `cargo run -p graphite-app -- --simulate-extension-host <folder>`. The Rust helper must be beside Graphite. It is disabled by default; Node and Code OSS are not required. The registered `graphite.extensions.restartHost` command restarts it manually. See [Services and simulated host](../architecture/services-host.md) for protocol limits, sample bindings and process tests. For targeted application tests that use the helper, run `cargo build -p graphite-extension-host` first; `cargo test --workspace` builds the process integration target automatically.

## Conventions

See [Regression testing](testing.md) for the coverage matrix, targeted test commands, stdio LSP fixture and remaining manual/platform checks.

- Add or update Rust tests for new and changed behavior and bug fixes, including relevant failure cases. Keep test data isolated and independent of personal files, network services and external language servers.
- Run relevant tests and `cargo test --workspace` before completing a code change; use `cargo build --workspace` to validate the build. Report failed or unrun checks, their reason and the platform tested. Document-only changes do not need new tests unless they change executable behavior.
- Native tests run locally on the available operating system. Verify window or GPU interactions manually when they cannot be reliably automated and report any remaining gaps.
- Keep crates focused; split a crate when it mixes concerns.
- The UI framework in `framework/` (Vitral, a git submodule; fetch it with `git submodule update --init framework`) is a separate Cargo workspace that must not depend on Graphite. Run `cargo test --workspace --manifest-path framework/Cargo.toml` too when it changes; see [UI framework workspace](../architecture/ui-framework.md).
- Update docs in `docs/en/` **and** the matching file in `docs/pt-BR/` in the same change.
- Support both Windows and Linux; use `#[cfg(...)]` when platform code is unavoidable.
- Never commit secrets (see [security policy](../security/policy.md)).

## Portable packages and Code OSS tests

For tag-triggered draft releases and Windows/Debian/Ubuntu/Fedora/Arch/AppImage packages, see [releases](releases.md).

On Windows or Linux x86-64, run `python packaging/portable.py` from the repository root with Python 3.11+, the Rust toolchain and a native C++ toolchain. On Windows, install Visual Studio Build Tools with Desktop development with C++, the Windows SDK and the matching Spectre-mitigated MSVC libraries. On Linux, install a C++ compiler and Make. The script verifies pinned source and Node archives, installs locked upstream dependencies, compiles the host and writes the platform archive to `target/portable/`. The Cargo build by itself requires neither Node nor the Code OSS downloads.

Extract the archive and run Graphite from the extracted directory. Keep the adjacent `code-oss` directory in place. To run the local VSIX integration test, set `GRAPHITE_TEST_CODE_OSS` to that directory and run `cargo test -p graphite-app official_host_runs_local_vsix_services_and_invalidates_sessions -- --ignored`. The separate `open_vsx_sort_command_edits_and_undoes` test also requires `GRAPHITE_TEST_OPEN_VSX_SAMPLE`, pointing to the pinned VSIX recorded in the [v0.5 validation](../features/v0.5-extensions-automation.md). The tests use local files by default. Set `GRAPHITE_TEST_OPEN_VSX_LIVE=1` to also verify the public registry search and download against the pinned file.
