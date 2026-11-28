# Vitral UI Framework Workspace

Graphite's next UI layer (rounded corners, shadows, Flexbox/Grid layout, animations, accessibility and webviews) is being built as Vitral, a general-purpose Rust UI framework. Vitral lives in its own public repository, [BadTonho/vitral](https://github.com/BadTonho/vitral), and is included here as a git submodule at `framework/`.

Its crates are named `vitral-*`. The name is not reserved on crates.io until the first version is published.

## Layout

```
Graphite/
├── Cargo.toml        Graphite workspace (excludes framework/)
├── .gitmodules       Points framework/ to the Vitral repository
├── crates/           Graphite crates
└── framework/        Git submodule: the Vitral repository
    ├── Cargo.toml    Own workspace, own Cargo.lock
    ├── crates/       core, layout, render, animation, a11y, platform, workspace-checks
    ├── docs/en/  docs/pt-BR/
    ├── AGENTS.md  LICENSE-MIT  LICENSE-APACHE  README.md
```

- The root `Cargo.toml` lists `framework` under `exclude`, so the framework never inherits Graphite's workspace settings, dependency versions or lock file.
- Graphite will depend on framework crates through `path = "framework/crates/..."`. Once Vitral is published on crates.io, that becomes a version dependency and the submodule is removed.

## Working with the submodule

Clone Graphite with the submodule, or fetch it in an existing clone:

```sh
git clone --recurse-submodules https://github.com/BadTonho/Graphite.git
git submodule update --init framework
```

Without the submodule, `framework/` stays empty: Graphite still builds and tests, but the framework steps of `graphite-xtask verify` fail.

To change Vitral:

1. Edit, commit and push inside `framework/`. It is a normal clone of the Vitral repository, with its own history and its own `AGENTS.md`.
2. In Graphite, commit the new submodule pointer (`git add framework`), so Graphite records which Vitral commit it uses.

A Graphite commit never contains Vitral files, only that pointer.

## Rules

- **One direction only:** Graphite may depend on the framework; the framework never depends on Graphite or on any code outside its repository. The `vitral_workspace_is_self_contained` test enforces this.
- **Errors:** the framework does not write logs. It returns errors, and Graphite records them through `app::error_log` at the right severity tier.
- **Text:** the framework has no translations. Graphite passes the strings it displays.

## Adoption

Graphite adopts Vitral component by component, starting at Vitral's stage 3 (basic components; see the [Vitral roadmap](https://github.com/BadTonho/vitral/blob/main/docs/en/roadmap/roadmap.md)). Graphite is Vitral's first real application, so problems in the API show up early, while Vitral's general-purpose rule keeps editor-only needs out of its core crates. Editor-specific widgets stay in Graphite or in optional Vitral crates.

## Checks

The framework has its own build, lint and tests:

```sh
cargo fmt --all --manifest-path framework/Cargo.toml -- --check
cargo clippy --workspace --all-targets --manifest-path framework/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path framework/Cargo.toml
```

`cargo run -p graphite-xtask -- verify` runs these after Graphite's own checks.

CI checks out the submodule at the Vitral commit Graphite pins and runs the same three checks. Vitral also has its own CI on Windows and Linux in its repository.
