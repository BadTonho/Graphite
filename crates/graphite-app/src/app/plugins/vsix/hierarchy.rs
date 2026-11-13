//! Call and type hierarchies from extensions (`registerCallHierarchyProvider`,
//! `registerTypeHierarchyProvider`).
//!
//! A hierarchy takes several steps: the provider first prepares the item under the cursor, then
//! answers for its callers and callees (or super and subtypes). The related items of both
//! directions are collected into one list shown in the locations picker, like the language server
//! version does.

use super::languages::Feature;
use super::*;
use crate::app::go_menu::GoSymbolItem;
use std::collections::BTreeSet;

/// Replies to hierarchy requests carry ids in `HIERARCHY_BASE..HIERARCHY_END`.
const HIERARCHY_BASE: u64 = 7_000_000;
const HIERARCHY_END: u64 = 8_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn is_hierarchy_reply(id: u64) -> bool {
    (HIERARCHY_BASE..HIERARCHY_END).contains(&id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Call,
    Type,
}

impl Kind {
    fn feature(self) -> Feature {
        match self {
            Kind::Call => Feature::CallHierarchy,
            Kind::Type => Feature::TypeHierarchy,
        }
    }

    fn prepare(self) -> &'static str {
        match self {
            Kind::Call => "$prepareCallHierarchy",
            Kind::Type => "$prepareTypeHierarchy",
        }
    }

    fn release(self) -> &'static str {
        match self {
            Kind::Call => "$releaseCallHierarchy",
            Kind::Type => "$releaseTypeHierarchy",
        }
    }

    /// The two follow-up methods, each with the label of its items and the key that holds the
    /// related item (`from`/`to` for calls; types are returned bare).
    fn directions(self) -> [(&'static str, &'static str, Option<&'static str>); 2] {
        match self {
            Kind::Call => [
                ("$provideCallHierarchyIncomingCalls", "← ", Some("from")),
                ("$provideCallHierarchyOutgoingCalls", "→ ", Some("to")),
            ],
            Kind::Type => [
                ("$provideTypeHierarchySupertypes", "↑ ", None),
                ("$provideTypeHierarchySubtypes", "↓ ", None),
            ],
        }
    }
}

/// What a follow-up call is for.
struct Waiting {
    label: &'static str,
    key: Option<&'static str>,
}

struct Active {
    kind: Kind,
    handle: u64,
    /// Id of the prepare call, until it is answered.
    prepare: Option<u64>,
    session_id: String,
    waiting: std::collections::BTreeMap<u64, Waiting>,
    items: Vec<GoSymbolItem>,
    deadline: Instant,
}

#[derive(Default)]
pub(super) struct Hierarchies {
    next_id: u64,
    active: Option<Active>,
}

impl Hierarchies {
    pub(super) fn busy(&self) -> bool {
        self.active.is_some()
    }

    fn next_id(&mut self) -> u64 {
        self.next_id = (self.next_id + 1) % (HIERARCHY_END - HIERARCHY_BASE);
        HIERARCHY_BASE + self.next_id
    }
}

/// A hierarchy item of Code OSS as a picker entry: the name with a direction `label`, at the
/// start of its selection range. `None` when the item lacks a name, a file or a position.
pub(super) fn picker_item(item: &Value, label: &str) -> Option<GoSymbolItem> {
    let name = item["name"].as_str()?;
    let uri = item["uri"].as_str()?;
    let line = item["selectionRange"]["startLineNumber"]
        .as_u64()?
        .checked_sub(1)? as usize;
    let mut found = crate::app::lsp_features::location_item(uri, line)?;
    found.name = format!("{label}{name}");
    Some(found)
}

/// The related items of one follow-up answer: `[{ from: item }]`, `[{ to: item }]` or bare items.
pub(super) fn related_items(result: &Value, label: &str, key: Option<&str>) -> Vec<GoSymbolItem> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| match key {
            Some(key) => picker_item(&entry[key], label),
            None => picker_item(entry, label),
        })
        .collect()
}

impl App {
    /// Starts a hierarchy for the symbol at the cursor if an extension provides one. Returns
    /// whether a request was sent; the answer opens the locations picker.
    pub(crate) fn start_vsix_hierarchy(&mut self, kind: Kind) -> bool {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return false;
        };
        let started = self.send_vsix_hierarchy_prepare(&mut runtime, kind);
        self.plugins.vsix = Some(runtime);
        match started {
            Ok(started) => started,
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn send_vsix_hierarchy_prepare(&self, runtime: &mut Runtime, kind: Kind) -> Result<bool> {
        if !runtime.ready || !self.vsix_workspace_trusted() {
            return Ok(false);
        }
        let id = self.active_document();
        let document = crate::app::services::snapshot_document(id, &self.documents[id])
            .map_err(|error| error.to_string())?;
        let Some(handle) = runtime.languages.provider_for(kind.feature(), &document) else {
            return Ok(false);
        };
        self.sync_vsix_documents(runtime)?;
        if let Some(previous) = runtime.hierarchy.active.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall","id":previous.prepare}))
                .ok();
        }
        let cursor = documents::position(&document.text, self.editor().cursor().char_idx);
        let call = runtime.hierarchy.next_id();
        runtime.session.send(json!({
            "type": "call", "id": call, "actor": "ExtHostLanguageFeatures",
            "method": kind.prepare(), "cancellable": true,
            "args": [handle, {"$graphiteUri": document.uri},
                {"lineNumber": cursor.line + 1, "column": cursor.character + 1}],
        }))?;
        runtime.hierarchy.active = Some(Active {
            kind,
            handle,
            prepare: Some(call),
            session_id: String::new(),
            waiting: Default::default(),
            items: Vec::new(),
            deadline: Instant::now() + TIMEOUT,
        });
        Ok(true)
    }

    /// Gives up on a hierarchy the host did not answer in time.
    pub(super) fn expire_vsix_hierarchy(&mut self, runtime: &mut Runtime) {
        if runtime
            .hierarchy
            .active
            .as_ref()
            .is_some_and(|active| Instant::now() >= active.deadline)
        {
            runtime.hierarchy.active = None;
            self.status = self.t("lsp.hierarchy.none").into();
        }
    }

    /// Handles one reply of a hierarchy: the prepared item, or the items of one direction.
    pub(super) fn finish_vsix_hierarchy(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some(active) = runtime.hierarchy.active.as_mut() else {
            return;
        };
        if active.prepare == Some(id) {
            active.prepare = None;
            let item = event["result"].as_array().and_then(|items| items.first());
            let (Some(item), None) = (item, event["error"].as_str()) else {
                runtime.hierarchy.active = None;
                self.status = self.t("lsp.hierarchy.none").into();
                return;
            };
            let (Some(session), Some(item_id)) =
                (item["_sessionId"].as_str(), item["_itemId"].as_str())
            else {
                runtime.hierarchy.active = None;
                self.status = self.t("lsp.hierarchy.none").into();
                return;
            };
            active.session_id = session.to_owned();
            let (kind, handle) = (active.kind, active.handle);
            for (method, label, key) in kind.directions() {
                let call = runtime.hierarchy.next_id();
                let sent = runtime.session.send(json!({
                    "type": "call", "id": call, "actor": "ExtHostLanguageFeatures",
                    "method": method, "cancellable": true,
                    "args": [handle, session, item_id],
                }));
                if sent.is_ok() {
                    if let Some(active) = runtime.hierarchy.active.as_mut() {
                        active.waiting.insert(call, Waiting { label, key });
                    }
                }
            }
            return;
        }
        let Some(waiting) = active.waiting.remove(&id) else {
            return;
        };
        if event["error"].as_str().is_none() {
            active
                .items
                .extend(related_items(&event["result"], waiting.label, waiting.key));
        }
        if !active.waiting.is_empty() {
            return;
        }
        let finished = runtime.hierarchy.active.take().unwrap();
        let _ = runtime.session.send(json!({
            "type": "call", "actor": "ExtHostLanguageFeatures",
            "method": finished.kind.release(), "args": [finished.handle, finished.session_id],
        }));
        if finished.items.is_empty() {
            self.status = self.t("lsp.hierarchy.none").into();
        } else {
            self.open_locations_picker(finished.items);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;

    fn item(name: &str, file: &std::path::Path, line: u64) -> Value {
        json!({
            "_sessionId": "s1", "_itemId": format!("i-{name}"), "name": name, "kind": 11,
            "uri": graphite_core_api::DocumentUri::from_path(file).unwrap().as_str(),
            "range": {"startLineNumber": line, "startColumn": 1, "endLineNumber": line + 1, "endColumn": 1},
            "selectionRange": {"startLineNumber": line, "startColumn": 1, "endLineNumber": line, "endColumn": 5},
        })
    }

    fn app_with_provider(dir: &TempDir, register: &str) -> (App, LoopbackHost, std::path::PathBuf) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.run_ui_command("graphite.security.trustWorkspace");
        let file = dir.path().join("code.txt");
        std::fs::write(&file, "one\ntwo\nthree\n").unwrap();
        app.open_file(file.clone());
        let (session, host) = Session::loopback();
        let mut runtime = Runtime::for_test(session);
        languages::language_rpc(
            &mut runtime.languages,
            "MainThreadLanguageFeatures",
            register,
            &[json!(3), json!([{"language": "*"}])],
        )
        .unwrap()
        .unwrap();
        app.plugins.vsix = Some(runtime);
        (app, host, file)
    }

    /// Everything Graphite has sent to the host since the last call.
    fn drain(host: &LoopbackHost) -> Vec<Value> {
        std::iter::from_fn(|| host.next_sent()).collect()
    }

    fn only(messages: &[Value], method: &str) -> Vec<Value> {
        messages
            .iter()
            .filter(|message| message["method"] == method)
            .cloned()
            .collect()
    }

    fn calls(host: &LoopbackHost, method: &str) -> Vec<Value> {
        only(&drain(host), method)
    }

    fn answer(host: &LoopbackHost, id: &Value, result: Value) {
        host.emit(json!({"type": "result", "id": id, "result": result}));
    }

    #[test]
    fn picker_items_carry_the_label_file_and_zero_based_line() {
        let dir = TempDir::new();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x\n").unwrap();
        let found = picker_item(&item("callee", &file, 5), "→ ").unwrap();
        assert_eq!(found.name, "→ callee");
        assert_eq!(found.line, 4);
        assert!(found.path.ends_with("a.txt"));
        assert!(picker_item(&json!({"name": "x"}), "").is_none());
        assert!(picker_item(&Value::Null, "").is_none());
        let mut zero = item("z", &file, 1);
        zero["selectionRange"]["startLineNumber"] = json!(0);
        assert!(picker_item(&zero, "").is_none(), "lines are 1-based");
    }

    #[test]
    fn related_items_read_wrapped_and_bare_entries() {
        let dir = TempDir::new();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x\n").unwrap();
        let calls = json!([{"from": item("a", &file, 1), "fromRanges": []}, {"nope": 1}]);
        assert_eq!(related_items(&calls, "← ", Some("from")).len(), 1);
        let types = json!([item("Base", &file, 2), item("Other", &file, 3)]);
        let names: Vec<_> = related_items(&types, "↑ ", None)
            .into_iter()
            .map(|i| i.name)
            .collect();
        assert_eq!(names, ["↑ Base", "↑ Other"]);
        assert!(related_items(&Value::Null, "", None).is_empty());
    }

    #[test]
    fn a_call_hierarchy_prepares_then_collects_callers_and_callees_into_the_picker() {
        let dir = TempDir::new();
        let (mut app, host, file) = app_with_provider(&dir, "$registerCallHierarchyProvider");
        app.editor_mut().set_cursor_line_col(1, 1);
        assert!(app.start_vsix_hierarchy(Kind::Call));
        let prepare = calls(&host, "$prepareCallHierarchy");
        assert_eq!(prepare.len(), 1);
        assert_eq!(prepare[0]["args"][0], 3);
        assert_eq!(prepare[0]["args"][2], json!({"lineNumber": 2, "column": 2}));

        answer(&host, &prepare[0]["id"], json!([item("main", &file, 2)]));
        app.poll_vsix();
        let sent = drain(&host);
        let incoming = only(&sent, "$provideCallHierarchyIncomingCalls");
        let outgoing = only(&sent, "$provideCallHierarchyOutgoingCalls");
        assert_eq!((incoming.len(), outgoing.len()), (1, 1));
        assert_eq!(incoming[0]["args"], json!([3, "s1", "i-main"]));
        assert!(
            app.go_picker.is_none(),
            "the picker waits for both directions"
        );

        answer(
            &host,
            &incoming[0]["id"],
            json!([{"from": item("caller", &file, 1), "fromRanges": []}]),
        );
        app.poll_vsix();
        assert!(app.go_picker.is_none());
        answer(
            &host,
            &outgoing[0]["id"],
            json!([{"to": item("callee", &file, 3), "fromRanges": []}]),
        );
        app.poll_vsix();

        let names: Vec<_> = app
            .go_picker
            .as_ref()
            .expect("the picker opened")
            .items
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        assert_eq!(names, ["← caller", "→ callee"]);
        let released = calls(&host, "$releaseCallHierarchy");
        assert_eq!(released[0]["args"], json!([3, "s1"]));
        assert!(!app.plugins.vsix.as_ref().unwrap().hierarchy.busy());
    }

    #[test]
    fn a_type_hierarchy_lists_supertypes_and_subtypes() {
        let dir = TempDir::new();
        let (mut app, host, file) = app_with_provider(&dir, "$registerTypeHierarchyProvider");
        assert!(app.start_vsix_hierarchy(Kind::Type));
        let prepare = calls(&host, "$prepareTypeHierarchy");
        answer(&host, &prepare[0]["id"], json!([item("Widget", &file, 1)]));
        app.poll_vsix();
        let sent = drain(&host);
        let supers = only(&sent, "$provideTypeHierarchySupertypes");
        let subs = only(&sent, "$provideTypeHierarchySubtypes");
        answer(&host, &supers[0]["id"], json!([item("Base", &file, 1)]));
        answer(
            &host,
            &subs[0]["id"],
            json!([item("Button", &file, 2), item("Label", &file, 3)]),
        );
        app.poll_vsix();
        let names: Vec<_> = app
            .go_picker
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|e| e.name.clone())
            .collect();
        assert_eq!(names, ["↑ Base", "↓ Button", "↓ Label"]);
        assert_eq!(calls(&host, "$releaseTypeHierarchy").len(), 1);
    }

    #[test]
    fn nothing_found_or_a_failure_ends_with_a_message_and_no_picker() {
        for prepared in [json!([]), json!(null)] {
            let dir = TempDir::new();
            let (mut app, host, _) = app_with_provider(&dir, "$registerCallHierarchyProvider");
            app.start_vsix_hierarchy(Kind::Call);
            let prepare = calls(&host, "$prepareCallHierarchy");
            answer(&host, &prepare[0]["id"], prepared);
            app.poll_vsix();
            assert!(app.go_picker.is_none());
            assert_eq!(app.status, app.t("lsp.hierarchy.none"));
            assert!(!app.plugins.vsix.as_ref().unwrap().hierarchy.busy());
        }

        let dir = TempDir::new();
        let (mut app, host, file) = app_with_provider(&dir, "$registerCallHierarchyProvider");
        app.start_vsix_hierarchy(Kind::Call);
        let prepare = calls(&host, "$prepareCallHierarchy");
        answer(&host, &prepare[0]["id"], json!([item("main", &file, 1)]));
        app.poll_vsix();
        let sent = drain(&host);
        let incoming = only(&sent, "$provideCallHierarchyIncomingCalls");
        let outgoing = only(&sent, "$provideCallHierarchyOutgoingCalls");
        host.emit(json!({"type": "result", "id": incoming[0]["id"], "error": "boom"}));
        answer(&host, &outgoing[0]["id"], json!([]));
        app.poll_vsix();
        assert!(
            app.go_picker.is_none(),
            "both directions came back empty or failed"
        );
        assert_eq!(app.status, app.t("lsp.hierarchy.none"));
    }

    #[test]
    fn without_a_matching_provider_no_request_is_made() {
        let dir = TempDir::new();
        let (mut app, host, _) = app_with_provider(&dir, "$registerTypeHierarchyProvider");
        assert!(
            !app.start_vsix_hierarchy(Kind::Call),
            "only a type provider is registered"
        );
        assert!(calls(&host, "$prepareCallHierarchy").is_empty());
        let mut none = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(!none.start_vsix_hierarchy(Kind::Call), "no extension host");
    }

    #[test]
    fn a_hierarchy_the_host_never_answers_expires() {
        let dir = TempDir::new();
        let (mut app, _host, _) = app_with_provider(&dir, "$registerCallHierarchyProvider");
        app.start_vsix_hierarchy(Kind::Call);
        app.plugins
            .vsix
            .as_mut()
            .unwrap()
            .hierarchy
            .active
            .as_mut()
            .unwrap()
            .deadline = Instant::now() - Duration::from_secs(1);
        app.poll_vsix();
        assert!(!app.plugins.vsix.as_ref().unwrap().hierarchy.busy());
        assert_eq!(app.status, app.t("lsp.hierarchy.none"));
    }

    #[test]
    fn reply_ids_are_recognised_by_range() {
        assert!(is_hierarchy_reply(HIERARCHY_BASE));
        assert!(!is_hierarchy_reply(HIERARCHY_END));
        assert!(!is_hierarchy_reply(6_000_000), "passive replies");
    }
}
