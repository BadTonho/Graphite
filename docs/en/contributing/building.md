# Building and Testing

## Requirements

- Recent stable Rust toolchain (`rustup`).
- A GPU/driver supported by `wgpu` (DirectX 12 or Vulkan on Windows; Vulkan or OpenGL on Linux).
- Linux: development packages for X11/Wayland and fontconfig.
- A C compiler (tree-sitter grammars compile C code): MSVC Build Tools on Windows, `gcc`/`clang` on Linux.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo run -p graphite-app -- <folder>
```

## Conventions

- Add or update Rust tests for new and changed behavior and bug fixes, including relevant failure cases. Keep test data isolated and independent of personal files, network services and external language servers.
- Run relevant tests and `cargo test --workspace` before completing a code change; use `cargo build --workspace` to validate the build. Report failed or unrun checks, their reason and the platform tested. Document-only changes do not need new tests unless they change executable behavior.
- Native tests run locally on the available operating system. Verify window or GPU interactions manually when they cannot be reliably automated and report any remaining gaps.
- Keep crates focused; split a crate when it mixes concerns.
- Update docs in `docs/en/` **and** the matching file in `docs/pt-BR/` in the same change.
- Support both Windows and Linux; use `#[cfg(...)]` when platform code is unavoidable.
- Never commit secrets (see [security policy](../security/policy.md)).

