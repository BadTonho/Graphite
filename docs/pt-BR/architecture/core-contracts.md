# Contratos do Núcleo do Editor

Este documento descreve três contratos que devem ser formalizados enquanto o código ainda é pequeno: **separação model/view**, o **sistema de comandos/contexto** e a **fronteira de plugins**. É uma orientação de projeto, não uma descrição da implementação atual. As referências a arquivos e linhas refletem o código da v0.3.

"Formalizar" aqui não significa reescrever a aplicação nem copiar a arquitetura inteira do VS Code. Significa definir identidades, interfaces e invariantes agora, para que cada subsistema evolua sem forçar mudanças nos outros. Hoje várias partes da aplicação dependem de convenções (por exemplo, todo chamador precisa lembrar de executar `snapshot_editor_view`) em vez de garantias impostas por tipos.

## 1. Model/View: documento não é editor

### A distinção

- **Document (model):** texto, caminho, linguagem, versão, estado "sujo", histórico de undo e sincronização com LSP.
- **View (editor):** seleções/cursores, scroll, folding, quebra de linha, decorações e foco.
- A relação é **1 documento : N views**. É isso que permite split editor, diff, "peek definition", preview de busca e o mesmo documento exibido em vários lugares.

### Como está hoje

- `Tab` (`crates/graphite-app/src/main.rs`) mistura as duas coisas: guarda `buffer`, `scroll_line`, `language`, `highlights`, `diagnostics` e `indentation`.
- O `Buffer` (`crates/graphite-text/src/lib.rs`) é dono de `selection` e `extra_selections`. Ou seja, a seleção está no modelo.
- O `View` (`crates/graphite-app/src/app/editor_groups.rs`) tem `selections` e `scroll_line`, mas são cópias sincronizadas manualmente. `snapshot_editor_view` copia do buffer para a view, e `restore_editor_view` copia de volta. A mesma duplicação existe entre `Tab::scroll_line` e `View::scroll_line`.
- Consequência: dois grupos mostrando o mesmo documento não conseguem manter cursores independentes, porque a fonte da verdade é o `Buffer`. Trocar de grupo sobrescreve a seleção do outro. Todo novo caminho de mutação (formatador LSP, edições de extensão, replace) precisa lembrar de fazer snapshot e restore na hora certa.

O tipo `View` já aponta na direção certa; o que falta é ele ser **dono** do estado de view, em vez de espelhá-lo.

### Modelo alvo

```rust
struct DocumentId(u64);

struct Document {
    text: Rope,
    version: u64,        // incrementa a cada edição
    path: Option<PathBuf>,
    language: Option<Language>,
    dirty: bool,
    undo: EditStack,     // undo/redo pertence ao documento
}

struct EditorView {
    document: DocumentId,
    selections: Vec<Selection>, // independentes por view
    scroll_line: usize,
    // folding, decorations, wrapping...
}
```

### Invariantes

1. Toda mutação de texto incrementa `version` exatamente uma vez.
2. Todas as views do mesmo documento observam o mesmo texto e a mesma versão.
3. Seleções são por view. Após uma edição, cada view mapeia suas seleções usando o mapeamento de posição da edição (o projeto já tem `map_position` em `graphite-text`).
4. Fechar uma view não fecha o documento enquanto outra view ainda o referencia.
5. Undo/redo pertence ao documento, não a uma view.
6. A sincronização com LSP é indexada por documento e versão, não por aba.

### Por que adiar isso dói

Quase todo caminho de entrada, desenho e comando hoje assume `self.tabs[self.active_tab].buffer.selection`. Introduzir splits, diffs, peek ou edições vindas de extensão exigiria mudar todos eles ao mesmo tempo. Bugs como "o cursor pulou", "a seleção do outro painel mudou" e "as posições de scroll se desalinharam" são difíceis de reproduzir e fáceis de reintroduzir. Separar o estado depois é um refactor transversal; separar agora é trabalho localizado.

### Primeiro passo de migração

Mover `selection` e `extra_selections` de `Buffer` para `View`. Fazer as operações de texto receberem as seleções (ou uma transação contendo a seleção antes e depois da edição). Introduzir um `DocumentStore` no `App`. Manter `Tab` temporariamente como um par `(DocumentId, ViewId)` para evitar uma reescrita total.

## 2. Command/Context: comandos nomeados e contexto central

### A distinção

- **Comando:** uma ação com ID estável (`workbench.action.files.save`), metadados (título, categoria, enablement) e um handler.
- **Contexto:** o conjunto de chaves que determina se um comando, keybinding ou menu está ativo (`editorTextFocus`, `editorHasSelection`, `resourceLangId` etc.).

### Como está hoje

- `execute_custom_command` (`crates/graphite-app/src/app/commands.rs`) é um `match` grande que muta `self` diretamente.
- As chaves de contexto são montadas à mão em `handle_keybinding_stroke` como um `HashMap<String, String>` de booleanos hardcoded. Cada novo contexto exige editar essa função.
- O parser de `when` já existe (`crates/graphite-app/src/keybindings/when.rs`) e é uma boa base, mas só é usado para keybindings, não para enablement de comandos, menus ou paleta.
- Não existe um registro de comandos. Como resultado, não há paleta de comandos, introspecção, argumentos, valores de retorno nem enablement. O cliente LSP já precisa disso: `CodeActionOrCommand::Command` (`crates/graphite-app/src/app/editor_commands.rs`) carrega um ID e argumentos que exigem um registro para serem executados.

### Modelo alvo

```rust
struct CommandDescriptor {
    id: CommandId,                 // string compatível com VS Code
    title: &'static str,
    category: Option<&'static str>,
    enablement: Option<when::Expr>,
    handler: CommandHandler,
}

// Um único ponto de entrada para todos os chamadores:
fn execute(&mut self, id: CommandId, args: serde_json::Value)
    -> Result<serde_json::Value, CommandError>;
```

Com isso:

- Keybindings, paleta, menus, LSP `workspace/executeCommand`, extensões e testes usam o mesmo caminho.
- `ContextKeyService` passa a ser a única fonte das chaves de contexto, alimentado por documentos, views, foco e widgets abertos. O `when` é usado por keybindings **e** por enablement de comandos.
- Os metadados alimentam automaticamente a paleta de comandos e uma futura UI de keybindings.
- Handlers recebem um `CommandContext` com serviços (documents, views, workspace, window, LSP) em vez de `&mut App`. Isso melhora a testabilidade e evita conflitos de borrow quando um comando precisa tocar em mais de uma parte do estado.

### Invariantes

1. Todo comando passa por `execute`; não existe um segundo caminho de despacho.
2. O enablement é checado centralmente.
3. Os argumentos são serializáveis (JSON).
4. Um erro de comando é exposto sem derrubar a aplicação.
5. Comandos que editam texto passam por uma transação de documento e participam do undo.
6. IDs de comando são estáveis e compatíveis com o VS Code quando existe um comando correspondente.

### Por que adiar isso dói

Cada nova funcionalidade vira mais um braço do `match`, e o acoplamento com `App` cresce. A paleta e a compatibilidade com extensões exigem exatamente o registro que não existe. O contrato que as extensões enxergam (`vscode.commands.registerCommand` / `executeCommand`) é essa camada; sem um registro, o bridge Node–Rust vira uma coleção de casos especiais. A lógica de `when` duplicada entre keybindings e enablement de comandos também começa a divergir.

### Primeiro passo de migração

Extrair os braços do `match` para handlers nomeados, adicionar `CommandRegistry` e `ContextKeyService`, migrar de 5 a 10 comandos e manter o `match` como fallback temporário. Depois, fazer a paleta de comandos listar o registro. Isso pode ser incremental.

## 3. Fronteira de plugins: contrato, processo e concorrência

### A distinção

A fronteira entre o núcleo Rust e código de terceiros — extension host Node, LSP/DAP e webviews — com contratos explícitos de mensagens, identidade de documentos, versões, edições, eventos, threading, cancelamento e ciclo de vida.

### Como está hoje

`docs/en/architecture/extension-compat.md` descreve uma direção sólida: o extension host do Code OSS em um processo Node separado, conectado por RPC, distribuído pelo OpenVSX e entregue em camadas de compatibilidade. O cliente LSP já é um ensaio disso: `LspUiEvent`, `LspCommand`, `WorkerMessage`, `mpsc` e `UserEvent::LspWake`. Porém, cada evento é um enum interno específico; ainda não há protocolo versionado nem um dispatcher central reutilizável.

### Contrato alvo

- **Crates separados:** `graphite-protocol` (tipos de mensagem, sem lógica), `graphite-plugin-host` (supervisor do processo Node, transporte, ciclo de vida) e as implementações dos `MainThread*` na aplicação.
- **Fixar a versão do Code OSS.** O protocolo (`extHost.protocol.ts`) evolui; a compatibilidade tem como alvo um commit ou release específico, não "VS Code" em geral.
- **Serviços do main thread**, mesmo que mínimos no começo: documents, commands, workspace/fs/config, window, languages, decorations, terminal e webview.
- **Contrato de documento e edição:** URI, versão, `contentChanges`, aplicação atômica de `WorkspaceEdit` com checagem de versão e mapeamento para o undo do documento. Sem isso, edições de extensão e digitação do usuário competem sob semântica indefinida.
- **Save participants:** `willSave`, `willSaveWaitUntil` e `didSave`. O format-on-save de extensões depende disso.
- **Abstração de FileSystem/URI**, mesmo que apenas local no começo. Extensões e LSP assumem URIs; hardcodar `std::fs` em todo lugar torna remote e webviews inviáveis depois.
- **Threading:** extensões nunca tocam o estado da UI diretamente. Toda mensagem entra em uma fila drenada pelo event loop, e a UI nunca bloqueia esperando RPC. O mecanismo de eventos do LSP é o protótipo; generalize-o para um `MainThreadDispatcher`.
- **Ciclo de vida:** request IDs, cancelamento, timeouts, backpressure, restart do host, isolamento de crash e nenhum estado vazando se o Node terminar.
- **Camadas de compatibilidade:** recursos declarativos, comandos simples, UI pesada e API profunda, como já documentado. Isso permite entregar valor antes de o host completo existir.

### Por que adiar isso dói

Se o núcleo assume posse absoluta do estado e o muta diretamente, o bridge precisa simular a semântica do VS Code (versões, eventos, edições concorrentes, saves, escopos de configuração) em cima de uma API que não foi projetada para isso. A parte mais difícil não é o RPC; é a **concorrência** entre extensões, o usuário, formatadores e LSP. Refatorar isso depois significa mexer em documentos, comandos, workspace, configuração e UI simultaneamente.

### Primeiro passo

Escrever o contrato dos quatro ou cinco serviços exigidos pelas camadas de compatibilidade 1–2: documents, commands, workspace/config e window. Criar o crate de protocolo e o dispatcher central. Usar o LSP como teste de aceitação: se `didOpen`/`didChange` versionado, `applyEdit`, `executeCommand`, diagnostics e configuração passarem por esses contratos, a fronteira está validada antes de escrever qualquer código do host Node.

## Como os três contratos interagem

Considere um usuário digitando, salvando e uma extensão formatando ao salvar:

1. **Digitação:** a view envia uma transação para o documento, `version` incrementa e o LSP recebe `didChange` com essa versão. (Model/View + fronteira)
2. **Ctrl+S:** o keybinding resolve para `workbench.action.files.save` no **registro**. O handler executa os save participants, a extensão devolve um `WorkspaceEdit`, o documento o aplica atomicamente após checar a versão, o undo registra a edição e cada view remapeia suas seleções. (Comandos + Model/View + fronteira)
3. **Uma extensão registra `myExt.fix`:** o registro aceita; paleta, menus e keybindings usam o mesmo `when`/chaves de contexto; um `executeCommand` vindo do LSP resolve para o mesmo registro. (Comandos + fronteira)

Se qualquer um dos três contratos for informal, esse fluxo quebra em algum ponto, e a falha normalmente aparece tarde.

## Ordem recomendada

1. **Model/View primeiro**, porque comandos e plugins dependem de identidade, versões e ciclo de vida de documento.
2. **Registro de comandos e chaves de contexto depois**, porque essa é a API interna consumida pela paleta, menus, LSP e extensões.
3. **Protocolo e fronteira de plugins por último**, desenhados sobre os dois anteriores e validados com o LSP como ensaio.

Formalizar significa escrever esses tipos, invariantes e testes — não implementar a arquitetura inteira do VS Code. As implementações podem permanecer simples e migrar por baixo dos contratos, com o build sempre verde.
