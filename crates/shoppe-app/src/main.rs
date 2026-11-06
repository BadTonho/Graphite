//! Graphite desktop application (v0.1): window, file tree and text editor.

mod app;
use app::accessibility::*;
use app::drawing_helpers::*;
use app::local_search::LocalSearch;
use app::quick_open::QuickOpen;
use app::theme::{Theme, DARK_THEME};

mod config;
mod keybindings;
mod lsp;
mod terminal;
#[cfg(test)]
mod test_support;
mod tree;

use accesskit::{
    Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node,
    NodeId, Role, Tree, TreeUpdate,
};
use lsp::{LspManager, LspUiEvent};
use graphite_render::{rgb, Color, Renderer};
use graphite_syntax::{Class, IncrementalHighlighter, Language, Span, TextEdit};
use graphite_text::Buffer;
use graphite_workspace::SearchResult;
use graphite_workspace::Workspace;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock, RwLock,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use terminal::TerminalSession;
use tree::FileTree;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
    window::{Window, WindowId},
};

const SIDEBAR_WIDTH: f32 = 260.0;
const PAD: f32 = 8.0;

/// Cached highlight result for the currently open buffer revision.
struct HighlightCache {
    revision: u64,
    lines: Vec<Vec<Span>>,
}

struct Gpu {
    window: Arc<Window>,
    renderer: Renderer,
    accessibility: accesskit_winit::Adapter,
    accessibility_actions: Arc<std::sync::Mutex<Vec<ActionRequest>>>,
}

struct Tab {
    id: u64,
    buffer: Buffer,
    scroll_line: usize,
    language: Option<Language>,
    highlights: Option<HighlightCache>,
    highlighter: IncrementalHighlighter,
    diagnostics: Vec<lsp_types::Diagnostic>,
    lsp_revision: Option<u64>,
    indentation: graphite_text::Indentation,
}

fn next_document_id() -> u64 {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

struct App {
    gpu: Option<Gpu>,
    tree: FileTree,
    tabs: Vec<Tab>,
    active_tab: usize,
    editor_groups: app::editor_groups::EditorGroups,
    mouse: PhysicalPosition<f64>,
    modifiers: ModifiersState,
    keymap: keybindings::Keymap,
    selecting: bool,
    reveal_cursor: bool,
    ime_composing: bool,
    ime_preedit: String,
    status: String,
    close_confirm: bool,
    close_window_confirm: bool,
    exit_requested: bool,
    search_query: Option<String>,
    search_results: Vec<SearchResult>,
    search_index: usize,
    search_generation: u64,
    search_cancel: Arc<std::sync::atomic::AtomicU64>,
    local_search: Option<LocalSearch>,
    quick_open: Option<QuickOpen>,
    smart_selection_history: Vec<graphite_text::Selection>,
    smart_selection_guard: Option<(usize, u64, graphite_text::Selection)>,
    snippet_query: Option<String>,
    snippets: Vec<config::Snippet>,
    font_size: f32,
    tab_size: usize,
    insert_spaces: bool,
    detect_indentation: bool,
    trim_trailing_whitespace: bool,
    rulers: Vec<usize>,
    word_wrap: bool,
    wrapping_indent: config::WrapIndent,
    read_only_patterns: Vec<String>,
    event_proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    lsp_manager: LspManager,
    lsp_sender: std::sync::mpsc::Sender<LspUiEvent>,
    lsp_queue: Arc<std::sync::Mutex<Vec<LspUiEvent>>>,
    completion_items: Vec<lsp_types::CompletionItem>,
    completion_index: usize,
    hover_text: Option<String>,
    lsp_request_id: u64,
    terminal: Option<TerminalSession>,
    terminal_visible: bool,
    terminal_focused: bool,
    terminal_problems: bool,
    terminal_status: Option<String>,
    terminal_size: (u16, u16),
}

impl App {
    fn new(
        root: PathBuf,
        event_proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    ) -> Self {
        Self::new_with_options(root, event_proxy, false)
    }

    fn new_with_options(
        root: PathBuf,
        event_proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
        isolated: bool,
    ) -> Self {
        let workspace = Workspace::open(&root).expect("failed to open folder");
        let (lsp_sender, lsp_receiver) = std::sync::mpsc::channel();
        let lsp_queue = Arc::new(std::sync::Mutex::new(Vec::new()));
        if let Some(proxy) = event_proxy.clone() {
            let queue = lsp_queue.clone();
            std::thread::Builder::new()
                .name("graphite-lsp-events".into())
                .spawn(move || {
                    while let Ok(event) = lsp_receiver.recv() {
                        if let Ok(mut pending) = queue.lock() {
                            pending.push(event);
                        }
                        let _ = proxy.send_event(UserEvent::LspWake);
                    }
                })
                .expect("failed to create LSP event thread");
        }
        let lsp_manager = if isolated {
            LspManager::default()
        } else {
            match LspManager::load_config(&config::lsp_config_path()) {
                Ok(config) => LspManager::from_config(config),
                Err(error) if config::lsp_config_path().exists() => {
                    let _ =
                        lsp_sender.send(LspUiEvent::Status(format!("LSP config error: {error}")));
                    LspManager::default()
                }
                Err(_) => LspManager::default(),
            }
        };
        let mut settings_error = None;
        if !isolated {
            DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
            match config::imported_theme_colors(&root) {
                Ok(colors) => Theme::set_imported_colors(colors),
                Err(error) => settings_error = Some(error),
            }
        }
        let prefs = if isolated {
            config::EditorPrefs {
                font_size: 15.0,
                tab_size: 4,
                insert_spaces: false,
                read_only_patterns: Vec::new(),
                detect_indentation: true,
                trim_trailing_whitespace: false,
                rulers: Vec::new(),
                word_wrap: false,
                wrapping_indent: config::WrapIndent::Same,
            }
        } else {
            config::editor_prefs_for_workspace(&root).unwrap_or_else(|error| {
                settings_error = Some(error);
                config::EditorPrefs {
                    font_size: 15.0,
                    tab_size: 4,
                    insert_spaces: false,
                    read_only_patterns: Vec::new(),
                    detect_indentation: true,
                    trim_trailing_whitespace: false,
                    rulers: Vec::new(),
                    word_wrap: false,
                    wrapping_indent: config::WrapIndent::Same,
                }
            })
        };
        let mut keymap = keybindings::Keymap::default();
        if !isolated {
            let path = config::app_config_dir().join("keybindings.json");
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    if let Err(error) = keymap.reload(&source) {
                        settings_error = Some(format!("Keybindings: {error}"));
                    }
                }
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    settings_error = Some(format!("Keybindings: {error}"));
                }
                Err(_) => {}
            }
        }
        Self {
            gpu: None,
            tree: FileTree::new(workspace),
            tabs: vec![Tab {
                id: next_document_id(),
                buffer: Buffer::new(),
                scroll_line: 0,
                language: None,
                highlights: None,
                highlighter: IncrementalHighlighter::new(),
                diagnostics: Vec::new(),
                lsp_revision: None,
                indentation: graphite_text::Indentation {
                    insert_spaces: prefs.insert_spaces,
                    tab_size: prefs.tab_size,
                },
            }],
            active_tab: 0,
            editor_groups: app::editor_groups::EditorGroups::new(0),
            mouse: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            keymap,
            selecting: false,
            reveal_cursor: true,
            ime_composing: false,
            ime_preedit: String::new(),
            status: settings_error.unwrap_or_else(|| String::from("Ctrl+S to save")),
            close_confirm: false,
            close_window_confirm: false,
            exit_requested: false,
            search_query: None,
            search_results: Vec::new(),
            search_index: 0,
            search_generation: 0,
            search_cancel: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            local_search: None,
            quick_open: None,
            smart_selection_history: Vec::new(),
            smart_selection_guard: None,
            snippet_query: None,
            snippets: if isolated {
                Vec::new()
            } else {
                config::load_snippets()
            },
            font_size: prefs.font_size,
            tab_size: prefs.tab_size,
            insert_spaces: prefs.insert_spaces,
            detect_indentation: prefs.detect_indentation,
            trim_trailing_whitespace: prefs.trim_trailing_whitespace,
            rulers: prefs.rulers,
            word_wrap: prefs.word_wrap,
            wrapping_indent: prefs.wrapping_indent,
            read_only_patterns: prefs.read_only_patterns,
            event_proxy,
            lsp_manager,
            lsp_sender,
            lsp_queue,
            completion_items: Vec::new(),
            completion_index: 0,
            hover_text: None,
            lsp_request_id: 0,
            terminal: None,
            terminal_visible: false,
            terminal_focused: false,
            terminal_problems: false,
            terminal_status: None,
            terminal_size: (0, 0),
        }
    }
}

fn same_file_path(a: &Path, b: &Path) -> bool {
    let normalized_a = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let normalized_b = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    #[cfg(windows)]
    {
        let normalize = |path: &Path| {
            let value = path.to_string_lossy().replace('/', "\\");
            value
                .strip_prefix(r"\\?\")
                .unwrap_or(&value)
                .to_ascii_lowercase()
        };
        normalize(&normalized_a) == normalize(&normalized_b)
    }
    #[cfg(not(windows))]
    {
        normalized_a == normalized_b
    }
}

fn main() {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("no current directory"));
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    event_loop
        .run_app(&mut App::new(root, Some(proxy)))
        .expect("event loop failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn opening_same_path_reuses_tab_and_documents_keep_separate_buffers() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        let first = root.join("first.rs");
        let second = root.join("second.rs");
        fs::write(&first, "one").unwrap();
        fs::write(&second, "two").unwrap();
        let mut app = App::new_with_options(root.to_path_buf(), None, true);
        app.open_file(first.clone());
        app.tabs[app.active_tab].buffer.set_cursor_line_col(0, 3);
        app.tabs[app.active_tab].buffer.insert("!");
        app.open_file(second);
        app.open_file(first);
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "one!");
        assert_eq!(app.tabs[0].buffer.text(), "one!");
        assert_eq!(app.tabs[1].buffer.text(), "two");
    }

    #[test]
    fn tab_switch_save_and_close_preserve_other_document() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("one.rs");
        let second = dir.path().join("two.rs");
        fs::write(&first, "fn first() {}\n").unwrap();
        fs::write(&second, "fn second() {}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first.clone());
        app.tabs[0].buffer.set_cursor_line_col(0, 2);
        app.tabs[0].buffer.insert("🌎");
        app.open_file(second);
        app.close_active_tab();
        assert_eq!(app.tabs.len(), 1);
        assert!(app.tabs[0].buffer.is_dirty());
        app.save();
        assert_eq!(fs::read_to_string(first).unwrap(), "fn🌎 first() {}\n");
        assert!(!app.tabs[0].buffer.is_dirty());
    }

    #[test]
    fn open_failure_does_not_discard_unsaved_document() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("unsaved 🌎");
        app.open_file(dir.path().join("missing.rs"));
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].buffer.text(), "unsaved 🌎");
        assert!(app.tabs[0].buffer.is_dirty());
        assert!(app.status.starts_with("Cannot open"));
    }

    #[test]
    fn configured_readonly_pattern_blocks_document_edits() {
        let dir = crate::test_support::TempDir::new();
        fs::create_dir(dir.path().join("generated")).unwrap();
        let file = dir.path().join("generated/data.rs");
        fs::write(&file, "original").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.read_only_patterns = vec!["generated/**".into()];
        app.open_file(file);
        assert!(app.tabs[0].buffer.is_read_only());
        app.tabs[0].buffer.insert("no");
        assert_eq!(app.tabs[0].buffer.text(), "original");
    }

    #[test]
    fn detect_indentation_and_trim_on_save_are_undoable() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("file.rs");
        fs::write(&file, "fn main() { \n    let x = 1;  \n}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        assert_eq!(
            app.tabs[0].indentation,
            graphite_text::Indentation {
                insert_spaces: true,
                tab_size: 4
            }
        );
        app.trim_trailing_whitespace = true;
        app.save();
        assert_eq!(
            fs::read_to_string(file).unwrap(),
            "fn main() {\n    let x = 1;\n}\n"
        );
        app.tabs[0].buffer.undo();
        assert_eq!(
            app.tabs[0].buffer.text(),
            "fn main() { \n    let x = 1;  \n}\n"
        );
        assert!(app.tabs[0].buffer.is_dirty());
    }

    #[test]
    fn closing_window_saves_dirty_files_and_refuses_untitled_document() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("first.rs");
        fs::write(&file, "a").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        app.tabs[0].buffer.insert("b");
        app.request_window_close();
        assert!(app.close_window_confirm);
        assert!(!app.exit_requested);
        app.save_all_before_close();
        assert!(app.exit_requested);
        assert_eq!(fs::read_to_string(file).unwrap(), "ba");
        let mut untitled = App::new_with_options(temp.path().to_path_buf(), None, true);
        untitled.tabs[0].buffer.insert("unsaved");
        untitled.request_window_close();
        untitled.save_all_before_close();
        assert!(!untitled.exit_requested);
        assert!(untitled.tabs[0].buffer.is_dirty());
        assert!(untitled.status.contains("Cannot close"));
    }

    #[test]
    fn highlighting_tracks_edits_undo_and_skipped_revisions() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "fn main() { let x = 1; }\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.refresh_highlights();
        app.tabs[0].buffer.set_cursor_line_col(0, 0);
        app.tabs[0].buffer.insert("// é\n");
        app.refresh_highlights();
        app.tabs[0].buffer.undo();
        app.tabs[0].buffer.insert("/* note */\n");
        app.refresh_highlights();
        let actual = app.tabs[0].highlights.as_ref().unwrap();
        assert_eq!(actual.revision, app.tabs[0].buffer.revision());
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(
            actual.lines,
            fresh.highlight_rope(Language::Rust, app.tabs[0].buffer.rope(), None)
        );
    }

    #[test]
    fn multiple_cursor_edits_keep_incremental_highlighting_consistent() {
        let temp = crate::test_support::TempDir::new();
        let path = temp.path().join("multi.rs");
        fs::write(&path, "let café = 1;\nlet café = 2;\n").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(path);
        app.refresh_highlights();
        app.tabs[0].buffer.set_cursor_line_col(0, 6);
        let previous_state = app.editor_state();
        app.tabs[0].buffer.select_occurrences(true);
        assert_ne!(previous_state, app.editor_state());
        app.tabs[0].buffer.insert("🌎");
        app.refresh_highlights();
        let tab = &app.tabs[0];
        assert_eq!(tab.buffer.text(), "let 🌎 = 1;\nlet 🌎 = 2;\n");
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(
            tab.highlights.as_ref().unwrap().lines,
            fresh.highlight_rope(Language::Rust, tab.buffer.rope(), None)
        );
        app.tabs[0].buffer.undo();
        assert_eq!(app.tabs[0].buffer.text(), "let café = 1;\nlet café = 2;\n");
    }

    #[test]
    fn multi_cursor_edit_rehighlights_and_invalidates_editor_state() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "let café = 1;\nlet café = 2;\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.refresh_highlights();
        app.tabs[0].buffer.set_cursor_line_col(0, 6);
        let before = app.editor_state();
        app.tabs[0].buffer.select_occurrences(true);
        assert_ne!(app.editor_state(), before);
        app.tabs[0].buffer.insert("🌎");
        app.refresh_highlights();
        let tab = &app.tabs[0];
        assert_eq!(tab.buffer.text(), "let 🌎 = 1;\nlet 🌎 = 2;\n");
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(
            tab.highlights.as_ref().unwrap().lines,
            fresh.highlight_rope(Language::Rust, tab.buffer.rope(), None)
        );
        app.tabs[0].buffer.undo();
        assert_eq!(app.tabs[0].buffer.text(), "let café = 1;\nlet café = 2;\n");
    }

    #[test]
    fn local_search_selects_matches_and_replacement_is_undoable() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("Olá olá");
        app.tabs[0].buffer.set_cursor_line_col(0, 0);
        app.open_local_search(true);
        let mut search = app.local_search.take().unwrap();
        search.query = "olá".into();
        search.replacement = "adeus".into();
        app.local_search_next(&mut search, false);
        assert_eq!(app.tabs[0].buffer.selected_text().as_deref(), Some("Olá"));
        app.local_replace_all(&mut search);
        assert_eq!(app.tabs[0].buffer.text(), "adeus adeus");
        app.tabs[0].buffer.undo();
        assert_eq!(app.tabs[0].buffer.text(), "Olá olá");
    }

    #[test]
    fn syntax_selection_expands_and_restores_cursor() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "fn main() { let value = 1; }\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.tabs[0].buffer.set_cursor_line_col(0, 18);
        let original = app.tabs[0].buffer.cursor().char_idx;
        app.smart_select(true);
        assert_eq!(app.tabs[0].buffer.selected_text().as_deref(), Some("value"));
        app.smart_select(true);
        assert!(app.tabs[0].buffer.selected_text().unwrap().len() > 5);
        app.smart_select(false);
        assert_eq!(app.tabs[0].buffer.selected_text().as_deref(), Some("value"));
        app.smart_select(false);
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, original);
        assert_eq!(app.tabs[0].buffer.selection(), None);
    }

    #[test]
    fn syntax_selection_history_is_invalidated_by_external_cursor_or_text_changes() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "fn main() { let value = 1; }\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.tabs[0].buffer.set_cursor_line_col(0, 18);
        app.smart_select(true);
        app.tabs[0].buffer.set_cursor_line_col(0, 0);
        app.smart_select(false);
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 0);
        app.tabs[0].buffer.set_cursor_line_col(0, 18);
        app.smart_select(true);
        app.tabs[0].buffer.insert("other");
        let cursor = app.tabs[0].buffer.cursor().char_idx;
        app.smart_select(false);
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, cursor);
    }

    #[test]
    fn project_search_ignores_results_from_an_outdated_request() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.search_query = Some("new".into());
        app.search_generation = 2;
        let hit = SearchResult {
            path: dir.path().join("x.rs"),
            line: 0,
            start: 0,
            end: 3,
            text: "new".into(),
        };
        assert!(!app.accept_project_search("old", 1, vec![hit.clone()]));
        assert!(app.search_results.is_empty());
        assert!(app.accept_project_search("new", 2, vec![hit]));
        assert_eq!(app.search_results.len(), 1);
        app.search_query = None;
        assert!(!app.accept_project_search("new", 2, Vec::new()));
        assert_eq!(app.search_results.len(), 1);
    }

    #[test]
    fn quick_open_discards_stale_results_and_respects_current_query() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.start_quick_open();
        app.quick_open.as_mut().unwrap().query = "main".into();
        app.search_generation = 4;
        let path = dir.path().join("main.rs");
        assert!(!app.accept_quick_open("old", 3, vec![path.clone()]));
        assert!(app.quick_open.as_ref().unwrap().paths.is_empty());
        assert!(app.accept_quick_open("main", 4, vec![path]));
        assert_eq!(app.quick_open.as_ref().unwrap().paths.len(), 1);
        app.quick_open = None;
        assert!(!app.accept_quick_open("main", 4, Vec::new()));
    }

    #[test]
    fn ime_preedit_and_commit_target_the_focused_input() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.ime_preedit("に".into());
        assert!(app.ime_composing);
        assert!(app.tabs[0].buffer.text().is_empty());
        app.ime_commit("日本");
        assert!(!app.ime_composing);
        assert_eq!(app.tabs[0].buffer.text(), "日本");
        app.open_local_search(false);
        app.ime_preedit("世".into());
        app.ime_commit("世界");
        assert_eq!(app.local_search.as_ref().unwrap().query, "世界");
        assert_eq!(app.tabs[0].buffer.text(), "日本");
        app.local_search = None;
        app.tabs[0].buffer.set_read_only(true);
        app.ime_commit("変更");
        assert_eq!(app.tabs[0].buffer.text(), "日本");
    }

    #[test]
    fn ime_cleared_preedit_commit_and_cancel_do_not_duplicate_or_modify_readonly_text() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.ime_preedit("に".into());
        app.ime_preedit(String::new());
        app.ime_commit("日本");
        assert_eq!(app.tabs[0].buffer.text(), "日本");
        app.ime_preedit("あ".into());
        app.ime_preedit(String::new());
        assert_eq!(app.tabs[0].buffer.text(), "日本");
        app.tabs[0].buffer.set_read_only(true);
        app.ime_preedit("字".into());
        app.ime_preedit(String::new());
        app.ime_commit("字");
        assert_eq!(app.tabs[0].buffer.text(), "日本");
        app.start_quick_open();
        app.ime_preedit("東".into());
        app.ime_preedit(String::new());
        app.ime_commit("東京");
        assert_eq!(app.quick_open.as_ref().unwrap().query, "東京");
    }

    #[test]
    fn accessibility_text_positions_map_to_buffer_offsets() {
        let mut b = Buffer::new();
        b.insert("one\ntwo");
        assert_eq!(accessibility_position_to_char(&b, 1, 2), 6);
    }
}
