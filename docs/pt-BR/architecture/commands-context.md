# Comandos e contexto

## Registro e ponto de entrada

`app/command_registry.rs` mantém as descrições dos comandos: ID estável, título em inglês, categoria, tipos de alvo aceitos, esquema de argumentos, enablement, visibilidade opcional de menu e handler. IDs duplicados e condições malformadas são rejeitados. Os handlers por domínio ficam em `app/command_handlers/`; `app/commands.rs` apenas adapta atalhos e contém suas regressões. Os 106 IDs do dispatcher anterior e todos os atalhos padrão estão registrados. Não existe fallback legado.

`App::execute_command` recebe um `CommandRequest` serializável e retorna um ticket com ID de execução, token de cancelamento e receptor de resultado sem bloqueio. Omitir o alvo significa usar o contexto global. Adaptadores da interface informam explicitamente uma view ou instância de widget. Exemplo:

```json
{"id":"type","arguments":{"text":"hello"},"target":{"kind":"view","id":42}}
```

Os resultados são valores JSON, com `null` para comandos sem dados de retorno. Consultas de linguagem retornam itens de completion, localização, texto de hover ou code actions; comandos do servidor retornam seu resultado JSON. Os erros distinguem comando desconhecido, argumentos inválidos, alvo inválido ou encerrado, comando desabilitado, cancelamento, falha de execução e documento obsoleto. O executor valida cancelamento, alvo, argumentos e enablement antes de chamar um handler ou dispensar sugestões. Atalhos personalizados preservam seus `args` JSON durante a resolução.

Handlers de edição recebem por empréstimo as seleções da view alvo explícita e usam as transações descritas em [Documento e view](document-view.md). A execução não troca o foco temporariamente. Comandos cuja finalidade é navegar, abrir um editor ou focar um grupo podem mudar o foco. Undo/redo mantêm o foco atual e as regras existentes de origem do histórico.

## Escopos de contexto

A resolução parte da instância de widget, passa por sua view proprietária e chega aos valores globais; widgets globais herdam diretamente do escopo global. Chaves locais prevalecem sobre ancestrais. IDs de widget identificam uma aparição; fechar e reabrir cria outro ID. Views e widgets encerrados perdem seus escopos e trabalhos pendentes. O contexto é atualizado nos limites dos eventos, antes da resolução de atalhos e antes e depois da execução, inclusive em chamadas programáticas sem entrada de teclado.

Os valores continuam sendo strings compatíveis com o avaliador existente. As chaves descrevem foco, disponibilidade do documento, caminho, permissões, seleções, undo/redo, visibilidade e conteúdo de widgets, comandos pendentes e carregamento de diff. O enablement de edição usa as permissões do documento alvo independentemente do foco físico. Comandos de widgets validam a instância e seu tipo.

Três condições continuam independentes: `when` seleciona atalhos elegíveis, `menu_when` controla visibilidade de menu e enablement autoriza a execução. O predicado de menu é uma API para consumidores; esta mudança não adiciona paleta de comandos nem interface de menus. Itens ocultos e atalhos inelegíveis não impedem uma chamada programática habilitada.

## Execuções pendentes e workers

Execuções pendentes são identificadas por ID. Workers recebem snapshots próprios e tokens de cancelamento cooperativo e enviam resultados ao event loop, único responsável por aplicar alterações em documentos ou interface. Gerações de consultas de busca também rejeitam resultados substituídos. Abrir um widget de busca ou snippets conclui quando ele é aberto; um comando que solicita consulta, operação Git, diff ou resposta LSP conclui quando esse trabalho retorna. Comandos de fechamento com alterações permanecem pendentes enquanto a confirmação existente de salvar/descartar/cancelar é exibida.

Antes de aplicar um resultado, o event loop verifica novamente cancelamento, existência do alvo, workspace e revisões relevantes. Checkout também valida o snapshot de todos os documentos abertos, preservando texto digitado durante a operação Git. Encerrar um alvo cancela seus trabalhos; respostas tardias não podem reabri-lo. Cancelamento não desfaz efeitos já confirmados em memória, no sistema de arquivos ou no Git. Chamadas nativas em andamento podem terminar antes de o worker observar o cancelamento. Execuções concluídas liberam seus registros pendentes e canais de resultado.

Trabalhos LSP correlacionam IDs de execução com IDs de requisição do protocolo. Cancelamento envia `$/cancelRequest`; respostas inválidas, transportes desconectados e encerramento do servidor concluem os tickets com erro. Quick fixes com comandos do servidor permanecem em `workspace/executeCommand`, incluindo seu resultado final. IDs locais desconhecidos nunca são encaminhados automaticamente. O LSP mantém seu transporte e ciclo de vida separados.

## Validação e limites

Testes nativos fixam o inventário anterior de comandos e verificam atalhos padrão, metadados, duplicidades, argumentos/resultados JSON, erros, precedência e limpeza de escopos, predicados independentes, alvos sem foco, rejeição somente leitura sem efeitos, regressões de transações, confirmações, workers simulados e respostas/cancelamento/encerramento de LSP simulado. Os testes usam dados isolados sem servidores externos.

Esta etapa preserva o salvamento síncrono. Interfaces internas de serviços e extension host pertencem ao item 3; coordenação e participantes de salvamento assíncrono pertencem ao item 4. Testes e build nativos no Windows são exigidos nesta implementação; execução no Linux e verificações interativas de UI/GPU não foram realizadas neste ambiente Windows.
