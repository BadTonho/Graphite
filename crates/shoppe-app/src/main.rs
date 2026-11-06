//! VsCodeShoppe desktop application (v0.1): window, file tree and text editor.

mod config;
mod tree;

use accesskit::{
    Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node,
    NodeId, Role, Tree, TreeUpdate,
};
use shoppe_render::{rgb, Color, Renderer};
use shoppe_syntax::{Class, IncrementalHighlighter, Language, Span, TextEdit};
use shoppe_text::Buffer;
use shoppe_workspace::SearchResult;
use shoppe_workspace::Workspace;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock, RwLock,
};
use std::{path::PathBuf, sync::Arc};
use tree::FileTree;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

const SIDEBAR_WIDTH: f32 = 260.0;
const PAD: f32 = 8.0;

struct Theme;
static DARK_THEME: AtomicBool = AtomicBool::new(true);
static THEME_OVERRIDES: OnceLock<RwLock<HashMap<u32, u32>>> = OnceLock::new();
impl Theme {
    const BG: u32 = 0x1e1e1e;
    const SIDEBAR: u32 = 0x252526;
    const TEXT: u32 = 0xd4d4d4;
    const DIM: u32 = 0x858585;
    const CURSOR: u32 = 0xaeafad;
    const ACTIVE_ROW: u32 = 0x37373d;
    const STATUS: u32 = 0x007acc;

    fn color(dark: u32, light: u32) -> u32 {
        let base = if DARK_THEME.load(Ordering::Relaxed) {
            dark
        } else {
            light
        };
        THEME_OVERRIDES
            .get_or_init(Default::default)
            .read()
            .ok()
            .and_then(|m| m.get(&base).copied())
            .unwrap_or(base)
    }

    fn class_color(class: Class) -> u32 {
        match class {
            Class::Default | Class::Punctuation | Class::Operator => {
                Self::color(Self::TEXT, 0x1e1e1e)
            }
            Class::Keyword => Self::color(0x569cd6, 0x0000ff),
            Class::Function => Self::color(0xdcdcaa, 0x795e26),
            Class::Type => Self::color(0x4ec9b0, 0x267f99),
            Class::String => Self::color(0xce9178, 0xa31515),
            Class::Number | Class::Constant => Self::color(0xb5cea8, 0x098658),
            Class::Comment => Self::color(0x6a9955, 0x008000),
            Class::Property | Class::Variable => Self::color(0x9cdcfe, 0x001080),
        }
    }

    fn set_imported_colors(colors: HashMap<String, String>) {
        let mappings = [
            ("editor.background", Self::BG, 0xffffff),
            ("sideBar.background", Self::SIDEBAR, 0xf3f3f3),
            ("editor.foreground", Self::TEXT, 0x1e1e1e),
            ("editorLineNumber.foreground", Self::DIM, 0x616161),
            ("editorCursor.foreground", Self::CURSOR, 0x202020),
            ("editor.lineHighlightBackground", Self::ACTIVE_ROW, 0xd6ebff),
            ("statusBar.background", Self::STATUS, 0x005a9e),
            ("syntax.keyword", 0x569cd6, 0x0000ff),
            ("syntax.function", 0xdcdcaa, 0x795e26),
            ("syntax.type", 0x4ec9b0, 0x267f99),
            ("syntax.string", 0xce9178, 0xa31515),
            ("syntax.number", 0xb5cea8, 0x098658),
            ("syntax.comment", 0x6a9955, 0x008000),
            ("syntax.variable", 0x9cdcfe, 0x001080),
        ];
        let mut overrides = HashMap::new();
        for (key, dark, light) in mappings {
            if let Some(value) = colors.get(key).and_then(|v| parse_hex_color(v)) {
                overrides.insert(dark, value);
                overrides.insert(light, value);
            }
        }
        if let Ok(mut current) = THEME_OVERRIDES.get_or_init(Default::default).write() {
            *current = overrides;
        }
    }
}
fn parse_hex_color(s: &str) -> Option<u32> {
    let s = s.strip_prefix('#')?;
    if s.len() < 6 {
        return None;
    }
    u32::from_str_radix(&s[..6], 16).ok()
}

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

struct AccessibilityActivation;
impl ActivationHandler for AccessibilityActivation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(accessibility_tree(&[], "VsCodeShoppe editor"))
    }
}
#[derive(Debug)]
enum UserEvent {
    AccessibilityWake,
}

struct AccessibilityActionQueue {
    queue: Arc<std::sync::Mutex<Vec<ActionRequest>>>,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
}
impl ActionHandler for AccessibilityActionQueue {
    fn do_action(&mut self, request: ActionRequest) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.push(request);
        }
        let _ = self.proxy.send_event(UserEvent::AccessibilityWake);
    }
}
struct AccessibilityDeactivation;
impl DeactivationHandler for AccessibilityDeactivation {
    fn deactivate_accessibility(&mut self) {}
}
fn accessibility_tree(titles: &[String], value: &str) -> TreeUpdate {
    let root = NodeId(0);
    let tablist = NodeId(1);
    let editor = NodeId(2);
    let tab_ids: Vec<_> = (0..titles.len()).map(|i| NodeId(10 + i as u64)).collect();
    let mut root_node = Node::new(Role::Window);
    root_node.set_children([tablist, editor]);
    root_node.set_label("VsCodeShoppe");
    let mut tabs_node = Node::new(Role::TabList);
    tabs_node.set_children(tab_ids.clone());
    let mut nodes = vec![(root, root_node), (tablist, tabs_node)];
    for (id, title) in tab_ids.iter().zip(titles) {
        let mut tab = Node::new(Role::Tab);
        tab.set_label(title.as_str());
        nodes.push((*id, tab));
    }
    let mut editor_node = Node::new(Role::TextInput);
    editor_node.set_value(value);
    let lines: Vec<_> = value.split_inclusive('\n').collect();
    let run_ids: Vec<_> = (0..lines.len()).map(|i| NodeId(1_000 + i as u64)).collect();
    editor_node.set_children(run_ids.clone());
    nodes.push((editor, editor_node));
    for (id, line) in run_ids.iter().zip(lines) {
        let text = line.trim_end_matches(['\n', '\r']);
        let mut run = Node::new(Role::TextRun);
        run.set_value(text);
        run.set_character_lengths(vec![1; text.chars().count()]);
        nodes.push((*id, run));
    }
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(root)),
        tree_id: accesskit::TreeId::ROOT,
        focus: editor,
    }
}

struct Tab {
    buffer: Buffer,
    scroll_line: usize,
    language: Option<Language>,
    highlights: Option<HighlightCache>,
    highlighter: IncrementalHighlighter,
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
}

impl App {
    fn new(
        root: PathBuf,
        event_proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    ) -> Self {
        let workspace = Workspace::open(&root).expect("failed to open folder");
        DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
        Theme::set_imported_colors(config::imported_theme_colors());
        let prefs = config::editor_prefs();
        Self {
            gpu: None,
            tree: FileTree::new(workspace),
            tabs: vec![Tab {
                buffer: Buffer::new(),
                scroll_line: 0,
                language: None,
                highlights: None,
                highlighter: IncrementalHighlighter::new(),
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
            snippets: config::load_snippets(),
            font_size: prefs.font_size,
            tab_size: prefs.tab_size,
            insert_spaces: prefs.insert_spaces,
            event_proxy,
        }
    }

    /// Re-highlights the buffer only when its text changed since the last run.
    fn refresh_highlights(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        let Some(lang) = tab.language else {
            tab.highlights = None;
            return;
        };
        let revision = tab.buffer.revision();
        if tab
            .highlights
            .as_ref()
            .is_some_and(|h| h.revision == revision)
        {
            return;
        }
        let edit = if tab
            .highlights
            .as_ref()
            .is_some_and(|h| h.revision + 1 == revision)
        {
            tab.buffer.last_change().map(|c| TextEdit {
                start_byte: c.start_byte,
                old_end_byte: c.old_end_byte,
                new_end_byte: c.new_end_byte,
                start_position: (c.start_row, c.start_column),
                old_end_position: (c.old_end_row, c.old_end_column),
                new_end_position: (c.new_end_row, c.new_end_column),
            })
        } else {
            None
        };
        let lines = tab.highlighter.highlight(lang, &tab.buffer.text(), edit);
        tab.highlights = Some(HighlightCache { revision, lines });
    }

    fn open_file(&mut self, path: PathBuf) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.buffer.path() == Some(path.as_path()))
        {
            self.active_tab = index;
            self.status = path.display().to_string();
            return;
        }
        match Buffer::open(&path) {
            Ok(buffer) => {
                let tab = Tab {
                    buffer,
                    scroll_line: 0,
                    language: Language::from_path(&path),
                    highlights: None,
                    highlighter: IncrementalHighlighter::new(),
                };
                if self.tabs.len() == 1
                    && self.tabs[0].buffer.path().is_none()
                    && !self.tabs[0].buffer.is_dirty()
                {
                    self.tabs[0] = tab;
                    self.active_tab = 0;
                } else {
                    self.tabs.push(tab);
                    self.active_tab = self.tabs.len() - 1;
                }
                self.status = path.display().to_string();
            }
            Err(e) => self.status = format!("Cannot open {}: {e}", path.display()),
        }
    }

    fn save(&mut self) {
        self.status = match self.tabs[self.active_tab].buffer.path() {
            None => "No file to save".into(),
            Some(_) => match self.tabs[self.active_tab].buffer.save() {
                Ok(()) => "Saved".into(),
                Err(e) => format!("Save failed: {e}"),
            },
        };
    }

    fn process_accessibility_actions(&mut self) {
        let actions = self
            .gpu
            .as_ref()
            .and_then(|g| {
                g.accessibility_actions
                    .lock()
                    .ok()
                    .map(|mut q| std::mem::take(&mut *q))
            })
            .unwrap_or_default();
        for request in actions {
            let id = request.target_node.0;
            if id >= 10 && id < 10 + self.tabs.len() as u64 && request.action == Action::Click {
                self.active_tab = (id - 10) as usize;
                continue;
            }
            if id == 2 {
                match (request.action, request.data) {
                    (Action::Focus, _) => self.status = "Editor focused".into(),
                    (Action::SetValue, Some(ActionData::Value(value))) => {
                        let b = &mut self.tabs[self.active_tab].buffer;
                        b.set_selection(0, b.len_chars());
                        b.insert(&value);
                    }
                    (Action::ReplaceSelectedText, Some(ActionData::Value(value))) => {
                        self.tabs[self.active_tab].buffer.insert(&value)
                    }
                    (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
                        let b = &mut self.tabs[self.active_tab].buffer;
                        let anchor = accessibility_position_to_char(
                            b,
                            selection.anchor.node,
                            selection.anchor.character_index,
                        );
                        let focus = accessibility_position_to_char(
                            b,
                            selection.focus.node,
                            selection.focus.character_index,
                        );
                        b.set_selection(anchor, focus);
                    }
                    _ => {}
                }
            }
        }
    }

    fn visible_lines(&self, renderer: &Renderer) -> usize {
        let h = renderer.size().1 - renderer.line_height() * 2.5;
        ((h / renderer.line_height()) as usize).max(1)
    }

    fn keep_cursor_visible(&mut self, visible: usize) {
        let (line, _) = self.tabs[self.active_tab].buffer.cursor_line_col();
        if line < self.tabs[self.active_tab].scroll_line {
            self.tabs[self.active_tab].scroll_line = line;
        } else if line >= self.tabs[self.active_tab].scroll_line + visible {
            self.tabs[self.active_tab].scroll_line = line + 1 - visible;
        }
    }

    fn gutter_width(&self, char_w: f32) -> f32 {
        let digits = self.tabs[self.active_tab]
            .buffer
            .len_lines()
            .to_string()
            .len()
            .max(3);
        digits as f32 * char_w + PAD * 2.0
    }

    fn click(&mut self) {
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);

        if mx < SIDEBAR_WIDTH {
            let first_row_y = lh * 1.5;
            if my >= first_row_y {
                let row = ((my - first_row_y) / lh) as usize;
                if let Some(path) = self.tree.activate(row) {
                    self.open_file(path);
                }
            }
        } else {
            if my < lh * 1.5 {
                let mut x = SIDEBAR_WIDTH;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let label = tab
                        .buffer
                        .path()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Untitled".into());
                    let width = (label.chars().count() as f32 + 4.0) * cw * 1.8;
                    if mx >= x && mx < x + width {
                        self.active_tab = i;
                        return;
                    }
                    x += width;
                }
                return;
            }
            let text_x = SIDEBAR_WIDTH + self.gutter_width(cw);
            let line =
                self.tabs[self.active_tab].scroll_line + ((my - lh * 1.5).max(0.0) / lh) as usize;
            let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
            self.tabs[self.active_tab]
                .buffer
                .set_cursor_line_col(line, col);
            self.selecting = true;
        }
    }

    fn drag_select(&mut self) {
        if !self.selecting {
            return;
        }
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let text_x = SIDEBAR_WIDTH + self.gutter_width(cw);
        let line = (self.tabs[self.active_tab].scroll_line
            + ((self.mouse.y as f32 - lh * 1.5).max(0.0) / lh) as usize)
            .min(self.tabs[self.active_tab].buffer.len_lines() - 1);
        let col = (((self.mouse.x as f32 - text_x) / cw).round().max(0.0)) as usize;
        let head = self.tabs[self.active_tab].buffer.line_start_char(line)
            + col.min(
                self.tabs[self.active_tab]
                    .buffer
                    .line(line)
                    .trim_end_matches(['\n', '\r'])
                    .chars()
                    .count(),
            );
        let anchor = self.tabs[self.active_tab]
            .buffer
            .selection()
            .map(|s| s.anchor)
            .unwrap_or(self.tabs[self.active_tab].buffer.cursor().char_idx);
        self.tabs[self.active_tab]
            .buffer
            .set_selection(anchor, head);
    }

    fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        if ctrl
            && self.modifiers.alt_key()
            && matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case("t"))
        {
            let dark = !DARK_THEME.load(Ordering::Relaxed);
            DARK_THEME.store(dark, Ordering::Relaxed);
            config::save_dark_theme(dark);
            return;
        }
        if self.close_confirm {
            let key = event.text.as_deref().unwrap_or("").to_ascii_lowercase();
            if key == "d" {
                self.close_confirm = false;
                self.close_active_tab();
                return;
            }
            if key == "s" {
                self.save();
                self.close_confirm = false;
                if !self.tabs[self.active_tab].buffer.is_dirty() {
                    self.close_active_tab();
                }
                return;
            }
            if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                self.close_confirm = false;
                self.status = "Close cancelled".into();
            }
            return;
        }
        if self.snippet_query.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => self.snippet_query = None,
                Key::Named(NamedKey::Backspace) => {
                    if let Some(q) = &mut self.snippet_query {
                        q.pop();
                    }
                }
                Key::Named(NamedKey::Enter) => {
                    let q = self.snippet_query.as_deref().unwrap_or("");
                    if let Some(snippet) = self
                        .snippets
                        .iter()
                        .find(|s| s.prefix.starts_with(q) || s.prefix == q)
                    {
                        self.tabs[self.active_tab].buffer.insert(&snippet.body);
                    }
                    self.snippet_query = None;
                }
                _ => {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            if let Some(q) = &mut self.snippet_query {
                                q.push_str(text);
                            }
                        }
                    }
                }
            }
            return;
        }
        let before = self.tabs[self.active_tab].buffer.cursor().char_idx;
        let mut moved = false;
        if self.search_query.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.search_query = None;
                    self.status = "Search closed".into();
                }
                Key::Named(NamedKey::Backspace) => {
                    if let Some(q) = &mut self.search_query {
                        q.pop();
                    }
                }
                Key::Named(NamedKey::ArrowDown) if !self.search_results.is_empty() => {
                    self.search_index = (self.search_index + 1) % self.search_results.len()
                }
                Key::Named(NamedKey::ArrowUp) if !self.search_results.is_empty() => {
                    self.search_index = self
                        .search_index
                        .checked_sub(1)
                        .unwrap_or(self.search_results.len() - 1)
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(hit) = self.search_results.get(self.search_index).cloned() {
                        self.open_file(hit.path);
                        let b = &mut self.tabs[self.active_tab].buffer;
                        let col = hit.text.get(..hit.start).unwrap_or("").chars().count();
                        b.set_cursor_line_col(hit.line, col);
                        self.search_query = None;
                    } else {
                        self.status = "No matches".into();
                    }
                }
                _ => {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            if let Some(q) = &mut self.search_query {
                                q.push_str(text);
                                self.search_results = self.tree.search(q);
                            }
                        }
                    }
                }
            }
            return;
        }
        match &event.logical_key {
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("i") => {
                let report = config::import_vscode();
                DARK_THEME.store(config::load_dark_theme(), Ordering::Relaxed);
                Theme::set_imported_colors(config::imported_theme_colors());
                let prefs = config::editor_prefs();
                self.font_size = prefs.font_size;
                self.tab_size = prefs.tab_size;
                self.insert_spaces = prefs.insert_spaces;
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer
                        .set_font_size(self.font_size * gpu.window.scale_factor() as f32);
                }
                self.snippets = config::load_snippets();
                self.status = if report.warnings.is_empty() {
                    format!("Imported {} VS Code items", report.imported)
                } else {
                    format!(
                        "Imported {} items; {} warning(s): {}",
                        report.imported,
                        report.warnings.len(),
                        report.warnings[0]
                    )
                };
            }
            Key::Character(c) if ctrl && shift && c.eq_ignore_ascii_case("p") => {
                self.snippets = config::load_snippets();
                self.snippet_query = Some(String::new());
                self.status = "Snippet: type prefix, Enter inserts, Esc closes".into();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("f") => {
                self.search_query = Some(String::new());
                self.search_results.clear();
                self.search_index = 0;
                self.status = "Search: type query; Up/Down select, Enter opens result".into();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("s") && self.close_confirm => {
                self.save();
                self.close_confirm = false;
                self.close_active_tab();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("d") && self.close_confirm => {
                self.close_confirm = false;
                self.close_active_tab();
            }
            Key::Named(NamedKey::Tab) if ctrl => {
                self.active_tab = (self.active_tab + 1) % self.tabs.len()
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("s") => self.save(),
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("z") => {
                self.tabs[self.active_tab].buffer.undo()
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("y") => {
                self.tabs[self.active_tab].buffer.redo()
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("a") => {
                let len = self.tabs[self.active_tab].buffer.len_chars();
                self.tabs[self.active_tab].buffer.set_selection(0, len);
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("w") => {
                if self.tabs[self.active_tab].buffer.is_dirty() {
                    self.close_confirm = true;
                    self.status = "Unsaved tab: S save, D discard, Esc cancel".into();
                } else {
                    self.close_active_tab();
                }
            }
            Key::Named(NamedKey::Escape) if self.close_confirm => {
                self.close_confirm = false;
                self.status = "Close cancelled".into();
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("c") => {
                if let Some(text) = self.tabs[self.active_tab].buffer.selected_text() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(text);
                    }
                }
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("x") => {
                if let Some(text) = self.tabs[self.active_tab].buffer.selected_text() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if clipboard.set_text(text).is_ok() {
                            self.tabs[self.active_tab].buffer.insert("");
                        }
                    }
                }
            }
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("v") => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.tabs[self.active_tab].buffer.insert(&text);
                    }
                }
            }
            Key::Named(NamedKey::Backspace) => self.tabs[self.active_tab].buffer.backspace(),
            Key::Named(NamedKey::Delete) => self.tabs[self.active_tab].buffer.delete(),
            Key::Named(NamedKey::Enter) => self.tabs[self.active_tab].buffer.insert("\n"),
            Key::Named(NamedKey::Tab) => {
                let value = if self.insert_spaces {
                    " ".repeat(self.tab_size)
                } else {
                    "\t".into()
                };
                self.tabs[self.active_tab].buffer.insert(&value);
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.tabs[self.active_tab].buffer.move_left();
                moved = true;
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.tabs[self.active_tab].buffer.move_right();
                moved = true;
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.tabs[self.active_tab].buffer.move_vertical(-1);
                moved = true;
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.tabs[self.active_tab].buffer.move_vertical(1);
                moved = true;
            }
            Key::Named(NamedKey::Home) => {
                self.tabs[self.active_tab].buffer.move_home();
                moved = true;
            }
            Key::Named(NamedKey::End) => {
                self.tabs[self.active_tab].buffer.move_end();
                moved = true;
            }
            _ if !ctrl => {
                if !self.ime_composing {
                    if let Some(text) = &event.text {
                        if !text.chars().any(char::is_control) {
                            self.tabs[self.active_tab].buffer.insert(text);
                        }
                    }
                }
            }
            _ => {}
        }
        if moved {
            let after = self.tabs[self.active_tab].buffer.cursor().char_idx;
            if shift {
                let anchor = self.tabs[self.active_tab]
                    .buffer
                    .selection()
                    .map(|s| s.anchor)
                    .unwrap_or(before);
                self.tabs[self.active_tab]
                    .buffer
                    .set_selection(anchor, after);
            }
        }
    }

    fn close_active_tab(&mut self) {
        if self.tabs.len() == 1 {
            self.tabs[0] = Tab {
                buffer: Buffer::new(),
                scroll_line: 0,
                language: None,
                highlights: None,
                highlighter: IncrementalHighlighter::new(),
            };
        } else {
            self.tabs.remove(self.active_tab);
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }
        self.status = "Tab closed".into();
    }

    fn draw(&mut self) {
        let Some(mut gpu) = self.gpu.take() else {
            return;
        };
        let r = &mut gpu.renderer;
        let (w, h) = r.size();
        let (lh, cw) = (r.line_height(), r.char_width());

        let visible = self.visible_lines(r);
        self.keep_cursor_visible(visible);
        self.refresh_highlights();
        let accessible_text = self.tabs[self.active_tab].buffer.text();
        let titles: Vec<_> = self
            .tabs
            .iter()
            .map(|t| {
                t.buffer
                    .path()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into())
            })
            .collect();
        gpu.accessibility
            .update_if_active(|| accessibility_tree(&titles, &accessible_text));

        // Sidebar
        r.draw_rect(
            0.0,
            0.0,
            SIDEBAR_WIDTH,
            h,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        r.draw_text(
            &self.tree.root_name().to_uppercase(),
            PAD,
            lh * 0.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        for (i, row) in self.tree.rows.iter().enumerate() {
            let y = lh * 1.5 + i as f32 * lh;
            if y > h {
                break;
            }
            let is_open = row.entry.path.as_path()
                == self.tabs[self.active_tab]
                    .buffer
                    .path()
                    .unwrap_or(std::path::Path::new(""));
            if is_open {
                r.draw_rect(
                    0.0,
                    y,
                    SIDEBAR_WIDTH,
                    lh,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            let marker = match (row.entry.is_dir, row.expanded) {
                (true, true) => "v ",
                (true, false) => "> ",
                _ => "  ",
            };
            let label = format!("{marker}{}", row.entry.name);
            r.draw_text(
                &label,
                PAD + row.depth as f32 * cw * 2.0,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }

        // Editor
        let gutter = self.gutter_width(cw);
        let text_x = SIDEBAR_WIDTH + gutter;
        let mut tab_x = SIDEBAR_WIDTH;
        for (i, tab) in self.tabs.iter().enumerate() {
            let label = tab
                .buffer
                .path()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into());
            let label = format!(
                " {}{} ",
                label,
                if tab.buffer.is_dirty() { " ●" } else { "" }
            );
            let color = if i == self.active_tab {
                Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
            } else {
                Theme::color(Theme::SIDEBAR, 0xf3f3f3)
            };
            r.draw_rect(
                tab_x,
                0.0,
                label.chars().count() as f32 * cw * 1.8,
                lh * 1.5,
                rgb(color),
            );
            r.draw_text(
                &label,
                tab_x + PAD,
                lh * 0.25,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            tab_x += label.chars().count() as f32 * cw * 1.8;
        }
        let (cur_line, cur_col) = self.tabs[self.active_tab].buffer.cursor_line_col();
        let last = (self.tabs[self.active_tab].scroll_line + visible)
            .min(self.tabs[self.active_tab].buffer.len_lines());
        for line in self.tabs[self.active_tab].scroll_line..last {
            let y = lh * 1.5 + (line - self.tabs[self.active_tab].scroll_line) as f32 * lh;
            if let Some(selection) = self.tabs[self.active_tab].buffer.selection() {
                let line_start = self.tabs[self.active_tab].buffer.line_start_char(line);
                let line_end = line_start
                    + self.tabs[self.active_tab]
                        .buffer
                        .line(line)
                        .trim_end_matches(['\n', '\r'])
                        .chars()
                        .count();
                let sel_start = selection.anchor.min(selection.head);
                let sel_end = selection.anchor.max(selection.head);
                let a = sel_start.max(line_start);
                let b = sel_end.min(line_end);
                if a < b {
                    r.draw_rect(
                        text_x + (a - line_start) as f32 * cw,
                        y,
                        (b - a) as f32 * cw,
                        lh,
                        rgb(0x264f78),
                    );
                }
            }
            let color = if line == cur_line {
                Theme::color(Theme::TEXT, 0x1e1e1e)
            } else {
                Theme::color(Theme::DIM, 0x616161)
            };
            let num = format!(
                "{:>width$}",
                line + 1,
                width = ((gutter - PAD * 2.0) / cw) as usize
            );
            r.draw_text(&num, SIDEBAR_WIDTH + PAD, y, rgb(color));
            draw_code_line(
                r,
                &self.tabs[self.active_tab].buffer.line(line),
                spans_for(&self.tabs[self.active_tab].highlights, line),
                text_x,
                y,
                cw,
                self.tab_size,
            );
        }
        if (self.tabs[self.active_tab].scroll_line..last).contains(&cur_line) {
            let prefix: String = self.tabs[self.active_tab]
                .buffer
                .line(cur_line)
                .chars()
                .take(cur_col)
                .collect();
            let col = display_line(&prefix, self.tab_size).chars().count();
            let y = lh * 1.5 + (cur_line - self.tabs[self.active_tab].scroll_line) as f32 * lh;
            r.draw_rect(
                text_x + col as f32 * cw,
                y,
                2.0,
                lh,
                rgb(Theme::color(Theme::CURSOR, 0x202020)),
            );
        }

        // Status bar
        let dirty = if self.tabs[self.active_tab].buffer.is_dirty() {
            " ●"
        } else {
            ""
        };
        let status = format!(
            " {}{}   Ln {}, Col {}",
            self.status,
            dirty,
            cur_line + 1,
            cur_col + 1
        );
        r.draw_rect(
            0.0,
            h - lh,
            w,
            lh,
            rgb(Theme::color(Theme::STATUS, 0x005a9e)),
        );
        r.draw_text(&status, 0.0, h - lh, rgb(0xffffff));
        if let Some(query) = &self.search_query {
            let label = format!(
                "Search: {query}  ({} matches; Up/Down, Enter, Esc)",
                self.search_results.len()
            );
            let height = lh * (self.search_results.len().min(6) as f32 + 1.2);
            r.draw_rect(
                SIDEBAR_WIDTH,
                h - lh - height,
                w - SIDEBAR_WIDTH,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &label,
                SIDEBAR_WIDTH + PAD,
                h - lh - height + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let start = self
                .search_index
                .saturating_sub(5)
                .min(self.search_results.len().saturating_sub(6));
            for (visible_i, (i, hit)) in self
                .search_results
                .iter()
                .enumerate()
                .skip(start)
                .take(6)
                .enumerate()
            {
                let y = h - lh - height + lh * (visible_i as f32 + 1.0);
                if i == self.search_index {
                    r.draw_rect(
                        SIDEBAR_WIDTH,
                        y,
                        w - SIDEBAR_WIDTH,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let rel = hit
                    .path
                    .strip_prefix(self.tree.root())
                    .unwrap_or(&hit.path)
                    .display();
                let row = format!("{}:{}  {}", rel, hit.line + 1, hit.text);
                r.draw_text(
                    &row,
                    SIDEBAR_WIDTH + PAD,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(query) = &self.snippet_query {
            let matches: Vec<_> = self
                .snippets
                .iter()
                .filter(|s| s.prefix.starts_with(query))
                .take(6)
                .collect();
            let y = h - lh * (matches.len() as f32 + 1.0);
            r.draw_rect(
                SIDEBAR_WIDTH,
                y,
                w - SIDEBAR_WIDTH,
                lh * (matches.len() as f32 + 1.0),
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &format!("Snippet prefix: {query}"),
                SIDEBAR_WIDTH + PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            for (i, s) in matches.iter().enumerate() {
                r.draw_text(
                    &s.prefix,
                    SIDEBAR_WIDTH + PAD,
                    y + lh * (i as f32 + 1.0),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }

        r.set_clear_color(rgb(Theme::color(Theme::BG, 0xffffff)));
        r.present();
        self.gpu = Some(gpu);
    }
}

fn accessibility_position_to_char(buffer: &Buffer, node: NodeId, index: usize) -> usize {
    let row = node.0.saturating_sub(1_000) as usize;
    let row = row.min(buffer.len_lines().saturating_sub(1));
    let text = buffer.line(row);
    buffer.line_start_char(row) + index.min(text.trim_end_matches(['\n', '\r']).chars().count())
}

fn spans_for(cache: &Option<HighlightCache>, line: usize) -> &[Span] {
    cache
        .as_ref()
        .and_then(|c| c.lines.get(line))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Draws one line, coloring runs of characters by their highlight class.
fn draw_code_line(
    r: &mut Renderer,
    line: &str,
    spans: &[Span],
    x: f32,
    y: f32,
    cw: f32,
    tab_width: usize,
) {
    let line = line.trim_end_matches(['\n', '\r']);
    let color_of = |class: Class| -> Color { rgb(Theme::class_color(class)) };

    let mut idx = 0;
    let mut col = 0usize;
    let mut run = String::new();
    let mut run_start_col = 0usize;
    let mut run_class = Class::Default;

    for (byte, ch) in line.char_indices() {
        while idx < spans.len() && spans[idx].range.end <= byte {
            idx += 1;
        }
        let class = match spans.get(idx) {
            Some(s) if s.range.start <= byte => s.class,
            _ => Class::Default,
        };
        if class != run_class && !run.is_empty() {
            r.draw_text(&run, x + run_start_col as f32 * cw, y, color_of(run_class));
            run.clear();
        }
        if run.is_empty() {
            run_start_col = col;
        }
        run_class = class;
        if ch == '\t' {
            run.push_str(&" ".repeat(tab_width));
            col += tab_width;
        } else {
            run.push(ch);
            col += 1;
        }
    }
    if !run.is_empty() {
        r.draw_text(&run, x + run_start_col as f32 * cw, y, color_of(run_class));
    }
}

/// Strips the line terminator and expands tabs for display.
fn display_line(line: &str, tab_width: usize) -> String {
    let mut out = String::with_capacity(line.len());
    for ch in line.trim_end_matches(['\n', '\r']).chars() {
        if ch == '\t' {
            out.push_str(&" ".repeat(tab_width));
        } else {
            out.push(ch);
        }
    }
    out
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("VsCodeShoppe")
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(1200.0, 760.0)),
                )
                .expect("failed to create window"),
        );
        let scale = window.scale_factor() as f32;
        let renderer =
            Renderer::new(window.clone(), self.font_size * scale).expect("failed to init renderer");
        let accessibility_actions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let accessibility = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            &window,
            AccessibilityActivation,
            AccessibilityActionQueue {
                queue: accessibility_actions.clone(),
                proxy: self
                    .event_proxy
                    .clone()
                    .expect("event loop proxy is required"),
            },
            AccessibilityDeactivation,
        );
        window.set_visible(true);
        window.set_ime_allowed(true);
        self.gpu = Some(Gpu {
            window,
            renderer,
            accessibility,
            accessibility_actions,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(gpu) = &mut self.gpu {
            gpu.accessibility.process_event(&gpu.window, &event);
        }
        self.process_accessibility_actions();
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = position;
                self.drag_select();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.click();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.selecting = false,
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 3.0,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 20.0,
                };
                let max = self.tabs[self.active_tab]
                    .buffer
                    .len_lines()
                    .saturating_sub(1) as f32;
                self.tabs[self.active_tab].scroll_line =
                    (self.tabs[self.active_tab].scroll_line as f32 + lines).clamp(0.0, max)
                        as usize;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_key(&event);
            }
            WindowEvent::Ime(winit::event::Ime::Enabled) => self.ime_composing = true,
            WindowEvent::Ime(winit::event::Ime::Preedit(_, _)) => self.ime_composing = true,
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.tabs[self.active_tab].buffer.insert(&text);
                self.ime_composing = false;
            }
            WindowEvent::Ime(winit::event::Ime::Disabled) => self.ime_composing = false,
            WindowEvent::RedrawRequested => self.draw(),
            _ => return,
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::AccessibilityWake => {
                self.process_accessibility_actions();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
        }
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
        let root = std::env::temp_dir().join("shoppe_app_tabs_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let first = root.join("first.rs");
        let second = root.join("second.rs");
        fs::write(&first, "one").unwrap();
        fs::write(&second, "two").unwrap();
        let mut app = App::new(root.clone(), None);
        app.open_file(first.clone());
        app.tabs[app.active_tab].buffer.set_cursor_line_col(0, 3);
        app.tabs[app.active_tab].buffer.insert("!");
        app.open_file(second);
        app.open_file(first);
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "one!");
        assert_eq!(app.tabs[0].buffer.text(), "one!");
        assert_eq!(app.tabs[1].buffer.text(), "two");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn accessibility_text_positions_map_to_buffer_offsets() {
        let mut b = Buffer::new();
        b.insert("one\ntwo");
        assert_eq!(accessibility_position_to_char(&b, NodeId(1_001), 2), 6);
    }
}
