use graphite_plugins::{
    manifest::{Manifest, Metadata},
    package, sdk,
    store::Store,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "graphite-plugin-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn project(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    sdk::create(&path).unwrap();
    fs::create_dir(path.join("dist")).unwrap();
    fs::write(
        path.join("dist/index.js"),
        "export function activate() { throw new Error('must not run during install'); }",
    )
    .unwrap();
    path
}
fn zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, body) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
}
#[test]
fn scaffold_pack_install_trust_update_and_remove_preserve_other_plugins() {
    let temp = Temp::new();
    let source = project(&temp.0, "example");
    assert!(sdk::create(&source).is_err());
    let store = Store::new(temp.0.join("store"));
    let package = package::pack(&source).unwrap();
    let first = store.install(&package).unwrap();
    assert!(!first.enabled);
    assert!(first.trusted_version.is_none());
    assert!(store.enable("local.example", false).is_err());
    store.enable("local.example", true).unwrap();
    assert!(
        Store::new(store.root.clone())
            .get("local.example")
            .unwrap()
            .enabled
    );
    store.disable("local.example").unwrap();
    assert!(!store.get("local.example").unwrap().enabled);
    let mut manifest: Manifest =
        serde_json::from_slice(&fs::read(source.join("graphite-plugin.json")).unwrap()).unwrap();
    manifest.version = "0.2.0".into();
    fs::write(
        source.join("graphite-plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let update = package::pack(&source).unwrap();
    store.install(&update).unwrap();
    let updated = store.get("local.example").unwrap();
    assert_eq!(updated.metadata.version(), "0.2.0");
    assert!(!updated.enabled);
    assert!(updated.trusted_version.is_none());
    assert!(store.root.join(&first.directory).exists());
    let peer = project(&temp.0, "example-other");
    let peer = package::pack(&peer).unwrap();
    let peer = store.install(&peer).unwrap();
    store.remove("local.example").unwrap();
    assert!(store.get("local.example").is_err());
    assert!(store.root.join(peer.directory).is_dir());
    assert!(store.get("local.example-other").is_ok());
}
#[test]
fn failed_update_keeps_previous_registry_and_contents() {
    let temp = Temp::new();
    let source = project(&temp.0, "example");
    let store = Store::new(temp.0.join("store"));
    let package = package::pack(&source).unwrap();
    let first = store.install(&package).unwrap();
    store.enable("local.example", true).unwrap();
    // A stale transaction lock deterministically simulates a publication failure.
    fs::write(store.root.join(".local.example.tmp"), "locked").unwrap();
    assert!(store.install(&package).is_err());
    assert!(store.get("local.example").unwrap().enabled);
    assert_eq!(
        store.get("local.example").unwrap().directory,
        first.directory
    );
    assert!(store.root.join(first.directory).is_dir());
    assert!(fs::read_dir(&store.root).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".staging")));
}
#[test]
fn invalid_manifests_and_archive_paths_are_rejected_without_writes() {
    let temp = Temp::new();
    for name in [
        "../escape",
        "C:/escape",
        "dir\\escape",
        "/escape",
        "a/../b",
        "NUL",
        "a./file",
        "a ",
    ] {
        assert!(!package::safe_path(name), "{name}");
        let path = temp.0.join("bad.vsix");
        zip(&path, &[(name, b"bad")]);
        assert!(package::inspect(&path).is_err());
        assert!(!temp.0.join("escape").exists());
    }
    let path = temp.0.join("bad.vsix");
    zip(&path, &[("a", b"x"), ("A", b"y")]);
    assert!(package::inspect(&path).is_err());
    zip(&path, &[("a", b"x"), ("a/b", b"y")]);
    assert!(package::inspect(&path).is_err());
    let source = project(&temp.0, "example");
    let mut m: Manifest =
        serde_json::from_slice(&fs::read(source.join("graphite-plugin.json")).unwrap()).unwrap();
    m.api_version = 2;
    assert!(m.validate().is_err());
    m.api_version = 1;
    m.commands[0].id = "undo".into();
    assert!(m.validate().is_err());
    m.commands.clear();
    m.version = "bad".into();
    assert!(m.validate().is_err());
    m.version = "1.0.0".into();
    m.main = "../evil.js".into();
    assert!(m.validate().is_err());
}
#[test]
fn vsix_installation_and_version_approval_do_not_run_code() {
    let temp = Temp::new();
    let path = temp.0.join("sample.vsix");
    let sentinel = temp.0.join("should-not-exist");
    let entry = format!(
        "require('node:fs').writeFileSync({}, 'bad')",
        serde_json::to_string(&sentinel).unwrap()
    );
    zip(&path, &[("extension/package.json", br#"{"publisher":"Example","name":"vsix","version":"1.0.0","engines":{"vscode":"^1.90.0"},"main":"main.js","contributes":{"commands":[{"command":"example.run","title":"Run"}]}}"#), ("extension/main.js", entry.as_bytes())]);
    let store = Store::new(temp.0.join("store"));
    let record = store.install(&path).unwrap();
    let Metadata::Vsix(m) = record.metadata else {
        panic!("not VSIX")
    };
    assert_eq!(m.contributions, vec!["commands"]);
    assert_eq!(m.main.as_deref(), Some("main.js"));
    assert_eq!(m.commands[0].id, "example.run");
    assert!(store.enable("example.vsix", false).is_err());
    store.enable("example.vsix", true).unwrap();
    let approved = store.get("example.vsix").unwrap();
    assert!(approved.enabled);
    assert_eq!(approved.trusted_version.as_deref(), Some("1.0.0"));
    // Reinstalling even the same version replaces the archive and approval.
    store.install(&path).unwrap();
    let replacement = store.get("example.vsix").unwrap();
    assert!(!replacement.enabled);
    assert_eq!(replacement.trusted_version, None);
    assert!(store
        .enable_installation("example.vsix", &approved.directory, true)
        .is_err());
    assert!(!temp.0.join("should-not-exist").exists());
    zip(&path, &[("extension/package.json", b"{}")]);
    assert!(package::inspect(&path).is_err());
}
#[test]
fn package_limits_reject_expansion_and_special_files() {
    let temp = Temp::new();
    let path = temp.0.join("large.VSIX");
    let manifest = br#"{"publisher":"example","name":"large","version":"1.0.0"}"#;
    zip(
        &path,
        &[
            ("extension/package.json", manifest),
            ("a", b"a"),
            ("b", b"b"),
        ],
    );
    let mut bytes = fs::read(&path).unwrap();
    let entries: Vec<_> = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(offset, sig)| (sig == b"PK\x01\x02").then_some(offset))
        .collect();
    let half = 2u32 * 1024 * 1024 * 1024;
    for (offset, size) in [
        (entries[1], half),
        (entries[2], half - manifest.len() as u32),
    ] {
        bytes[offset + 24..offset + 28].copy_from_slice(&size.to_le_bytes());
    }
    // Inspection uses the central directory size and reads only the manifest.
    fs::write(&path, &bytes).unwrap();
    assert!(package::inspect(&path).is_ok());
    let native = temp.0.join("large.graphite-plugin");
    fs::write(&native, &bytes).unwrap();
    assert!(package::inspect(&native).unwrap_err().contains("500 MiB"));
    let second = entries[2];
    bytes[second + 24..second + 28]
        .copy_from_slice(&(half - manifest.len() as u32 + 1).to_le_bytes());
    fs::write(&path, bytes).unwrap();
    assert!(package::inspect(&path).unwrap_err().contains("4 GiB"));
    let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    writer
        .add_symlink(
            "link",
            "../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.finish().unwrap();
    assert!(package::inspect(&path).is_err());
}

#[test]
fn vsix_entry_limit_accepts_zip64_and_reports_counts_above_the_ceiling() {
    let temp = Temp::new();
    let path = temp.0.join("many-entries.vsix");
    let write_archive = |count| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("extension/package.json", options)
            .unwrap();
        writer
            .write_all(br#"{"publisher":"example","name":"many","version":"1.0.0"}"#)
            .unwrap();
        for index in 1..count {
            writer
                .start_file(format!("extension/{index}.txt"), options)
                .unwrap();
        }
        writer.finish().unwrap();
    };

    write_archive(100_000);
    assert_eq!(package::inspect(&path).unwrap().id(), "example.many");
    let native = temp.0.join("same-entries.graphite-plugin");
    fs::rename(&path, &native).unwrap();
    let error = package::inspect(&native).unwrap_err();
    assert_eq!(error, "Package has 100000 entries (limit 10000)");

    write_archive(100_001);
    let store = Store::new(temp.0.join("store"));
    let error = store.install(&path).unwrap_err();
    assert_eq!(error, "Package has 100001 entries (limit 100000)");
    assert!(!store.root.exists());
    assert_eq!(
        package::entry_limit_diagnostic(&error),
        Some(error.as_str())
    );
    assert_eq!(
        package::entry_limit_diagnostic(&format!("{error} unexpected content")),
        None
    );
}

#[test]
fn reviewed_update_cannot_overwrite_a_changed_installation() {
    let temp = Temp::new();
    let path = temp.0.join("sample.vsix");
    let store = Store::new(temp.0.join("store"));
    let write_version = |version: &str| {
        let manifest = format!(r#"{{"publisher":"example","name":"test","version":"{version}"}}"#);
        zip(&path, &[("extension/package.json", manifest.as_bytes())]);
    };
    write_version("1.0.0");
    let initial = store.install(&path).unwrap();
    store.enable("example.test", true).unwrap();
    write_version("2.0.0");
    assert!(store
        .install_update(&path, "example.test", &initial.directory, "3.0.0")
        .is_err());
    assert_eq!(
        store.get("example.test").unwrap().directory,
        initial.directory
    );
    assert!(store.get("example.test").unwrap().enabled);
    write_version("1.1.0");
    let replacement = store.install(&path).unwrap();
    write_version("2.0.0");
    assert!(store
        .install_update(&path, "example.test", &initial.directory, "2.0.0")
        .is_err());
    assert_eq!(
        store.get("example.test").unwrap().directory,
        replacement.directory
    );
    let updated = store
        .install_update(&path, "example.test", &replacement.directory, "2.0.0")
        .unwrap();
    assert_eq!(updated.metadata.version(), "2.0.0");
    assert!(!updated.enabled);
    assert!(updated.trusted_version.is_none());
}
#[test]
fn missing_node_has_an_actionable_error() {
    assert!(sdk::check_node(Path::new("graphite-test-nonexistent-node"))
        .unwrap_err()
        .contains("Node.js"));
}

#[test]
fn link_dev_tracks_source_directory_refreshes_manifest_and_unlink_preserves_source() {
    let temp = Temp::new();
    let source = project(&temp.0, "dev-example");
    let store = Store::new(temp.0.join("store"));
    let linked = store.link_dev(&source).unwrap();
    assert!(linked.enabled);
    assert_eq!(linked.trusted_version.as_deref(), Some("0.1.0"));
    assert_eq!(
        linked.install_dir(&store.root),
        fs::canonicalize(&source).unwrap()
    );

    let mut manifest: Manifest =
        serde_json::from_slice(&fs::read(source.join("graphite-plugin.json")).unwrap()).unwrap();
    manifest.version = "0.3.0".into();
    manifest.description = "Live updated".into();
    fs::write(
        source.join("graphite-plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let refreshed = store.get("local.dev-example").unwrap();
    assert_eq!(refreshed.metadata.version(), "0.3.0");
    assert_eq!(refreshed.trusted_version.as_deref(), Some("0.3.0"));
    if let Metadata::Native(native) = refreshed.metadata {
        assert_eq!(native.description, "Live updated");
    } else {
        panic!("expected native manifest");
    }

    store.unlink_dev("local.dev-example").unwrap();
    assert!(store.get("local.dev-example").is_err());
    assert!(source.join("graphite-plugin.json").exists());
    assert!(source.join("dist/index.js").exists());
}
