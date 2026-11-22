use ring::digest::{digest, SHA256};
use std::{fs, path::PathBuf, process::Command};

#[test]
fn executable_starts_again_after_rejecting_a_tampered_update() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let config = root.join("config");
    let source = std::env::var_os("GRAPHITE_TEST_PACKAGED_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_graphite")));
    let install = root.join("install");
    fs::create_dir(&install).unwrap();
    let executable = install.join(format!("graphite{}", std::env::consts::EXE_SUFFIX));
    fs::copy(source, &executable).unwrap();
    let original = digest(&SHA256, &fs::read(&executable).unwrap());

    let run = |project: &str| {
        let output = Command::new(&executable)
            .current_dir(root)
            .env("APPDATA", &config)
            .env("XDG_CONFIG_HOME", &config)
            .env("GRAPHITE_LANG", "en")
            .args(["plugin", "create", project])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(root.join(project).join("graphite-plugin.json").is_file());
    };
    run("first-start");

    let app_config = config.join("Graphite");
    let updates = app_config.join("profiles/Default/updates");
    fs::create_dir(&updates).unwrap();
    let payload = updates.join("changed.zip");
    fs::write(&payload, b"package changed after verification").unwrap();
    let marker = updates.join("pending-install.json");
    fs::write(
        &marker,
        serde_json::json!({
            "tag": "v99.0.0",
            "staged_path": payload,
            "sha256": "0".repeat(64),
        })
        .to_string(),
    )
    .unwrap();

    run("rejected-update");
    assert!(!marker.exists());
    assert!(!updates.join("just-updated.json").exists());
    let current = digest(&SHA256, &fs::read(&executable).unwrap());
    assert_eq!(current.as_ref(), original.as_ref());

    let log_file = app_config.join("logs/errors/high.jsonl");
    let log = fs::read_to_string(&log_file).unwrap();
    let entry: serde_json::Value = serde_json::from_str(&log).unwrap();
    assert_eq!(entry["source"], "updates.apply");
    assert_eq!(entry["severity"], "high");
    assert!(entry["message"]
        .as_str()
        .unwrap()
        .contains("refusing to install"));

    run("after-restart");
    assert_eq!(fs::read_to_string(log_file).unwrap(), log);
}
