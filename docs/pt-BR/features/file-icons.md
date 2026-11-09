# Ícones de arquivos

O Graphite desenha um ícone colorido ao lado de cada arquivo e pasta em três
lugares:

- na barra lateral do **Explorer**, incluindo chevrons que indicam o estado da
  pasta;
- na lista do **Quick Open** (`Ctrl+P`);
- nas **abas do editor**.

O ícone é escolhido pelo nome do arquivo, não pelo conteúdo, então funciona
para arquivos que não estão abertos e não depende de um language server.

## De onde vêm os ícones

Os ícones são glifos de um subset embutido do **Nerd Fonts Symbols Only
v3.5.1**, renomeado para a família `Graphite Icons`. O renderer carrega a fonte
na inicialização e desenha cada glifo pelo atlas de glifos existente, tingido
com a cor daquele tipo de arquivo. O binário continua autocontido: nenhum
arquivo de ícone é lido do disco e os mesmos assets funcionam no Windows e no
Linux.

O subset contém 165 glifos e está versionado em
`crates/graphite-render/assets/graphite-icons.ttf`. Licenças e instruções de
recompilação estão em
[`crates/graphite-render/assets/README.md`](../../../crates/graphite-render/assets/README.md).

## Mapeamento

O mapeamento fica em `crates/graphite-app/src/file_icons.rs` e contém apenas
dados. A resolução segue esta ordem:

1. **Nomes especiais** — `Cargo.toml`, `Cargo.lock`, `package.json`,
   `tsconfig.json`, `Dockerfile`, `Makefile`, `CMakeLists.txt`, `.gitignore`,
   `.env*`, `LICENSE*`, `README*` e arquivos de projeto semelhantes.
2. **Prefixos especiais** — nomes de configuração de ferramentas como
   `vite.config.*`, `webpack.config.*`, `eslint.config.*`, `tailwind.config.*`,
   `docker-compose.*` e `requirements*.txt`.
3. **Extensão** — o sufixo após o último ponto, comparado sem diferenciar
   maiúsculas de minúsculas.
4. **Fallback** — um ícone genérico de arquivo; pastas desconhecidas usam os
   ícones de pasta fechada/aberta.

Destaques da cobertura de linguagens:

| Linguagem | Extensões | Linguagem | Extensões |
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

O mapeamento também cobre formatos de dados e configuração (JSON, YAML, TOML,
XML, SQL), ferramentas de build (CMake, Make, Gradle, Maven, Bazel), bancos de
dados, containers e infraestrutura (Docker, Kubernetes, Helm, Terraform,
Nginx), documentos, imagens, áudio, vídeo, fontes, arquivos compactados, lock
files e binários.

## Cores

Cada mapeamento guarda uma cor de 24 bits escolhida para combinar com a marca da
linguagem ou ferramenta (por exemplo Rust `#ce422b`, TypeScript `#3178c6`, Go
`#00add8`). O renderer tinge o glifo com essa cor, então os ícones continuam
reconhecíveis nos temas escuro e claro.

## Adicionando um ícone

1. Adicione o codepoint do Nerd Fonts em `AVAILABLE_GLYPHS` em
   `crates/graphite-render/src/icons.rs`.
2. Recompile o subset com
   `python crates/graphite-render/assets/generate-icons.py --source SymbolsNerdFont-Regular.ttf`.
3. Adicione uma constante e uma entrada de mapeamento em
   `crates/graphite-app/src/file_icons.rs`.
4. Execute `cargo test -p graphite-render` e
   `cargo test -p graphite-app --bin graphite file_icons`. Os testes verificam
   que cada glifo mapeado existe no subset e é rasterizado.

## Verificação

A cobertura automatizada inclui mapeamento por extensão, comparação sem
diferenciar maiúsculas de minúsculas, nomes e prefixos especiais, fallbacks e
disponibilidade dos glifos. Uma folha de contato para inspeção visual pode ser
gerada a partir do subset com Pillow; a referência existente é
`target/icon-contact-sheet.png`.
