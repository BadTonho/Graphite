# Chat, Inline Chat, and Agent Sessions

Graphite includes an integrated conversational assistant and workspace agent built around the `graphite-chat` crate and embedded UI surfaces inside `graphite-app`.

## Providers and Configuration

- Select **Set up a connection** on first use. To manage connections later, open the connection selector and choose **Configure connections**, press `F4` in Chat, or use **Configure Chat Connections** in the command palette. Choose a provider and enter a name, model ID, and any required API key. Graphite fills in the official endpoint for supported providers. For **OpenAI compatible**, enter the API base URL, for example `http://localhost:11434/v1` for Ollama. HTTPS is required for remote endpoints.
- Connection settings are local to the active profile. API keys are stored in Windows Credential Manager or Linux Secret Service. If the system store cannot save a key, **Use for this session** keeps it in memory until the app closes. Graphite removes the previous `chat-provider.json` after successfully migrating its configuration and any saved key.
- Select a saved connection in the panel or choose **Use connection** from its settings. Confirm **New conversation** to switch from a conversation with messages. Open **History** to resume a conversation with its original connection. If that connection is unavailable, the conversation remains readable.
- Write a message and select **Send** or press `Enter`. Choose **Ask**, **Edit**, or **Agent** next to the message field. Use **Attach context** or `F6` to add an editor file, selection, or diagnostics, and select an attachment to remove it before sending.
- The response streams in the background. Use **Stop** or `Esc` to cancel an active reply before switching.

| Provider | What to enter |
| --- | --- |
| OpenAI or Claude | The provider's API key and model ID. |
| OpenCode Console | A Console API key for a workspace with credits and the model ID from the API catalog. Usage is billed as you go. [OpenCode Console](https://opencode.ai/docs/console/api/inference) |
| OpenCode Go | An API key covered by a Go subscription and the model ID from the Go API catalog. [OpenCode Go](https://opencode.ai/docs/console/go/) |
| Gemini | A Google AI Studio API key and a Gemini model ID supported by Google's Chat Completions endpoint. [Gemini API](https://ai.google.dev/gemini-api/docs/openai) |
| OpenRouter | An OpenRouter API key and a model ID in `provider/model` format. [OpenRouter API](https://openrouter.ai/docs/api/reference/overview) |

OpenCode free-tier chat models that are restricted to the OpenCode client are unavailable in Graphite. Graphite uses a separate OpenCode session identifier for each conversation. Tool availability depends on the selected model. [OpenCode harnesses](https://opencode.ai/docs/console/harnesses)

## Editor and AI workbench modes

Graphite starts in **Editor** mode. Click the mode button in the title bar. It shows **AI** in Editor mode and **Editor** in AI mode. You can also run **Toggle Editor / AI Mode** from the Command Palette. AI mode puts Chat in the main area, with the same conversation, attachments, draft and reply in progress. The Explorer and editor tabs are available in Editor mode.

Use **Open File** or Quick Open in AI mode to show an editable file beside Chat. Narrow windows stack the two panes. The file uses the same document and undo history as Editor mode. **Close panel** returns the space to Chat and keeps the file and any unsaved changes available in Editor mode. Switching back restores the active editor tab and its view.

The integrated terminal starts collapsed the first time you enter AI mode. Use **Toggle Terminal** to open it. Terminal sessions continue when switching modes.

## Surfaces and Navigation

- **Secondary Side Bar** (`Ctrl+Alt+I`, `View -> Chat`, or `chat.action.focus`): Shows Chat alongside extension views. Open **Outline** from the command palette.
- **Editor Surface** (`workbench.action.chat.openInEditor`): Expands the active chat session into the main editor area.
- **Quick Chat** (`Ctrl+Shift+Alt+C`, `graphite.chat.openQuickChat`): Opens a floating overlay that can be promoted into the side bar (`workbench.action.quickchat.openInChatView`) or dismissed (`Esc`).
- **Inline Chat** (`Ctrl+I`, `inlineChat.focus`, or `inlineChat.fixDiagnostics`): Anchors directly to the active document selection or diagnostics so edits can be reviewed and applied (`inlineChat2.keep`) or moved to the side bar (`inlineChat2.continueInChat`).

## Agent Modes, Tools, and Approvals

- Modes (`Tab` inside Chat to cycle):
  - **Ask**: The default for new chats. Reading or searching workspace files requires approval for each tool call.
  - **Edit**: Proposes file edits tracked with before/after snapshots for review.
  - **Agent**: Executes workspace tools (reading, searching, editing, and terminal commands inside the workspace root) across multi-step turns.
- **Terminal commands**: `run_command` runs in the workspace root through `cmd /C` on Windows or `sh -c` on Linux, with no input. A command is stopped, together with the processes it started, when it runs longer than 2 minutes or when you select **Stop**. At most 24 KB of output is returned to the model.
- **Path confinement**: All file tool paths are resolved inside the workspace root (`resolve_workspace_path`). Links are resolved too, so a link inside the workspace that points elsewhere is refused, including for files that do not exist yet. Workspace search skips links.
- **Approvals**: Edits, terminal commands, and plugin tools ask for approval with **Allow once**, **Always allow here**, or **Deny**. Outside Ask mode, reading and searching run without asking unless **Chat: Cycle Tool Approval Policy** turns that off.
  - **Always allow here** applies only to the current workspace. For a terminal command the button reads **Always allow this command**, and only that exact command runs without asking; any other command still asks.
  - In a restricted (untrusted) workspace, nothing is remembered and approvals given in other workspaces are not used.
  - **Chat: Reset Trusted Tools** forgets every remembered approval. Approvals saved by earlier versions applied to every workspace and are no longer used.

In the composer, choose **File**, **Selection**, or **Diagnostics** to attach the active editor context to the next message. Each selected attachment is marked in the composer. Inline Chat uses the text selection or the line at the cursor.

## Review proposed file changes

Use **Review files** in the Chat panel to open the diff for a proposal. **Apply** writes the displayed version to disk and refreshes the editor. **Discard** removes the proposal from the queue. Use **Apply all** to apply the listed proposals individually. The results show which files were applied and which still need attention.

In AI mode, select a diff line and choose **Open Hunk in Editor**. If you have not selected a line, the action opens the current change. New files can be opened after applying their proposals. Choose **Return to Review** in the Editor title bar or Command Palette to return to your conversation and diff position. Unsaved edits stay in the shared buffer and can cause a proposal conflict.

Graphite checks the original workspace, disk contents and editor before writing. A file that changed, is read-only, is being saved or has unsaved edits stays pending. Use **Stop** to cancel a turn while retaining proposals already received. Reopen the conversation in the original workspace to continue the review.

Applied file changes create checkpoints for **Undo Edit** and **Redo Edit**. These actions check the file again before writing. The tool approval for proposing an edit is separate from the decision to apply it. Terminal and plugin tools retain their own approval rules.

## Recover and continue a task

Each Chat turn shows the files the assistant proposed or the user applied, discarded or left in conflict. It also records the status of terminal commands and displays the exit code and available output, including results from test commands. The file summary covers changes tracked through Chat review.

If Graphite closes during an active turn, reopening the conversation marks that turn as **Interrupted**. Proposals already received stay available for review. A terminal command whose result could not be saved displays **No result recorded**.

Choose **Continue task** in the original workspace and with the conversation's connection available. This starts a new turn in the same conversation using its saved history. New tool calls follow the workspace approval rules. Applying a proposal leaves test execution to the agent or user.

## Sessions, Clipboard, and Export

- Sessions persist per profile with pinning, archiving, unread markers, orientation toggles (`side-by-side` vs `stacked`), and view filters (`Archived`, `AutomationRuns`, `Scheduled`).
- Code blocks extracted from responses can be navigated (`nextCodeBlock` / `previousCodeBlock`), copied to the clipboard (`copyCodeBlock`), inserted at the cursor or into a new file/notebook cell, or sent directly to the integrated terminal (`runInTerminal`).
- Conversations and agent debug logs export to and import from JSON under `<profile>/chat-exports/`.

> **Note:** Interactive GPU/window rendering of the Chat overlay, Inline Chat box, and Provider Setup modal has not been visually inspected in a running desktop window; behavior is verified through unit and command integration tests.
