//! Built-in chat, inline chat, and agent session integration.

use super::*;

mod connection_ui;
mod connection_view;
mod connections;
pub(crate) mod context;
pub(crate) mod drawing;
mod edits;
mod layout;
mod review;
mod review_view;
pub(crate) mod state;
mod task;
pub(crate) mod ui;
pub(crate) mod worker;

pub(crate) use state::{ChatState, ChatSurface, InlineChatState, SessionsFilter};
pub(crate) use worker::ChatUiEvent;

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_chat::{
        AgentTurnEvent, ApprovalDecision, PendingToolApproval, ProposedEdit, TurnStatus, Usage,
    };
    use serde_json::json;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};

    #[test]
    fn chat_turn_events_stage_accept_discard_and_undo_edits_in_open_buffers() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("demo.rs");
        std::fs::write(&file, "fn main() { 1 }\n").unwrap();

        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        let turn_id = app.chat.active_session_mut().start_turn("Change 1 to 2");
        let session_id = app.chat.active_session_id.clone();
        let run = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session_id.clone(), turn_id, run.clone()));

        let (tx, rx) = mpsc::channel();
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: run.clone(),
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
        let root = app.tree.root().to_path_buf();
        assert!(app
            .chat
            .policy
            .is_trusted(&root, "propose_edit", &json!({})));

        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: run.clone(),
            session_id: session_id.clone(),
            turn_id,
            event: AgentTurnEvent::ToolResult {
                id: "call_1".into(),
                name: "propose_edit".into(),
                content: "Staged".into(),
                is_error: false,
                proposed_edit: Some(ProposedEdit {
                    path: file.clone(),
                    before: Some(b"fn main() { 1 }\n".to_vec()),
                    after: "fn main() { 2 }\n".into(),
                }),
            },
            approval_reply: None,
        });
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: run.clone(),
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

        assert_eq!(app.editor().text(), "fn main() { 1 }\n");
        assert_eq!(app.chat.active_session().proposals.len(), 1);
        assert!(app.accept_chat_edit(&file));
        assert_eq!(app.editor().text(), "fn main() { 2 }\n");
        assert!(app.undo_chat_edit());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn main() { 1 }\n");
        assert!(app.redo_chat_edit());
        assert_eq!(app.editor().text(), "fn main() { 2 }\n");
    }

    fn pending_command(app: &mut App, command: &str) -> mpsc::Receiver<ApprovalDecision> {
        let (tx, rx) = mpsc::channel();
        app.chat.pending_approval = Some((
            PendingToolApproval {
                id: "call".into(),
                name: "run_command".into(),
                summary: format!("Run `{command}`"),
                input: json!({ "command": command }),
            },
            tx,
        ));
        rx
    }

    #[test]
    fn always_allowing_a_command_remembers_only_that_command_in_this_workspace() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.chat.active_session_mut().mode = graphite_chat::AgentMode::Agent;
        let rx = pending_command(&mut app, "cargo test");

        app.respond_chat_approval(ApprovalDecision::AllowAlways);

        assert_eq!(rx.recv().unwrap(), ApprovalDecision::AllowAlways);
        let root = app.tree.root().to_path_buf();
        let policy = &app.chat.policy;
        assert!(!policy.requires_approval(&root, "run_command", &json!({"command": "cargo test"})));
        assert!(policy.requires_approval(&root, "run_command", &json!({"command": "cargo run"})));
        let other = crate::test_support::TempDir::new();
        assert!(policy.requires_approval(
            other.path(),
            "run_command",
            &json!({"command": "cargo test"})
        ));
    }

    #[test]
    fn restricted_workspace_never_remembers_an_approval() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.chat.active_session_mut().mode = graphite_chat::AgentMode::Agent;
        app.workspace_trust = Some(crate::app::phase5::WorkspaceTrust::restricted(
            dir.path().join("trust.json"),
        ));
        let rx = pending_command(&mut app, "cargo test");
        assert!(!app.chat_allows_always());

        app.respond_chat_approval(ApprovalDecision::AllowAlways);

        assert_eq!(rx.recv().unwrap(), ApprovalDecision::AllowOnce);
        assert!(!app.chat.policy.has_trusted());
    }

    #[test]
    fn chat_storage_failures_are_reported_instead_of_ignored() {
        let dir = crate::test_support::TempDir::new();
        // A file where the profile folder should be: nothing can be saved inside it.
        let profile = dir.path().join("profile");
        std::fs::write(&profile, "not a folder").unwrap();
        let state = ChatState::load_from_profile(&profile);

        assert!(!state.persist_active_session(&profile, false));
        assert!(!state.persist_all_sessions(&profile, false));
        assert!(!state.persist_policy(&profile, false));
        // An isolated profile is never written, so it cannot fail.
        assert!(state.persist_active_session(&profile, true));
        assert!(state.persist_policy(&profile, true));
    }

    #[test]
    fn chat_commands_do_not_write_an_isolated_profile() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let profile = app.profile_root.clone();
        let sessions = profile.join(graphite_chat::SESSIONS_DIR);
        let policy = profile.join(graphite_chat::APPROVAL_POLICY_FILE);
        assert!(!sessions.exists() && !policy.exists());

        for command in [
            "workbench.action.chat.clearHistory",
            "workbench.action.chat.resetTrustedTools",
            "agentSessionSection.archive",
        ] {
            app.run_ui_command(command);
        }

        assert!(!sessions.exists(), "sessions were written");
        assert!(!policy.exists(), "tool approvals were written");
    }

    #[test]
    fn discard_chat_edit_preserves_file_and_open_document() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "pub fn a() {}\n").unwrap();

        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        let turn_id = app.chat.active_session_mut().start_turn("Rewrite");
        let session_id = app.chat.active_session_id.clone();
        let run = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session_id.clone(), turn_id, run.clone()));

        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: run.clone(),
            session_id,
            turn_id,
            event: AgentTurnEvent::ToolResult {
                id: "t1".into(),
                name: "propose_edit".into(),
                content: "ok".into(),
                is_error: false,
                proposed_edit: Some(ProposedEdit {
                    path: file.clone(),
                    before: Some(b"pub fn a() {}\n".to_vec()),
                    after: "pub fn b() {}\n".into(),
                }),
            },
            approval_reply: None,
        });
        assert_eq!(app.editor().text(), "pub fn a() {}\n");
        assert!(app.discard_chat_edit(&file));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "pub fn a() {}\n");
        assert!(app.chat.active_session().proposals.is_empty());
        assert!(app.chat.active_session().checkpoints.is_empty());
    }

    #[test]
    fn inline_chat_keep_and_continue_in_chat_work() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.editor_mut().insert("let x = 1;");
        app.editor_mut().set_selection(0, 10);
        let doc = app.active_document();

        app.chat.inline_chat = Some(InlineChatState {
            connection: None,
            opencode_session: None,
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
            connection: None,
            opencode_session: None,
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
        assert!(app.handle_chat_key(&Key::Named(NamedKey::F4), None, false, false));
        assert!(app.chat.provider_dialog.is_some());
        assert!(app.handle_chat_key(&Key::Named(NamedKey::F2), None, false, false));
        assert!(app.handle_chat_key(&Key::Character("test".into()), Some("test"), false, false));
        let dialog = app.chat.provider_dialog.as_mut().unwrap();
        dialog.draft.model = "demo-model".into();
        dialog.focus_field = 5;
        assert!(app.handle_chat_key(
            &Key::Character("fixture".into()),
            Some("fixture"),
            false,
            false
        ));
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Enter), None, false, false));
        assert!(app.chat.provider_dialog.is_none());
        let connection = app.chat.active_connection().unwrap();
        assert_eq!(connection.name, "test");
        assert_eq!(
            app.chat
                .session_keys
                .get(&connection.id)
                .map(String::as_str),
            Some("fixture")
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

    #[test]
    fn changing_connections_starts_a_clean_conversation_and_preserves_the_previous_one() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        for name in ["local A", "local B"] {
            app.add_chat_connection();
            let draft = &mut app.chat.provider_dialog.as_mut().unwrap().draft;
            draft.name = name.into();
            draft.provider = connections::Provider::Compatible;
            draft.model = "fixture-model".into();
            draft.base_url = "http://127.0.0.1:4321/v1".into();
            app.save_chat_provider_from_dialog(false);
            if name == "local A" {
                app.chat
                    .active_session_mut()
                    .start_turn("Only A receives this");
            } else {
                assert_eq!(app.chat.active_connection().unwrap().name, "local A");
                assert_eq!(app.chat.connections.items.len(), 1);
                assert!(app
                    .chat
                    .provider_dialog
                    .as_ref()
                    .unwrap()
                    .confirmation
                    .is_some());
                app.confirm_chat_connection_change();
            }
        }
        assert_eq!(app.chat.active_connection().unwrap().name, "local B");
        assert!(app.chat.active_session().turns.is_empty());
        assert_eq!(
            app.chat.active_session().mode,
            graphite_chat::AgentMode::Ask
        );
        let previous = &app.chat.sessions[1];
        assert_eq!(previous.turns[0].prompt, "Only A receives this");
        assert_eq!(
            app.chat
                .connections
                .get(previous.connection_id.as_deref().unwrap())
                .unwrap()
                .name,
            "local A"
        );

        app.open_chat_provider_dialog();
        app.remove_chat_connection();
        assert!(app.chat.active_connection().is_none());
        assert_eq!(app.chat.connections.items.len(), 1);
    }

    #[test]
    fn standalone_chat_context_includes_only_explicit_attachments() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("attachment_demo.txt");
        std::fs::write(&file, "editor-only fixture").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file);
        let prompt = |app: &App| {
            graphite_chat::build_system_prompt(
                graphite_chat::AgentMode::Ask,
                &app.chat_context_for_turn(),
            )
        };
        assert!(!prompt(&app).contains("editor-only fixture"));
        assert!(!prompt(&app).contains(&dir.path().display().to_string()));
        app.toggle_chat_attachment(0);
        assert!(prompt(&app).contains("editor-only fixture"));
        app.chat.clear_outgoing_draft();
        assert!(!prompt(&app).contains("editor-only fixture"));
    }

    #[test]
    fn stopping_a_chat_releases_connection_selection_and_ignores_late_events() {
        use std::sync::atomic::Ordering;

        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let session_id = app.chat.active_session_id.clone();
        let turn_id = app.chat.active_session_mut().start_turn("fixture prompt");
        let cancel = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session_id.clone(), turn_id, cancel.clone()));
        app.cancel_chat_turn();
        assert!(cancel.load(Ordering::Relaxed));
        assert!(app.chat.running_turn.is_none());
        app.open_chat_provider_dialog();
        assert!(app.chat.provider_dialog.is_some());
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: cancel,
            session_id,
            turn_id,
            event: AgentTurnEvent::DeltaText("late text".into()),
            approval_reply: None,
        });
        let turn = app.chat.active_session().turns.last().unwrap();
        assert_eq!(turn.status, TurnStatus::Cancelled);
        assert!(turn.assistant_text().is_empty());
    }

    #[test]
    fn switching_user_profiles_reloads_chat_and_rejects_the_previous_turn() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let profiles = crate::app::phase5::Profiles::open(&dir.path().join("config")).unwrap();
        profiles.create("Other").unwrap();
        app.profile_root = profiles.path("Default").unwrap();
        app.profiles = Some(profiles);
        app.add_chat_connection();
        let draft = &mut app.chat.provider_dialog.as_mut().unwrap().draft;
        draft.name = "Default connection".into();
        draft.provider = connections::Provider::Compatible;
        draft.model = "fixture-model".into();
        draft.base_url = "http://localhost:4321/v1".into();
        app.save_chat_provider_from_dialog(false);
        let session_id = app.chat.active_session_id.clone();
        let turn_id = app
            .chat
            .active_session_mut()
            .start_turn("Default profile only");
        let run = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session_id.clone(), turn_id, run.clone()));

        app.switch_user_profile("Other").unwrap();
        assert!(app.chat.connections.items.is_empty());
        assert!(app.chat.active_session().turns.is_empty());
        assert!(run.load(std::sync::atomic::Ordering::Relaxed));
        app.switch_user_profile("Default").unwrap();
        assert_eq!(
            app.chat.active_connection().unwrap().name,
            "Default connection"
        );
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run,
            session_id,
            turn_id,
            event: AgentTurnEvent::DeltaText("late reply".into()),
            approval_reply: None,
        });
        let turn = &app.chat.active_session().turns[0];
        assert_eq!(turn.status, TurnStatus::Cancelled);
        assert!(turn.assistant_text().is_empty());
    }

    #[test]
    fn opencode_conversation_identifiers_survive_reload_and_are_distinct_for_new_chats() {
        let profile = tempfile::tempdir().unwrap();
        let connection = connections::Connection {
            id: "test-connection".into(),
            name: "OpenCode Go".into(),
            model: "fixture-model".into(),
            provider: connections::Provider::OpenCodeGo,
            ..Default::default()
        };
        let mut chat = ChatState::default();
        chat.connections.items.push(connection.clone());
        chat.connections.preferred = Some(connection.id.clone());
        chat.connections.save(profile.path()).unwrap();
        let original = chat
            .opencode_session(&connection, profile.path(), false)
            .unwrap();
        assert!(original.starts_with("ses_"));
        assert_eq!(
            chat.opencode_session(&connection, profile.path(), false)
                .unwrap(),
            original
        );

        let mut reloaded = ChatState::load_from_profile(profile.path());
        assert_eq!(
            reloaded
                .opencode_session(&connection, profile.path(), false)
                .unwrap(),
            original
        );
        reloaded.new_session(graphite_chat::AgentMode::Ask);
        assert_ne!(
            reloaded
                .opencode_session(&connection, profile.path(), false)
                .unwrap(),
            original
        );
    }

    #[test]
    fn opencode_inline_reuses_its_identifier_only_for_the_same_connection() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_inline_chat();
        let inline = app.chat.inline_chat.as_mut().unwrap();
        let console = ("console".into(), "model-a".into());
        let go = ("go".into(), "model-a".into());
        let id = inline.opencode_session_for(&console).unwrap();
        inline.connection = Some(console.clone());
        assert_eq!(inline.opencode_session_for(&console).unwrap(), id);
        assert_ne!(inline.opencode_session_for(&go).unwrap(), id);
    }

    #[test]
    fn configuration_selector_reaches_the_new_providers_and_can_cycle_backwards() {
        use connections::Provider;
        use winit::keyboard::{Key, NamedKey};

        let dir = crate::test_support::TempDir::new();
        for (language, compatible) in [
            ("en", "OpenAI compatible"),
            ("pt-BR", "Compatível com OpenAI"),
        ] {
            let mut app = App::new_with_options(dir.path().into(), None, true);
            app.translator =
                graphite_i18n::Translator::new(graphite_i18n::Language::parse(language).unwrap());
            app.open_chat_sidebar();
            for key in [NamedKey::F4, NamedKey::ArrowDown] {
                assert!(app.handle_chat_key(&Key::Named(key), None, false, false));
            }
            for expected in [
                Provider::Claude,
                Provider::OpenCodeConsole,
                Provider::OpenCodeGo,
                Provider::Gemini,
                Provider::OpenRouter,
                Provider::Compatible,
                Provider::OpenAi,
            ] {
                assert!(app.handle_chat_key(&Key::Named(NamedKey::ArrowRight), None, false, false));
                assert_eq!(
                    app.chat.provider_dialog.as_ref().unwrap().draft.provider,
                    expected
                );
            }
            assert!(app.handle_chat_key(&Key::Named(NamedKey::ArrowLeft), None, false, false));
            assert_eq!(
                app.chat.provider_dialog.as_ref().unwrap().draft.provider,
                Provider::Compatible
            );
            assert_eq!(app.t("chat.compatible_provider"), compatible);
            for key in [NamedKey::Space, NamedKey::ArrowUp, NamedKey::Enter] {
                assert!(app.handle_chat_key(&Key::Named(key), None, false, false));
            }
            assert_eq!(
                app.chat.provider_dialog.as_ref().unwrap().draft.provider,
                Provider::OpenRouter
            );
            assert_eq!(app.t("secondary_sidebar.chat"), "Chat");
        }
    }

    #[test]
    fn narrow_chat_controls_open_setup_attach_context_and_stop_the_live_turn() {
        use winit::keyboard::{Key, NamedKey};

        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_chat_sidebar();
        app.secondary_sidebar_user_width = Some(200.0);
        let layout = |app: &App| app.chat_panel_layout(960.0, 760.0, 18.0);
        let click = |app: &mut App, rect: layout::Rect| {
            assert!(app.handle_chat_click(
                rect.x + rect.w / 2.0,
                rect.y + rect.h / 2.0,
                960.0,
                760.0,
                18.0
            ));
        };
        let initial = layout(&app);
        assert!(initial.toolbar[0].bottom() <= initial.toolbar[1].y);
        assert!(initial.setup.bottom() <= initial.body.bottom());
        click(&mut app, initial.setup);
        assert_eq!(app.chat.provider_dialog.as_ref().unwrap().focus_field, 1);

        assert!(app.handle_chat_key(&Key::Named(NamedKey::Enter), None, false, false));
        assert_eq!(
            app.chat.provider_dialog.as_ref().unwrap().field_error,
            Some((1, "chat.name_required"))
        );
        let dialog = app.chat.provider_dialog.as_mut().unwrap();
        dialog.draft.name = "Local fixture".into();
        dialog.draft.provider = connections::Provider::Compatible;
        dialog.draft.model = "fixture-model".into();
        dialog.draft.base_url = "http://example.invalid/v1".into();
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Enter), None, false, false));
        assert_eq!(
            app.chat.provider_dialog.as_ref().unwrap().field_error,
            Some((3, "chat.url_invalid"))
        );
        app.chat.provider_dialog.as_mut().unwrap().draft.base_url =
            "http://localhost:4321/v1".into();
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Enter), None, false, false));
        assert!(app.chat.provider_dialog.is_none());
        assert!(app.chat.focused);

        let context = layout(&app).context;
        click(&mut app, context);
        let first = layout(&app).menu.unwrap()[0];
        click(&mut app, first);
        assert!(app.chat.attachments.active_file.is_some());
        let chip = layout(&app).attachments[0].1;
        click(&mut app, chip);
        assert!(app.chat.attachments.active_file.is_none());

        assert!(app.handle_chat_key(&Key::Named(NamedKey::F6), None, false, false));
        assert!(app.handle_chat_key(&Key::Named(NamedKey::Enter), None, false, false));
        assert!(app.chat.attachments.active_file.is_some());

        let send = layout(&app).send;
        click(&mut app, send);
        assert!(app.chat.active_session().turns.is_empty());
        app.chat.input = "fixture prompt".into();
        click(&mut app, send);
        assert_eq!(app.status, "Event loop unavailable for background chat");
        assert_eq!(app.chat.input, "fixture prompt");

        let turn_id = app.chat.active_session_mut().start_turn("fixture prompt");
        let cancel = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((app.chat.active_session_id.clone(), turn_id, cancel.clone()));
        click(&mut app, send);
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        assert!(app.chat.running_turn.is_none());
    }

    #[test]
    fn editing_the_model_preserves_the_original_connection_for_history() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.add_chat_connection();
        let draft = &mut app.chat.provider_dialog.as_mut().unwrap().draft;
        draft.name = "Local fixture".into();
        draft.provider = connections::Provider::Compatible;
        draft.base_url = "http://localhost:4321/v1".into();
        draft.model = "original-model".into();
        app.save_chat_provider_from_dialog(false);
        let original_id = app.chat.active_connection().unwrap().id.clone();
        app.chat.active_session_mut().start_turn("fixture prompt");

        app.open_chat_provider_dialog();
        app.chat.provider_dialog.as_mut().unwrap().draft.model = "next-model".into();
        app.save_chat_provider_from_dialog(false);
        assert_eq!(
            app.chat.active_connection().unwrap().model,
            "original-model"
        );
        app.confirm_chat_connection_change();
        assert_eq!(app.chat.active_connection().unwrap().model, "next-model");
        let previous = &app.chat.sessions[1];
        assert_eq!(
            previous.connection_id.as_deref(),
            Some(original_id.as_str())
        );
        app.chat.active_session_id = previous.id.clone();
        assert_eq!(
            app.chat.active_connection().unwrap().model,
            "original-model"
        );
    }
}
