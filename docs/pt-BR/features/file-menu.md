# Menu Arquivo e workspaces

O menu **Arquivo** da barra de título oferece ações locais para arquivos, editores, janelas, perfis e workspaces. A geometria do menu é compartilhada entre o ponteiro e a navegação por teclado. Pressione **F10** para abrir, use as setas para navegar, **Enter** para ativar e **Esc** para fechar ou voltar de um submenu.

## Arquivos e editores

- **Novo Arquivo de Texto** cria outra aba sem título. **Novo Arquivo**, **Abrir Arquivo** e **Abrir Pasta** usam o seletor nativo do sistema.
- **Salvar**, **Salvar Como**, **Salvar Tudo** e **Reverter Arquivo** atuam no editor atual pelo modelo de documentos do Graphite.
- **Salvamento Automático** começa desativado. Quando ativado, um arquivo nomeado com alterações é salvo após um segundo sem edição. A opção fica em `files.json` do perfil ativo.
- Fechar um editor ou uma janela usa o fluxo existente de salvar, descartar ou cancelar.

## Workspaces

O Graphite aceita arquivos `.code-workspace` no formato padrão, com uma lista `folders`. Os caminhos podem ser absolutos ou relativos ao arquivo do workspace. Use **Adicionar Pasta ao Workspace**, **Remover Pasta do Workspace** (remove a raiz do arquivo ativo ou, se não houver arquivo ativo, a última raiz), **Salvar Workspace Como** ou **Duplicar Workspace** para gerenciar workspaces com várias raízes. Explorador, abertura rápida e busca percorrem todas as raízes. Ao trocar de workspace, abre-se uma janela centralizada que lista os documentos com alterações não salvas e pergunta o que fazer antes de substituir as pastas: **Salvar tudo** (`S`), **Descartar** (`D`) ou **Cancelar** (`Esc`), também disponíveis como botões. A mesma janela aparece ao fechar a janela do app ou reverter um arquivo com alterações não salvas. Os caminhos recentes ficam no perfil ativo.

## Preferências e perfis

**Preferências** abre as configurações, atalhos, snippets, painel de extensões, configuração de tarefas e opções de tema do perfil. Configurações e atalhos são arquivos JSON editáveis. Tarefas abrem `.vscode/tasks.json`; se o arquivo não existir, o Graphite cria um arquivo vazio compatível com o VS Code. **Nova Janela com Perfil** inicia outro processo do Graphite com o perfil escolhido sem alterar o perfil ativo na janela atual.

Compartilhamento e sincronização online não fazem parte deste menu local.

Os submenus abrem ao lado da linha a que pertencem, assim que o mouse passa por ela, e podem se aninhar: **Perfis** e **Temas** abrem ao lado de **Preferências** sem substituí-lo. Esquerda ou Esc fecham um nível por vez; um submenu que não cabe à direita abre à esquerda.
