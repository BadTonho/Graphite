# Comandos de tarefas

As tarefas são definidas em `.vscode/tasks.json` do espaço de trabalho e exigem que ele seja confiável. Estes comandos complementam Executar tarefa de build e Encerrar tarefa.

| Comando | O que faz |
| --- | --- |
| Executar tarefa... | Abre a paleta de comandos filtrada para as tarefas do espaço de trabalho. |
| Executar a tarefa de teste / de limpeza / de recompilação | Executa a tarefa padrão desse grupo: a tarefa cujo `group` tem esse nome, ou a marcada com `isDefault`. Indisponível quando o arquivo não tem nenhuma. |
| Executar de novo a última tarefa | Executa outra vez a tarefa que rodou por último nesta sessão. |
| Mostrar tarefas em execução | Mostra o painel quando uma tarefa está rodando; indisponível caso contrário. |
| Reiniciar a tarefa em execução | Para a tarefa em execução e a inicia de novo. |
| Encerrar tarefa | Para a tarefa em execução. |
| Mostrar o log das tarefas | Mostra o painel com a saída da última tarefa. |
| Alternar problemas no painel | Alterna o painel entre a saída das tarefas e a lista de problemas. |
| Configurar o executor de tarefas, Abrir tarefas do espaço de trabalho, Configurar a tarefa de build padrão, Configurar a tarefa de teste padrão | Abrem `.vscode/tasks.json`, criando-o quando não existe. |

Tarefas do usuário (fora de um espaço de trabalho) e a escolha de qual tarefa é a padrão não têm diálogo; edite o `group` da tarefa no arquivo.
