# Gramáticas TextMate de extensões

O Graphite realça muitas linguagens com gramáticas tree-sitter embutidas. Para qualquer outro arquivo, uma extensão habilitada pode fornecer uma **gramática TextMate** (`contributes.grammars`), e o Graphite colore o arquivo com ela.

## Como um arquivo recebe uma gramática

1. A extensão declara a linguagem em `contributes.languages`: um `id` e as terminações de nome de arquivo (`extensions`, como `.toy`) ou nomes inteiros (`filenames`, como `Toyfile`) que ela cobre. A comparação ignora maiúsculas e minúsculas, e a terminação mais longa vence (`.d.ts` vence `.ts`).
2. `contributes.grammars` indica a gramática dessa linguagem (`language`, `scopeName`, `path`). Uma gramática sem `language` não é usada para arquivos próprios; outras gramáticas podem incluí-la.
3. Arquivos que o tree-sitter já conhece continuam com o tree-sitter. A gramática só é usada quando não há uma embutida.

Os arquivos de gramática podem ser `.tmLanguage.json` ou listas de propriedades XML (`.tmLanguage`, `.plist`). Um arquivo de gramática precisa estar dentro da pasta da extensão.

## O que o motor suporta

- Regras `match`, regras `begin`/`end` (incluindo `applyEndPatternLast`) e regras `begin`/`while`, com `name` e `contentName`.
- Grupos de captura com escopo próprio (`captures`, `beginCaptures`, `endCaptures`, `whileCaptures`), inclusive grupos tokenizados por `patterns` próprios.
- Padrões `end` que usam o texto do begin (`\1`).
- `include` da própria gramática (`$self`), da gramática do documento (`$base`), de uma regra do repositório (`#nome`) e de outras gramáticas (`source.outra`, `source.outra#regra`). As gramáticas incluídas são carregadas da extensão que as contribui.
- `\G`, e as classes `\h` e `\H` do Oniguruma. Um padrão que o motor de expressões regulares não consegue compilar é ignorado; o resto da gramática continua funcionando.
- O realce é incremental: depois de uma edição, só as linhas até o ponto em que as regras abertas voltam a ser as de antes são tokenizadas de novo.

Não suportados: injeções (`injections`, `injectionSelector`), `embeddedLanguages`, `tokenTypes` e recursos do Oniguruma que o motor não tem (por exemplo alguns nomes de propriedade como `\p{Alpha}` e lookbehind de tamanho variável).

## Cores

Os escopos de um token são mapeados para as classes que os temas colorem: o escopo mais interno que tem uma classe decide, então as aspas de uma string recebem a cor da string e escopos `meta` simples passam a escolha para fora.

| Prefixo de escopo | Classe |
|---|---|
| `comment` | comentário |
| `string`, `string.regexp`, `markup.italic`, `markup.raw` | string |
| `constant.numeric` | número |
| `constant`, `variable.language`, `support.constant` | constante |
| `keyword`, `storage`, `markup.heading` | palavra-chave |
| `keyword.operator`, `markup.list` | operador |
| `entity.name.function`, `support.function`, `meta.function-call` | função |
| `entity.name.type`, `entity.name.class`, `support.type`, `entity.name.tag` | tipo |
| `variable.parameter`, `variable` | variável |
| `variable.other.property`, `support.type.property-name`, `entity.other.attribute-name` | propriedade |

As cores são as do tema de cores ativo, inclusive temas vindos de extensões.
