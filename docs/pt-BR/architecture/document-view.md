# Documento e view

## Propriedade e identidade

`Document` mantém um `shoppe-text::Buffer`, linguagem, codificação, fim de linha, indentação e caches derivados do documento. O buffer mantém o Rope, revisão, referência do texto salvo e histórico compartilhado de undo/redo. Ele não tem cursor ou seleção ativa.

`DocumentStore` mantém documentos sob chaves `DocumentId`. As chaves são separadas dos dados mutáveis do documento; substituir os dados não invalida a busca por identidade. Acesso e remoção usam IDs; posições na coleção não são identidades de documentos. Os grupos mantêm a ordem das abas. O documento ativo sempre vem da view em foco.

Cada `View` mantém um `ViewId` estável, `SelectionState`, scroll, composição IME e estado de navegação da seleção sintática. Dividir copia seleções e scroll para uma nova identidade de view e inicia um novo estado transitório de entrada. Trocas de foco não copiam estado de apresentação para o documento. Desenho, acessibilidade, navegação, diagnósticos e LSP resolvem documentos por ID.

## Edição e histórico

`Buffer::edit(&mut SelectionState, ViewId)` retorna um `Editor` por empréstimo. Suas operações de navegação e edição atualizam o estado da view fornecida. `EditorRead` recebe um documento e estado de seleção por empréstimo para consultas. `App::editor_mut` cria uma sessão de edição que publica cada mapeamento de posições aplicado às demais views ao concluir a operação e sincroniza a revisão resultante com LSP e validação de schema.

As transações validam intervalos de caracteres não sobrepostos antes de alterar texto. `TransactionOutcome` expõe revisão resultante, tamanho em caracteres, indicador de alteração efetiva, edições de posição e restauração opcional de seleções do histórico. `apply_document_transactions` retorna um resultado por alvo. Operações rejeitadas preservam texto, seleções, histórico e revisão. Operações sem alteração preservam revisão e histórico de redo. Transações efetivas, undo e redo avançam a revisão uma vez cada.

Entradas de histórico registram opcionalmente o `ViewId` de origem e suas seleções anteriores e posteriores. Undo/redo restauram as seleções dessa view se ela existir e remapeiam todas as demais, incluindo a view que invocou a operação. Foco e scroll permanecem onde o usuário os deixou. Se a origem tiver sido fechada, todas as views existentes são remapeadas; undo não a recria.

A substituição por busca registra cada ocorrência como uma edição constituinte; assim, o cursor de outra view entre ocorrências acompanha o deslocamento correto. Clipboard, IME, multicursor, completion, comandos de linha, acessibilidade e quick fixes usam as mesmas regras de transação e mapeamento.

O estado dirty compara o texto e formato atuais com o último salvamento bem-sucedido. Desfazer até o texto salvo limpa o dirty do texto enquanto avança a revisão. Uma falha ao salvar preserva a referência salva. Recarregar arquivo usa uma transação e mantém revisões crescentes; uma recarga efetiva limpa o histórico de texto obsoleto.

## Ciclo de vida e edições de workspace

Fechar uma view compartilhada preserva documento e histórico. Fechar a última view alterada continua exigindo a decisão existente de salvar, descartar ou cancelar. O histórico de abas fechadas mantém seleções e scroll da última view separados do documento; reabrir preserva identidade e histórico do documento e atribui uma nova identidade à view.

Edições de workspace preparadas mantêm leases de documentos. Fechar a última view durante uma operação pendente mantém o documento no store até a liberação do lease. Uma edição pendente efetiva publica seu resultado dirty em uma view de segundo plano. Os demais documentos fechados são destacados para o histórico de abas fechadas; o event loop também coleta leases liberados. Descartar uma edição preparada libera seus leases sem alterar texto. Reabrir um arquivo que já possui um documento ativo o ativa em vez de criar uma duplicata.

A preparação resolve caminhos locais canônicos e intervalos UTF-16. O commit verifica novamente identidades estáveis, revisões esperadas, caminhos, estado somente leitura, permissões de escrita e snapshots de arquivos antes fechados, antes de alterar qualquer alvo. Um alvo fechado alterado, ilegível ou aberto após a preparação rejeita o lote. Alvos fechados com alteração efetiva tornam-se documentos dirty em segundo plano; alvos sem alteração não criam documento ou view. Edições de workspace não gravam arquivos.

A atomicidade cobre texto e histórico de undo em memória. Persistência em disco e operações de recursos de arquivo ficam fora dessa garantia. Undo multidocumento continua sendo por documento.

## Validação e trabalho posterior

Testes nativos usam dados temporários isolados e recursos simulados de clipboard/LSP. Cobrem views independentes, IME, multicursor, clipboard, busca, completion, comandos de linha, acessibilidade, origens do histórico, operações sem alteração, revisões crescentes na recarga, metadados de salvamento, fechamento/reabertura, liberação de leases e falhas multidocumento.

Execute `cargo test --workspace` e `cargo build --workspace` na plataforma disponível. A verificação interativa nativa de UI/GPU continua pendente no Windows e no Linux; não é obrigatório executar testes Linux na mesma sessão de desenvolvimento Windows.

O registro de comandos, a fronteira do host de extensões e o coordenador de salvamento assíncrono permanecem etapas posteriores separadas. O host Code OSS real, uma versão fixada, o teste de uma extensão real e a compatibilidade `.vsix` continuam sendo trabalho futuro obrigatório.
