//! Color swatches from extensions (`registerColorProvider`): CSS-like colors in the text are
//! tinted with the color itself, and outlined, so they can be read at a glance.
//!
//! Graphite has no inline widgets, so instead of a small square before the text the range is
//! drawn with a translucent background in the color and a border.
//!
//! The **Show color picker** command works on the color at the cursor: it asks the provider for
//! the ways to write that color (`provideColorPresentations`), lets the user pick one or type
//! another color, and applies the presentation's edit to the text.

use super::decorations::Color;
use super::languages::Feature;
use super::*;
use graphite_core_api::DocumentId;

/// Replies to color requests carry ids in `COLOR_BASE..COLOR_END`.
const COLOR_BASE: u64 = 10_000_000;
const COLOR_END: u64 = 11_000_000;
const TIMEOUT: Duration = Duration::from_secs(10);
/// Opacity of the tint behind the text.
const TINT_ALPHA: f32 = 0.55;

pub(super) fn is_color_reply(id: u64) -> bool {
    (COLOR_BASE..COLOR_END).contains(&id)
}

/// One color range: 0-based `(line, UTF-16 column)` start and end, and its RGBA channels (0..=1).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColorRange {
    pub start: (u32, u32),
    pub end: (u32, u32),
    pub color: Color,
}

struct Request {
    id: u64,
    document: DocumentId,
    revision: u64,
    uri: String,
    deadline: Instant,
}

/// One way to write a color, as the provider offers it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Presentation {
    pub label: String,
    /// The edits that write it: the main edit and any additional ones, as 1-based ranges.
    pub edits: Vec<Value>,
}

/// A color picker in progress.
#[derive(Clone, Debug)]
pub(super) struct Picker {
    uri: String,
    document: DocumentId,
    revision: u64,
    handle: u64,
    range: ColorRange,
    /// The presentation request in flight.
    request: Option<u64>,
    presentations: Vec<Presentation>,
}

#[derive(Default)]
pub(super) struct Colors {
    next_id: u64,
    request: Option<Request>,
    done: Option<(DocumentId, u64)>,
    /// Document whose swatches are drawn.
    shown: Option<String>,
    pub(super) picker: Option<Picker>,
}

impl Colors {
    pub(super) fn busy(&self) -> bool {
        self.request.is_some()
            || self
                .picker
                .as_ref()
                .is_some_and(|picker| picker.request.is_some())
    }

    fn next_id(&mut self) -> u64 {
        self.next_id = (self.next_id + 1) % (COLOR_END - COLOR_BASE);
        COLOR_BASE + self.next_id
    }
}

/// The ranges of an `IRawColorInfo[]`: `{ color: [r, g, b, a], range }`. Entries with a missing
/// range, wrong channel count or an empty range are dropped; channels are clamped to 0..=1.
pub(super) fn color_ranges(result: &Value) -> Vec<ColorRange> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let channels = item["color"].as_array().filter(|c| c.len() == 4)?;
            let mut color = [0.0f32; 4];
            for (slot, channel) in color.iter_mut().zip(channels) {
                *slot = (channel.as_f64()? as f32).clamp(0.0, 1.0);
            }
            let coordinate = |name: &str| {
                u32::try_from(item["range"][name].as_u64()?)
                    .ok()?
                    .checked_sub(1)
            };
            let start = (coordinate("startLineNumber")?, coordinate("startColumn")?);
            let end = (coordinate("endLineNumber")?, coordinate("endColumn")?);
            (end > start).then_some(ColorRange { start, end, color })
        })
        .collect()
}

/// The presentations of an `IColorPresentationDto[]`.
pub(super) fn presentations(result: &Value) -> Vec<Presentation> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let label = item["label"]
                .as_str()
                .filter(|label| !label.is_empty())?
                .to_owned();
            let mut edits = Vec::new();
            if item["textEdit"].is_object() {
                edits.push(item["textEdit"].clone());
            }
            edits.extend(
                item["additionalTextEdits"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
            Some(Presentation { label, edits })
        })
        .collect()
}

/// The range of a color as the 1-based `IRange` the extension host expects.
fn range_dto(range: &ColorRange) -> Value {
    json!({
        "startLineNumber": range.start.0 + 1, "startColumn": range.start.1 + 1,
        "endLineNumber": range.end.0 + 1, "endColumn": range.end.1 + 1,
    })
}

/// The color as the `#rrggbb` (or `#rrggbbaa`) text the picker offers for editing.
pub(super) fn hex_of(color: Color) -> String {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    let [r, g, b, a] = color;
    if a < 1.0 {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            byte(r),
            byte(g),
            byte(b),
            byte(a)
        )
    } else {
        format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
    }
}

/// The background tint for a swatch: the color itself, translucent so the text stays readable.
pub(super) fn tint(color: Color) -> Color {
    [color[0], color[1], color[2], TINT_ALPHA * color[3]]
}

impl App {
    /// Asks for the colors of the active document when they are out of date.
    pub(super) fn drive_vsix_colors(&mut self, runtime: &mut Runtime) -> Result<()> {
        if !self.vsix_workspace_trusted()
            || !runtime.languages.has_provider(&[Feature::DocumentColors])
        {
            return Ok(());
        }
        let now = Instant::now();
        if runtime
            .colors
            .request
            .as_ref()
            .is_some_and(|request| now >= request.deadline)
        {
            runtime.colors.request = None;
        }
        let document = self.active_document();
        let revision = self.documents[document].buffer.revision();
        if runtime.colors.done == Some((document, revision)) || runtime.colors.request.is_some() {
            return Ok(());
        }
        let snapshot = crate::app::services::snapshot_document(document, &self.documents[document])
            .map_err(|error| error.to_string())?;
        runtime.colors.done = Some((document, revision));
        // Swatches of an older text would tint the wrong words.
        if let Some(shown) = runtime.colors.shown.take() {
            runtime.decorations.clear_colors(&shown);
        }
        let Some(handle) = runtime.provider(Feature::DocumentColors, &snapshot) else {
            return Ok(());
        };
        let id = runtime.colors.next_id();
        runtime.session.send(json!({
            "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
            "method": "$provideDocumentColors", "cancellable": true,
            "args": [handle, {"$graphiteUri": snapshot.uri}],
        }))?;
        runtime.colors.request = Some(Request {
            id,
            document,
            revision,
            uri: snapshot.uri.as_str().to_owned(),
            deadline: now + TIMEOUT,
        });
        Ok(())
    }

    /// Starts the color picker on the color at the cursor of the active document.
    pub(crate) fn start_color_picker(&mut self) -> std::result::Result<(), String> {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return Err(self.t("color.no_provider").to_owned());
        };
        let outcome = self.start_color_picker_in(&mut runtime);
        self.plugins.vsix = Some(runtime);
        outcome
    }

    fn start_color_picker_in(&mut self, runtime: &mut Runtime) -> std::result::Result<(), String> {
        if !Self::vsix_prompt_is_free(runtime) {
            return Err("Another extension prompt is open".into());
        }
        let document = self.active_document();
        let snapshot = crate::app::services::snapshot_document(document, &self.documents[document])
            .map_err(|error| error.to_string())?;
        let (line, column) = self.editor().cursor_line_col();
        let text = self.editor().line(line);
        let utf16: usize = text.chars().take(column).map(char::len_utf16).sum();
        let uri = snapshot.uri.as_str().to_owned();
        let range = runtime
            .decorations
            .color_at(&uri, line as u32, utf16 as u32)
            .ok_or_else(|| self.t("color.none_at_cursor").to_owned())?;
        let handle = runtime
            .provider(Feature::DocumentColors, &snapshot)
            .ok_or_else(|| self.t("color.no_provider").to_owned())?;
        let picker = Picker {
            uri,
            document,
            revision: self.documents[document].buffer.revision(),
            handle,
            range,
            request: None,
            presentations: Vec::new(),
        };
        self.request_color_presentations(runtime, picker)
    }

    /// Asks the provider how the picker's color can be written.
    fn request_color_presentations(
        &mut self,
        runtime: &mut Runtime,
        mut picker: Picker,
    ) -> std::result::Result<(), String> {
        let id = runtime.colors.next_id();
        runtime
            .session
            .send(json!({
                "type": "call", "id": id, "actor": "ExtHostLanguageFeatures",
                "method": "$provideColorPresentations", "cancellable": true,
                "args": [picker.handle, {"$graphiteUri": picker.uri},
                    {"color": picker.range.color, "range": range_dto(&picker.range)}],
            }))
            .map_err(|error| error.to_string())?;
        picker.request = Some(id);
        runtime.colors.picker = Some(picker);
        Ok(())
    }

    /// The presentations arrived: the user picks one.
    fn finish_color_presentations(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(mut picker) = runtime.colors.picker.take() else {
            return;
        };
        picker.request = None;
        if let Some(error) = event["error"].as_str() {
            self.status = error.to_owned();
            return;
        }
        picker.presentations = presentations(&event["result"]);
        let item = |id: String, title: String| PaletteItem {
            id,
            title,
            source: "VSIX".into(),
            enabled: true,
        };
        let mut items = vec![item("edit".into(), self.t("color.edit").to_owned())];
        items.extend(
            picker
                .presentations
                .iter()
                .enumerate()
                .map(|(index, presentation)| item(index.to_string(), presentation.label.clone())),
        );
        let title = self.tf(
            "color.pick.title",
            &[("color", &hex_of(picker.range.color))],
        );
        runtime.colors.picker = Some(picker);
        self.open_vsix_internal_prompt(
            runtime,
            super::prompts::Kind::ColorChoice,
            title,
            items,
            String::new(),
        );
    }

    /// The user picked a presentation, or asked to type another color.
    pub(crate) fn resolve_color_choice(&mut self, selected: Option<String>) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        match selected.as_deref() {
            None => runtime.colors.picker = None,
            Some("edit") => {
                let initial = runtime
                    .colors
                    .picker
                    .as_ref()
                    .map(|picker| hex_of(picker.range.color))
                    .unwrap_or_default();
                let title = self.t("color.input.title").to_owned();
                self.open_vsix_internal_prompt(
                    &mut runtime,
                    super::prompts::Kind::ColorInput,
                    title,
                    Vec::new(),
                    initial,
                );
            }
            Some(index) => {
                let presentation = index.parse::<usize>().ok().and_then(|index| {
                    runtime
                        .colors
                        .picker
                        .as_ref()?
                        .presentations
                        .get(index)
                        .cloned()
                });
                if let (Some(picker), Some(presentation)) =
                    (runtime.colors.picker.take(), presentation)
                {
                    self.apply_color_presentation(&picker, &presentation);
                }
            }
        }
        self.plugins.vsix = Some(runtime);
    }

    /// The user typed a color: its presentations are asked for.
    pub(crate) fn resolve_color_input(&mut self, typed: Option<String>) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        if let Some(mut picker) = runtime.colors.picker.take() {
            match typed
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                None => {}
                Some(text) => match super::decorations::parse_color(&json!(text)) {
                    Some(color) => {
                        picker.range.color = color;
                        if let Err(error) = self.request_color_presentations(&mut runtime, picker) {
                            self.status = error;
                        }
                    }
                    None => self.status = self.tf("color.not_a_color", &[("text", &text)]),
                },
            }
        }
        self.plugins.vsix = Some(runtime);
    }

    /// Writes a presentation into the document, if the text has not changed since the picker
    /// started.
    fn apply_color_presentation(&mut self, picker: &Picker, presentation: &Presentation) {
        let document = picker.document;
        if self.active_document() != document
            || self.documents[document].buffer.revision() != picker.revision
        {
            self.status = self.t("color.changed").to_owned();
            return;
        }
        let edits = if presentation.edits.is_empty() {
            // Without an edit of its own the label replaces the color's text.
            vec![json!({"range": range_dto(&picker.range), "text": presentation.label})]
        } else {
            presentation.edits.clone()
        };
        let version = self.documents[document].buffer.revision();
        if let Err(error) = self.apply_vsix_editor_edits(&picker.uri, version, &json!(edits)) {
            self.status = error;
        }
    }

    /// Applies the answer if the document has not moved on.
    pub(super) fn finish_vsix_colors(&mut self, runtime: &mut Runtime, event: &Value) {
        if runtime.colors.picker.as_ref().is_some_and(|picker| {
            picker.request.is_some() && picker.request == event["id"].as_u64()
        }) {
            self.finish_color_presentations(runtime, event);
            return;
        }
        let Some(request) = runtime
            .colors
            .request
            .take_if(|request| event["id"].as_u64() == Some(request.id))
        else {
            return;
        };
        if event["error"].is_string() {
            // Not asked again until the document changes.
            return;
        }
        let current = self.active_document() == request.document
            && self.documents[request.document].buffer.revision() == request.revision;
        if !current {
            runtime.colors.done = None;
            return;
        }
        let ranges = color_ranges(&event["result"]);
        runtime.decorations.set_colors(&request.uri, ranges);
        runtime.colors.shown = Some(request.uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;

    fn range(line: u64, from: u64, to: u64) -> Value {
        json!({"startLineNumber": line, "startColumn": from, "endLineNumber": line, "endColumn": to})
    }

    fn color(line: u64, from: u64, to: u64, rgba: [f64; 4]) -> Value {
        json!({"color": rgba, "range": range(line, from, to)})
    }

    #[test]
    fn colors_convert_ranges_to_zero_based_and_clamp_channels() {
        let ranges = color_ranges(&json!([
            color(1, 5, 12, [1.0, 0.0, 0.5, 1.0]),
            color(2, 1, 4, [2.0, -1.0, 0.25, 0.5]),
            color(3, 4, 4, [0, 0, 0, 1].map(f64::from)),
            {"color": [1, 2, 3], "range": range(1, 1, 2)},
            {"color": [1, 2, 3, 4]},
            {"range": range(1, 1, 2)},
        ]));
        assert_eq!(ranges.len(), 2);
        assert_eq!((ranges[0].start, ranges[0].end), ((0, 4), (0, 11)));
        assert_eq!(ranges[0].color, [1.0, 0.0, 0.5, 1.0]);
        assert_eq!(ranges[1].color, [1.0, 0.0, 0.25, 0.5], "clamped to 0..=1");
        assert!(color_ranges(&Value::Null).is_empty());
    }

    #[test]
    fn the_tint_is_translucent_and_keeps_the_color() {
        let tinted = tint([0.2, 0.4, 0.6, 1.0]);
        assert_eq!(&tinted[..3], &[0.2, 0.4, 0.6]);
        assert!((tinted[3] - TINT_ALPHA).abs() < f32::EPSILON);
        assert!(
            tint([1.0, 1.0, 1.0, 0.0])[3] == 0.0,
            "a transparent color stays invisible"
        );
    }

    fn app_with_color_provider(dir: &TempDir, text: &str) -> (App, LoopbackHost) {
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
            "$registerDocumentColorProvider",
            &[json!(6), json!([{"language": "*"}])],
        )
        .unwrap()
        .unwrap();
        app.plugins.vsix = Some(runtime);
        (app, host)
    }

    fn requests(host: &LoopbackHost) -> Vec<Value> {
        std::iter::from_fn(|| host.next_sent())
            .filter(|message| message["method"] == "$provideDocumentColors")
            .collect()
    }

    fn uri(dir: &TempDir) -> String {
        graphite_core_api::DocumentUri::from_path(&dir.path().join("a.txt"))
            .unwrap()
            .as_str()
            .to_owned()
    }

    #[test]
    fn colors_are_requested_tinted_outlined_and_cleared_on_edit() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_color_provider(&dir, "color: #ff0000;\n");
        app.poll_vsix();
        let sent = requests(&host);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["args"][0], 6);

        host.emit(json!({"type": "result", "id": sent[0]["id"],
            "result": [color(1, 8, 15, [1.0, 0.0, 0.0, 1.0])]}));
        app.poll_vsix();
        let marks = app.vsix_line_decorations(&uri(&dir), 0, true);
        assert_eq!(marks.len(), 1);
        assert_eq!((marks[0].start, marks[0].end), (Some(7), Some(14)));
        assert_eq!(marks[0].look.border, Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(marks[0].look.background, Some(tint([1.0, 0.0, 0.0, 1.0])));

        app.poll_vsix();
        assert!(
            requests(&host).is_empty(),
            "reused while the text is unchanged"
        );

        app.editor_mut().insert("x");
        app.poll_vsix();
        assert!(
            app.vsix_line_decorations(&uri(&dir), 0, true).is_empty(),
            "swatches of an older text are removed at once"
        );
        assert_eq!(requests(&host).len(), 1, "and asked for again");
    }

    #[test]
    fn several_swatches_on_a_line_and_across_lines_are_all_drawn() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_color_provider(&dir, "a b\nc\n");
        app.poll_vsix();
        let sent = requests(&host);
        host.emit(json!({"type": "result", "id": sent[0]["id"], "result": [
            color(1, 1, 2, [1.0, 0.0, 0.0, 1.0]),
            color(1, 3, 4, [0.0, 1.0, 0.0, 1.0]),
            color(2, 1, 2, [0.0, 0.0, 1.0, 1.0]),
        ]}));
        app.poll_vsix();
        assert_eq!(app.vsix_line_decorations(&uri(&dir), 0, true).len(), 2);
        assert_eq!(app.vsix_line_decorations(&uri(&dir), 1, true).len(), 1);
    }

    #[test]
    fn a_failing_provider_is_not_asked_again_until_the_text_changes() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_color_provider(&dir, "x\n");
        app.poll_vsix();
        let sent = requests(&host);
        host.emit(json!({"type": "result", "id": sent[0]["id"], "error": "boom"}));
        app.poll_vsix();
        app.poll_vsix();
        assert!(requests(&host).is_empty());
        assert!(app.vsix_line_decorations(&uri(&dir), 0, true).is_empty());
    }

    #[test]
    fn an_answer_for_an_outdated_revision_is_dropped_and_asked_again() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_color_provider(&dir, "#fff\n");
        app.poll_vsix();
        let stale = requests(&host);
        app.editor_mut().insert("x");
        host.emit(json!({"type": "result", "id": stale[0]["id"],
            "result": [color(1, 1, 5, [1.0, 1.0, 1.0, 1.0])]}));
        app.poll_vsix();
        assert!(app.vsix_line_decorations(&uri(&dir), 0, true).is_empty());
        app.poll_vsix();
        assert_eq!(requests(&host).len(), 1);
    }

    fn presentation_requests(host: &LoopbackHost) -> Vec<Value> {
        std::iter::from_fn(|| host.next_sent())
            .filter(|message| message["method"] == "$provideColorPresentations")
            .collect()
    }

    /// An app whose first line is `color: #ff0000;` with the swatch of the color delivered.
    fn app_with_a_red_swatch(dir: &TempDir) -> (App, LoopbackHost) {
        let (mut app, host) = app_with_color_provider(dir, "color: #ff0000;\n");
        app.poll_vsix();
        let sent = requests(&host);
        host.emit(json!({"type": "result", "id": sent[0]["id"],
            "result": [color(1, 8, 15, [1.0, 0.0, 0.0, 1.0])]}));
        app.poll_vsix();
        (app, host)
    }

    fn presentation(label: &str, text: &str) -> Value {
        json!({"label": label, "textEdit": {"range": range(1, 8, 15), "text": text}})
    }

    fn answer(app: &mut App, query: &str, selected: Option<&str>) {
        // The palette closes itself before it reports the choice.
        app.command_palette = None;
        app.answer_vsix_prompt(Some((query.to_owned(), selected.map(str::to_owned))));
    }

    #[test]
    fn the_color_picker_offers_the_providers_presentations_and_applies_the_one_picked() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_a_red_swatch(&dir);
        app.editor_mut().set_cursor_line_col(0, 10);
        app.start_color_picker().unwrap();
        let asked = presentation_requests(&host);
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0]["args"][0], 6);
        assert_eq!(asked[0]["args"][2]["color"], json!([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(asked[0]["args"][2]["range"], range(1, 8, 15));

        host.emit(json!({"type": "result", "id": asked[0]["id"], "result": [
            presentation("#ff0000", "#ff0000"),
            presentation("rgb(255, 0, 0)", "rgb(255, 0, 0)"),
            {"label": ""},
        ]}));
        app.poll_vsix();
        let palette = app
            .command_palette
            .as_ref()
            .expect("the formats are offered");
        let titles: Vec<&str> = palette
            .items
            .iter()
            .map(|item| item.title.as_str())
            .collect();
        assert_eq!(
            titles[1..],
            ["#ff0000", "rgb(255, 0, 0)"],
            "an empty label is dropped"
        );
        assert!(palette
            .extension_prompt
            .as_ref()
            .unwrap()
            .0
            .contains("#ff0000"));
        answer(&mut app, "", Some("1"));
        assert_eq!(
            app.editor().text(),
            "color: rgb(255, 0, 0);\n",
            "{}",
            app.status
        );
        // The picker is over: nothing else is waiting.
        assert!(app.plugins.vsix.as_ref().unwrap().colors.picker.is_none());
    }

    #[test]
    fn a_presentation_without_an_edit_replaces_the_color_text_with_its_label() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_a_red_swatch(&dir);
        app.editor_mut().set_cursor_line_col(0, 8);
        app.start_color_picker().unwrap();
        let asked = presentation_requests(&host);
        host.emit(json!({"type": "result", "id": asked[0]["id"], "result": [{"label": "red"}]}));
        app.poll_vsix();
        answer(&mut app, "", Some("0"));
        assert_eq!(app.editor().text(), "color: red;\n");
    }

    #[test]
    fn another_color_can_be_typed_and_then_written_in_a_format() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_a_red_swatch(&dir);
        app.editor_mut().set_cursor_line_col(0, 10);
        app.start_color_picker().unwrap();
        let asked = presentation_requests(&host);
        host.emit(json!({"type": "result", "id": asked[0]["id"], "result": [presentation("#ff0000", "#ff0000")]}));
        app.poll_vsix();
        answer(&mut app, "", Some("edit"));
        let palette = app
            .command_palette
            .as_ref()
            .expect("a box to type the color in");
        assert!(palette.items.is_empty());
        assert_eq!(palette.query, "#ff0000", "starts from the current color");

        // Not a color: reported, nothing asked.
        answer(&mut app, "banana", None);
        assert!(app.status.contains("banana"), "{}", app.status);
        assert!(presentation_requests(&host).is_empty());

        // Start again with a real color.
        app.start_color_picker().unwrap();
        let asked = presentation_requests(&host);
        host.emit(json!({"type": "result", "id": asked[0]["id"], "result": [presentation("#ff0000", "#ff0000")]}));
        app.poll_vsix();
        answer(&mut app, "", Some("edit"));
        answer(&mut app, "#00ff0080", None);
        let again = presentation_requests(&host);
        assert_eq!(again.len(), 1);
        let channels = again[0]["args"][2]["color"].as_array().unwrap();
        assert_eq!(channels[..2], [json!(0.0), json!(1.0)]);
        assert!((channels[3].as_f64().unwrap() - 128.0 / 255.0).abs() < 1e-6);
        host.emit(json!({"type": "result", "id": again[0]["id"], "result": [presentation("#00ff0080", "#00ff0080")]}));
        app.poll_vsix();
        answer(&mut app, "", Some("0"));
        assert_eq!(app.editor().text(), "color: #00ff0080;\n");
    }

    #[test]
    fn the_color_picker_needs_a_color_a_provider_and_an_unchanged_document() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_a_red_swatch(&dir);
        app.editor_mut().set_cursor_line_col(0, 2);
        assert!(
            app.start_color_picker().unwrap_err().contains("no color"),
            "outside the color"
        );
        assert!(presentation_requests(&host).is_empty());

        app.editor_mut().set_cursor_line_col(0, 10);
        app.start_color_picker().unwrap();
        let asked = presentation_requests(&host);
        // An error from the provider is shown and leaves no picker behind.
        host.emit(json!({"type": "result", "id": asked[0]["id"], "error": "no presentations"}));
        app.poll_vsix();
        assert!(app.status.contains("no presentations"));
        assert!(app.command_palette.is_none());

        // The text changed while the list was open: nothing is applied.
        app.start_color_picker().unwrap();
        let asked = presentation_requests(&host);
        host.emit(json!({"type": "result", "id": asked[0]["id"], "result": [presentation("#ff0000", "#ff0000")]}));
        app.poll_vsix();
        app.editor_mut().insert("x");
        answer(&mut app, "", Some("0"));
        assert!(app.status.contains("changed"), "{}", app.status);
        assert_eq!(
            app.editor().text(),
            "color: #ffx0000;
",
            "the edit was not applied over the change"
        );
    }

    #[test]
    fn colors_are_written_as_hex_and_presentations_ignore_invalid_entries() {
        assert_eq!(hex_of([1.0, 0.0, 0.5, 1.0]), "#ff0080");
        assert_eq!(hex_of([0.0, 0.0, 0.0, 0.5]), "#00000080");
        assert_eq!(hex_of([2.0, -1.0, 0.0, 1.0]), "#ff0000", "clamped");
        let found = presentations(&json!([
            {"label": "a", "textEdit": {"range": range(1, 1, 2), "text": "a"},
             "additionalTextEdits": [{"range": range(1, 1, 1), "text": "x"}]},
            {"label": ""}, {"textEdit": {}}, "oops",
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].edits.len(), 2);
        assert!(presentations(&Value::Null).is_empty());
    }

    #[test]
    fn reply_ids_are_recognised_by_range() {
        assert!(is_color_reply(COLOR_BASE));
        assert!(!is_color_reply(COLOR_END));
        assert!(!is_color_reply(9_000_000), "code lens replies");
    }
}
