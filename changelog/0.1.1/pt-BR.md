## Novas funcionalidades

- Conecte o Chat à OpenAI, ao Claude, ao OpenCode Console, ao OpenCode Go, ao Gemini, ao OpenRouter ou a um serviço compatível com OpenAI. As conversas mantêm o provedor e o modelo originais, as chaves usam o cofre do sistema e o contexto do editor pode ser anexado a mensagens individuais.
- Revise e recupere rascunhos locais de arquivos de texto não salvos e abas sem título após um encerramento inesperado.
- Escolha quais componentes instalar no Windows. Plugins VSIX, Copilot SDK e Foundry Local SDK podem ser selecionados separadamente e vêm marcados por padrão.

## Melhorias

- O painel Chat agora tem configuração guiada de conexões, botões de conversa identificados e um campo de mensagem com envio, cancelamento e anexos de contexto removíveis.
- Simplifique a barra lateral secundária com acesso à Estrutura pela paleta de comandos.

## Correções

- O token de login do GitHub agora fica no Gerenciador de Credenciais do Windows ou no Secret Service do Linux, em vez de texto puro na pasta do perfil. Tokens salvos por versões anteriores são movidos para lá automaticamente, e falhas ao manter o login são registradas no log de erros.
- O log de erros agora oculta tokens de login do GitHub (`gho_`) e outros tokens do GitHub, Google, GitLab, Slack e AWS, credenciais `Basic` e `x-api-key`, segredos `chave=valor` e senhas em URLs, que antes podiam ser gravados em disco. O log de erros e o relator de problemas agora compartilham um único conjunto de regras de ocultação.
- As ferramentas do Chat não conseguem mais acessar arquivos fora do workspace por um link simbólico ou junction: criar um arquivo dentro de uma pasta vinculada é recusado como a leitura, e a busca no workspace não segue mais links, o que também evita que ela entre em loop.
- Conversas do Chat e aprovações de ferramentas que não podem ser salvas agora são registradas no log de erros de severidade média em vez de se perderem em silêncio, comandos do Chat não gravam mais em um perfil isolado, e **Chat: Abrir Edição do Chat em Diff** informa quando o arquivo de comparação não pode ser criado.
- O Chat agora encerra a resposta com um erro quando um serviço de modelo envia um único evento maior que 8 MiB, em vez de consumir cada vez mais memória, e não fica mais lento em respostas cujas linhas chegam em muitos pedaços pequenos.
- Comandos do Chat no terminal não derrubam mais a resposta quando uma saída longa termina no meio de um caractere acentuado ou emoji, e não a travam mais para sempre: um comando que passa de 2 minutos, ou é interrompido com **Parar**, é encerrado junto com os processos que iniciou. A saída é limitada durante a leitura, e argumentos entre aspas agora chegam ao `cmd` no Windows exatamente como escritos.
- **Sempre permitir** no Chat agora vale apenas para o workspace atual e, para comandos no terminal, apenas para o comando exato aprovado. Antes, uma única aprovação permitia ao assistente executar qualquer comando ou editar qualquer arquivo em todos os workspaces. Workspaces restritos nunca lembram aprovações, e aprovações salvas por versões anteriores voltam a ser pedidas.
- Edições do Chat não sobrescrevem, descartam, desfazem nem refazem mais um arquivo cujo editor tem alterações não salvas, o que antes apagava essas alterações e o histórico de desfazer do editor. O modelo agora é informado se a edição foi realmente gravada, e falhas de gravação são informadas e registradas no log em vez de aparecerem como aplicadas. Plugins nativos que gravam arquivos do workspace têm a mesma proteção.
- Comandos `graphite plugin` que falham agora gravam uma única entrada no log de erros de severidade alta.
- Ícones SVG, bitmap e codicon contribuídos por plugins agora são renderizados na Barra de Atividades e na Barra Lateral Secundária em vez de exibir apenas a primeira letra do título do container.
- `vscode.window.tabGroups` (`ExtHostEditorTabs`) agora é inicializado e sincronizado no extension host VSIX para evitar falhas ao acessar `activeTabGroup` na ativação, a capacidade da fila do extension host foi ampliada para ativações concorrentes, e erros de ativação/execução de extensões passaram a ser gravados no log de erros de severidade média.
- Webviews de extensão agora são servidas em uma origem local `127.0.0.1` com `POST /__graphite_ipc` (evitando colisões com `window.ipc` somente leitura e bloqueios de `frame-ancestors` para iframes locais aninhados), e atualizações de recursos/intervalos de `MainThreadComments` são aceitas sem exibir erro.
- O Explorador de arquivos agora rola com a roda do mouse quando o ponteiro está sobre a barra lateral primária e exibe uma barra de rolagem quando os itens ultrapassam a área visível.
- Adicionado espaçamento horizontal entre a margem de números de linha do editor e o início do texto.
