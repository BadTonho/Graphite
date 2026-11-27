//! Resuming a saved task and rendering its recorded activity.

use super::*;
use graphite_chat::{Block, ChatTurn, CommandStatus, FileActivityStatus};

impl App {
    fn continuable_turn(&self) -> Option<&ChatTurn> {
        if self.chat.running_turn.is_some() || self.chat.active_connection().is_none() {
            return None;
        }
        let turn = self.chat.active_session().turns.last()?;
        let workspace = self.tree.root().canonicalize().ok()?;
        (turn.status.can_continue() && turn.workspace.as_ref() == Some(&workspace)).then_some(turn)
    }

    pub(super) fn chat_task_layout(
        &self,
        body: super::layout::Rect,
        lh: f32,
    ) -> (Option<super::layout::Rect>, super::layout::Rect) {
        if body.h < lh * 3.0
            || (self.chat.review_list.is_some()
                && (!self.chat.active_session().proposals.is_empty()
                    || !self.chat.reviewed_in_batch.is_empty()))
            || self.continuable_turn().is_none()
        {
            return (None, body);
        }
        let button = body.row(body.y, lh * 1.5);
        let transcript = body.row(button.bottom(), body.bottom() - button.bottom());
        (Some(button), transcript)
    }

    pub(crate) fn continue_chat_task(&mut self) {
        let Some(prompt) = self.chat_continuation_prompt() else {
            self.status = self.t("chat.task.cannot_continue").into();
            return;
        };
        self.submit_chat_prompt(prompt);
    }

    pub(super) fn chat_continuation_prompt(&self) -> Option<String> {
        let turn = self.continuable_turn()?;
        let mut prompt = self.t("chat.task.continue_prompt").to_owned();
        for line in self.chat_task_activity(turn, false).into_iter().take(40) {
            prompt.push('\n');
            prompt.push_str(&line);
        }
        let count = self
            .chat
            .active_session()
            .proposals
            .iter()
            .filter(|proposal| Some(&proposal.workspace) == turn.workspace.as_ref())
            .count();
        prompt.push('\n');
        prompt.push_str(&self.tf("chat.task.pending_count", &[("count", &count)]));
        Some(prompt)
    }

    pub(super) fn chat_task_activity(&self, turn: &ChatTurn, include_output: bool) -> Vec<String> {
        let blocks: Vec<_> = turn.steps.iter().flat_map(|step| &step.blocks).collect();
        let commands = blocks.iter().filter_map(|block| match block {
            Block::ToolUse { id, name, input } if name == graphite_chat::TOOL_RUN_COMMAND => {
                Some((id, input.get("command").and_then(serde_json::Value::as_str)))
            }
            _ => None,
        });
        let mut lines = Vec::new();
        for (id, command) in commands {
            let label = command.map_or_else(
                || self.t("chat.task.command").into(),
                |command| {
                    crate::app::error_log::sanitize_message(command)
                        .chars()
                        .take(180)
                        .collect::<String>()
                },
            );
            let status = turn
                .commands
                .iter()
                .find(|command| &command.id == id)
                .map_or(CommandStatus::Unknown, |entry| entry.status);
            let detail = match status {
                CommandStatus::Exited(code) => self.tf("chat.task.exit", &[("code", &code)]),
                CommandStatus::Started
                    if !turn.status.can_continue()
                        && !matches!(turn.status, graphite_chat::TurnStatus::Done) =>
                {
                    self.t("chat.task.started").into()
                }
                CommandStatus::Denied => self.t("chat.task.denied").into(),
                CommandStatus::TimedOut => self.t("chat.task.timeout").into(),
                CommandStatus::Cancelled => self.t("chat.task.cancelled").into(),
                CommandStatus::Failed => self.t("chat.task.command_failed").into(),
                CommandStatus::Started | CommandStatus::Unknown => {
                    self.t("chat.task.unknown").into()
                }
            };
            lines.push(format!("{label} · {detail}"));
            if include_output {
                if let Some(content) = blocks.iter().find_map(|block| match block {
                    Block::ToolResult {
                        id: result,
                        content,
                        ..
                    } if result == id => Some(content),
                    _ => None,
                }) {
                    let visible: String = crate::app::error_log::sanitize_message(content)
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .take(4)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .chars()
                        .take(360)
                        .collect();
                    if !visible.is_empty() {
                        lines.push(visible);
                    }
                }
            }
        }
        for file in &turn.files {
            let short = turn
                .workspace
                .as_deref()
                .and_then(|root| file.path.strip_prefix(root).ok())
                .unwrap_or(&file.path);
            let state = match file.status {
                FileActivityStatus::Proposed => "chat.task.proposed",
                FileActivityStatus::Applied => "chat.task.applied",
                FileActivityStatus::Discarded => "chat.task.discarded",
                FileActivityStatus::Conflict => "chat.task.conflict",
                FileActivityStatus::Replaced => "chat.task.replaced",
                FileActivityStatus::Reverted => "chat.task.reverted",
            };
            lines.push(format!("{} · {}", short.display(), self.t(state)));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::{ChatState, ChatUiEvent};
    use graphite_chat::{AgentMode, AgentTurnEvent, ChatSession, TurnStatus, TOOL_RUN_COMMAND};
    use std::sync::{atomic::AtomicBool, mpsc, Arc};

    #[test]
    fn saved_running_turn_recovers_and_offers_continuation_in_its_workspace() {
        let profile = tempfile::tempdir().unwrap();
        let workspace = crate::test_support::TempDir::new();
        let root = workspace.path().canonicalize().unwrap();
        let connection = super::super::connections::Connection {
            id: "fixture".into(),
            name: "Fixture".into(),
            model: "fixture-model".into(),
            ..Default::default()
        };
        let mut session = ChatSession::new("task-fixture", "Task", AgentMode::Agent);
        session.connection_id = Some(connection.id.clone());
        session.connection_destination = Some(connection.destination());
        let id = session.start_turn("Run the checks");
        let turn = session.turn_mut(id).unwrap();
        turn.workspace = Some(root);
        turn.status = TurnStatus::WaitingApproval;
        session.record_tool_use(
            id,
            "tool-id".into(),
            TOOL_RUN_COMMAND.into(),
            serde_json::json!({"command":"cargo test"}),
        );
        session.record_file_status(
            id,
            &workspace.path().join("a.rs"),
            FileActivityStatus::Proposed,
        );
        graphite_chat::save_session(profile.path(), &session).unwrap();

        let mut app = App::new_with_options(workspace.path().to_path_buf(), None, true);
        app.chat = ChatState::load_from_profile(profile.path());
        app.chat.connections.items.push(connection);
        let recovered = app.chat.active_session();
        assert_eq!(recovered.id, "task-fixture");
        assert_eq!(recovered.turns[0].status, TurnStatus::Interrupted);
        assert_eq!(
            graphite_chat::load_sessions(profile.path())[0].turns[0].status,
            TurnStatus::Interrupted
        );
        let activity = app.chat_task_activity(&recovered.turns[0], true).join("\n");
        assert!(activity.contains("cargo test"));
        assert!(activity.contains(app.t("chat.task.unknown")));
        assert!(activity.contains("a.rs"));
        assert!(app.chat_continuation_prompt().unwrap().contains("a.rs"));

        let other = crate::test_support::TempDir::new();
        let mut different = App::new_with_options(other.path().to_path_buf(), None, true);
        different.chat = app.chat;
        assert!(different.chat_continuation_prompt().is_none());
    }

    #[test]
    fn a_late_command_result_updates_only_its_cancelled_turn() {
        let workspace = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(workspace.path().to_path_buf(), None, true);
        let session = app.chat.active_session_id.clone();
        let old_turn = app.chat.active_session_mut().start_turn("run a command");
        app.chat.active_session_mut().record_tool_use(
            old_turn,
            "one".into(),
            TOOL_RUN_COMMAND.into(),
            serde_json::json!({"command":"cargo test"}),
        );
        let token = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session.clone(), old_turn, token.clone()));
        let (reply, result) = mpsc::channel();
        app.handle_chat_ui_event(ChatUiEvent::CommandStarting {
            run: token.clone(),
            session_id: session.clone(),
            turn_id: old_turn,
            id: "one".into(),
            reply,
        });
        assert!(result.try_recv().unwrap());
        app.cancel_chat_turn();
        let new_turn = app.chat.active_session_mut().start_turn("continue");
        let new_token = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session.clone(), new_turn, new_token.clone()));

        for (id, status) in [
            ("unrelated", CommandStatus::Exited(0)),
            ("one", CommandStatus::Cancelled),
        ] {
            app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
                run: token.clone(),
                session_id: session.clone(),
                turn_id: old_turn,
                event: AgentTurnEvent::CommandUpdate {
                    id: id.into(),
                    status,
                },
                approval_reply: None,
            });
        }
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: token,
            session_id: session,
            turn_id: old_turn,
            event: AgentTurnEvent::Cancelled,
            approval_reply: None,
        });
        let active = app.chat.active_session();
        assert_eq!(active.turns[0].status, TurnStatus::Cancelled);
        assert_eq!(active.turns[0].commands[0].status, CommandStatus::Cancelled);
        assert_eq!(active.turns[0].commands.len(), 1);
        assert!(active.turns[1].commands.is_empty());
        assert!(Arc::ptr_eq(
            &app.chat.running_turn.as_ref().unwrap().2,
            &new_token
        ));
    }
}
