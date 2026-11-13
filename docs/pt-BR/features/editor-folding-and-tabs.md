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

### Mais comandos de dobra

As regiões são aninhadas: uma região dentro de outra está um nível abaixo. Além da indentação, uma região pode vir de marcadores `#region` / `#endregion` depois de um símbolo de comentário (`// #region`, `# region`, `<!-- #region`), que funcionam também onde a indentação não muda.

| Comando | Atalho | O que faz |
| --- | --- | --- |
| Alternar dobra (`editor.toggleFold`) | nenhum | Dobra a região sob o cursor, ou a desdobra se já estiver dobrada. |
| Dobrar recursivamente / Desdobrar recursivamente | `Ctrl+K Ctrl+[` / `Ctrl+K Ctrl+]` | Dobra ou desdobra a região sob o cursor e todas as regiões dentro dela. |
| Alternar dobra recursivamente | `Ctrl+K Ctrl+Shift+L` | Inverte a região sob o cursor e põe tudo dentro dela no novo estado. |
| Dobrar nível 1 a 7 | `Ctrl+K Ctrl+1` a `7` | Dobra todas as regiões dessa profundidade, menos as que contêm um cursor. |
| Dobrar / Desdobrar todas as regiões exceto as selecionadas | `Ctrl+K Ctrl+-` / `Ctrl+K Ctrl+=` | Vale para todas as regiões, menos a do cursor e as regiões ao redor ou dentro dela. |
| Dobrar todos os comentários de bloco | `Ctrl+K Ctrl+/` | Dobra toda região que começa com o símbolo de comentário de bloco da linguagem. |
| Dobrar todas as regiões / Desdobrar todas as regiões | `Ctrl+K Ctrl+8` / `Ctrl+K Ctrl+9` | Dobra ou desdobra toda região que começa numa linha de marcador `#region`. |
| Ir para o próximo / anterior intervalo de dobra | nenhum | Na primeira linha de uma região, vai para a próxima ou anterior do mesmo nível dentro do mesmo pai. Em outras linhas, vai para o início de região mais próximo abaixo ou acima. |
| Ir para a dobra pai | nenhum | Vai para a primeira linha da região ao redor do cursor, ou da região pai se já estiver numa primeira linha. |
| Criar intervalo de dobra a partir da seleção | `Ctrl+K Ctrl+,` | Cria uma região com as linhas selecionadas e a dobra. |
| Remover intervalos de dobra manuais | `Ctrl+K Ctrl+.` | Remove os intervalos manuais sob o cursor ou tocados pela seleção; um cursor fora de qualquer intervalo manual remove todos. |
| Alternar dobra de imports | nenhum | Dobra ou desdobra o primeiro bloco de duas ou mais linhas `import` / `use` / `#include`. |

Intervalos feitos à mão e a dobra de imports duram até o documento mudar, como as próprias dobras. `Ctrl+K Ctrl+L` continua em Desdobrar.

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
