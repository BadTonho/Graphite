//! Text editor decorations from extensions (`window.createTextEditorDecorationType` and
//! `TextEditor.setDecorations`). The extension host sends each decoration type's style once and
//! then the ranges it applies to a document. Graphite keeps both and the editor draws them:
//! background and border colors, whole-line marks, the text before a range (`before`) and after
//! a line (`after`), gutter icons, overview ruler marks and hover messages.
//! Fonts are not drawn.

use super::*;

pub(crate) type Color = [f32; 4];

/// Largest number of ranges kept for one document and decoration type.
const MARK_LIMIT: usize = 20_000;
/// Longest `after` text drawn, in characters.
const AFTER_LIMIT: usize = 300;
/// Decoration type key of the built-in document highlights.
const HIGHLIGHT_KEY: &str = "graphite.documentHighlight";
/// Decoration type key of the built-in document links.
const LINK_KEY: &str = "graphite.documentLink";
/// Decoration type key of the built-in code lens texts.
const LENS_KEY: &str = "graphite.codeLens";

/// Text drawn after the end of a line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct After {
    pub text: String,
    pub color: Option<Color>,
}

/// How decorated text looks in one theme.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Look {
    pub background: Option<Color>,
    pub border: Option<Color>,
    /// Color of a line drawn under the text (document links).
    pub underline: Option<Color>,
    pub after: Option<After>,
    /// Text drawn at the start of the range.
    pub before: Option<After>,
    /// An image drawn in the gutter of the first line of the range.
    pub gutter_icon: Option<std::path::PathBuf>,
    /// Color of the mark on the overview ruler.
    pub overview: Option<Color>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Style {
    dark: Look,
    light: Look,
    whole_line: bool,
}

/// A decorated range: 0-based lines and UTF-16 columns.
#[derive(Clone, Debug, Default, PartialEq)]
struct Mark {
    start: (u32, u32),
    end: (u32, u32),
    /// `after` text of this instance, which replaces the type's.
    after: Option<After>,
    /// `before` text of this instance, which replaces the type's.
    before: Option<After>,
    /// What the extension asked to show when hovering the range.
    hover: Option<String>,
}

/// What to draw on one line for one decoration.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LineMark {
    pub look: Look,
    pub whole_line: bool,
    /// UTF-16 columns of the decorated part; `None` means the line start or end.
    pub start: Option<u32>,
    pub end: Option<u32>,
    /// Only on the last line of the range.
    pub after: Option<After>,
    /// Only on the first line of the range.
    pub before: Option<After>,
    /// Only on the first line of the range.
    pub gutter_icon: Option<std::path::PathBuf>,
    /// The `after` text is a code lens, which the editor may draw above the line instead.
    pub lens: bool,
}

#[derive(Default)]
pub(crate) struct Decorations {
    styles: BTreeMap<String, Style>,
    /// Marks by editor (the document URI) and decoration type key.
    marks: BTreeMap<String, BTreeMap<String, Vec<Mark>>>,
    /// Color swatches by editor.
    colors: BTreeMap<String, Vec<super::colors::ColorRange>>,
}

fn hex_byte(digits: &str) -> Option<f32> {
    u8::from_str_radix(digits, 16)
        .ok()
        .map(|value| f32::from(value) / 255.0)
}

/// Color from `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb(...)`, `rgba(...)` or a theme color
/// reference such as `{ "id": "editorError.foreground" }`.
pub(crate) fn parse_color(value: &Value) -> Option<Color> {
    if let Some(id) = value["id"].as_str() {
        return theme_color(id);
    }
    let text = value.as_str()?.trim();
    if let Some(hex) = text.strip_prefix('#') {
        let digits: Vec<String> = match hex.len() {
            3 | 4 => hex.chars().map(|ch| format!("{ch}{ch}")).collect(),
            6 | 8 => hex
                .as_bytes()
                .chunks(2)
                .map(|pair| String::from_utf8_lossy(pair).into_owned())
                .collect(),
            _ => return None,
        };
        let channel = |index: usize| hex_byte(digits.get(index)?);
        return Some([
            channel(0)?,
            channel(1)?,
            channel(2)?,
            channel(3).unwrap_or(1.0),
        ]);
    }
    let inner = text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f32> = inner
        .split(',')
        .map(|part| part.trim().parse::<f32>().ok())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [r, g, b] => Some([r / 255.0, g / 255.0, b / 255.0, 1.0]),
        [r, g, b, a] => Some([r / 255.0, g / 255.0, b / 255.0, a.clamp(0.0, 1.0)]),
        _ => None,
    }
}

/// Dark-theme approximations of the theme colors extensions refer to most.
fn theme_color(id: &str) -> Option<Color> {
    let hex = match id {
        "editor.findMatchBackground" => "#515c6a",
        "editor.findMatchHighlightBackground" => "#ea5c0055",
        "editor.selectionBackground" | "editor.selectionHighlightBackground" => "#264f78",
        "editor.wordHighlightBackground" => "#575757b8",
        "editor.lineHighlightBackground" => "#ffffff0a",
        "editor.hoverHighlightBackground" => "#264f7840",
        "editorError.foreground" | "errorForeground" => "#f14c4c",
        "editorError.background" => "#f14c4c20",
        "editorWarning.foreground" => "#cca700",
        "editorWarning.background" => "#cca70020",
        "editorInfo.foreground" => "#3794ff",
        "editorInfo.background" => "#3794ff20",
        "editorHint.foreground" => "#eeeeeeb3",
        "diffEditor.insertedTextBackground" => "#9bb95533",
        "diffEditor.removedTextBackground" => "#ff000033",
        "editorInlayHint.foreground" | "editorCodeLens.foreground" => "#969696",
        "editorGutter.addedBackground" => "#2ea043",
        "editorGutter.modifiedBackground" => "#0078d4",
        "editorGutter.deletedBackground" => "#f85149",
        "editor.foreground" => "#d4d4d4",
        _ => return None,
    };
    parse_color(&Value::String(hex.into()))
}

/// Color of a CSS-like border such as `1px solid red` or `2px dashed rgba(1, 2, 3, 0.5)`.
fn border_color(options: &Value) -> Option<Color> {
    for key in ["borderColor", "outlineColor"] {
        if let Some(color) = parse_color(&options[key]) {
            return Some(color);
        }
    }
    for key in ["border", "outline"] {
        let Some(text) = options[key].as_str() else {
            continue;
        };
        // The color is the last part after the width and the line style.
        let mut parts = text.split_whitespace();
        let rest = parts.by_ref().skip(2).collect::<Vec<_>>().join(" ");
        if let Some(color) = parse_color(&Value::String(rest)) {
            return Some(color);
        }
        if let Some(color) = text
            .split_whitespace()
            .find_map(|part| parse_color(&Value::String(part.into())))
        {
            return Some(color);
        }
    }
    None
}

fn after(options: &Value) -> Option<After> {
    injected_text(options, &["after", "afterInjectedText"])
}

fn before(options: &Value) -> Option<After> {
    injected_text(options, &["before", "beforeInjectedText"])
}

/// The path of an image given as a file URI or a path.
fn image_path(value: &Value) -> Option<std::path::PathBuf> {
    let text = value
        .as_str()
        .or_else(|| value["fsPath"].as_str())
        .or_else(|| value["path"].as_str())?;
    if text.starts_with("file:") {
        return graphite_core_api::DocumentUri::parse(text)
            .ok()?
            .to_path()
            .ok();
    }
    Some(std::path::PathBuf::from(text))
}

/// What a decoration says to show on hover: its markdown strings, one paragraph each.
fn hover_text(value: &Value) -> Option<String> {
    let parts: Vec<String> = match value {
        Value::Array(items) => items.iter().filter_map(markdown_value).collect(),
        other => markdown_value(other).into_iter().collect(),
    };
    let text = parts.join("\n\n");
    (!text.trim().is_empty()).then_some(text)
}

fn markdown_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value["value"].as_str())
        .map(str::to_owned)
        .filter(|text| !text.is_empty())
}

fn injected_text(options: &Value, keys: &[&str]) -> Option<After> {
    let content = keys
        .iter()
        .map(|key| &options[key])
        .find(|content| content["contentText"].is_string())?;
    let text: String = content["contentText"]
        .as_str()?
        .chars()
        .take(AFTER_LIMIT)
        .collect();
    (!text.is_empty()).then(|| After {
        text,
        color: parse_color(&content["color"]),
    })
}

/// A look from one set of render options (the base style or its `dark`/`light` variant).
fn look(options: &Value) -> Look {
    Look {
        background: parse_color(&options["backgroundColor"]),
        border: border_color(options),
        underline: None,
        after: after(options),
        before: before(options),
        gutter_icon: image_path(&options["gutterIconPath"]),
        overview: parse_color(&options["overviewRulerColor"]),
    }
}

/// Fields a variant sets replace the base look; the rest is inherited.
fn overlay(base: &Look, variant: &Look) -> Look {
    Look {
        background: variant.background.or(base.background),
        border: variant.border.or(base.border),
        underline: variant.underline.or(base.underline),
        after: variant.after.clone().or_else(|| base.after.clone()),
        before: variant.before.clone().or_else(|| base.before.clone()),
        gutter_icon: variant
            .gutter_icon
            .clone()
            .or_else(|| base.gutter_icon.clone()),
        overview: variant.overview.or(base.overview),
    }
}

fn style(options: &Value) -> Style {
    let base = look(options);
    Style {
        dark: overlay(&base, &look(&options["dark"])),
        light: overlay(&base, &look(&options["light"])),
        whole_line: options["isWholeLine"].as_bool().unwrap_or(false),
    }
}

fn zero_based(range: &Value) -> Option<((u32, u32), (u32, u32))> {
    let coordinate = |name: &str| u32::try_from(range[name].as_u64()?).ok()?.checked_sub(1);
    Some((
        (coordinate("startLineNumber")?, coordinate("startColumn")?),
        (coordinate("endLineNumber")?, coordinate("endColumn")?),
    ))
}

impl Decorations {
    fn set(&mut self, editor: &str, key: &str, marks: Vec<Mark>) {
        let by_key = self.marks.entry(editor.to_owned()).or_default();
        if marks.is_empty() {
            by_key.remove(key);
        } else {
            by_key.insert(key.to_owned(), marks);
        }
    }

    /// Draws the occurrences of the symbol at the cursor, as reported by a document highlight
    /// provider, with a built-in decoration type.
    pub(super) fn set_highlights(&mut self, editor: &str, ranges: Vec<((u32, u32), (u32, u32))>) {
        let color = theme_color("editor.wordHighlightBackground");
        let look = Look {
            background: color,
            ..Look::default()
        };
        self.styles
            .entry(HIGHLIGHT_KEY.to_owned())
            .or_insert(Style {
                dark: look.clone(),
                light: look,
                whole_line: false,
            });
        let marks = ranges
            .into_iter()
            .take(MARK_LIMIT)
            .map(|(start, end)| Mark {
                start,
                end,
                after: None,
                ..Default::default()
            })
            .collect();
        self.set(editor, HIGHLIGHT_KEY, marks);
    }

    pub(super) fn clear_highlights(&mut self, editor: &str) {
        self.set(editor, HIGHLIGHT_KEY, Vec::new());
    }

    /// Underlines the links an extension reported, as 0-based `((line, column), (line, column))`
    /// ranges, with a built-in decoration type.
    pub(super) fn set_links(&mut self, editor: &str, ranges: Vec<((u32, u32), (u32, u32))>) {
        let look = Look {
            underline: theme_color("textLink.foreground")
                .or_else(|| parse_color(&Value::String("#3794ff".into()))),
            ..Look::default()
        };
        self.styles.entry(LINK_KEY.to_owned()).or_insert(Style {
            dark: look.clone(),
            light: look,
            whole_line: false,
        });
        let marks = ranges
            .into_iter()
            .take(MARK_LIMIT)
            .map(|(start, end)| Mark {
                start,
                end,
                after: None,
                ..Default::default()
            })
            .collect();
        self.set(editor, LINK_KEY, marks);
    }

    pub(super) fn clear_links(&mut self, editor: &str) {
        self.set(editor, LINK_KEY, Vec::new());
    }

    /// Shows `(0-based line, text)` pairs as muted text at the end of those lines, with a
    /// built-in decoration type.
    pub(super) fn set_lenses(&mut self, editor: &str, lines: Vec<(u32, String)>) {
        self.styles.entry(LENS_KEY.to_owned()).or_default();
        let marks = lines
            .into_iter()
            .take(MARK_LIMIT)
            .map(|(line, text)| Mark {
                start: (line, 0),
                end: (line, 0),
                after: Some(After { text, color: None }),
                ..Default::default()
            })
            .collect();
        self.set(editor, LENS_KEY, marks);
    }

    pub(super) fn clear_lenses(&mut self, editor: &str) {
        self.set(editor, LENS_KEY, Vec::new());
    }

    /// Shows the swatches of a color provider for `editor`.
    pub(super) fn set_colors(&mut self, editor: &str, ranges: Vec<super::colors::ColorRange>) {
        if ranges.is_empty() {
            self.colors.remove(editor);
        } else {
            self.colors.insert(
                editor.to_owned(),
                ranges.into_iter().take(MARK_LIMIT).collect(),
            );
        }
    }

    pub(super) fn clear_colors(&mut self, editor: &str) {
        self.colors.remove(editor);
    }

    pub(crate) fn set_style(&mut self, key: String, options: &Value) {
        self.styles.insert(key, style(options));
    }

    pub(crate) fn remove_style(&mut self, key: &str) {
        self.styles.remove(key);
        for by_key in self.marks.values_mut() {
            by_key.remove(key);
        }
        self.marks.retain(|_, by_key| !by_key.is_empty());
    }

    pub(crate) fn set_mark_ranges(
        &mut self,
        editor: &str,
        key: &str,
        ranges: Vec<((u32, u32), (u32, u32))>,
    ) {
        let marks = ranges
            .into_iter()
            .take(MARK_LIMIT)
            .map(|(start, end)| Mark {
                start,
                end,
                after: None,
                ..Default::default()
            })
            .collect();
        self.set(editor, key, marks);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.marks.values().all(BTreeMap::is_empty) && self.colors.values().all(Vec::is_empty)
    }

    /// The color swatch of `editor` that contains a 0-based line and UTF-16 column.
    pub(super) fn color_at(
        &self,
        editor: &str,
        line: u32,
        column: u32,
    ) -> Option<super::colors::ColorRange> {
        self.colors
            .get(editor)?
            .iter()
            .find(|range| (line, column) >= range.start && (line, column) <= range.end)
            .cloned()
    }

    /// The lines of `editor` that have an overview ruler mark, with its color, in line order.
    pub(crate) fn overview_marks(&self, editor: &str, dark: bool) -> Vec<(usize, Color)> {
        let mut found = Vec::new();
        for (key, marks) in self.marks.get(editor).into_iter().flatten() {
            let Some(style) = self.styles.get(key) else {
                continue;
            };
            let look = if dark { &style.dark } else { &style.light };
            let Some(color) = look.overview else {
                continue;
            };
            found.extend(marks.iter().map(|mark| (mark.start.0 as usize, color)));
        }
        found.sort_by_key(|(line, _)| *line);
        found
    }

    /// What the extensions want to show on hover at a 0-based line and UTF-16 column.
    pub(crate) fn hover_at(&self, editor: &str, line: u32, column: u32) -> Vec<String> {
        let mut found = Vec::new();
        for marks in self
            .marks
            .get(editor)
            .into_iter()
            .flat_map(BTreeMap::values)
        {
            for mark in marks {
                let Some(hover) = &mark.hover else {
                    continue;
                };
                let after_start = (line, column) >= mark.start;
                // A range ends before its end position; an empty range still hovers at its start.
                let before_end = (line, column) < mark.end
                    || mark.start == mark.end && (line, column) == mark.start;
                if after_start && before_end {
                    found.push(hover.clone());
                }
            }
        }
        found
    }

    /// Everything to draw on `line` of `editor` in the given theme.
    pub(crate) fn on_line(&self, editor: &str, line: usize, dark: bool) -> Vec<LineMark> {
        let line = line as u32;
        // An editor may have only color swatches and no marks.
        let no_marks = BTreeMap::new();
        let by_key = self.marks.get(editor).unwrap_or(&no_marks);
        let mut found = Vec::new();
        for (key, marks) in by_key {
            let Some(style) = self.styles.get(key) else {
                continue;
            };
            let look = if dark { &style.dark } else { &style.light };
            for mark in marks
                .iter()
                .filter(|mark| mark.start.0 <= line && line <= mark.end.0)
            {
                let last = line == mark.end.0;
                let first = line == mark.start.0;
                let instance = mark.after.clone().filter(|_| last);
                let after = instance.or_else(|| look.after.clone().filter(|_| last));
                let before = mark
                    .before
                    .clone()
                    .or_else(|| look.before.clone())
                    .filter(|_| first);
                let gutter_icon = look.gutter_icon.clone().filter(|_| first);
                let mut look = look.clone();
                look.after = None;
                look.before = None;
                look.gutter_icon = None;
                found.push(LineMark {
                    look,
                    whole_line: style.whole_line,
                    start: first.then_some(mark.start.1),
                    end: last.then_some(mark.end.1),
                    after,
                    before,
                    gutter_icon,
                    lens: key == LENS_KEY,
                });
            }
        }
        for range in self.colors.get(editor).into_iter().flatten() {
            if range.start.0 <= line && line <= range.end.0 {
                found.push(LineMark {
                    look: Look {
                        background: Some(super::colors::tint(range.color)),
                        border: Some(range.color),
                        ..Look::default()
                    },
                    whole_line: false,
                    start: (line == range.start.0).then_some(range.start.1),
                    end: (line == range.end.0).then_some(range.end.1),
                    after: None,
                    ..Default::default()
                });
            }
        }
        found
    }
}

/// Answers the decoration requests of `MainThreadTextEditors`, or returns `None`.
pub(super) fn dispatch(
    runtime: &mut Runtime,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    if actor != "MainThreadTextEditors" {
        return None;
    }
    let text = |index: usize| args.get(index).and_then(Value::as_str);
    let decorations = &mut runtime.decorations;
    let result = match method {
        "$registerTextEditorDecorationType" => match (text(1), args.get(2)) {
            (Some(key), Some(options)) => {
                decorations.styles.insert(key.to_owned(), style(options));
                Ok(Value::Null)
            }
            _ => Err("Invalid decoration type".to_owned()),
        },
        "$removeTextEditorDecorationType" => {
            if let Some(key) = text(0) {
                decorations.styles.remove(key);
                for by_key in decorations.marks.values_mut() {
                    by_key.remove(key);
                }
            }
            Ok(Value::Null)
        }
        "$trySetDecorations" => match (text(0), text(1), args.get(2).and_then(Value::as_array)) {
            (Some(editor), Some(key), Some(ranges)) => {
                let marks = ranges
                    .iter()
                    .filter_map(|item| {
                        let (start, end) = zero_based(&item["range"])?;
                        let options = &item["renderOptions"];
                        let instance = after(options)
                            .or_else(|| after(&options["dark"]))
                            .or_else(|| after(&options["light"]));
                        let instance_before = before(options)
                            .or_else(|| before(&options["dark"]))
                            .or_else(|| before(&options["light"]));
                        Some(Mark {
                            start,
                            end,
                            after: instance,
                            before: instance_before,
                            hover: hover_text(&item["hoverMessage"]),
                        })
                    })
                    .take(MARK_LIMIT)
                    .collect();
                decorations.set(editor, key, marks);
                Ok(json!(true))
            }
            _ => Err("Invalid decorations".to_owned()),
        },
        // Flat `[startLine, startColumn, endLine, endColumn, ...]`, 1-based.
        "$trySetDecorationsFast" => match (text(0), text(1), args.get(2).and_then(Value::as_array))
        {
            (Some(editor), Some(key), Some(numbers)) => {
                let numbers: Vec<u32> = numbers
                    .iter()
                    .filter_map(|number| u32::try_from(number.as_u64()?).ok())
                    .collect();
                let marks = numbers
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter_map(|range| {
                        Some(Mark {
                            start: (range[0].checked_sub(1)?, range[1].checked_sub(1)?),
                            end: (range[2].checked_sub(1)?, range[3].checked_sub(1)?),
                            after: None,
                            ..Default::default()
                        })
                    })
                    .take(MARK_LIMIT)
                    .collect();
                decorations.set(editor, key, marks);
                Ok(json!(true))
            }
            _ => Err("Invalid decorations".to_owned()),
        },
        _ => return None,
    };
    Some(result)
}

impl App {
    /// What extensions show on hover at a 0-based line and UTF-16 column of this document.
    pub(crate) fn vsix_decoration_hover(&self, uri: &str, line: u32, column: u32) -> Vec<String> {
        self.plugins
            .vsix
            .as_ref()
            .map(|runtime| runtime.decorations.hover_at(uri, line, column))
            .unwrap_or_default()
    }

    /// The overview ruler marks of the document with this URI.
    pub(crate) fn vsix_overview_marks(&self, uri: &str, dark: bool) -> Vec<(usize, Color)> {
        self.plugins
            .vsix
            .as_ref()
            .map(|runtime| runtime.decorations.overview_marks(uri, dark))
            .unwrap_or_default()
    }

    /// Decorations to draw on `line` of the document with this URI.
    pub(crate) fn vsix_line_decorations(
        &self,
        uri: &str,
        line: usize,
        dark: bool,
    ) -> Vec<LineMark> {
        let mut marks = self
            .plugins
            .vsix
            .as_ref()
            .filter(|runtime| !runtime.decorations.is_empty())
            .map(|runtime| runtime.decorations.on_line(uri, line, dark))
            .unwrap_or_default();
        if !self.extension_host.decorations.is_empty() {
            marks.extend(self.extension_host.decorations.on_line(uri, line, dark));
        }
        for host in self.plugins.hosts.values() {
            if !host.decorations.is_empty() {
                marks.extend(host.decorations.on_line(uri, line, dark));
            }
        }
        marks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered(options: Value) -> Decorations {
        let mut decorations = Decorations::default();
        decorations.styles.insert("type".into(), style(&options));
        decorations
    }

    #[test]
    fn colors_are_read_from_hex_functions_and_theme_references() {
        assert_eq!(parse_color(&json!("#ff0000")), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_color(&json!("#f00")), Some([1.0, 0.0, 0.0, 1.0]));
        let half = parse_color(&json!("#00000080")).unwrap();
        assert!((half[3] - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(
            parse_color(&json!("rgba(255, 0, 0, 0.5)")),
            Some([1.0, 0.0, 0.0, 0.5])
        );
        assert_eq!(
            parse_color(&json!("rgb(0, 255, 0)")),
            Some([0.0, 1.0, 0.0, 1.0])
        );
        assert!(parse_color(&json!({"id": "editorError.foreground"})).is_some());
        assert!(parse_color(&json!({"id": "unknown.color"})).is_none());
        assert!(parse_color(&json!("#12")).is_none());
        assert!(parse_color(&json!("banana")).is_none());
        assert!(parse_color(&Value::Null).is_none());
    }

    #[test]
    fn borders_use_the_color_part_of_css_shorthand() {
        assert_eq!(
            border_color(&json!({"border": "1px solid #00ff00"})),
            Some([0.0, 1.0, 0.0, 1.0])
        );
        assert_eq!(
            border_color(&json!({"outline": "2px dashed rgba(0, 0, 255, 1)"})),
            Some([0.0, 0.0, 1.0, 1.0])
        );
        assert_eq!(
            border_color(&json!({"borderColor": "#ff0000"})),
            Some([1.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(border_color(&json!({"border": "1px solid"})), None);
    }

    #[test]
    fn theme_variants_override_the_base_look() {
        let decorations = registered(json!({
            "backgroundColor": "#ff0000", "dark": {"backgroundColor": "#00ff00"}, "isWholeLine": true,
        }));
        let style = &decorations.styles["type"];
        assert_eq!(style.dark.background, Some([0.0, 1.0, 0.0, 1.0]));
        assert_eq!(style.light.background, Some([1.0, 0.0, 0.0, 1.0]));
        assert!(style.whole_line);
    }

    #[test]
    fn multi_line_ranges_split_into_per_line_columns() {
        let mut decorations = registered(json!({"backgroundColor": "#ff0000"}));
        decorations.set(
            "file:///a.txt",
            "type",
            vec![Mark {
                start: (1, 3),
                end: (3, 5),
                after: None,
                ..Default::default()
            }],
        );
        let first = &decorations.on_line("file:///a.txt", 1, true)[0];
        assert_eq!((first.start, first.end), (Some(3), None));
        let middle = &decorations.on_line("file:///a.txt", 2, true)[0];
        assert_eq!((middle.start, middle.end), (None, None));
        let last = &decorations.on_line("file:///a.txt", 3, true)[0];
        assert_eq!((last.start, last.end), (None, Some(5)));
        assert!(decorations.on_line("file:///a.txt", 0, true).is_empty());
        assert!(decorations.on_line("file:///a.txt", 4, true).is_empty());
        assert!(decorations.on_line("file:///other.txt", 2, true).is_empty());
    }

    #[test]
    fn before_text_and_gutter_icons_belong_to_the_first_line_of_a_range() {
        let mut decorations = registered(json!({
            "before": {"contentText": "▶ ", "color": "#888888"},
            "gutterIconPath": "/icons/dot.svg",
            "dark": {"gutterIconPath": "/icons/dark.png"},
        }));
        decorations.set(
            "e",
            "type",
            vec![
                Mark {
                    start: (2, 4),
                    end: (3, 1),
                    ..Default::default()
                },
                Mark {
                    start: (7, 0),
                    end: (7, 2),
                    before: Some(After {
                        text: "own ".into(),
                        color: None,
                    }),
                    ..Default::default()
                },
            ],
        );
        let first = &decorations.on_line("e", 2, true)[0];
        assert_eq!(first.before.as_ref().unwrap().text, "▶ ");
        assert_eq!(
            first.gutter_icon.as_deref(),
            Some(std::path::Path::new("/icons/dark.png"))
        );
        let light = &decorations.on_line("e", 2, false)[0];
        assert!(light
            .gutter_icon
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .ends_with("dot.svg"));
        let second = &decorations.on_line("e", 3, true)[0];
        assert!(second.before.is_none() && second.gutter_icon.is_none());
        assert_eq!(
            decorations.on_line("e", 7, true)[0]
                .before
                .as_ref()
                .unwrap()
                .text,
            "own "
        );
        assert!(
            first.look.before.is_none(),
            "the look itself carries neither"
        );
    }

    #[test]
    fn overview_marks_list_the_lines_with_their_colors_in_order() {
        let mut decorations = registered(
            json!({"overviewRulerColor": "#ff0000", "dark": {"overviewRulerColor": "#00ff00"}}),
        );
        decorations.set(
            "e",
            "type",
            vec![
                Mark {
                    start: (9, 0),
                    end: (9, 1),
                    ..Default::default()
                },
                Mark {
                    start: (3, 0),
                    end: (5, 1),
                    ..Default::default()
                },
            ],
        );
        assert_eq!(
            decorations.overview_marks("e", true),
            [(3, [0.0, 1.0, 0.0, 1.0]), (9, [0.0, 1.0, 0.0, 1.0])]
        );
        assert_eq!(
            decorations.overview_marks("e", false)[0].1,
            [1.0, 0.0, 0.0, 1.0]
        );
        assert!(decorations.overview_marks("other", true).is_empty());
        let plain = registered(json!({"backgroundColor": "#ff0000"}));
        assert!(plain.overview_marks("e", true).is_empty());
    }

    #[test]
    fn hover_messages_come_from_the_ranges_that_contain_the_position() {
        let mut runtime_decorations = Decorations::default();
        let mut runtime = None::<()>;
        let _ = &mut runtime;
        runtime_decorations
            .styles
            .insert("type".into(), style(&json!({})));
        let items = json!([
            {"range": {"startLineNumber": 1, "startColumn": 2, "endLineNumber": 1, "endColumn": 6},
             "hoverMessage": [{"value": "first"}, {"value": "second"}]},
            {"range": {"startLineNumber": 3, "startColumn": 1, "endLineNumber": 3, "endColumn": 1},
             "hoverMessage": "empty range"},
            {"range": {"startLineNumber": 5, "startColumn": 1, "endLineNumber": 5, "endColumn": 4}},
        ]);
        let marks: Vec<Mark> = items
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| {
                let (start, end) = zero_based(&item["range"])?;
                Some(Mark {
                    start,
                    end,
                    hover: hover_text(&item["hoverMessage"]),
                    ..Default::default()
                })
            })
            .collect();
        runtime_decorations.set("e", "type", marks);
        assert_eq!(runtime_decorations.hover_at("e", 0, 3), ["first\n\nsecond"]);
        assert!(
            runtime_decorations.hover_at("e", 0, 0).is_empty(),
            "before the range"
        );
        assert_eq!(
            runtime_decorations.hover_at("e", 0, 1),
            ["first

second"],
            "the start is inclusive"
        );
        assert!(
            runtime_decorations.hover_at("e", 0, 5).is_empty(),
            "the end is exclusive"
        );
        assert_eq!(runtime_decorations.hover_at("e", 2, 0), ["empty range"]);
        assert!(
            runtime_decorations.hover_at("e", 4, 1).is_empty(),
            "no message, nothing to show"
        );
        assert_eq!(hover_text(&json!("")), None);
        assert_eq!(
            hover_text(&json!([{"value": "a"}, "b"])).as_deref(),
            Some("a\n\nb")
        );
    }

    #[test]
    fn after_text_appears_only_on_the_last_line_and_instances_override_the_type() {
        let mut decorations =
            registered(json!({"after": {"contentText": " type", "color": "#888888"}}));
        decorations.set(
            "e",
            "type",
            vec![
                Mark {
                    start: (0, 0),
                    end: (1, 2),
                    after: None,
                    ..Default::default()
                },
                Mark {
                    start: (4, 0),
                    end: (4, 3),
                    after: Some(After {
                        text: " instance".into(),
                        color: None,
                    }),
                    ..Default::default()
                },
            ],
        );
        assert_eq!(decorations.on_line("e", 0, true)[0].after, None);
        assert_eq!(
            decorations.on_line("e", 1, true)[0]
                .after
                .as_ref()
                .unwrap()
                .text,
            " type"
        );
        assert_eq!(
            decorations.on_line("e", 4, true)[0]
                .after
                .as_ref()
                .unwrap()
                .text,
            " instance"
        );
    }

    #[test]
    fn dispatch_registers_types_sets_marks_and_removes_them() {
        let mut decorations = Decorations::default();
        // Exercise the pure parts through the same parsing the requests use.
        decorations
            .styles
            .insert("k".into(), style(&json!({"backgroundColor": "#112233"})));
        let flat: Vec<u32> = vec![1, 1, 1, 4, 0, 1, 1, 2];
        let marks: Vec<Mark> = flat
            .as_chunks::<4>()
            .0
            .iter()
            .filter_map(|range| {
                Some(Mark {
                    start: (range[0].checked_sub(1)?, range[1].checked_sub(1)?),
                    end: (range[2].checked_sub(1)?, range[3].checked_sub(1)?),
                    after: None,
                    ..Default::default()
                })
            })
            .collect();
        // The second range has a 0 coordinate (invalid, 1-based) and is dropped.
        assert_eq!(marks.len(), 1);
        decorations.set("e", "k", marks);
        assert!(!decorations.is_empty());
        decorations.set("e", "k", Vec::new());
        assert!(decorations.is_empty());
    }

    #[test]
    fn range_coordinates_are_one_based_and_validated() {
        let range =
            json!({"startLineNumber": 2, "startColumn": 1, "endLineNumber": 2, "endColumn": 5});
        assert_eq!(zero_based(&range), Some(((1, 0), (1, 4))));
        assert!(zero_based(
            &json!({"startLineNumber": 0, "startColumn": 1, "endLineNumber": 1, "endColumn": 1})
        )
        .is_none());
        assert!(zero_based(&json!({})).is_none());
    }
}
