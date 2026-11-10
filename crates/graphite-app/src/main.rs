//! Graphite desktop application (v0.1): window, file tree and text editor.

mod app;
use app::accessibility::*;
use app::drawing_helpers::*;
use app::local_search::LocalSearch;
use app::quick_open::QuickOpen;
use app::theme::{Theme, DARK_THEME};

mod branding;
mod cli;
mod config;
mod document;
mod document_store;
use document::Document;
mod debugger;
mod file_format;
mod file_icons;
mod json_schema;
mod keybindings;
mod lsp;
mod ports;
mod previews;
mod process_explorer;
mod scm;
mod terminal;
mod test_explorer;
#[cfg(test)]
mod test_support;
mod tree;
mod webview;

use accesskit::{
    Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node,
    NodeId, Role, Tree, TreeUpdate,
};
use graphite_render::{rgb, Color, Renderer};
use graphite_syntax::{Class, IncrementalHighlighter, Language, Span, TextEdit};
use graphite_text::Buffer;
use graphite_workspace::SearchResult;
use graphite_workspace::Workspace;
use lsp::{LspManager, LspUiEvent};
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

const ACTIVITY_BAR_WIDTH: f32 = 48.0;
const SIDEBAR_WIDTH: f32 = ACTIVITY_BAR_WIDTH + 260.0;
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

use graphite_core_api::DocumentId;

fn next_document_id() -> DocumentId {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    DocumentId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

struct App {
    gpu: Option<Gpu>,
    tree: FileTree,
    documents: document_store::DocumentStore,

    editor_groups: app::editor_groups::EditorGroups,
    mouse: PhysicalPosition<f64>,
    modifiers: ModifiersState,
    keymap: keybindings::Keymap,
    context_keys: app::context_keys::ContextKeyService,
    command_runtime: app::command_execution::CommandRuntime,
    extension_host: app::extension_host::ExtensionRuntime,
    plugins: app::plugins::Plugins,
    emmet_runtime: Option<app::emmet::EmmetRuntime>,
    workspace_trust: Option<app::phase5::WorkspaceTrust>,
    profiles: Option<app::phase5::Profiles>,
    profile_root: PathBuf,
    /// Isolated constructors inject configuration so tests never read user settings.
    service_configuration_override: Option<serde_json::Value>,
    translator: graphite_i18n::Translator,
    saving: app::saving::SaveRuntime,
    document_events: app::services::DocumentEventHub,
    selecting: bool,
    reveal_cursor: bool,
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
    command_palette: Option<app::command_palette::CommandPalette>,
    snippet_query: Option<String>,
    snippets: Vec<config::Snippet>,
    snippet_load_warnings: Vec<String>,
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
    json_schema_manager: Option<json_schema::JsonSchemaManager>,
    lsp_sender: std::sync::mpsc::Sender<LspUiEvent>,
    lsp_queue: Arc<std::sync::Mutex<Vec<LspUiEvent>>>,
    completion_items: Vec<lsp_types::CompletionItem>,
    completion_index: usize,
    hover_text: Option<String>,
    lsp_request_id: u64,
    inlay_request_id: u64,
    quick_fix_actions: Vec<lsp_types::CodeActionOrCommand>,
    quick_fix_index: usize,
    quick_fix_visible: bool,
    navigation_history: Vec<(DocumentId, usize)>,
    navigation_history_index: Option<usize>,
    recent_documents: Vec<DocumentId>,
    diagnostic_navigation: Option<(DocumentId, lsp_types::Range, String)>,
    closed_tabs: Vec<app::editor_groups::ClosedTab>,
    terminals: Vec<TerminalSession>,
    terminal_names: Vec<String>,
    terminal_tab_offset: usize,
    terminal_profiles: Vec<config::TerminalProfile>,
    selected_terminal_profile: usize,
    active_terminal: usize,
    split_terminal: Option<usize>,
    focused_terminal: usize,
    terminal_visible: bool,
    terminal_focused: bool,
    terminal_problems: bool,
    terminal_status: Option<String>,
    task_output: Vec<String>,
    task_cancel: Option<Arc<AtomicBool>>,
    task_diagnostics: Vec<app::phase5::TaskDiagnostic>,
    terminal_size: (u16, u16),
    terminal_sizes: Vec<(u16, u16)>,
    _scm_watcher: Option<notify::RecommendedWatcher>,
    scm_snapshot: scm::Snapshot,
    scm_visible: bool,
    scm_selected: usize,
    scm_repositories_collapsed: bool,
    scm_staged_collapsed: bool,
    scm_changes_collapsed: bool,
    scm_scroll: usize,
    branch_picker: bool,
    branch_selected: usize,
    scm_commit_message: String,
    scm_commit_prompt: bool,
    diff_view: Option<app::scm_ui::DiffView>,
    diff_loading: bool,
    conflict_view: Option<app::scm_ui::ConflictView>,
    zen_mode: bool,
    centered_editor: bool,
    outline_visible: bool,
    process_panel_visible: bool,
    process_metrics: Vec<process_explorer::ProcessMetric>,
    process_scroll: usize,
    process_cancel: Arc<AtomicBool>,
    debug: debugger::DebugRuntime,
    debug_panel_visible: bool,
    debug_input: String,
    debug_input_focused: bool,
    debug_scroll: usize,
    test_explorer: test_explorer::TestExplorer,
    tests_visible: bool,
    ports: ports::PortManager,
    ports_visible: bool,
    selected_port_process: Option<u32>,
    markdown_preview: Option<previews::MarkdownPreview>,
    preview_source_scroll: Option<usize>,
    image_preview: Option<previews::ImagePreview>,
    preview_zoom: f32,
    webview: Option<webview::WebviewHost>,
    webview_html: String,
    webview_handle: Option<String>,
    graphite_detail_webview: Option<webview::WebviewHost>,
    graphite_detail_webview_target: Option<(u64, String)>,
    graphite_detail_webview_html: Option<String>,
    graphite_detail_webview_bounds: Option<[f64; 4]>,
    screencast_mode: bool,
    screencast_cancel: Arc<AtomicBool>,
    screencast_shortcut: Option<(String, std::time::Instant)>,
    startup_metrics: Vec<String>,
    cli_waiters: Vec<app::cli_ui::CliWaiter>,
    initial_cli_request: Option<cli::CliRequest>,
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
        let initialization_started = std::time::Instant::now();
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
        let config_root = config::app_config_dir();
        let profiles = if isolated {
            None
        } else {
            app::phase5::Profiles::open(&config_root).ok()
        };
        let profile_root = profiles
            .as_ref()
            .and_then(|profiles| profiles.path(profiles.active()).ok())
            .unwrap_or_else(|| config_root.clone());
        let language_selection =
            (!isolated).then(|| config::language_selection_in(&profile_root, &root));
        let translator = graphite_i18n::Translator::new(
            language_selection
                .as_ref()
                .map(|selection| selection.language)
                .unwrap_or_else(graphite_i18n::Language::fallback),
        );
        let language_warning = language_selection
            .and_then(|selection| selection.warning)
            .map(|warning| warning.localized(&translator));
        let lsp_manager = if isolated {
            LspManager::default()
        } else {
            match LspManager::load_config(&config::lsp_config_path(), &translator) {
                Ok(config) => LspManager::from_config(config),
                Err(error) if config::lsp_config_path().exists() => {
                    let _ = lsp_sender.send(LspUiEvent::Status(
                        translator.format("lsp.config_error", &[("error", &error)]),
                    ));
                    LspManager::default()
                }
                Err(_) => LspManager::default(),
            }
        };
        let mut settings_error = if !isolated && profiles.is_none() {
            Some("Unable to initialize configuration profiles".to_owned())
        } else {
            None
        };
        if !isolated {
            DARK_THEME.store(
                config::load_dark_theme_from(&profile_root),
                Ordering::Relaxed,
            );
            match config::imported_theme_colors_in(&profile_root, &root, &translator) {
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
            config::editor_prefs_for_workspace_in(&profile_root, &root, &translator).unwrap_or_else(
                |error| {
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
                },
            )
        };
        let (snippets, snippet_load_warnings) = if isolated {
            (Vec::new(), Vec::new())
        } else {
            config::load_snippets_for_workspace_in(&profile_root, &root, &translator)
        };
        let (terminal_profiles, selected_terminal_profile, terminal_config_error) = if isolated {
            (Vec::new(), 0, None)
        } else {
            match config::terminal_settings_in(&profile_root, &translator) {
                Ok(settings) => (settings.profiles, settings.default_profile, None),
                Err(error) => (Vec::new(), 0, Some(error)),
            }
        };
        let mut keymap = keybindings::Keymap::default();
        if !isolated {
            let path = profile_root.join("keybindings.json");
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    if let Err(error) = keymap.reload(&source, &translator) {
                        settings_error =
                            Some(translator.format("config.keybindings", &[("error", &error)]));
                    }
                }
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    settings_error =
                        Some(translator.format("config.keybindings", &[("error", &error)]));
                }
                Err(_) => {}
            }
        }
        let scm_watcher = event_proxy.clone().and_then(|proxy| {
            use notify::Watcher;
            let mut watcher =
                notify::recommended_watcher(move |_result: notify::Result<notify::Event>| {
                    let _ = proxy.send_event(UserEvent::ScmRefresh);
                })
                .ok()?;
            watcher
                .watch(&root, notify::RecursiveMode::Recursive)
                .ok()?;
            Some(watcher)
        });
        let initialization_ms = initialization_started.elapsed().as_secs_f64() * 1000.0;
        let initial_document_id = next_document_id();
        Self {
            gpu: None,
            tree: FileTree::new(workspace),
            documents: document_store::DocumentStore::new(
                initial_document_id,
                Document {
                    buffer: Buffer::new(),

                    language: None,
                    highlights: None,
                    sticky_headers_cache: None,
                    highlighter: IncrementalHighlighter::new(),
                    diagnostics: Vec::new(),
                    inlay_hints: Vec::new(),
                    inlay_revision: None,
                    schema_diagnostics: Vec::new(),
                    schema_revision: None,
                    lsp_revision: None,
                    indentation: graphite_text::Indentation {
                        insert_spaces: prefs.insert_spaces,
                        tab_size: prefs.tab_size,
                    },
                    saved_format: (file_format::Encoding::Utf8, file_format::LineEnding::Lf),
                    encoding: file_format::Encoding::Utf8,
                    line_ending: file_format::LineEnding::Lf,
                },
            ),
            editor_groups: app::editor_groups::EditorGroups::new(initial_document_id),
            mouse: PhysicalPosition::new(0.0, 0.0),
            modifiers: ModifiersState::empty(),
            keymap,
            context_keys: app::context_keys::ContextKeyService::default(),
            command_runtime: Default::default(),
            extension_host: Default::default(),
            plugins: Default::default(),
            emmet_runtime: None,
            profiles,
            profile_root,
            workspace_trust: if isolated {
                None
            } else {
                Some(
                    app::phase5::WorkspaceTrust::open(
                        config::app_config_dir().join("workspace-trust.json"),
                    )
                    .unwrap_or_else(|_| {
                        app::phase5::WorkspaceTrust::restricted(
                            config::app_config_dir().join("workspace-trust.json"),
                        )
                    }),
                )
            },
            service_configuration_override: isolated.then(|| serde_json::json!({})),
            translator,
            saving: Default::default(),
            document_events: Default::default(),
            selecting: false,
            reveal_cursor: true,
            status: language_warning
                .or(settings_error)
                .or_else(|| terminal_config_error.clone())
                .or_else(|| snippet_load_warnings.first().cloned())
                .unwrap_or_else(|| translator.text("status.save_hint").to_owned()),
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
            command_palette: None,
            snippet_query: None,
            snippets,
            snippet_load_warnings,
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
            json_schema_manager: (!isolated).then(|| {
                json_schema::JsonSchemaManager::new(
                    lsp_sender.clone(),
                    config::app_config_dir().join("schema-cache"),
                    translator,
                )
            }),
            lsp_sender,
            lsp_queue,
            completion_items: Vec::new(),
            completion_index: 0,
            hover_text: None,
            lsp_request_id: 0,
            inlay_request_id: 0,
            quick_fix_actions: Vec::new(),
            quick_fix_index: 0,
            quick_fix_visible: false,
            navigation_history: Vec::new(),
            navigation_history_index: None,
            recent_documents: Vec::new(),
            diagnostic_navigation: None,
            closed_tabs: Vec::new(),
            terminals: Vec::new(),
            terminal_names: Vec::new(),
            terminal_tab_offset: 0,
            terminal_profiles,
            selected_terminal_profile,
            active_terminal: 0,
            split_terminal: None,
            focused_terminal: 0,
            terminal_visible: false,
            terminal_focused: false,
            terminal_problems: false,
            terminal_status: terminal_config_error,
            task_output: Vec::new(),
            task_cancel: None,
            task_diagnostics: Vec::new(),
            terminal_size: (0, 0),
            terminal_sizes: Vec::new(),
            _scm_watcher: scm_watcher,
            scm_snapshot: scm::Snapshot::default(),
            scm_visible: false,
            scm_selected: 0,
            scm_repositories_collapsed: false,
            scm_staged_collapsed: false,
            scm_changes_collapsed: false,
            scm_scroll: 0,
            branch_picker: false,
            branch_selected: 0,
            scm_commit_message: String::new(),
            scm_commit_prompt: false,
            diff_view: None,
            diff_loading: false,
            conflict_view: None,
            zen_mode: false,
            centered_editor: false,
            outline_visible: false,
            process_panel_visible: false,
            process_metrics: Vec::new(),
            process_scroll: 0,
            process_cancel: Arc::new(AtomicBool::new(true)),
            debug: debugger::DebugRuntime::default(),
            debug_panel_visible: false,
            debug_input: String::new(),
            debug_input_focused: false,
            debug_scroll: 0,
            test_explorer: test_explorer::TestExplorer::default(),
            tests_visible: false,
            ports: ports::PortManager::default(),
            ports_visible: false,
            selected_port_process: None,
            markdown_preview: None,
            preview_source_scroll: None,
            image_preview: None,
            preview_zoom: 1.0,
            webview: None,
            webview_html: String::new(),
            webview_handle: None,
            graphite_detail_webview: None,
            graphite_detail_webview_target: None,
            graphite_detail_webview_html: None,
            graphite_detail_webview_bounds: None,
            screencast_mode: false,
            screencast_cancel: Arc::new(AtomicBool::new(true)),
            screencast_shortcut: None,
            startup_metrics: vec![translator.format(
                "startup.app_init",
                &[("duration", &format!("{initialization_ms:.1}"))],
            )],
            cli_waiters: Vec::new(),
            initial_cli_request: None,
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
    let cwd = std::env::current_dir().expect("no current directory");
    let cli_translator = graphite_i18n::Translator::new(config::language_selection(&cwd).language);
    let mut args = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "plugin") {
        let result = match args.as_slice() {
            [_, action, directory] if action == "create" => {
                graphite_plugins::sdk::create(&cwd.join(directory))
                    .map(|_| cli_translator.text("cli.plugin_created").to_owned())
            }
            [_, action, directory] if action == "pack" => {
                graphite_plugins::package::pack(&cwd.join(directory))
                    .map(|path| path.display().to_string())
            }
            _ => Err(cli_translator.text("cli.plugin_usage").into()),
        };
        match result {
            Ok(message) => println!("{message}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
        return;
    }
    let simulate_host = args.iter().any(|arg| arg == "--simulate-extension-host");
    args.retain(|arg| arg != "--simulate-extension-host");
    let request = match cli::parse_args(args, cwd.clone(), &cli_translator) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let ipc_server_child = std::env::var_os("GRAPHITE_IPC_SERVER_CHILD").is_some();
    if !ipc_server_child && !simulate_host {
        match cli::send_to_existing(&request, &cli_translator) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => eprintln!(
                "{}",
                cli_translator.format("cli.forward_failed", &[("error", &error)])
            ),
        }
        if request.wait {
            let executable = std::env::current_exe().expect("could not locate current executable");
            let mut child = std::process::Command::new(executable)
                .env("GRAPHITE_IPC_SERVER_CHILD", "1")
                .current_dir(&cwd)
                .spawn()
                .expect("could not start the editor window for --wait");
            let mut forwarded = false;
            for _ in 0..100 {
                match cli::send_to_existing(&request, &cli_translator) {
                    Ok(true) => {
                        forwarded = true;
                        break;
                    }
                    _ => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
            if !forwarded {
                let _ = child.kill();
                eprintln!("{}", cli_translator.text("cli.wait_unavailable"));
                std::process::exit(1);
            }
            return;
        }
    }
    let root = match &request.action {
        cli::CliAction::Open { paths, location } => {
            let candidate = location
                .as_ref()
                .map(|(path, _, _)| path)
                .or_else(|| paths.first());
            candidate
                .filter(|path| path.is_dir())
                .cloned()
                .or_else(|| candidate.and_then(|path| path.parent()).map(PathBuf::from))
                .unwrap_or(cwd)
        }
        cli::CliAction::Diff { .. } => cwd,
    };
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let _ = cli::start_ipc_server(proxy.clone());
    let mut app = App::new(root, Some(proxy));
    app.initial_cli_request = Some(request);
    app.initialize_plugins();
    app.extension_host.enabled = simulate_host;
    if simulate_host {
        if let Err(error) = app.restart_extension_host() {
            app.status = error.to_string();
        }
    }
    event_loop.run_app(&mut app).expect("event loop failed");
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
        app.editor_mut().set_cursor_line_col(0, 3);
        app.editor_mut().insert("!");
        app.open_file(second);
        app.open_file(first);
        assert_eq!(app.documents.len(), 2);
        assert_eq!(app.documents[app.active_document()].buffer.text(), "one!");
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "one!");
        assert_eq!(app.documents[app.document_at(1)].buffer.text(), "two");
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
        app.test_editor_mut(0).set_cursor_line_col(0, 2);
        app.test_editor_mut(0).insert("🌎");
        app.open_file(second);
        app.close_active_tab();
        assert_eq!(app.documents.len(), 1);
        assert!(app.documents[app.document_at(0)].is_dirty());
        app.save();
        assert_eq!(fs::read_to_string(first).unwrap(), "fn🌎 first() {}\n");
        assert!(!app.documents[app.document_at(0)].is_dirty());
    }

    #[test]
    fn open_failure_does_not_discard_unsaved_document() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("unsaved 🌎");
        app.open_file(dir.path().join("missing.rs"));
        assert_eq!(app.documents.len(), 1);
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "unsaved 🌎"
        );
        assert!(app.documents[app.document_at(0)].is_dirty());
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
        assert!(app.documents[app.document_at(0)].buffer.is_read_only());
        app.test_editor_mut(0).insert("no");
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "original");
    }

    #[test]
    fn detect_indentation_and_trim_on_save_are_undoable() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("file.rs");
        fs::write(&file, "fn main() { \n    let x = 1;  \n}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        assert_eq!(
            app.documents[app.document_at(0)].indentation,
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
        app.test_editor_mut(0).undo();
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "fn main() { \n    let x = 1;  \n}\n"
        );
        assert!(app.documents[app.document_at(0)].is_dirty());
    }

    #[test]
    fn closing_window_saves_dirty_files_and_refuses_untitled_document() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("first.rs");
        fs::write(&file, "a").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        app.test_editor_mut(0).insert("b");
        app.request_window_close();
        assert!(app.close_window_confirm);
        assert!(!app.exit_requested);
        app.save_all_before_close();
        app.wait_for_saves();
        assert!(app.exit_requested);
        assert_eq!(fs::read_to_string(file).unwrap(), "ba");
        let mut untitled = App::new_with_options(temp.path().to_path_buf(), None, true);
        untitled.test_editor_mut(0).insert("unsaved");
        untitled.request_window_close();
        untitled.save_all_before_close();
        untitled.wait_for_saves();
        assert!(!untitled.exit_requested);
        assert!(untitled.documents[untitled.document_at(0)].is_dirty());
        assert!(untitled.status.contains("Cannot close"));
    }

    #[test]
    fn failed_save_all_restores_focused_document_and_keeps_unsaved_contents() {
        let temp = crate::test_support::TempDir::new();
        let a = temp.path().join("a.txt");
        let b = temp.path().join("b.txt");
        fs::write(&a, "one  \n").unwrap();
        fs::write(&b, "two  \n").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_file(a.clone());
        app.test_editor_mut(0).insert("é");
        app.open_file(b.clone());
        app.test_editor_mut(1).insert("🙂");
        app.trim_trailing_whitespace = true;
        fs::remove_file(&a).unwrap();
        fs::create_dir(&a).unwrap();
        app.save_all_before_close();
        app.wait_for_saves();
        assert!(!app.exit_requested);
        assert_eq!(app.active_document(), app.document_at(1));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "éone  \n");
        assert_eq!(app.documents[app.document_at(1)].buffer.text(), "🙂two\n");
        assert!(app.documents[app.document_at(0)].is_dirty());
        assert!(!app.documents[app.document_at(1)].is_dirty());
        assert!(app.status.contains("Cannot close"));
    }

    #[test]
    fn completion_on_readonly_document_keeps_existing_selection() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("let value = 1;");
        app.test_editor_mut(0).set_selection(4, 9);
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .buffer
            .set_read_only(true);
        app.completion_items.push(lsp_types::CompletionItem {
            label: "other".into(),
            ..Default::default()
        });
        app.accept_completion();
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "let value = 1;"
        );
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("value"));
        assert_eq!(app.status, "Read-only document");
    }

    #[test]
    fn completion_updates_every_cursor_and_remains_one_undo_step() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("café café");
        app.test_editor_mut(0).set_selections(vec![
            graphite_text::Selection { anchor: 4, head: 4 },
            graphite_text::Selection { anchor: 9, head: 9 },
        ]);
        app.completion_items.push(lsp_types::CompletionItem {
            label: "crème".into(),
            ..Default::default()
        });
        app.accept_completion();
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "crème crème"
        );
        app.test_editor_mut(0).undo();
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "café café");
    }

    #[test]
    fn highlighting_tracks_edits_undo_and_skipped_revisions() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "fn main() { let x = 1; }\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.refresh_highlights();
        app.test_editor_mut(0).set_cursor_line_col(0, 0);
        app.test_editor_mut(0).insert("// é\n");
        app.refresh_highlights();
        app.test_editor_mut(0).undo();
        app.test_editor_mut(0).insert("/* note */\n");
        app.refresh_highlights();
        let actual = app.documents[app.document_at(0)]
            .highlights
            .as_ref()
            .unwrap();
        assert_eq!(
            actual.revision,
            app.documents[app.document_at(0)].buffer.revision()
        );
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(
            actual.lines,
            fresh.highlight_rope(
                Language::Rust,
                app.documents[app.document_at(0)].buffer.rope(),
                None
            )
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
        app.test_editor_mut(0).set_cursor_line_col(0, 6);
        let previous_state = app.editor_state();
        app.test_editor_mut(0).select_occurrences(true);
        assert_ne!(previous_state, app.editor_state());
        app.test_editor_mut(0).insert("🌎");
        app.refresh_highlights();
        let tab = &app.documents[app.document_at(0)];
        assert_eq!(tab.buffer.text(), "let 🌎 = 1;\nlet 🌎 = 2;\n");
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(
            tab.highlights.as_ref().unwrap().lines,
            fresh.highlight_rope(Language::Rust, tab.buffer.rope(), None)
        );
        app.test_editor_mut(0).undo();
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "let café = 1;\nlet café = 2;\n"
        );
    }

    #[test]
    fn multi_cursor_edit_rehighlights_and_invalidates_editor_state() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "let café = 1;\nlet café = 2;\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.refresh_highlights();
        app.test_editor_mut(0).set_cursor_line_col(0, 6);
        let before = app.editor_state();
        app.test_editor_mut(0).select_occurrences(true);
        assert_ne!(app.editor_state(), before);
        app.test_editor_mut(0).insert("🌎");
        app.refresh_highlights();
        let tab = &app.documents[app.document_at(0)];
        assert_eq!(tab.buffer.text(), "let 🌎 = 1;\nlet 🌎 = 2;\n");
        let mut fresh = IncrementalHighlighter::new();
        assert_eq!(
            tab.highlights.as_ref().unwrap().lines,
            fresh.highlight_rope(Language::Rust, tab.buffer.rope(), None)
        );
        app.test_editor_mut(0).undo();
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "let café = 1;\nlet café = 2;\n"
        );
    }

    #[test]
    fn local_search_selects_matches_and_replacement_is_undoable() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("Olá olá");
        app.test_editor_mut(0).set_cursor_line_col(0, 0);
        app.open_local_search(true);
        let mut search = app.local_search.take().unwrap();
        search.query = "olá".into();
        search.replacement = "adeus".into();
        app.local_search_next(&mut search, false);
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("Olá"));
        app.local_replace_all(&mut search);
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "adeus adeus"
        );
        app.test_editor_mut(0).undo();
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "Olá olá");
    }

    #[test]
    fn syntax_selection_expands_and_restores_cursor() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "fn main() { let value = 1; }\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.test_editor_mut(0).set_cursor_line_col(0, 18);
        let original = app.test_editor(0).cursor().char_idx;
        app.smart_select(true);
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("value"));
        app.smart_select(true);
        assert!(app.test_editor(0).selected_text().unwrap().len() > 5);
        app.smart_select(false);
        assert_eq!(app.test_editor(0).selected_text().as_deref(), Some("value"));
        app.smart_select(false);
        assert_eq!(app.test_editor(0).cursor().char_idx, original);
        assert_eq!(app.test_editor(0).selection(), None);
    }

    #[test]
    fn syntax_selection_history_is_invalidated_by_external_cursor_or_text_changes() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("main.rs");
        fs::write(&path, "fn main() { let value = 1; }\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(path);
        app.test_editor_mut(0).set_cursor_line_col(0, 18);
        app.smart_select(true);
        app.test_editor_mut(0).set_cursor_line_col(0, 0);
        app.smart_select(false);
        assert_eq!(app.test_editor(0).cursor().char_idx, 0);
        app.test_editor_mut(0).set_cursor_line_col(0, 18);
        app.smart_select(true);
        app.test_editor_mut(0).insert("other");
        let cursor = app.test_editor(0).cursor().char_idx;
        app.smart_select(false);
        assert_eq!(app.test_editor(0).cursor().char_idx, cursor);
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
        assert!(app.editor_groups.active_view_mut().ime_composing);
        assert!(app.documents[app.document_at(0)].buffer.text().is_empty());
        app.ime_commit("日本");
        assert!(!app.editor_groups.active_view_mut().ime_composing);
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "日本");
        app.open_local_search(false);
        app.ime_preedit("世".into());
        app.ime_commit("世界");
        assert_eq!(app.local_search.as_ref().unwrap().query, "世界");
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "日本");
        app.local_search = None;
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .buffer
            .set_read_only(true);
        app.ime_commit("変更");
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "日本");
    }

    #[test]
    fn ime_cleared_preedit_commit_and_cancel_do_not_duplicate_or_modify_readonly_text() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.ime_preedit("に".into());
        app.ime_preedit(String::new());
        app.ime_commit("日本");
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "日本");
        app.ime_preedit("あ".into());
        app.ime_preedit(String::new());
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "日本");
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .buffer
            .set_read_only(true);
        app.ime_preedit("字".into());
        app.ime_preedit(String::new());
        app.ime_commit("字");
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "日本");
        app.start_quick_open();
        app.ime_preedit("東".into());
        app.ime_preedit(String::new());
        app.ime_commit("東京");
        assert_eq!(app.quick_open.as_ref().unwrap().query, "東京");
    }

    #[test]
    fn accessibility_text_positions_map_to_buffer_offsets() {
        let mut b_document = Buffer::new();
        let mut b_state = graphite_text::SelectionState::default();
        let mut b = b_document.edit(&mut b_state, graphite_text::ViewId(1));
        b.insert("one\ntwo");
        assert_eq!(accessibility_position_to_char(&b, 1, 2), 6);
    }

    #[test]
    fn app_open_save_preserves_utf16_encoding_and_crlf_while_editing_unicode() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("utf16.txt");
        fs::write(
            &file,
            file_format::encode(
                "primeira\nsegunda\n",
                file_format::Encoding::Utf16Le,
                file_format::LineEnding::Crlf,
            ),
        )
        .unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        assert_eq!(
            app.documents[app.document_at(0)].encoding,
            file_format::Encoding::Utf16Le
        );
        assert_eq!(
            app.documents[app.document_at(0)].line_ending,
            file_format::LineEnding::Crlf
        );
        assert_eq!(
            app.documents[app.document_at(0)].buffer.text(),
            "primeira\nsegunda\n"
        );
        app.test_editor_mut(0).set_cursor_line_col(0, 8);
        app.test_editor_mut(0).insert(" 🌎");
        app.save();
        let decoded = file_format::decode(
            &fs::read(file).unwrap(),
            &graphite_i18n::Translator::english(),
        )
        .unwrap();
        assert_eq!(decoded.text, "primeira 🌎\nsegunda\n");
        assert_eq!(decoded.encoding, file_format::Encoding::Utf16Le);
        assert_eq!(decoded.line_ending, file_format::LineEnding::Crlf);
    }

    #[test]
    fn cli_folder_switch_waits_for_unsaved_decision() {
        let first = crate::test_support::TempDir::new();
        let second = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(first.path().to_path_buf(), None, true);
        app.apply_cli_request(
            cli::CliRequest {
                action: cli::CliAction::Open {
                    paths: vec![second.path().to_path_buf()],
                    location: None,
                },
                wait: false,
            },
            None,
        );
        assert_eq!(app.tree.root(), second.path().canonicalize().unwrap());
        app.test_editor_mut(0).insert("unsaved");
        let third = crate::test_support::TempDir::new();
        app.apply_cli_request(
            cli::CliRequest {
                action: cli::CliAction::Open {
                    paths: vec![third.path().to_path_buf()],
                    location: None,
                },
                wait: false,
            },
            None,
        );
        assert_eq!(app.tree.root(), second.path().canonicalize().unwrap());
        assert!(app.close_window_confirm);
        assert!(app.workspace_decision_pending());
    }

    #[test]
    fn cli_wait_is_acknowledged_only_after_requested_document_closes() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("wait.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (acknowledge, acknowledged) = std::sync::mpsc::channel();
        app.apply_cli_request(
            cli::CliRequest {
                action: cli::CliAction::Open {
                    paths: vec![file],
                    location: None,
                },
                wait: true,
            },
            Some(acknowledge),
        );
        assert!(acknowledged.try_recv().is_err());
        app.close_active_tab();
        assert!(acknowledged.try_recv().is_ok());
    }

    #[test]
    fn branch_checkout_refuses_unsaved_editor_buffers() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("unsaved");
        app.scm_snapshot.branches.push("other".into());
        app.open_branch_picker();
        app.run_ui_command("graphite.scm.checkout");
        assert!(app.status.contains("unsaved changes"));
    }

    #[test]
    fn branch_checkout_reload_replaces_clean_buffer_and_file_format() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("branch.rs");
        fs::write(&file, "old\nsecond\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file.clone());
        app.test_editor_mut(0).set_cursor_line_col(1, 2);
        let decoded = file_format::DecodedFile {
            text: "new\n".into(),
            encoding: file_format::Encoding::Utf16Be,
            line_ending: file_format::LineEnding::Crlf,
        };
        app.reload_after_branch_checkout(vec![(file, Ok(decoded))]);
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "new\n");
        assert!(!app.documents[app.document_at(0)].is_dirty());
        assert_eq!(
            app.documents[app.document_at(0)].encoding,
            file_format::Encoding::Utf16Be
        );
        assert_eq!(
            app.documents[app.document_at(0)].line_ending,
            file_format::LineEnding::Crlf
        );
        assert_eq!(app.test_editor(0).cursor_line_col(), (1, 0));
    }

    #[test]
    fn zen_and_centered_layout_reserve_a_right_outline_and_center_eighty_columns() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.centered_editor = true;
        app.outline_visible = true;
        let rect = app.group_rectangles(1400.0, 900.0, 20.0)[0];
        assert!(rect.x > 300.0);
        assert!(rect.width <= 680.0);
        app.zen_mode = true;
        let zen_rect = app.group_rectangles(1400.0, 900.0, 20.0)[0];
        assert!(zen_rect.x > 300.0);
        assert!(zen_rect.y == 0.0);
    }

    #[test]
    fn startup_performance_panel_has_app_initialization_measurement() {
        let dir = crate::test_support::TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(app
            .startup_metrics
            .iter()
            .any(|metric| metric.starts_with("App and service initialization:")));
    }
}
