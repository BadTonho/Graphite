# Testes de regressão

A suíte da etapa 5 consolida os contratos de documento/view, comandos, serviços, host e salvamento assíncrono. Suas fixtures usam diretórios temporários, dados simulados próprios e processos Rust locais, sem GPU, rede, arquivos pessoais ou servidor de linguagem instalado. O workspace completo também inclui testes de integração de plugins nativos e Emmet que exigem Node.js 24+, além de testes locais de processos/compilador/Git; instale os pré-requisitos de [compilação](building.md). Os testes de Code OSS/rede e ipykernel real são explicitamente ignorados por padrão.

## Executando a suíte

Execute na raiz do repositório:

```sh
cargo run -p graphite-xtask -- verify
```

O verificador local confere a formatação, executa o Clippy tratando avisos como erros, compila o workspace (incluindo auxiliares de processo) e executa os testes do workspace com quatro threads. Executa todas as etapas mesmo após falhas, preserva a saída dos comandos e apresenta durações e um resumo PASS/FAIL. Código de saída 0 significa que as quatro etapas automatizadas passaram; falhas ou ferramentas indisponíveis retornam 1, e argumentos inválidos retornam 2. Não instala ferramentas, reescreve arquivos-fonte, abre o app nem aciona a CI. O Cargo pode preencher seus caches normais; a primeira compilação totalmente offline não é garantida.

Exige os pré-requisitos existentes de compilação Rust/nativa e os componentes `rustfmt` e `clippy`. O comando é o mesmo no Windows e no Linux. O verificador não exige Python, PowerShell ou dependências Rust adicionais. Execute após concluir uma alteração; durante o desenvolvimento, use testes filtrados e, antes das versões, a CI manual existente.

Os testes ignorados continuam visíveis na saída do Cargo e não contam como aprovados. Uma execução bem-sucedida não valida o outro sistema operacional, integrações com serviços reais, interface/GPU ou aplicativo empacotado. Consulte a [matriz de verificação](verification-matrix.md), o [roteiro de verificação manual](manual-verification.md) e os [resultados locais registrados](verification-results.md).

Os testes do workspace compilam a fixture stdio `graphite-test-lsp` por seu alvo de integração. Antes de executar apenas os testes unitários da aplicação, compile os dois auxiliares de processo:

```sh
cargo build -p graphite-extension-host
cargo build -p graphite-app --bin graphite-test-lsp
cargo test -p graphite-app --bin graphite
```

Filtros úteis para o último comando são `document_view_tests`, `accessibility_actions::tests`, `services::tests`, `workspace_edit::tests`, `command_tests`, `lsp::stdio_tests` e `saving::`. Execute `cargo test -p graphite-extension-host` para enquadramento, supervisor e testes do processo real do host, e `cargo test -p graphite-text` para contratos de transações e gravação de arquivos.

O binário da fixture pertence à infraestrutura de testes da aplicação e nunca é iniciado pela configuração padrão do editor. Implementa apenas comandos de teste programados e o enquadramento LSP padrão. Não é um servidor de linguagem para usuários finais.

## Verificações de estabilidade no Windows

- `cargo test -p graphite-app --bin graphite interrupted_save_preserves_last_complete_version_on_reopen` encerra um processo do editor sem interface depois que o salvamento sincroniza o arquivo temporário e antes de substituir o documento. Uma nova instância abre o último salvamento concluído, confere o texto e as quebras de linha e salva outra alteração. A recuperação automática de buffers não salvos após um encerramento inesperado do processo segue pendente.
- `cargo test -p graphite-app --bin graphite window_close_buttons_save_discard_or_keep_edits` exercita o tratamento de cliques do próprio diálogo e verifica o arquivo no disco e se a janela seria fechada para cada opção. Use o [roteiro manual](manual-verification.md) para verificar entrada física, renderização e o aplicativo instalado.
- `cargo test -p graphite-app --test startup_update` inicia três vezes uma cópia descartável do executável, rejeita uma atualização preparada cujo checksum não confere e verifica que o executável original pode continuar em uso. Defina `GRAPHITE_TEST_PACKAGED_EXE` com o caminho absoluto do `graphite.exe` extraído do pacote de lançamento ou instalado pelo assistente para aplicar a mesma verificação a esse executável.
- Em uma conta descartável do Windows, defina `GRAPHITE_TEST_WINDOWS_INSTALLER` com o caminho do instalador real e execute `python -m unittest discover -s packaging/release/tests -p test_windows_install.py -v`. O teste instala em uma pasta temporária, verifica o runtime incluído, o atalho, o registro de desinstalação e a primeira execução pela CLI, e então executa o desinstalador e verifica a remoção. O instalador real escreve no menu Iniciar e no registro dessa conta.

## Medindo a cobertura de código

Com `cargo-llvm-cov` e o componente `llvm-tools-preview` da toolchain Rust disponíveis, execute `cargo llvm-cov --workspace --locked --html --output-dir target/graphite-coverage`. Abra `target/graphite-coverage/html/index.html` para ver as linhas executadas e investigar lacunas por arquivo. A cobertura depende do sistema operacional e dos cenários executados, incluindo quais testes opcionais rodaram.

## Matriz de cobertura

Os nomes abaixo identificam módulos de testes nativos ou testes individuais de regressão; a cobertura existente é reutilizada em vez de duplicada.

| Requisito | Evidência automatizada |
|---|---|
| Seleções, scroll, IME e multicursor independentes | `document_view_tests`, especialmente `multicursor_transactions_noop_and_history_keep_peer_state_independent`; `accessibility_actions::tests::accessible_multicursor_edit_preserves_focus_scroll_and_peer_ime` |
| Clipboard, busca/substituição, completion e edições de linha | `search_completion_clipboard_and_line_commands_remap_peer_views`; testes de equivalência em `command_tests::parity` |
| Origem do histórico, origem fechada, fechamento/reabertura e retenção | `undo_restores_origin_selections_without_changing_focus_or_scroll`, `closed_origin_is_not_resurrected_by_undo_and_reopened_view_has_new_identity`; `workspace_edit::tests` |
| Versões, no-op, redo e texto/formato salvos | Testes de transação de `graphite-text`; `unchanged_save_keeps_revision_and_redo_history`; `saved_text_and_format_baselines_survive_undo_redo_and_format_roundtrip` |
| Edições UTF-16 atômicas em abertos/fechados/untitled | `services::tests::mixed_open_closed_untitled_batch_publishes_only_after_success`; `stdio_apply_edit_uses_event_loop_transactions_and_rejects_partial_batches` |
| Rejeição preserva texto, histórico, views, eventos, retenção e disco | `rejected_batches_preserve_views_redo_events_leases_and_disk` cobre falhas de leitura, sobreposição, divisão de surrogate, versões obsoletas e documentos read-only |
| Mudanças entre preparação e commit | `workspace_edit::tests` cobre mudanças de versão em memória, conteúdo de arquivo fechado, permissões, alvos recém-abertos e preparação cancelada |
| Inventário de comandos, atalhos, metadados e argumentos | `command_tests::inventory_preserves_every_legacy_id_and_default_binding`, testes de registro duplicado e argumentos de atalhos |
| Precedência e ciclo de vida dos contextos, enablement atual e visibilidade separada | Testes de escopos, enablement e widgets de `command_tests`; `readonly_widget_rejection_preserves_suggestions_selection_and_history` |
| Edições equivalentes por teclado, widget e API | `keyboard_widget_and_programmatic_completion_have_identical_document_effects` compara texto, versões, seleções das duas views, foco e scroll |
| Cancelamento assíncrono, falhas e conclusão única | Testes de workers de `command_tests`; `lsp::requests::tests`; testes de requisições pendentes do host |
| Inicialização do worker LSP real e resultados JSON | `lsp::stdio_tests::stdio_initialization_execute_command_and_malformed_response` |
| LSP applyEdit, respostas inválidas, cancelamento, respostas tardias e EOF | `lsp::stdio_tests`; `lsp::command_tests` com canais simulados também cobre instâncias obsoletas/substituídas e transações pelos serviços |
| Ordem de participantes, timeout, resultados inválidos e digitação durante preparação | `saving::tests`; `typing_and_format_change_during_host_participant_save_latest_snapshot`; testes de participantes LSP |
| Digitação durante escrita, filas por documento e documentos concorrentes | `saving::tests`; `saving::view_tests::separate_documents_write_concurrently_without_waiting_for_other_document` |
| Falhas de escrita, permissões, encoding/EOL e limpeza de temporários | `saving::tests`, `saving::view_tests`, testes de formato e testes de arquivos de `graphite-text` |
| Falha parcial em Salvar tudo | `save_all_keeps_successful_file_and_dirty_untitled_after_partial_failure` verifica que gravações bem-sucedidas permanecem confirmadas e views untitled continuam alteradas/abertas |
| Fechamento pendente, decisões de workspace, descarte e CLI --wait | `saving::lifecycle_tests`; testes de decisões de comandos |
| Queda real do host, limpeza, reinício e continuidade do editor/LSP | `graphite-extension-host/tests/process.rs`; `lsp::command_tests::real_host_crash_cleans_resources_but_editor_and_lsp_continue_then_restart`; testes de queda do host no salvamento |

## Determinismo e asserções de falha

Testes de prazos passam valores controlados de `Instant` aos rastreadores e ao polling do coordenador; não aguardam um ou trinta segundos para provocar expiração. Testes de workers usam canais para pausar antes/depois de assumir o commit. Testes de protocolo sincronizam por respostas e confirmações ordenadas no fluxo. Limites de espera/polling detectam travamentos; tempo decorrido não comprova conclusão de uma operação.

Testes de rejeição verificam estado observável e assinaturas e depois exercitam redo para comprovar a preservação do histórico. Edições de workspace bem-sucedidas permanecem em memória até salvar explicitamente. Salvar tudo é intencionalmente atômico por documento: uma falha não desfaz outras gravações bem-sucedidas. Cancelar um solicitante não reverte uma escrita cujo commit já foi assumido.

## Plataformas e verificações manuais

Esta etapa é validada com testes automatizados nativos e compilação no Windows. Execução no Linux e verificações interativas de UI/GPU no Windows/Linux não foram realizadas nesta sessão Windows. Testes de árvores/ações de acessibilidade e estado IME sem janela não substituem verificações de tecnologia assistiva, método de entrada ou renderização do sistema.

A verificação manual posterior deve conferir digitação e IME em views divididas, integração com clipboard, navegação acessível, scroll independente, avisos de salvamento visíveis, diálogos salvar/descartar/cancelar e responsividade da janela durante salvamento. Registre o sistema operacional, as verificações executadas e falhas, sem marcar verificações indisponíveis como aprovadas.

Antes da entrega, revise `git status`, diff completo, conteúdo staged e `.gitignore` em busca de segredos e artefatos temporários. `sugestoes.md` permanece como acompanhamento local ignorado; a matriz de cobertura é documentação versionada. Validação não implica commit ou push.

O host Code OSS/Node real, sua versão fixada, teste com extensão real e compatibilidade `.vsix` permanecem como trabalho posterior obrigatório. Esta suíte não declara compatibilidade com eles.
