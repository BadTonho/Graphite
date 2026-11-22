use super::tests::pause_writer;
use crate::App;
use std::{
    fs,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

const CHILD_WORKSPACE: &str = "GRAPHITE_TEST_INTERRUPTED_SAVE_WORKSPACE";
const DOCUMENT: &str = "save.txt";
const READY: &str = "writer-before-replace";
const SAVED: &str = "Salvo: café 🌎\n";
const UNCOMMITTED: &str = "Ainda não gravado\n";

#[test]
fn interrupted_save_preserves_last_complete_version_on_reopen() {
    if let Some(path) = std::env::var_os(CHILD_WORKSPACE) {
        save_until_replacement(Path::new(&path));
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let child_temp = temp.path().join("child-temp");
    fs::create_dir(&workspace).unwrap();
    fs::create_dir(&child_temp).unwrap();
    let document = workspace.join(DOCUMENT);
    fs::write(&document, b"Original\r\n").unwrap();

    let (_, test_module) = module_path!().split_once("::").unwrap();
    let test = format!("{test_module}::interrupted_save_preserves_last_complete_version_on_reopen");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test])
        .env(CHILD_WORKSPACE, &workspace)
        .env("TEMP", &child_temp)
        .env("TMP", &child_temp)
        .env("TMPDIR", &child_temp)
        .spawn()
        .unwrap();

    // The marker is written only after the real writer has synced its temporary file.
    let ready = workspace.join(READY);
    let deadline = Instant::now() + Duration::from_secs(15);
    let reached_replacement = loop {
        match child.try_wait() {
            Ok(None) if ready.is_file() => break true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => break false,
        }
    };
    let killed = child.kill();
    let status = child.wait().unwrap();
    assert!(
        reached_replacement,
        "writer never reached replacement: {status}"
    );
    killed.expect("terminate the editor while its writer is blocked");
    assert!(!status.success());

    assert_eq!(fs::read_to_string(&document).unwrap(), "Salvo: café 🌎\r\n");
    let mut reopened = App::new_with_options(workspace.clone(), None, true);
    reopened.open_file(document.clone());
    assert_eq!(reopened.editor().text(), SAVED);
    assert!(!reopened.editor().is_dirty());

    reopened.set_save_test_text("Depois de reabrir\n");
    reopened.save_document(reopened.active_document());
    reopened.wait_for_saves();
    assert!(!reopened.editor().is_dirty());
    assert_eq!(
        fs::read_to_string(document).unwrap(),
        "Depois de reabrir\r\n"
    );
}

fn save_until_replacement(workspace: &Path) {
    let mut app = App::new_with_options(workspace.into(), None, true);
    app.open_file(workspace.join(DOCUMENT));
    let document = app.active_document();
    app.set_save_test_text(SAVED);
    app.save_document(document);
    app.wait_for_saves();
    assert!(!app.editor().is_dirty());

    app.set_save_test_text(UNCOMMITTED);
    app.save_document(document);
    let (started, resume) = pause_writer(&mut app, document, false);
    app.poll_saves(Instant::now());
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    fs::write(workspace.join(READY), b"").unwrap();
    // The parent must terminate this process before the writer can replace the file.
    thread::sleep(Duration::from_secs(4));
    drop(resume);
    panic!("interrupted-save process was not terminated");
}
