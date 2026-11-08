# Plugins nativos e inspeção de VSIX

Plugins da API Graphite v1 executam JavaScript em um processo Node.js 24+ do sistema por plugin. TypeScript é compilado antes do empacotamento. `shoppe-plugins` cuida de manifestos, validação ZIP, instalação, geração de projetos e descoberta no GitHub sem depender da aplicação/GPU. A aplicação mantém o painel e o roteamento das sessões; os serviços existentes aplicam mudanças de documentos e comandos no loop de eventos.

## Limite do runtime

O host CommonJS incluído carrega a entrada JavaScript e chama `activate(context, graphite)`. São aceitos módulos ES e exports CommonJS. `deactivate()` é opcional. O host reutiliza enquadramento, handshake, filas limitadas, prazos, cancelamento e descarte do protocolo v2 de [Serviços e host simulado](services-host.md). Esse RPC é do Graphite, não do Code OSS. Stdout é reservado às mensagens; a saída do console vai para stderr.

Cada processo tem supervisor, comandos, inscrições de documentos, IDs de requisição e sessão próprios. A aplicação encaminha chamadas ao plugin responsável e mantém o simulador separado. Comandos devem corresponder ao manifesto e não podem substituir comandos internos. A API v1 expõe comandos, consultas/eventos/edições versionadas de documentos, consultas de workspace/configuração e mensagens. Não expõe painéis arbitrários, webviews, participantes de salvamento, execução de comandos, salvamento de arquivos ou foco de views. Mudanças preservam validação UTF-16, somente leitura, edições atômicas e undo.

Desabilitar, substituir ou remover um plugin libera seus registros e trabalhos pendentes sem encerrar outros hosts ou LSP. Reiniciar manualmente cria nova sessão. Falhas não provocam reinício automático. O encerramento normal oferece um prazo limitado para `deactivate()` antes de terminar o processo. O processo separado contém falhas, mas não isola código Node malicioso. Habilite apenas código confiável.

## Pacotes e persistência

Pacotes nativos são ZIPs `.graphite-plugin`, com `graphite-plugin.json` na raiz. VSIX é lido por `extension/package.json`. A instalação nunca executa npm, scripts de ciclo de vida ou código do pacote. VSIX fornece somente metadados: sua entrada e contribuições não são ativadas e ele não pode ser habilitado.

O diretório de configuração do usuário contém `plugins/`, registros JSON por plugin e diretórios imutáveis de versões. A instalação valida e extrai para um diretório temporário novo e substitui atomicamente o registro. Uma falha na publicação preserva o registro e pacote anteriores. Diretórios anteriores permanecem disponíveis aos hosts ativos até a remoção; esta respeita o limite exato da identidade do plugin. Toda instalação bem-sucedida começa desabilitada e sem versão confiável, inclusive atualizações e reinstalações. A confiança é registrada para a versão instalada.

Pacotes/downloads têm limite de 100 MiB; arquivos ZIP, de 500 MiB expandidos e 10.000 entradas. Caminhos absolutos ou com travessia, aliases do Windows, caminhos duplicados, colisões de caixa, colisões entre arquivo/diretório, links simbólicos e arquivos especiais são rejeitados. Falhas de extração removem dados temporários. O empacotamento exclui arquivos ocultos e `target`, rejeita extensões sensíveis de chaves/bancos e links e exige entrada JavaScript válida.

## Descoberta e interface

O painel Plugins abre com `Ctrl+Shift+X` ou `graphite.plugins.open`. Sistema de arquivos, rede e verificação do Node executam em workers. O catálogo busca repositórios públicos com `topic:graphite-plugin`, pagina 30 resultados, mantém cache de 60 segundos e respeita cabeçalhos de limite de requisições. Não usa credenciais GitHub.

Selecionar um repositório consulta sua última release estável e lista assets `.graphite-plugin` e `.vsix`. Downloads passam pelo mesmo validador/instalador local. O usuário revisa identidade/versão antes de instalar e concede confiança separadamente antes de executar código nativo. Atualizações são escolhidas manualmente nas releases; não há atualização em segundo plano nem clone de repositórios. A instalação local funciona offline.

Os testes usam diretórios isolados, servidor HTTP local e exemplos reais em Node. Consulte [Desenvolvimento de plugins](../contributing/plugins.md) para manifesto, SDK e controles.
