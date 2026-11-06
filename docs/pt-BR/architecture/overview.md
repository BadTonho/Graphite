# Visão Geral da Arquitetura

## Crates

```
shoppe-app ──► shoppe-render     (wgpu + cosmic-text + Window do winit)
           ├─► shoppe-text       (buffer com ropey)
           ├─► shoppe-workspace  (crate ignore)
           └─► shoppe-syntax     (tree-sitter)
```

## Renderização

- **Stack de UI:** UI própria sobre `wgpu` (desenho na GPU) e `cosmic-text` (shaping, fontes, bidi, fallback), com `winit` para janelas e entrada. Escolhida em vez de egui/GPUI/iced por independência, controle de desempenho e licenças limpas.
- **Pipeline:** um único pipeline de quads texturizados. Retângulos amostram um bloco sólido 2×2 reservado no atlas; glifos amostram um atlas de cobertura `R8` preenchido sob demanda por um empacotador de prateleiras.
- **Cache de texto:** linhas já moldadas são cacheadas por `(texto, tamanho da fonte)`; cada glifo é rasterizado uma vez por `CacheKey`.
- **Formato da superfície:** não-sRGB quando disponível, para que as cores apareçam como definidas.

## Texto e highlighting

- O texto fica em um `ropey::Rope`; o cursor é um deslocamento em caracteres. Toda edição incrementa um contador `revision`.
- `shoppe-syntax` roda o `tree-sitter-highlight` e devolve spans por linha (intervalos de bytes) com uma `Class` semântica. O app mapeia classes para cores e só refaz o highlight quando a revisão muda.

## Plataformas

Windows e Linux; as diferenças de plataforma ficam a cargo de `winit`, `wgpu` e `cosmic-text`.

## Limitações conhecidas (v0.1)

- Re-highlight do arquivo inteiro a cada edição (ainda sem parsing incremental).
- Sem seleção, área de transferência, desfazer/refazer ou abas.
- O cache de glifos nunca é limpo.
- Glifos coloridos (emoji) são desenhados como máscaras de alpha.
- Acessibilidade (`accesskit`) e IME ainda não foram integrados.

