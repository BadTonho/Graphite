//! Graphite desktop application (v0.1): window, file tree and text editor.

mod app;
use app::accessibility::*;
use app::drawing_helpers::*;
use app::theme::{Theme, DARK_THEME};

mod config;
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
    buffer: Buffer,
    scroll_line: usize,
    language: Option<Language>,
    highlights: Option<HighlightCache>,
    highlighter: IncrementalHighlighter,
    diagnostics: Vec<lsp_types::Diagnostic>,
    lsp_revision: Option<u64>,
}

struct App {
    gpu: Option<Gpu>,
    tree: FileTree,
    tabs: Vec<Tab>,
    active_tab: usize,
    mouse: PhysicalPosition<f64>,
    modifiers: ModifiersState,
    selecting: bool,
    ime_composing: bool,
    status: String,
    close_confirm: bool,
    search_query: Option<String>,
    search_results: Vec<SearchResult>,
    search_index: usize,
    snippet_query: Option<String>,
    snippets: Vec<config::Snippet>,
    font_size: f32,
    tab_size: usize,
    insert_spaces: bool,
    event_proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    lsp_manager: LspManager,
    lsp_sender: std::sync::mpsc::Sender<LspUiEvent>,
    lsp_queue: Arc<std::sync::Mutex<Vec<LspUiEvent>>>,
    completion_items: Vec<lsp_types::CompletionItem>,
    completion_index: usize,
    hover_text: Option<String>,
    lsp_request_id: u64,
    lsp_chord: bool,
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
        if !isolated {
            DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
            Theme::set_imported_colors(config::imported_theme_colors());
        }
        let prefs = if isolated {
            config::EditorPrefs {
                font_size: 15.0,
                tab_size: 4,
                insert_spaces: false,
            }
        } else {
            config::editor_prefs()
        };
        Self {
            gpu: None,
            tree: FileTree::new(workspace),
            tabs: vec![Tab {
                buffer: Buffer::new(),
                scroll_line: 0,
                language: None,
                highlights: None,
                highlighter: IncrementalHighlighter::new(),
                diagnostics: Vec::new(),
                lsp_revision: None,
            }],
            active_tab: 0,
            mouse: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            selecting: false,
            ime_composing: false,
            status: String::from("Ctrl+S to save"),
            close_confirm: false,
            search_query: None,
            search_results: Vec::new(),
            search_index: 0,
            snippet_query: None,
            snippets: if isolated {
                Vec::new()
            } else {
                config::load_snippets()
            },
            font_size: prefs.font_size,
            tab_size: prefs.tab_size,
            insert_spaces: prefs.insert_spaces,
            event_proxy,
            lsp_manager,
            lsp_sender,
            lsp_queue,
            completion_items: Vec::new(),
            completion_index: 0,
            hover_text: None,
            lsp_request_id: 0,
            lsp_chord: false,
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
    fn accessibility_text_positions_map_to_buffer_offsets() {
        let mut b = Buffer::new();
        b.insert("one\ntwo");
        assert_eq!(accessibility_position_to_char(&b, NodeId(1_001), 2), 6);
    }
}
