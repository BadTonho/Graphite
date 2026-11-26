# Visão Geral da Arquitetura

## Crates

```
graphite-app ──► graphite-render     (wgpu + cosmic-text + Window do winit)
           ├─► graphite-text       (buffer com ropey)
           ├─► graphite-workspace  (crate ignore)
           └─► graphite-syntax     (tree-sitter)
```

## Responsabilidades dos módulos

- `graphite-core-api` fornece contratos de serviços independentes da interface e dados serializáveis. `graphite-extension-host` fornece um supervisor reutilizável de processos e simulador Rust ativado explicitamente. O LSP usa os mesmos serviços com transporte e ciclo de vida próprios por instância. Consulte [Serviços e host simulado](services-host.md).
- `graphite-app` separa eventos da janela, entrada do editor, operações de documentos, desenho, acessibilidade, tema e integrações com LSP e terminal em `app/`. Os módulos de configuração separam preferências, temas, snippets, caminhos e importação do VS Code; os módulos de LSP separam gerenciador, processo, distribuição de mensagens e conversão do protocolo.
- `graphite-text` separa entrada e saída de arquivos, edições e histórico, e navegação do cursor. `graphite-workspace` separa a busca no projeto da listagem de pastas.
- `graphite-render` separa a modelagem do texto e rasterização de glifos do pipeline da superfície GPU. `graphite-syntax` mantém os mecanismos de realce incremental e de documento completo em módulos distintos.

## Renderização

- **Stack de UI:** UI própria sobre `wgpu` (desenho na GPU) e `cosmic-text` (shaping, fontes, bidi, fallback), com `winit` para janelas e entrada. Escolhida em vez de egui/GPUI/iced por independência, controle de desempenho e licenças limpas.
- **Pipeline:** um único pipeline de quads texturizados. Retângulos amostram um bloco sólido 2×2 reservado no atlas; glifos amostram um atlas de cobertura `R8` preenchido sob demanda por um empacotador de prateleiras.
- **Cache de texto:** linhas já moldadas são cacheadas por `(texto, tamanho da fonte)`; cada glifo é rasterizado uma vez por `CacheKey`.
- **Formato da superfície:** não-sRGB quando disponível, para que as cores apareçam como definidas.
- **Recorte:** retângulos e glifos são recortados aos limites do editor, com as coordenadas de textura ajustadas para glifos parcialmente visíveis. A mesma operação poderá delimitar painéis divididos.

## Texto e highlighting

- O texto fica em um `ropey::Rope`; o cursor é um deslocamento em caracteres. Toda edição incrementa um contador `revision`.
- Documentos usam chaves estáveis e compartilham texto/histórico entre views. Cada view mantém exclusivamente seleções, scroll, composição IME e navegação da seleção. Sessões de edição por empréstimo remapeiam as demais views e sincronizam o LSP; undo/redo preservam o foco. Consulte [Documento e view](document-view.md). A verificação nativa de UI/GPU está pendente no Windows e no Linux.
- O mapeamento de linhas visuais calcula quebras e converte posições de caracteres Unicode e colunas do ponteiro sem alterar o documento. A renderização, o ponteiro e o movimento vertical do cursor usam esse mapeamento quando a quebra está ativa; a verificação interativa permanece pendente.
- `graphite-syntax` oferece parsing incremental do tree-sitter sobre blocos da Rope e um mecanismo para o documento completo; ambos devolvem spans por linha (intervalos de bytes) com uma `Class` semântica. O app mapeia classes para cores e só refaz o realce quando a revisão muda.

## Comandos

`CommandRegistry` centraliza metadados, alvos explícitos, enablement e resultados tipados. Handlers por domínio compartilham a entrada para teclado, widgets e acessibilidade. Contextos globais, de views e de widgets são resolvidos por seus ancestrais. Workers devolvem resultados ao event loop com verificações de cancelamento e versão; edições em views sem foco preservam o foco. Consulte [Comandos e contexto](commands-context.md).

## Plataformas

Windows e Linux; as diferenças de plataforma ficam a cargo de `winit`, `wgpu` e `cosmic-text`.

Nunca duplique uma conexão TCP com `TcpStream::try_clone`. No Windows a cópia é herdável, então todo processo iniciado depois (um terminal, uma tarefa, um kernel de notebook, um comando executado pelo Chat) fica com a conexão, e fechá-la no Graphite não a encerra. Leia e escreva por meio de `&TcpStream`, que implementa `Read` e `Write`, e compartilhe uma conexão entre threads com `Arc<TcpStream>`. Sockets retornados por `TcpListener::accept` e arquivos duplicados com `File::try_clone` não são herdáveis.

## Limitações conhecidas

- O realce incremental pode invalidar o restante do documento após uma edição.
- O cache de glifos nunca é limpo.
- Glifos coloridos (emoji) são desenhados como máscaras de alpha.
- A verificação interativa da janela e da recuperação da GPU ainda está pendente no Windows e no Linux.

