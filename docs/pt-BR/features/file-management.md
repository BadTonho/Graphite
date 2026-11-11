# Gestão de arquivos e workspace

## Explorador

- **Selecione** uma entrada clicando nela. A seleção é o alvo das teclas abaixo. Clicar no editor devolve o foco do teclado ao editor.
- **Renomear** (`F2`) abre um campo com o nome atual. Um nome com pastas move a entrada, por exemplo `src/lib/antigo.rs`. Caminhos absolutos e `..` são recusados, então a entrada continua dentro do workspace.
- **Mover arrastando e soltando**: pressione uma entrada e solte-a sobre a linha de uma pasta para movê-la para dentro dela. Soltar sobre um arquivo move a entrada para a pasta desse arquivo. Uma pasta não pode ser movida para dentro de si mesma, e nomes existentes nunca são sobrescritos.
- **Excluir** (`Delete`) pede confirmação e depois remove o arquivo ou a pasta. A exclusão é permanente; o Graphite não usa a Lixeira.
- Documentos abertos acompanham renomeações e movimentos. Uma entrada com edições não salvas não pode ser movida ou excluída até que as edições sejam salvas ou descartadas.
- **Aninhamento de arquivos**: arquivos companheiros aparecem recuados sob o arquivo principal, por exemplo `app.js` e `app.d.ts` sob `app.ts`, ou `package-lock.json` sob `package.json`. As regras são internas (veja `src/file_nesting.rs`) e estão sempre ativas.

## Alterações externas

Quando o observador de arquivos informa uma mudança no workspace, os arquivos abertos são comparados com o disco:

- Um documento limpo que mudou no disco é recarregado, e a barra de status informa quantos arquivos foram recarregados.
- Um documento com edições não salvas mantém essas edições, e a barra de status informa o conflito. Salve ou descarte para resolver.
- Arquivos gravados no último segundo são verificados de novo depois que se estabilizam, para não ler um salvamento que ainda está sendo escrito.

## Busca no projeto

O painel de busca (`Ctrl + Shift + F`) tem estas opções:

| Opção | Tecla |
|---|---|
| Expressão regular | `Alt + R` |
| Diferenciar maiúsculas | `Alt + C` |
| Palavra inteira | `Alt + W` |
| Mostrar campos de inclusão/exclusão | `Ctrl + Shift + J` |

Com os campos visíveis, `Tab` alterna entre a consulta, a substituição (no modo de substituir), **arquivos a incluir** e **arquivos a excluir**. As listas de globs são separadas por vírgula, por exemplo `src/**, *.rs` e `target/**`. Globs de inclusão limitam a busca aos arquivos correspondentes; globs de exclusão ignoram os correspondentes. Uma expressão regular ou glob inválida é informada na barra de status e limpa os resultados.

Substituir tudo no modo de expressão regular insere o texto de substituição literalmente; referências de captura como `$1` ainda não são expandidas.

## Salvamento automático

`files.autoSave` no `settings.json` define quando documentos alterados são salvos:

| Valor | Quando |
|---|---|
| `off` | Nunca. |
| `afterDelay` | Após `files.autoSaveDelay` milissegundos sem edições (padrão `1000`). |
| `onFocusChange` | Quando o foco sai de um documento para outro. |
| `onWindowChange` | Quando a janela do Graphite perde o foco. |

Se `files.autoSave` não estiver definido, vale a opção do menu Arquivo: ligada significa `afterDelay`, desligada significa `off`. Documentos sem caminho de arquivo e documentos somente leitura nunca são salvos automaticamente.

## Histórico local

Cada salvamento registra o texto do arquivo no perfil (pasta `history/`), a menos que ele seja idêntico à versão mais recente. São mantidas até 50 versões por arquivo, e as mais antigas são removidas primeiro.

Execute **Abrir histórico local** pela paleta de comandos para listar as versões do arquivo ativo, da mais nova para a mais antiga, com data (UTC) e número de linhas. Ao escolher uma, ela abre somente leitura em uma nova aba, para que você possa copiar o texto de volta. Sessões isoladas não gravam histórico.

O histórico local não substitui o Git: ele registra o que foi salvo pelo editor, não commits.
