# Linguagens Suportadas e Descoberta Padrão de LSP

O Graphite combina gramáticas Tree-sitter incrementais embutidas com descoberta automática de Language Servers (`LSP`) no `PATH`, preservando a prioridade de qualquer configuração explícita em `lsp.json`.

## Linguagens Tree-sitter Nativas (`crates/graphite-syntax`)

Cada linguagem embutida conta com destaque de sintaxe completo e incremental, detecção de escopo (*Outline* / *Sticky Scroll*) e *selection ranges*:

| Linguagem | Extensões | Fallback LSP (`PATH`) |
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

## Precedência de Configuração (`crates/graphite-app/src/language/lsp/defaults.rs`)

1. Entradas definidas manualmente em `lsp.json` têm prioridade absoluta para a extensão correspondente.
2. Para extensões sem entrada explícita, o `LspManager` verifica os candidatos de `DEFAULT_LANGUAGE_SERVERS` no `PATH` (respeitando `PATHEXT` no Windows e bits de execução no Linux/Unix).
3. Quando um servidor LSP não está instalado no sistema, nenhum processo é iniciado e o editor continua operando normalmente com o Tree-sitter nativo.
