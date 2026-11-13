use super::*;

impl App {
    pub(crate) fn open_active_preview(&mut self) -> Result<(), String> {
        let path = self.documents[self.active_document()]
            .buffer
            .path()
            .ok_or("Save the file before opening its preview")?
            .to_path_buf();
        if path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        }) {
            let text = self.documents[self.active_document()].buffer.text();
            self.markdown_preview = Some(crate::previews::render_markdown(&path, &text));
            self.preview_source_scroll = None;
            self.image_preview = None;
            self.preview_zoom = 1.0;
            let html = self.markdown_preview.as_ref().unwrap().html.clone();
            self.show_webview_html(&html)?;
            return Ok(());
        }
        if crate::previews::is_previewable_image(&path) {
            self.image_preview = Some(crate::previews::inspect_image(&path)?);
            self.markdown_preview = None;
            self.preview_source_scroll = None;
            self.preview_zoom = 1.0;
            self.show_image_preview(&path)?;
            return Ok(());
        }
        Err("Preview is available for Markdown and PNG images".into())
    }

    pub(crate) fn preview_zoom(&mut self, factor: f32) {
        self.preview_zoom = (self.preview_zoom * factor).clamp(0.1, 8.0);
        if let Some(path) = self
            .image_preview
            .as_ref()
            .map(|preview| preview.source.clone())
        {
            let _ = self.show_image_preview(&path);
        }
    }

    pub(crate) fn show_webview_html(&mut self, html: &str) -> Result<(), String> {
        let window = self
            .gpu
            .as_ref()
            .map(|gpu| gpu.window.clone())
            .ok_or("The native window is not ready")?;
        self.webview = Some(crate::webview::WebviewHost::open(&window, html)?);
        self.webview_html = html.to_owned();
        Ok(())
    }

    pub(crate) fn show_image_preview(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let encoded = base64(&bytes);
        let width = self
            .image_preview
            .as_ref()
            .map(|preview| preview.width)
            .unwrap_or(0);
        let height = self
            .image_preview
            .as_ref()
            .map(|preview| preview.height)
            .unwrap_or(0);
        let bytes = self
            .image_preview
            .as_ref()
            .map(|preview| preview.bytes)
            .unwrap_or_default();
        let size = if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KiB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
        };
        let html = format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\"><style>body{{margin:0;background:#202020;color:#ddd;font:14px sans-serif}}header{{position:fixed;top:0;background:#252526;padding:8px;width:100%}}img{{display:block;margin:64px auto;max-width:calc(100% - 32px);max-height:calc(100vh - 80px);object-fit:contain;transform:scale({});transform-origin:center top}}</style></head><body><header>{width} × {height} · {size} · {}</header><img src=\"data:image/png;base64,{encoded}\"></body></html>", self.preview_zoom, path.file_name().unwrap_or_default().to_string_lossy());
        self.show_webview_html(&html)
    }

    pub(crate) fn close_webview(&mut self) {
        self.webview = None;
        self.webview_html.clear();
        self.webview_handle = None;
        self.preview_source_scroll = None;
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}
