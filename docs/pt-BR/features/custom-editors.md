# Visualizações personalizadas somente leitura

Alguns arquivos não podem ser editados como texto simples. O Graphite os abre em uma **visualização somente leitura** em vez de falhar:

| Arquivo | Visualização |
|---|---|
| Binário, ou texto que não é UTF-8 | Um dump hexadecimal: deslocamento, 16 bytes em hexadecimal e uma coluna ASCII. Arquivos acima de 1 MiB mostram o primeiro 1 MiB e informam quantos bytes ficaram de fora. |
| Notebook Jupyter (`.ipynb`) | As células em ordem. Cada célula tem um cabeçalho (`---- cell N (code) ----`), seu código e suas saídas (texto de stream, resultados `text/plain` e erros). |

As visualizações são documentos somente leitura. Dá para selecionar, pesquisar e copiar, mas não salvar, então o arquivo original nunca é sobrescrito com o texto renderizado. Arquivos alterados no disco não são recarregados dentro de uma visualização, porque o texto dela não é o conteúdo do arquivo.

Notebooks são exibidos apenas em modo leitura. Editar células, executá-las e saídas ricas (imagens, HTML) ainda não são suportados.
