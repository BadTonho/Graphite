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

pub(crate) fn github_readme_document(rendered_html: &str) -> String {
    let rendered_html = github_readme_fragment(rendered_html);
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src https:; style-src 'unsafe-inline'; font-src https: data:; script-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'"><style>
        :root,html,body{{min-height:100%;color-scheme:dark;background:#0d1117!important;color:#e6edf3!important;font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif}}
        *{{box-sizing:border-box}}body{{min-height:100vh;margin:0;padding:24px 32px 48px;background:#0d1117!important;color:#e6edf3!important;overflow-wrap:anywhere}}
        .markdown-body{{max-width:1012px;margin:0 auto}}.markdown-body>*:first-child{{margin-top:0!important}}
        .markdown-body h1,.markdown-body h2{{padding-bottom:.3em;border-bottom:1px solid #30363d}}
        .markdown-body h1{{font-size:2em}}.markdown-body h2{{font-size:1.5em}}.markdown-body h3{{font-size:1.25em}}
        .markdown-body h1,.markdown-body h2,.markdown-body h3,.markdown-body h4{{margin:24px 0 16px;font-weight:600;line-height:1.25}}
        .markdown-body p,.markdown-body blockquote,.markdown-body ul,.markdown-body ol,.markdown-body dl,.markdown-body table,.markdown-body pre{{margin:0 0 16px}}
        .markdown-body a{{color:#4493f8;text-decoration:none}}.markdown-body a:hover{{text-decoration:underline}}
        .markdown-body strong{{font-weight:600}}.markdown-body blockquote{{padding:0 1em;color:#9198a1;border-left:.25em solid #3d444d}}
        .markdown-body code,.markdown-body tt{{padding:.2em .4em;border-radius:6px;background:#151b23;font:85% ui-monospace,SFMono-Regular,Consolas,monospace}}
        .markdown-body pre{{padding:16px;overflow:auto;border-radius:6px;background:#151b23;line-height:1.45}}
        .markdown-body pre code{{padding:0;background:transparent;font-size:100%;white-space:pre}}
        .markdown-body table{{display:block;width:max-content;max-width:100%;overflow:auto;border-spacing:0;border-collapse:collapse}}
        .markdown-body th,.markdown-body td{{padding:6px 13px;border:1px solid #3d444d}}
        .markdown-body tr:nth-child(2n){{background:#151b23}}.markdown-body img{{max-width:100%;height:auto;background:#fff;border-radius:4px}}
        .markdown-body hr{{height:.25em;margin:24px 0;border:0;background:#30363d}}
        .markdown-body ul,.markdown-body ol{{padding-left:2em}}.markdown-body li+li{{margin-top:.25em}}
        .markdown-body .task-list-item{{list-style:none}}.markdown-body .task-list-item input{{margin:0 .5em 0 -1.5em;accent-color:#1f6feb}}
        .markdown-body kbd{{padding:2px 5px;border:1px solid #3d444d;border-radius:4px;background:#151b23;box-shadow:inset 0 -1px #3d444d;font:11px ui-monospace,monospace}}
        </style></head><body><article class="markdown-body">{rendered_html}</article></body></html>"#
    )
}

fn github_readme_fragment(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    let Some(body_start) = lower.find("<body") else {
        return html;
    };
    let Some(content_start) = lower[body_start..]
        .find('>')
        .map(|offset| body_start + offset + 1)
    else {
        return html;
    };
    let content_end = lower[content_start..]
        .rfind("</body>")
        .map(|offset| content_start + offset)
        .unwrap_or(html.len());
    &html[content_start..content_end]
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
    fn github_readme_document_has_dark_gfm_styles_and_a_restrictive_policy() {
        let document = github_readme_document(
            "<h1>Example</h1><img src=\"https://example.test/logo.png\"><script>alert(1)</script><form></form>",
        );
        assert!(document.contains(".markdown-body table"));
        assert!(document.contains(".markdown-body .task-list-item"));
        assert!(document.contains("img-src https:"));
        assert!(document.contains("script-src 'none'"));
        assert!(document.contains("form-action 'none'"));
        assert!(document.contains("<h1>Example</h1>"));
    }

    #[test]
    fn github_readme_document_uses_body_from_a_full_html_response() {
        let document = github_readme_document(
            "<!doctype html><html><head><title>GitHub</title></head><body><h1>README content</h1></body></html>",
        );
        assert!(document.contains("<h1>README content</h1>"));
        assert!(!document.contains("<title>GitHub</title>"));
        assert!(document.contains("background:#0d1117!important"));
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
