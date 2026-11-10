//! Safe Markdown rendering and image metadata for native preview panes.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MarkdownPreview {
    pub source: PathBuf,
    pub html: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImagePreview {
    pub source: PathBuf,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

pub(crate) fn render_markdown(source: &Path, text: &str) -> MarkdownPreview {
    let body = markdown_to_html(text);
    MarkdownPreview {
        source: source.to_path_buf(),
        html: format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: file:; style-src 'unsafe-inline'; font-src data: file:; script-src 'unsafe-inline'\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>body{{font:15px/1.65 system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 2rem;color:#d4d4d4;background:#1e1e1e}}pre{{overflow:auto;padding:1rem;background:#252526}}code{{font-family:monospace}}img{{max-width:100%}}a{{color:#75beff}}</style></head><body>{body}<script>let pending=false;addEventListener('scroll',()=>{{if(pending)return;pending=true;requestAnimationFrame(()=>{{pending=false;window.ipc.postMessage(JSON.stringify({{type:'markdown-scroll',ratio:scrollY/Math.max(1,document.documentElement.scrollHeight-innerHeight)}}));}})}});window.addEventListener('message',event=>{{if(event.data?.type==='markdown-scroll-to')scrollTo(0,event.data.ratio*Math.max(0,document.documentElement.scrollHeight-innerHeight));}});</script></body></html>"
        ),
    }
}

fn markdown_to_html(text: &str) -> String {
    let mut out = String::new();
    let mut paragraph = Vec::new();
    let mut code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            flush_paragraph(&mut out, &mut paragraph);
            if code {
                out.push_str("</code></pre>");
            } else {
                out.push_str("<pre><code>");
            }
            code = !code;
            continue;
        }
        if code {
            out.push_str(&escape_html(line));
            out.push('\n');
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_paragraph(&mut out, &mut paragraph);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_paragraph(&mut out, &mut paragraph);
            out.push_str(&format!("<h3>{}</h3>", inline_markdown(rest)));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_paragraph(&mut out, &mut paragraph);
            out.push_str(&format!("<h2>{}</h2>", inline_markdown(rest)));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_paragraph(&mut out, &mut paragraph);
            out.push_str(&format!("<h1>{}</h1>", inline_markdown(rest)));
        } else {
            paragraph.push(inline_markdown(trimmed));
        }
    }
    flush_paragraph(&mut out, &mut paragraph);
    if code {
        out.push_str("</code></pre>");
    }
    out
}

fn flush_paragraph(out: &mut String, lines: &mut Vec<String>) {
    if !lines.is_empty() {
        out.push_str("<p>");
        out.push_str(&lines.join("<br>"));
        out.push_str("</p>");
        lines.clear();
    }
}

fn inline_markdown(text: &str) -> String {
    let mut escaped = escape_html(text);
    for (delimiter, tag) in [("**", "strong"), ("*", "em"), ("`", "code")] {
        let mut parts = escaped.split(delimiter);
        let mut result = String::new();
        if let Some(first) = parts.next() {
            result.push_str(first);
        }
        for (index, part) in parts.enumerate() {
            let open = index % 2 == 0;
            if open {
                result.push('<');
                result.push_str(tag);
                result.push('>');
            } else {
                result.push_str("</");
                result.push_str(tag);
                result.push('>');
            }
            result.push_str(part);
        }
        escaped = result;
    }
    escaped
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn inspect_image(path: &Path) -> Result<ImagePreview, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    let (width, height) = image::image_dimensions(path).map_err(|error| error.to_string())?;
    Ok(ImagePreview {
        source: path.to_path_buf(),
        width,
        height,
        bytes: metadata.len(),
    })
}

pub(crate) fn is_previewable_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_preview_escapes_raw_html_and_disallows_active_content() {
        let preview = render_markdown(
            Path::new("README.md"),
            "# Title\n\n<script>alert(1)</script>",
        );
        assert!(preview.html.contains("<h1>Title</h1>"));
        assert!(preview.html.contains("&lt;script&gt;"));
        assert!(preview.html.contains("default-src 'none'"));
        assert!(!preview.html.contains("<script>alert(1)</script>"));
        assert!(preview.html.contains("type:'markdown-scroll'"));
    }

    #[test]
    fn image_inspection_reports_dimensions_size_and_supported_formats() {
        let temp = crate::test_support::TempDir::new();
        let path = temp.path().join("fixture.png");
        let image = image::RgbaImage::from_pixel(3, 2, image::Rgba([20, 30, 40, 255]));
        image.save(&path).unwrap();
        let preview = inspect_image(&path).unwrap();
        assert_eq!((preview.width, preview.height), (3, 2));
        assert_eq!(preview.bytes, std::fs::metadata(&path).unwrap().len());
        assert!(is_previewable_image(&path));
        assert!(!is_previewable_image(Path::new("photo.jpg")));
    }
}
