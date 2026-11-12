# Pesquisa de compatibilidade de extensões

`packaging/compat/survey.py` mede quais APIs `vscode.*` extensões reais usam e compara com o que o Graphite suporta. É estática: baixa cada pacote do Open VSX, lê o manifesto e varre o ponto de entrada em busca de acessos como `window.createTerminal`. **Nada de um pacote baixado é executado nem extraído para o disco.**

```bash
python3 -I packaging/compat/survey.py ARQUIVO_LISTA PASTA_SAIDA
```

`ARQUIVO_LISTA` tem um `namespace.nome` por linha; `PASTA_SAIDA` recebe os pacotes e o `report.md`. A lista de suportadas é `packaging/compat/supported_apis.txt` (mantenha em sincronia com [a superfície da API](../features/extension-api.md)) e os nomes válidos de API vêm de `api_members.txt`, gerado a partir do `vscode.d.ts` do commit fixado do Code OSS.

## Como ler o resultado

A varredura é uma heurística. Extensões que embutem `vscode-languageclient` parecem usar quase todos os provedores, porque a biblioteca os registra de forma genérica; o relatório os lista separadamente. APIs que o próprio extension host responde (`workspace.fs`) aparecem como lacunas até entrarem na lista de suportadas. Para dados exatos, execute uma extensão e leia `<perfil>/code-oss/unsupported-api.log`, que registra toda chamada que o Graphite não respondeu.

## Primeira execução (30 extensões populares, uso direto)

| API | Extensões |
|---|---|
| `window.createTextEditorDecorationType` | 9 |
| `workspace.createFileSystemWatcher` | 4 (agora suportado) |
| `window.showOpenDialog` | 4 |
| `window.createTreeView` | 3 |
| `languages.registerCodeLensProvider` | 3 |
| `tasks.executeTask`, `tasks.fetchTasks` | 2 |
| `debug.*` | 1 a 2 |

Decorações de texto, code lenses, tree views e webviews precisam de renderização no editor ou na barra lateral e são as próximas lacunas de maior efeito; Debug e sistemas de arquivos virtuais afetam poucas extensões.
