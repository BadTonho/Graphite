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

- Keep crates focused; split a crate when it mixes concerns.
- Update docs in `docs/en/` **and** the matching file in `docs/pt-BR/` in the same change.
- Support both Windows and Linux; use `#[cfg(...)]` when platform code is unavoidable.
- Never commit secrets (see [security policy](../security/policy.md)).

