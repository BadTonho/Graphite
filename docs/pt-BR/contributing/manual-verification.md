# Verificação manual

Execute primeiro `cargo run -p graphite-xtask -- verify` e registre as falhas. Use um workspace descartável e um perfil Graphite isolado (uma conta descartável do sistema para verificar pacotes); nunca use arquivos pessoais ou credenciais como fixtures. As verificações ficam pendentes até alguém registrar sistema operacional, revisão do app, pacote/compilação local, cenário e resultado observado. Um item desmarcado não é aprovado.

## Verificações essenciais

- [ ] Inicie o aplicativo, abra uma pasta temporária, redimensione a janela e use teclado e mouse. Confirme ausência de corrupção visual ou erros de inicialização.
- [ ] Crie um arquivo com acentos, emoji e várias linhas; edite, desfaça/refaça, salve, feche e reabra. Confirme o conteúdo exato salvo.
- [ ] No workspace descartável, salve uma alteração, edite o arquivo novamente e encerre à força aquele processo do Graphite antes do segundo salvamento. Reabra o arquivo e confirme a integridade da última versão salva. Registre se a edição posterior pode ser recuperada.
- [ ] Busque e substitua texto, incluindo regex e busca sem resultados; confirme as alterações e o resultado de desfazer.
- [ ] Abra views divididas, digite com IME, role independentemente e copie/cole pela área de transferência do sistema. Confirme consistência da outra view.
- [ ] Feche um documento alterado usando Salvar, Descartar e Cancelar em tentativas separadas. Confirme conteúdo salvo, preservação ao cancelar e janela responsiva.
- [ ] Altere uma configuração e reinicie. Confirme persistência. No perfil descartável, introduza JSON inválido e confirme erro compreensível e preservação do arquivo; repare e confirme recuperação.
- [ ] Abra um terminal, execute um comando inofensivo, interrompa e feche. Confirme entrada, saída, redimensionamento e encerramento do shell.
- [ ] Provoque uma falha segura, como abrir um arquivo temporário inexistente. Inspecione o arquivo do nível correspondente em `<app_config_dir>/logs/errors/`, verificando JSONL válido e erro apropriado. Nunca provoque um crash no perfil pessoal nem insira segredos reais.

## Verificações após mudanças relacionadas

- [ ] Explorer: criar/renomear/copiar/mover/excluir; cancelar exclusão de arquivo alterado. Confirme consistência entre disco e abas.
- [ ] Temas, ícones, folding, menus, atalhos, foco e acessibilidade: confirme aparência, alvos do ponteiro e comportamento com leitor de tela/IME.
- [ ] Git em repositório descartável: modificar, preparar, criar commit, trocar branch e resolver conflito. Confirme status, diff e disco.
- [ ] Linguagem com servidor local configurado: completion, navegação, rename e formatação; encerre o servidor e confirme recuperação do editor.
- [ ] Depure programa descartável com adaptador configurado: breakpoint, passo, parada, falha de tarefa prévia e cancelamento.
- [ ] Extensões empacotadas: instale VSIX local de teste conhecido, confie, ative, invoque, desative e reinicie. Confira providers, eventos de terminal, webviews e Test Explorer com a [lista Windows](windows-manual-testing.md).
- [ ] Notebooks: abrir/salvar, editar células, executar/cancelar com kernel descartável preparado, conferir saídas e encerramento.
- [ ] Chat, login, sync e remoto: use somente contas/serviços de teste preparados explicitamente; confirme cancelamento, erros offline e recuperação de conflitos. Testes simulados locais não comprovam interoperabilidade real.

## Antes de uma versão

- [ ] Execute a CI manual existente em Windows e Linux na revisão de lançamento; registre os resultados.
- [ ] Compile e abra o distribuível real em cada sistema conforme [compilação](building.md) e repita as verificações essenciais.
- [ ] No Windows, execute o instalador em uma conta descartável, abra o Graphite pelo atalho do menu Iniciar e rode o [teste de atualização na inicialização](testing.md#verificações-de-estabilidade-no-windows) com o executável instalado. Desinstale e confira se o atalho e os arquivos do aplicativo foram removidos.
- [ ] Com instalações descartáveis e artefatos de atualização controlados, verifique atualização, reinício, versão, rollback e rejeição de pacote inválido. Não use o executável de desenvolvimento em execução como alvo.
- [ ] No Linux, confira explicitamente X11/Wayland e ciclo de vida GTK/WebKit; no Windows, WebView2 e integração com shell. Testes headless não validam esses pontos.

Registre cada resultado como PASS, FAIL ou PENDING, incluindo motivo e encaminhamento de falhas ou ambientes indisponíveis. Vincule cada defeito a um teste de regressão quando corrigido.
