# Comandos de linha e de caixa

Comandos de texto que reescrevem linhas inteiras ou a caixa das palavras. Todos se desfazem em um passo, exigem um editor gravável e estão na paleta de comandos.

## Linhas

| Comando | Efeito |
| --- | --- |
| **Recuar linha** (`Ctrl+]`) | Leva cada linha selecionada ao próximo ponto de recuo, usando o tamanho de tabulação e a opção espaços/tabs do documento. Linhas vazias são puladas quando várias linhas estão selecionadas. Um cursor em linha em branco termina depois do novo recuo. |
| **Remover recuo da linha** (`Ctrl+[`) | Leva cada linha selecionada ao ponto de recuo anterior. Linhas sem espaço no início ficam como estão. |
| **Inverter linhas** | Inverte a ordem das linhas selecionadas. Com um cursor só, inverte o documento inteiro, sem a linha vazia depois da última quebra de linha. Cada cursor mantém a coluna e acompanha sua linha. |
| **Ordenar linhas em ordem decrescente** | Ordena as linhas selecionadas em ordem decrescente. Com um cursor só, ordena o documento inteiro. |
| **Sort lines ascending** | Igual, em ordem crescente. Um cursor só agora também ordena o documento inteiro. |
| **Duplicar seleção** | Copia cada seleção logo depois dela e seleciona a cópia; um cursor sem seleção duplica a linha para baixo. |
| **Transpor caracteres ao redor do cursor** | Troca os caracteres antes e depois do cursor. No fim de uma linha, troca o último caractere com a quebra de linha, levando esse caractere para o início da próxima. Não faz nada no início de uma linha nem no fim do texto. |

Os separadores de linha (`\n` ou `\r\n`) ficam onde estavam; só o texto das linhas se move.

## Caixa

Estes comandos agem em cada seleção, ou na palavra sob o cursor quando nada está selecionado. Uma palavra tem letras, dígitos e sublinhados.

| Comando | Exemplo |
| --- | --- |
| **Transformar em maiúsculas** | `Straße` → `STRASSE` |
| **Transformar em minúsculas** | `HeLLo` → `hello` |
| **Transformar em título (Title Case)** | `don't stop` → `Don't Stop` |
| **Transformar em snake_case** | `parseHTTPResponse` → `parse_http_response` |
| **Transformar em camelCase** | `hello_world-foo bar` → `helloWorldFooBar` |
| **Transformar em PascalCase** | `HELLO_WORLD` → `HelloWorld` |
| **Transformar em kebab-case** | `parseHTTPResponse` → `parse-http-response` |

Detalhes:

- O camelCase mantém os espaços quando a seleção ocupa várias linhas e não mexe em uma sigla no início (`HTTP server` → `HTTPServer`).
- O PascalCase mantém o `.` onde está e põe em minúsculas o resto de palavras todas em maiúsculas.
- O kebab-case troca um sublinhado entre dois caracteres não vazios por hífen. As trocas não se sobrepõem, então em `a_b_c` só o primeiro sublinhado muda (`a-b_c`).
- Dois cursores na mesma palavra mudam a palavra uma vez. Um cursor que não toca palavra alguma não faz nada.
