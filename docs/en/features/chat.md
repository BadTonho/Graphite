# Chat, Inline Chat, and Agent Sessions

Graphite includes an integrated conversational assistant and workspace agent built around the `graphite-chat` crate and embedded UI surfaces inside `graphite-app`.

## Providers and Configuration

- Supported transports: Anthropic Messages API (`anthropic`) and OpenAI-compatible Chat Completions endpoints (`openai-compatible`).
- Streaming transport: Server-Sent Events (SSE) parsed in a background worker thread so the editor stays responsive.
- Storage: Provider configuration and API keys are stored locally in `chat-provider.json` under the active user profile folder (`workbench.action.openLanguageModelsJson`).
- Setup modal: Triggered automatically when submitting a prompt without valid credentials, or by pressing `F4` inside the Chat input box.

## Surfaces and Navigation

- **Secondary Side Bar** (`Ctrl+Alt+I`, `View -> Chat`, or `chat.action.focus`): Houses the conversation transcript, tool approval bar, pending edit summary, collapsible Agent Sessions list, and composer input.
- **Editor Surface** (`workbench.action.chat.openInEditor`): Expands the active chat session into the main editor area.
- **Quick Chat** (`Ctrl+Shift+Alt+C`, `graphite.chat.openQuickChat`): Opens a floating overlay that can be promoted into the side bar (`workbench.action.quickchat.openInChatView`) or dismissed (`Esc`).
- **Inline Chat** (`Ctrl+I`, `inlineChat.focus`, or `inlineChat.fixDiagnostics`): Anchors directly to the active document selection or diagnostics so edits can be reviewed and applied (`inlineChat2.keep`) or moved to the side bar (`inlineChat2.continueInChat`).

## Agent Modes, Tools, and Approvals

- Modes (`Ctrl+M` inside Chat to cycle):
  - **Ask**: Read-only Q&A grounded in open editor context and read/search workspace tools.
  - **Edit**: Proposes file edits tracked with before/after snapshots for review.
  - **Agent**: Executes workspace tools (reading, searching, editing, and terminal commands inside the workspace root) across multi-step turns.
- **Path confinement & Approvals**: All file tool paths are resolved inside the workspace root (`resolve_workspace_path`). Mutable operations and terminal commands respect `ToolApprovalPolicy` (`AlwaysAsk`, `AutoApproveReads`, `AutoApproveAll`), prompting interactively with Accept Once / Deny decisions when confirmation is required.
- **Checkpoints & Edits**: Pending file edits can be accepted, discarded, inspected in diff view (`chatEditing.openFileInDiff`), or rolled back via session checkpoints (`workbench.action.chat.undoEdit` / `redoEdit` / `restoreLastCheckpoint`).

## Sessions, Clipboard, and Export

- Sessions persist per profile with pinning, archiving, unread markers, orientation toggles (`side-by-side` vs `stacked`), and view filters (`Archived`, `AutomationRuns`, `Scheduled`).
- Code blocks extracted from responses can be navigated (`nextCodeBlock` / `previousCodeBlock`), copied to the clipboard (`copyCodeBlock`), inserted at the cursor or into a new file/notebook cell, or sent directly to the integrated terminal (`runInTerminal`).
- Conversations and agent debug logs export to and import from JSON under `<profile>/chat-exports/`.

> **Note:** Interactive GPU/window rendering of the Chat overlay, Inline Chat box, and Provider Setup modal has not been visually inspected in a running desktop window; behavior is verified through unit and command integration tests.
