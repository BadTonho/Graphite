# Dobra de código e abas de pré-visualização/fixadas

## Dobra de código

O Graphite dobra código pela indentação. Uma linha abre uma dobra quando a próxima linha não vazia tem indentação maior que ela. A dobra cobre todas as linhas seguintes que estão vazias ou indentadas mais fundo, e termina na última linha não vazia desse bloco. A dobra não depende da linguagem e funciona em qualquer arquivo, mas pode diferir de uma dobra baseada em sintaxe em layouts incomuns.

| Comando | Atalho padrão | O que faz |
|---|---|---|
| Dobrar (`editor.fold`) | `Ctrl+Shift+[` | Dobra o bloco mais interno em volta do cursor. Se esse bloco já estiver dobrado, dobra o bloco que o envolve. O cursor vai para a primeira linha da dobra. |
| Desdobrar (`editor.unfold`) | `Ctrl+K Ctrl+L` | Desdobra a dobra que começa na linha do cursor, ou a dobra que a esconde. |
| Dobrar tudo (`editor.foldAll`) | `Ctrl+K Ctrl+0` | Dobra todos os blocos de nível superior. |
| Desdobrar tudo (`editor.unfoldAll`) | `Ctrl+K Ctrl+J` | Remove todas as dobras da view. |

`Ctrl+Shift+]` já está associado a "ir para colchete", por isso desdobrar usa `Ctrl+K Ctrl+L`. Os atalhos podem ser alterados em `keybindings.json`.

Uma linha dobrada mostra ` …` depois do texto. As dobras são guardadas por view, então o mesmo documento pode estar dobrado de forma diferente em dois grupos.

Cada aba tem um botão de fechar (×) na extremidade direita. Clique nele para fechar aquela aba, mesmo que ela não seja a ativa. Uma aba com alterações não salvas continua pedindo confirmação antes de fechar.

Limitações nesta versão:

- As dobras são apagadas quando o documento muda. Elas não são mantidas entre edições.
- A dobra não está disponível com quebra de linha ativa. Os comandos de dobra mostram uma mensagem na barra de status nesse caso, e as dobras existentes não são desenhadas no modo com quebra de linha.
- Ainda não há um chevron clicável na gutter; a dobra é feita pelo teclado.
- Se a navegação ou a busca levar o cursor para dentro de um bloco dobrado, o bloco é desdobrado para o cursor continuar visível.

## Abas de pré-visualização e fixadas

**Abas de pré-visualização.** Abrir um arquivo pelo explorador (clique simples) ou pelo Abertura rápida (`Ctrl+P`) mostra o arquivo em uma aba de pré-visualização. Essa aba aparece com o texto em cor mais suave. Abrir outro arquivo por esses caminhos substitui a aba de pré-visualização em vez de criar uma nova. Editar o documento torna a aba permanente.

**Abas fixadas.** O comando *Fixar ou desafixar aba* (`editor.action.togglePinTab`) fixa a aba ativa. Uma aba fixada é marcada com uma linha azul na borda superior, continua aberta quando uma pré-visualização abre, e é movida para antes das abas não fixadas do seu grupo. Fixar uma aba de pré-visualização a torna permanente. O comando não tem atalho padrão; associe um em `keybindings.json` se quiser.

Limitações nesta versão:

- Arquivos abertos pela linha de comando, por resultados de busca, pela lista de problemas ou por comandos continuam como abas permanentes.
- O texto de pré-visualização fica em cor mais suave, e não em itálico, porque a fonte embutida não tem estilo itálico.
- Clicar duas vezes numa aba ainda não a fixa.
