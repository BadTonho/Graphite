# Serviços internos e host de extensões simulado

## Fronteiras

`shoppe-core-api` define identidade de documento, URIs validadas, snapshots, edições UTF-16, requisições/resultados de comandos, cancelamento, erros e interfaces de serviços de documentos/comandos/workspace/janela. Depende de serialização e tratamento de URIs, sem tipos de GPU, aplicação ou LSP. As implementações do event loop ficam em `shoppe-app/app/services/`. Adaptadores convertem seus tipos de protocolo na fronteira.

Snapshots consultam documentos abertos em memória, incluindo texto alterado; arquivos fechados usam o provedor local e a decodificação UTF-8/UTF-16 existente. URIs `file:` convertem caminhos do sistema sem conversões de string com perda; `untitled:` identifica documentos sem título. Esquemas não suportados e caminhos não representáveis produzem erros. Snapshots de arquivos fechados não criam documentos ou views.

Edições versionadas reutilizam preparação, retenção de documentos e commit atômico em memória. Todos os alvos são validados antes de publicação ou alteração. Permanecem os limites de workspace, verificações de somente leitura, formatos salvos, operações sem mudança e undo. Metades de pares substitutos UTF-16 e intervalos inválidos são rejeitados. Chamadas aninhadas do host carregam o ID da execução pai, permitindo registrar o histórico de seleções na view de origem sem mudar o foco. Gravações em disco continuam separadas e usam o [coordenador de salvamento assíncrono](async-saving.md).

Assinaturas de documentos emitem snapshots de abertura, alteração e fechamento. Sessões de edição por empréstimo publicam alterações; a reconciliação detecta abertura, coleta e mudanças de metadados. Documentos inalterados não são copiados a cada passagem pelo event loop. Cada assinatura tem fila limitada; saturação é informada, e descartar uma assinatura libera sua propriedade. Um documento retido não é fechado apenas porque sua última view desapareceu.

A configuração de workspace combina as camadas existentes com as preferências efetivas do editor. Serviços de janela abrem documentos, focam views explicitamente e exibem mensagens. Comandos usam a API de execução do item 2. Handlers internos continuam pertencendo à aplicação; adaptadores de protocolo não podem alterar seu estado a partir de workers.

## Processo e protocolo

`shoppe-extension-host` contém um supervisor reutilizável e um executável simulado. É desativado por padrão e executado como processo Rust separado, oculto no Windows. Não carrega extensões, Node ou Code OSS.

O protocolo simulado está na versão 2: mensagens JSON com comprimento de quatro bytes em big-endian, por stdin/stdout. Stderr é reservado para logs. O handshake estabelece uma nova sessão; requisições correlacionam sessão e ID, separadamente em cada direção. Respostas de sessões antigas ou duplicadas são ignoradas; framing inválido ou mensagens incompatíveis encerram a sessão.

O prazo de handshake é de cinco segundos. Requisições pendentes comuns têm prazo de 30 segundos e máximo de 128 entradas por direção; mensagens são limitadas a 16 MiB. As filas são limitadas e retornam erros tipados em saturação. A aplicação limita registros de uma sessão a 1.024. Cancelamento é cooperativo e não desfaz edições ou efeitos de sistema de arquivos/Git já confirmados. Chamadas que aguardam uma confirmação existente suspendem o prazo de RPC e mantêm suporte a cancelamento.

Comandos do host têm metadados em inglês, tipos de alvo e condições de enablement. Seus registros não podem substituir comandos internos nem duplicar outro comando. Handles de registros e assinaturas pertencem a uma sessão e podem ser liberados explicitamente. A entrada central de comandos resolve os comandos registrados pelo host; IDs locais desconhecidos nunca são enviados automaticamente ao LSP.

Workers mantêm dados de transporte e notificam o editor. Somente o event loop aplica requisições e resultados dos serviços, verificando sessão, cancelamento, existência do alvo e versões. O loop agenda verificações de prazo enquanto há trabalho pendente e pode dormir quando está ocioso. Saída do processo, EOF, falha de protocolo ou reinício manual falham execuções pendentes e liberam comandos e assinaturas. A coleta do processo filho acontece fora do event loop. O reinício é manual, cria uma sessão nova e não repete registros anteriores.

## Integração LSP

O LSP mantém transporte, framing e ciclo de vida próprios. A publicação de documentos e as edições de workspace usam os mesmos contratos de serviços. Cada instância de servidor identifica seus eventos, impedindo que uma instância antiga publique diagnósticos, aplique edições ou encerre requisições de uma substituta.

Requisições enviadas têm prazo de 30 segundos, limite de 128 entradas, validação de respostas e conclusão única. Expiração envia `$/cancelRequest`; falha de transporte e encerramento falham pendências. A inicialização mantém seu prazo absoluto de 20 segundos. Requisições recebidas mantêm o limite existente de 64 entradas e prazo de 30 segundos; cancelamento ou expiração antes de o event loop assumir a requisição impede a aplicação, enquanto uma edição já assumida termina atomicamente.

`workspace/configuration` e `workspace/workspaceFolders` consultam os serviços de workspace. Mensagens de janela usam o serviço correspondente; `window/showMessageRequest` exibe a mensagem e retorna uma escolha dispensada (`null`). Comandos de servidor em quick fixes continuam em `workspace/executeCommand`. O supervisor do host e o gerenciador LSP não encerram um ao outro.

## Desenvolvimento e validação

A fixture local `shoppe-test-lsp` exercita o worker LSP de produção por stdin/stdout, incluindo inicialização, resultados JSON, transações de serviços por applyEdit, respostas inválidas, cancelamento, respostas tardias e EOF com requisições pendentes. Consulte [Testes de regressão](../contributing/testing.md) para comandos e a matriz completa de cobertura. Testes por canais também verificam rejeição de instâncias anteriores e prazos controlados.

Depois de `cargo build --workspace`, execute:

```bash
cargo run -p shoppe-app -- --simulate-extension-host <pasta>
```

O executável auxiliar fica ao lado do Graphite. Para testes específicos da aplicação que usam o processo real, compile-o antes com `cargo build -p shoppe-extension-host`; os testes do workspace o compilam pelo alvo de integração de processos. Não é necessário servidor de linguagem externo nem serviço de rede.

O simulador registra `shoppe.simulated.echo`, `shoppe.simulated.wait`, `shoppe.simulated.crash` e `shoppe.simulated.service`. Echo retorna JSON, wait exercita cancelamento/prazos, crash exercita isolamento e service chama o contrato de serviços do editor. Reinicie com `shoppe.extensions.restartHost`, por exemplo por um atalho personalizado:

```json
{"key":"ctrl+alt+r","command":"shoppe.extensions.restartHost","when":"extensionHostEnabled"}
```

Testes cobrem snapshots, assinaturas e saturação, edições atômicas com documentos abertos/fechados/sem título, permissões, UTF-16, erros de metadados/registros, handshake e queda/reinício de processo real, cancelamento e prazos com tempo controlado, mensagens antigas, histórico de views sem foco, decisões pendentes, continuidade do LSP e instâncias antigas de servidor. A validação automatizada nativa é realizada no Windows; execução no Linux e verificações interativas de UI/GPU não foram realizadas neste ambiente.

Os recursos da sessão também incluem participantes de salvamento. `ServiceRequest::Save` retorna o resultado de um ticket de salvamento, e chamadas vinculadas ao participante permitem apenas consultas. Participantes têm prazos independentes de um segundo; liberar um recurso ou encerrar uma sessão invalida seu resultado pendente. Decisões do usuário suspendem prazos RPC comuns e `Resumed` os restabelece após a decisão. Consulte [Salvamento assíncrono](async-saving.md) para contratos de preparação, gravação e ciclo de vida.

O host Code OSS real continua como etapa posterior obrigatória: fixar um release ou commit, integrar seu host Node real e validar uma extensão real antes de declarar compatibilidade `.vsix`.
