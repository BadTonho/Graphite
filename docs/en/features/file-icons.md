# File icons

Graphite draws a colored icon next to every file and folder in three places:

- the **Explorer** sidebar, including chevrons that show folder state;
- the **Quick Open** list (`Ctrl+P`);
- **editor tabs**.

The icon is chosen from the file name, not from the file contents, so it works
for files that are not open and does not require a language server.

## Where the icons come from

Icons are glyphs from an embedded subset of **Nerd Fonts Symbols Only v3.5.1**,
renamed to the family `Graphite Icons`. The renderer loads the font at startup
and draws each glyph through the existing glyph atlas, tinted with the color
for that file type. This keeps the binary self-contained: no icon files are
read from disk, and the same assets work on Windows and Linux.

The subset contains 165 glyphs and is checked into
`crates/graphite-render/assets/graphite-icons.ttf`. Licensing and rebuild
instructions are in
[`crates/graphite-render/assets/README.md`](../../../crates/graphite-render/assets/README.md).

## Mapping

The mapping lives in `crates/graphite-app/src/explorer/file_icons.rs` and is data-only.
Resolution runs in this order:

1. **Special file names** — `Cargo.toml`, `Cargo.lock`, `package.json`,
   `tsconfig.json`, `Dockerfile`, `Makefile`, `CMakeLists.txt`, `.gitignore`,
   `.env*`, `LICENSE*`, `README*`, and similar project files.
2. **Special prefixes** — tool configuration names such as `vite.config.*`,
   `webpack.config.*`, `eslint.config.*`, `tailwind.config.*`, `docker-compose.*`
   and `requirements*.txt`.
3. **Extension** — the longest suffix after the final dot, matched
   case-insensitively.
4. **Fallback** — a generic file icon; unknown folders use the closed/open
   folder icons.

Highlights of the language coverage:

| Language | Extensions | Language | Extensions |
| --- | --- | --- | --- |
| Rust | `.rs` | Go | `.go` |
| Python | `.py`, `.pyi`, `.pyw` | Ruby | `.rb`, `.erb`, `.gemspec` |
| JavaScript | `.js`, `.mjs`, `.cjs` | PHP | `.php` |
| TypeScript | `.ts`, `.mts`, `.cts` | Swift | `.swift` |
| React | `.jsx`, `.tsx` | Kotlin | `.kt`, `.kts` |
| HTML/CSS | `.html`, `.css` | Dart | `.dart` |
| Sass/Less | `.scss`, `.sass`, `.less` | Scala | `.scala`, `.sc` |
| Vue/Svelte/Astro | `.vue`, `.svelte`, `.astro` | Haskell | `.hs`, `.lhs` |
| C/C++ | `.c`, `.h`, `.cpp`, `.hpp` | Elixir/Erlang | `.ex`, `.exs`, `.erl` |
| C# | `.cs`, `.csx` | Clojure | `.clj`, `.cljs`, `.edn` |
| Java | `.java` | Julia/Zig/Nim | `.jl`, `.zig`, `.nim` |

The mapping also covers data and configuration formats (JSON, YAML, TOML, XML,
SQL), build tools (CMake, Make, Gradle, Maven, Bazel), databases, containers
and infrastructure (Docker, Kubernetes, Helm, Terraform, Nginx), documents,
images, audio, video, fonts, archives, lock files and binaries.

## Colors

Each mapping stores a 24-bit color chosen to match the language or tool brand
(for example Rust `#ce422b`, TypeScript `#3178c6`, Go `#00add8`). The renderer
tints the glyph with that color, so icons stay recognizable in both the dark
and light themes.

## Adding an icon

1. Add the Nerd Fonts codepoint to `AVAILABLE_GLYPHS` in
   `crates/graphite-render/src/icons.rs`.
2. Rebuild the subset with
   `python crates/graphite-render/assets/generate-icons.py --source SymbolsNerdFont-Regular.ttf`.
3. Add a constant and a mapping entry in
   `crates/graphite-app/src/explorer/file_icons.rs`.
4. Run `cargo test -p graphite-render` and
   `cargo test -p graphite-app --bin graphite file_icons`. The tests verify that
   every mapped glyph exists in the subset and rasterizes.

## Verification

Automated coverage includes extension mapping, case-insensitive matching,
special names and prefixes, fallbacks, and glyph availability. A contact sheet
for visual inspection can be generated from the subset with Pillow; the
existing reference is `target/icon-contact-sheet.png`.
