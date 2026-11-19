//! Native webview host. Windows uses a child WebView2; Linux creates a GTK
//! WebKitGTK window so the same surface remains available on X11 and Wayland.

use super::{webview_assets, webview_theme};
use crate::app::error_log::{self, ErrorSeverity};
#[cfg(target_os = "linux")]
use gtk::prelude::*;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use wry::{http::Request, WebView, WebViewBuilder};

#[cfg(windows)]
pub(crate) struct WebviewHost {
    view: WebView,
    messages: Arc<Mutex<Vec<String>>>,
}

#[cfg(target_os = "linux")]
pub(crate) struct WebviewHost {
    view: WebView,
    window: gtk::Window,
    messages: Arc<Mutex<Vec<String>>>,
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(crate) struct WebviewHost {
    view: WebView,
    messages: Arc<Mutex<Vec<String>>>,
}

/// What an extension view page sends once its document has loaded and its scripts have run.
pub(crate) const PAGE_LOADED: &str = "__graphite_loaded";
/// Appended to [`PAGE_SCRIPT`] for extension overlays so the host knows when to deliver messages.
const LOADED_SIGNAL: &str = concat!(
    "(()=>{const l=()=>window.ipc.postMessage('",
    "__graphite_loaded",
    "');document.readyState==='loading'?document.addEventListener('DOMContentLoaded',l):l();})();"
);

/// Reports, to the error log, which kinds of messages an extension page exchanged with its
/// extension and whether its document rendered. Only message `type` names are sent, never their
/// content. Runs in extension overlays only.
const TRAFFIC_PROBE: &str = concat!(
    "(()=>{const out=[],inn=[];",
    "const d=m=>window.ipc.postMessage('__graphite_diag:'+String(m).slice(0,300));",
    "const ty=m=>{try{const v=typeof m==='string'?JSON.parse(m):m;",
    "const i=v&&(v.message||v.request);",
    "return String((v&&v.type)||typeof v).slice(0,24)+(i&&i.type?'/'+String(i.type).slice(0,32):'')+(i&&i.error!==undefined?'!':'')}",
    "catch(e){return 'raw'}};",
    "const api=window.acquireVsCodeApi;",
    "window.acquireVsCodeApi=()=>{const a=api();const p=a.postMessage;",
    "a.postMessage=m=>{out.push(ty(m));return p(m)};return a};",
    "addEventListener('message',e=>{inn.push(ty(e.data))},true);",
    "const report=()=>d('traffic: page sent '+out.length+' ['+out.slice(0,12)+'], received '+inn.length+' ['+inn.slice(0,12)+'], '+",
    "(document.body?document.body.innerText.length:0)+' characters of text, '+document.querySelectorAll('*').length+' elements');",
    "setTimeout(report,6000);setTimeout(report,20000);})();"
);

/// Marks a message the page script sent about its own failures, not for the extension.
const DIAGNOSTIC_PREFIX: &str = "__graphite_diag:";
/// Most page diagnostics recorded per run, so a failing page cannot flood the error log.
const MAX_DIAGNOSTICS: usize = 20;
static DIAGNOSTICS: AtomicUsize = AtomicUsize::new(0);

/// Records a page diagnostic in the error log. Returns whether `body` was one.
fn record_page_diagnostic(body: &str) -> bool {
    let Some(detail) = body.strip_prefix(DIAGNOSTIC_PREFIX) else {
        return false;
    };
    if DIAGNOSTICS.fetch_add(1, Ordering::Relaxed) < MAX_DIAGNOSTICS {
        error_log::record(
            ErrorSeverity::Medium,
            "webview.page",
            format!("Extension view reported: {detail}"),
        );
    }
    true
}

impl WebviewHost {
    pub(crate) fn open(parent: &winit::window::Window, html: &str) -> Result<Self, String> {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let builder = WebViewBuilder::new()
            .with_html(enforce_csp(html))
            .with_ipc_handler(move |request: Request<String>| {
                if record_page_diagnostic(request.body()) {
                    return;
                }
                if request.body().len() <= 64 * 1024 {
                    if let Ok(mut queue) = sink.lock() {
                        queue.push(request.body().clone());
                    }
                }
            });
        #[cfg(windows)]
        let view = builder
            .build_as_child(parent)
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "linux")]
        let (view, window) = {
            use wry::WebViewBuilderExtUnix;
            gtk::init().map_err(|error| error.to_string())?;
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title("Graphite Webview");
            window.set_default_size(1000, 720);
            let container = gtk::Fixed::new();
            window.add(&container);
            window.show_all();
            let view = builder
                .build_gtk(&container)
                .map_err(|error| error.to_string())?;
            let _ = parent;
            (view, window)
        };
        #[cfg(not(any(windows, target_os = "linux")))]
        let view = builder
            .build_as_child(parent)
            .map_err(|error| error.to_string())?;
        #[cfg(windows)]
        {
            let scale = parent.scale_factor();
            let size = parent.inner_size();
            view.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0, 0).into(),
                size: wry::dpi::LogicalSize::new(
                    size.width as f64 / scale,
                    size.height as f64 / scale,
                )
                .into(),
            })
            .map_err(|error| error.to_string())?;
        }
        #[cfg(windows)]
        {
            let _ = parent;
            Ok(Self { view, messages })
        }
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                view,
                window,
                messages,
            })
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = parent;
            Ok(Self { view, messages })
        }
    }

    /// Open a read-only document surface aligned over a native editor rectangle.
    /// Windows hosts it as a child WebView2. Linux uses a borderless GTK/WebKitGTK
    /// surface positioned over the editor area on window managers that support it.
    pub(crate) fn open_overlay(
        parent: &winit::window::Window,
        html: &str,
        bounds: [f64; 4],
    ) -> Result<Self, String> {
        let messages = Arc::new(Mutex::new(Vec::new()));
        Self::build_overlay(
            parent,
            WebViewBuilder::new().with_html(html.to_owned()),
            messages,
            bounds,
            "Graphite README",
            None,
            (13, 17, 23, 255),
        )
    }

    /// Open an extension-provided webview over `bounds` (a sidebar view). The page and the local
    /// files it references from `roots` are served from a private origin, so module scripts,
    /// styles and fonts load as same-origin requests. The content is wrapped in the enforced
    /// policy (scripts only when `enable_scripts`), gets the `acquireVsCodeApi` shim, and its
    /// `postMessage` calls are queued for [`Self::take_messages`].
    pub(crate) fn open_extension_overlay(
        parent: &winit::window::Window,
        html: &str,
        bounds: [f64; 4],
        enable_scripts: bool,
        roots: Vec<PathBuf>,
    ) -> Result<Self, String> {
        static NEXT_ORIGIN: AtomicUsize = AtomicUsize::new(0);
        let scheme = format!(
            "graphite-view-{}",
            NEXT_ORIGIN.fetch_add(1, Ordering::Relaxed)
        );
        let page = enforce_view_csp(
            &webview_assets::rewrite_resource_urls(html),
            enable_scripts,
            webview_theme::Kind::current(),
        );
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let builder = WebViewBuilder::new()
            .with_custom_protocol(scheme.clone(), move |_, request| {
                webview_assets::serve(&page, &roots, &request)
            })
            .with_url(webview_assets::page_url(&scheme))
            // Injected by the engine, so the page's own script policy (nonces) cannot block it.
            .with_initialization_script(format!("{PAGE_SCRIPT}{LOADED_SIGNAL}{TRAFFIC_PROBE}"))
            .with_ipc_handler(move |request: Request<String>| {
                if record_page_diagnostic(request.body()) {
                    return;
                }
                if request.body().len() <= 64 * 1024 {
                    if let Ok(mut queue) = sink.lock() {
                        queue.push(request.body().clone());
                    }
                }
            });
        Self::build_overlay(
            parent,
            builder,
            messages,
            bounds,
            "Graphite View",
            Some(scheme),
            webview_theme::background(),
        )
    }

    fn build_overlay(
        parent: &winit::window::Window,
        builder: WebViewBuilder<'_>,
        messages: Arc<Mutex<Vec<String>>>,
        bounds: [f64; 4],
        title: &str,
        internal_scheme: Option<String>,
        background: (u8, u8, u8, u8),
    ) -> Result<Self, String> {
        let builder = builder
            .with_background_color(background)
            .with_navigation_handler(move |url| {
                if url == "about:blank"
                    || internal_scheme
                        .as_deref()
                        .is_some_and(|scheme| webview_assets::is_internal_url(scheme, &url))
                {
                    return true;
                }
                open_external_https(&url);
                false
            })
            .with_new_window_req_handler(|url, _| {
                open_external_https(&url);
                wry::NewWindowResponse::Deny
            })
            .with_download_started_handler(|_, _| false)
            .with_permission_handler(|_| wry::PermissionResponse::Deny);
        #[cfg(windows)]
        let view = builder
            .build_as_child(parent)
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "linux")]
        let (view, window) = {
            use wry::WebViewBuilderExtUnix;
            gtk::init().map_err(|error| error.to_string())?;
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_decorated(false);
            window.set_resizable(false);
            let container = gtk::Fixed::new();
            window.add(&container);
            let view = builder
                .build_gtk(&container)
                .map_err(|error| error.to_string())?;
            (view, window)
        };
        #[cfg(not(any(windows, target_os = "linux")))]
        let view = builder
            .build_as_child(parent)
            .map_err(|error| error.to_string())?;
        let _ = title;

        #[cfg(windows)]
        let host = Self { view, messages };
        #[cfg(target_os = "linux")]
        let host = Self {
            view,
            window,
            messages,
        };
        #[cfg(not(any(windows, target_os = "linux")))]
        let host = Self { view, messages };
        host.set_overlay_bounds(parent, bounds)?;
        #[cfg(target_os = "linux")]
        if let Some(window) = host.overlay_window() {
            window.show_all();
        }
        Ok(host)
    }

    /// Shows or hides the surface without destroying its page state.
    pub(crate) fn set_visible(&self, visible: bool) -> Result<(), String> {
        self.view
            .set_visible(visible)
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "linux")]
        if let Some(window) = self.overlay_window() {
            if visible {
                window.show_all();
            } else {
                window.hide();
            }
        }
        Ok(())
    }

    pub(crate) fn take_messages(&self) -> Vec<String> {
        self.messages
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default()
    }

    pub(crate) fn evaluate_script(&self, script: &str) -> Result<(), String> {
        self.view
            .evaluate_script(script)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn set_bounds(&self, parent: &winit::window::Window) -> Result<(), String> {
        #[cfg(windows)]
        {
            let scale = parent.scale_factor();
            let size = parent.inner_size();
            self.view
                .set_bounds(wry::Rect {
                    position: wry::dpi::LogicalPosition::new(0, 0).into(),
                    size: wry::dpi::LogicalSize::new(
                        size.width as f64 / scale,
                        size.height as f64 / scale,
                    )
                    .into(),
                })
                .map_err(|error| error.to_string())?;
        }
        let _ = parent;
        Ok(())
    }

    pub(crate) fn set_overlay_bounds(
        &self,
        parent: &winit::window::Window,
        bounds: [f64; 4],
    ) -> Result<(), String> {
        let scale = parent.scale_factor();
        let [x, y, width, height] = bounds;
        #[cfg(windows)]
        self.view
            .set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(x / scale, y / scale).into(),
                size: wry::dpi::LogicalSize::new(width / scale, height / scale).into(),
            })
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "linux")]
        {
            use wry::dpi::{LogicalPosition, LogicalSize};
            self.view
                .set_bounds(wry::Rect {
                    position: LogicalPosition::new(0.0, 0.0).into(),
                    size: LogicalSize::new(width / scale, height / scale).into(),
                })
                .map_err(|error| error.to_string())?;
            if let Some(window) = self.overlay_window() {
                window.resize(
                    (width / scale).round() as i32,
                    (height / scale).round() as i32,
                );
                if let Ok(origin) = parent.outer_position() {
                    window.move_(
                        (origin.x as f64 / scale + x / scale).round() as i32,
                        (origin.y as f64 / scale + y / scale).round() as i32,
                    );
                }
            }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        self.view
            .set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(x / scale, y / scale).into(),
                size: wry::dpi::LogicalSize::new(width / scale, height / scale).into(),
            })
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn overlay_window(&self) -> Option<&gtk::Window> {
        Some(&self.window)
    }

    pub(crate) fn pump(&self) {
        #[cfg(target_os = "linux")]
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        let _ = &self.view;
        #[cfg(target_os = "linux")]
        let _ = &self.window;
    }
}

pub(crate) fn open_external_https(target: &str) {
    let Some(target) = external_https_url(target) else {
        return;
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("explorer.exe");
        command.arg(&target);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(&target);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(&target);
        command
    };
    let _ = command.spawn();
}

fn external_https_url(target: &str) -> Option<String> {
    let url = reqwest::Url::parse(target).ok()?;
    (url.scheme() == "https" && url.host_str().is_some()).then(|| url.to_string())
}

/// Runs in every page: the `acquireVsCodeApi` shim and reports of the page's own failures.
const PAGE_SCRIPT: &str = concat!(
        "(()=>{let state;",
        "window.acquireVsCodeApi=()=>({postMessage:m=>window.ipc.postMessage(JSON.stringify(m)),",
        "getState:()=>state,setState:s=>(state=s)});",
        // Failures of the page itself, reported to the error log: script errors, rejected
        // promises, blocked resources and policy violations.
        "const d=m=>window.ipc.postMessage('__graphite_diag:'+String(m).slice(0,300));",
        "addEventListener('error',e=>{const t=e.target;",
        "d(t&&t!==window&&(t.src||t.href)?'could not load '+(t.src||t.href):",
        "'script error: '+e.message+' ('+e.lineno+':'+e.colno+')')},true);",
        "addEventListener('unhandledrejection',e=>d('unhandled rejection: '+(e.reason&&e.reason.message||e.reason)));",
        "document.addEventListener('securitypolicyviolation',e=>d('policy blocked '+e.violatedDirective+': '+e.blockedURI));",
        "})();"
    );

fn enforce_csp(html: &str) -> String {
    enforce_csp_with(html, true)
}

fn enforce_csp_with(html: &str, enable_scripts: bool) -> String {
    let csp = if enable_scripts {
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: file:; media-src data: file:; style-src 'unsafe-inline' file:; script-src 'unsafe-inline' file:; font-src data: file:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri file:\">"
    } else {
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: file:; media-src data: file:; style-src 'unsafe-inline' file:; script-src 'none'; font-src data: file:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri file:\">"
    };
    inject_head(html, &format!("{csp}<script>{PAGE_SCRIPT}</script>"))
}

/// Inserts `head` at the start of the document's `<head>`, creating the document when needed.
fn inject_head(html: &str, head: &str) -> String {
    if let Some(index) = html.to_ascii_lowercase().find("<head>") {
        let split = index + "<head>".len();
        format!("{}{}{}", &html[..split], head, &html[split..])
    } else {
        format!("<html><head>{head}</head><body>{html}</body></html>")
    }
}

/// Like [`enforce_csp_with`], for pages served from their own origin: local resources are
/// same-origin (`'self'`) instead of `file:`.
fn enforce_view_csp(html: &str, enable_scripts: bool, kind: webview_theme::Kind) -> String {
    let scripts = if enable_scripts {
        "'unsafe-inline' 'self'"
    } else {
        "'none'"
    };
    let csp = format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: 'self'; media-src data: 'self'; style-src 'unsafe-inline' 'self'; script-src {scripts}; font-src data: 'self'; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'self'\">"
    );
    inject_head(
        &webview_theme::themed_body(html, kind),
        &format!("{csp}{}", webview_theme::style_element(kind)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforced_policy_is_added_even_when_extension_supplies_its_own_html() {
        let html = enforce_csp("<html><head><title>ok</title></head><body>x</body></html>");
        assert!(html.contains("default-src 'none'"));
        assert!(html.contains("connect-src 'none'"));
        assert!(html.contains("<title>ok</title>"));
    }

    #[test]
    fn page_diagnostics_are_recognized_and_kept_from_the_extension() {
        assert!(!record_page_diagnostic("{\"type\":\"ready\"}"));
        assert!(record_page_diagnostic("__graphite_diag:script error: boom"));
        assert!(enforce_csp("<p>x</p>").contains("securitypolicyviolation"));
    }

    #[test]
    fn view_pages_use_their_own_origin_and_get_the_shim_outside_the_page_policy() {
        let page = enforce_view_csp(
            "<html><head></head><body>x</body></html>",
            true,
            webview_theme::Kind::Dark,
        );
        assert!(page.contains("script-src 'unsafe-inline' 'self'"));
        assert!(page.contains("style-src 'unsafe-inline' 'self'"));
        assert!(!page.contains("file:"));
        // The theme variables and the body class that pages style themselves with.
        assert!(page.contains("--vscode-editor-foreground:"));
        assert!(page.contains("class=\"vscode-dark\""));
        // The shim is injected by the engine; an inline copy would be blocked by page nonces.
        assert!(!page.contains("acquireVsCodeApi"));
        assert!(PAGE_SCRIPT.contains("acquireVsCodeApi"));
        assert!(
            enforce_view_csp("<p>x</p>", false, webview_theme::Kind::Dark)
                .contains("script-src 'none'")
        );
    }

    #[test]
    fn the_loaded_signal_matches_what_the_host_listens_for() {
        assert!(LOADED_SIGNAL.contains(PAGE_LOADED));
        assert!(
            !TRAFFIC_PROBE.contains("innerHTML"),
            "the probe must not send page content"
        );
        assert!(
            !PAGE_SCRIPT.contains(PAGE_LOADED),
            "only extension overlays send it"
        );
    }

    #[test]
    fn scripts_are_blocked_unless_the_view_enables_them() {
        let html = "<html><head></head><body></body></html>";
        assert!(enforce_csp_with(html, false).contains("script-src 'none'"));
        assert!(!enforce_csp_with(html, false).contains("script-src 'unsafe-inline'"));
        assert!(enforce_csp_with(html, true).contains("script-src 'unsafe-inline'"));
        assert!(enforce_csp_with("<p>x</p>", false).contains("script-src 'none'"));
    }

    #[test]
    fn readme_external_navigation_only_accepts_https() {
        assert_eq!(
            external_https_url("https://github.com/owner/repo"),
            Some("https://github.com/owner/repo".into())
        );
        assert!(external_https_url("http://github.com/owner/repo").is_none());
        assert!(external_https_url("javascript:alert(1)").is_none());
    }
}
