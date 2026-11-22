use super::*;
use crate::file_format;
use std::{process::Command, thread};

const CHILD: &str = "GRAPHITE_TEST_DRAFT_RECOVERY_WORKSPACE";
const ORIGINAL: &str = "original\n";
const EDIT: &str = "café 🌎\n";

fn app(workspace: &Path, profile: &Path) -> App {
    let mut app = App::new_with_options(workspace.into(), None, true);
    app.profile_root = profile.into();
    app.drafts = Recovery::for_profile(profile);
    app
}

fn drain(app: &mut App, now: Instant) {
    let started = Instant::now();
    loop {
        app.draft_tick(now);
        if !app.drafts_busy() {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "draft write did not finish"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn snapshot(app: &App, id: DocumentId) -> PathBuf {
    app.drafts.tracked[&id].snapshot.clone()
}

fn insert(app: &mut App, text: &str) {
    let end = app.editor().len_chars();
    app.editor_mut()
        .apply_transaction(&[graphite_text::TextReplacement {
            range: 0..end,
            text: text.into(),
        }])
        .unwrap();
}

#[test]
fn killed_process_recovers_file_and_untitled_without_writing_the_source() {
    if let Some(workspace) = std::env::var_os(CHILD) {
        let workspace = PathBuf::from(workspace);
        let mut app = app(&workspace, &workspace.join("profile"));
        app.open_file(workspace.join("document.txt"));
        insert(&mut app, EDIT);
        app.new_untitled_document();
        insert(&mut app, "untitled draft");
        drain(&mut app, Instant::now());
        assert_eq!(
            app.drafts
                .tracked
                .values()
                .filter(|entry| entry.written.is_some())
                .count(),
            2
        );
        fs::write(workspace.join("ready"), b"").unwrap();
        thread::sleep(Duration::from_secs(25));
        panic!("test process was not terminated");
    }

    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let path = root.join("document.txt");
    let original = file_format::encode(ORIGINAL, Encoding::Utf16Be, LineEnding::Crlf);
    fs::write(&path, &original).unwrap();
    let (_, module) = module_path!().split_once("::").unwrap();
    let test =
        format!("{module}::killed_process_recovers_file_and_untitled_without_writing_the_source");
    let child_temp = root.join("child-temp");
    fs::create_dir(&child_temp).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test])
        .env(CHILD, root)
        .env("TEMP", &child_temp)
        .env("TMP", &child_temp)
        .env("TMPDIR", &child_temp)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut ready = false;
    while Instant::now() < deadline && matches!(child.try_wait(), Ok(None)) {
        if root.join("ready").is_file() {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let killed = child.kill();
    let status = child.wait().unwrap();
    assert!(ready, "the child did not write both drafts: {status}");
    killed.unwrap();
    assert!(!status.success());

    let profile = root.join("profile");
    let mut reopened = app(root, &profile);
    let items = reopened.draft_items();
    assert_eq!(items.len(), 2);
    let file = items
        .iter()
        .find(|item| store::read(&item.path).unwrap().original.is_some())
        .unwrap();
    let untitled = items.iter().find(|item| item.path != file.path).unwrap();
    reopened.resolve_draft(&file.path, true).unwrap();
    let id = reopened.active_document();
    assert_eq!(reopened.editor().path(), Some(path.as_path()));
    assert_eq!(reopened.editor().text(), EDIT);
    assert!(reopened.documents[id].recovery_pending_save);
    reopened.auto_save = true;
    reopened.autosave_tick();
    assert!(!reopened.autosave_deadlines.contains_key(&id));
    assert_eq!(fs::read(&path).unwrap(), original);
    reopened.save_document(id);
    reopened.wait_for_saves();
    drain(&mut reopened, Instant::now());
    assert!(!file.path.exists());
    assert_eq!(
        fs::read(&path).unwrap(),
        file_format::encode(EDIT, Encoding::Utf16Be, LineEnding::Crlf)
    );

    reopened.resolve_draft(&untitled.path, true).unwrap();
    assert!(reopened.editor().path().is_none());
    assert_eq!(reopened.editor().text(), "untitled draft");
    // A second process can still recover the adopted checkpoint if this window dies.
    drop(reopened);
    let mut reopened = app(root, &profile);
    reopened.resolve_draft(&untitled.path, true).unwrap();
    let destination = root.join("recovered.txt");
    reopened.save_active_document_as(&destination).unwrap();
    drain(&mut reopened, Instant::now());
    assert!(!untitled.path.exists());
    assert_eq!(fs::read_to_string(destination).unwrap(), "untitled draft");
}

#[test]
fn conflicts_restore_to_a_separate_tab_and_leave_the_source_unchanged() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    for conflict in ["changed", "missing", "readonly", "dirty", "unreadable"] {
        let profile = root.join(conflict);
        let path = root.join("document.txt");
        fs::write(&path, ORIGINAL).unwrap();
        let mut live = app(root, &profile);
        live.open_file(path.clone());
        insert(&mut live, EDIT);
        drain(&mut live, Instant::now());
        let draft = snapshot(&live, live.active_document());
        drop(live);
        let mut reopened = app(root, &profile);
        match conflict {
            "changed" => fs::write(&path, "external").unwrap(),
            "missing" => fs::remove_file(&path).unwrap(),
            "unreadable" => fs::write(&path, [0xFF]).unwrap(),
            "readonly" => reopened.read_only_patterns = vec!["document.txt".into()],
            "dirty" => {
                reopened.open_file(path.clone());
                insert(&mut reopened, "another edit");
            }
            _ => unreachable!(),
        }
        let disk = fs::read(&path).ok();
        reopened.resolve_draft(&draft, true).unwrap();
        assert!(reopened.editor().path().is_none(), "{conflict}");
        assert_eq!(reopened.editor().text(), EDIT, "{conflict}");
        assert_eq!(
            reopened.documents[reopened.active_document()]
                .recovery_label
                .as_deref(),
            Some("document.txt")
        );
        assert_eq!(fs::read(&path).ok(), disk, "{conflict}");
        if conflict == "dirty" {
            assert_eq!(
                reopened.documents[reopened.documents.id_for_path(&path).unwrap()]
                    .buffer
                    .text(),
                "another edit"
            );
        }
    }
}

#[test]
fn snapshots_are_local_to_the_original_profile_and_live_sessions_are_excluded() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let profiles = crate::app::phase5::Profiles::open(root).unwrap();
    profiles.create("Other").unwrap();
    let first = profiles.path("Default").unwrap();
    let second = profiles.path("Other").unwrap();
    fs::write(
        second.join("settings.json"),
        r#"{"files.draftRecovery": false}"#,
    )
    .unwrap();
    let mut live = app(root, &first);
    live.profiles = Some(profiles);
    let id = live.active_document();
    insert(&mut live, "first draft");
    let now = Instant::now();
    drain(&mut live, now);
    let draft = snapshot(&live, id);
    let mut other_window = app(root, &first);
    assert!(other_window.draft_items().is_empty());
    assert!(other_window.resolve_draft(&draft, true).is_err());

    live.switch_user_profile("Other").unwrap();
    insert(&mut live, "first draft after profile switch");
    live.new_untitled_document();
    insert(&mut live, "never captured");
    drain(&mut live, now + INTERVAL);
    assert_eq!(
        store::read(&draft).unwrap().text,
        "first draft after profile switch"
    );
    assert!(!store::root(&second).exists());
    drop(live);
    assert_eq!(other_window.draft_items().len(), 1);
    assert!(app(root, &second).draft_items().is_empty());
}

#[test]
fn save_with_newer_typing_keeps_the_new_draft_until_a_clean_save() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let path = root.join("document.txt");
    fs::write(&path, ORIGINAL).unwrap();
    let mut app = app(root, &root.join("profile"));
    app.open_file(path.clone());
    let id = app.active_document();
    insert(&mut app, "saving\n");
    let now = Instant::now();
    drain(&mut app, now);
    app.save_document(id);
    app.poll_saves(now);
    insert(&mut app, "newer\n");
    app.wait_for_saves();
    drain(&mut app, now + INTERVAL);
    let path_to_draft = snapshot(&app, id);
    let draft = store::read(&path_to_draft).unwrap();
    assert_eq!(draft.text, "newer\n");
    assert_eq!(draft.base, Some(file_format::fingerprint(b"saving\n")));
    app.save_document(id);
    let copy = root.join("save-as.txt");
    assert!(app.save_active_document_as(&copy).is_err());
    assert!(!copy.exists());
    app.wait_for_saves();
    drain(&mut app, now + INTERVAL);
    assert!(!path_to_draft.exists());
    assert_eq!(fs::read_to_string(path).unwrap(), "newer\n");
}

#[test]
fn review_ignores_corruption_and_requires_confirmation_to_discard() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let profile = root.join("profile");
    let mut live = app(root, &profile);
    insert(&mut live, EDIT);
    drain(&mut live, Instant::now());
    let draft = snapshot(&live, live.active_document());
    let corrupt = draft.with_file_name("corrupt.json");
    fs::write(&corrupt, b"broken").unwrap();
    drop(live);
    let mut reopened = app(root, &profile);
    reopened.run_ui_command("graphite.files.reviewDrafts");
    assert_eq!(reopened.quick_input_model().unwrap().rows.len(), 1);
    assert!(reopened.quick_input_accept_row(0));
    assert!(reopened.quick_input_accept_row(1));
    assert!(draft.exists());
    reopened.quick_input_dismiss();
    assert!(draft.exists());
    reopened.review_drafts();
    assert!(reopened.quick_input_accept_row(0));
    assert!(reopened.quick_input_accept_row(1));
    assert!(reopened.quick_input_accept_row(1));
    assert!(!draft.exists());
    assert!(corrupt.exists());
}

#[cfg(windows)]
#[test]
fn failed_checkpoint_replacement_preserves_previous_draft_and_retries() {
    use std::os::windows::fs::OpenOptionsExt;

    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let mut app = app(root, &root.join("profile"));
    let id = app.active_document();
    let now = Instant::now();
    insert(&mut app, "first draft");
    drain(&mut app, now);
    let draft = snapshot(&app, id);
    let in_use = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&draft)
        .unwrap();
    insert(&mut app, "later draft");
    drain(&mut app, now + INTERVAL);
    assert_eq!(store::read(&draft).unwrap().text, "first draft");
    drop(in_use);
    drain(&mut app, now + INTERVAL + RETRY);
    assert_eq!(store::read(&draft).unwrap().text, "later draft");
}
