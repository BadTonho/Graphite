//! Code lenses from extensions (`registerCodeLensProvider`): short actionable texts attached to
//! lines of code, such as "3 references" or "Run test".
//!
//! Graphite has no space between editor lines, so each lens is shown as muted text at the end of
//! its line (several lenses on one line are joined with ` | `). Ctrl+click at the end of such a line,
//! or the **Run Code Lens on Current Line** command, runs the first lens of the line.
//!
//! A lens may arrive without its command; those are resolved one by one (up to [`RESOLVE_LIMIT`]
//! per document revision), and the line text fills in as the answers come back. Commands that are
//! registered by an extension, or that Graphite has itself, can run; others are reported.

use super::languages::Feature;
use super::*;
use graphite_core_api::DocumentId;
use std::collections::BTreeMap;

/// Replies to code lens requests carry ids in `LENS_BASE..LENS_END`.
const LENS_BASE: u64 = 9_000_000;
const LENS_END: u64 = 10_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);
/// Most lenses resolved for one revision of a document.
const RESOLVE_LIMIT: usize = 50;
/// Longest text shown for one line, in characters.
const LINE_LIMIT: usize = 120;

pub(super) fn is_lens_reply(id: u64) -> bool {
    (LENS_BASE..LENS_END).contains(&id)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LensCommand {
    pub id: String,
    pub title: String,
    pub args: Vec<Value>,
}

#[derive(Clone, Debug)]
struct Lens {
    /// 0-based line and start column, to order lenses on one line.
    line: u32,
    column: u32,
    /// The lens as the provider sent it, handed back to `$resolveCodeLens`.
    dto: Value,
    command: Option<LensCommand>,
}

/// The lenses of the active document at one revision.
pub(crate) struct LensSet {
    pub uri: String,
    document: DocumentId,
    revision: u64,
    lenses: Vec<Lens>,
}

struct Listing {
    id: u64,
    handle: u64,
    document: DocumentId,
    revision: u64,
    uri: String,
}

#[derive(Default)]
pub(super) struct CodeLenses {
    next_id: u64,
    listing: Option<Listing>,
    done: Option<(DocumentId, u64)>,
    /// Resolve calls in flight: reply id to the index of the lens they fill in.
    resolving: BTreeMap<u64, usize>,
    /// Provider session to release once every lens is resolved.
    release: Option<(u64, u64)>,
    handle: u64,
    deadline: Option<Instant>,
    pub(crate) set: Option<LensSet>,
}

impl CodeLenses {
    pub(super) fn busy(&self) -> bool {
        self.listing.is_some() || !self.resolving.is_empty()
    }

    fn next_id(&mut self) -> u64 {
        self.next_id = (self.next_id + 1) % (LENS_END - LENS_BASE);
        LENS_BASE + self.next_id
    }
}

/// The lenses of an `ICodeLensListDto`. Lenses without a usable range are dropped.
fn parse_lenses(result: &Value) -> Vec<Lens> {
    result["lenses"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|dto| {
            let line = u32::try_from(dto["range"]["startLineNumber"].as_u64()?)
                .ok()?
                .checked_sub(1)?;
            let column = dto["range"]["startColumn"]
                .as_u64()
                .unwrap_or(1)
                .saturating_sub(1) as u32;
            Some(Lens {
                line,
                column,
                command: parse_command(&dto["command"]),
                dto: dto.clone(),
            })
        })
        .collect()
}

/// `{ id, title, arguments? }` as a command; `None` without both an id and a title.
fn parse_command(value: &Value) -> Option<LensCommand> {
    let id = value["id"].as_str().filter(|id| !id.is_empty())?;
    let title = value["title"].as_str()?;
    Some(LensCommand {
        id: id.to_owned(),
        title: title.to_owned(),
        args: value["arguments"].as_array().cloned().unwrap_or_default(),
    })
}

/// The text shown at the end of each line: the titles of its resolved lenses, joined with ` | `.
fn line_texts(lenses: &[Lens]) -> Vec<(u32, String)> {
    let mut by_line: BTreeMap<u32, Vec<&Lens>> = BTreeMap::new();
    for lens in lenses {
        if lens.command.is_some() {
            by_line.entry(lens.line).or_default().push(lens);
        }
    }
    by_line
        .into_iter()
        .map(|(line, mut lenses)| {
            lenses.sort_by_key(|lens| lens.column);
            let text = lenses
                .iter()
                .filter_map(|lens| lens.command.as_ref())
                .map(|command| command.title.replace('\n', " "))
                .collect::<Vec<_>>()
                .join(" | ");
            (line, text.chars().take(LINE_LIMIT).collect())
        })
        .collect()
}

impl App {
    /// Asks for the lenses of the active document when they are out of date.
    pub(super) fn drive_vsix_code_lens(&mut self, runtime: &mut Runtime) -> Result<()> {
        if !self.vsix_workspace_trusted() || !runtime.languages.has_provider(&[Feature::CodeLens]) {
            return Ok(());
        }
        let now = Instant::now();
        if runtime
            .lenses
            .deadline
            .is_some_and(|deadline| now >= deadline)
        {
            runtime.lenses.listing = None;
            runtime.lenses.resolving.clear();
            runtime.lenses.deadline = None;
        }
        let document = self.active_document();
        let revision = self.documents[document].buffer.revision();
        if runtime.lenses.done == Some((document, revision)) || runtime.lenses.listing.is_some() {
            return Ok(());
        }
        let snapshot = crate::app::services::snapshot_document(document, &self.documents[document])
            .map_err(|error| error.to_string())?;
        runtime.lenses.done = Some((document, revision));
        // Lenses of an older text would sit on the wrong lines.
        if let Some(old) = runtime.lenses.set.take() {
            runtime.decorations.clear_lenses(&old.uri);
        }
        runtime.lenses.resolving.clear();
        let Some(handle) = runtime.provider(Feature::CodeLens, &snapshot) else {
            return Ok(());
        };
        let id = runtime.lenses.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideCodeLenses", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri}],
        }))?;
        runtime.lenses.handle = handle;
        runtime.lenses.deadline = Some(now + TIMEOUT);
        runtime.lenses.listing = Some(Listing {
            id,
            handle,
            document,
            revision,
            uri: snapshot.uri.as_str().to_owned(),
        });
        Ok(())
    }

    /// Handles the list of lenses or the resolution of one.
    pub(super) fn finish_vsix_code_lens(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let current = |document: DocumentId, revision: u64, app: &App| {
            app.active_document() == document
                && app.documents[document].buffer.revision() == revision
        };
        if runtime.lenses.listing.as_ref().is_some_and(|l| l.id == id) {
            let listing = runtime.lenses.listing.take().unwrap();
            if event["error"].is_string() {
                // Not asked again until the document changes.
                return;
            }
            if !current(listing.document, listing.revision, self) {
                runtime.lenses.done = None;
                return;
            }
            let lenses = parse_lenses(&event["result"]);
            let cache = event["result"]["cacheId"].as_u64();
            let mut to_resolve = Vec::new();
            for (index, lens) in lenses.iter().enumerate() {
                if lens.command.is_none() && lens.dto["cacheId"].is_array() {
                    to_resolve.push(index);
                }
            }
            to_resolve.truncate(RESOLVE_LIMIT);
            runtime.lenses.set = Some(LensSet {
                uri: listing.uri.clone(),
                document: listing.document,
                revision: listing.revision,
                lenses,
            });
            for index in to_resolve {
                let dto = runtime.lenses.set.as_ref().unwrap().lenses[index]
                    .dto
                    .clone();
                let call = runtime.lenses.next_id();
                let sent = runtime.session.send(json!({
                    "type": "call", "id": call, "actor": "ExtHostLanguageFeatures",
                    "method": "$resolveCodeLens", "cancellable": true,
                    "args": [listing.handle, dto],
                }));
                if sent.is_ok() {
                    runtime.lenses.resolving.insert(call, index);
                }
            }
            runtime.lenses.release = cache.map(|cache| (listing.handle, cache));
            self.show_vsix_lenses(runtime);
            self.release_lenses_when_done(runtime);
        } else if let Some(index) = runtime.lenses.resolving.remove(&id) {
            if event["error"].as_str().is_none() {
                if let Some(command) = parse_command(&event["result"]["command"]) {
                    if let Some(lens) = runtime
                        .lenses
                        .set
                        .as_mut()
                        .and_then(|set| set.lenses.get_mut(index))
                    {
                        lens.command = Some(command);
                    }
                }
            }
            self.show_vsix_lenses(runtime);
            self.release_lenses_when_done(runtime);
        }
    }

    fn release_lenses_when_done(&mut self, runtime: &mut Runtime) {
        if runtime.lenses.resolving.is_empty() {
            runtime.lenses.deadline = None;
            if let Some((handle, cache)) = runtime.lenses.release.take() {
                let _ = runtime.session.send(json!({
                    "type": "call", "actor": "ExtHostLanguageFeatures",
                    "method": "$releaseCodeLenses", "args": [handle, cache],
                }));
            }
        }
    }

    /// Draws the lens text of every line that has a resolved lens.
    fn show_vsix_lenses(&mut self, runtime: &mut Runtime) {
        let Some(set) = runtime.lenses.set.as_ref() else {
            return;
        };
        let texts = line_texts(&set.lenses);
        let uri = set.uri.clone();
        runtime.decorations.set_lenses(&uri, texts);
    }

    /// The first resolved lens command on `line` of the active document, if the lenses describe
    /// its current text.
    pub(crate) fn code_lens_on_line(&self, line: usize) -> Option<LensCommand> {
        let document = self.active_document();
        let revision = self.documents[document].buffer.revision();
        self.plugins
            .vsix
            .as_ref()?
            .lenses
            .set
            .as_ref()
            .filter(|set| set.document == document && set.revision == revision)?
            .lenses
            .iter()
            .filter(|lens| lens.line as usize == line)
            .min_by_key(|lens| lens.column)
            .filter(|lens| lens.command.is_some())
            .and_then(|lens| lens.command.clone())
    }

    /// The code lens text of `line` of the document with this URI.
    pub(crate) fn code_lens_text(&self, uri: &str, line: usize) -> Option<String> {
        self.vsix_line_decorations(uri, line, true)
            .into_iter()
            .find(|mark| mark.lens)
            .and_then(|mark| mark.after)
            .map(|after| after.text)
    }

    /// Runs the first code lens of `line`. Returns whether there was one.
    pub(crate) fn run_code_lens_on_line(&mut self, line: usize) -> bool {
        let Some(command) = self.code_lens_on_line(line) else {
            return false;
        };
        self.run_lens_command(command);
        true
    }

    /// Runs the first code lens of the cursor line. Returns whether there was one.
    pub(crate) fn run_code_lens_on_cursor_line(&mut self) -> bool {
        let line = self.editor().cursor_line_col().0;
        let Some(command) = self.code_lens_on_line(line) else {
            return false;
        };
        self.run_lens_command(command);
        true
    }

    /// Ctrl+click at the end of a line that has a lens runs it.
    pub(crate) fn run_code_lens_at_line_end(&mut self) -> bool {
        let (line, column) = self.editor().cursor_line_col();
        let length = self
            .editor()
            .line(line)
            .trim_end_matches(['\r', '\n'])
            .chars()
            .count();
        column >= length && self.run_code_lens_on_cursor_line()
    }

    pub(super) fn run_lens_command(&mut self, command: LensCommand) {
        if crate::app::command_registry::registry()
            .get(&command.id)
            .is_some()
        {
            let argument = command.args.first().cloned().unwrap_or_default();
            self.run_ui_command_args(&command.id, argument);
            return;
        }
        let registered = self
            .plugins
            .vsix
            .as_ref()
            .is_some_and(|runtime| runtime.registered.contains(&command.id));
        if !registered {
            if let Some(mut runtime) = self.plugins.vsix.take() {
                self.record_requested_vsix_command(
                    &mut runtime,
                    &command.id,
                    super::requested_commands::Source::CodeLens,
                );
                self.plugins.vsix = Some(runtime);
            }
            self.status = format!("The code lens command {} is not available", command.id);
            return;
        }
        if let Err(error) =
            self.run_vsix_command_with_args(&format!(":{}", command.id), command.args)
        {
            self.status = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;

    fn lens(line: u64, title: Option<&str>, cache: bool) -> Value {
        let mut lens = json!({"range": {
            "startLineNumber": line, "startColumn": 1, "endLineNumber": line, "endColumn": 5
        }});
        if let Some(title) = title {
            lens["command"] = json!({"id": "ext.run", "title": title, "arguments": [line]});
        }
        if cache {
            lens["cacheId"] = json!([3, line]);
        }
        lens
    }

    fn app_with_lens_provider(dir: &TempDir, text: &str) -> (App, LoopbackHost) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.run_ui_command("graphite.security.trustWorkspace");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, text).unwrap();
        app.open_file(file);
        let (session, host) = Session::loopback();
        let mut runtime = Runtime::for_test(session);
        languages::language_rpc(
            &mut runtime.languages,
            "MainThreadLanguageFeatures",
            "$registerCodeLensSupport",
            &[json!(4), json!([{"language": "*"}]), json!(null)],
        )
        .unwrap()
        .unwrap();
        runtime.registered.insert("ext.run".into());
        app.plugins.vsix = Some(runtime);
        (app, host)
    }

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

    fn answer(host: &LoopbackHost, id: &Value, result: Value) {
        host.emit(json!({"type": "result", "id": id, "result": result}));
    }

    fn uri(dir: &TempDir) -> String {
        graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned()
    }

    #[test]
    fn lenses_parse_with_or_without_commands_and_drop_bad_ranges() {
        let lenses = parse_lenses(&json!({"lenses": [
            lens(3, Some("2 references"), false),
            lens(5, None, true),
            {"range": {"startLineNumber": 0}},
            {"command": {"id": "x", "title": "y"}},
        ]}));
        assert_eq!(lenses.len(), 2);
        assert_eq!(lenses[0].line, 2);
        assert_eq!(lenses[0].command.as_ref().unwrap().args, [json!(3)]);
        assert!(lenses[1].command.is_none());
        assert!(parse_command(&json!({"id": "", "title": "t"})).is_none());
        assert!(parse_command(&json!({"id": "a"})).is_none());
    }

    #[test]
    fn lenses_on_one_line_are_joined_in_column_order_and_unresolved_ones_wait() {
        let mut first = parse_lenses(&json!({"lenses": [lens(1, Some("B"), false)]}))[0].clone();
        first.column = 10;
        let mut second = parse_lenses(&json!({"lenses": [lens(1, Some("A"), false)]}))[0].clone();
        second.column = 2;
        let pending = parse_lenses(&json!({"lenses": [lens(2, None, true)]}))[0].clone();
        let texts = line_texts(&[first, second, pending]);
        assert_eq!(
            texts,
            [(0, "A | B".to_owned())],
            "the unresolved line is not shown yet"
        );
    }

    #[test]
    fn long_lens_text_is_cut_and_newlines_become_spaces() {
        let long = "x".repeat(500);
        let lenses = parse_lenses(&json!({"lenses": [lens(1, Some(&long), false)]}));
        assert_eq!(line_texts(&lenses)[0].1.chars().count(), LINE_LIMIT);
        let multi = parse_lenses(&json!({"lenses": [lens(1, Some("a\nb"), false)]}));
        assert_eq!(line_texts(&multi)[0].1, "a b");
    }

    #[test]
    fn lenses_are_listed_resolved_shown_and_released() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\ntwo\nthree\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 4);

        answer(
            &host,
            &sent[0]["id"],
            json!({"lenses": [lens(1, Some("run"), false), lens(3, None, true)], "cacheId": 8}),
        );
        app.poll_vsix();
        let sent = drain(&host);
        let resolves = only(&sent, "$resolveCodeLens");
        assert_eq!(
            resolves.len(),
            1,
            "only the lens without a command is resolved"
        );
        assert_eq!(resolves[0]["args"][1]["cacheId"], json!([3, 3]));
        assert!(
            only(&sent, "$releaseCodeLenses").is_empty(),
            "still resolving"
        );

        let marks = |app: &App, line| app.vsix_line_decorations(&uri(&dir), line, true);
        assert_eq!(marks(&app, 0).len(), 1, "the resolved lens shows at once");
        assert!(marks(&app, 2).is_empty());
        assert_eq!(marks(&app, 0)[0].after.as_ref().unwrap().text, "run");

        answer(
            &host,
            &resolves[0]["id"],
            json!({"range": lens(3, None, false)["range"],
                   "command": {"id": "ext.run", "title": "3 references"}}),
        );
        app.poll_vsix();
        assert_eq!(
            marks(&app, 2)[0].after.as_ref().unwrap().text,
            "3 references"
        );
        let released = only(&drain(&host), "$releaseCodeLenses");
        assert_eq!(released[0]["args"], json!([4, 8]));
        assert!(!app.plugins.vsix.as_ref().unwrap().lenses.busy());
    }

    #[test]
    fn unwrapped_lines_get_a_lens_row_above_them_and_clicking_it_runs_the_lens() {
        use crate::app::editing::lens_rows::BodyRow;
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\n  two\nthree\nfour\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        answer(
            &host,
            &sent[0]["id"],
            json!({"lenses": [lens(2, Some("run"), false), lens(4, Some("3 refs"), false)]}),
        );
        app.poll_vsix();
        let uri = uri(&dir);
        assert_eq!(app.code_lens_text(&uri, 1).as_deref(), Some("run"));
        assert_eq!(app.code_lens_text(&uri, 2), None);
        assert!(
            app.vsix_line_decorations(&uri, 1, true)[0].lens,
            "marked as a lens"
        );

        // Their rows: the lens row goes above its line and the rest move down.
        let lines: Vec<usize> = (0..4).collect();
        let rows = app.body_rows_for_active(&lines, 10);
        assert_eq!(
            rows,
            [
                BodyRow::Line(0),
                BodyRow::Lens(1),
                BodyRow::Line(1),
                BodyRow::Line(2),
                BodyRow::Lens(3),
                BodyRow::Line(3),
            ]
        );
        assert_eq!(app.lens_rows_in_view(4), 2);
        assert_eq!(app.lens_rows_before(2), 1);

        // Screen rows map back to lines; a lens row belongs to its line and says so.
        assert_eq!(app.editor_line_at_row(0), 0);
        assert_eq!(app.editor_line_at_row(1), 1);
        assert_eq!(app.editor_line_at_row(2), 1);
        assert_eq!(app.editor_line_at_row(3), 2);
        assert_eq!(app.editor_line_at_row(5), 3);
        assert_eq!(app.editor_row_lens_line(1), Some(1));
        assert_eq!(app.editor_row_lens_line(2), None);
        assert_eq!(app.editor_row_lens_line(0), None);
        assert!(app.run_code_lens_on_line(1), "the lens of the line is run");
        assert!(!app.run_code_lens_on_line(2));

        // With word wrap the lenses stay at the end of the line, so there are no lens rows.
        app.word_wrap = true;
        assert_eq!(app.body_rows_for_active(&lines, 10).len(), 4);
        assert_eq!(app.lens_rows_in_view(4), 0);
        assert_eq!(
            app.code_lens_text(&uri, 1).as_deref(),
            Some("run"),
            "still known"
        );
        assert_eq!(app.editor_line_at_row(2), 2);
    }

    #[test]
    fn editing_clears_the_lenses_and_asks_again() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\n");
        app.poll_vsix();
        let first = only(&drain(&host), "$provideCodeLenses");
        answer(
            &host,
            &first[0]["id"],
            json!({"lenses": [lens(1, Some("run"), false)]}),
        );
        app.poll_vsix();
        assert_eq!(app.vsix_line_decorations(&uri(&dir), 0, true).len(), 1);

        app.editor_mut().insert("x");
        assert!(
            app.code_lens_on_line(0).is_none(),
            "lenses of an older text do not run"
        );
        app.poll_vsix();
        assert!(app.vsix_line_decorations(&uri(&dir), 0, true).is_empty());
        assert_eq!(only(&drain(&host), "$provideCodeLenses").len(), 1);
    }

    #[test]
    fn a_failing_provider_is_not_asked_again_until_the_text_changes() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        host.emit(json!({"type": "result", "id": sent[0]["id"], "error": "boom"}));
        app.poll_vsix();
        app.poll_vsix();
        assert!(only(&drain(&host), "$provideCodeLenses").is_empty());
    }

    #[test]
    fn running_a_lens_executes_an_extension_command_with_its_arguments() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\ntwo\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        answer(
            &host,
            &sent[0]["id"],
            json!({"lenses": [lens(2, Some("run"), false)]}),
        );
        app.poll_vsix();

        app.editor_mut().set_cursor_line_col(0, 1);
        assert!(
            !app.run_code_lens_on_cursor_line(),
            "no lens on the first line"
        );
        app.editor_mut().set_cursor_line_col(1, 1);
        assert!(app.run_code_lens_on_cursor_line());
        app.poll_vsix();
        let execute = drain(&host)
            .into_iter()
            .find(|message| message["type"] == "execute")
            .expect("the command was sent to the host");
        assert_eq!(execute["command"], "ext.run");
        assert_eq!(execute["args"], json!([2]));
    }

    #[test]
    fn ctrl_click_at_the_end_of_the_line_runs_the_lens_but_not_in_the_text() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "word\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        answer(
            &host,
            &sent[0]["id"],
            json!({"lenses": [lens(1, Some("run"), false)]}),
        );
        app.poll_vsix();
        app.editor_mut().set_cursor_line_col(0, 2);
        assert!(!app.run_code_lens_at_line_end(), "inside the text");
        app.editor_mut().set_cursor_line_col(0, 4);
        assert!(app.run_code_lens_at_line_end());
    }

    #[test]
    fn unavailable_commands_are_reported_not_run() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        let mut other = lens(1, Some("go"), false);
        other["command"]["id"] = json!("editor.action.showReferences");
        answer(&host, &sent[0]["id"], json!({"lenses": [other]}));
        app.poll_vsix();
        app.editor_mut().set_cursor_line_col(0, 0);
        assert!(app.run_code_lens_on_cursor_line());
        assert!(
            app.status
                .contains("editor.action.showReferences is not available"),
            "{}",
            app.status
        );
    }

    #[test]
    fn graphite_commands_run_with_the_first_argument() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_lens_provider(&dir, "one\ntwo\n");
        app.poll_vsix();
        let sent = only(&drain(&host), "$provideCodeLenses");
        let mut go = lens(1, Some("go"), false);
        go["command"] = json!({"id": "graphite.editor.setCursor", "title": "go",
                               "arguments": [{"line": 1, "column": 2}]});
        answer(&host, &sent[0]["id"], json!({"lenses": [go]}));
        app.poll_vsix();
        app.editor_mut().set_cursor_line_col(0, 0);
        assert!(app.run_code_lens_on_cursor_line());
        assert_eq!(app.editor().cursor_line_col(), (1, 2));
    }

    #[test]
    fn reply_ids_are_recognised_by_range() {
        assert!(is_lens_reply(LENS_BASE));
        assert!(!is_lens_reply(LENS_END));
        assert!(!is_lens_reply(8_000_000), "hierarchy replies");
    }
}
