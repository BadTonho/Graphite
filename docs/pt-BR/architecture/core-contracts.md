# Contratos do Núcleo do Editor

Este documento propõe três contratos: **documento/view**, **comandos e contexto** e **fronteira de extensões**. O comportamento alvo e os critérios de aceitação são decisões de projeto para implementação futura, não afirmações sobre capacidades atuais. Os exemplos de tipos são ilustrativos, não APIs Rust definitivas. Formalizar esses contratos não exige implementar toda a arquitetura do VS Code de uma vez.

## 1. Documento e view

### Estado atual

`Tab` em `crates/shoppe-app/src/main.rs` combina estado de documento e apresentação. `Buffer` em `crates/shoppe-text/src/lib.rs` mantém as seleções ativas e o histórico de texto. `View` em `crates/shoppe-app/src/app/editor_groups.rs` guarda seleções e scroll por grupo; snapshots e restaurações sincronizam a view ativa com o buffer, e edições de posição remapeiam as demais views. O teste existente `app_views_share_text_and_history_with_independent_selections` exercita seleções independentes, texto compartilhado e undo. O risco é que os caminhos de mutação precisem executar essa sincronização consistentemente, em vez de a propriedade do estado garanti-la.

`Buffer::apply_transaction` já valida substituições não sobrepostas, e substituições efetivas, undo e redo atualizam sua revisão em `crates/shoppe-text/src/edit.rs`. Isso oferece uma base útil, mas não demonstra que toda operação futura de documento satisfaça o contrato proposto.

`shoppe-text` também oferece pré-validação e uma transação multidocumento em memória com revisões esperadas opcionais. Ela ainda não está integrada às edições de workspace do editor nem garante atomicidade em disco; a migração abaixo continua necessária.

O caminho de quick fix pré-valida edições de texto, rejeita operações de recursos não suportadas e posições UTF-16 inválidas e verifica as versões de documento fornecidas pelo LSP. Ele ainda grava arquivos fechados separadamente, portanto suas edições de workspace não são atômicas entre gravações em disco e documentos abertos.

### Contrato alvo

- Um documento tem identidade estável, texto, URI ou identidade sem título, linguagem, versão crescente, estado salvo e histórico de undo. Um documento pode ter várias views; cada view é dona de suas seleções, scroll, foco e demais estados de apresentação. Fechar uma view mantém o documento vivo enquanto outra o referenciar.
- Uma transação de texto é uma alteração lógica, que pode conter várias substituições não sobrepostas sobre a mesma revisão. Uma transação efetiva incrementa a versão desse documento exatamente uma vez; uma operação sem alteração não a incrementa. Undo e redo que alteram texto produzem, cada um, uma nova versão. Todas as views observam o mesmo texto e versão, e suas seleções são remapeadas pela alteração, inclusive em undo e redo.
- O estado *dirty* reflete se o conteúdo atual e os metadados relevantes para salvamento correspondem ao último salvamento bem-sucedido, não apenas se os números de versão coincidem: undo pode restaurar o conteúdo salvo enquanto avança a versão. Uma falha ao salvar não atualiza o estado salvo de referência.
- Uma edição com versão esperada é rejeitada se o documento não estiver mais nessa versão. Em uma edição de texto com vários documentos, identidades, versões esperadas, permissões e intervalos são validados antes da aplicação; ou todas as alterações de texto são aplicadas ou nenhuma é. Cada documento alterado avança uma vez e registra uma transação no histórico de undo. Essa garantia inicial de atomicidade exclui operações de arquivo e persistência; elas não devem ser apresentadas como parte da operação atômica.
- A sincronização com LSP usa identidade e versão do documento, não identidade da aba. A seleção da view ativa após a edição é definida pela operação; as demais views são remapeadas a partir de suas posições anteriores.

### Critérios de aceitação

1. Duas views de um documento mantêm seleções e scroll independentes após trocas de foco, edições, undo e redo; ambas veem o mesmo texto.
2. Transações efetivas, operações sem alteração, undo e redo seguem as regras de versão; desfazer até o conteúdo salvo limpa o estado *dirty* sem reduzir a versão.
3. Uma versão obsoleta, um intervalo inválido ou um destino somente leitura rejeita toda a edição de texto multidocumento sem alterar documentos, versões ou históricos.
4. Fechar uma view preserva o documento e seu histórico enquanto outra view permanecer.

### Migração incremental

Introduzir identidades estáveis de documento e view, depois mover a propriedade das seleções de `Buffer` para as views e passar o estado de seleção explicitamente às operações de edição. Encaminhar todas as alterações de texto por transações de documento e centralizar o remapeamento de views. Migrar chamadores e testes gradualmente; manter adaptadores para a estrutura atual de `Tab` até deixarem de ser necessários.

## 2. Comandos e contexto

### Estado atual

`execute_custom_command` em `crates/shoppe-app/src/app/commands.rs` despacha por um `match` extenso e altera a aplicação diretamente. O handler de keybindings monta valores de contexto localmente; `crates/shoppe-app/src/keybindings/when.rs` fornece o parser de expressões para resolução de keybindings. O código de quick fix trata `CodeActionOrCommand` em `crates/shoppe-app/src/app/editor_commands.rs`, mas esses caminhos não estabelecem um registro unificado nem um contrato geral assíncrono de comandos.

### Contrato alvo

- Um registro associa um ID estável a metadados, enablement opcional e um handler. Usar IDs compatíveis com VS Code quando houver comando correspondente com semântica compatível. Argumentos e resultados que cruzam processos são serializáveis; handlers recebem um contexto explícito de serviços em vez de propriedade irrestrita da aplicação.
- Um único ponto de execução recebe ID, argumentos, contexto alvo e token de cancelamento e retorna assincronamente um resultado ou erro tipado. O cancelamento é cooperativo e não implica desfazer efeitos já confirmados. Comandos desconhecidos, desabilitados, com falha ou cancelados são informados sem derrubar ou bloquear a interface.
- Um contexto global fornece chaves compartilhadas; cada view ou widget alvo tem um contexto próprio. Uma chave é resolvida primeiro no escopo alvo e depois no global. Foco e alvos explícitos determinam o escopo avaliado; chamadas sem alvo usam o escopo global.
- Expressões `when` de keybindings controlam a elegibilidade do atalho; condições de menu controlam visibilidade; enablement controla se o comando pode ser executado. Elas compartilham a avaliação de contexto, mas continuam sendo condições distintas. O registro verifica enablement centralmente, inclusive em chamadas programáticas.
- Keybindings, menus, paleta, extensões e testes entram pelo registro. Comandos LSP são adaptados ao protocolo: comandos destinados a um servidor de linguagem usam sua requisição `workspace/executeCommand`; apenas comandos do cliente são encaminhados ao registro local quando apropriado. Alterações de texto usam transações de documento e undo.

### Critérios de aceitação

1. Um comando chamado por keybinding e programaticamente chega ao mesmo handler e observa o escopo alvo pretendido; uma chave local prevalece sobre a global.
2. Um item de menu oculto ou atalho inelegível não desabilita o comando por si só; um comando desabilitado é rejeitado mesmo em chamada direta.
3. IDs desconhecidos, erros de handler e cancelamento são informados, e um handler assíncrono pendente não congela a interface. Comandos LSP destinados ao servidor continuam sendo enviados ao servidor.
4. Um comando que edita texto produz o mesmo comportamento de versão, remapeamento de views e undo que outra transação de documento.

### Migração incremental

Adicionar o registro e o serviço de contexto com escopos, extrair um pequeno conjunto de comandos existentes para handlers e encaminhar chamadas pelo registro com fallback legado temporário. Migrar os comandos restantes e remover o fallback; usar os metadados do registro em uma futura paleta de comandos.

## 3. Fronteira de extensões e protocolos

### Estado atual

`docs/en/architecture/extension-compat.md` descreve um extension host Code OSS planejado em processo Node separado e compatibilidade em camadas. O cliente LSP em `crates/shoppe-app/src/lsp.rs` e sua integração com a interface já usam mensagens de worker e notificações ao event loop. É um exemplo útil de concorrência, mas não estabelece um protocolo de extensão versionado compartilhado nem um dispatcher reutilizável para o extension host.

### Contrato alvo

- Definir serviços internos reutilizáveis de documento, comandos, workspace/configuração e janela, além de mensagens, identidade, versões, cancelamento e ciclo de vida explícitos. Fixar o release ou commit do Code OSS visado pelo bridge de extensões; adicionar outros serviços conforme as camadas de compatibilidade exigirem.
- Manter adaptadores de protocolo distintos: LSP e extension host têm semânticas, direções, transportes e ciclos de vida diferentes. Eles podem chamar os mesmos serviços internos, mas não precisam compartilhar transporte ou dispatcher. Um crate de protocolo e um crate supervisor do plugin host são fronteiras propostas, sujeitas ao protocolo real do host escolhido.
- Extensões e workers nunca alteram o estado da interface diretamente. Entregar resultados ao event loop responsável; não bloquear a interface esperando RPC. Acompanhar requisições, cancelamento, prazos, controle de fluxo, encerramento e reinício do host, liberando registros e requisições pendentes associados a ele quando terminar.
- Definir abstrações de URI e sistema de arquivos, começando por arquivos locais. Alterações de documento versionadas e edições de texto passam pelo contrato de documento; converter posições e garantias específicas de cada protocolo em sua fronteira. Operações sobre recursos e persistência em disco ficam fora da garantia inicial de atomicidade de texto multidocumento.
- Participantes de salvamento executam assincronamente em sequência limitada por tempo. Associar as edições retornadas por cada participante à versão de documento que ele observou e rejeitá-las se essa versão estiver obsoleta. Em caso de falha, timeout ou edições obsoletas, informar a falha do participante e continuar salvando o conteúdo atual, sem bloquear a interface. Um salvamento bem-sucedido atualiza o estado salvo de referência para o conteúdo efetivamente gravado: edições feitas enquanto a gravação estiver pendente permanecem *dirty*. Emitir eventos de antes e depois de salvar apropriados ao protocolo, incluindo `willSave`, `willSaveWaitUntil` e `didSave` quando suportados.

### Critérios de aceitação

1. Alterações LSP versionadas e edições de extensão chegam às mesmas invariantes de documento por seus respectivos adaptadores; uma edição obsoleta é rejeitada sem modificar o texto.
2. Um participante de salvamento que responde após nova digitação não pode sobrescrevê-la; timeout e falha são informados, o conteúdo atual ainda pode ser salvo e a interface permanece responsiva.
3. Digitar durante um salvamento pendente deixa o conteúdo mais recente *dirty* após a gravação do snapshot anterior; uma falha de gravação preserva o estado salvo de referência anterior.
4. O encerramento do host falha as requisições pendentes e libera seus registros sem encerrar o editor; os ciclos de vida de LSP e host permanecem independentes.

### Migração incremental

Especificar primeiro as interfaces dos serviços de documento, comandos, workspace/configuração e janela. Adaptar fluxos LSP existentes como casos de aceitação dos serviços compartilhados, mantendo despacho e comportamento de protocolo específicos do LSP. Introduzir separadamente o supervisor do host e seu bridge versionado, depois acrescentar camadas de compatibilidade gradualmente.

O host Code OSS real continua sendo uma etapa posterior obrigatória: fixar um release ou commit, integrar o host Node real e validá-lo com uma extensão real antes de declarar compatibilidade.

## Como os contratos interagem

A digitação envia uma transação para a view em foco, avança a versão do documento e envia a alteração LSP correspondente. Salvar é resolvido pelo registro, executa participantes com prazo e aplica somente edições cujas versões esperadas ainda coincidam; grava um snapshot e atualiza o estado salvo de referência apenas em caso de sucesso. Cada view afetada é remapeada. Um comando de extensão usa o registro e o contexto com escopo, enquanto um comando de servidor de linguagem é enviado ao servidor quando o contrato LSP exigir.

## Ordem recomendada

1. Identidade de documento, transações e propriedade de estado das views.
2. Registro de comandos e contexto com escopos, migrados gradualmente.
3. Contratos de serviços compartilhados e adaptadores separados para LSP e extension host, seguidos das camadas de compatibilidade.

Cada etapa deve acrescentar testes para suas invariantes e manter a aplicação utilizável durante a migração.
