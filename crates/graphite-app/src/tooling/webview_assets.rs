//! Serves an extension view's page and its local resources through a private custom protocol.
//!
//! The page is loaded from the protocol's own origin, so scripts (including ES modules), styles,
//! fonts and images the extension references from its allowed folders are same-origin requests.
//! Loading them as `file:` URLs from an inline page is not an option: browsers refuse module
//! scripts and cross-origin local files there. Every resource request is checked against the
//! allowed roots after canonicalization, so `..` segments and symbolic links cannot escape them.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};
use wry::http::{header, Request, Response, StatusCode};

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

fn respond(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(body))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

/// Answers one protocol request: the page, an allowed local file, or an error.
pub(crate) fn serve(
    page: &str,
    roots: &[PathBuf],
    request: &Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path();
    if path == PAGE_PATH {
        return respond(
            StatusCode::OK,
            "text/html; charset=utf-8",
            page.as_bytes().to_vec(),
        );
    }
    let Some(file) = resolve_resource(path, roots) else {
        return respond(StatusCode::FORBIDDEN, "text/plain", b"Forbidden".to_vec());
    };
    let readable = std::fs::metadata(&file)
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= MAX_RESOURCE);
    match readable.then(|| std::fs::read(&file).ok()).flatten() {
        Some(bytes) => respond(StatusCode::OK, content_type(&file), bytes),
        None => respond(StatusCode::NOT_FOUND, "text/plain", b"Not found".to_vec()),
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
}
