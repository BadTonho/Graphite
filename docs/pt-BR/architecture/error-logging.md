# Registro de Erros em Quatro Níveis

O Graphite persiste todo erro em tempo de execução, `panic!` não tratado e falha de subsistema em quatro arquivos separados por nível de severidade no diretório de configuração do usuário.

## Local de Armazenamento

- **Windows:** `%APPDATA%\graphite\logs\errors\`
- **Linux:** `$XDG_CONFIG_HOME/graphite/logs/errors/` (ou `~/.config/graphite/logs/errors/`)

Cada nível grava em seu próprio arquivo JSON Lines (`.jsonl`):

| Nível | Arquivo | Escopo |
| :--- | :--- | :--- |
| **4 (Mais alto)** | `critical.jsonl` | Crashes fatais, `panic!` não tratados, falhas de inicialização de GPU/janela. |
| **3 (Alto)** | `high.jsonl` | Falhas em fluxos principais (E/S de arquivos, configuração corrompida, erro ao baixar/aplicar/verificar SHA-256 de updates, crash do host de extensões). |
| **2 (Médio)** | `medium.jsonl` | Degradação de subsistema com o editor continuando em execução (LSP, Git/SCM, terminal, plugins, erro em verificação manual de updates). |
| **1 (Mais baixo)** | `low.jsonl` | Falhas ou travamentos em funções não críticas/secundárias (erro silencioso na verificação em segundo plano de updates, timeouts opcionais de metadados). |

## Formato de Entrada e Rotação

Cada linha é um objeto JSON independente contendo:

- `timestamp_unix_ms`: Timestamp UTC em milissegundos (epoch)
- `severity`: `"critical" | "high" | "medium" | "low"`
- `source`: Identificador do subsistema (ex.: `"panic"`, `"updates"`, `"window.create"`)
- `message`: Mensagem de erro sanitizada
- `os`: Sistema operacional (`"windows"`, `"linux"`, etc.)
- `version`: Versão do pacote Graphite

Quando um arquivo atinge 5 MB, ele é rotacionado para `<severity>.1.jsonl` antes de novas gravações.

## Segurança e Privacidade

Antes de gravar em disco, as mensagens passam por `sanitize_message`, que oculta tokens Bearer e prefixos comuns de chaves de API (`Bearer ...`, `ghp_...`, `github_pat_...`, `sk-...`). Conteúdos brutos de arquivos e credenciais nunca devem ser registrados.
