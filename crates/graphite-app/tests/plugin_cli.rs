use std::{fs, path::Path, process::Command};
#[test]
fn plugin_cli_creates_project_rejects_unbuilt_entry_and_packages_without_opening_window() {
    let root = std::env::temp_dir().join(format!("graphite-plugin-cli-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let cli = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_graphite"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap()
    };
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
    let dev_output = Command::new(env!("CARGO_BIN_EXE_graphite"))
        .current_dir(&root)
        .env("GRAPHITE_PLUGIN_STORE", &store_dir)
        .args(["plugin", "dev", "example"])
        .output()
        .unwrap();
    assert!(
        dev_output.status.success(),
        "{}",
        String::from_utf8_lossy(&dev_output.stderr)
    );
    assert!(store_dir.join("local.example.json").exists());

    let unlink_output = Command::new(env!("CARGO_BIN_EXE_graphite"))
        .current_dir(&root)
        .env("GRAPHITE_PLUGIN_STORE", &store_dir)
        .args(["plugin", "unlink", "local.example"])
        .output()
        .unwrap();
    assert!(
        unlink_output.status.success(),
        "{}",
        String::from_utf8_lossy(&unlink_output.stderr)
    );
    assert!(!store_dir.join("local.example.json").exists());
    assert!(root.join("example/dist/index.js").exists());
}
