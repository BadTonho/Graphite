# Navegação entre alterações e comandos da caixa de commit

| Comando | O que faz |
| --- | --- |
| Ir para a próxima alteração / Ir para a alteração anterior (e `Próxima alteração`, `Alteração anterior`, e os ids da gutter de diff) | Move o cursor para a primeira linha do próximo ou do anterior bloco alterado do arquivo ativo, como mostram as marcas coloridas da gutter. Linhas alteradas consecutivas contam como um bloco só. No último (ou primeiro) bloco, dá a volta até a outra ponta e avisa na barra de status. Um arquivo sem alterações avisa e não se move. |
| Fazer commit das alterações preparadas (`scm.acceptInput`) | Faz o commit do que está preparado com a mensagem da caixa de commit. Mensagem vazia é recusada. |
| Limpar a mensagem de commit | Esvazia a caixa de commit. |
| Abrir o repositório no terminal | Abre um novo terminal na pasta do repositório do arquivo ativo (a pasta do espaço de trabalho quando o arquivo está fora de qualquer repositório). |

As linhas alteradas vêm da última atualização do controle de código-fonte, então acompanham o que a gutter mostra.
