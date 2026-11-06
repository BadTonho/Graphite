# Roadmap

- [x] **v0.1 — Base:** janela `winit`, renderizador `wgpu`, `cosmic-text`, árvore de pastas, editar/salvar, highlighting com tree-sitter
- [ ] **v0.2 — Edição e personalização:** concluir seleção, clipboard, desfazer/refazer, realce incremental, acessibilidade e IME; adicionar abas, busca no projeto (ripgrep), temas e importação do subconjunto suportado de temas, snippets e `settings.json` do VS Code.
- [ ] **v0.3:** LSP nativo + terminal integrado
- [ ] **v0.4 — Prova da arquitetura de extensões:** extension host Node + ponte RPC com um subconjunto mínimo da API (`commands`, `workspace`, `window`, `languages`)
- [ ] **v0.5:** git completo (API SCM), tree views, decorations
- [ ] **v0.6:** webviews (`wry`), depuração (DAP)
- [ ] **v1.0+:** ampliar a API guiado pela suíte de compatibilidade; GitHub (`octocrab`); plugins WASM onde a compatibilidade não alcançar

Critérios da v0.2: cada aba mantém documento/cursor/seleção/scroll/estado de alteração; edição e histórico seguros para Unicode; busca respeitando exclusões do workspace; importação preservando arquivos de origem e apresentando erros claros para itens inválidos ou não suportados; realce incremental; integração de acessibilidade e IME. Verificar fluxos principais no Windows e Linux e cobrir buffer, busca e importação com testes unitários. A importação aplica somente configurações suportadas e não promete compatibilidade completa com o VS Code.

