# Salvamento assíncrono

O salvamento é coordenado por `DocumentId`. Cada documento executa um ciclo por vez; documentos diferentes gravam simultaneamente. Pedidos recebidos durante um ciclo compartilham um ciclo seguinte, com tickets de conclusão e cancelamento separados. Uma retenção mantém o documento até a conclusão e reconciliação do worker. Workers possuem snapshots e dados de transporte; somente o event loop altera documentos, histórico, views e referências salvas.

## Preparação e participantes

O coordenador captura versão de origem, texto, caminho, codificação e fim de linha. Os participantes executam sequencialmente: primeiro a instância LSP atual, depois os participantes compatíveis do host simulado na ordem de registro dos recursos e, por último, a remoção opcional de espaços ao final das linhas. Cada participante tem prazo de um segundo, verificado mesmo sem entrada do teclado. Falhas e timeouts aparecem como avisos; não impedem a gravação.

Participantes editam um snapshot preparado, nunca o buffer atual. Registros do host usam `RegisterSaveParticipant`, com ID único e filtro opcional de linguagem; um filtro vazio aceita todas as linguagens. Os identificadores pertencem à sessão e aceitam `Dispose`. `ParticipateInSave` carrega recurso, invocação, motivo e snapshot; `SaveParticipantResult` deve repetir a invocação e a versão real de origem. A invocação distingue o texto preparado da entrada de outro participante sem inventar versões de documento. Chamadas de serviço vinculadas ao pai permitem apenas consultas. Edições independentes continuam usando transações normais.

Todos os intervalos UTF-16 recebidos são validados juntos, incluindo limites de pares substitutos, ordenação e sobreposição. Resultados inválidos, obsoletos, liberados ou de sessões/instâncias antigas são ignorados com aviso. Se texto ou formato atuais mudarem durante um participante, a preparação dele é descartada, o snapshot é atualizado e os participantes restantes continuam uma vez. Não há repetição automática de participantes.

O LSP anuncia capacidades de sincronização de salvamento e respeita as opções `willSave`, `willSaveWaitUntil`, `save` e `includeText` do servidor. A expiração envia `$/cancelRequest`; respostas tardias não podem aplicar edições. Gravações bem-sucedidas enviam `didSave` com o texto efetivamente gravado quando solicitado, mesmo que exista texto atual mais recente. Falhas não enviam `didSave`.

## Gravação e reconciliação

A codificação e a escrita no sistema de arquivos ocorrem em um worker. Links existentes são resolvidos para seus alvos; arquivos são gravados por um temporário no mesmo diretório, sincronizados e substituídos. Permissões e comportamento de formato existentes são preservados. Um controle atômico permite cancelamento antes da substituição; após o worker assumir o commit, seu resultado precisa ser reconciliado mesmo que a view, o ticket ou o host solicitante tenham encerrado. Temporários são removidos em falha ou cancelamento.

Após sucesso, as edições preparadas e o trim tornam-se uma única transação com undo somente se versão e formato de origem ainda coincidirem e o documento continuar editável. Trechos inalterados preservam seus mapeamentos de posição; seleções das demais views, histórico da origem e foco seguem as regras normais de transação. Preparações sem mudança preservam versão, undo e redo.

A referência salva sempre passa a ser o texto e formato efetivamente gravados. Digitação e alterações de formato concorrentes permanecem intactas e dirty quando diferem dessa referência. Uma falha de escrita não aplica a preparação nem altera histórico ou referência salva. Cancelamento nunca desfaz um commit confirmado.

## Comandos, serviços e decisões

`workbench.action.files.save` e `workbench.action.files.saveAll` concluem após as gravações solicitadas e retornam JSON `null`. Salvar tudo informa falhas por documento e mantém as gravações bem-sucedidas. Os atalhos existentes são preservados; nenhum atalho padrão de salvar tudo é acrescentado. O contexto da view expõe `documentSaving` sem impedir salvamentos enfileirados.

`SaveService::save(SaveRequest, CancellationToken)` aceita a URI de um documento em memória e retorna um ticket cujo resultado JSON é `SaveOutcome`: identidade do documento, versão de origem, estado dirty na reconciliação e avisos. `ServiceRequest::Save` expõe essa operação ao host simulado. Documentos untitled sem caminho falham sem fechar; Save As permanece fora desta alteração.

Salvar antes de fechar aba/janela ou trocar de workspace permanece pendente sem bloquear a entrada. Falhas, edições posteriores e gravações adicionais pendentes mantêm o documento ou janela abertos e preservam o workspace atual. Não há repetição automática até a digitação parar. Descartar cancela trabalhos anteriores ao commit e aguarda gravações já assumidas; edições feitas após a decisão de descarte cancelam a transição.

Trocas de workspace oferecem salvar/descartar/cancelar e preservam abas de arquivos e foco. Descartar prepara todas as recargas antes de alterar qualquer buffer, recarrega arquivos alterados do disco e fecha views untitled alteradas. O workspace alvo é preparado antes do descarte. Instâncias LSP são desvinculadas somente após a aceitação, aguardadas fora do event loop e recriadas para documentos mantidos. A continuação CLI e a confirmação de `--wait` são mantidas; cancelar uma decisão confirma seu tratamento em vez de provocar outra janela.

O protocolo simulado está na versão 2. Confirmações do usuário suspendem os prazos RPC comuns; `Resumed` restabelece um novo prazo de 30 segundos após a decisão. Prazos de participantes permanecem em um segundo, independentemente da suspensão RPC.

## Validação e limites

Os testes usam arquivos temporários, relógios controlados e canais que pausam workers antes ou depois de assumir o commit. Cobrem filas, gravações independentes, cancelamento, dirty/histórico/formato, mapeamento de views, barreiras de decisões, continuação CLI, resultados inválidos/obsoletos e quedas reais do host simulado. Testes de participantes LSP usam canais simulados com dados próprios; testes adicionais de worker usam uma fixture Rust stdio local. Nenhum exige servidor externo. Consulte a [matriz de cobertura de regressão](../contributing/testing.md).

Compile o auxiliar antes de testes de processo específicos: `cargo build -p graphite-extension-host`. Seus cenários `--save-edit`, `--save-wait`, `--save-crash`, `--save-stale`, `--save-invalid`, `--save-mutate` e `--save-check-lsp` são fixtures de teste, não opções do editor. Execute `cargo test --workspace` e `cargo build --workspace` para validação completa. Execução no Linux e verificação interativa de UI/GPU no Windows/Linux não foram realizadas neste ambiente Windows.

A atomicidade é por documento, não entre todos os salvamentos ou recursos do sistema de arquivos. O host Code OSS/Node real, versão fixada, teste de extensão real e compatibilidade `.vsix` permanecem como trabalho posterior obrigatório.
