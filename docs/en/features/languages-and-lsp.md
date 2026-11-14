# Supported Languages and Default LSP Discovery

Graphite combines bundled incremental Tree-sitter grammars with automatic Language Server (`LSP`) discovery on `PATH`, while keeping manual `lsp.json` configuration authoritative.

## Built-in Tree-sitter Languages (`crates/graphite-syntax`)

Every built-in language supports full and incremental syntax highlighting, scope detection (Outline / Sticky Scroll), and selection ranges:

| Language | File Extensions | LSP Fallback (`PATH`) |
| --- | --- | --- |
| Rust | `.rs` | `rust-analyzer` |
| Python | `.py`, `.pyi` | `pyright-langserver`, `basedpyright-langserver`, `pylsp` |
| JavaScript | `.js`, `.mjs`, `.cjs`, `.jsx` | `typescript-language-server --stdio` |
| TypeScript | `.ts`, `.mts`, `.cts` | `typescript-language-server --stdio` |
| TSX | `.tsx` | `typescript-language-server --stdio` |
| JSON / JSONC | `.json`, `.jsonc` | `vscode-json-language-server --stdio` |
| HTML | `.html`, `.htm` | `vscode-html-language-server --stdio` |
| CSS | `.css` | `vscode-css-language-server --stdio` |
| C | `.c`, `.h` | `clangd` |
| C++ | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` | `clangd` |
| Go | `.go` | `gopls` |
| TOML | `.toml` | `taplo lsp stdio` |
| YAML | `.yml`, `.yaml` | `yaml-language-server --stdio` |
| Bash / Shell | `.sh`, `.bash`, `.zsh` | `bash-language-server start` |
| Markdown | `.md`, `.markdown` | `marksman server` |
| Java | `.java` | `jdtls` |
| C# | `.cs` | `csharp-ls`, `OmniSharp -lsp`, `omnisharp -lsp` |
| PHP | `.php`, `.phtml` | `intelephense --stdio`, `phpactor language-server` |
| Kotlin | `.kt`, `.kts` | `kotlin-language-server` |
| Scala | `.scala`, `.sc` | `metals` |
| Ruby | `.rb`, `.rake`, `.gemspec` | `ruby-lsp`, `solargraph stdio` |
| SQL | `.sql` | `sql-language-server up --method stdio`, `sqls` |
| PowerShell | `.ps1`, `.psm1`, `.psd1` | `powershell-editor-services` |
| Lua | `.lua` | `lua-language-server` |
| Zig | `.zig` | `zls` |
| Elixir | `.ex`, `.exs` | `elixir-ls` |
| SCSS | `.scss` | `vscode-css-language-server --stdio`, `css-languageserver --stdio` |

## Configuration Precedence (`crates/graphite-app/src/language/lsp/defaults.rs`)

1. Entries declared in `lsp.json` are loaded first and always take priority per file extension.
2. Any extension not explicitly configured in `lsp.json` falls back to `DEFAULT_LANGUAGE_SERVERS` when the candidate executable exists on `PATH` (checking `PATHEXT` on Windows and executable permissions on Unix/Linux).
3. If an LSP binary is not installed, no process is spawned and native Tree-sitter highlighting remains active without error noise.
