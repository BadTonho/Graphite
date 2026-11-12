# Compatibilidade com extensões do VS Code

O Graphite inicia o host de extensões oficial do Code OSS **1.145.2** (commit `c4453f8ea430795f941c1b2ff6c9321135a74a02`) em um processo separado do Node.js 24.22.1. Os pacotes portáteis Windows e Linux x86-64 incluem o runtime e os avisos de licença. O build comum do Cargo permite inspecionar e instalar VSIX sem preparar o runtime.

## Runtime e autorização

O processo do Graphite troca mensagens com limites de tamanho por canais privados do processo filho. O adaptador Node usa `PersistentProtocol` e `RPCProtocol` do upstream por um socket local ou pipe nomeado por sessão. Ele faz o handshake oficial e converte as chamadas. O Rust controla as operações do editor, o registro, as versões dos documentos, os diagnósticos, as caixas de diálogo e o ciclo de vida.

O painel Plugins instala os pacotes VSIX desabilitados. A instalação aprovada, a confiança do workspace e o perfil ativo determinam quais pacotes o host recebe. O primeiro comando elegível da paleta inicia o host. Comandos declarados aparecem na paleta antes da ativação. Comandos registrados, provedores, diagnósticos e solicitações pendentes pertencem à sessão. Revogar a confiança, trocar de workspace ou perfil ou alterar instalações habilitadas encerra a sessão. O processo roda com as permissões do usuário, então aprove apenas código em que você confia.

## Matriz de APIs da Fase 5

| Namespace | Fluxo suportado |
|---|---|
| `commands` | Ativar um comando declarado sob demanda, registrar e remover comandos, executar comandos registrados de extensões e chamar comandos do Graphite. |
| `workspace` | Ler pastas e documentos, observar eventos de abertura, alteração, salvamento e fechamento, aplicar edições de texto versionadas com desfazer e ler configurações. Alterações de configuração VSIX e seus eventos são isolados por perfil e workspace. |
| `window` | Ler o editor e as seleções ativas, mostrar documentos e exibir mensagens, progresso, caixas de entrada e listas de seleção. Pressione Esc para cancelar uma entrada ou seleção. |
| `languages` | Registrar provedores de sugestões, hover, formatação de documentos, definição, declaração, definição de tipo, implementação, referências, símbolos de documento, code actions, renomear e ajuda de assinatura, e publicar diagnósticos em Problemas. Os comandos do editor correspondentes (F12, Ir para Declaração, Definição de Tipo, Implementações, Referências, Ir para Símbolo no Editor, Correção Rápida, Renomear Símbolo, Dicas de Parâmetro) usam primeiro um provedor ativo. Definição, referências, implementação, renomear e ajuda de assinatura voltam ao language server quando não há provedor; a Correção Rápida mantém o language server quando há um configurado. Os resultados dos provedores são convertidos para o formato LSP em `language_results.rs`. |

A matriz cobre os fluxos suportados na revisão fixada. Extensões que dependem de outras APIs do editor podem receber erros de operação indisponível. Temas, atalhos e outras contribuições declarativas VSIX podem ser inspecionados. Webviews, depuração e Test Explorer pertencem à Fase 6. O Open VSX é o registro público usado pelo Graphite.

Veja [builds portáteis](../contributing/building.md), [instalação de plugins](../contributing/plugins.md) e [validação da v0.5](../features/v0.5-extensions-automation.md).
