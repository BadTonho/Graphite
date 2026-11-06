# Roadmap

- [x] **v0.1 — Base:** janela `winit`, renderizador `wgpu`, `cosmic-text`, árvore de pastas, editar/salvar, highlighting com tree-sitter
- [ ] **v0.2 — Edição e personalização:** concluir seleção, clipboard, desfazer/refazer, realce incremental, acessibilidade e IME; adicionar abas, busca no projeto (ripgrep), temas e importação do subconjunto suportado de temas, snippets e `settings.json` do VS Code.
- [ ] **v0.3 — Servidores de linguagem e terminal:** cliente LSP stdio genérico para servidores instalados e configurados pelo usuário; sincronização por documento e diagnósticos; autocomplete, definição, hover e navegação por diagnósticos; terminal PTY multiplataforma com parser ANSI e shell iniciado na raiz do workspace.
- [ ] **v0.4 — Prova da arquitetura de extensões:** extension host Node + ponte RPC com um subconjunto mínimo da API (`commands`, `workspace`, `window`, `languages`)
- [ ] **v0.5:** git completo (API SCM), tree views, decorations
- [ ] **v0.6:** webviews (`wry`), depuração (DAP)
- [ ] **v1.0+:** ampliar a API guiado pela suíte de compatibilidade; GitHub (`octocrab`); plugins WASM onde a compatibilidade não alcançar

Verificação da v0.1: o buffer Rope carrega e salva UTF-8 em blocos, salva por meio de um arquivo temporário no mesmo diretório e preserva o caminho e o estado de alteração em caso de falha. A análise incremental do tree-sitter lê diretamente os blocos da Rope. Os testes cobrem arquivos Unicode grandes, falhas de entrada e saída, regras de exclusão em pastas aninhadas e realce incremental. O build e os testes automatizados do workspace passaram no Windows; a verificação interativa da janela, da recuperação da GPU e do Linux ainda está pendente.

Critérios da v0.2: cada aba mantém documento/cursor/seleção/scroll/estado de alteração; edição e histórico seguros para Unicode; busca respeitando exclusões do workspace; importação preservando arquivos de origem e apresentando erros claros para itens inválidos ou não suportados; realce incremental; integração de acessibilidade e IME. Verificar fluxos principais no Windows e Linux e cobrir buffer, busca e importação com testes unitários. A importação aplica somente configurações suportadas e não promete compatibilidade completa com o VS Code.

Critérios da v0.3: processos de servidores configurados rodam fora da thread da interface e encerram com o app; sincronização e diagnósticos por documento respeitam versões e posições UTF-16; autocomplete, definição, hover e navegação por diagnósticos funcionam por aba; o shell do terminal usa PTY multiplataforma, parser ANSI, entrada de teclado, redimensionamento e histórico de rolagem. Verificar compilação e interação básica no Windows e Linux.

