//! Native webview host. Windows uses a child WebView2; Linux creates a GTK
//! WebKitGTK window so the same surface remains available on X11 and Wayland.

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

fn enforce_csp(html: &str) -> String {
    const CSP: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: file:; media-src data: file:; style-src 'unsafe-inline' file:; script-src 'unsafe-inline' file:; font-src data: file:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri file:\">";
    const API: &str = "<script>(()=>{let state;window.acquireVsCodeApi=()=>({postMessage:m=>window.ipc.postMessage(JSON.stringify(m)),getState:()=>state,setState:s=>(state=s)});})();</script>";
    if let Some(index) = html.to_ascii_lowercase().find("<head>") {
        let split = index + "<head>".len();
        format!("{}{}{}{}", &html[..split], CSP, API, &html[split..])
    } else {
        format!("<html><head>{CSP}{API}</head><body>{html}</body></html>")
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
}
