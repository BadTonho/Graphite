# Localização

O Graphite possui um sistema de localização resolvido em tempo de compilação.
Todas as strings visíveis da interface passam por um `Translator`, e os
catálogos de tradução são embutidos no binário, portanto o editor em execução
nunca lê arquivos de idioma do disco.

## Idiomas suportados

| Idioma | Código | Catálogos |
| --- | --- | --- |
| Inglês | `en` | Completo (fallback) |
| Português (Brasil) | `pt-BR` | Completo |

O inglês é o idioma de origem e o fallback final. Qualquer string ausente em
outro idioma resolve para o inglês antes de recorrer à própria chave de
tradução.

## Selecionando o idioma da interface

O idioma de inicialização é resolvido nesta ordem de precedência:

1. Variável de ambiente `GRAPHITE_LANG`.
2. `workbench.language` no arquivo de configurações do usuário.
3. Idioma do sistema operacional.
4. Inglês.

`workbench.language` aceita uma etiqueta de idioma como `pt-BR` ou `pt`, ou os
valores `auto` e `system`, que usam o idioma do sistema operacional. As
etiquetas não diferenciam maiúsculas de minúsculas e aceitam separadores `_` ou
`-` (`pt_br` corresponde a `pt-BR`). Um idioma base corresponde a um único
catálogo regional quando essa correspondência é inequívoca.

As configurações do usuário são lidas do arquivo importado do VS Code e do
arquivo de configurações do Graphite:

- Windows: `%APPDATA%\Graphite\imported\settings.json` e
  `%APPDATA%\Graphite\settings.json`
- Linux: `$XDG_CONFIG_HOME/Graphite/imported/settings.json` (ou
  `~/.config/Graphite/...`) e `$XDG_CONFIG_HOME/Graphite/settings.json`

Exemplo:

```json
{
  "workbench.language": "pt-BR"
}
```

`GRAPHITE_LANG=pt-BR` substitui a configuração em uma sessão. Um valor
desconhecido gera um aviso localizado e passa para a próxima fonte. O
lançador standalone `code` resolve `GRAPHITE_LANG` e depois o idioma do sistema
operacional para seus próprios diagnósticos.

## Estrutura dos catálogos

Todas as fontes de tradução ficam no diretório `locales/` na raiz do
repositório:

```text
locales/
  languages.json
  README.md
  en/
    common.json
    app.json
    commands.json
    terminal.json
    scm.json
    lsp.json
    plugins.json
    accessibility.json
  pt-BR/
    ...os mesmos arquivos...
```

`locales/languages.json` declara cada idioma embutido com seu código, nome em
inglês e nome nativo. Cada diretório de idioma usa os mesmos nomes de arquivo
para manter os domínios pequenos e revisáveis:

| Arquivo | Domínio |
| --- | --- |
| `common.json` | Ações compartilhadas, avisos de idioma, textos genéricos de status |
| `app.json` | Interface do editor, overlays, documentos, salvamento, configuração, CLI, JSON schema |
| `commands.json` | Títulos e categorias de comandos |
| `terminal.json` | Painel do terminal, perfis e diagnósticos |
| `scm.json` | Status de controle de código-fonte, diffs e conflitos |
| `lsp.json` | Status de servidores de linguagem e correções rápidas |
| `plugins.json` | Barra lateral de plugins, runtime e host de extensões |
| `accessibility.json` | Rótulos para leitores de tela |

Os arquivos são objetos JSON planos que mapeiam chaves estáveis para texto.
Placeholders usam chaves nomeadas entre chaves, como `{path}` ou `{error}`;
chaves literais são escritas como `{{` e `}}`. Os nomes dos placeholders nunca
devem ser traduzidos.

## Validação em tempo de compilação

`graphite-i18n/build.rs` valida os catálogos antes da compilação da crate. A
compilação falha quando:

- um arquivo JSON é inválido, tem chaves duplicadas ou não é um objeto de strings;
- um idioma declarado em `languages.json` não possui diretório;
- uma chave está ausente em um idioma ou existe apenas fora do inglês;
- um idioma usa placeholders diferentes do inglês.

Os catálogos validados são gerados em `OUT_DIR` e embutidos no binário. Isso
mantém o comportamento idêntico no Windows e no Linux e evita versões
parcialmente traduzidas.

## Usando as traduções

A API de runtime fica na crate `graphite-i18n`:

```rust
use graphite_i18n::{Language, Translator};

let translator = Translator::new(Language::parse("pt-BR").unwrap());
assert_eq!(translator.text("common.cancel"), "Cancelar");

let message = translator.format(
    "document.open_failed",
    &[("path", &path.display()), ("error", &error)],
);

let title = translator.text_or("command.myPlugin.command", "My command");
```

Pontos importantes:

- `Translator` é `Copy`. Threads de trabalho capturam o valor por cópia e o
  tomam emprestado localmente, em vez de compartilhar uma referência entre
  threads.
- `text` recorre ao inglês e depois à própria chave; `text_or` aceita um
  fallback explícito de metadados em inglês; `has` e `keys` auxiliam
  ferramentas e testes.
- Títulos e categorias de comandos são resolvidos por chaves estáveis
  (`command.<id>` e `command.category.<lowercase>`), com os metadados em
  inglês como fallback. Comandos de plugins sem catálogo continuam exibindo os
  títulos declarados.
- Diagnósticos de configuração recebem `&Translator` na camada de configuração.
  O status de saída do terminal armazena o código numérico e é localizado no
  momento da desenho.
- Nos testes, construa o tradutor com `Translator::english()` para manter as
  asserções determinísticas. `App::set_interface_language` troca o idioma da
  interface em testes que verificam a saída em português.

## Escopo da localização

As superfícies localizadas incluem a moldura da janela, a barra de status,
overlays do editor e de busca, painéis, entradas da paleta de comandos,
diagnósticos de atalhos e configurações, mensagens de controle de
código-fonte, status e correções rápidas de servidores de linguagem,
diagnósticos de JSON schema, interface do terminal, status de plugins e do
host de extensões, rótulos de acessibilidade e erros de CLI/ciclo de vida.

Textos técnicos fornecidos por ferramentas externas permanecem inalterados:
stderr do sistema operacional, Git e processos, mensagens de resposta de
servidores de linguagem e payloads de erro do protocolo de plugins trocados com
hosts de extensão. Essas strings são detalhes de API; os rótulos, status e
invólucros de falha ao redor são localizados.

## Adicionando um idioma

1. Adicione uma entrada em `locales/languages.json`.
2. Crie `locales/<código>/` com os mesmos oito arquivos de catálogo.
3. Traduza apenas os valores; mantenha chaves, nomes de placeholders e chaves
   escapadas inalterados.
4. Execute `cargo test -p graphite-i18n` e `cargo build --workspace`.
5. Atualize a tabela de idiomas em `locales/README.md` e o índice de
   documentação.

A compilação falha até que o novo idioma defina todas as chaves do inglês com
placeholders idênticos.

## Verificação

- `cargo test -p graphite-i18n` verifica paridade dos catálogos, fallback,
  interpolação e análise de etiquetas de idioma.
- `cargo test -p graphite-app` exercita a interface localizada com o inglês
  como padrão e inclui testes que trocam o idioma da interface em casos
  individuais.
- `cargo test --workspace` e `cargo build --workspace` validam o workspace
  completo na plataforma atual.
