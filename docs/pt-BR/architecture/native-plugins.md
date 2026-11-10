# Plugins nativos e inspeção de VSIX

Plugins da API Graphite v1 executam JavaScript em um processo Node.js 24+ do sistema por plugin. TypeScript é compilado antes do empacotamento. `graphite-plugins` cuida de manifestos, validação ZIP, instalação e geração de projetos sem depender da aplicação/GPU. A aplicação mantém o estado da barra lateral de Plugins e o roteamento das sessões; os serviços existentes aplicam mudanças de documentos e comandos no loop de eventos.

## Limite do runtime

O host CommonJS incluído carrega a entrada JavaScript e chama `activate(context, graphite)`. São aceitos módulos ES e exports CommonJS. `deactivate()` é opcional. O host reutiliza enquadramento, handshake, filas limitadas, prazos, cancelamento e descarte do protocolo v2 de [Serviços e host simulado](services-host.md). Esse RPC é do Graphite, não do Code OSS. Stdout é reservado às mensagens; a saída do console vai para stderr.

Cada processo tem supervisor, comandos, inscrições de documentos, IDs de requisição e sessão próprios. A aplicação encaminha chamadas ao plugin responsável e mantém o simulador separado. Comandos devem corresponder ao manifesto e não podem substituir comandos internos. A API v1 expõe comandos, consultas/eventos/edições versionadas de documentos, consultas de workspace/configuração e mensagens. Não expõe painéis arbitrários, webviews, participantes de salvamento, execução de comandos, salvamento de arquivos ou foco de views. Mudanças preservam validação UTF-16, somente leitura, edições atômicas e undo.

Desabilitar, substituir ou remover um plugin libera seus registros e trabalhos pendentes sem encerrar outros hosts ou LSP. Reiniciar manualmente cria nova sessão. Falhas não provocam reinício automático. O encerramento normal oferece um prazo limitado para `deactivate()` antes de terminar o processo. O processo separado contém falhas, mas não isola código Node malicioso. Habilite apenas código confiável.

## Pacotes e persistência

Pacotes nativos são ZIPs `.graphite-plugin`, com `graphite-plugin.json` na raiz. VSIX é lido por `extension/package.json`. A instalação nunca executa npm, scripts de ciclo de vida ou código do pacote. A barra lateral de Plugins pesquisa no Open VSX, baixa o VSIX mais recente, valida o pacote e pede confirmação antes de instalar. VSIX fornece somente metadados até que o host Code OSS e sua ponte de API sejam integrados. Lojas e aprovações ficam separadas por perfil.

O diretório de configuração do usuário contém `plugins/`, registros JSON por plugin e diretórios imutáveis de versões. A instalação valida e extrai para um diretório temporário novo e substitui atomicamente o registro. Uma falha na publicação preserva o registro e pacote anteriores. Diretórios anteriores permanecem disponíveis aos hosts ativos até a remoção; esta respeita o limite exato da identidade do plugin. Toda instalação bem-sucedida começa desabilitada e sem versão confiável, inclusive atualizações e reinstalações. A confiança é registrada para a versão instalada. Atualizações do Open VSX conferem identidade e versão do arquivo baixado contra os metadados do registro antes de substituir o registro instalado.

Pacotes/downloads têm limite de 100 MiB; arquivos ZIP, de 500 MiB expandidos e 10.000 entradas. Caminhos absolutos ou com travessia, aliases do Windows, caminhos duplicados, colisões de caixa, colisões entre arquivo/diretório, links simbólicos e arquivos especiais são rejeitados. Falhas de extração removem dados temporários. O empacotamento exclui arquivos ocultos e `target`, rejeita extensões sensíveis de chaves/bancos e links e exige entrada JavaScript válida.

## Descoberta e interface

A barra lateral de Plugins abre com `Ctrl+Shift+X` ou `graphite.plugins.open`. Ela reutiliza a barra lateral de 260 px ao lado da barra de atividades, mantendo o editor visível. A barra lateral só recebe entrada de teclado enquanto está focada; o editor, Quick Open, localizar, pesquisa do projeto, Controle de código, terminal e superfícies de confirmação têm precedência caso contrário. Sistema de arquivos, rede e verificação do Node executam em workers. O cliente Open VSX carrega até 30 resultados por busca e usa HTTPS para o registro público.

Selecionar um resultado do Open VSX baixa a última versão para um arquivo temporário que será inspecionado. Instalação e atualização de VSIX usam o validador local de pacotes e exigem confirmação. Atualizações redefinem habilitação e confiança por versão. Plugins Graphite nativos ainda exigem confiança explícita por versão e confiança do workspace; revogar a confiança do workspace encerra hosts ativos e tarefas. A ativação de VSIX Code OSS continua indisponível até que a ponte do host esteja pronta. A instalação de pacotes locais continua disponível offline.

Os testes usam diretórios isolados, servidor HTTP local e exemplos reais em Node. Consulte [Desenvolvimento de plugins](../contributing/plugins.md) para manifesto, SDK e controles.
