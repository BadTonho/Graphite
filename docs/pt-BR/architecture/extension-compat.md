# Compatibilidade com Extensões do VS Code (planejado)

**Implementação atual:** o Graphite pesquisa no Open VSX e instala pacotes VSIX inspecionados, mostrando manifesto e categorias de contribuições. Código VSIX ainda não executa. Plugins JS/TS nativos do Graphite usam API e host Node próprios; consulte [Plugins nativos e inspeção de VSIX](native-plugins.md). A integração Code OSS fixada abaixo continua sendo trabalho necessário.

O alvo de compatibilidade da Fase 5 é o Code OSS **1.145.2**, commit `c4453f8ea430795f941c1b2ff6c9321135a74a02`. A superfície-alvo é a API pública `vscode.d.ts` nos namespaces `commands`, `workspace`, `window` e `languages`. Esse alvo está documentado para implementação e testes; atualmente, nenhuma API desta matriz é atendida pelo host Code OSS.

Serviços internos compartilhados e um host Rust simulado e isolado estão implementados; consulte [Serviços e host simulado](services-host.md). Isso valida sessões, propriedade de recursos, requisições e isolamento de falhas. Ele não fala o protocolo Code OSS. O host Code OSS/Node descrito abaixo continua necessário: fixe sua versão, implemente a ponte do lado principal e ative um VSIX real antes de declarar compatibilidade `.vsix`.

## Abordagem

Rodar o **extension host do Code OSS** (licença MIT, `microsoft/vscode`) como um **processo Node.js separado**, ligado ao editor Rust por RPC:

```
[ Graphite (Rust) ] ←─ ponte RPC ─→ [ Extension Host (Node) ] → [ extensões .vsix ]
```

- **Reaproveitado como está:** carregamento upstream de `.vsix`, ativação, módulo `vscode` e transporte RPC, depois que o host oficial for empacotado.
- **Alvo da ponte na Fase 5:** implementações Rust `MainThread*` para commands, workspace, window e languages, evoluídas com base nas declarações fixadas e nos testes.
- **Fases posteriores:** webviews, depuração e Test Explorer não fazem parte do alvo da Fase 5; outras APIs públicas exigem decisões de compatibilidade separadas.
- **Spawn lazy:** o Node só sobe quando há extensão instalada e pode ser encerrado quando ocioso.
- **Métrica de progresso:** as extensões mais populares do OpenVSX formam a suíte de testes de compatibilidade.

## Matriz de APIs da Fase 5

| Namespace `vscode.d.ts` | Alvo da Fase 5 | Status atual | Evidência necessária |
|---|---|---|---|
| `commands` | Registrar/executar comandos e observar seu ciclo de vida | Não implementado para VSIX | Ativação de VSIX local e fixture de chamada de comando |
| `workspace` | Workspace, documentos, edições, configurações e eventos | Não implementado para VSIX | Testes VSIX locais de leitura, escrita, eventos e falhas |
| `window` | Mensagens, progresso, entrada/seleções rápidas e interface do editor | Não implementado para VSIX | Testes VSIX locais de solicitação/resposta e cancelamento |
| `languages` | Provedores, diagnósticos e registro de recursos de linguagem | Não implementado para VSIX | Testes VSIX locais do ciclo de vida dos provedores e resultados |

Esta matriz descreve o alvo de aceite, não a compatibilidade atual. Webviews, depuração e Test Explorer continuam atribuídos à Fase 6.

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

