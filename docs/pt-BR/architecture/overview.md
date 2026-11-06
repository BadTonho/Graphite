# Visão Geral da Arquitetura

## Crates

```
shoppe-app ──► shoppe-render     (wgpu + cosmic-text + Window do winit)
           ├─► shoppe-text       (buffer com ropey)
           ├─► shoppe-workspace  (crate ignore)
           └─► shoppe-syntax     (tree-sitter)
```

## Responsabilidades dos módulos

- `shoppe-app` separa eventos da janela, entrada do editor, operações de documentos, desenho, acessibilidade, tema e integrações com LSP e terminal em `app/`. Os módulos de configuração separam preferências, temas, snippets, caminhos e importação do VS Code; os módulos de LSP separam gerenciador, processo, distribuição de mensagens e conversão do protocolo.
- `shoppe-text` separa entrada e saída de arquivos, edições e histórico, e navegação do cursor. `shoppe-workspace` separa a busca no projeto da listagem de pastas.
- `shoppe-render` separa a modelagem do texto e rasterização de glifos do pipeline da superfície GPU. `shoppe-syntax` mantém os mecanismos de realce incremental e de documento completo em módulos distintos.

## Renderização

- **Stack de UI:** UI própria sobre `wgpu` (desenho na GPU) e `cosmic-text` (shaping, fontes, bidi, fallback), com `winit` para janelas e entrada. Escolhida em vez de egui/GPUI/iced por independência, controle de desempenho e licenças limpas.
- **Pipeline:** um único pipeline de quads texturizados. Retângulos amostram um bloco sólido 2×2 reservado no atlas; glifos amostram um atlas de cobertura `R8` preenchido sob demanda por um empacotador de prateleiras.
- **Cache de texto:** linhas já moldadas são cacheadas por `(texto, tamanho da fonte)`; cada glifo é rasterizado uma vez por `CacheKey`.
- **Formato da superfície:** não-sRGB quando disponível, para que as cores apareçam como definidas.
- **Recorte:** retângulos e glifos são recortados aos limites do editor, com as coordenadas de textura ajustadas para glifos parcialmente visíveis. A mesma operação poderá delimitar painéis divididos.

## Texto e highlighting

- O texto fica em um `ropey::Rope`; o cursor é um deslocamento em caracteres. Toda edição incrementa um contador `revision`.
- Transações registram seus intervalos em caracteres para mapear posições durante edições, desfazer e refazer. Um modelo de grupos guarda seleções e rolagem independentes para visualizações do mesmo documento e calcula a geometria de divisões aninhadas. O ciclo de abas, eventos de edição, controles de divisão e renderização usam esse modelo; a verificação interativa está pendente.
- O mapeamento de linhas visuais calcula quebras e converte posições de caracteres Unicode e colunas do ponteiro sem alterar o documento. A renderização, o ponteiro e o movimento vertical do cursor usam esse mapeamento quando a quebra está ativa; a verificação interativa permanece pendente.
- `shoppe-syntax` oferece parsing incremental do tree-sitter sobre blocos da Rope e um mecanismo para o documento completo; ambos devolvem spans por linha (intervalos de bytes) com uma `Class` semântica. O app mapeia classes para cores e só refaz o realce quando a revisão muda.

## Plataformas

Windows e Linux; as diferenças de plataforma ficam a cargo de `winit`, `wgpu` e `cosmic-text`.

## Limitações conhecidas

- O realce incremental pode invalidar o restante do documento após uma edição.
- O cache de glifos nunca é limpo.
- Glifos coloridos (emoji) são desenhados como máscaras de alpha.
- A verificação interativa da janela e da recuperação da GPU ainda está pendente no Windows e no Linux.

