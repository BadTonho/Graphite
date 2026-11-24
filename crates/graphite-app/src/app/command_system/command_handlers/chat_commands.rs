//! Chat, inline chat, and agent session command handlers.

use super::*;
use crate::app::chat::{ChatSurface, SessionsFilter};
use crate::app::clipboard::{ClipboardAccess, SystemClipboard};
use crate::scm_changes::ChangeSet;
use graphite_chat::{save_approval_policy, save_session, ChatSession, EditReviewStatus};
use std::path::PathBuf;

const CHAT_FOCUS: &str = "chatInputVisible || sideBarVisible || true";
const EDITOR_OR_CHAT: &str = "true";

type Entry = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Handler,
);

fn copy_text(app: &mut App, text: String, label: &str) {
    app.chat.last_copied_text = Some(text.clone());
    let mut clipboard = SystemClipboard;
    if clipboard.set_text(&text).is_ok() {
        app.status = format!("Copied {label}");
    } else {
        app.status = format!("Prepared {label} in chat clipboard buffer");
    }
}

fn export_dir(app: &App) -> PathBuf {
    app.profile_root.join("chat-exports")
}

fn latest_file_with_suffix(dir: &Path, suffix: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(suffix) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(t, _)| modified >= *t) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

fn active_selection_with_file(app: &mut App) -> Option<String> {
    let selected = app.editor().selected_texts().join("\n");
    if selected.trim().is_empty() {
        return None;
    }
    let doc_idx = app.active_document();
    let path_label = app
        .documents
        .get(doc_idx)
        .and_then(|doc| doc.buffer.path())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "untitled".to_owned());
    Some(format!("From `{path_label}`:\n```\n{selected}\n```\n"))
}

fn focus_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_chat_sidebar();
    Ok(())
}

fn toggle_chat_visibility(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_chat_sidebar();
    Ok(())
}

fn open_in_sidebar(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_chat_sidebar();
    Ok(())
}

fn open_in_editor(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_chat_in_editor();
    Ok(())
}

fn open_quick_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_quick_chat();
    Ok(())
}

fn close_quick_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_quick_chat();
    Ok(())
}

fn quick_chat_to_view(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.surface = ChatSurface::SideBar;
    app.open_chat_sidebar();
    Ok(())
}

fn new_local_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.new_chat_session();
    if !app.chat.visible {
        app.open_chat_sidebar();
    }
    Ok(())
}

fn new_editor_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.new_chat_session();
    app.open_chat_in_editor();
    Ok(())
}

fn ask_selection_in_side_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(snippet) = active_selection_with_file(app) {
        app.chat.input = snippet;
    }
    app.open_chat_sidebar();
    Ok(())
}

fn send_to_new_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let snippet = active_selection_with_file(app).unwrap_or_else(|| app.chat.input.clone());
    app.new_chat_session();
    app.chat.input = snippet;
    app.open_chat_sidebar();
    Ok(())
}

fn add_files_from_references(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let doc_idx = app.active_document();
    if let Some(path) = app
        .documents
        .get(doc_idx)
        .and_then(|d| d.buffer.path())
        .map(|p| p.display().to_string())
    {
        if !app.chat.input.contains(&path) {
            if !app.chat.input.is_empty() && !app.chat.input.ends_with(' ') {
                app.chat.input.push(' ');
            }
            app.chat.input.push_str(&format!("#file:{path} "));
        }
        app.open_chat_sidebar();
        app.status = format!("Added reference {path}");
    } else {
        app.status = "Active document has no file path".into();
    }
    Ok(())
}

fn retry_turn(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.retry_last_chat_turn();
    Ok(())
}

fn accept_all_files(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.accept_all_chat_edits();
    Ok(())
}

fn discard_all_files(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.discard_all_chat_edits();
    Ok(())
}

fn accept_active_file(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let active_path = app
        .documents
        .get(app.active_document())
        .and_then(|d| d.buffer.path())
        .map(|p| p.to_path_buf());
    let target = active_path.or_else(|| {
        app.chat
            .active_session()
            .pending_edits
            .iter()
            .find(|e| e.status == EditReviewStatus::Pending)
            .map(|e| e.path.clone())
    });
    if let Some(path) = target {
        app.accept_chat_edit(&path);
    }
    Ok(())
}

fn discard_active_file(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let active_path = app
        .documents
        .get(app.active_document())
        .and_then(|d| d.buffer.path())
        .map(|p| p.to_path_buf());
    let target = active_path.or_else(|| {
        app.chat
            .active_session()
            .pending_edits
            .iter()
            .find(|e| e.status == EditReviewStatus::Pending)
            .map(|e| e.path.clone())
    });
    if let Some(path) = target {
        app.discard_chat_edit(&path);
    }
    Ok(())
}

fn open_pending_file_diff(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let pending = app
        .chat
        .active_session()
        .pending_edits
        .iter()
        .find(|e| e.status == EditReviewStatus::Pending)
        .cloned()
        .or_else(|| app.chat.active_session().pending_edits.last().cloned());
    if let Some(edit) = pending {
        let right = PathBuf::from(&edit.path);
        let dir = export_dir(app);
        let _ = std::fs::create_dir_all(&dir);
        let name = right
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.txt");
        let left = dir.join(format!("before-{name}"));
        let _ = std::fs::write(&left, edit.before.as_deref().unwrap_or(""));
        app.open_diff_paths(left, right);
    } else {
        app.open_multi_diff(ChangeSet::All);
    }
    Ok(())
}

fn undo_edit(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.undo_chat_edit();
    Ok(())
}

fn redo_edit(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.redo_chat_edit();
    Ok(())
}

fn clear_history(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    {
        let s = app.chat.active_session_mut();
        s.turns.clear();
        s.pending_edits.clear();
        s.checkpoints.clear();
        s.undone_checkpoints.clear();
        s.log("session.clear", "Cleared conversation turns");
    }
    app.chat.scroll = 0;
    app.chat.selected_code_block = 0;
    let _ = save_session(&app.profile_root, app.chat.active_session());
    app.status = "Cleared chat history".into();
    Ok(())
}

fn clear_input_history(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.active_session_mut().input_history.clear();
    app.chat.input_history_cursor = None;
    let _ = save_session(&app.profile_root, app.chat.active_session());
    app.status = "Cleared chat input history".into();
    Ok(())
}

fn compact_conversation(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let removed = app.chat.active_session_mut().compact_conversation(4);
    let _ = save_session(&app.profile_root, app.chat.active_session());
    app.status = format!("Compacted {removed} older chat turns");
    Ok(())
}

fn copy_all_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let md = app.chat.active_session().export_markdown();
    copy_text(app, md, "full chat conversation");
    Ok(())
}

fn copy_final_response(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(turn) = app.chat.active_session().turns.last() else {
        app.status = "No chat response to copy".into();
        return Ok(());
    };
    copy_text(app, turn.assistant_text(), "final chat response");
    Ok(())
}

fn copy_code_block(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(block) = app.active_chat_code_block() else {
        app.status = "No code block in active chat".into();
        return Ok(());
    };
    copy_text(app, block.code, "chat code block");
    Ok(())
}

fn copy_katex_source(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(turn) = app.chat.active_session().turns.last() else {
        app.status = "No math block in chat".into();
        return Ok(());
    };
    let reply = turn.assistant_text();
    let tex = if let Some(start) = reply.find("$$") {
        let rest = &reply[start + 2..];
        rest.find("$$")
            .map(|end| rest[..end].trim().to_owned())
            .unwrap_or_else(|| reply.clone())
    } else {
        reply
    };
    copy_text(app, tex, "math source");
    Ok(())
}

fn copy_session_link(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let link = format!("graphite://chat/session/{}", app.chat.active_session().id);
    copy_text(app, link, "chat session link");
    Ok(())
}

fn insert_code_block(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.insert_active_code_block(false);
    Ok(())
}

fn insert_into_new_file(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.insert_active_code_block(true);
    Ok(())
}

fn insert_into_notebook(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(block) = app.active_chat_code_block() else {
        app.status = "No code block in active chat".into();
        return Ok(());
    };
    if app.active_notebook().is_some() {
        let code = block.code;
        app.with_notebook(|session| {
            let idx = session.selected().0;
            session.insert_cell(idx + 1, crate::notebook::CellKind::Code, &code);
        });
        app.status = "Inserted chat code block into notebook".into();
    } else {
        app.insert_active_code_block(false);
    }
    Ok(())
}

fn insert_search_results(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let count = app.search_results.len();
    if count == 0 {
        app.status = "No active search results to insert".into();
        return Ok(());
    }
    let summary: Vec<String> = app
        .search_results
        .iter()
        .take(20)
        .map(|m| format!("{}:{}: {}", m.path.display(), m.line, m.text.trim()))
        .collect();
    if !app.chat.input.is_empty() && !app.chat.input.ends_with('\n') {
        app.chat.input.push('\n');
    }
    app.chat.input.push_str(&format!(
        "Search results ({count} matches):\n{}\n",
        summary.join("\n")
    ));
    app.open_chat_sidebar();
    app.status = format!("Added {} search results to chat", summary.len());
    Ok(())
}

fn run_in_terminal(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_active_code_block_in_terminal();
    Ok(())
}

fn next_code_block(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.step_chat_code_block(true);
    Ok(())
}

fn previous_code_block(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.step_chat_code_block(false);
    Ok(())
}

fn step_user_prompt(app: &mut App, forward: bool) {
    let len = app.chat.active_session().turns.len();
    if len == 0 {
        return;
    }
    let idx = if forward {
        (app.chat.scroll + 1).min(len - 1)
    } else {
        app.chat.scroll.saturating_sub(1)
    };
    let prompt = app
        .chat
        .active_session()
        .turns
        .get(idx)
        .map(|t| t.prompt.clone());
    app.chat.scroll = idx;
    if let Some(prompt) = prompt {
        app.chat.input = prompt;
    }
}

fn next_user_prompt(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    step_user_prompt(app, true);
    Ok(())
}

fn previous_user_prompt(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    step_user_prompt(app, false);
    Ok(())
}

fn export_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let dir = export_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.chat.json", app.chat.active_session().id));
    let json = app
        .chat
        .active_session()
        .export_json()
        .map_err(|e| CommandError::Failed(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| CommandError::Failed(e.to_string()))?;
    app.open_file(path.clone());
    app.status = format!("Exported chat to {}", path.display());
    Ok(())
}

fn import_chat(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if app.chat_connection_busy() {
        return Ok(());
    }
    let is_chat_json = app
        .documents
        .get(app.active_document())
        .and_then(|d| d.buffer.path())
        .is_some_and(|p| p.to_string_lossy().ends_with(".chat.json"));
    let raw = if is_chat_json {
        app.editor().text()
    } else if let Some(path) = latest_file_with_suffix(&export_dir(app), ".chat.json") {
        std::fs::read_to_string(&path).map_err(|e| CommandError::Failed(e.to_string()))?
    } else {
        app.status = "No exported chat file found to import".into();
        return Ok(());
    };
    let mut imported =
        ChatSession::import_json(&raw).map_err(|e| CommandError::Failed(e.to_string()))?;
    imported.connection_id = None;
    imported.connection_destination = None;
    imported.opencode_session = None;
    let id = imported.id.clone();
    if let Some(existing) = app.chat.sessions.iter_mut().find(|s| s.id == id) {
        *existing = imported;
    } else {
        if imported.title.is_empty() {
            imported.title = format!("Imported {}", imported.id);
        }
        app.chat.sessions.insert(0, imported);
    }
    app.chat.clear_outgoing_draft();
    app.chat.active_session_id = id.clone();
    let _ = save_session(&app.profile_root, app.chat.active_session());
    app.open_chat_sidebar();
    app.status = format!("Imported chat session {id}");
    Ok(())
}

fn export_debug_log(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let dir = export_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.debug.json", app.chat.active_session().id));
    let json = serde_json::to_string_pretty(&app.chat.active_session().debug_log)
        .map_err(|e| CommandError::Failed(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| CommandError::Failed(e.to_string()))?;
    app.open_file(path.clone());
    app.status = format!("Exported agent debug log to {}", path.display());
    Ok(())
}

fn import_debug_log(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(path) = latest_file_with_suffix(&export_dir(app), ".debug.json") else {
        app.status = "No exported agent debug log found".into();
        return Ok(());
    };
    let raw = std::fs::read_to_string(&path).map_err(|e| CommandError::Failed(e.to_string()))?;
    let logs: Vec<graphite_chat::AgentLogEntry> =
        serde_json::from_str(&raw).map_err(|e| CommandError::Failed(e.to_string()))?;
    app.chat.active_session_mut().debug_log = logs;
    app.chat.debug_panel_open = true;
    app.open_chat_sidebar();
    app.status = format!("Imported agent debug log from {}", path.display());
    Ok(())
}

fn edit_tool_approval(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.policy.auto_approve_read_only = !app.chat.policy.auto_approve_read_only;
    let _ = save_approval_policy(&app.profile_root, &app.chat.policy);
    app.status = format!(
        "Chat auto-approve read-only tools: {}",
        app.chat.policy.auto_approve_read_only
    );
    Ok(())
}

fn reset_trusted_tools(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.policy.auto_approve_read_only = true;
    app.chat.policy.trusted_tools.clear();
    let _ = save_approval_policy(&app.profile_root, &app.chat.policy);
    app.status = "Reset trusted chat tools".into();
    Ok(())
}

fn toggle_debug_panel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.debug_panel_open = !app.chat.debug_panel_open;
    app.open_chat_sidebar();
    Ok(())
}

fn toggle_context_usage(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.show_context_usage = !app.chat.show_context_usage;
    let tokens = app.chat.active_session().estimated_tokens();
    app.status = format!("Estimated chat context: ~{tokens} tokens");
    Ok(())
}

fn toggle_default_visibility(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.default_visible = !app.chat.default_visible;
    app.status = if app.chat.default_visible {
        "Chat default visibility enabled"
    } else {
        "Chat default visibility disabled"
    }
    .into();
    Ok(())
}

fn tip_action(app: &mut App, kind: u8) {
    const TIP_COUNT: usize = 4;
    match kind {
        0 => app.chat.tips_disabled = true,
        1 => {
            if !app.chat.dismissed_tips.contains(&app.chat.tip_index) {
                app.chat.dismissed_tips.push(app.chat.tip_index);
            }
            app.chat.tip_index = (app.chat.tip_index + 1) % TIP_COUNT;
        }
        2 => app.chat.tip_index = (app.chat.tip_index + 1) % TIP_COUNT,
        3 => app.chat.tip_index = (app.chat.tip_index + TIP_COUNT - 1) % TIP_COUNT,
        4 => {
            app.chat.tips_disabled = false;
            app.chat.dismissed_tips.clear();
        }
        _ => {}
    }
}

fn open_chat_connections(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_chat_provider_dialog();
    Ok(())
}

fn open_chat_settings(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_preference_file("settings.json");
    Ok(())
}

fn browse_agent_plugins(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_plugins();
    Ok(())
}

fn new_untitled_prompt(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.new_untitled_document();
    app.editor_mut()
        .insert("---\nmode: agent\ndescription: Reusable chat prompt\n---\n\n");
    app.status = "Created untitled prompt template".into();
    Ok(())
}

fn pin_session(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_pin_active_session(Some(true));
    Ok(())
}

fn unpin_session(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_pin_active_session(Some(false));
    Ok(())
}

fn archive_session(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_archive_active_session(Some(true));
    Ok(())
}

fn unarchive_session(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_archive_active_session(Some(false));
    Ok(())
}

fn archive_all_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    for session in &mut app.chat.sessions {
        session.archived = true;
        let _ = save_session(&app.profile_root, session);
    }
    app.status = "Archived all agent sessions".into();
    Ok(())
}

fn unarchive_all_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    for session in &mut app.chat.sessions {
        session.archived = false;
        let _ = save_session(&app.profile_root, session);
    }
    app.status = "Unarchived all agent sessions".into();
    Ok(())
}

fn mark_all_sessions_read(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    for session in &mut app.chat.sessions {
        session.unread = false;
        let _ = save_session(&app.profile_root, session);
    }
    app.status = "Marked all agent sessions read".into();
    Ok(())
}

fn toggle_show_agent_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.show_agent_sessions = !app.chat.show_agent_sessions;
    app.open_chat_sidebar();
    Ok(())
}

fn collapse_agent_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.show_agent_sessions = false;
    Ok(())
}

fn find_agent_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.show_agent_sessions = true;
    app.open_chat_sidebar();
    Ok(())
}

fn refresh_agent_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let loaded = graphite_chat::load_sessions(&app.profile_root);
    if !loaded.is_empty() {
        app.chat.sessions = loaded;
        if !app
            .chat
            .sessions
            .iter()
            .any(|s| s.id == app.chat.active_session_id)
        {
            app.chat.active_session_id = app.chat.sessions[0].id.clone();
        }
    }
    app.status = format!("Refreshed {} agent sessions", app.chat.sessions.len());
    Ok(())
}

fn orientation_side_by_side(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.sessions_side_by_side = true;
    app.chat.show_agent_sessions = true;
    app.open_chat_sidebar();
    Ok(())
}

fn orientation_stacked(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.chat.sessions_side_by_side = false;
    app.chat.show_agent_sessions = true;
    app.open_chat_sidebar();
    Ok(())
}

fn filter_sessions(app: &mut App, filter: SessionsFilter) {
    app.chat.sessions_filter = filter;
    app.chat.show_agent_sessions = true;
    app.open_chat_sidebar();
}

fn inline_focus(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_inline_chat();
    Ok(())
}

fn inline_fix_diagnostics(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let doc_idx = app.active_document();
    let msgs: Vec<String> = app
        .documents
        .get(doc_idx)
        .map(|d| {
            d.diagnostics
                .iter()
                .chain(&d.schema_diagnostics)
                .take(5)
                .map(|diag| format!("Line {}: {}", diag.range.start.line + 1, diag.message))
                .collect()
        })
        .unwrap_or_default();
    app.open_inline_chat();
    if let Some(inline) = app.chat.inline_chat.as_mut() {
        inline.prompt = if msgs.is_empty() {
            "Fix any issues in this selection".into()
        } else {
            format!("Fix diagnostics:\n{}", msgs.join("\n"))
        };
    }
    Ok(())
}

fn inline_close(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_inline_chat();
    Ok(())
}

fn inline_keep(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.keep_inline_chat();
    Ok(())
}

fn inline_continue(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.continue_inline_in_chat();
    Ok(())
}

fn inline_rephrase(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if app.chat.inline_chat.is_none() {
        app.open_inline_chat();
    }
    if let Some(inline) = app.chat.inline_chat.as_mut() {
        inline.error = None;
        inline.proposed_replacement = None;
        inline.response.clear();
    }
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let entries: &[Entry] = &[
        (
            "chat.action.focus",
            "Focus Chat",
            "Chat",
            CHAT_FOCUS,
            focus_chat,
        ),
        (
            "workbench.chat.action.focusLastFocused",
            "Focus Last Chat",
            "Chat",
            CHAT_FOCUS,
            focus_chat,
        ),
        (
            "workbench.action.chat.focusInput",
            "Focus Chat Input",
            "Chat",
            CHAT_FOCUS,
            focus_chat,
        ),
        (
            "workbench.action.chat.openInSidebar",
            "Open Chat in Side Bar",
            "Chat",
            EDITOR_OR_CHAT,
            open_in_sidebar,
        ),
        (
            "workbench.action.chat.openInEditor",
            "Open Chat in Editor",
            "Chat",
            EDITOR_OR_CHAT,
            open_in_editor,
        ),
        (
            "workbench.action.openChatToSide",
            "Open Chat to the Side",
            "Chat",
            EDITOR_OR_CHAT,
            toggle_chat_visibility,
        ),
        (
            "graphite.chat.openQuickChat",
            "Open Quick Chat",
            "Chat",
            EDITOR_OR_CHAT,
            open_quick_chat,
        ),
        (
            "workbench.action.quickchat.close",
            "Close Quick Chat",
            "Chat",
            EDITOR_OR_CHAT,
            close_quick_chat,
        ),
        (
            "workbench.action.quickchat.openInChatView",
            "Open Quick Chat in Chat View",
            "Chat",
            EDITOR_OR_CHAT,
            quick_chat_to_view,
        ),
        (
            "workbench.action.chat.newLocalChat",
            "New Chat",
            "Chat",
            EDITOR_OR_CHAT,
            new_local_chat,
        ),
        (
            "workbench.action.chatEditor.newChat",
            "New Chat Editor",
            "Chat",
            EDITOR_OR_CHAT,
            new_editor_chat,
        ),
        (
            "workbench.action.chat.askSelectionInSideChat",
            "Ask Selection in Side Chat",
            "Chat",
            EDITOR_OR_CHAT,
            ask_selection_in_side_chat,
        ),
        (
            "workbench.action.chat.sendToNewChat",
            "Send to New Chat",
            "Chat",
            EDITOR_OR_CHAT,
            send_to_new_chat,
        ),
        (
            "workbench.action.edits.addFilesFromReferences",
            "Add Files from References",
            "Chat",
            EDITOR_OR_CHAT,
            add_files_from_references,
        ),
        (
            "workbench.action.chat.retry",
            "Retry Last Request",
            "Chat",
            EDITOR_OR_CHAT,
            retry_turn,
        ),
        (
            "chatEditing.acceptAllFiles",
            "Accept All Edits",
            "Chat",
            EDITOR_OR_CHAT,
            accept_all_files,
        ),
        (
            "chatEditing.acceptFile",
            "Accept File Edits",
            "Chat",
            EDITOR_OR_CHAT,
            accept_active_file,
        ),
        (
            "chatEditing.discardAllFiles",
            "Discard All Edits",
            "Chat",
            EDITOR_OR_CHAT,
            discard_all_files,
        ),
        (
            "chatEditing.discardFile",
            "Discard File Edits",
            "Chat",
            EDITOR_OR_CHAT,
            discard_active_file,
        ),
        (
            "chatEditing.openFileInDiff",
            "Open Chat Edit in Diff",
            "Chat",
            EDITOR_OR_CHAT,
            open_pending_file_diff,
        ),
        (
            "chatEditor.action.reviewChanges",
            "Review Chat Changes",
            "Chat",
            EDITOR_OR_CHAT,
            open_pending_file_diff,
        ),
        (
            "chatEditor.action.toggleDiff",
            "Toggle Chat Diff",
            "Chat",
            EDITOR_OR_CHAT,
            open_pending_file_diff,
        ),
        (
            "chatEditor.action.showAccessibleDiffView",
            "Show Accessible Chat Diff",
            "Chat",
            EDITOR_OR_CHAT,
            open_pending_file_diff,
        ),
        (
            "workbench.action.chat.applyCompareEdits",
            "Apply Compare Edits",
            "Chat",
            EDITOR_OR_CHAT,
            accept_all_files,
        ),
        (
            "workbench.action.chat.discardCompareEdits",
            "Discard Compare Edits",
            "Chat",
            EDITOR_OR_CHAT,
            discard_all_files,
        ),
        (
            "workbench.action.chat.openCompareBlockInDiffEditor",
            "Open Compare Block in Diff Editor",
            "Chat",
            EDITOR_OR_CHAT,
            open_pending_file_diff,
        ),
        (
            "workbench.action.chat.undoEdit",
            "Undo Chat Edit",
            "Chat",
            EDITOR_OR_CHAT,
            undo_edit,
        ),
        (
            "workbench.action.chat.undoEdits",
            "Undo Chat Edits",
            "Chat",
            EDITOR_OR_CHAT,
            undo_edit,
        ),
        (
            "workbench.action.chat.redoEdit",
            "Redo Chat Edit",
            "Chat",
            EDITOR_OR_CHAT,
            redo_edit,
        ),
        (
            "workbench.action.chat.redoEdit2",
            "Redo Chat Edit (Session)",
            "Chat",
            EDITOR_OR_CHAT,
            redo_edit,
        ),
        (
            "workbench.action.chat.restoreLastCheckpoint",
            "Restore Last Checkpoint",
            "Chat",
            EDITOR_OR_CHAT,
            undo_edit,
        ),
        (
            "workbench.action.chat.clearHistory",
            "Clear Chat History",
            "Chat",
            EDITOR_OR_CHAT,
            clear_history,
        ),
        (
            "workbench.action.chat.clearInputHistory",
            "Clear Chat Input History",
            "Chat",
            EDITOR_OR_CHAT,
            clear_input_history,
        ),
        (
            "workbench.action.chat.compactAgentHostConversation",
            "Compact Chat Conversation",
            "Chat",
            EDITOR_OR_CHAT,
            compact_conversation,
        ),
        (
            "workbench.action.chat.copyAll",
            "Copy All Chat",
            "Chat",
            EDITOR_OR_CHAT,
            copy_all_chat,
        ),
        (
            "workbench.action.chat.copyFinalResponse",
            "Copy Final Response",
            "Chat",
            EDITOR_OR_CHAT,
            copy_final_response,
        ),
        (
            "workbench.action.chat.copyCodeBlock",
            "Copy Code Block",
            "Chat",
            EDITOR_OR_CHAT,
            copy_code_block,
        ),
        (
            "workbench.action.chat.copyKatexMathSource",
            "Copy Math Source",
            "Chat",
            EDITOR_OR_CHAT,
            copy_katex_source,
        ),
        (
            "workbench.action.chat.copySessionLink",
            "Copy Session Link",
            "Chat",
            EDITOR_OR_CHAT,
            copy_session_link,
        ),
        (
            "workbench.action.chat.insertCodeBlock",
            "Insert Code Block at Cursor",
            "Chat",
            EDITOR_OR_CHAT,
            insert_code_block,
        ),
        (
            "workbench.action.chat.insertIntoNewFile",
            "Insert Code Block Into New File",
            "Chat",
            EDITOR_OR_CHAT,
            insert_into_new_file,
        ),
        (
            "workbench.action.chat.insertIntoNotebook",
            "Insert Code Block Into Notebook",
            "Chat",
            EDITOR_OR_CHAT,
            insert_into_notebook,
        ),
        (
            "workbench.action.chat.insertSearchResults",
            "Insert Search Results Into Chat",
            "Chat",
            EDITOR_OR_CHAT,
            insert_search_results,
        ),
        (
            "workbench.action.chat.runInTerminal",
            "Run Code Block in Terminal",
            "Chat",
            EDITOR_OR_CHAT,
            run_in_terminal,
        ),
        (
            "workbench.action.chat.nextCodeBlock",
            "Next Code Block",
            "Chat",
            EDITOR_OR_CHAT,
            next_code_block,
        ),
        (
            "workbench.action.chat.previousCodeBlock",
            "Previous Code Block",
            "Chat",
            EDITOR_OR_CHAT,
            previous_code_block,
        ),
        (
            "workbench.action.chat.nextUserPrompt",
            "Next User Prompt",
            "Chat",
            EDITOR_OR_CHAT,
            next_user_prompt,
        ),
        (
            "workbench.action.chat.previousUserPrompt",
            "Previous User Prompt",
            "Chat",
            EDITOR_OR_CHAT,
            previous_user_prompt,
        ),
        (
            "workbench.action.chat.editRequests",
            "Edit Last Request",
            "Chat",
            EDITOR_OR_CHAT,
            previous_user_prompt,
        ),
        (
            "workbench.action.chat.export",
            "Export Chat",
            "Chat",
            EDITOR_OR_CHAT,
            export_chat,
        ),
        (
            "workbench.action.chat.import",
            "Import Chat",
            "Chat",
            EDITOR_OR_CHAT,
            import_chat,
        ),
        (
            "workbench.action.chat.exportAgentDebugLog",
            "Export Agent Debug Log",
            "Chat",
            EDITOR_OR_CHAT,
            export_debug_log,
        ),
        (
            "workbench.action.chat.importAgentDebugLog",
            "Import Agent Debug Log",
            "Chat",
            EDITOR_OR_CHAT,
            import_debug_log,
        ),
        (
            "workbench.action.chat.editToolApproval",
            "Cycle Tool Approval Policy",
            "Chat",
            EDITOR_OR_CHAT,
            edit_tool_approval,
        ),
        (
            "workbench.action.chat.resetTrustedTools",
            "Reset Trusted Tools",
            "Chat",
            EDITOR_OR_CHAT,
            reset_trusted_tools,
        ),
        (
            "workbench.action.chat.openAgentDebugPanel",
            "Toggle Agent Debug Panel",
            "Chat",
            EDITOR_OR_CHAT,
            toggle_debug_panel,
        ),
        (
            "workbench.action.chat.openAgentDebugPanelForSession",
            "Open Agent Debug Panel for Session",
            "Chat",
            EDITOR_OR_CHAT,
            toggle_debug_panel,
        ),
        (
            "workbench.action.chat.showContextUsage",
            "Show Chat Context Usage",
            "Chat",
            EDITOR_OR_CHAT,
            toggle_context_usage,
        ),
        (
            "workbench.action.chat.toggleShowContextUsage",
            "Toggle Context Usage Display",
            "Chat",
            EDITOR_OR_CHAT,
            toggle_context_usage,
        ),
        (
            "workbench.action.chat.toggleDefaultVisibility",
            "Toggle Chat Default Visibility",
            "Chat",
            EDITOR_OR_CHAT,
            toggle_default_visibility,
        ),
        (
            "workbench.action.chat.disableTips",
            "Disable Chat Tips",
            "Chat",
            EDITOR_OR_CHAT,
            |app, _| {
                tip_action(app, 0);
                Ok(())
            },
        ),
        (
            "workbench.action.chat.dismissTip",
            "Dismiss Current Chat Tip",
            "Chat",
            EDITOR_OR_CHAT,
            |app, _| {
                tip_action(app, 1);
                Ok(())
            },
        ),
        (
            "workbench.action.chat.dismissTipToolbar",
            "Dismiss Chat Tip Toolbar",
            "Chat",
            EDITOR_OR_CHAT,
            |app, _| {
                tip_action(app, 1);
                Ok(())
            },
        ),
        (
            "workbench.action.chat.nextTip",
            "Next Chat Tip",
            "Chat",
            EDITOR_OR_CHAT,
            |app, _| {
                tip_action(app, 2);
                Ok(())
            },
        ),
        (
            "workbench.action.chat.previousTip",
            "Previous Chat Tip",
            "Chat",
            EDITOR_OR_CHAT,
            |app, _| {
                tip_action(app, 3);
                Ok(())
            },
        ),
        (
            "workbench.action.chat.resetDismissedTips",
            "Reset Dismissed Chat Tips",
            "Chat",
            EDITOR_OR_CHAT,
            |app, _| {
                tip_action(app, 4);
                Ok(())
            },
        ),
        (
            "workbench.action.openLanguageModelsJson",
            "Configure Chat Connections",
            "Chat",
            EDITOR_OR_CHAT,
            open_chat_connections,
        ),
        (
            "workbench.action.chat.manageSettings",
            "Manage Chat Settings",
            "Chat",
            EDITOR_OR_CHAT,
            open_chat_settings,
        ),
        (
            "workbench.action.chat.openAgentHostSettings",
            "Open Agent Host Settings",
            "Chat",
            EDITOR_OR_CHAT,
            open_chat_settings,
        ),
        (
            "workbench.action.chat.openAgentSessionSettings",
            "Open Agent Session Settings",
            "Chat",
            EDITOR_OR_CHAT,
            open_chat_settings,
        ),
        (
            "workbench.action.chat.openFeatureSettings",
            "Open Chat Feature Settings",
            "Chat",
            EDITOR_OR_CHAT,
            open_chat_settings,
        ),
        (
            "workbench.agentPlugins.browse",
            "Browse Agent Plugins",
            "Chat",
            EDITOR_OR_CHAT,
            browse_agent_plugins,
        ),
        (
            "workbench.action.chat.addPluginMarketplace",
            "Add Agent Plugin Marketplace",
            "Chat",
            EDITOR_OR_CHAT,
            browse_agent_plugins,
        ),
        (
            "workbench.command.new.untitled.prompt",
            "New Untitled Prompt File",
            "Chat",
            EDITOR_OR_CHAT,
            new_untitled_prompt,
        ),
        // Inline Chat
        (
            "inlineChat.focus",
            "Start Inline Chat",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_focus,
        ),
        (
            "inlineChat.fixDiagnostics",
            "Fix Diagnostics with Inline Chat",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_fix_diagnostics,
        ),
        (
            "inlineChat.dismissEditorAffordance",
            "Dismiss Inline Chat",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_close,
        ),
        (
            "inlineChat2.close",
            "Close Inline Chat",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_close,
        ),
        (
            "inlineChat2.keep",
            "Accept Inline Chat Changes",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_keep,
        ),
        (
            "inlineChat2.continueInChat",
            "Continue Inline Chat in Side Bar",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_continue,
        ),
        (
            "inlineChat2.rephrase",
            "Rephrase Inline Chat Prompt",
            "Inline Chat",
            EDITOR_OR_CHAT,
            inline_rephrase,
        ),
        // Agent Sessions
        (
            "agentSession.pin",
            "Pin Agent Session",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            pin_session,
        ),
        (
            "agentSession.unpin",
            "Unpin Agent Session",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            unpin_session,
        ),
        (
            "agentSession.archive",
            "Archive Agent Session",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            archive_session,
        ),
        (
            "agentSession.unarchive",
            "Unarchive Agent Session",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            unarchive_session,
        ),
        (
            "agentSessionSection.archive",
            "Archive Session Section",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            archive_all_sessions,
        ),
        (
            "agentSessionSection.unarchive",
            "Unarchive Session Section",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            unarchive_all_sessions,
        ),
        (
            "agentSessionSection.markRead",
            "Mark Session Section Read",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            mark_all_sessions_read,
        ),
        (
            "agentSessionSection.collapseAll",
            "Collapse Agent Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            collapse_agent_sessions,
        ),
        (
            "agentSessionsViewer.find",
            "Filter Agent Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            find_agent_sessions,
        ),
        (
            "agentSessionsViewer.refresh",
            "Refresh Agent Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            refresh_agent_sessions,
        ),
        (
            "workbench.action.chat.archiveAllAgentSessions",
            "Archive All Agent Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            archive_all_sessions,
        ),
        (
            "workbench.action.chat.markAllAgentSessionsRead",
            "Mark All Agent Sessions Read",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            mark_all_sessions_read,
        ),
        (
            "workbench.action.chat.toggleShowAgentSessions",
            "Toggle Agent Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            toggle_show_agent_sessions,
        ),
        (
            "workbench.action.chat.setAgentSessionsOrientationSideBySide",
            "Set Agent Sessions Side by Side",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            orientation_side_by_side,
        ),
        (
            "workbench.action.chat.setAgentSessionsOrientationStacked",
            "Set Agent Sessions Stacked",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            orientation_stacked,
        ),
        (
            "sessionsViewPane.filterArchived",
            "Filter Archived Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            |app, _| {
                filter_sessions(app, SessionsFilter::Archived);
                Ok(())
            },
        ),
        (
            "sessionsViewPane.filterAutomationRuns",
            "Filter Automation Run Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            |app, _| {
                filter_sessions(app, SessionsFilter::AutomationRuns);
                Ok(())
            },
        ),
        (
            "sessionsViewPane.filterScheduled",
            "Filter Scheduled Sessions",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            |app, _| {
                filter_sessions(app, SessionsFilter::Scheduled);
                Ok(())
            },
        ),
        (
            "sessionsViewPane.resetFilters",
            "Reset Agent Session Filters",
            "Agent Sessions",
            EDITOR_OR_CHAT,
            |app, _| {
                filter_sessions(app, SessionsFilter::All);
                Ok(())
            },
        ),
    ];

    for (id, title, category, enablement, handler) in entries {
        add(
            registry, id, title, category, ALL, enablement, None, *handler,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::InlineChatState;
    use graphite_chat::PendingFileEdit;

    #[test]
    fn chat_commands_open_export_import_copy_and_edit_cycles_work() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);

        app.run_ui_command("chat.action.focus");
        assert!(app.chat.visible);
        assert_eq!(app.chat.surface, ChatSurface::SideBar);

        app.run_ui_command("workbench.action.chat.openInEditor");
        assert_eq!(app.chat.surface, ChatSurface::Editor);

        app.run_ui_command("graphite.chat.openQuickChat");
        assert_eq!(app.chat.surface, ChatSurface::QuickChat);
        app.run_ui_command("workbench.action.quickchat.openInChatView");
        assert_eq!(app.chat.surface, ChatSurface::SideBar);

        let tid = app.chat.active_session_mut().start_turn("Explain rust");
        app.chat
            .active_session_mut()
            .append_text(tid, "Here:\n```rust\nfn demo() {}\n```");

        app.run_ui_command("workbench.action.chat.copyCodeBlock");
        assert_eq!(app.chat.last_copied_text.as_deref(), Some("fn demo() {}"));

        app.run_ui_command("workbench.action.chat.copyFinalResponse");
        assert!(app
            .chat
            .last_copied_text
            .as_deref()
            .is_some_and(|s| s.contains("fn demo() {}")));

        app.run_ui_command("workbench.action.chat.copySessionLink");
        assert!(app
            .chat
            .last_copied_text
            .as_deref()
            .is_some_and(|s| s.starts_with("graphite://chat/session/")));

        let orig_id = app.chat.active_session().id.clone();
        app.run_ui_command("workbench.action.chat.export");
        app.run_ui_command("workbench.action.chat.newLocalChat");
        assert_ne!(app.chat.active_session().id, orig_id);

        app.run_ui_command("workbench.action.chat.import");
        assert_eq!(app.chat.active_session().id, orig_id);

        app.run_ui_command("workbench.action.chat.insertIntoNewFile");
        assert!(app.editor().text().contains("fn demo() {}"));
    }

    #[test]
    fn inline_chat_sessions_and_tool_policy_commands_work() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("sample.rs");
        std::fs::write(&file, "fn x() {}\n").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file.clone());
        app.run_ui_command("editor.action.selectAll");
        app.run_ui_command("inlineChat.focus");
        assert!(app.chat.inline_chat.is_some());
        if let Some(inline) = app.chat.inline_chat.as_mut() {
            inline.proposed_replacement = Some("fn x() { 1 }\n".into());
        }
        app.run_ui_command("inlineChat2.keep");
        assert!(app.chat.inline_chat.is_none());
        assert_eq!(app.editor().text(), "fn x() { 1 }\n");

        let doc_id = app.active_document();
        app.chat.inline_chat = Some(InlineChatState {
            connection: None,
            opencode_session: None,
            document: doc_id,
            anchor_line: 0,
            original_selection: None,
            prompt: "refactor x".into(),
            response: "done".into(),
            proposed_replacement: None,
            running: false,
            error: None,
        });
        app.run_ui_command("inlineChat2.continueInChat");
        assert!(app.chat.inline_chat.is_none());
        assert!(app.chat.visible);
        assert!(app
            .chat
            .active_session()
            .turns
            .iter()
            .any(|t| t.prompt == "refactor x"));

        app.run_ui_command("agentSession.pin");
        assert!(app.chat.active_session().pinned);
        app.run_ui_command("agentSession.archive");
        assert!(app.chat.active_session().archived);
        app.run_ui_command("sessionsViewPane.filterAutomationRuns");
        assert_eq!(app.chat.sessions_filter, SessionsFilter::AutomationRuns);
        app.run_ui_command("sessionsViewPane.resetFilters");
        assert_eq!(app.chat.sessions_filter, SessionsFilter::All);

        app.run_ui_command("workbench.action.chat.editToolApproval");
        assert!(!app.chat.policy.auto_approve_read_only);
        app.chat.policy.trusted_tools.insert("run_command".into());
        app.run_ui_command("workbench.action.chat.resetTrustedTools");
        assert!(app.chat.policy.auto_approve_read_only);
        assert!(app.chat.policy.trusted_tools.is_empty());

        app.chat
            .active_session_mut()
            .pending_edits
            .push(PendingFileEdit {
                path: file,
                before: Some("fn x() {}\n".into()),
                after: "fn x() { 1 }\n".into(),
                status: EditReviewStatus::Pending,
                checkpoint_id: 1,
            });
        app.run_ui_command("chatEditing.acceptFile");
        assert_eq!(
            app.chat.active_session().pending_edits[0].status,
            EditReviewStatus::Accepted
        );
    }
}
