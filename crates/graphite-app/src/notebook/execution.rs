//! Running cells: the kernel a notebook is attached to, the requests in flight and how the
//! kernel's events become cell outputs.

use super::kernel::{KernelEvent, KernelHandle, KernelSpec, KernelState};
use super::model::CellKind;
use super::session::Session;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// What the notebook shows about its kernel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum KernelStatus {
    /// No kernel is attached.
    #[default]
    None,
    Starting,
    Idle,
    Busy,
    /// The kernel ended; the text is what it printed, when it printed anything.
    Dead(String),
}

/// Where a cell is in a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunState {
    /// Sent to the kernel, waiting for its turn.
    Queued,
    Running,
}

/// The kernel of a notebook with everything tied to it.
#[derive(Default)]
pub(crate) struct KernelSlot {
    pub handle: Option<KernelHandle>,
    pub spec: Option<KernelSpec>,
    pub status: KernelStatus,
    pub language: Option<String>,
    /// Request id to the uid of the cell it runs.
    requests: HashMap<String, u64>,
    pub(super) run_states: HashMap<u64, RunState>,
    /// Cells whose outputs are cleared when the next output arrives.
    clear_pending: HashSet<u64>,
    /// Display id to the cell that shows it.
    displays: HashMap<String, u64>,
    /// Requests whose reply, and whose idle status, arrived. The two come on different
    /// channels in either order; a request is over only when both are in, so the outputs that
    /// trail the reply are not lost.
    replied: HashSet<String>,
    idled: HashSet<String>,
}

impl KernelSlot {
    /// Forgets the requests in flight, for when the cells they refer to are gone.
    pub(crate) fn forget_requests(&mut self) {
        self.requests.clear();
        self.run_states.clear();
        self.clear_pending.clear();
        self.displays.clear();
        self.replied.clear();
        self.idled.clear();
    }

    /// Ends the request when both its reply and its idle status have arrived. Returns whether it
    /// ended.
    fn try_finish(&mut self, request: &str) -> bool {
        if !(self.replied.contains(request) && self.idled.contains(request)) {
            return false;
        }
        self.replied.remove(request);
        self.idled.remove(request);
        if let Some(uid) = self.requests.remove(request) {
            self.run_states.remove(&uid);
        }
        true
    }
}

/// Appends `output` to `outputs`, joining it to the last one when both are text of the same
/// stream.
fn push_output(outputs: &mut Vec<Value>, output: Value) {
    if output["output_type"] == "stream" {
        if let Some(last) = outputs.last_mut() {
            if last["output_type"] == "stream" && last["name"] == output["name"] {
                let joined = format!(
                    "{}{}",
                    super::output::text_of(&last["text"]).unwrap_or_default(),
                    output["text"].as_str().unwrap_or("")
                );
                last["text"] = json!(joined);
                return;
            }
        }
    }
    outputs.push(output);
}

impl Session {
    pub(crate) fn kernel_status(&self) -> &KernelStatus {
        &self.kernel.status
    }

    /// The kernel the notebook was last attached to, whether or not it is still running.
    pub(crate) fn kernel_spec(&self) -> Option<&KernelSpec> {
        self.kernel.spec.as_ref()
    }

    /// Asks the kernel to exit. Its end is reported as an event, after which the notebook
    /// starts a new one when something is run.
    pub(crate) fn shutdown_kernel(&mut self) {
        if let Some(handle) = self.kernel.handle.as_ref() {
            handle.shutdown();
        }
        self.kernel.forget_requests();
    }

    pub(crate) fn run_state(&self, uid: u64) -> Option<RunState> {
        self.kernel.run_states.get(&uid).copied()
    }

    /// Whether any cell is waiting for or running on the kernel.
    pub(crate) fn is_running(&self) -> bool {
        !self.kernel.run_states.is_empty()
    }

    /// Attaches a started kernel, replacing (and so ending) the previous one.
    pub(crate) fn attach_kernel(&mut self, handle: KernelHandle, spec: KernelSpec) {
        self.kernel.forget_requests();
        self.kernel.handle = Some(handle);
        self.kernel.spec = Some(spec);
        self.kernel.status = KernelStatus::Starting;
        self.kernel.language = None;
    }

    /// Detaches the kernel, which ends it.
    pub(crate) fn detach_kernel(&mut self) {
        self.kernel.forget_requests();
        self.kernel.handle = None;
        self.kernel.status = KernelStatus::None;
    }

    /// Sends the code cells among `indices` to the kernel, in order. Empty cells are skipped.
    /// Returns how many were sent.
    pub(crate) fn run_cells(&mut self, indices: &[usize]) -> Result<usize, String> {
        let Some(handle) = self.kernel.handle.as_ref() else {
            return Err("no kernel".into());
        };
        let mut sent = 0;
        for &index in indices {
            let Some(state) = self.cells.get(index) else {
                continue;
            };
            let source = state.text();
            if state.cell.kind != CellKind::Code || source.trim().is_empty() {
                continue;
            }
            let id = handle.execute(&source);
            self.kernel.requests.insert(id, state.uid);
            self.kernel.run_states.insert(state.uid, RunState::Queued);
            sent += 1;
        }
        Ok(sent)
    }

    /// Interrupts what the kernel is running.
    pub(crate) fn interrupt_kernel(&self) {
        if let Some(handle) = self.kernel.handle.as_ref() {
            handle.interrupt();
        }
    }

    /// Removes the outputs and execution count of the cells among `indices`.
    pub(crate) fn clear_outputs(&mut self, indices: &[usize]) {
        for &index in indices {
            if let Some(state) = self.cells.get_mut(index) {
                state.cell.outputs.clear();
                state.touch_outputs();
                state.cell.execution_count = None;
            }
        }
    }

    /// Takes the events the kernel produced and applies them. Returns whether the notebook
    /// changed in a way that has to be saved or redrawn.
    pub(crate) fn poll_kernel(&mut self) -> bool {
        let events = match self.kernel.handle.as_ref() {
            Some(handle) => handle.drain(),
            None => return false,
        };
        let mut changed = false;
        for event in events {
            changed |= self.apply_event(event);
        }
        changed
    }

    /// Back to idle once nothing of ours is running.
    fn settle_status(&mut self) {
        if self.kernel.run_states.is_empty() && self.kernel.status == KernelStatus::Busy {
            self.kernel.status = KernelStatus::Idle;
        }
    }

    /// The uid of the cell a request of this notebook's kernel runs.
    pub(crate) fn request_cell(&self, request: &str) -> Option<u64> {
        self.kernel.requests.get(request).copied()
    }

    fn cell_of(&self, request: &str) -> Option<usize> {
        let uid = *self.kernel.requests.get(request)?;
        self.cells.iter().position(|state| state.uid == uid)
    }

    /// Applies one kernel event. Returns whether something visible changed.
    pub(crate) fn apply_event(&mut self, event: KernelEvent) -> bool {
        match event {
            KernelEvent::Status { state, parent } => {
                // An idle status that names one of our requests also says that request is over
                // on the output channel.
                if let (KernelState::Idle, Some(parent)) = (state, parent.as_ref()) {
                    if self.kernel.requests.contains_key(parent) {
                        self.kernel.idled.insert(parent.clone());
                        self.kernel.try_finish(parent);
                    }
                }
                self.kernel.status = match state {
                    KernelState::Starting => KernelStatus::Starting,
                    KernelState::Busy => KernelStatus::Busy,
                    // Idle only once nothing of ours is left to run.
                    KernelState::Idle if self.kernel.run_states.is_empty() => KernelStatus::Idle,
                    KernelState::Idle => KernelStatus::Busy,
                    KernelState::Dead => KernelStatus::Dead(String::new()),
                };
                true
            }
            KernelEvent::ExecuteInput { parent, count } => {
                let Some(index) = self.cell_of(&parent) else {
                    return false;
                };
                let uid = self.cells[index].uid;
                self.kernel.run_states.insert(uid, RunState::Running);
                self.kernel.displays.retain(|_, owner| *owner != uid);
                self.kernel.clear_pending.remove(&uid);
                let state = &mut self.cells[index];
                state.cell.execution_count = Some(count);
                state.cell.outputs.clear();
                state.touch_outputs();
                true
            }
            KernelEvent::Output { parent, mut output } => {
                let Some(index) = self.cell_of(&parent) else {
                    return false;
                };
                let uid = self.cells[index].uid;
                // The display id only serves updates; it is not part of the notebook.
                let display_id = output
                    .as_object_mut()
                    .and_then(|object| object.remove("transient"))
                    .and_then(|transient| transient["display_id"].as_str().map(str::to_owned));
                if let Some(id) = display_id {
                    self.kernel.displays.insert(id, uid);
                }
                if self.kernel.clear_pending.remove(&uid) {
                    self.cells[index].cell.outputs.clear();
                    self.cells[index].touch_outputs();
                }
                push_output(&mut self.cells[index].cell.outputs, output);
                self.cells[index].touch_outputs();
                true
            }
            KernelEvent::UpdateDisplay {
                display_id,
                mut output,
                ..
            } => {
                let Some(&uid) = self.kernel.displays.get(&display_id) else {
                    return false;
                };
                let Some(state) = self.cells.iter_mut().find(|state| state.uid == uid) else {
                    return false;
                };
                // Replace the last display output of the cell: the one that was shown for the id.
                if let Some(object) = output.as_object_mut() {
                    object.remove("transient");
                }
                match state
                    .cell
                    .outputs
                    .iter_mut()
                    .rev()
                    .find(|existing| existing["output_type"] == "display_data")
                {
                    Some(existing) => *existing = output,
                    None => state.cell.outputs.push(output),
                }
                state.touch_outputs();
                true
            }
            KernelEvent::ClearOutput { parent, wait } => {
                let Some(index) = self.cell_of(&parent) else {
                    return false;
                };
                let uid = self.cells[index].uid;
                if wait {
                    self.kernel.clear_pending.insert(uid);
                } else {
                    self.cells[index].cell.outputs.clear();
                    self.cells[index].touch_outputs();
                }
                true
            }
            KernelEvent::Reply { parent, count, .. } => {
                let Some(&uid) = self.kernel.requests.get(&parent) else {
                    return false;
                };
                if let Some(state) = self.cells.iter_mut().find(|state| state.uid == uid) {
                    if state.cell.execution_count.is_none() {
                        state.cell.execution_count = count;
                    }
                }
                self.kernel.replied.insert(parent.clone());
                self.kernel.try_finish(&parent);
                self.settle_status();
                true
            }
            KernelEvent::Info { language, .. } => {
                self.kernel.language = language;
                false
            }
            KernelEvent::Exited { details } => {
                self.kernel.handle = None;
                self.kernel.forget_requests();
                self.kernel.status = KernelStatus::Dead(details.trim().to_owned());
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::kernel::Command;
    use super::super::model::{Cell, Notebook};
    use super::*;
    use std::sync::mpsc::{channel, Receiver, Sender};

    struct Scripted {
        commands: Receiver<Command>,
        events: Sender<KernelEvent>,
    }

    fn spec() -> KernelSpec {
        KernelSpec {
            name: "python3".into(),
            display_name: "Python 3".into(),
            language: "python".into(),
            argv: vec!["python".into()],
            env: Default::default(),
            interrupt_mode: "signal".into(),
        }
    }

    fn running(sources: &[&str]) -> (Session, Scripted) {
        let mut notebook = Notebook::new();
        notebook.cells = sources
            .iter()
            .map(|source| Cell::new(CellKind::Code, source))
            .collect();
        let mut session = Session::from_notebook(notebook);
        let (command_tx, commands) = channel();
        let (events, event_rx) = channel();
        session.attach_kernel(
            KernelHandle::from_parts(command_tx, event_rx, "key", true),
            spec(),
        );
        (session, Scripted { commands, events })
    }

    /// The request ids sent so far, in order.
    fn sent_ids(scripted: &Scripted) -> Vec<String> {
        let mut ids = Vec::new();
        while let Ok(Command::Send { frames, .. }) = scripted.commands.try_recv() {
            let message = super::super::kernel::decode_for_tests(b"key", &frames);
            if message.msg_type() == "execute_request" {
                ids.push(message.id().to_owned());
            }
        }
        ids
    }

    fn feed(session: &mut Session, scripted: &Scripted, event: KernelEvent) {
        scripted.events.send(event).unwrap();
        session.poll_kernel();
    }

    fn input(parent: &str, count: u64) -> KernelEvent {
        KernelEvent::ExecuteInput {
            parent: parent.into(),
            count,
        }
    }

    fn stream(parent: &str, name: &str, text: &str) -> KernelEvent {
        KernelEvent::Output {
            parent: parent.into(),
            output: json!({"output_type": "stream", "name": name, "text": text}),
        }
    }

    fn reply(parent: &str, ok: bool, count: u64) -> KernelEvent {
        KernelEvent::Reply {
            parent: parent.into(),
            ok,
            count: Some(count),
        }
    }

    /// The end of a request: its reply on the shell channel and its idle status on IOPub.
    fn finish_run(session: &mut Session, scripted: &Scripted, parent: &str, ok: bool, count: u64) {
        feed(session, scripted, reply(parent, ok, count));
        feed(
            session,
            scripted,
            KernelEvent::Status {
                state: KernelState::Idle,
                parent: Some(parent.into()),
            },
        );
    }

    #[test]
    fn running_cells_sends_code_cells_in_order_and_skips_the_rest() {
        let (mut session, scripted) = running(&["a = 1", "   ", "b = 2"]);
        session.insert_cell(3, CellKind::Markdown, "# text");
        let sent = session.run_cells(&[0, 1, 2, 3, 99]).unwrap();
        assert_eq!(sent, 2);
        assert_eq!(sent_ids(&scripted).len(), 2);
        let first = session.cells[0].uid;
        assert_eq!(session.run_state(first), Some(RunState::Queued));
        assert!(session.is_running());
        assert_eq!(session.run_state(session.cells[1].uid), None);
    }

    #[test]
    fn without_a_kernel_nothing_is_sent() {
        let mut session = Session::from_notebook(Notebook::new());
        assert!(session.run_cells(&[0]).is_err());
        assert!(!session.poll_kernel());
        session.interrupt_kernel();
        assert_eq!(session.kernel_status(), &KernelStatus::None);
    }

    #[test]
    fn a_run_fills_in_the_count_the_outputs_and_the_status() {
        let (mut session, scripted) = running(&["print('hi')"]);
        session.run_cells(&[0]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        session.cells[0].cell.outputs =
            vec![json!({"output_type": "stream", "name": "stdout", "text": "old\n"})];
        feed(
            &mut session,
            &scripted,
            KernelEvent::Status {
                state: KernelState::Busy,
                parent: Some(id.clone()),
            },
        );
        assert_eq!(session.kernel_status(), &KernelStatus::Busy);
        feed(&mut session, &scripted, input(&id, 4));
        // The old outputs go when the run starts, not when it was queued.
        assert!(session.cells[0].cell.outputs.is_empty());
        assert_eq!(session.cells[0].cell.execution_count, Some(4));
        assert_eq!(
            session.run_state(session.cells[0].uid),
            Some(RunState::Running)
        );
        feed(&mut session, &scripted, stream(&id, "stdout", "hi\n"));
        feed(&mut session, &scripted, stream(&id, "stdout", "there\n"));
        feed(&mut session, &scripted, stream(&id, "stderr", "warn\n"));
        feed(
            &mut session,
            &scripted,
            KernelEvent::Output {
                parent: id.clone(),
                output: json!({"output_type": "execute_result", "execution_count": 4, "data": {"text/plain": "1"}, "metadata": {}}),
            },
        );
        feed(
            &mut session,
            &scripted,
            KernelEvent::Status {
                state: KernelState::Idle,
                parent: Some(id.clone()),
            },
        );
        // Still running until the reply arrives.
        assert_eq!(session.kernel_status(), &KernelStatus::Busy);
        finish_run(&mut session, &scripted, &id, true, 4);
        assert_eq!(session.kernel_status(), &KernelStatus::Idle);
        assert!(!session.is_running());
        let outputs = &session.cells[0].cell.outputs;
        assert_eq!(outputs.len(), 3);
        // Chunks of one stream are joined; another stream stays apart.
        assert_eq!(outputs[0]["text"], "hi\nthere\n");
        assert_eq!(outputs[1]["name"], "stderr");
        assert_eq!(outputs[2]["output_type"], "execute_result");
    }

    #[test]
    fn outputs_that_trail_the_reply_are_kept_until_the_idle_status() {
        let (mut session, scripted) = running(&["a"]);
        session.run_cells(&[0]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        feed(&mut session, &scripted, input(&id, 1));
        // The shell reply overtakes the output on the other channel.
        feed(&mut session, &scripted, reply(&id, true, 1));
        assert!(session.is_running());
        feed(&mut session, &scripted, stream(&id, "stdout", "late\n"));
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
        feed(
            &mut session,
            &scripted,
            KernelEvent::Status {
                state: KernelState::Idle,
                parent: Some(id.clone()),
            },
        );
        assert!(!session.is_running());
        assert_eq!(session.kernel_status(), &KernelStatus::Idle);
        // Anything after the end is ignored.
        feed(&mut session, &scripted, stream(&id, "stdout", "too late\n"));
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
    }
    #[test]
    fn several_cells_report_to_their_own_requests() {
        let (mut session, scripted) = running(&["one", "two"]);
        session.run_cells(&[0, 1]).unwrap();
        let ids = sent_ids(&scripted);
        feed(&mut session, &scripted, input(&ids[0], 1));
        // The second cell is still queued while the first runs.
        assert_eq!(
            session.run_state(session.cells[1].uid),
            Some(RunState::Queued)
        );
        feed(&mut session, &scripted, stream(&ids[0], "stdout", "1\n"));
        finish_run(&mut session, &scripted, &ids[0], true, 1);
        feed(&mut session, &scripted, input(&ids[1], 2));
        feed(&mut session, &scripted, stream(&ids[1], "stdout", "2\n"));
        finish_run(&mut session, &scripted, &ids[1], true, 2);
        assert_eq!(session.cells[0].cell.outputs[0]["text"], "1\n");
        assert_eq!(session.cells[1].cell.outputs[0]["text"], "2\n");
        assert_eq!(session.cells[1].cell.execution_count, Some(2));
    }

    #[test]
    fn a_failed_reply_still_finishes_the_cell_and_keeps_the_error() {
        let (mut session, scripted) = running(&["1/0", "later"]);
        session.run_cells(&[0, 1]).unwrap();
        let ids = sent_ids(&scripted);
        feed(&mut session, &scripted, input(&ids[0], 1));
        feed(
            &mut session,
            &scripted,
            KernelEvent::Output {
                parent: ids[0].clone(),
                output: json!({"output_type": "error", "ename": "ZeroDivisionError", "evalue": "x", "traceback": []}),
            },
        );
        finish_run(&mut session, &scripted, &ids[0], false, 1);
        // The kernel aborts what was queued behind the failure and says so.
        finish_run(&mut session, &scripted, &ids[1], false, 2);
        assert!(!session.is_running());
        assert_eq!(
            session.cells[0].cell.outputs[0]["ename"],
            "ZeroDivisionError"
        );
        assert!(session.cells[1].cell.outputs.is_empty());
    }

    #[test]
    fn clearing_output_now_or_when_the_next_output_comes() {
        let (mut session, scripted) = running(&["x"]);
        session.run_cells(&[0]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        feed(&mut session, &scripted, input(&id, 1));
        feed(&mut session, &scripted, stream(&id, "stdout", "10%\n"));
        feed(
            &mut session,
            &scripted,
            KernelEvent::ClearOutput {
                parent: id.clone(),
                wait: true,
            },
        );
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
        feed(&mut session, &scripted, stream(&id, "stdout", "20%\n"));
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
        assert_eq!(session.cells[0].cell.outputs[0]["text"], "20%\n");
        feed(
            &mut session,
            &scripted,
            KernelEvent::ClearOutput {
                parent: id,
                wait: false,
            },
        );
        assert!(session.cells[0].cell.outputs.is_empty());
    }

    #[test]
    fn a_display_can_be_updated_in_place() {
        let (mut session, scripted) = running(&["x"]);
        session.run_cells(&[0]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        feed(&mut session, &scripted, input(&id, 1));
        feed(
            &mut session,
            &scripted,
            KernelEvent::Output {
                parent: id.clone(),
                output: json!({"output_type": "display_data", "data": {"text/plain": "0%"}, "metadata": {}, "transient": {"display_id": "bar"}}),
            },
        );
        let stored = &session.cells[0].cell.outputs[0];
        assert!(stored.get("transient").is_none());
        feed(
            &mut session,
            &scripted,
            KernelEvent::UpdateDisplay {
                parent: id.clone(),
                display_id: "bar".into(),
                output: json!({"output_type": "display_data", "data": {"text/plain": "100%"}, "metadata": {}}),
            },
        );
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
        assert_eq!(
            session.cells[0].cell.outputs[0]["data"]["text/plain"],
            "100%"
        );
        // An unknown id changes nothing.
        feed(
            &mut session,
            &scripted,
            KernelEvent::UpdateDisplay {
                parent: id,
                display_id: "other".into(),
                output: json!({}),
            },
        );
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
    }

    #[test]
    fn events_for_deleted_cells_or_unknown_requests_are_ignored() {
        let (mut session, scripted) = running(&["a", "b"]);
        session.run_cells(&[1]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        session.set_focus(1, false);
        session.delete_selected();
        feed(&mut session, &scripted, input(&id, 1));
        feed(&mut session, &scripted, stream(&id, "stdout", "lost\n"));
        feed(
            &mut session,
            &scripted,
            stream("unknown", "stdout", "lost\n"),
        );
        assert!(session.cells[0].cell.outputs.is_empty());
        finish_run(&mut session, &scripted, &id, true, 1);
        assert!(!session.is_running());
    }

    #[test]
    fn the_kernel_ending_clears_the_runs_and_remembers_why() {
        let (mut session, scripted) = running(&["a"]);
        session.run_cells(&[0]).unwrap();
        feed(
            &mut session,
            &scripted,
            KernelEvent::Exited {
                details: " died\n".into(),
            },
        );
        assert_eq!(session.kernel_status(), &KernelStatus::Dead("died".into()));
        assert!(!session.is_running());
        assert!(session.run_cells(&[0]).is_err());
    }

    #[test]
    fn clear_outputs_removes_outputs_and_counts() {
        let (mut session, _scripted) = running(&["a", "b"]);
        session.cells[0].cell.outputs = vec![json!({"output_type": "stream"})];
        session.cells[0].cell.execution_count = Some(3);
        session.cells[1].cell.outputs = vec![json!({"output_type": "stream"})];
        session.clear_outputs(&[0, 7]);
        assert!(session.cells[0].cell.outputs.is_empty());
        assert_eq!(session.cells[0].cell.execution_count, None);
        assert_eq!(session.cells[1].cell.outputs.len(), 1);
    }

    #[test]
    fn a_reload_keeps_the_kernel_but_forgets_requests() {
        let (mut session, scripted) = running(&["a"]);
        session.run_cells(&[0]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        let mut other = Notebook::new();
        other.cells = vec![Cell::new(CellKind::Code, "a")];
        session.reload(other);
        assert!(!session.is_running());
        assert_eq!(session.kernel_status(), &KernelStatus::Starting);
        feed(&mut session, &scripted, stream(&id, "stdout", "late\n"));
        assert!(session.cells[0].cell.outputs.is_empty());
        // The kernel still works for new runs.
        assert_eq!(session.run_cells(&[0]).unwrap(), 1);
    }

    #[test]
    fn replacing_and_detaching_the_kernel_resets_the_state() {
        let (mut session, scripted) = running(&["a"]);
        session.run_cells(&[0]).unwrap();
        let (command_tx, new_commands) = channel();
        let (_events, event_rx) = channel();
        session.attach_kernel(
            KernelHandle::from_parts(command_tx, event_rx, "key", false),
            spec(),
        );
        assert!(!session.is_running());
        // The old handle was dropped, which asks its kernel to end.
        let mut ended = false;
        while let Ok(command) = scripted.commands.try_recv() {
            ended |= matches!(command, Command::Kill);
        }
        assert!(ended);
        session.detach_kernel();
        assert_eq!(session.kernel_status(), &KernelStatus::None);
        assert!(matches!(new_commands.recv().unwrap(), Command::Kill));
    }

    #[test]
    fn the_drawn_outputs_are_built_once_and_rebuilt_when_they_change() {
        let (mut session, scripted) = running(&["x"]);
        session.run_cells(&[0]).unwrap();
        let id = sent_ids(&scripted).remove(0);
        feed(&mut session, &scripted, input(&id, 1));
        feed(&mut session, &scripted, stream(&id, "stdout", "one\n"));
        let first = session.cells[0].blocks();
        let again = session.cells[0].blocks();
        assert!(std::sync::Arc::ptr_eq(&first, &again));
        feed(&mut session, &scripted, stream(&id, "stdout", "two\n"));
        let changed = session.cells[0].blocks();
        assert!(!std::sync::Arc::ptr_eq(&first, &changed));
        let super::super::rich::Block::Lines(lines) = &changed[0] else {
            panic!("expected lines");
        };
        assert_eq!(lines, &["one", "two"]);
        // Clearing and changing the kind rebuild it too.
        session.clear_outputs(&[0]);
        assert!(session.cells[0].blocks().is_empty());
        session.cells[0].cell.outputs =
            vec![json!({"output_type": "stream", "name": "stdout", "text": "x\n"})];
        assert_eq!(session.cells[0].blocks().len(), 1);
        session.set_kind(CellKind::Markdown);
        assert!(session.cells[0].blocks().is_empty());
        session.undo();
        assert_eq!(session.cells[0].blocks().len(), 1);
    }
    #[test]
    fn info_events_report_the_language_without_asking_for_a_redraw() {
        let (mut session, scripted) = running(&["a"]);
        scripted
            .events
            .send(KernelEvent::Info {
                language: Some("python".into()),
                banner: None,
            })
            .unwrap();
        assert!(!session.poll_kernel());
        assert_eq!(session.kernel.language.as_deref(), Some("python"));
    }
}
