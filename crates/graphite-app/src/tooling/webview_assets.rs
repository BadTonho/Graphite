//! Serves an extension view's page and its local resources through a private custom protocol.
//!
//! The page is loaded from the protocol's own origin, so scripts (including ES modules), styles,
//! fonts and images the extension references from its allowed folders are same-origin requests.
//! Loading them as `file:` URLs from an inline page is not an option: browsers refuse module
//! scripts and cross-origin local files there. Every resource request is checked against the
//! allowed roots after canonicalization, so `..` segments and symbolic links cannot escape them.

#[cfg(test)]
use std::borrow::Cow;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
};
use wry::http::StatusCode;
#[cfg(test)]
use wry::http::{header, Request, Response};

/// Path of the page itself on the custom origin.
pub(crate) const PAGE_PATH: &str = "/index.html";
/// Path prefix under which allowed local files are served.
const RESOURCE_PREFIX: &str = "/r/";
/// Largest resource served, in bytes.
const MAX_RESOURCE: u64 = 64 * 1024 * 1024;

/// Rewrites the `file:///` URLs the extension host produced for local resources into
/// origin-relative URLs under [`RESOURCE_PREFIX`].
pub(crate) fn rewrite_resource_urls(html: &str) -> String {
    html.replace("file:///", RESOURCE_PREFIX)
}

/// The URL the page is loaded from for a protocol named `scheme`.
#[cfg(test)]
pub(crate) fn page_url(scheme: &str) -> String {
    format!("{scheme}://localhost{PAGE_PATH}")
}

/// Whether `url` belongs to the custom origin named `scheme` on any platform.
pub(crate) fn is_internal_url(scheme: &str, url: &str) -> bool {
    [
        format!("{scheme}://"),
        format!("http://{scheme}.localhost/"),
        format!("https://{scheme}.localhost/"),
    ]
    .iter()
    .any(|prefix| url.starts_with(prefix.as_str()))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = input.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Resolves a resource request path (`/r/C:/dir/file.js`) to a file inside `roots`.
fn resolve_resource(request_path: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let decoded = percent_decode(request_path.strip_prefix(RESOURCE_PREFIX)?)?;
    if decoded.contains('\0') || decoded.split(['/', '\\']).any(|part| part == "..") {
        return None;
    }
    let bytes = decoded.as_bytes();
    let has_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let local = if has_drive || decoded.starts_with('/') {
        decoded
    } else {
        format!("/{decoded}")
    };
    let resolved = std::fs::canonicalize(Path::new(&local)).ok()?;
    roots
        .iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .any(|root| resolved.starts_with(root))
        .then_some(resolved)
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("wasm") => "application/wasm",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
fn respond(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(body))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

/// Answers one protocol request: the page, an allowed local file, or an error.
#[cfg(test)]
pub(crate) fn serve(
    page: &str,
    roots: &[PathBuf],
    request: &Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let (status, mime, body) = serve_path(page, roots, request.uri().path());
    respond(status, mime, body)
}

pub(crate) fn serve_path(
    page: &str,
    roots: &[PathBuf],
    path: &str,
) -> (StatusCode, &'static str, Vec<u8>) {
    if path == PAGE_PATH {
        return (
            StatusCode::OK,
            "text/html; charset=utf-8",
            page.as_bytes().to_vec(),
        );
    }
    let Some(file) = resolve_resource(path, roots) else {
        return (StatusCode::FORBIDDEN, "text/plain", b"Forbidden".to_vec());
    };
    let readable = std::fs::metadata(&file)
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= MAX_RESOURCE);
    match readable.then(|| std::fs::read(&file).ok()).flatten() {
        Some(bytes) => (StatusCode::OK, content_type(&file), bytes),
        None => (StatusCode::NOT_FOUND, "text/plain", b"Not found".to_vec()),
    }
}

/// Serves an extension view's page and allowed local files on `http://127.0.0.1:<port>` so
/// nested iframes whose `frame-ancestors` allow `http://127.0.0.1:*` can be embedded.
pub(crate) struct LoopbackOrigin {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LoopbackOrigin {
    pub(crate) fn start(
        page: String,
        roots: Vec<PathBuf>,
        on_ipc: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let Ok(stream) = stream else { break };
                let page = page.clone();
                let roots = roots.clone();
                let on_ipc = on_ipc.clone();
                thread::spawn(move || handle_http(stream, &page, &roots, on_ipc.as_ref()));
            }
        });
        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }

    pub(crate) fn page_url(&self) -> String {
        format!("http://127.0.0.1:{}{PAGE_PATH}", self.port)
    }
}

impl Drop for LoopbackOrigin {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle_http(
    mut stream: TcpStream,
    page: &str,
    roots: &[PathBuf],
    on_ipc: &(dyn Fn(String) + Send + Sync),
) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(cloned) => cloned,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let mut content_length = 0usize;
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err()
            || header_line == "\r\n"
            || header_line.is_empty()
        {
            break;
        }
        if let Some((name, value)) = header_line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    if method == "POST" && path == "/__graphite_ipc" {
        if content_length > 48 * 1024 * 1024 {
            let _ = stream.write_all(
                b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            return;
        }
        let mut body = vec![0u8; content_length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        if let Ok(text) = String::from_utf8(body) {
            on_ipc(text);
        }
        let _ = stream.write_all(
            b"HTTP/1.1 204 No Content\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        );
        return;
    }
    let (status, mime, body) = match method {
        "GET" | "HEAD" => serve_path(page, roots, path),
        _ => (
            StatusCode::METHOD_NOT_ALLOWED,
            "text/plain",
            b"Method not allowed".to_vec(),
        ),
    };
    let reason = status.canonical_reason().unwrap_or("OK");
    let header = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status.as_u16(),
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if method != "HEAD" {
        let _ = stream.write_all(&body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(path: &str) -> Request<Vec<u8>> {
        Request::builder()
            .uri(format!("graphite-view://localhost{path}"))
            .body(Vec::new())
            .unwrap()
    }

    fn url_path(file: &Path) -> String {
        let text = std::fs::canonicalize(file)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let clean = text.strip_prefix("//?/").unwrap_or(&text).to_owned();
        let uri = format!("file:///{}", clean.trim_start_matches('/'));
        rewrite_resource_urls(&uri)
    }

    fn body(response: &Response<Cow<'static, [u8]>>) -> String {
        String::from_utf8_lossy(response.body()).into_owned()
    }

    #[test]
    fn the_page_is_served_and_file_urls_become_origin_relative() {
        let response = serve("<p>hi</p>", &[], &request(PAGE_PATH));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body(&response), "<p>hi</p>");
        assert_eq!(
            rewrite_resource_urls(r#"<script src="file:///C:/x/a.js"></script>"#),
            r#"<script src="/r/C:/x/a.js"></script>"#
        );
        assert_eq!(
            page_url("graphite-view-1"),
            "graphite-view-1://localhost/index.html"
        );
    }

    #[test]
    fn allowed_files_are_served_with_their_type_and_others_are_refused() {
        let dir = crate::test_support::TempDir::new();
        let allowed = dir.path().join("allowed");
        std::fs::create_dir_all(allowed.join("nested")).unwrap();
        std::fs::write(allowed.join("nested/app.js"), "export const x = 1;").unwrap();
        std::fs::write(allowed.join("a b.css"), "body{}").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "secret").unwrap();
        let roots = [allowed.clone()];

        let script = serve(
            "",
            &roots,
            &request(&url_path(&allowed.join("nested/app.js"))),
        );
        assert_eq!(script.status(), StatusCode::OK);
        assert_eq!(body(&script), "export const x = 1;");
        assert_eq!(
            script.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );

        let spaced = url_path(&allowed.join("a b.css")).replace(' ', "%20");
        let style = serve("", &roots, &request(&spaced));
        assert_eq!(style.status(), StatusCode::OK);
        assert_eq!(
            style.headers()[header::CONTENT_TYPE],
            "text/css; charset=utf-8"
        );

        let outside = serve(
            "",
            &roots,
            &request(&url_path(&dir.path().join("secret.txt"))),
        );
        assert_eq!(outside.status(), StatusCode::FORBIDDEN);
        assert!(!body(&outside).contains("secret"));
    }

    #[test]
    fn traversal_encoded_traversal_and_missing_files_are_refused() {
        let dir = crate::test_support::TempDir::new();
        let allowed = dir.path().join("allowed");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "secret").unwrap();
        let roots = [allowed.clone()];
        let base = url_path(&allowed);

        for path in [
            format!("{base}/../secret.txt"),
            format!("{base}/%2e%2e/secret.txt"),
            format!("{base}/..%5Csecret.txt"),
            format!("{base}/bad%zz"),
            format!("{base}/a%00b"),
            "/elsewhere".to_owned(),
        ] {
            assert_eq!(
                serve("", &roots, &request(&path)).status(),
                StatusCode::FORBIDDEN,
                "{path}"
            );
        }
        assert_eq!(
            serve("", &roots, &request(&format!("{base}/missing.js"))).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            serve("", &roots, &request(&base)).status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn the_custom_origin_is_recognized_on_every_platform() {
        assert!(is_internal_url(
            "graphite-view-1",
            "graphite-view-1://localhost/index.html"
        ));
        assert!(is_internal_url(
            "graphite-view-1",
            "http://graphite-view-1.localhost/index.html"
        ));
        assert!(is_internal_url(
            "graphite-view-1",
            "https://graphite-view-1.localhost/r/x"
        ));
        assert!(!is_internal_url("graphite-view-1", "https://example.com/"));
        assert!(!is_internal_url(
            "graphite-view-1",
            "http://graphite-view-12.localhost/"
        ));
        assert!(!is_internal_url("graphite-view-1", "about:blank"));
    }

    #[test]
    fn loopback_origin_serves_page_and_receives_ipc_posts() {
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = received.clone();
        let origin = LoopbackOrigin::start(
            "<h1>ok</h1>".into(),
            Vec::new(),
            Arc::new(move |body| sink.lock().unwrap().push(body)),
        )
        .unwrap();

        let mut get_stream = TcpStream::connect(("127.0.0.1", origin.port)).unwrap();
        get_stream
            .write_all(b"GET /index.html HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let mut get_response = String::new();
        get_stream.read_to_string(&mut get_response).unwrap();
        assert!(get_response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(get_response.ends_with("<h1>ok</h1>"));

        let payload = "{\"type\":\"ready\"}";
        let mut post_stream = TcpStream::connect(("127.0.0.1", origin.port)).unwrap();
        let req = format!(
            "POST /__graphite_ipc HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{payload}",
            payload.len()
        );
        post_stream.write_all(req.as_bytes()).unwrap();
        let mut post_response = String::new();
        post_stream.read_to_string(&mut post_response).unwrap();
        assert!(post_response.starts_with("HTTP/1.1 204 No Content\r\n"));
        assert_eq!(received.lock().unwrap().as_slice(), [payload]);
    }
}
