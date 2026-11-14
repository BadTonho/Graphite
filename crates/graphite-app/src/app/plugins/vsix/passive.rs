//! Language features an extension provides without being asked: inlay hints and document
//! highlights.
//!
//! Unlike hover or definition they are not started by a command. Each tick the drivers below
//! compare what the editor shows with what the active document needs (its revision, the cursor)
//! and ask the extension host when something is out of date. Inlay hints reuse the editor's
//! existing hint display (the same one language servers feed); highlights are drawn as a
//! built-in decoration.

use super::languages::Feature;
use super::*;
use graphite_core_api::{DocumentId, DocumentSnapshot};

/// Replies to passive requests carry ids in `PASSIVE_BASE..PASSIVE_END`.
pub(super) const PASSIVE_BASE: u64 = 6_000_000;
const PASSIVE_END: u64 = 7_000_000;
/// The cursor must rest this long before highlights are requested.
const HIGHLIGHT_DEBOUNCE: Duration = Duration::from_millis(150);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn is_passive_reply(id: u64) -> bool {
    (PASSIVE_BASE..PASSIVE_END).contains(&id)
}

/// What a request was made for. A reply is only used while the document still matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub(super) document: DocumentId,
    pub(super) revision: u64,
    pub(super) cursor: usize,
}

pub(super) struct InFlight {
    pub(super) id: u64,
    pub(super) key: Key,
    pub(super) handle: u64,
    pub(super) uri: String,
    pub(super) deadline: Instant,
}

#[derive(Default)]
pub(super) struct Passive {
    next_id: u64,
    inlay: Option<InFlight>,
    fold: Option<InFlight>,
    links_request: Option<InFlight>,
    pub(super) semantic_request: Option<InFlight>,
    pub(super) semantic_done: Option<(DocumentId, u64)>,
    /// The semantic tokens of the active document, drawn over the syntax colors.
    pub(crate) semantic: Option<super::semantic::SemanticTokens>,
    /// Revision of the document the last link request covered.
    links_done: Option<(DocumentId, u64)>,
    /// The links of the active document, for Ctrl+click.
    pub(super) links: Option<LinkSet>,
    /// Revision of the document the last folding request covered.
    fold_done: Option<(DocumentId, u64)>,
    highlight: Option<InFlight>,
    /// Revision of the document the last inlay request covered.
    inlay_done: Option<(DocumentId, u64)>,
    /// The cursor state the last highlight request or reply covered.
    highlight_done: Option<Key>,
    /// A cursor state waiting out the debounce.
    highlight_wanted: Option<(Instant, Key)>,
    /// Document whose highlights are currently drawn.
    highlight_shown: Option<String>,
}

impl Passive {
    /// Whether the event loop should keep ticking for these features.
    pub(super) fn busy(&self) -> bool {
        self.inlay.is_some()
            || self.fold.is_some()
            || self.links_request.is_some()
            || self.semantic_request.is_some()
            || self.highlight.is_some()
            || self.highlight_wanted.is_some()
    }

    pub(super) fn next_id(&mut self) -> u64 {
        self.next_id = (self.next_id + 1) % (PASSIVE_END - PASSIVE_BASE);
        PASSIVE_BASE + self.next_id
    }
}

/// The inlay hints of an `IInlayHintsDto` as language server hints.
pub(super) fn inlay_hints(result: &Value) -> Vec<lsp_types::InlayHint> {
    result["hints"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|hint| {
            let line = hint["position"]["lineNumber"].as_u64()?.checked_sub(1)?;
            let character = hint["position"]["column"].as_u64()?.checked_sub(1)?;
            let label = match &hint["label"] {
                Value::String(text) => json!(text),
                Value::Array(parts) => Value::Array(
                    parts
                        .iter()
                        .map(|part| json!({"value": part["label"].as_str().unwrap_or_default()}))
                        .collect(),
                ),
                _ => return None,
            };
            serde_json::from_value(json!({
                "position": {"line": line, "character": character},
                "label": label,
                // Code OSS and LSP number the kinds alike: 1 type, 2 parameter.
                "kind": hint["kind"].as_u64().filter(|kind| matches!(kind, 1 | 2)),
                "paddingLeft": hint["paddingLeft"].as_bool(),
                "paddingRight": hint["paddingRight"].as_bool(),
            }))
            .ok()
        })
        .collect()
}

/// The regions of an `IFoldingRangeDto[]` as 0-based start line to last line. Several regions
/// starting on one line keep the longest; regions that do not span two lines are dropped.
pub(super) fn folding_ranges(result: &Value) -> crate::app::folding::Provided {
    let mut regions = crate::app::folding::Provided::new();
    for item in result.as_array().into_iter().flatten() {
        let (Some(start), Some(end)) = (item["start"].as_u64(), item["end"].as_u64()) else {
            continue;
        };
        let (Some(start), Some(end)) = (start.checked_sub(1), end.checked_sub(1)) else {
            continue;
        };
        if end > start {
            let entry = regions.entry(start as usize).or_insert(end as usize);
            *entry = (*entry).max(end as usize);
        }
    }
    regions
}

/// The ranges of an `IDocumentHighlightDto[]`, as 0-based `((line, column), (line, column))`.
pub(super) fn highlight_ranges(result: &Value) -> Vec<((u32, u32), (u32, u32))> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let coordinate = |name: &str| {
                u32::try_from(item["range"][name].as_u64()?)
                    .ok()?
                    .checked_sub(1)
            };
            Some((
                (coordinate("startLineNumber")?, coordinate("startColumn")?),
                (coordinate("endLineNumber")?, coordinate("endColumn")?),
            ))
        })
        .collect()
}

/// One link of the active document: 0-based `(line, UTF-16 column)` start and end, and where it
/// goes (`None` when the provider leaves it to be resolved, which is not supported yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DocLink {
    pub start: (u32, u32),
    pub end: (u32, u32),
    pub target: Option<String>,
}

/// The links of one document, with the document and revision they describe.
pub(crate) struct LinkSet {
    pub uri: String,
    pub document: DocumentId,
    pub revision: u64,
    pub links: Vec<DocLink>,
}

/// The links of an `ILinksListDto`. Ranges that are empty or malformed are dropped.
pub(super) fn document_links(result: &Value) -> Vec<DocLink> {
    result["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|link| {
            let coordinate = |name: &str| {
                u32::try_from(link["range"][name].as_u64()?)
                    .ok()?
                    .checked_sub(1)
            };
            let start = (coordinate("startLineNumber")?, coordinate("startColumn")?);
            let end = (coordinate("endLineNumber")?, coordinate("endColumn")?);
            if end <= start {
                return None;
            }
            Some(DocLink {
                start,
                end,
                // The host turns URIs into strings; a plain string is also accepted.
                target: link["url"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

/// The link covering `(line, column)`, if any.
pub(super) fn link_at(links: &[DocLink], line: u32, column: u32) -> Option<&DocLink> {
    links
        .iter()
        .find(|link| link.start <= (line, column) && (line, column) < link.end)
}

impl App {
    /// Opens the extension link under the cursor of the active document. Returns whether there
    /// was one (even if its target could not be opened, which is reported in the status line).
    pub(crate) fn open_document_link_at_cursor(&mut self) -> bool {
        let document = self.active_document();
        let revision = self.documents[document].buffer.revision();
        let cursor = documents::position(
            &self.documents[document].buffer.text(),
            self.editor().cursor().char_idx,
        );
        let Some(link) = self
            .plugins
            .vsix
            .as_ref()
            .and_then(|runtime| runtime.passive.links.as_ref())
            .filter(|set| set.document == document && set.revision == revision)
            .and_then(|set| link_at(&set.links, cursor.line, cursor.character))
            .cloned()
        else {
            return false;
        };
        let Some(target) = link.target else {
            self.status =
                "This link has to be resolved by its extension, which is not supported yet".into();
            return true;
        };
        if target.starts_with("https://") {
            crate::app::plugins::drawing::open_external_https(&target);
        } else if let Ok(uri) = graphite_core_api::DocumentUri::parse(&target) {
            match uri.to_path() {
                Ok(path) if path.is_file() => self.open_file(path),
                Ok(path) => self.status = format!("{} does not exist", path.display()),
                Err(_) => self.status = format!("Cannot open {target}"),
            }
        } else {
            self.status = format!("Cannot open {target}");
        }
        true
    }
}

impl Runtime {
    pub(super) fn provider(&self, feature: Feature, document: &DocumentSnapshot) -> Option<u64> {
        self.languages.provider_for(feature, document)
    }
}

impl App {
    /// Asks the extension host for whatever inlay hints or highlights are out of date.
    pub(super) fn drive_vsix_passive(&mut self, runtime: &mut Runtime) -> Result<()> {
        if !self.vsix_workspace_trusted()
            || !runtime.languages.has_provider(&[
                Feature::InlayHints,
                Feature::DocumentHighlights,
                Feature::FoldingRanges,
                Feature::DocumentLinks,
                Feature::SemanticTokens,
            ])
        {
            return Ok(());
        }
        let now = Instant::now();
        for in_flight in [
            &mut runtime.passive.inlay,
            &mut runtime.passive.fold,
            &mut runtime.passive.links_request,
            &mut runtime.passive.semantic_request,
            &mut runtime.passive.highlight,
        ] {
            if in_flight
                .as_ref()
                .is_some_and(|request| now >= request.deadline)
            {
                if let Some(request) = in_flight.take() {
                    runtime
                        .session
                        .send(json!({"type":"cancelCall","id":request.id}))?;
                }
            }
        }
        let document = self.active_document();
        let revision = self.documents[document].buffer.revision();
        let cursor = self.editor().cursor().char_idx;
        let key = Key {
            document,
            revision,
            cursor,
        };
        let wants_inlay = self.documents[document].inlay_revision != Some(revision)
            && runtime.passive.inlay_done != Some((document, revision))
            && !runtime.passive.inlay.as_ref().is_some_and(|request| {
                request.key.revision == revision && request.key.document == document
            })
            && !self.language_server_serves_active_document();
        let wants_fold = self.documents[document]
            .provided_folds
            .as_ref()
            .map(|(revision, _)| *revision)
            != Some(revision)
            && runtime.passive.fold_done != Some((document, revision))
            && !runtime.passive.fold.as_ref().is_some_and(|request| {
                request.key.revision == revision && request.key.document == document
            });
        let wants_links = runtime.passive.links_done != Some((document, revision))
            && !runtime
                .passive
                .links_request
                .as_ref()
                .is_some_and(|request| {
                    request.key.revision == revision && request.key.document == document
                });
        let wants_semantic = runtime.passive.semantic_done != Some((document, revision))
            && !runtime
                .passive
                .semantic_request
                .as_ref()
                .is_some_and(|request| {
                    request.key.revision == revision && request.key.document == document
                });
        let wants_highlight = runtime.passive.highlight_done != Some(key)
            && runtime
                .passive
                .highlight
                .as_ref()
                .map(|request| request.key)
                != Some(key);
        if !wants_inlay && !wants_highlight && !wants_fold && !wants_links && !wants_semantic {
            if runtime.passive.highlight_done == Some(key) {
                runtime.passive.highlight_wanted = None;
            }
            return Ok(());
        }
        let snapshot = crate::app::services::snapshot_document(document, &self.documents[document])
            .map_err(|error| error.to_string())?;
        let uri = snapshot.uri.as_str().to_owned();
        if wants_inlay {
            self.request_vsix_inlay_hints(runtime, &snapshot, key)?;
        }
        if wants_fold {
            self.request_vsix_folds(runtime, &snapshot, key)?;
        }
        if wants_links {
            self.request_vsix_links(runtime, &snapshot, key)?;
        }
        if wants_semantic {
            self.request_vsix_semantic_tokens(runtime, &snapshot, key)?;
        }
        if wants_highlight {
            // A stale highlight would point at the wrong text, so drop it right away.
            if runtime
                .passive
                .highlight_done
                .is_some_and(|done| done != key)
            {
                if let Some(shown) = runtime.passive.highlight_shown.take() {
                    runtime.decorations.clear_highlights(&shown);
                }
            }
            match runtime.passive.highlight_wanted {
                Some((since, wanted)) if wanted == key => {
                    if now.duration_since(since) >= HIGHLIGHT_DEBOUNCE {
                        runtime.passive.highlight_wanted = None;
                        self.request_vsix_highlights(runtime, &snapshot, &uri, key)?;
                    }
                }
                _ => runtime.passive.highlight_wanted = Some((now, key)),
            }
        }
        Ok(())
    }

    fn language_server_serves_active_document(&self) -> bool {
        self.documents[self.active_document()]
            .buffer
            .path()
            .is_some_and(|path| self.lsp_manager.supports_path(path))
    }

    fn request_vsix_inlay_hints(
        &self,
        runtime: &mut Runtime,
        snapshot: &DocumentSnapshot,
        key: Key,
    ) -> Result<()> {
        let Some(handle) = runtime.provider(Feature::InlayHints, snapshot) else {
            runtime.passive.inlay_done = Some((key.document, key.revision));
            return Ok(());
        };
        if let Some(previous) = runtime.passive.inlay.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall","id":previous.id}))?;
        }
        let end = documents::position(&snapshot.text, snapshot.text.chars().count());
        let id = runtime.passive.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideInlayHints", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri}, {
                "startLineNumber": 1, "startColumn": 1,
                "endLineNumber": end.line + 1, "endColumn": end.character + 1,
            }],
        }))?;
        runtime.passive.inlay_done = Some((key.document, key.revision));
        runtime.passive.inlay = Some(InFlight {
            id,
            key,
            handle,
            uri: snapshot.uri.as_str().to_owned(),
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        Ok(())
    }

    fn request_vsix_folds(
        &self,
        runtime: &mut Runtime,
        snapshot: &DocumentSnapshot,
        key: Key,
    ) -> Result<()> {
        runtime.passive.fold_done = Some((key.document, key.revision));
        let Some(handle) = runtime.provider(Feature::FoldingRanges, snapshot) else {
            return Ok(());
        };
        if let Some(previous) = runtime.passive.fold.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall","id":previous.id}))?;
        }
        let id = runtime.passive.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideFoldingRanges", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri}, {}],
        }))?;
        runtime.passive.fold = Some(InFlight {
            id,
            key,
            handle,
            uri: snapshot.uri.as_str().to_owned(),
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        Ok(())
    }

    fn request_vsix_links(
        &self,
        runtime: &mut Runtime,
        snapshot: &DocumentSnapshot,
        key: Key,
    ) -> Result<()> {
        runtime.passive.links_done = Some((key.document, key.revision));
        // Links of an older text would underline the wrong words.
        if let Some(old) = runtime.passive.links.take() {
            runtime.decorations.clear_links(&old.uri);
        }
        let Some(handle) = runtime.provider(Feature::DocumentLinks, snapshot) else {
            return Ok(());
        };
        if let Some(previous) = runtime.passive.links_request.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall","id":previous.id}))?;
        }
        let id = runtime.passive.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideDocumentLinks", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri}],
        }))?;
        runtime.passive.links_request = Some(InFlight {
            id,
            key,
            handle,
            uri: snapshot.uri.as_str().to_owned(),
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        Ok(())
    }

    fn request_vsix_highlights(
        &self,
        runtime: &mut Runtime,
        snapshot: &DocumentSnapshot,
        uri: &str,
        key: Key,
    ) -> Result<()> {
        let Some(handle) = runtime.provider(Feature::DocumentHighlights, snapshot) else {
            runtime.passive.highlight_done = Some(key);
            return Ok(());
        };
        if let Some(previous) = runtime.passive.highlight.take() {
            runtime
                .session
                .send(json!({"type":"cancelCall","id":previous.id}))?;
        }
        let at = documents::position(&snapshot.text, key.cursor);
        let id = runtime.passive.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideDocumentHighlights", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri},
                {"lineNumber": at.line + 1, "column": at.character + 1}],
        }))?;
        runtime.passive.highlight_done = Some(key);
        runtime.passive.highlight = Some(InFlight {
            id,
            key,
            handle,
            uri: uri.to_owned(),
            deadline: Instant::now() + REQUEST_TIMEOUT,
        });
        Ok(())
    }

    /// Applies the reply of a passive request if the document has not moved on.
    pub(super) fn finish_vsix_passive(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let current = |key: &Key, app: &App| {
            app.active_document() == key.document
                && app.documents[key.document].buffer.revision() == key.revision
        };
        if runtime.passive.inlay.as_ref().is_some_and(|r| r.id == id) {
            let request = runtime.passive.inlay.take().unwrap();
            if event["error"].is_string() {
                // A provider that fails is not asked again until the document changes, or it
                // would be asked on every tick.
                return;
            }
            if !current(&request.key, self) {
                // Ask again for the document as it is now.
                runtime.passive.inlay_done = None;
                return;
            }
            if let Some(cache) = event["result"]["cacheId"].as_u64() {
                let _ = runtime.session.send(json!({
                    "type": "call", "actor": "ExtHostLanguageFeatures",
                    "method": "$releaseHints", "args": [request.handle, cache],
                }));
            }
            let tab = &mut self.documents[request.key.document];
            tab.inlay_hints = inlay_hints(&event["result"]);
            tab.inlay_revision = Some(request.key.revision);
        } else if runtime
            .passive
            .semantic_request
            .as_ref()
            .is_some_and(|r| r.id == id)
        {
            self.finish_vsix_semantic_tokens(runtime, event);
        } else if runtime
            .passive
            .links_request
            .as_ref()
            .is_some_and(|r| r.id == id)
        {
            let request = runtime.passive.links_request.take().unwrap();
            if event["error"].is_string() {
                return;
            }
            if !current(&request.key, self) {
                runtime.passive.links_done = None;
                return;
            }
            if let Some(cache) = event["result"]["cacheId"].as_u64() {
                let _ = runtime.session.send(json!({
                    "type": "call", "actor": "ExtHostLanguageFeatures",
                    "method": "$releaseDocumentLinks", "args": [request.handle, cache],
                }));
            }
            let links = document_links(&event["result"]);
            let ranges = links.iter().map(|link| (link.start, link.end)).collect();
            runtime.decorations.set_links(&request.uri, ranges);
            runtime.passive.links = Some(LinkSet {
                uri: request.uri,
                document: request.key.document,
                revision: request.key.revision,
                links,
            });
        } else if runtime.passive.fold.as_ref().is_some_and(|r| r.id == id) {
            let request = runtime.passive.fold.take().unwrap();
            if event["error"].is_string() {
                // Not asked again until the document changes.
                return;
            }
            if !current(&request.key, self) {
                runtime.passive.fold_done = None;
                return;
            }
            let regions = folding_ranges(&event["result"]);
            self.documents[request.key.document].provided_folds =
                Some((request.key.revision, regions));
        } else if runtime
            .passive
            .highlight
            .as_ref()
            .is_some_and(|r| r.id == id)
        {
            let request = runtime.passive.highlight.take().unwrap();
            let cursor_unmoved = self.editor().cursor().char_idx == request.key.cursor;
            if event["error"].is_string() || !current(&request.key, self) || !cursor_unmoved {
                return;
            }
            let ranges = highlight_ranges(&event["result"]);
            runtime.decorations.set_highlights(&request.uri, ranges);
            runtime.passive.highlight_shown = Some(request.uri);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlay_hints_convert_positions_labels_kinds_and_padding() {
        let hints = inlay_hints(&json!({"hints": [
            {"position": {"lineNumber": 3, "column": 5}, "label": ": number", "kind": 1,
             "paddingLeft": true},
            {"position": {"lineNumber": 1, "column": 1},
             "label": [{"label": "name"}, {"label": ":"}], "kind": 2, "paddingRight": true},
            {"position": {"lineNumber": 2, "column": 2}, "label": "plain", "kind": 9},
        ]}));
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].position, lsp_types::Position::new(2, 4));
        assert!(
            matches!(&hints[0].label, lsp_types::InlayHintLabel::String(text) if text == ": number")
        );
        assert_eq!(hints[0].kind, Some(lsp_types::InlayHintKind::TYPE));
        assert_eq!(hints[0].padding_left, Some(true));
        assert_eq!(hints[1].kind, Some(lsp_types::InlayHintKind::PARAMETER));
        assert_eq!(hints[1].padding_right, Some(true));
        let lsp_types::InlayHintLabel::LabelParts(parts) = &hints[1].label else {
            panic!("expected label parts");
        };
        let joined: String = parts.iter().map(|part| part.value.as_str()).collect();
        assert_eq!(joined, "name:");
        assert_eq!(hints[2].kind, None, "unknown kinds are dropped");
    }

    #[test]
    fn malformed_inlay_hints_are_skipped_not_fatal() {
        assert!(inlay_hints(&Value::Null).is_empty());
        assert!(inlay_hints(&json!({"hints": "nope"})).is_empty());
        let hints = inlay_hints(&json!({"hints": [
            {"position": {"lineNumber": 0, "column": 1}, "label": "zero line"},
            {"position": {"lineNumber": 1}, "label": "no column"},
            {"position": {"lineNumber": 1, "column": 1}},
            {"position": {"lineNumber": 1, "column": 1}, "label": 5},
            {"position": {"lineNumber": 1, "column": 1}, "label": "ok"},
        ]}));
        assert_eq!(hints.len(), 1);
        assert!(matches!(&hints[0].label, lsp_types::InlayHintLabel::String(text) if text == "ok"));
    }

    #[test]
    fn highlight_ranges_are_zero_based_and_skip_bad_entries() {
        let ranges = highlight_ranges(&json!([
            {"range": {"startLineNumber": 1, "startColumn": 2, "endLineNumber": 1, "endColumn": 5}, "kind": 2},
            {"range": {"startLineNumber": 0, "startColumn": 1, "endLineNumber": 1, "endColumn": 1}},
            {"range": {"startLineNumber": 4, "startColumn": 1}},
            {"kind": 1},
        ]));
        assert_eq!(ranges, [((0, 1), (0, 4))]);
        assert!(highlight_ranges(&Value::Null).is_empty());
    }

    #[test]
    fn reply_ids_are_recognised_by_range() {
        assert!(is_passive_reply(PASSIVE_BASE));
        assert!(is_passive_reply(PASSIVE_END - 1));
        assert!(!is_passive_reply(PASSIVE_END));
        assert!(!is_passive_reply(5_000_000), "the debug adapter reply");
        let mut passive = Passive::default();
        let first = passive.next_id();
        assert_ne!(first, passive.next_id());
        assert!(is_passive_reply(first));
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;

    /// A trusted app with `a.txt` open, a loopback extension host and these providers registered.
    fn app_with_providers(dir: &TempDir, text: &str, providers: &[&str]) -> (App, LoopbackHost) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.run_ui_command("graphite.security.trustWorkspace");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, text).unwrap();
        app.open_file(file);
        let (session, host) = Session::loopback();
        let mut runtime = Runtime::for_test(session);
        for (index, method) in providers.iter().enumerate() {
            languages::language_rpc(
                &mut runtime.languages,
                "MainThreadLanguageFeatures",
                method,
                &[json!(index + 1), json!([{"language": "*"}])],
            )
            .unwrap()
            .unwrap();
        }
        app.plugins.vsix = Some(runtime);
        (app, host)
    }

    /// Everything Graphite sent to the host so far that calls `method`.
    fn calls(host: &LoopbackHost, method: &str) -> Vec<Value> {
        std::iter::from_fn(|| host.next_sent())
            .filter(|message| message["method"] == method)
            .collect()
    }

    fn reply(host: &LoopbackHost, id: &Value, result: Value) {
        host.emit(json!({"type": "result", "id": id, "result": result}));
    }

    const INLAY: &str = "$registerInlayHintsProvider";
    const HIGHLIGHT: &str = "$registerDocumentHighlightProvider";
    const RANGE: &str = "$registerRangeFormattingSupport";

    fn hint(line: u64, column: u64, label: &str) -> Value {
        json!({"hints": [{"position": {"lineNumber": line, "column": column}, "label": label}]})
    }

    #[test]
    fn inlay_hints_are_requested_for_the_document_and_shown_once_answered() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "let x = 1;\nlet y = 2;\n", &[INLAY]);
        app.poll_vsix();
        let sent = calls(&host, "$provideInlayHints");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 1, "the provider handle");
        assert_eq!(
            sent[0]["args"][2]["endLineNumber"], 3,
            "covers the whole document"
        );
        assert_eq!(sent[0]["cancellable"], true);

        reply(&host, &sent[0]["id"], hint(1, 6, ": number"));
        app.poll_vsix();
        let tab = &app.documents[app.active_document()];
        assert_eq!(tab.inlay_hints.len(), 1);
        assert_eq!(tab.inlay_hints[0].position, lsp_types::Position::new(0, 5));
        assert_eq!(tab.inlay_revision, Some(tab.buffer.revision()));

        app.poll_vsix();
        assert!(
            calls(&host, "$provideInlayHints").is_empty(),
            "an unchanged document is not asked again"
        );
    }

    #[test]
    fn editing_the_document_asks_for_fresh_inlay_hints() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "let x = 1;\n", &[INLAY]);
        app.poll_vsix();
        let first = calls(&host, "$provideInlayHints");
        reply(&host, &first[0]["id"], hint(1, 6, ": number"));
        app.poll_vsix();

        app.editor_mut().insert("// note\n");
        app.poll_vsix();
        let second = calls(&host, "$provideInlayHints");
        assert_eq!(second.len(), 1, "the edit changed the revision");
        assert_ne!(second[0]["id"], first[0]["id"]);
    }

    #[test]
    fn an_answer_for_an_outdated_revision_is_dropped_and_asked_again() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "let x = 1;\n", &[INLAY]);
        app.poll_vsix();
        let stale = calls(&host, "$provideInlayHints");
        app.editor_mut().insert("// note\n");
        reply(&host, &stale[0]["id"], hint(1, 6, "stale"));
        app.poll_vsix();
        let tab = &app.documents[app.active_document()];
        assert!(
            tab.inlay_hints.is_empty(),
            "the stale hints must not be shown"
        );
        // The poll that dropped the answer also asked for the current revision.
        app.poll_vsix();
        assert_eq!(calls(&host, "$provideInlayHints").len(), 1);
    }

    #[test]
    fn a_failing_inlay_provider_is_not_asked_again_until_the_document_changes() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "x\n", &[INLAY]);
        app.poll_vsix();
        let sent = calls(&host, "$provideInlayHints");
        host.emit(json!({"type": "result", "id": sent[0]["id"], "error": "boom"}));
        app.poll_vsix();
        assert!(app.documents[app.active_document()].inlay_hints.is_empty());
        app.poll_vsix();
        app.poll_vsix();
        app.poll_vsix();
        assert!(
            calls(&host, "$provideInlayHints").is_empty(),
            "a failing provider must not be asked on every tick"
        );
        app.editor_mut().insert("y");
        app.poll_vsix();
        assert_eq!(
            calls(&host, "$provideInlayHints").len(),
            1,
            "asked after an edit"
        );
    }

    #[test]
    fn nothing_is_requested_without_a_provider_or_in_an_untrusted_workspace() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "x\n", &[]);
        app.poll_vsix();
        app.poll_vsix();
        assert!(std::iter::from_fn(|| host.next_sent())
            .all(|message| message["method"] != "$provideInlayHints"
                && message["method"] != "$provideDocumentHighlights"));

        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "x\n", &[INLAY, HIGHLIGHT]);
        app.workspace_trust = Some(crate::app::phase5::WorkspaceTrust::restricted(
            dir.path().join("restricted.json"),
        ));
        app.poll_vsix();
        assert!(std::iter::from_fn(|| host.next_sent()).next().is_none());
    }

    fn range(line: u64, from: u64, to: u64) -> Value {
        json!({"startLineNumber": line, "startColumn": from, "endLineNumber": line, "endColumn": to})
    }

    #[test]
    fn highlights_wait_for_the_cursor_to_rest_and_are_drawn_when_answered() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "foo bar foo\n", &[HIGHLIGHT]);
        app.editor_mut().set_cursor_line_col(0, 1);
        app.poll_vsix();
        assert!(
            calls(&host, "$provideDocumentHighlights").is_empty(),
            "the debounce has not elapsed"
        );
        std::thread::sleep(HIGHLIGHT_DEBOUNCE + Duration::from_millis(60));
        app.poll_vsix();
        let sent = calls(&host, "$provideDocumentHighlights");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][2], json!({"lineNumber": 1, "column": 2}));

        reply(
            &host,
            &sent[0]["id"],
            json!([{"range": range(1, 1, 4)}, {"range": range(1, 9, 12)}]),
        );
        app.poll_vsix();
        let uri = graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned();
        let marks = app.vsix_line_decorations(&uri, 0, true);
        assert_eq!(marks.len(), 2, "both occurrences are marked");
        assert_eq!(marks[0].start, Some(0));
        assert_eq!(marks[0].end, Some(3));
        assert!(marks[0].look.background.is_some());
    }

    #[test]
    fn moving_the_cursor_clears_stale_highlights_and_drops_late_answers() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "foo bar foo\n", &[HIGHLIGHT]);
        let uri = graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned();
        app.editor_mut().set_cursor_line_col(0, 1);
        app.poll_vsix();
        std::thread::sleep(HIGHLIGHT_DEBOUNCE + Duration::from_millis(60));
        app.poll_vsix();
        let first = calls(&host, "$provideDocumentHighlights");
        reply(&host, &first[0]["id"], json!([{"range": range(1, 1, 4)}]));
        app.poll_vsix();
        assert_eq!(app.vsix_line_decorations(&uri, 0, true).len(), 1);

        app.editor_mut().set_cursor_line_col(0, 5);
        app.poll_vsix();
        assert!(
            app.vsix_line_decorations(&uri, 0, true).is_empty(),
            "highlights for the old cursor position are removed at once"
        );
        std::thread::sleep(HIGHLIGHT_DEBOUNCE + Duration::from_millis(60));
        app.poll_vsix();
        let second = calls(&host, "$provideDocumentHighlights");
        assert_eq!(second.len(), 1);
        app.editor_mut().set_cursor_line_col(0, 9);
        reply(&host, &second[0]["id"], json!([{"range": range(1, 5, 8)}]));
        app.poll_vsix();
        assert!(
            app.vsix_line_decorations(&uri, 0, true).is_empty(),
            "an answer for a position the cursor left is not drawn"
        );
    }

    #[test]
    fn range_formatting_sends_the_selection_and_applies_the_edits() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "a  b\nc  d\n", &[RANGE]);
        app.editor_mut().set_selection(5, 9);
        assert!(app.request_vsix_language(languages::Feature::FormatRange));
        let sent = calls(&host, "$provideDocumentRangeFormattingEdits");
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0]["args"][2],
            json!({"startLineNumber": 2, "startColumn": 1, "endLineNumber": 2, "endColumn": 5})
        );
        assert_eq!(sent[0]["args"][3]["tabSize"], app.tab_size);
        reply(
            &host,
            &sent[0]["id"],
            json!([{"range": range(2, 2, 4), "text": " "}]),
        );
        app.poll_vsix();
        assert_eq!(app.editor().text(), "a  b\nc d\n", "{}", app.status);
    }

    #[test]
    fn range_formatting_without_a_selection_formats_the_cursor_line() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "one\ntwo  words\nthree\n", &[RANGE]);
        app.editor_mut().set_cursor_line_col(1, 2);
        assert!(app.request_vsix_language(languages::Feature::FormatRange));
        let sent = calls(&host, "$provideDocumentRangeFormattingEdits");
        assert_eq!(sent[0]["args"][2], range(2, 1, 11));
    }

    #[test]
    fn range_formatting_is_unavailable_without_a_provider() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_providers(&dir, "x\n", &[INLAY]);
        assert!(!app.request_vsix_language(languages::Feature::FormatRange));
    }

    #[test]
    fn line_ranges_count_utf16_units_and_ignore_line_endings() {
        assert_eq!(languages::line_range("héllo\r\nx\n", 1), range(1, 1, 6));
        assert_eq!(languages::line_range("a🌎\n", 1), range(1, 1, 4));
        assert_eq!(
            languages::line_range("a\n", 2),
            range(2, 1, 1),
            "the empty last line"
        );
        assert_eq!(
            languages::line_range("a\n", 9),
            range(9, 1, 1),
            "past the end"
        );
    }

    #[test]
    fn folding_ranges_convert_to_zero_based_regions_and_keep_the_longest() {
        let regions = folding_ranges(&json!([
            {"start": 1, "end": 3},
            {"start": 1, "end": 6, "kind": 3},
            {"start": 4, "end": 4},
            {"start": 0, "end": 5},
            {"start": 7},
            {"end": 9},
            "nope",
        ]));
        assert_eq!(regions.into_iter().collect::<Vec<_>>(), [(0, 5)]);
        assert!(folding_ranges(&Value::Null).is_empty());
    }

    const FOLD: &str = "$registerFoldingRangeProvider";

    #[test]
    fn folding_ranges_are_requested_stored_and_used_by_the_fold_commands() {
        let dir = TempDir::new();
        // No indentation, so the built-in rules find nothing to fold here.
        let (mut app, host) = app_with_providers(&dir, "one\ntwo\nthree\nfour\n", &[FOLD]);
        app.poll_vsix();
        let sent = calls(&host, "$provideFoldingRanges");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 1);

        app.editor_mut().set_cursor_line_col(0, 0);
        app.fold_at_cursor();
        assert!(
            app.editor_groups.active_view().folded.is_empty(),
            "nothing to fold until the extension answers"
        );

        reply(
            &host,
            &sent[0]["id"],
            json!([{"start": 1, "end": 3}, {"start": 2, "end": 2}]),
        );
        app.poll_vsix();
        let tab = &app.documents[app.active_document()];
        assert_eq!(tab.provided_folds().unwrap().len(), 1);

        app.fold_at_cursor();
        assert_eq!(app.editor_groups.active_view().folded, [0]);
        app.poll_vsix();
        assert!(
            calls(&host, "$provideFoldingRanges").is_empty(),
            "the answer is reused while the text is unchanged"
        );
    }

    #[test]
    fn provided_regions_describe_one_revision_and_are_asked_again_after_an_edit() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "a\nb\nc\n", &[FOLD]);
        app.poll_vsix();
        let first = calls(&host, "$provideFoldingRanges");
        reply(&host, &first[0]["id"], json!([{"start": 1, "end": 3}]));
        app.poll_vsix();
        assert!(app.documents[app.active_document()]
            .provided_folds()
            .is_some());

        app.editor_mut().insert("x");
        assert!(
            app.documents[app.active_document()]
                .provided_folds()
                .is_none(),
            "regions for an older text must not be used"
        );
        app.poll_vsix();
        assert_eq!(calls(&host, "$provideFoldingRanges").len(), 1);
    }

    #[test]
    fn a_failing_folding_provider_is_not_asked_again_until_the_document_changes() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "a\nb\n", &[FOLD]);
        app.poll_vsix();
        let sent = calls(&host, "$provideFoldingRanges");
        host.emit(json!({"type": "result", "id": sent[0]["id"], "error": "boom"}));
        app.poll_vsix();
        app.poll_vsix();
        assert!(calls(&host, "$provideFoldingRanges").is_empty());
        assert!(app.documents[app.active_document()]
            .provided_folds()
            .is_none());
    }

    /// An app with `text` open and an on-type formatter for `triggers`.
    fn app_with_on_type_formatter(
        dir: &TempDir,
        text: &str,
        triggers: &[&str],
    ) -> (App, LoopbackHost) {
        let (mut app, host) = app_with_providers(dir, text, &[]);
        let runtime = app.plugins.vsix.as_mut().unwrap();
        languages::language_rpc(
            &mut runtime.languages,
            "MainThreadLanguageFeatures",
            "$registerOnTypeFormattingSupport",
            &[
                json!(7),
                json!([{"language": "*"}]),
                json!(triggers),
                json!("pub.ext"),
            ],
        )
        .unwrap()
        .unwrap();
        (app, host)
    }

    #[test]
    fn typing_a_trigger_character_asks_the_extension_to_format_and_applies_the_edits() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_on_type_formatter(&dir, "let x=1\n", &[";", "}"]);
        app.format_on_type = true;
        app.editor_mut().set_cursor_line_col(0, 7);
        app.editor_mut().insert(";");
        app.format_on_type(";");
        let sent = calls(&host, "$provideOnTypeFormattingEdits");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 7);
        assert_eq!(
            sent[0]["args"][2],
            json!({"lineNumber": 1, "column": 9}),
            "after the typed ;"
        );
        assert_eq!(sent[0]["args"][3], ";");
        assert_eq!(sent[0]["args"][4]["tabSize"], app.tab_size);

        reply(
            &host,
            &sent[0]["id"],
            json!([{"range": range(1, 6, 7), "text": " = "}]),
        );
        app.poll_vsix();
        assert_eq!(app.editor().text(), "let x = 1;\n", "{}", app.status);
    }

    #[test]
    fn only_trigger_characters_with_the_setting_on_are_sent() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_on_type_formatter(&dir, "x\n", &[";"]);
        app.format_on_type = true;
        app.format_on_type("a");
        assert!(
            calls(&host, "$provideOnTypeFormattingEdits").is_empty(),
            "not a trigger"
        );
        app.format_on_type(";;");
        assert!(
            calls(&host, "$provideOnTypeFormattingEdits").is_empty(),
            "pasted text, not typing"
        );
        app.format_on_type("");
        assert!(calls(&host, "$provideOnTypeFormattingEdits").is_empty());

        app.format_on_type = false;
        app.format_on_type(";");
        assert!(
            calls(&host, "$provideOnTypeFormattingEdits").is_empty(),
            "editor.formatOnType is off by default"
        );
    }

    #[test]
    fn on_type_formatting_needs_a_provider_and_a_single_cursor() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "x\n", &[INLAY]);
        app.format_on_type = true;
        app.format_on_type(";");
        assert!(
            calls(&host, "$provideOnTypeFormattingEdits").is_empty(),
            "no provider"
        );

        let (mut app, host) = app_with_on_type_formatter(&dir, "ab\n", &[";"]);
        app.format_on_type = true;
        app.editor_mut().set_selections(vec![
            graphite_text::Selection { anchor: 0, head: 0 },
            graphite_text::Selection { anchor: 1, head: 1 },
        ]);
        app.format_on_type(";");
        assert!(
            calls(&host, "$provideOnTypeFormattingEdits").is_empty(),
            "several cursors"
        );
    }

    const LINKS: &str = "$registerDocumentLinkProvider";

    fn link(range: Value, url: Option<&str>) -> Value {
        let mut link = json!({ "range": range });
        if let Some(url) = url {
            link["url"] = json!(url);
        }
        link
    }

    #[test]
    fn document_links_are_converted_and_bad_ones_dropped() {
        let links = document_links(&json!({"links": [
            link(range(1, 5, 12), Some("https://example.com")),
            link(range(2, 1, 4), None),
            link(range(3, 4, 4), Some("https://empty.example")),
            {"url": "https://norange.example"},
        ], "cacheId": 3}));
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].start, (0, 4));
        assert_eq!(links[0].end, (0, 11));
        assert_eq!(links[0].target.as_deref(), Some("https://example.com"));
        assert_eq!(links[1].target, None, "left for the extension to resolve");
        assert!(document_links(&Value::Null).is_empty());
    }

    #[test]
    fn link_at_matches_the_half_open_range() {
        let links = vec![DocLink {
            start: (1, 4),
            end: (1, 9),
            target: None,
        }];
        assert!(link_at(&links, 1, 3).is_none());
        assert!(link_at(&links, 1, 4).is_some());
        assert!(link_at(&links, 1, 8).is_some());
        assert!(link_at(&links, 1, 9).is_none(), "the end is exclusive");
        assert!(link_at(&links, 0, 5).is_none());
    }

    #[test]
    fn links_are_requested_underlined_and_followed_with_a_click() {
        let dir = TempDir::new();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "linked file\n").unwrap();
        let target_uri = graphite_core_api::DocumentUri::from_path(&target).unwrap();
        let (mut app, host) = app_with_providers(&dir, "see other.txt now\n", &[LINKS]);
        app.poll_vsix();
        let sent = calls(&host, "$provideDocumentLinks");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 1);

        reply(
            &host,
            &sent[0]["id"],
            json!({"links": [link(range(1, 5, 14), Some(target_uri.as_str()))], "cacheId": 9}),
        );
        app.poll_vsix();
        let uri = graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned();
        let marks = app.vsix_line_decorations(&uri, 0, true);
        assert_eq!(marks.len(), 1);
        assert!(marks[0].look.underline.is_some(), "the link is underlined");
        assert_eq!((marks[0].start, marks[0].end), (Some(4), Some(13)));
        let released = calls(&host, "$releaseDocumentLinks");
        assert_eq!(released[0]["args"], json!([1, 9]));

        // A click outside any link does nothing; inside it opens the target file.
        app.editor_mut().set_cursor_line_col(0, 1);
        assert!(!app.open_document_link_at_cursor());
        app.editor_mut().set_cursor_line_col(0, 6);
        assert!(app.open_document_link_at_cursor());
        assert_eq!(app.editor().text(), "linked file\n", "{}", app.status);
    }

    #[test]
    fn editing_clears_the_underlines_and_asks_again() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "x https://a.example\n", &[LINKS]);
        app.poll_vsix();
        let first = calls(&host, "$provideDocumentLinks");
        reply(
            &host,
            &first[0]["id"],
            json!({"links": [link(range(1, 3, 21), Some("https://a.example"))]}),
        );
        app.poll_vsix();
        let uri = graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned();
        assert_eq!(app.vsix_line_decorations(&uri, 0, true).len(), 1);

        app.editor_mut().insert("yy");
        assert!(
            !app.open_document_link_at_cursor(),
            "links for an older text must not be followed"
        );
        app.poll_vsix();
        assert!(app.vsix_line_decorations(&uri, 0, true).is_empty());
        assert_eq!(
            calls(&host, "$provideDocumentLinks").len(),
            1,
            "asked for the new text"
        );
    }

    #[test]
    fn unresolved_links_and_missing_files_are_reported_not_opened() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_providers(&dir, "aaaa bbbb\n", &[LINKS]);
        app.poll_vsix();
        let sent = calls(&host, "$provideDocumentLinks");
        let missing =
            graphite_core_api::DocumentUri::from_path(&dir.path().join("missing.txt")).unwrap();
        reply(
            &host,
            &sent[0]["id"],
            json!({"links": [link(range(1, 1, 5), None), link(range(1, 6, 10), Some(missing.as_str()))]}),
        );
        app.poll_vsix();
        app.editor_mut().set_cursor_line_col(0, 1);
        assert!(app.open_document_link_at_cursor());
        assert!(app.status.contains("has to be resolved"), "{}", app.status);
        app.editor_mut().set_cursor_line_col(0, 7);
        assert!(app.open_document_link_at_cursor());
        assert!(app.status.contains("does not exist"), "{}", app.status);
    }

    /// A semantic token provider whose legend is `keyword`, `variable`.
    fn app_with_semantic_tokens(dir: &TempDir, text: &str) -> (App, LoopbackHost) {
        let (mut app, host) = app_with_providers(dir, text, &[]);
        let runtime = app.plugins.vsix.as_mut().unwrap();
        languages::language_rpc(
            &mut runtime.languages,
            "MainThreadLanguageFeatures",
            "$registerDocumentSemanticTokensProvider",
            &[
                json!(5),
                json!([{"language": "*"}]),
                json!({"tokenTypes": ["keyword", "variable"], "tokenModifiers": []}),
                json!(null),
            ],
        )
        .unwrap()
        .unwrap();
        (app, host)
    }

    fn doc_uri(dir: &TempDir) -> String {
        graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned()
    }

    #[test]
    fn semantic_tokens_are_requested_and_drawn_over_the_syntax_colors() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_semantic_tokens(&dir, "let x\nx\n");
        app.poll_vsix();
        let sent = calls(&host, "$provideDocumentSemanticTokens");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 5);
        assert_eq!(sent[0]["args"][2], 0, "no previous result id");

        // `let` (keyword) at 0..3 and `x` (variable) at 4..5 on line 0; `x` again on line 1.
        reply(
            &host,
            &sent[0]["id"],
            json!({"$u32": [9, 1, 15, 0, 0, 3, 0, 0, 0, 4, 1, 1, 0, 1, 0, 1, 1, 0]}),
        );
        app.poll_vsix();
        let revision = app.documents[app.active_document()].buffer.revision();
        let uri = doc_uri(&dir);
        let first = app.semantic_spans(&uri, revision, 0, "let x\n");
        let classes: Vec<_> = first.iter().map(|s| (s.range.clone(), s.class)).collect();
        assert_eq!(
            classes,
            [
                (0..3, graphite_syntax::Class::Keyword),
                (4..5, graphite_syntax::Class::Variable)
            ]
        );
        assert_eq!(app.semantic_spans(&uri, revision, 1, "x\n").len(), 1);
        assert!(app.semantic_spans(&uri, revision, 2, "").is_empty());
        assert!(
            app.semantic_spans(&uri, revision + 1, 0, "let x\n")
                .is_empty(),
            "tokens for an older text are not drawn"
        );
        assert!(app
            .semantic_spans("file:///other", revision, 0, "let x\n")
            .is_empty());

        app.poll_vsix();
        assert!(
            calls(&host, "$provideDocumentSemanticTokens").is_empty(),
            "reused while unchanged"
        );
    }

    #[test]
    fn an_edit_asks_again_and_a_delta_answer_is_ignored() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_semantic_tokens(&dir, "x\n");
        app.poll_vsix();
        let first = calls(&host, "$provideDocumentSemanticTokens");
        // A delta answer (kind 2) is not understood: no tokens are kept.
        reply(&host, &first[0]["id"], json!({"$u32": [1, 2, 0]}));
        app.poll_vsix();
        let revision = app.documents[app.active_document()].buffer.revision();
        assert!(app
            .semantic_spans(&doc_uri(&dir), revision, 0, "x\n")
            .is_empty());

        app.editor_mut().insert("y");
        app.poll_vsix();
        assert_eq!(calls(&host, "$provideDocumentSemanticTokens").len(), 1);
    }

    #[test]
    fn a_failing_semantic_provider_is_not_asked_again_until_the_text_changes() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_semantic_tokens(&dir, "x\n");
        app.poll_vsix();
        let sent = calls(&host, "$provideDocumentSemanticTokens");
        host.emit(json!({"type": "result", "id": sent[0]["id"], "error": "boom"}));
        app.poll_vsix();
        app.poll_vsix();
        assert!(calls(&host, "$provideDocumentSemanticTokens").is_empty());
    }

    #[test]
    fn semantic_spans_replace_syntax_classes_in_the_overlay() {
        use crate::app::drawing_helpers::overlay_spans;
        use graphite_syntax::{Class, Span};
        let base = [
            Span {
                range: 0..3,
                class: Class::Keyword,
            },
            Span {
                range: 4..9,
                class: Class::String,
            },
        ];
        let top = [Span {
            range: 2..6,
            class: Class::Variable,
        }];
        let merged = overlay_spans("let \"abc\"", &base, &top);
        let classes: Vec<_> = merged.iter().map(|s| (s.range.clone(), s.class)).collect();
        assert_eq!(
            classes,
            [
                (0..2, Class::Keyword),
                (2..6, Class::Variable),
                (6..9, Class::String)
            ]
        );
        assert!(overlay_spans("", &[], &[]).is_empty());
    }

    #[test]
    fn language_server_tokens_are_drawn_unless_an_extension_provides_its_own() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_providers(&dir, "let x\n", &[]);
        let revision = app.documents[app.active_document()].buffer.revision();
        let uri = doc_uri(&dir);
        app.lsp_semantic = Some(crate::semantic_tokens::DocumentTokens {
            uri: uri.clone(),
            revision,
            lines: [(0usize, vec![(0u32, 3u32, graphite_syntax::Class::Keyword)])]
                .into_iter()
                .collect(),
        });
        let spans = app.semantic_spans(&uri, revision, 0, "let x\n");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].range, 0..3);
        assert!(
            app.semantic_spans(&uri, revision + 1, 0, "let x\n")
                .is_empty(),
            "stale"
        );
        assert!(app
            .semantic_spans("file:///other", revision, 0, "let x\n")
            .is_empty());
        assert!(
            app.semantic_spans(&uri, revision, 1, "x\n").is_empty(),
            "no tokens on that line"
        );
    }
}
