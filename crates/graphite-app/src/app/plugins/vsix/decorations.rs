//! Text editor decorations from extensions (`window.createTextEditorDecorationType` and
//! `TextEditor.setDecorations`). The extension host sends each decoration type's style once and
//! then the ranges it applies to a document. Graphite keeps both and the editor draws them:
//! background and border colors, whole-line marks and the text after a line (`after`).
//! Fonts, gutter icons, overview ruler marks and `before` text are not drawn.

use super::*;

pub(crate) type Color = [f32; 4];

/// Largest number of ranges kept for one document and decoration type.
const MARK_LIMIT: usize = 20_000;
/// Longest `after` text drawn, in characters.
const AFTER_LIMIT: usize = 300;

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
    pub after: Option<After>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Style {
    dark: Look,
    light: Look,
    whole_line: bool,
}

/// A decorated range: 0-based lines and UTF-16 columns.
#[derive(Clone, Debug, PartialEq)]
struct Mark {
    start: (u32, u32),
    end: (u32, u32),
    /// `after` text of this instance, which replaces the type's.
    after: Option<After>,
}

/// What to draw on one line for one decoration.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineMark {
    pub look: Look,
    pub whole_line: bool,
    /// UTF-16 columns of the decorated part; `None` means the line start or end.
    pub start: Option<u32>,
    pub end: Option<u32>,
    /// Only on the last line of the range.
    pub after: Option<After>,
}

#[derive(Default)]
pub(super) struct Decorations {
    styles: BTreeMap<String, Style>,
    /// Marks by editor (the document URI) and decoration type key.
    marks: BTreeMap<String, BTreeMap<String, Vec<Mark>>>,
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
    let content = ["after", "afterInjectedText"]
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
        after: after(options),
    }
}

/// Fields a variant sets replace the base look; the rest is inherited.
fn overlay(base: &Look, variant: &Look) -> Look {
    Look {
        background: variant.background.or(base.background),
        border: variant.border.or(base.border),
        after: variant.after.clone().or_else(|| base.after.clone()),
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

    pub(super) fn is_empty(&self) -> bool {
        self.marks.values().all(BTreeMap::is_empty)
    }

    /// Everything to draw on `line` of `editor` in the given theme.
    pub(crate) fn on_line(&self, editor: &str, line: usize, dark: bool) -> Vec<LineMark> {
        let line = line as u32;
        let Some(by_key) = self.marks.get(editor) else {
            return Vec::new();
        };
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
                let instance = mark.after.clone().filter(|_| last);
                let after = instance.or_else(|| look.after.clone().filter(|_| last));
                let mut look = look.clone();
                look.after = None;
                found.push(LineMark {
                    look,
                    whole_line: style.whole_line,
                    start: (line == mark.start.0).then_some(mark.start.1),
                    end: last.then_some(mark.end.1),
                    after,
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
                        Some(Mark {
                            start,
                            end,
                            after: instance,
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
                    .chunks_exact(4)
                    .filter_map(|range| {
                        Some(Mark {
                            start: (range[0].checked_sub(1)?, range[1].checked_sub(1)?),
                            end: (range[2].checked_sub(1)?, range[3].checked_sub(1)?),
                            after: None,
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
    /// Decorations to draw on `line` of the document with this URI.
    pub(crate) fn vsix_line_decorations(
        &self,
        uri: &str,
        line: usize,
        dark: bool,
    ) -> Vec<LineMark> {
        self.plugins
            .vsix
            .as_ref()
            .filter(|runtime| !runtime.decorations.is_empty())
            .map(|runtime| runtime.decorations.on_line(uri, line, dark))
            .unwrap_or_default()
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
                },
                Mark {
                    start: (4, 0),
                    end: (4, 3),
                    after: Some(After {
                        text: " instance".into(),
                        color: None,
                    }),
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
            .chunks_exact(4)
            .filter_map(|range| {
                Some(Mark {
                    start: (range[0].checked_sub(1)?, range[1].checked_sub(1)?),
                    end: (range[2].checked_sub(1)?, range[3].checked_sub(1)?),
                    after: None,
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
