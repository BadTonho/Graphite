//! Built-in chat, inline chat, and agent session integration.

use super::*;

pub(crate) mod context;
pub(crate) mod drawing;
pub(crate) mod state;
pub(crate) mod ui;
pub(crate) mod worker;

pub(crate) use state::{ChatState, ChatSurface, InlineChatState, SessionsFilter};
pub(crate) use worker::ChatUiEvent;

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_chat::{
        AgentTurnEvent, ApprovalDecision, EditReviewStatus, PendingToolApproval, ProposedEdit,
        TurnStatus, Usage,
    };
    use serde_json::json;
    use std::sync::mpsc;

    #[test]
    fn chat_turn_events_stage_accept_discard_and_undo_edits_in_open_buffers() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("demo.rs");
        std::fs::write(&file, "fn main() { 1 }\n").unwrap();

        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        let turn_id = app.chat.active_session_mut().start_turn("Change 1 to 2");
        let session_id = app.chat.active_session_id.clone();

        let (tx, rx) = mpsc::channel();
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            session_id: session_id.clone(),
            turn_id,
            event: AgentTurnEvent::ApprovalRequested(PendingToolApproval {
                id: "call_1".into(),
                name: "propose_edit".into(),
                summary: "Edit demo.rs".into(),
                input: json!({}),
            }),
            approval_reply: Some(tx),
        });
        assert!(app.chat.pending_approval.is_some());
        assert_eq!(
            app.chat.active_session().turns[0].status,
            TurnStatus::WaitingApproval
        );

        app.respond_chat_approval(ApprovalDecision::AllowAlways);
        assert_eq!(rx.recv().unwrap(), ApprovalDecision::AllowAlways);
        assert!(app.chat.policy.trusted_tools.contains("propose_edit"));

        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            session_id: session_id.clone(),
            turn_id,
            event: AgentTurnEvent::ToolResult {
                id: "call_1".into(),
                name: "propose_edit".into(),
                content: "Staged".into(),
                is_error: false,
                proposed_edit: Some(ProposedEdit {
                    path: file.clone(),
                    before: Some("fn main() { 1 }\n".into()),
                    after: "fn main() { 2 }\n".into(),
                }),
            },
            approval_reply: None,
        });
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            session_id,
            turn_id,
            event: AgentTurnEvent::Finished {
                usage: Usage {
                    input_tokens: 15,
                    output_tokens: 5,
                },
            },
            approval_reply: None,
        });

        assert_eq!(app.editor().text(), "fn main() { 2 }\n");
        assert_eq!(
            app.chat.active_session().pending_edits[0].status,
            EditReviewStatus::Pending
        );

        assert!(app.undo_chat_edit());
        assert_eq!(app.editor().text(), "fn main() { 1 }\n");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn main() { 1 }\n");

        assert!(app.redo_chat_edit());
        assert_eq!(app.editor().text(), "fn main() { 2 }\n");

        assert!(app.accept_chat_edit(&file));
        assert_eq!(
            app.chat.active_session().pending_edits[0].status,
            EditReviewStatus::Accepted
        );
    }

    #[test]
    fn discard_chat_edit_reverts_file_and_open_document() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "pub fn a() {}\n").unwrap();

        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        let turn_id = app.chat.active_session_mut().start_turn("Rewrite");
        let session_id = app.chat.active_session_id.clone();

        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            session_id,
            turn_id,
            event: AgentTurnEvent::ToolResult {
                id: "t1".into(),
                name: "propose_edit".into(),
                content: "ok".into(),
                is_error: false,
                proposed_edit: Some(ProposedEdit {
                    path: file.clone(),
                    before: Some("pub fn a() {}\n".into()),
                    after: "pub fn b() {}\n".into(),
                }),
            },
            approval_reply: None,
        });
        assert_eq!(app.editor().text(), "pub fn b() {}\n");

        assert!(app.discard_chat_edit(&file));
        assert_eq!(app.editor().text(), "pub fn a() {}\n");
        assert_eq!(
            app.chat.active_session().pending_edits[0].status,
            EditReviewStatus::Discarded
        );
    }

    #[test]
    fn inline_chat_keep_and_continue_in_chat_work() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.editor_mut().insert("let x = 1;");
        app.editor_mut().set_selection(0, 10);
        let doc = app.active_document();

        app.chat.inline_chat = Some(InlineChatState {
            document: doc,
            anchor_line: 0,
            original_selection: Some((0, 10)),
            prompt: "Make constant".into(),
            response: "```rust\nconst X: i32 = 1;\n```".into(),
            proposed_replacement: Some("const X: i32 = 1;".into()),
            running: false,
            error: None,
        });

        assert!(app.keep_inline_chat());
        assert_eq!(app.editor().text(), "const X: i32 = 1;");
        assert!(app.chat.inline_chat.is_none());

        app.chat.inline_chat = Some(InlineChatState {
            document: doc,
            anchor_line: 0,
            original_selection: None,
            prompt: "Explain this".into(),
            response: "It declares a constant.".into(),
            proposed_replacement: None,
            running: false,
            error: None,
        });
        app.continue_inline_in_chat();
        assert!(app.chat.visible);
        assert_eq!(app.chat.active_session().turns.len(), 1);
        assert_eq!(
            app.chat.active_session().turns[0].assistant_text(),
            "It declares a constant."
        );
    }

    #[test]
    fn secondary_sidebar_keyboard_provider_dialog_and_code_block_navigation_work() {
        use winit::keyboard::{Key, NamedKey};

        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert_eq!(app.secondary_sidebar_width(), 0.0);

        app.toggle_outline();
        assert_eq!(app.secondary_sidebar_width(), 240.0);

        app.open_chat_sidebar();
        assert_eq!(app.secondary_sidebar_width(), 320.0);
        assert!(app.chat.visible && app.chat.focused);

        // Input typing & history navigation
        assert!(app.handle_chat_key(&Key::Character("h".into()), Some("h"), false, false));
        assert!(app.handle_chat_key(&Key::Character("i".into()), Some("i"), false, false));
        assert_eq!(app.chat.input, "hi");
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Backspace), None, false, false));
        assert_eq!(app.chat.input, "h");

        // Mode cycling via Tab
        let initial_mode = app.chat.active_session().mode;
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Tab), None, false, false));
        assert_ne!(app.chat.active_session().mode, initial_mode);

        // Provider setup dialog editing & saving
        app.open_chat_provider_dialog();
        assert!(app.chat.provider_dialog.is_some());
        assert!(app.handle_chat_key(&Key::Character("s".into()), Some("s"), false, false));
        assert!(app.handle_chat_key(&Key::Character("k".into()), Some("k"), false, false));
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Enter), None, false, false));
        assert!(app.chat.provider_dialog.is_none());
        assert_eq!(
            app.chat
                .provider
                .as_ref()
                .and_then(|p| p.api_key.as_deref()),
            Some("sk")
        );

        // Code block navigation and insertion
        let tid = app.chat.active_session_mut().start_turn("Give code");
        app.chat.active_session_mut().append_text(
            tid,
            "```rust\nlet a = 1;\n```\n\n```rust\nlet b = 2;\n```\n",
        );
        assert!(app.step_chat_code_block(true));
        assert_eq!(app.active_chat_code_block().unwrap().code, "let b = 2;");
        assert!(app.insert_active_code_block(false));
        assert_eq!(app.editor().text(), "let b = 2;");
    }
}
