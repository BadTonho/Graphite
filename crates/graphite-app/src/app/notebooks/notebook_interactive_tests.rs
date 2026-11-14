use super::notebook_kernel_tests::{configure, requests, send, spec, started};
use super::*;
use crate::notebook::kernel::{KernelEvent, KernelState};
use crate::notebook::{InteractiveKind, KernelStatus, Session};
use serde_json::json;

fn app_with(
    specs: Vec<crate::notebook::kernel::KernelSpec>,
) -> (crate::test_support::TempDir, App) {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    configure(&mut app, specs);
    (dir, app)
}

fn session(app: &App) -> &Session {
    app.active_notebook().expect("a notebook")
}

fn interactive(app: &App) -> &crate::notebook::Interactive {
    session(app).interactive().expect("an interactive window")
}

fn type_input(app: &mut App, text: &str) {
    app.with_notebook(|session| session.set_input_text(text));
}

#[test]
fn opening_gives_an_interactive_tab_that_is_never_modified() {
    let (_dir, mut app) = app_with(vec![spec("python3", "python")]);
    app.run_ui_command("interactive.open");
    assert_eq!(app.interactive_context(), (true, true));
    assert_eq!(interactive(&app).kind, InteractiveKind::Window);
    let document = app.active_document();
    assert!(!app.documents[document].is_dirty());
    let label = super::group_drawing::tab_label(&app.documents[document], &app.translator);
    assert!(
        label.contains(app.t("notebook.interactive.title")),
        "{label}"
    );
    app.run_ui_command("graphite.repl.open");
    let label =
        super::group_drawing::tab_label(&app.documents[app.active_document()], &app.translator);
    assert!(label.contains(app.t("notebook.repl.title")), "{label}");
    assert_eq!(interactive(&app).kind, InteractiveKind::Repl);
}

#[test]
fn running_the_input_starts_a_kernel_adds_a_cell_and_shows_the_outputs() {
    let (_dir, mut app) = app_with(vec![spec("python3", "python")]);
    app.run_ui_command("interactive.open");
    type_input(&mut app, "1 + 1");
    app.run_ui_command("interactive.execute");
    assert_eq!(started(), 1);
    let sent = requests(0);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "1 + 1");
    assert_eq!(interactive(&app).input_text(), "");
    assert_eq!(session(&app).cells.len(), 1);
    assert_eq!(session(&app).cells[0].text(), "1 + 1");
    let id = sent[0].0.clone();
    send(
        0,
        KernelEvent::ExecuteInput {
            parent: id.clone(),
            count: 1,
        },
    );
    send(
        0,
        KernelEvent::Output {
            parent: id.clone(),
            output: json!({"output_type": "execute_result", "execution_count": 1, "data": {"text/plain": "2"}, "metadata": {}}),
        },
    );
    send(
        0,
        KernelEvent::Reply {
            parent: id.clone(),
            ok: true,
            count: Some(1),
        },
    );
    send(
        0,
        KernelEvent::Status {
            state: KernelState::Idle,
            parent: Some(id),
        },
    );
    app.poll_kernels();
    assert_eq!(session(&app).cells[0].cell.outputs.len(), 1);
    assert_eq!(session(&app).kernel_status(), &KernelStatus::Idle);
    // Running more adds cells in order.
    type_input(&mut app, "x = 3");
    app.run_ui_command("interactive.execute");
    assert_eq!(started(), 1);
    assert_eq!(session(&app).cells.len(), 2);
    assert_eq!(session(&app).focus, 1);
    assert!(!app.documents[app.active_document()].is_dirty());
}

#[test]
fn a_blank_input_runs_nothing_and_a_missing_kernel_keeps_what_was_typed() {
    let (_dir, mut app) = app_with(Vec::new());
    app.run_ui_command("interactive.open");
    app.run_ui_command("interactive.execute");
    assert_eq!(started(), 0);
    type_input(&mut app, "keep me");
    app.run_ui_command("interactive.execute");
    assert_eq!(app.status, app.t("notebook.error.no_kernels"));
    assert_eq!(interactive(&app).input_text(), "keep me");
    assert!(session(&app).is_hidden_cell(0));
    assert!(interactive(&app).history.is_empty());
}

#[test]
fn the_repl_runs_on_its_own_command_and_keeps_a_history() {
    let (_dir, mut app) = app_with(vec![spec("python3", "python")]);
    app.run_ui_command("graphite.repl.open");
    for code in ["a = 1", "b = 2"] {
        type_input(&mut app, code);
        app.run_ui_command("repl.execute");
    }
    assert_eq!(requests(0).len(), 2);
    type_input(&mut app, "draft");
    app.run_ui_command("interactive.history.previous");
    assert_eq!(interactive(&app).input_text(), "b = 2");
    app.run_ui_command("interactive.history.previous");
    assert_eq!(interactive(&app).input_text(), "a = 1");
    app.run_ui_command("interactive.history.next");
    app.run_ui_command("interactive.history.next");
    assert_eq!(interactive(&app).input_text(), "draft");
}

#[test]
fn focus_moves_between_the_input_and_the_history_and_commands_follow_it() {
    let (_dir, mut app) = app_with(vec![spec("python3", "python")]);
    app.run_ui_command("interactive.open");
    type_input(&mut app, "one");
    app.run_ui_command("interactive.execute");
    type_input(&mut app, "two");
    app.run_ui_command("interactive.execute");
    app.run_ui_command("interactive.history.focus");
    assert_eq!(app.interactive_context(), (true, false));
    // The input commands need the input to have the keyboard.
    app.run_ui_command("interactive.execute");
    assert!(app.status.contains("interactive.execute"));
    app.run_ui_command("notebook.focusPreviousEditor");
    assert_eq!(session(&app).focus, 0);
    app.run_ui_command("repl.focusLastItemExecuted");
    assert_eq!(session(&app).focus, 1);
    assert_eq!(app.interactive_context(), (true, false));
    app.run_ui_command("interactive.input.focus");
    assert_eq!(app.interactive_context(), (true, true));
    type_input(&mut app, "typed");
    app.run_ui_command("interactive.input.clear");
    assert_eq!(interactive(&app).input_text(), "");
    app.run_ui_command("interactive.scrollToTop");
    assert_eq!(session(&app).scroll, 0.0);
    app.run_ui_command("interactive.scrollToBottom");
}

#[test]
fn a_selection_or_line_from_an_editor_goes_to_one_window_and_picks_the_language_kernel() {
    let (dir, mut app) = app_with(vec![spec("julia", "julia"), spec("python3", "python")]);
    let path = dir.path().join("script.py");
    std::fs::write(&path, "first = 1\nsecond = first + 1\nthird = 3\n").unwrap();
    app.open_file(path.clone());
    app.editor_mut().set_cursor_line_col(1, 0);
    app.run_ui_command("graphite.interactive.executeSelection");
    // The Python file chose the Python kernel without asking.
    assert_eq!(started(), 1);
    let sent = requests(0);
    assert_eq!(sent[0].1, "second = first + 1");
    assert!(app.interactive_context().0);
    assert_eq!(session(&app).kernel_spec().unwrap().name, "python3");
    // From the editor again: the same window takes it.
    let window = app.active_document();
    app.activate_editor_document(app.documents.id_for_path(&path).unwrap());
    app.editor_mut().set_selection(0, 9);
    app.run_ui_command("graphite.interactive.executeSelection");
    assert_eq!(app.active_document(), window);
    assert_eq!(requests(0)[0].1, "first = 1");
    assert_eq!(session(&app).cells.len(), 2);
    assert_eq!(started(), 1);
}

#[test]
fn sending_an_empty_line_is_refused() {
    let (dir, mut app) = app_with(vec![spec("python3", "python")]);
    let path = dir.path().join("blank.py");
    std::fs::write(&path, "\n\nx = 1\n").unwrap();
    app.open_file(path);
    app.run_ui_command("graphite.interactive.executeSelection");
    assert_eq!(app.status, app.t("notebook.interactive.nothing_to_send"));
    assert!(!app.interactive_context().0);
}

#[test]
fn interactive_commands_are_disabled_in_other_editors() {
    let (_dir, mut app) = app_with(Vec::new());
    app.run_ui_command("interactive.input.clear");
    assert!(app.status.contains("interactive.input.clear"));
    app.run_ui_command("interactive.history.previous");
    assert!(app.status.contains("interactive.history.previous"));
}

#[test]
fn an_interactive_window_can_be_saved_as_a_notebook() {
    let (dir, mut app) = app_with(vec![spec("python3", "python")]);
    app.run_ui_command("interactive.open");
    type_input(&mut app, "answer = 42");
    app.run_ui_command("interactive.execute");
    let document = app.active_document();
    let json = app.documents[document].buffer.text();
    let parsed = crate::notebook::Notebook::parse(&json).unwrap();
    assert_eq!(parsed.cells.len(), 1);
    assert_eq!(parsed.cells[0].source, "answer = 42");
    // The text is exactly a notebook file, so writing it is saving the history.
    let path = dir.path().join("history.ipynb");
    std::fs::write(&path, json).unwrap();
    app.open_file(path);
    assert_eq!(session(&app).cells[0].text(), "answer = 42");
    assert!(session(&app).interactive().is_none());
}
