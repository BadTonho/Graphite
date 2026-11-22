use std::{fs, path::Path, process::Command};

fn plugin_cli(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graphite"));
    command
        .current_dir(root)
        .env("APPDATA", root.join("config"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("GRAPHITE_PLUGIN_STORE", root.join("store"));
    command
}

#[test]
fn failed_plugin_command_records_one_high_severity_error() {
    let root = tempfile::tempdir().unwrap();
    let output = plugin_cli(root.path())
        .env("GRAPHITE_LANG", "en")
        .args(["plugin", "unknown"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("Usage: graphite plugin "), "{stderr}");

    let log = fs::read_to_string(root.path().join("config/Graphite/logs/errors/high.jsonl"))
        .expect("the failed plugin command should write a high-severity log");
    assert_eq!(log.lines().count(), 1, "{log}");
    let entry: serde_json::Value = serde_json::from_str(&log).unwrap();
    assert_eq!(entry["severity"], "high");
    assert_eq!(entry["source"], "cli.plugin");
    assert_eq!(entry["message"], stderr.trim_end());
}

#[test]
fn plugin_cli_creates_project_rejects_unbuilt_entry_and_packages_without_opening_window() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let cli = |args: &[&str]| plugin_cli(root).args(args).output().unwrap();
    assert!(cli(&["plugin", "create", "example"]).status.success());
    assert!(root.join("example/graphite.d.ts").exists());
    assert!(!cli(&["plugin", "create", "example"]).status.success());
    assert!(!cli(&["plugin", "pack", "example"]).status.success());
    fs::create_dir(root.join("example/dist")).unwrap();
    fs::write(
        root.join("example/dist/index.js"),
        "export function activate() {};",
    )
    .unwrap();
    let output = cli(&["plugin", "pack", "example"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = String::from_utf8(output.stdout).unwrap();
    assert!(Path::new(path.trim()).exists());
    assert!(!cli(&["plugin", "unknown", "example"]).status.success());

    let store_dir = root.join("store");
    let vsix = root.join("fixture.vsix");
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(fs::File::create(&vsix).unwrap());
        zip.start_file(
            "extension/package.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(br#"{"publisher":"fixture","name":"vsix","version":"1.0.0"}"#)
            .unwrap();
        zip.finish().unwrap();
    }
    let installed = cli(&["plugin", "install", "fixture.vsix"]);
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    let record = graphite_plugins::store::Store::new(store_dir.clone())
        .get("fixture.vsix")
        .unwrap();
    assert!(!record.enabled);
    assert!(record.trusted_version.is_none());

    let dev_output = cli(&["plugin", "dev", "example"]);
    assert!(
        dev_output.status.success(),
        "{}",
        String::from_utf8_lossy(&dev_output.stderr)
    );
    assert!(store_dir.join("local.example.json").exists());

    let unlink_output = cli(&["plugin", "unlink", "local.example"]);
    assert!(
        unlink_output.status.success(),
        "{}",
        String::from_utf8_lossy(&unlink_output.stderr)
    );
    assert!(!store_dir.join("local.example.json").exists());
    assert!(root.join("example/dist/index.js").exists());
}
