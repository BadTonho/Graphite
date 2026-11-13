use super::go_menu::GoPickerKind;
use super::*;
use crate::notebook::kernel::{decode_for_tests, Command, KernelEvent, KernelHandle, KernelSpec, KernelState, Notify};
use crate::notebook::{KernelStatus, Mode, RunState};
use serde_json::json;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver, Sender};

const SAMPLE: &str = r##"{
 "cells": [
  {"cell_type": "markdown", "id": "m1", "metadata": {}, "source": ["# Title"]},
  {"cell_type": "code", "id": "c1", "metadata": {}, "execution_count": 1,
   "outputs": [{"output_type": "stream", "name": "stdout", "text": ["old\n"]}], "source": ["print('hi')"]},
  {"cell_type": "code", "id": "c2", "metadata": {}, "execution_count": null, "outputs": [], "source": ["x = 1"]}
 ],
 "metadata": {"kernelspec": {"name": "python3", "display_name": "Python 3", "language": "python"}},
 "nbformat": 4,
 "nbformat_minor": 5
}"##;

/// The test's end of a kernel the application started.
struct Scripted {
    commands: Receiver<Command>,
    events: Sender<KernelEvent>,
}

thread_local! {
    static STARTED: RefCell<Vec<Scripted>> = const { RefCell::new(Vec::new()) };
    static FAIL: RefCell<Option<String>> = const { RefCell::new(None) };
    static SPECS: RefCell<Vec<KernelSpec>> = const { RefCell::new(Vec::new()) };
}

fn factory(_: &KernelSpec, _: &Path, _: Notify) -> Result<KernelHandle, String> {
    if let Some(error) = FAIL.with(|fail| fail.borrow().clone()) {
        return Err(error);
    }
    let (command_tx, commands) = channel();
    let (events, event_rx) = channel();
    STARTED.with(|started| started.borrow_mut().push(Scripted { commands, events }));
    Ok(KernelHandle::from_parts(command_tx, event_rx, "key", true))
}

fn lister() -> Vec<KernelSpec> {
    SPECS.with(|specs| specs.borrow().clone())
}

/// Points the application at the scripted kernel and the given installed kernels.
fn configure(app: &mut App, specs: Vec<KernelSpec>) {
    STARTED.with(|started| started.borrow_mut().clear());
    FAIL.with(|fail| *fail.borrow_mut() = None);
    SPECS.with(|slot| *slot.borrow_mut() = specs);
    app.kernel_factory = factory;
    app.kernel_lister = lister;
}

fn spec(name: &str, language: &str) -> KernelSpec {
    KernelSpec {
        name: name.into(),
        display_name: format!("{name} display"),
        language: language.into(),
        argv: vec!["x".into()],
        env: Default::default(),
        interrupt_mode: "signal".into(),
    }
}

fn open(specs: Vec<KernelSpec>) -> (crate::test_support::TempDir, App) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("run.ipynb");
    std::fs::write(&path, SAMPLE).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    configure(&mut app, specs);
    app.open_file(path);
    (dir, app)
}

fn session(app: &App) -> &crate::notebook::Session {
    app.active_notebook().expect("a notebook")
}

fn started() -> usize {
    STARTED.with(|started| started.borrow().len())
}

/// The execute requests that reached the kernel number `index`, as (id, code).
fn requests(index: usize) -> Vec<(String, String)> {
    STARTED.with(|started| {
        let started = started.borrow();
        let mut found = Vec::new();
        while let Ok(command) = started[index].commands.try_recv() {
            if let Command::Send { frames, .. } = command {
                let message = decode_for_tests(b"key", &frames);
                if message.msg_type() == "execute_request" {
                    found.push((message.id().to_owned(), message.content["code"].as_str().unwrap().to_owned()));
                }
            }
        }
        found
    })
}

fn send(index: usize, event: KernelEvent) {
    STARTED.with(|started| started.borrow()[index].events.send(event).unwrap());
}

fn buffer_text(app: &App) -> String {
    app.documents[app.active_document()].buffer.text()
}

#[test]
fn running_a_cell_starts_the_kernel_the_notebook_names_and_sends_the_code() {
    let (_dir, mut app) = open(vec![spec("julia", "julia"), spec("python3", "python")]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.execute");
    assert_eq!(started(), 1);
    assert_eq!(app.status, app.tf("notebook.running", &[("count", &1usize)]));
    let sent = requests(0);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "print('hi')");
    assert_eq!(session(&app).kernel_status(), &KernelStatus::Starting);
    assert_eq!(session(&app).kernel_spec().unwrap().name, "python3");
    let uid = session(&app).cells[1].uid;
    assert_eq!(session(&app).run_state(uid), Some(RunState::Queued));
}

#[test]
fn kernel_events_become_outputs_and_reach_the_document_text() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.execute");
    let id = requests(0).remove(0).0;
    send(0, KernelEvent::Status { state: KernelState::Busy, parent: Some(id.clone()) });
    send(0, KernelEvent::ExecuteInput { parent: id.clone(), count: 7 });
    send(
        0,
        KernelEvent::Output {
            parent: id.clone(),
            output: json!({"output_type": "stream", "name": "stdout", "text": "hi there\n"}),
        },
    );
    send(0, KernelEvent::Reply { parent: id.clone(), ok: true, count: Some(7) });
    send(0, KernelEvent::Status { state: KernelState::Idle, parent: Some(id) });
    app.poll_kernels();
    let cell = &session(&app).cells[1].cell;
    assert_eq!(cell.execution_count, Some(7));
    assert_eq!(cell.outputs.len(), 1);
    assert_eq!(cell.outputs[0]["text"], "hi there\n");
    assert_eq!(session(&app).kernel_status(), &KernelStatus::Idle);
    // Outputs are part of the saved file, so the document is modified and carries them.
    let document = app.active_document();
    assert!(app.documents[document].is_dirty());
    assert!(buffer_text(&app).contains("hi there"));
    assert!(buffer_text(&app).contains("\"execution_count\": 7"));
    assert!(!buffer_text(&app).contains("old\\n"));
}

#[test]
fn the_notebook_remembers_its_kernel_in_its_metadata() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("plain.ipynb");
    std::fs::write(
        &path,
        r#"{"nbformat":4,"nbformat_minor":5,"metadata":{},"cells":[{"cell_type":"code","id":"a","metadata":{},"source":"1","outputs":[],"execution_count":null}]}"#,
    )
    .unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    configure(&mut app, vec![spec("only", "python")]);
    app.open_file(path);
    // A single installed kernel is chosen without asking.
    app.run_ui_command("notebook.cell.execute");
    assert_eq!(started(), 1);
    assert_eq!(session(&app).metadata().kernel_name(), Some("only"));
    let text = buffer_text(&app);
    assert!(text.contains("\"name\": \"only\""), "{text}");
    assert!(text.contains("\"display_name\": \"only display\""));
}

#[test]
fn without_kernels_nothing_runs_and_the_user_is_told() {
    let (_dir, mut app) = open(Vec::new());
    app.run_ui_command("notebook.cell.execute");
    assert_eq!(started(), 0);
    assert_eq!(app.status, app.t("notebook.error.no_kernels"));
}

#[test]
fn an_ambiguous_choice_opens_the_kernel_picker_instead_of_guessing() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("amb.ipynb");
    std::fs::write(
        &path,
        r#"{"nbformat":4,"nbformat_minor":5,"metadata":{"kernelspec":{"name":"gone","language":"python"}},"cells":[{"cell_type":"code","id":"a","metadata":{},"source":"1","outputs":[],"execution_count":null}]}"#,
    )
    .unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    configure(&mut app, vec![spec("py-a", "python"), spec("py-b", "python")]);
    app.open_file(path);
    app.run_ui_command("notebook.cell.execute");
    assert_eq!(started(), 0);
    let picker = app.go_picker.as_ref().expect("the kernel picker");
    assert_eq!(picker.kind, GoPickerKind::Kernel);
    assert_eq!(picker.items.len(), 2);
    assert_eq!(app.status, app.t("notebook.select_kernel"));
    // Picking the second one starts it.
    app.go_picker.as_mut().unwrap().selected = 1;
    app.accept_go_picker();
    assert_eq!(started(), 1);
    assert_eq!(session(&app).kernel_spec().unwrap().name, "py-b");
    assert!(app.status.contains("py-b display"));
}

#[test]
fn a_kernel_that_cannot_start_reports_why_and_runs_nothing() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    FAIL.with(|fail| *fail.borrow_mut() = Some("no such program".into()));
    app.run_ui_command("notebook.cell.execute");
    assert!(app.status.contains("no such program"));
    assert_eq!(session(&app).kernel_status(), &KernelStatus::None);
    // Once it can start, the same command works.
    FAIL.with(|fail| *fail.borrow_mut() = None);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.execute");
    assert_eq!(started(), 1);
}

#[test]
fn run_all_above_and_below_cover_the_right_cells() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.execute");
    let codes: Vec<_> = requests(0).into_iter().map(|(_, code)| code).collect();
    // The markdown cell is not code.
    assert_eq!(codes, ["print('hi')", "x = 1"]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.executeCellsAbove");
    assert_eq!(requests(0).len(), 1);
    app.run_ui_command("notebook.focusPreviousEditor");
    app.run_ui_command("notebook.cell.executeCellAndBelow");
    assert_eq!(requests(0).len(), 2);
    // One kernel served all of it.
    assert_eq!(started(), 1);
}

#[test]
fn run_and_advance_selects_the_next_cell_or_adds_one() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.executeAndSelectBelow");
    assert_eq!(session(&app).focus, 2);
    assert_eq!(session(&app).cells.len(), 3);
    // On the last cell a new one is added and edited.
    app.run_ui_command("notebook.cell.executeAndSelectBelow");
    assert_eq!(session(&app).cells.len(), 4);
    assert_eq!(session(&app).focus, 3);
    assert_eq!(session(&app).mode, Mode::Edit);
    app.run_ui_command("notebook.cell.quitEdit");
    app.with_notebook(|session| session.set_focus(0, false));
    app.run_ui_command("notebook.cell.executeAndInsertBelow");
    assert_eq!(session(&app).cells.len(), 5);
    assert_eq!(session(&app).focus, 1);
}

#[test]
fn interrupting_needs_something_running() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.cancelExecution");
    assert_eq!(app.status, app.t("notebook.nothing_running"));
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.execute");
    requests(0);
    app.run_ui_command("notebook.cancelExecution");
    assert_eq!(app.status, app.t("notebook.interrupted"));
    let mut control = 0;
    STARTED.with(|started| {
        while let Ok(command) = started.borrow()[0].commands.try_recv() {
            if matches!(command, Command::Send { control: true, .. }) {
                control += 1;
            }
        }
    });
    assert_eq!(control, 1);
}

#[test]
fn restarting_replaces_the_kernel_and_forgets_running_cells() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.execute");
    app.run_ui_command("graphite.notebook.restartKernel");
    assert_eq!(started(), 2);
    assert!(!session(&app).is_running());
    assert_eq!(app.status, app.t("notebook.kernel_restarted"));
    // The first kernel was told to end.
    let ended = STARTED.with(|started| {
        let started = started.borrow();
        let mut ended = false;
        while let Ok(command) = started[0].commands.try_recv() {
            ended |= matches!(command, Command::Kill);
        }
        ended
    });
    assert!(ended);
    app.run_ui_command("graphite.notebook.restartKernelAndRunAll");
    assert_eq!(started(), 3);
    assert_eq!(requests(2).len(), 2);
}

#[test]
fn restarting_without_a_kernel_is_refused() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("graphite.notebook.restartKernel");
    assert_eq!(app.status, app.t("notebook.error.no_kernel"));
    assert_eq!(started(), 0);
}

#[test]
fn shutting_down_stops_the_kernel_and_the_next_run_starts_a_new_one() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.execute");
    app.run_ui_command("graphite.notebook.shutdownKernel");
    let asked = STARTED.with(|started| {
        let started = started.borrow();
        let mut asked = false;
        while let Ok(command) = started[0].commands.try_recv() {
            if let Command::Send { control: true, frames } = command {
                asked |= decode_for_tests(b"key", &frames).msg_type() == "shutdown_request";
            }
        }
        asked
    });
    assert!(asked);
    send(0, KernelEvent::Exited { details: String::new() });
    app.poll_kernels();
    assert_eq!(session(&app).kernel_status(), &KernelStatus::Dead(String::new()));
    app.run_ui_command("notebook.cell.execute");
    assert_eq!(started(), 2);
    assert_eq!(requests(1).len(), 1);
}

#[test]
fn outputs_can_be_cleared_for_the_selection_or_everywhere() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.focusNextEditor");
    assert_eq!(session(&app).cells[1].cell.outputs.len(), 1);
    app.run_ui_command("notebook.cell.clearOutputs");
    assert!(session(&app).cells[1].cell.outputs.is_empty());
    assert_eq!(session(&app).cells[1].cell.execution_count, None);
    assert!(!buffer_text(&app).contains("old\\n"));
    app.run_ui_command("notebook.clearAllCellsOutputs");
    assert!(session(&app).cells.iter().all(|state| state.cell.outputs.is_empty()));
}

#[test]
fn closing_the_notebook_ends_its_kernel() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    app.run_ui_command("notebook.cell.execute");
    let document = app.active_document();
    app.documents[document].notebook = None;
    let ended = STARTED.with(|started| {
        let started = started.borrow();
        let mut ended = false;
        while let Ok(command) = started[0].commands.try_recv() {
            ended |= matches!(command, Command::Kill);
        }
        ended
    });
    assert!(ended);
}

#[test]
fn the_kernel_label_names_the_kernel_and_its_state() {
    let (_dir, mut app) = open(vec![spec("python3", "python")]);
    let label = app.notebook_kernel_label(session(&app));
    assert!(label.starts_with("python3"), "{label}");
    assert!(label.contains(app.t("notebook.kernel.not_started")));
    app.run_ui_command("notebook.cell.execute");
    send(0, KernelEvent::Status { state: KernelState::Idle, parent: None });
    app.poll_kernels();
    let label = app.notebook_kernel_label(session(&app));
    assert!(label.starts_with("python3 display"), "{label}");
    assert!(label.contains(app.t("notebook.kernel.idle")));
}

#[test]
fn run_commands_are_disabled_outside_a_notebook() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.run_ui_command("notebook.cell.execute");
    assert!(app.status.contains("notebook.cell.execute"));
}
