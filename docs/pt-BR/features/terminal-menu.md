# Menu Terminal

O menu **Terminal** da barra de título reúne os comandos do terminal integrado e das tarefas. A navegação pelo teclado (setas, Enter e Escape) usa as mesmas linhas, estados de habilitação e nós de acessibilidade da entrada por ponteiro.

## Sessões de terminal

**Novo Terminal** abre uma sessão. **Dividir Terminal** mostra dois terminais lado a lado e fica habilitado quando já existe um terminal. **Nova Janela do Terminal** aparece na lista, mas fica desabilitado até que terminais possam abrir em uma janela separada.

## Executando código

**Executar Arquivo Ativo** digita o caminho entre aspas do arquivo ativo no terminal em foco, criando uma sessão se não houver nenhuma. No Windows, a entrada usa o operador de chamada `&` do PowerShell. Nos outros sistemas, o shell recebe o caminho entre aspas, então o arquivo precisa ser executável (ou começar com um shebang) para rodar. Documentos sem título não têm caminho e desabilitam esta linha.

**Executar Texto Selecionado** envia o texto selecionado ao terminal em foco, uma linha por vez. Fica desabilitado quando nada está selecionado.

**Executar Tarefa de Compilação** roda a tarefa de compilação padrão de `.vscode/tasks.json`, a mesma ação de **Ctrl+Shift+B**. **Executar Tarefa** abre a paleta de comandos filtrada para as tarefas desse arquivo. As tarefas só rodam em workspaces confiáveis.

## Controle de tarefas

Estas linhas ficam habilitadas somente enquanto uma tarefa está em execução:

- **Mostrar Tarefas em Execução** exibe o painel do terminal.
- **Reiniciar Tarefa em Execução** cancela a tarefa e a inicia novamente depois que ela para.
- **Encerrar Tarefa** interrompe a tarefa.

## Configuração de tarefas

**Configurar Tarefas** e **Configurar Tarefa de Compilação Padrão** abrem `.vscode/tasks.json`, criando o arquivo com uma lista de tarefas vazia quando ele não existe. A tarefa de compilação padrão é definida editando esse arquivo; ainda não há um seletor.
