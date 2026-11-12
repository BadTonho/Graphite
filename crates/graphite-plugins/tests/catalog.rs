use graphite_plugins::catalog::{Asset, Catalog};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};
fn server(
    replies: Vec<(u16, String, String)>,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    let join = thread::spawn(move || {
        for (status, headers, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            log.lock()
                .unwrap()
                .push(String::from_utf8(request).unwrap());
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len()).unwrap();
        }
    });
    (base, requests, join)
}
#[test]
fn search_topic_pagination_cache_and_release_asset_selection() {
    let body = r#"{"items":[{"full_name":"author/plugin","description":"Example","stargazers_count":3,"html_url":"https://github.com/author/plugin"}]}"#;
    let release = r#"{"tag_name":"v1.0.0","draft":false,"prerelease":false,"assets":[{"name":"one.graphite-plugin","size":1,"browser_download_url":"https://github.com/one"},{"name":"two.vsix","size":2,"browser_download_url":"https://github.com/two"},{"name":"source.zip","size":2,"browser_download_url":"https://github.com/three"}]}"#;
    let (base, log, join) = server(vec![
        (200, String::new(), body.into()),
        (200, String::new(), body.into()),
        (200, String::new(), release.into()),
    ]);
    let mut catalog = Catalog::with_base(&base).unwrap();
    assert_eq!(
        catalog.search("test", 1).unwrap()[0].full_name,
        "author/plugin"
    );
    assert_eq!(catalog.search("test", 1).unwrap().len(), 1);
    catalog.search("test", 2).unwrap();
    assert_eq!(catalog.assets("author/plugin").unwrap().len(), 2);
    assert!(catalog.assets("../other/path").is_err());
    join.join().unwrap();
    let requests = log.lock().unwrap();
    assert!(requests[0].contains("topic%3Agraphite-plugin"));
    assert!(requests[1].contains("page=2"));
    assert_eq!(requests.len(), 3);
}
#[test]
fn missing_release_and_rate_limit_are_reported_and_backoff_prevents_requests() {
    let (base, _, join) = server(vec![
        (404, String::new(), "{}".into()),
        (429, "Retry-After: 60\r\n".into(), "{}".into()),
    ]);
    let mut catalog = Catalog::with_base(&base).unwrap();
    assert!(catalog
        .assets("author/plugin")
        .unwrap_err()
        .contains("No stable GitHub release"));
    assert!(catalog.search("", 1).is_err());
    assert!(catalog.search("", 2).unwrap_err().contains("rate limit"));
    join.join().unwrap();
}

#[test]
fn release_without_supported_assets_is_listed_as_empty() {
    let release = r#"{"tag_name":"v1.0.0","draft":false,"prerelease":false,"assets":[{"name":"source.zip","size":2,"browser_download_url":"https://github.com/three"}]}"#;
    let (base, _, join) = server(vec![(200, String::new(), release.into())]);
    let mut catalog = Catalog::with_base(&base).unwrap();
    assert!(catalog.assets("author/plugin").unwrap().is_empty());
    join.join().unwrap();
}

#[test]
fn details_loads_repository_readme_and_sums_only_installable_asset_downloads() {
    let repository = r#"{"full_name":"author/plugin","description":"Example","stargazers_count":3,"html_url":"https://github.com/author/plugin","homepage":"https://example.test","topics":["graphite-plugin","editor"],"license":{"name":"MIT License","spdx_id":"MIT"},"open_issues_count":2}"#;
    let readme = r#"{"encoding":"base64","content":"IyBIZWxsbyBXb3JsZAo="}"#;
    let release = r#"{"tag_name":"v1.2.0","name":"Version 1.2.0","published_at":"2026-10-30T10:00:00Z","draft":false,"prerelease":false,"assets":[{"name":"plugin.graphite-plugin","size":100,"download_count":9,"browser_download_url":"https://github.com/author/plugin/releases/download/v1/plugin.graphite-plugin"},{"name":"plugin.vsix","size":200,"download_count":4,"browser_download_url":"https://github.com/author/plugin/releases/download/v1/plugin.vsix"},{"name":"source.zip","size":300,"download_count":100,"browser_download_url":"https://github.com/author/plugin/releases/download/v1/source.zip"}]}"#;
    let (base, log, join) = server(vec![
        (200, String::new(), repository.into()),
        (200, String::new(), readme.into()),
        (
            200,
            String::new(),
            "<h1>Hello World</h1><p><img src=\"https://github.com/author/plugin/raw/main/logo.png\"></p>"
                .into(),
        ),
        (200, String::new(), release.into()),
    ]);
    let mut catalog = Catalog::with_base(&base).unwrap();
    let details = catalog.details("author/plugin").unwrap();
    assert_eq!(details.readme.as_deref(), Some("# Hello World\n"));
    assert!(details
        .readme_html
        .as_deref()
        .unwrap()
        .contains("<h1>Hello World</h1>"));
    assert_eq!(details.repository.stargazers_count, 3);
    assert_eq!(details.repository.topics, ["graphite-plugin", "editor"]);
    let release = details.release.unwrap();
    assert_eq!(release.tag_name, "v1.2.0");
    assert_eq!(release.assets.len(), 2);
    assert_eq!(release.downloads, 13);
    assert_eq!(release.size, 300);
    join.join().unwrap();
    let requests = log.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[2].starts_with("GET /repos/author/plugin/readme HTTP/1.1"));
    assert!(requests[2].contains("accept: application/vnd.github.html+json"));
}

#[test]
fn details_keep_repository_visible_when_readme_and_stable_release_are_missing() {
    let repository = r#"{"full_name":"author/plugin","description":null,"stargazers_count":0,"html_url":"https://github.com/author/plugin"}"#;
    let (base, _, join) = server(vec![
        (200, String::new(), repository.into()),
        (404, String::new(), "{}".into()),
        (404, String::new(), "{}".into()),
    ]);
    let mut catalog = Catalog::with_base(&base).unwrap();
    let details = catalog.details("author/plugin").unwrap();
    assert!(details.readme.is_none());
    assert!(details.release.is_none());
    join.join().unwrap();
}

#[test]
fn downloads_are_bounded_and_partial_downloads_are_removed() {
    let (base, _, join) = server(vec![
        (200, String::new(), "abc".into()),
        (200, String::new(), "ab".into()),
        (500, String::new(), "error".into()),
    ]);
    let mut catalog = Catalog::with_base(&base).unwrap();
    let path =
        std::env::temp_dir().join(format!("graphite-catalog-download-{}", std::process::id()));
    let asset = Asset {
        name: "sample.vsix".into(),
        size: 3,
        browser_download_url: format!("{base}/asset"),
        download_count: 0,
    };
    catalog.download(&asset, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"abc");
    std::fs::remove_file(&path).unwrap();
    assert!(catalog.download(&asset, &path).is_err());
    assert!(!path.exists());
    assert!(catalog.download(&asset, &path).is_err());
    assert!(!path.exists());
    let huge = Asset {
        size: 101 * 1024 * 1024,
        ..asset
    };
    assert!(catalog.download(&huge, &path).is_err());
    join.join().unwrap();
}
