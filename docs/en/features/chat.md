# Graphite Chat, Inline Chat, and Agent Sessions

Graphite includes an integrated conversational assistant and workspace agent built around the `graphite-chat` crate and embedded UI surfaces inside `graphite-app`.

## Providers and Configuration

- Open **Configure connections** in Graphite Chat, press `F4` in the chat, or use **Configure Chat Connections** in the command palette. Give the connection a name, select a provider, and enter the model ID and any required API key. Graphite fills in the official endpoint for supported providers. For **OpenAI compatible**, enter the API base URL, for example `http://localhost:11434/v1` for Ollama. HTTPS is required for remote endpoints.
- Connection settings are local to the active profile. API keys are stored in Windows Credential Manager or Linux Secret Service. If the system store cannot save a key, **Use for this session** keeps it in memory until the app closes. Graphite removes the previous `chat-provider.json` after successfully migrating its configuration and any saved key.
- Choose **Use** to switch connections. Selecting a different connection or model starts a fresh conversation. Existing conversations with a valid original connection can be resumed from history. Legacy and imported conversations, and those whose original destination has changed, remain readable.
- The response streams in the background. Use **Stop** or `Esc` to cancel an active reply before switching.

| Provider | What to enter |
| --- | --- |
| OpenAI or Claude | The provider's API key and model ID. |
| OpenCode Console | A Console API key for a workspace with credits and the model ID from the API catalog. Usage is billed as you go. [OpenCode Console](https://opencode.ai/docs/console/api/inference) |
| OpenCode Go | An API key covered by a Go subscription and the model ID from the Go API catalog. [OpenCode Go](https://opencode.ai/docs/console/go/) |
| Gemini | A Google AI Studio API key and a Gemini model ID supported by Google's Chat Completions endpoint. [Gemini API](https://ai.google.dev/gemini-api/docs/openai) |
| OpenRouter | An OpenRouter API key and a model ID in `provider/model` format. [OpenRouter API](https://openrouter.ai/docs/api/reference/overview) |

OpenCode free-tier chat models that are restricted to the OpenCode client are unavailable in Graphite. Graphite uses a separate OpenCode session identifier for each conversation. Tool availability depends on the selected model. [OpenCode harnesses](https://opencode.ai/docs/console/harnesses)

## Surfaces and Navigation

- **Secondary Side Bar** (`Ctrl+Alt+I`, `View -> Chat`, or `chat.action.focus`): Shows Graphite Chat alongside extension views. Open **Outline** from the command palette.
- **Editor Surface** (`workbench.action.chat.openInEditor`): Expands the active chat session into the main editor area.
- **Quick Chat** (`Ctrl+Shift+Alt+C`, `graphite.chat.openQuickChat`): Opens a floating overlay that can be promoted into the side bar (`workbench.action.quickchat.openInChatView`) or dismissed (`Esc`).
- **Inline Chat** (`Ctrl+I`, `inlineChat.focus`, or `inlineChat.fixDiagnostics`): Anchors directly to the active document selection or diagnostics so edits can be reviewed and applied (`inlineChat2.keep`) or moved to the side bar (`inlineChat2.continueInChat`).

## Agent Modes, Tools, and Approvals

- Modes (`Tab` inside Chat to cycle):
  - **Ask**: The default for new chats. Reading or searching workspace files requires approval for each tool call.
  - **Edit**: Proposes file edits tracked with before/after snapshots for review.
  - **Agent**: Executes workspace tools (reading, searching, editing, and terminal commands inside the workspace root) across multi-step turns.
- **Path confinement & Approvals**: All file tool paths are resolved inside the workspace root (`resolve_workspace_path`). Mutable operations and terminal commands respect `ToolApprovalPolicy` (`AlwaysAsk`, `AutoApproveReads`, `AutoApproveAll`), prompting interactively with Accept Once / Deny decisions when confirmation is required.
- **Checkpoints & Edits**: Pending file edits can be accepted, discarded, inspected in diff view (`chatEditing.openFileInDiff`), or rolled back via session checkpoints (`workbench.action.chat.undoEdit` / `redoEdit` / `restoreLastCheckpoint`).

In the composer, choose **File**, **Selection**, or **Diagnostics** to attach the active editor context to the next message. Each selected attachment is marked in the composer. Inline Chat uses the text selection or the line at the cursor.

## Sessions, Clipboard, and Export

- Sessions persist per profile with pinning, archiving, unread markers, orientation toggles (`side-by-side` vs `stacked`), and view filters (`Archived`, `AutomationRuns`, `Scheduled`).
- Code blocks extracted from responses can be navigated (`nextCodeBlock` / `previousCodeBlock`), copied to the clipboard (`copyCodeBlock`), inserted at the cursor or into a new file/notebook cell, or sent directly to the integrated terminal (`runInTerminal`).
- Conversations and agent debug logs export to and import from JSON under `<profile>/chat-exports/`.

> **Note:** Interactive GPU/window rendering of the Chat overlay, Inline Chat box, and Provider Setup modal has not been visually inspected in a running desktop window; behavior is verified through unit and command integration tests.
