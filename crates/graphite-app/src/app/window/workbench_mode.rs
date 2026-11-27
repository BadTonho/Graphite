//! Window presentation for the shared Editor and AI workbench.

use super::*;
use crate::app::{
    chat::ChatSurface,
    editor_groups::{GroupRect, SplitDirection, ViewId},
    focus_regions::FocusRegion,
};

mod review;
pub(crate) use review::ReviewReturn;

pub(crate) struct AiMode {
    editor_view: ViewId,
    editor_focus: FocusRegion,
    editor_chat: (bool, ChatSurface),
    editor_terminal: (bool, bool),
    file_group: Option<u64>,
    review_tab: bool,
    selected_row: Option<usize>,
}

pub(crate) struct AiLayout {
    pub chat: GroupRect,
    pub review: Option<GroupRect>,
    pub review_tabs: Option<[GroupRect; 2]>,
    pub file: Option<(GroupRect, GroupRect)>,
    pub terminal: GroupRect,
}

impl App {
    pub(crate) fn toggle_workbench_mode(&mut self) {
        if self.close_confirm || self.close_window_confirm || self.workspace_decision_pending() {
            return;
        }
        if self.ai_mode.is_some() {
            self.suspend_ai_review();
            self.close_ai_file_panel();
            let mode = self.ai_mode.take().expect("leaving AI mode");
            self.editor_groups.focus_view(mode.editor_view);
            (self.chat.visible, self.chat.surface) = mode.editor_chat;
            self.ai_terminal_layout = (self.terminal_visible, self.terminal_problems);
            (self.terminal_visible, self.terminal_problems) = mode.editor_terminal;
            self.focus_region(
                if self.available_focus_regions().contains(&mode.editor_focus) {
                    mode.editor_focus
                } else {
                    FocusRegion::Editor
                },
            );
        } else {
            self.ai_mode = Some(AiMode {
                editor_view: self.editor_groups.active_view().id,
                editor_focus: self.current_focus_region(),
                editor_chat: (self.chat.visible, self.chat.surface),
                editor_terminal: (self.terminal_visible, self.terminal_problems),
                file_group: None,
                review_tab: false,
                selected_row: None,
            });
            (self.terminal_visible, self.terminal_problems) = self.ai_terminal_layout;
            self.focus_ai_chat();
            self.restore_ai_review();
        }
        let show_editor = self.ai_mode.is_none();
        for view in [self.webview.as_ref(), self.graphite_detail_webview.as_ref()]
            .into_iter()
            .flatten()
        {
            if let Err(error) = view.set_visible(show_editor) {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Medium,
                    "workbench.webview_visibility",
                    &error,
                );
            }
        }
        self.close_workbench_menu();
        self.selecting = false;
        self.panel_resize_dragging = false;
        self.terminal_toolbar_menu = None;
        self.reveal_cursor = false;
        self.resize_terminal();
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn focus_ai_chat(&mut self) {
        self.chat.surface = ChatSurface::Editor;
        self.chat.visible = true;
        self.focus_region(FocusRegion::Chat);
    }

    pub(crate) fn ai_file_group(&self) -> Option<u64> {
        self.ai_mode
            .as_ref()?
            .file_group
            .filter(|&group| self.editor_groups.views(group).is_some())
    }

    pub(crate) fn open_file_in_ai(&mut self, path: PathBuf) {
        let Some(mode) = &self.ai_mode else { return };
        let editor_view = mode.editor_view;
        let previous_view = self.editor_groups.active_view().id;
        self.editor_groups.focus_view(editor_view);
        self.open_text_document(path.clone());
        let Some(document) = self.documents.id_for_path(&path) else {
            self.editor_groups.focus_view(previous_view);
            return;
        };
        self.finish_ai_review();
        let source = self.editor_groups.active_view().id;
        let group = if let Some(group) = self.ai_file_group() {
            self.editor_groups.focus(group);
            let previous = self.editor_groups.active_view().id;
            self.editor_groups
                .activate_document(document, Default::default(), 0);
            if previous != self.editor_groups.active_view().id {
                self.editor_groups.close_view(previous, false);
            }
            group
        } else {
            self.editor_groups
                .split_view(source, SplitDirection::Horizontal)
        };
        self.ai_mode
            .as_mut()
            .expect("opening in AI mode")
            .file_group = Some(group);
        self.editor_groups.focus_view(editor_view);
        self.focus_editor_group(group);
        self.focus_region(FocusRegion::Editor);
        self.reveal_cursor = false;
    }

    pub(crate) fn close_ai_file_panel(&mut self) {
        let Some(mode) = &mut self.ai_mode else {
            return;
        };
        if let Some(group) = mode.file_group.take() {
            let editor_group = self.editor_groups.group_for_view(mode.editor_view);
            if let Some(editor_group) = editor_group {
                self.editor_groups.join_groups(group, editor_group);
                self.editor_groups.focus_view(mode.editor_view);
            }
        }
        self.selecting = false;
        self.reveal_cursor = false;
        self.focus_ai_chat();
    }

    pub(crate) fn ai_layout(&self, width: f32, height: f32, line_height: f32) -> AiLayout {
        let top = self.top_menu_height().min(height);
        let bottom = (height - line_height).max(top);
        let terminal_height = if self.terminal_visible {
            terminal_panel_height_with_delta(height, 0).min((bottom - top) * 0.45)
        } else {
            0.0
        };
        let terminal = GroupRect {
            id: 0,
            x: 0.0,
            y: bottom - terminal_height,
            width,
            height: terminal_height,
        };
        let mut chat = GroupRect {
            id: 0,
            x: 0.0,
            y: top,
            width,
            height: terminal.y - top,
        };
        let mut review_tabs = None;
        let mut review = None;
        let wide = width >= 720.0 * self.sidebar_scale_factor;
        if self.chat_review_open() {
            let mut panel = chat;
            if wide {
                chat.width = (width / 3.0).floor();
                panel.x = chat.width;
                panel.width -= chat.width;
                review = Some(panel);
            } else {
                let tab_height = (line_height * 1.5).min(chat.height);
                let first = GroupRect {
                    width: (width / 2.0).floor(),
                    height: tab_height,
                    ..chat
                };
                let second = GroupRect {
                    x: first.width,
                    width: width - first.width,
                    ..first
                };
                review_tabs = Some([first, second]);
                chat.y += tab_height;
                chat.height -= tab_height;
                if self.ai_review_tab_selected() {
                    review = Some(chat);
                    chat.height = 0.0;
                }
            }
        }
        let file = self
            .ai_file_group()
            .filter(|_| !self.chat_review_open())
            .map(|id| {
                let mut panel = GroupRect { id, ..chat };
                if wide {
                    chat.width = (width * 0.48).floor();
                    panel.x = chat.width;
                    panel.width -= chat.width;
                } else {
                    chat.height = (chat.height * 0.55).floor();
                    panel.y += chat.height;
                    panel.height -= chat.height;
                }
                let header = GroupRect {
                    height: (line_height * 1.5).min(panel.height),
                    ..panel
                };
                panel.y += header.height;
                panel.height -= header.height;
                (header, panel)
            });
        AiLayout {
            chat,
            review,
            review_tabs,
            file,
            terminal,
        }
    }

    pub(crate) fn draw_ai_file_header(&self, r: &mut Renderer, header: GroupRect, lh: f32) {
        let (x, y, w, h) = (header.x, header.y, header.width, header.height);
        r.set_clip(Some([x, y, x + w, y + h]));
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        r.draw_rect(x, y, w, h, background);
        let label = crate::app::group_drawing::tab_label(
            &self.documents[self.active_document()],
            &self.translator,
        );
        let close = self.t("workbench.mode.close_file");
        let close_x = (x + w - (close.chars().count() as f32 * r.char_width() + PAD * 2.0)).max(x);
        r.set_clip(Some([x, y, close_x, y + h]));
        r.draw_text(&label, x + PAD, y + (h - lh) * 0.5, text);
        r.set_clip(Some([close_x, y, x + w, y + h]));
        r.draw_text(close, close_x + PAD, y + (h - lh) * 0.5, text);
        r.set_clip(None);
    }

    pub(crate) fn ai_file_close_hit(&self, header: GroupRect, x: f32, y: f32, cw: f32) -> bool {
        let close = self.t("workbench.mode.close_file");
        let close_width = close.chars().count() as f32 * cw + PAD * 2.0;
        x >= header.x.max(header.x + header.width - close_width)
            && x < header.x + header.width
            && y >= header.y
            && y < header.y + header.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::ChatUiEvent;
    use crate::test_support::TempDir;
    use graphite_chat::{AgentTurnEvent, ApprovalDecision, PendingToolApproval};
    use std::sync::{atomic::AtomicBool, mpsc, Arc};

    #[test]
    fn switching_modes_preserves_the_editor_view_and_an_in_flight_chat() {
        let dir = TempDir::new();
        let file = dir.path().join("editor.txt");
        std::fs::write(&file, "first\nsecond\nthird").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file);
        app.editor_mut().insert("draft ");
        app.editor_mut().set_selection(1, 4);
        app.editor_groups.active_view_mut().scroll_line = 2;
        app.editor_groups.active_view_mut().scroll_col = 3;
        let before = app.editor_groups.active_view().clone();
        let group = app.editor_groups.focused();
        app.open_inline_chat();
        app.chat.inline_chat.as_mut().unwrap().prompt = "inline draft".into();
        let session = app.chat.active_session_id.clone();
        let turn = app.chat.active_session_mut().start_turn("test");
        let run = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session.clone(), turn, run.clone()));
        app.chat.input = "next prompt".into();

        app.run_ui_command("graphite.workbench.toggleMode");
        assert!(app.ai_mode.is_some());
        assert!(app.handle_chat_key(&Key::Character("x".into()), Some("x"), false, false));
        assert_eq!(
            app.chat.inline_chat.as_ref().unwrap().prompt,
            "inline draft"
        );
        assert_eq!(app.available_focus_regions(), vec![FocusRegion::Chat]);
        assert!(app.group_rectangles(1200.0, 800.0, 20.0).is_empty());
        let (reply, approval) = mpsc::channel();
        app.handle_chat_ui_event(ChatUiEvent::TurnEvent {
            run: run.clone(),
            session_id: session.clone(),
            turn_id: turn,
            event: AgentTurnEvent::ApprovalRequested(PendingToolApproval {
                id: "call".into(),
                name: "read_file".into(),
                summary: "Read test file".into(),
                input: serde_json::json!({}),
            }),
            approval_reply: Some(reply),
        });
        app.run_ui_command("graphite.workbench.toggleMode");
        assert!(app.ai_mode.is_none());
        assert_eq!(app.editor_groups.focused(), group);
        assert_eq!(app.editor_groups.active_view(), &before);
        assert!(app.documents[before.document].buffer.is_dirty());
        assert_eq!(app.chat.active_session_id, session);
        assert_eq!(app.chat.input, "next prompt");
        assert!(Arc::ptr_eq(
            &app.chat.running_turn.as_ref().unwrap().2,
            &run
        ));
        assert!(app.chat.pending_approval.is_some());
        app.respond_chat_approval(ApprovalDecision::Deny);
        assert_eq!(approval.try_recv().unwrap(), ApprovalDecision::Deny);
    }

    #[test]
    fn ai_file_panel_uses_the_shared_document_and_keeps_edits_on_close() {
        let dir = TempDir::new();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&second, "second").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(first.clone());
        let original = app.editor_groups.active_view().id;
        let original_doc = app.active_document();

        app.toggle_workbench_mode();
        app.open_file_preview(first);
        assert_eq!(app.active_document(), original_doc);
        assert_ne!(app.editor_groups.active_view().id, original);
        assert_eq!(app.editor_groups.view_count(original_doc), 2);
        app.editor_mut().insert("!");
        assert!(app.documents[original_doc].buffer.is_dirty());
        app.editor_mut().undo();
        assert_eq!(app.documents[original_doc].buffer.text(), "first");

        app.open_file(second.clone());
        let document = app.active_document();
        assert_ne!(document, original_doc);
        app.editor_mut().insert("+");
        app.save();
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "+second");
        app.editor_mut().insert("x");
        let view = app.editor_groups.active_view().id;
        app.open_file(dir.path().join("missing.txt"));
        assert_eq!(app.editor_groups.active_view().id, view);
        assert_eq!(app.documents[document].buffer.text(), "+xsecond");

        app.close_ai_file_panel();
        assert!(app.ai_file_group().is_none());
        assert!(app.chat.focused);
        assert!(app.documents[document].buffer.is_dirty());
        app.toggle_workbench_mode();
        assert_eq!(app.editor_groups.active_view().id, original);
        assert!(app
            .editor_groups
            .all_views()
            .any(|view| view.document == document));
    }

    #[test]
    fn ai_layout_stacks_the_file_when_narrow_and_reuses_the_existing_terminal() {
        let dir = TempDir::new();
        let file = dir.path().join("example.txt");
        std::fs::write(&file, "contents").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let (_, _output) = app.spawn_virtual_terminal("AI layout test").unwrap();
        app.terminal_visible = true;
        app.view_preferences.panel_visible = false;
        app.toggle_workbench_mode();
        assert!(!app.terminal_visible);
        app.open_file(file);
        let wide = app.ai_layout(1200.0, 800.0, 20.0);
        let (header, editor) = wide.file.unwrap();
        assert_eq!(wide.chat.x + wide.chat.width, header.x);
        assert!(editor.width > 0.0);
        let narrow = app.ai_layout(500.0, 800.0, 20.0);
        assert_eq!(narrow.chat.y + narrow.chat.height, narrow.file.unwrap().0.y);
        app.toggle_terminal();
        assert!(app.terminal_focused && !app.chat.focused);
        assert!(!app.view_preferences.panel_visible);
        assert_eq!(app.terminals.len(), 1);
        assert!(app.ai_layout(500.0, 800.0, 20.0).terminal.height > 0.0);
        app.toggle_terminal();
        assert!(app.chat.focused && !app.terminal_visible);
        app.toggle_workbench_mode();
        assert!(app.terminal_visible);
        assert!(!app.view_preferences.panel_visible);
        assert_eq!(app.terminals.len(), 1);
    }
}
