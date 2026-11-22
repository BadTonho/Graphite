# Recuperação de rascunhos

O Graphite guarda um rascunho local dos arquivos de texto modificados e das abas sem título. Quando uma sessão anterior deixa rascunhos, escolha **Revisar rascunhos** na abertura ou na paleta de comandos. Selecione um rascunho para restaurá-lo ou confirmar seu descarte. Escolha **Mais tarde** para deixá-lo disponível.

Quando o original ainda corresponde à versão em que o rascunho se baseou, o Graphite abre o texto recuperado como alterações não salvas no documento original. Se o arquivo mudou, desapareceu, ficou somente leitura ou conflita com edições abertas, o Graphite abre uma aba sem título separada. Use **Salvar** ou **Salvar Como** para gravar o texto recuperado. O salvamento automático aguarda até que as alterações recuperadas sejam salvas explicitamente ou descartadas.

A configuração **Arquivos: Recuperação de rascunhos** (`files.draftRecovery`) vem ativada e se aplica ao perfil do usuário. Desativá-la interrompe a gravação de novas cópias. Os rascunhos existentes continuam disponíveis para revisão e descarte. Os dados ficam na pasta `draft-recovery/` do perfil, fora da sincronização das configurações. Quem tem acesso aos seus arquivos de usuário pode conseguir ler esses dados.

Ao reiniciar, o Graphite consegue recuperar a última cópia concluída. Edições recentes podem ainda estar pendentes quando o processo encerra. A recuperação cobre apenas texto. Sessões de notebooks, editores personalizados, posições do cursor e histórico de desfazer ficam fora desta funcionalidade.
