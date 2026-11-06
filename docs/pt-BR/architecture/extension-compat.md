# Compatibilidade com Extensões do VS Code (planejado)

A compatibilidade com extensões `.vsix` é um **objetivo central** e molda a arquitetura desde o início.

## Abordagem

Rodar o **extension host do Code OSS** (licença MIT, `microsoft/vscode`) como um **processo Node.js separado**, ligado ao editor Rust por RPC:

```
[ Graphite (Rust) ] ←─ ponte RPC ─→ [ Extension Host (Node) ] → [ extensões .vsix ]
```

- **Reaproveitado como está:** carregamento de `.vsix`, ativação, módulo `vscode`, transporte RPC.
- **Implementado em Rust:** o lado `MainThread*` (documentos, comandos, workspace, window, languages, tree views, decorations, webviews, SCM, debug), crescendo sob demanda.
- **Spawn lazy:** o Node só sobe quando há extensão instalada e pode ser encerrado quando ocioso.
- **Métrica de progresso:** as extensões mais populares do OpenVSX formam a suíte de testes de compatibilidade.

## Camadas de extensões

| Camada | Conteúdo | Custo |
|---|---|---|
| 1. Declarativas | Temas, icon themes, snippets, grammars | Baixo — importador |
| 2. Comandos simples | Formatadores, linters | Médio — host + ponte mínima |
| 3. UI pesada | Tree views, decorations, webviews | Alto — sob demanda |
| 4. API profunda | Debuggers, Copilot clássico | Muito alto — geralmente há alternativa |

## Marketplace e licenças

- O código do sistema de extensões (host, API, RPC, formato `.vsix`) é MIT.
- Os termos do Marketplace oficial da Microsoft restringem o uso a produtos dela: **não é usado**. A distribuição vai pelo **[OpenVSX](https://open-vsx.org)**.
- Algumas extensões da Microsoft (C# Dev Kit, Pylance, Remote-*) são bloqueadas por licença ao VS Code; usam-se alternativas abertas.

## Riscos

A compatibilidade nunca é completa: novas APIs saem a cada release e extensões usam comportamentos não documentados. É oferecida como *compatibilidade*, não como garantia.

