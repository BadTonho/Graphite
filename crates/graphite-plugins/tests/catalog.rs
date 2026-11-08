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
    let release = r#"{"draft":false,"prerelease":false,"assets":[{"name":"one.graphite-plugin","size":1,"browser_download_url":"https://github.com/one"},{"name":"two.vsix","size":2,"browser_download_url":"https://github.com/two"},{"name":"source.zip","size":2,"browser_download_url":"https://github.com/three"}]}"#;
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
    assert!(catalog.assets("author/plugin").is_err());
    assert!(catalog.search("", 1).is_err());
    assert!(catalog.search("", 2).unwrap_err().contains("rate limit"));
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
