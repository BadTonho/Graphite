//! Native webview host. Windows uses a child WebView2; Linux creates a GTK
//! WebKitGTK window so the same surface remains available on X11 and Wayland.

#[cfg(target_os = "linux")]
use gtk::prelude::*;
use std::sync::{Arc, Mutex};
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

impl WebviewHost {
    pub(crate) fn open(parent: &winit::window::Window, html: &str) -> Result<Self, String> {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let builder = WebViewBuilder::new()
            .with_html(enforce_csp(html))
            .with_ipc_handler(move |request: Request<String>| {
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
        )
    }

    /// Open an extension-provided webview over `bounds` (a sidebar view). The content is wrapped in
    /// the enforced policy (scripts only when `enable_scripts`), gets the `acquireVsCodeApi` shim,
    /// and its `postMessage` calls are queued for [`Self::take_messages`].
    pub(crate) fn open_extension_overlay(
        parent: &winit::window::Window,
        html: &str,
        bounds: [f64; 4],
        enable_scripts: bool,
    ) -> Result<Self, String> {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let builder = WebViewBuilder::new()
            .with_html(enforce_csp_with(html, enable_scripts))
            .with_ipc_handler(move |request: Request<String>| {
                if request.body().len() <= 64 * 1024 {
                    if let Ok(mut queue) = sink.lock() {
                        queue.push(request.body().clone());
                    }
                }
            });
        Self::build_overlay(parent, builder, messages, bounds, "Graphite View")
    }

    fn build_overlay(
        parent: &winit::window::Window,
        builder: WebViewBuilder<'_>,
        messages: Arc<Mutex<Vec<String>>>,
        bounds: [f64; 4],
        title: &str,
    ) -> Result<Self, String> {
        let builder = builder
            .with_background_color((13, 17, 23, 255))
            .with_navigation_handler(|url| {
                if url == "about:blank" {
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

fn enforce_csp(html: &str) -> String {
    enforce_csp_with(html, true)
}

fn enforce_csp_with(html: &str, enable_scripts: bool) -> String {
    let csp = if enable_scripts {
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: file:; media-src data: file:; style-src 'unsafe-inline' file:; script-src 'unsafe-inline' file:; font-src data: file:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri file:\">"
    } else {
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: file:; media-src data: file:; style-src 'unsafe-inline' file:; script-src 'none'; font-src data: file:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri file:\">"
    };
    const API: &str = "<script>(()=>{let state;window.acquireVsCodeApi=()=>({postMessage:m=>window.ipc.postMessage(JSON.stringify(m)),getState:()=>state,setState:s=>(state=s)});})();</script>";
    if let Some(index) = html.to_ascii_lowercase().find("<head>") {
        let split = index + "<head>".len();
        format!("{}{}{}{}", &html[..split], csp, API, &html[split..])
    } else {
        format!("<html><head>{csp}{API}</head><body>{html}</body></html>")
    }
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
