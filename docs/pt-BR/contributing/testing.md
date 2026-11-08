# Testes de regressão

A suíte da etapa 5 consolida os contratos de documento/view, comandos, serviços, host e salvamento assíncrono. Os testes usam diretórios temporários, dados simulados próprios e processos Rust locais. Não exigem GPU, rede, arquivos pessoais, Node ou servidor de linguagem instalado.

## Executando a suíte

Execute na raiz do repositório:

```sh
cargo test --workspace
cargo build --workspace
cargo fmt --all -- --check
```

Os testes do workspace compilam a fixture stdio `shoppe-test-lsp` por seu alvo de integração. Antes de executar apenas os testes unitários da aplicação, compile os dois auxiliares de processo:

```sh
cargo build -p shoppe-extension-host
cargo build -p shoppe-app --bin shoppe-test-lsp
cargo test -p shoppe-app --bin graphite
```

Filtros úteis para o último comando são `document_view_tests`, `accessibility_actions::tests`, `services::tests`, `workspace_edit::tests`, `command_tests`, `lsp::stdio_tests` e `saving::`. Execute `cargo test -p shoppe-extension-host` para enquadramento, supervisor e testes do processo real do host, e `cargo test -p shoppe-text` para contratos de transações e gravação de arquivos.

O binário da fixture pertence à infraestrutura de testes da aplicação e nunca é iniciado pela configuração padrão do editor. Implementa apenas comandos de teste programados e o enquadramento LSP padrão. Não é um servidor de linguagem para usuários finais.

## Matriz de cobertura

Os nomes abaixo identificam módulos de testes nativos ou testes individuais de regressão; a cobertura existente é reutilizada em vez de duplicada.

| Requisito | Evidência automatizada |
|---|---|
| Seleções, scroll, IME e multicursor independentes | `document_view_tests`, especialmente `multicursor_transactions_noop_and_history_keep_peer_state_independent`; `accessibility_actions::tests::accessible_multicursor_edit_preserves_focus_scroll_and_peer_ime` |
| Clipboard, busca/substituição, completion e edições de linha | `search_completion_clipboard_and_line_commands_remap_peer_views`; testes de equivalência em `command_tests::parity` |
| Origem do histórico, origem fechada, fechamento/reabertura e retenção | `undo_restores_origin_selections_without_changing_focus_or_scroll`, `closed_origin_is_not_resurrected_by_undo_and_reopened_view_has_new_identity`; `workspace_edit::tests` |
| Versões, no-op, redo e texto/formato salvos | Testes de transação de `shoppe-text`; `unchanged_save_keeps_revision_and_redo_history`; `saved_text_and_format_baselines_survive_undo_redo_and_format_roundtrip` |
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
| Falhas de escrita, permissões, encoding/EOL e limpeza de temporários | `saving::tests`, `saving::view_tests`, testes de formato e testes de arquivos de `shoppe-text` |
| Falha parcial em Salvar tudo | `save_all_keeps_successful_file_and_dirty_untitled_after_partial_failure` verifica que gravações bem-sucedidas permanecem confirmadas e views untitled continuam alteradas/abertas |
| Fechamento pendente, decisões de workspace, descarte e CLI --wait | `saving::lifecycle_tests`; testes de decisões de comandos |
| Queda real do host, limpeza, reinício e continuidade do editor/LSP | `shoppe-extension-host/tests/process.rs`; `lsp::command_tests::real_host_crash_cleans_resources_but_editor_and_lsp_continue_then_restart`; testes de queda do host no salvamento |

## Determinismo e asserções de falha

Testes de prazos passam valores controlados de `Instant` aos rastreadores e ao polling do coordenador; não aguardam um ou trinta segundos para provocar expiração. Testes de workers usam canais para pausar antes/depois de assumir o commit. Testes de protocolo sincronizam por respostas e confirmações ordenadas no fluxo. Limites de espera/polling detectam travamentos; tempo decorrido não comprova conclusão de uma operação.

Testes de rejeição verificam estado observável e assinaturas e depois exercitam redo para comprovar a preservação do histórico. Edições de workspace bem-sucedidas permanecem em memória até salvar explicitamente. Salvar tudo é intencionalmente atômico por documento: uma falha não desfaz outras gravações bem-sucedidas. Cancelar um solicitante não reverte uma escrita cujo commit já foi assumido.

## Plataformas e verificações manuais

Esta etapa é validada com testes automatizados nativos e compilação no Windows. Execução no Linux e verificações interativas de UI/GPU no Windows/Linux não foram realizadas nesta sessão Windows. Testes de árvores/ações de acessibilidade e estado IME sem janela não substituem verificações de tecnologia assistiva, método de entrada ou renderização do sistema.

A verificação manual posterior deve conferir digitação e IME em views divididas, integração com clipboard, navegação acessível, scroll independente, avisos de salvamento visíveis, diálogos salvar/descartar/cancelar e responsividade da janela durante salvamento. Registre o sistema operacional, as verificações executadas e falhas, sem marcar verificações indisponíveis como aprovadas.

Antes da entrega, revise `git status`, diff completo, conteúdo staged e `.gitignore` em busca de segredos e artefatos temporários. `sugestoes.md` permanece como acompanhamento local ignorado; a matriz de cobertura é documentação versionada. Validação não implica commit ou push.

O host Code OSS/Node real, sua versão fixada, teste com extensão real e compatibilidade `.vsix` permanecem como trabalho posterior obrigatório. Esta suíte não declara compatibilidade com eles.
