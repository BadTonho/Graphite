# Etapas restantes

Plano do trabalho de paridade de recursos que ainda falta fazer. As etapas 1 a 3 estão feitas até onde a arquitetura atual permite (veja o [Inventário de paridade de recursos](feature-parity.md)); este documento lista o que sobrou, na ordem proposta. O checklist local `Itens/comandos-faltando.md` (fora do commit) acompanha cada id de comando.

## Situação atual

| Etapa | Escopo | Estado |
| --- | --- | --- |
| 1. Núcleo do editor | Linhas, caixa, dobramento, multi-cursor, âncora de seleção, colchetes, mover texto, valores | Feita, com as sobras abaixo |
| 2. Shell do workbench | Grupos de editores, partes da janela, abrir rápido, navegação em listas, barra de atividades, breadcrumbs | Feita, com as sobras abaixo |
| 3. Ferramentas do dia a dia | Explorador, controle de código-fonte, histórico local, linha do tempo, outline, preferências, tarefas, depuração, extensões, snippets, saída | Feita, com as sobras abaixo |
| 4. Editores de merge e de diff | Editor de merge de três vias, diff de vários arquivos | Feita, com as sobras abaixo |
| 5. Notebooks | Editor de notebook, janela interativa, REPL | Feita, com as sobras abaixo |
| 6. Serviços | Chat, sync, remoto, atualização e afins | Feita, com as exclusões abaixo |

## Etapa 4: editor de merge e diff de vários arquivos

Cerca de 32 comandos, mas o trabalho é uma interface nova, não um conjunto de atalhos.

1. **Modelo de conflito.** Ler os marcadores de conflito (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`) em blocos com o texto atual, o entrante e a base. Hoje o aplicativo só tem uma vista de conflito simples; o modelo precisa aceitar vários conflitos por arquivo e continuar valendo enquanto o arquivo é editado.
2. **Layout de três painéis.** Entrante e atual lado a lado em cima, o resultado embaixo, cada um com números de linha, diferenças destacadas e rolagem sincronizada. Reaproveita o desenho de diff que já existe.
3. **Ações por bloco.** Aceitar o atual, aceitar o entrante, aceitar os dois (em qualquer ordem) e ignorar, por bloco. Ir para o conflito seguinte ou anterior. Voltar um bloco ao estado sem resolver.
4. **Fluxo de resolução.** Contador de conflitos não resolvidos, "concluir merge" que grava o resultado e prepara o arquivo no Git, e "aceitar tudo" do atual ou do entrante.
5. **Pontos de entrada.** Abrir pela lista do controle de código-fonte para um arquivo em conflito, pela paleta de comandos e pela vista de conflito existente.
6. **Diff de vários arquivos.** Uma vista rolável com os diffs de vários arquivos (por exemplo tudo que está preparado ou tudo que mudou), recolhível por arquivo, com próxima e anterior alteração entre arquivos.
7. **Comandos.** Ids do editor de merge (aceitar, navegar, alternar layout, mostrar base, reiniciar) e do diff de vários arquivos, com títulos nos dois idiomas.
8. **Testes e documentação.** Leitura de conflitos com marcadores incomuns e CRLF, cada ação de bloco, o texto do resultado para cada combinação, e documentação em EN e PT-BR.

Decisões: a base é um quarto painel opcional (no topo ou no centro) alimentado pela seção `|||||||` dos marcadores; o painel de resultado é somente leitura, e **Abrir o resultado no editor** entrega o texto até agora a um editor comum para terminar à mão. Veja [Editor de mesclagem](../features/merge-editor.md) e [Diff de vários arquivos](../features/multi-file-diff.md).

## Etapa 5: notebooks

Cerca de 76 comandos. Um subsistema à parte.

1. **Modelo do documento.** Ler e gravar o formato JSON `.ipynb` sem perder campos que ele não entende: células (código, markdown), código-fonte, saídas, contagens de execução, metadados.
2. **Editor de notebook.** Uma lista vertical de células com um editor em cada uma, seleção e foco de célula, adicionar, excluir, mover, dividir, juntar, trocar o tipo, recolher entrada e saída, e desfazer entre células.
3. **Execução.** Conexão com um kernel (protocolo Jupyter sobre ZeroMQ ou um processo local), escolha de kernel, iniciar, reiniciar, interromper, executar célula, executar tudo, executar acima e abaixo.
4. **Saídas.** Primeiro fluxos de texto e de erro, depois imagens, HTML e dados de exibição ricos, com limites de tamanho.
5. **Janela interativa e REPL.** Um notebook de rascunho ligado a um kernel, com uma caixa de entrada embaixo.
6. **Comandos, testes e documentação.** Os ids de comando de notebook, documentação em EN e PT-BR, testes com um kernel falso.

Decisões: o protocolo é o do Jupyter sobre um transporte ZeroMQ próprio, falando com um kernel que o aplicativo inicia como processo local (nada de dependência externa de ZeroMQ). As saídas ricas têm limites de tamanho: imagens passam por um segundo atlas RGBA do renderizador, e HTML, Markdown, LaTeX e JSON aparecem como texto. Veja [Editor de notebook](../features/notebook-editor.md) e [Janela interativa](../features/interactive-window.md).

## Etapa 6: recursos que dependem de serviços

Implementada em módulos e comandos dedicados, com as decisões de backend registradas abaixo.

| Área | Decisão de backend e arquitetura | Referência |
| --- | --- | --- |
| Chat, chat inline, agentes, sessões | Provedores SSE em `graphite-chat` (Anthropic e compatíveis com OpenAI, `chat-provider.json`), `ToolApprovalPolicy` explícito, diff de chat inline e sessões persistidas | [Chat, Chat Inline e Sessões de Agentes](../features/chat.md) |
| Sincronização de dados do usuário | Gist privado no GitHub (`read:user gist`) com mesclagem JSON de três vias, aceitação manual de conflitos e backups locais com timestamp (`sync-backups/`) | [Sincronização de Dados do Usuário](../features/user-data-sync.md) |
| Sessões de edição, remoto e túneis | Cliente OpenSSH do sistema (`ssh`) com leitura de `~/.ssh/config`, relay WebSocket RFC 6455 (`ws://`), limpeza de portas e snapshots em `<perfil>/edit-sessions.json` | [Workspaces Remotos, Túneis e Sessões de Edição](../features/remote-and-edit-sessions.md) |
| Atualização | GitHub Releases (`stable` / `prerelease`), ordenação `SemVer` estrita e verificação `ring::digest::SHA256` antes do staging | [Atualizações e Relator de Problemas com Redação de Segredos](../features/updates-and-issue-reporter.md) |
| Relato de problemas, diagnóstico | Redação automática de segredos e caminhos, log de solução de problemas e URLs pré-preenchidas de issue no GitHub | [Atualizações e Relator de Problemas com Redação de Segredos](../features/updates-and-issue-reporter.md) |

Exclusões intencionais e verificações não executadas:
- Comandos de voz e fluxos proprietários de cobrança/conta continuam excluídos.
- A renderização visual interativa em janela desktop dos novos painéis de chat/quick chat exige janela ativa e não foi verificada visualmente.

## Sobras das etapas 1 a 3

Itens que ficaram de fora porque dependem de algo que o aplicativo ainda não tem. Cada um pode ser retomado quando o pré-requisito existir.

**Núcleo do editor**
- Comandos do editor de diff: precisam da interface do editor de diff.
- Comandos de acessibilidade e de inspeção do editor.

**Shell do workbench**
- Foco de teclado para a barra de atividades, a barra de status, o banner e as vistas do próprio painel.
- Mover e redimensionar vistas, e a posição das notificações.
- Breadcrumbs navegáveis (focar, percorrer, escolher itens).
- Comandos de lista para resultados de busca, a lista do controle de código-fonte e as outras listas; busca por digitação; seleção múltipla.

**Ferramentas do dia a dia**
- Canais de saída, logs compostos (`workbench.output.action.clearOutput` já está implementado).
- Comandos de gerar e pré-visualizar temas.
- Comandos de teste que existem para extensões.
- Controle de código-fonte: foco da entrada e dos grupos de recursos, commit seguinte e anterior (`git.viewHistory` com grafo Git já está implementado).
- Depuração: testes contra um adaptador de depuração real (o cliente só foi testado contra um simulado). Marcas de breakpoint na margem, pontos de interrupção de dados (`setDataBreakpoints`), decoradores do Test Explorer na margem e popup de hover de variáveis no debug pausado já estão implementados.
- Extensões: atualizar todas, verificar atualizações, ativar só no espaço de trabalho, recomendações, lista de embutidas, abrir a pasta de extensões.

**Editores de merge e de diff**
- Comandos de desenvolvimento do editor de merge (copiar, salvar e carregar o conteúdo como JSON, abrir uma seleção em um editor de merge temporário): ferramentas de desenvolvedor, sem previsão.
- O painel da base só tem o que os marcadores de conflito trazem; calcular uma base para marcadores de duas vias exige os estágios do índice.
- Digitar direto no painel de resultado (use Abrir o resultado no editor).
- Comandos do editor de diff (`switchSide`, `collapseAllUnchangedRegions`, `showAllUnchangedRegions`, `compareEditor.nextChange`/`previousChange`) já estão implementados; reverter, código movido e diff de notebook ainda exigem que a vista de diff de dois arquivos vire um editor.

**Notebooks**
- Saídas ricas: SVG, HTML desenhado como página (hoje é lido como texto), widgets interativos e saídas de aplicativos.
- Kernels remotos ou de servidor Jupyter: só kernels locais são iniciados.
- Formatar o notebook ou a célula: depende de ligar a formatação por linguagem às células.
- Multi-cursor dentro de uma célula e os comandos de cursor de notebook.
- Selecionar toda a saída de uma célula, mostrar saídas vazias, posição da barra de ferramentas da célula, ordem dos tipos MIME e os comandos de layout e de diagnóstico de layout.
- Diff de notebook (próximo, anterior, reverter entrada, saídas e metadados): exige que a vista de diff vire um editor.
- Comandos de chat em células: pertencem à Etapa 6.
- Testes contra um `ipykernel` real: existe um teste ignorado por padrão (defina `GRAPHITE_TEST_KERNEL_PYTHON`); o restante usa um kernel falso que fala o protocolo de verdade por sockets de verdade.
- O desenho das imagens e das células precisa de uma janela e não foi verificado visualmente.

## Regras de trabalho para cada etapa

- Um commit por passo, feito só quando a suíte de testes inteira passa.
- Documentação em inglês e português para toda mudança visível ao usuário.
- Nenhuma menção ao editor de referência no código, nos textos, na documentação nem nas mensagens de commit.
- O que precisa de uma janela para ser visto é relatado como não verificado visualmente.
- O checklist local é atualizado a cada commit.
