# Comandos do histórico local

O Graphite guarda no perfil uma cópia do arquivo a cada vez que ele é salvo (até 50 versões por arquivo, ignorando saves que não mudam nada). Estes comandos trabalham com essas versões salvas do arquivo ativo.

| Comando | O que faz |
| --- | --- |
| Procurar entrada do histórico local para abrir | Lista as versões, da mais nova à mais antiga, com data e tamanho. Escolher uma abre em modo somente leitura. |
| Procurar entrada do histórico local para restaurar | Lista as versões; escolher uma troca o texto do arquivo por ela. A troca é uma edição só, então Desfazer traz seu texto de volta. |
| Comparar entrada do histórico local com o arquivo | Lista as versões; escolher uma abre um diff entre ela e o arquivo. |
| Comparar o arquivo com a última entrada do histórico local | Abre um diff entre o arquivo e a versão salva mais recente. |
| Criar entrada do histórico local | Salva uma versão agora. Avisa quando o arquivo não mudou desde a última versão. |
| Excluir entrada do histórico local | Lista as versões; escolher uma a exclui. |
| Excluir todo o histórico local do arquivo | Exclui todas as versões salvas do arquivo. |
| Restaurar a entrada do histórico local aberta | Com uma versão salva aberta no editor, põe o texto dela no arquivo original. |

As entradas são nomeadas pela data, então renomear uma entrada, escolher uma entrada para comparar com outra e mostrar a pasta no gerenciador de arquivos do sistema não estão disponíveis. O histórico só é guardado em perfis; uma sessão sem perfil não salva nada.
