# Resultados de verificação local

## Execução em 2026-11-16

Windows, commit-base `8035ce5` mais as alterações desta implementação na árvore de trabalho. Ferramentas: Rust/Cargo 1.97.0, Node.js 24.18.0, Git 2.53.0.windows.1. Comando: `cargo run -p graphite-xtask -- verify`. O verificador executa `cargo test --workspace --no-fail-fast -- --test-threads=4` para que a falha de um alvo não impeça os demais.

| Verificação | Resultado | Evidência |
|---|---|---|
| Formatação | FAIL | Diferenças preexistentes em dez arquivos de auth, core-api, plugins, syntax e text |
| Clippy, avisos como erros | FAIL | Código/variáveis sem uso, complexidade de tipos, lifetimes desnecessários e lints de loops/alocações preexistentes; compilação interrompida antes de analisar todos os crates |
| Compilação do workspace | PASS | `cargo build --workspace`, com avisos preexistentes |
| Testes do workspace | PASS | 1.761 aprovados, 0 falhas, 10 ignorados, somando os resumos de todos os alvos de teste |
| Clippy do próprio verificador | PASS | `cargo clippy -p graphite-xtask --all-targets -- -D warnings` |
| Verificador geral | FAIL (saída 1) | Formatação e Clippy continuam reprovados |
| Interface/GPU interativa e pacote Windows | PENDING | Somente verificações headless; não houve teste interativo do app/pacote |
| Compilação/testes/interface Linux | PENDING | Este ambiente de execução é Windows |

A saída bruta foi capturada localmente em `target/local-verification.txt`, ignorado pelo Git. É um artefato local, não um relatório versionado de lançamento. Reexecute o verificador na revisão atual; este registro é uma observação datada, não um indicador atualizado continuamente.

## Problemas tratados

A primeira execução encontrou quatro testes do app falhando. Instâncias isoladas de `App` ainda apontavam para a configuração real, causando gravações negadas e falha na persistência do tema selecionado. Apps headless isolados agora possuem perfis temporários independentes durante sua vida, com regressão de independência e limpeza. A fixture Emmet prepara seu worker em perfil temporário, sem usar o perfil do usuário. Os quatro testes inicialmente reprovados passaram na execução final do workspace.

Foram adicionados testes observáveis de persistência/recarga das configurações, rejeição sem sobrescrever configuração corrompida, recuperação após reparo, preservação dos dados em falha de gravação e mensagens/notificações sanitizadas gravadas em cada severidade. Foram reutilizados os testes existentes de sucesso e falha de salvamento/fechamento/histórico/busca/atualização. Quatro testes do verificador cobrem subprocessos reais, sucesso, falha, continuidade, programas indisponíveis, códigos de saída e compilação antes dos testes.

## Pendências priorizadas

1. Resolver os problemas preexistentes de formatação e Clippy e reexecutar as verificações estritas. Não suprimir avisos globalmente nem declarar aprovação geral antes de todas as etapas passarem.
2. Concluir as [verificações manuais](manual-verification.md) dos fluxos essenciais e pacotes reais em Windows/Linux; executar a CI manual existente na revisão de lançamento.
3. Preparar fixtures Code OSS e executar explicitamente os nove testes VSIX ignorados; dois envolvem amostra pública/rede. Preparar `GRAPHITE_TEST_KERNEL_PYTHON` com ipykernel para o décimo ignorado. Usar filtros individuais com `--ignored`, sem executar indiscriminadamente uma suíte dependente de rede.
4. Ampliar a cobertura além dos testes representativos da [matriz](verification-matrix.md), começando pelo despacho dos comandos `list.*`, sem testes diretos no handler. Revisar separadamente interoperabilidade com serviços reais e cada produtor de erros.

Os gatilhos da CI, a versão do app e os changelogs publicados não foram alterados. A versão 0.1.0 continua sendo o lançamento inicial sem changelog.
