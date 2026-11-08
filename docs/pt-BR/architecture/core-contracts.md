# Contratos do Núcleo do Editor

Este documento descreve os contratos **documento/view**, **comandos e contexto**, **serviços internos** e **fronteira de extensões simulada** implementados. O host Code OSS real e participantes assíncronos de salvamento permanecem como trabalhos futuros.

## 1. Documento e view

### Estado atual

`Document` mantém texto, linguagem, formato e caches do documento. `DocumentStore` usa chaves imutáveis independentes dos dados do documento; não oferece slice mutável nem acesso posicional. Os grupos mantêm a ordem das abas, e cada `View` mantém suas seleções, scroll, composição IME e navegação da seleção sintática. O documento ativo é derivado da view em foco.

`Buffer` contém texto e histórico sem cursor ativo. `Buffer::edit` recebe por empréstimo o `SelectionState` e o `ViewId` da view; as operações de texto aplicam transações e expõem mapeamentos de posições por `TransactionOutcome`. As sessões de edição da aplicação remapeiam as outras views e publicam versões para LSP e validação de schema. Undo/redo restauram as seleções da view de origem quando ela ainda existe, remapeiam as demais e mantêm o foco atual. Os adaptadores de sincronização por snapshot/restauração foram removidos.

Edições de workspace preparadas retêm documentos abertos e revalidam identidades, versões, intervalos, permissões e snapshots de arquivos fechados antes do commit. Alvos fechados sem alteração não criam abas. A atomicidade cobre texto e histórico em memória; operações de arquivo e persistência em disco permanecem fora dessa garantia. Consulte [Implementação de documento e view](document-view.md) para detalhes de ciclo de vida e validação.

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

A migração de documento/view está concluída. Os chamadores de edição usam estado da view por empréstimo, chaves estáveis de documento e remapeamento centralizado. Os testes de regressão cobrem views compartilhadas, origem do histórico, operações sem alteração, retenção por operações pendentes e falhas multidocumento. A verificação nativa de UI/GPU continua pendente no Windows e no Linux.

## 2. Comandos e contexto

### Estado atual

`CommandRegistry` descreve todos os comandos locais, com handlers separados por domínio. Teclado, ações semânticas de widgets, acessibilidade e chamadas programáticas usam `App::execute_command`, validando argumentos, alvos explícitos e enablement antes do despacho. Requisições e resultados são serializáveis; tickets acompanham conclusão síncrona, cancelamento cooperativo, resultados de workers e confirmações pendentes. `ContextKeyService` resolve escopos globais, de views e de widgets e se atualiza sem exigir evento de teclado. O dispatcher legado e seu fallback foram removidos. Consulte [Comandos e contexto](commands-context.md) para API, ciclo de vida e testes implementados.

### Contrato alvo

- Um registro associa um ID estável a metadados, enablement e um handler. IDs e atalhos existentes são preservados. Argumentos e resultados são serializáveis. Handlers internos recebem por empréstimo a aplicação do event loop com alvo explícito; adaptadores externos usam contratos de serviços compartilhados. Comandos pertencentes à sessão do host participam da mesma API de execução.
- Um único ponto de execução recebe ID, argumentos, contexto alvo e token de cancelamento e retorna assincronamente um resultado ou erro tipado. O cancelamento é cooperativo e não implica desfazer efeitos já confirmados. Comandos desconhecidos, desabilitados, com falha ou cancelados são informados sem derrubar ou bloquear a interface.
- Um contexto global fornece chaves compartilhadas; cada view ou widget alvo tem um contexto próprio. Uma chave é resolvida primeiro no escopo alvo e depois no global. Foco e alvos explícitos determinam o escopo avaliado; chamadas sem alvo usam o escopo global.
- Expressões `when` de keybindings controlam a elegibilidade do atalho; condições de menu controlam visibilidade; enablement controla se o comando pode ser executado. Elas compartilham a avaliação de contexto, mas continuam sendo condições distintas. O registro verifica enablement centralmente, inclusive em chamadas programáticas.
- As fontes existentes de comandos entram pelo registro; futuros adaptadores de menus, paleta e extensões usarão a mesma API. Entrada bruta de texto e transporte de terminal mantêm seus fluxos. Comandos destinados ao servidor usam `workspace/executeCommand`; IDs locais desconhecidos nunca são encaminhados automaticamente. Alterações de texto usam transações e undo. Edições em views sem foco e undo/redo preservam o foco.

### Critérios de aceitação

1. Um comando chamado por keybinding e programaticamente chega ao mesmo handler e observa o escopo alvo pretendido; uma chave local prevalece sobre a global.
2. Um item de menu oculto ou atalho inelegível não desabilita o comando por si só; um comando desabilitado é rejeitado mesmo em chamada direta.
3. IDs desconhecidos, erros de handler e cancelamento são informados, e um handler assíncrono pendente não congela a interface. Comandos LSP destinados ao servidor continuam sendo enviados ao servidor.
4. Um comando que edita texto produz o mesmo comportamento de versão, remapeamento de views e undo que outra transação de documento.

### Migração incremental

A migração de comandos está concluída, incluindo todos os IDs do dispatcher anterior, atalhos padrão e argumentos JSON. Workers devolvem resultados próprios ao event loop, que revalida cancelamento, existência do alvo, workspace e versões antes de aplicá-los. O salvamento permanece síncrono. Interfaces compartilhadas e comandos pertencentes a sessões são descritos abaixo; uma paleta permanece como trabalho futuro.

## 3. Fronteira de extensões e protocolos

### Estado atual

`graphite-core-api` define contratos independentes de serviços e dados serializáveis; a aplicação os implementa no event loop. `graphite-extension-host` oferece um processo Rust simulado e supervisor reutilizável com framing versionado, sessões, registros com proprietário, cancelamento, prazos e reinício manual. Uma falha do host libera seus recursos enquanto edição e LSP continuam. Consulte [Serviços internos e host de extensões simulado](services-host.md).

Publicação e edições LSP usam os serviços de documentos; consultas de workspace e mensagens usam serviços de workspace/janela. Instâncias de servidor identificam seus eventos, e requisições pendentes têm limites, cancelamento, prazos e limpeza no encerramento. Edições canceladas antes de a interface assumir a requisição não são aplicadas; edições já assumidas terminam atomicamente. Isso implementa a fronteira simulada; o host Code OSS real e participantes assíncronos de salvamento permanecem em etapas separadas.

### Contrato alvo

- Serviços reutilizáveis de documento, comandos, workspace/configuração e janela expõem mensagens, identidade, versões, cancelamento e ciclo de vida explícitos. O bridge real posterior deverá fixar o release ou commit do Code OSS e adicionar os serviços exigidos pelas camadas reais de compatibilidade.
- Manter adaptadores de protocolo distintos: LSP e host simulado têm transportes e ciclos de vida separados, chamando serviços compartilhados. O protocolo simulado não declara compatibilidade com o futuro protocolo Code OSS.
- Extensões e workers nunca alteram o estado da interface diretamente. Entregar resultados ao event loop responsável; não bloquear a interface esperando RPC. Acompanhar requisições, cancelamento, prazos, controle de fluxo, encerramento e reinício do host, liberando registros e requisições pendentes associados a ele quando terminar.
- Definir abstrações de URI e sistema de arquivos, começando por arquivos locais. Alterações de documento versionadas e edições de texto passam pelo contrato de documento; converter posições e garantias específicas de cada protocolo em sua fronteira. Operações sobre recursos e persistência em disco ficam fora da garantia inicial de atomicidade de texto multidocumento.
- O item 4 adicionará participantes assíncronos de salvamento em sequência limitada, rejeição de edições obsoletas e gravações de snapshots que preservem texto novo como dirty. O salvamento permanece síncrono na implementação atual.

### Critérios de aceitação

1. Alterações LSP versionadas e edições de extensão chegam às mesmas invariantes de documento por seus respectivos adaptadores; uma edição obsoleta é rejeitada sem modificar o texto.
2. Um participante de salvamento que responde após nova digitação não pode sobrescrevê-la; timeout e falha são informados, o conteúdo atual ainda pode ser salvo e a interface permanece responsiva.
3. Digitar durante um salvamento pendente deixa o conteúdo mais recente *dirty* após a gravação do snapshot anterior; uma falha de gravação preserva o estado salvo de referência anterior.
4. O encerramento do host falha as requisições pendentes e libera seus registros sem encerrar o editor; os ciclos de vida de LSP e host permanecem independentes.

### Migração incremental

Interfaces de serviços, adaptador LSP e supervisor do host simulado estão implementados. Testes nativos exercitam falhas de protocolo, queda/reinício de processo real, limpeza de recursos e operação independente do LSP. As próximas etapas são salvamento assíncrono e integração do host Code OSS real.

O host Code OSS real continua sendo uma etapa posterior obrigatória: fixar um release ou commit, integrar o host Node real e validá-lo com uma extensão real antes de declarar compatibilidade.

## Como os contratos interagem

A digitação envia uma transação para a view em foco, avança a versão do documento e envia a alteração LSP correspondente. Salvar é resolvido pelo registro e atualmente grava de forma síncrona, atualizando o estado salvo de referência somente em caso de sucesso. Cada view afetada é remapeada. Um futuro coordenador executará participantes com prazo e rejeitará edições obsoletas antes de gravar snapshots assincronamente. Futuros comandos de extensão usarão o registro e o contexto com escopo; comandos de servidores de linguagem já usam o transporte exigido pelo LSP.

## Ordem recomendada

1. Identidade de documento, transações e propriedade de estado das views.
2. Registro de comandos e contexto com escopos, migrados gradualmente.
3. Contratos de serviços compartilhados e adaptadores separados para LSP e extension host, seguidos das camadas de compatibilidade.

Cada etapa deve acrescentar testes para suas invariantes e manter a aplicação utilizável durante a migração.
