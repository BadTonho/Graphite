//! Theme variables and base styles for extension webview pages.
//!
//! Extension pages style themselves with the `--vscode-*` CSS variables and the `vscode-dark` /
//! `vscode-light` body classes that the reference editor injects into every webview. Without them
//! text, buttons and surfaces fall back to undefined colors. This module builds the same variables
//! from Graphite's current theme: the core surfaces follow the palette (including the user's color
//! overrides), and the remaining widget colors use fixed dark and light defaults.

use crate::app::theme::{Theme, DARK_THEME, HIGH_CONTRAST};
use std::{fmt::Write as _, sync::atomic::Ordering};

/// The theme family the page is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Dark,
    Light,
    HighContrast,
}

impl Kind {
    pub(crate) fn current() -> Self {
        if HIGH_CONTRAST.load(Ordering::Relaxed) {
            Self::HighContrast
        } else if DARK_THEME.load(Ordering::Relaxed) {
            Self::Dark
        } else {
            Self::Light
        }
    }

    /// Value of the body class and the `data-vscode-theme-kind` attribute.
    pub(crate) fn class(self) -> &'static str {
        match self {
            Self::Dark => "vscode-dark",
            Self::Light => "vscode-light",
            Self::HighContrast => "vscode-high-contrast",
        }
    }

    fn is_light(self) -> bool {
        self == Self::Light
    }
}

fn hex(color: u32) -> String {
    format!("#{:06x}", color & 0x00ff_ffff)
}

/// Core colors that follow the Graphite palette: `(variable, palette slot, light fallback)`.
fn palette_colors() -> Vec<(&'static str, String)> {
    let background = hex(Theme::color(Theme::BG, 0xffffff));
    let side = hex(Theme::color(Theme::SIDEBAR, 0xf3f3f3));
    let text = hex(Theme::color(Theme::TEXT, 0x1e1e1e));
    let dim = hex(Theme::color(Theme::DIM, 0x616161));
    let active = hex(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff));
    let accent = hex(Theme::color(Theme::STATUS, 0x005fb8));
    let cursor = hex(Theme::color(Theme::CURSOR, 0x000000));
    vec![
        ("foreground", text.clone()),
        ("editor-foreground", text.clone()),
        ("editor-background", background.clone()),
        ("editorGutter-background", background.clone()),
        ("sideBar-background", side.clone()),
        ("multiDiffEditor-background", background),
        ("descriptionForeground", dim.clone()),
        ("icon-foreground", text.clone()),
        ("editorLineNumber-foreground", dim.clone()),
        ("editorCursor-foreground", cursor),
        ("list-activeSelectionBackground", active.clone()),
        ("list-activeSelectionForeground", text.clone()),
        ("menu-selectionBackground", active),
        ("menu-selectionForeground", text),
        ("focusBorder", accent.clone()),
        ("button-background", accent.clone()),
        ("textLink-foreground", accent.clone()),
        ("textLink-activeForeground", accent.clone()),
        ("sash-hoverBorder", accent.clone()),
        ("progressBar-background", accent),
        ("editorWidget-background", side.clone()),
        ("editorHoverWidget-background", side.clone()),
        ("editorSuggestWidget-background", side.clone()),
        ("editorActionList-background", side.clone()),
        ("menu-background", side.clone()),
        ("notifications-background", side),
    ]
}

/// Fixed colors: `(variable, dark, light)`. Alpha colors use `rgba()`.
const FIXED: &[(&str, &str, &str)] = &[
    ("errorForeground", "#f85149", "#e51400"),
    ("disabledForeground", "#cccccc60", "#61616180"),
    ("widget-border", "#313131", "#e5e5e5"),
    ("widget-shadow", "rgba(0,0,0,0.36)", "rgba(0,0,0,0.16)"),
    ("contrastBorder", "transparent", "transparent"),
    ("contrastActiveBorder", "transparent", "transparent"),
    ("textCodeBlock-background", "#2b2b2b", "#f8f8f8"),
    ("textPreformat-foreground", "#d0d0d0", "#3b3b3b"),
    ("textPreformat-background", "#3c3c3c", "#0000001f"),
    ("textBlockQuote-background", "#2b2b2b", "#f2f2f2"),
    ("textBlockQuote-border", "#616161", "#c8c8c8"),
    ("button-foreground", "#ffffff", "#ffffff"),
    ("button-hoverBackground", "#026ec1", "#0258a8"),
    (
        "button-border",
        "rgba(255,255,255,0.07)",
        "rgba(0,0,0,0.07)",
    ),
    (
        "button-separator",
        "rgba(255,255,255,0.4)",
        "rgba(255,255,255,0.4)",
    ),
    ("button-secondaryBackground", "#313131", "#e5e5e5"),
    ("button-secondaryForeground", "#cccccc", "#3b3b3b"),
    ("button-secondaryHoverBackground", "#3c3c3c", "#cccccc"),
    ("input-background", "#313131", "#ffffff"),
    ("input-foreground", "#cccccc", "#3b3b3b"),
    ("input-border", "#3c3c3c", "#cecece"),
    ("input-placeholderForeground", "#989898", "#767676"),
    ("inputOption-activeBorder", "#2488db", "#005fb8"),
    (
        "inputOption-hoverBackground",
        "rgba(90,93,94,0.5)",
        "rgba(184,184,184,0.31)",
    ),
    ("inputValidation-infoBorder", "#007acc", "#007acc"),
    ("inlineChatInput-border", "#454545", "#c8c8c8"),
    ("badge-background", "#616161", "#cccccc"),
    ("badge-foreground", "#f8f8f8", "#3b3b3b"),
    ("banner-background", "#04395e", "#a2a2a2"),
    ("banner-foreground", "#ffffff", "#000000"),
    ("banner-iconForeground", "#3794ff", "#1a85ff"),
    ("actionBar-toggledBackground", "#383a49", "#dddddd"),
    ("list-hoverBackground", "#2a2d2e", "#f2f2f2"),
    ("list-dropBackground", "#383b3d", "#d6ebff"),
    ("list-highlightForeground", "#2aaaff", "#0066bf"),
    ("list-focusHighlightForeground", "#2aaaff", "#0066bf"),
    ("menu-foreground", "#cccccc", "#3b3b3b"),
    ("menu-border", "#454545", "#cecece"),
    ("menu-selectionBorder", "transparent", "transparent"),
    ("menu-separatorBackground", "#606060", "#d4d4d4"),
    (
        "toolbar-hoverBackground",
        "rgba(90,93,94,0.31)",
        "rgba(184,184,184,0.31)",
    ),
    (
        "keybindingLabel-background",
        "rgba(128,128,128,0.17)",
        "rgba(221,221,221,0.4)",
    ),
    ("keybindingLabel-foreground", "#cccccc", "#3b3b3b"),
    (
        "keybindingLabel-border",
        "rgba(51,51,51,0.6)",
        "rgba(204,204,204,0.4)",
    ),
    (
        "keybindingLabel-bottomBorder",
        "rgba(68,68,68,0.6)",
        "rgba(187,187,187,0.4)",
    ),
    ("scrollbar-shadow", "#000000", "#dddddd"),
    (
        "scrollbarSlider-background",
        "rgba(121,121,121,0.4)",
        "rgba(100,100,100,0.4)",
    ),
    (
        "scrollbarSlider-hoverBackground",
        "rgba(100,100,100,0.7)",
        "rgba(100,100,100,0.7)",
    ),
    (
        "scrollbarSlider-activeBackground",
        "rgba(191,191,191,0.4)",
        "rgba(0,0,0,0.6)",
    ),
    (
        "minimapSlider-background",
        "rgba(121,121,121,0.2)",
        "rgba(100,100,100,0.2)",
    ),
    (
        "minimapSlider-hoverBackground",
        "rgba(100,100,100,0.35)",
        "rgba(100,100,100,0.35)",
    ),
    (
        "minimapSlider-activeBackground",
        "rgba(191,191,191,0.2)",
        "rgba(0,0,0,0.3)",
    ),
    ("editorWidget-foreground", "#cccccc", "#3b3b3b"),
    ("editorWidget-border", "#454545", "#c8c8c8"),
    ("editorWidget-resizeBorder", "#0078d4", "#005fb8"),
    ("editorHoverWidget-foreground", "#cccccc", "#3b3b3b"),
    ("editorHoverWidget-border", "#454545", "#c8c8c8"),
    (
        "editorHoverWidget-highlightForeground",
        "#2aaaff",
        "#0066bf",
    ),
    (
        "editorHoverWidget-statusBarBackground",
        "#282828",
        "#ececec",
    ),
    ("editorSuggestWidget-foreground", "#cccccc", "#3b3b3b"),
    ("editorSuggestWidget-border", "#454545", "#c8c8c8"),
    (
        "editorSuggestWidget-highlightForeground",
        "#2aaaff",
        "#0066bf",
    ),
    (
        "editorSuggestWidget-focusHighlightForeground",
        "#2aaaff",
        "#0066bf",
    ),
    (
        "editorSuggestWidget-selectedForeground",
        "#ffffff",
        "#000000",
    ),
    (
        "editorSuggestWidgetStatus-foreground",
        "#cccccc80",
        "#3b3b3b80",
    ),
    ("editorActionList-focusBackground", "#04395e", "#e8e8e8"),
    ("editorActionList-focusForeground", "#ffffff", "#000000"),
    ("editorActionList-foreground", "#cccccc", "#3b3b3b"),
    ("editor-selectionBackground", "#264f78", "#add6ff"),
    ("editor-inactiveSelectionBackground", "#3a3d41", "#e5ebf1"),
    (
        "editor-selectionHighlightBackground",
        "#add6ff26",
        "#add6ff80",
    ),
    ("editor-findMatchBackground", "#515c6a", "#a8ac94"),
    (
        "editor-findMatchHighlightBackground",
        "#ea5c0055",
        "#ea5c0055",
    ),
    (
        "editor-findRangeHighlightBackground",
        "#3a3d4166",
        "#b4b4b44d",
    ),
    ("editor-hoverHighlightBackground", "#264f7840", "#add6ff26"),
    ("editor-rangeHighlightBackground", "#ffffff0b", "#fdff0033"),
    ("editor-symbolHighlightBackground", "#ea5c0055", "#ea5c0055"),
    ("editor-wordHighlightBackground", "#575757b8", "#57575740"),
    (
        "editor-wordHighlightStrongBackground",
        "#004972b8",
        "#0e639c40",
    ),
    (
        "editor-wordHighlightTextBackground",
        "#575757b8",
        "#57575740",
    ),
    ("editor-foldBackground", "#264f784d", "#add6ff4d"),
    ("editor-foldPlaceholderForeground", "#808080", "#808080"),
    ("editor-linkedEditingBackground", "#ffffff12", "#ff00004d"),
    ("editor-placeholder-foreground", "#ffffff56", "#00000056"),
    (
        "editor-snippetTabstopHighlightBackground",
        "#7c7c7c4d",
        "#0a326433",
    ),
    (
        "editor-snippetFinalTabstopHighlightBackground",
        "transparent",
        "transparent",
    ),
    ("editorLineNumber-activeForeground", "#cccccc", "#171184"),
    ("editorRuler-foreground", "#5a5a5a", "#d3d3d3"),
    ("editorWhitespace-foreground", "#e3e4e229", "#33333333"),
    ("editorBracketMatch-background", "#0064001a", "#0064001a"),
    ("editorBracketMatch-border", "#888888", "#b9b9b9"),
    ("editorLink-activeForeground", "#4e94ce", "#0000ff"),
    (
        "editorGhostText-foreground",
        "rgba(255,255,255,0.34)",
        "rgba(0,0,0,0.47)",
    ),
    ("editorGhostText-background", "transparent", "transparent"),
    ("editorGhostText-border", "transparent", "transparent"),
    ("editorCodeLens-foreground", "#999999", "#919191"),
    ("editorGutter-commentRangeForeground", "#37373d", "#d5d8e9"),
    (
        "editorGutter-foldingControlForeground",
        "#c5c5c5",
        "#424242",
    ),
    ("editorLightBulb-foreground", "#ffcc00", "#ddb100"),
    ("editorLightBulbAutoFix-foreground", "#75beff", "#007acc"),
    ("editorLightBulbAi-foreground", "#ffcc00", "#ddb100"),
    ("editorError-background", "transparent", "transparent"),
    ("editorError-border", "transparent", "transparent"),
    ("editorWarning-background", "transparent", "transparent"),
    ("editorWarning-border", "transparent", "transparent"),
    ("editorInfo-background", "transparent", "transparent"),
    ("editorInfo-border", "transparent", "transparent"),
    ("editorHint-border", "transparent", "transparent"),
    ("editorStickyScroll-background", "#1f1f1f", "#ffffff"),
    ("editorStickyScroll-border", "transparent", "transparent"),
    ("editorStickyScroll-shadow", "#000000", "#dddddd"),
    ("editorStickyScrollHover-background", "#2a2d2e", "#f0f0f0"),
    (
        "editorUnicodeHighlight-background",
        "#bd9b0326",
        "#bd9b0326",
    ),
    ("editorUnicodeHighlight-border", "#bd9b03", "#cea33d"),
    ("editorUnnecessaryCode-border", "transparent", "transparent"),
    (
        "editorMarkerNavigationInfo-headerBackground",
        "#3794ff1a",
        "#1a85ff1a",
    ),
    ("diffEditor-border", "#444444", "#e5e5e5"),
    ("diffEditor-diagonalFill", "#cccccc33", "#22222233"),
    (
        "diffEditor-insertedLineBackground",
        "#9bb95533",
        "#9bb95533",
    ),
    (
        "diffEditor-insertedTextBackground",
        "#9ccc2c33",
        "#9ccc2c40",
    ),
    (
        "diffEditor-insertedTextBorder",
        "transparent",
        "transparent",
    ),
    ("diffEditor-removedLineBackground", "#ff000033", "#ff000033"),
    ("diffEditor-removedTextBackground", "#ff000033", "#ff000033"),
    ("diffEditor-removedTextBorder", "transparent", "transparent"),
    ("diffEditor-move-border", "#8b8b8b9c", "#8b8b8b9c"),
    ("diffEditor-moveActive-border", "#ffa500", "#ffa500"),
    (
        "diffEditor-unchangedCodeBackground",
        "#74747429",
        "#b8b8b829",
    ),
    ("diffEditor-unchangedRegionBackground", "#252526", "#f3f3f3"),
    ("diffEditor-unchangedRegionForeground", "#cccccc", "#3b3b3b"),
    ("diffEditor-unchangedRegionShadow", "#000000", "#737373bf"),
    (
        "diffEditorGutter-insertedLineBackground",
        "#9ccc2c33",
        "#9ccc2c40",
    ),
    (
        "diffEditorGutter-removedLineBackground",
        "#ff000033",
        "#ff000033",
    ),
    ("multiDiffEditor-border", "#2b2b2b", "#e5e5e5"),
    ("multiDiffEditor-headerBackground", "#262626", "#f3f3f3"),
    ("peekViewEditor-background", "#001f33", "#f2f8fc"),
    (
        "peekViewEditor-matchHighlightBackground",
        "#ea5c004d",
        "#ea5c004d",
    ),
    (
        "peekViewEditor-matchHighlightBorder",
        "transparent",
        "transparent",
    ),
    ("peekViewEditorGutter-background", "#001f33", "#f2f8fc"),
    (
        "peekViewEditorStickyScroll-background",
        "#001f33",
        "#f2f8fc",
    ),
    ("peekViewResult-background", "#252526", "#f3f3f3"),
    ("peekViewResult-fileForeground", "#ffffff", "#1e1e1e"),
    ("peekViewResult-lineForeground", "#bbbbbb", "#646465"),
    (
        "peekViewResult-matchHighlightBackground",
        "#ea5c004d",
        "#ea5c004d",
    ),
    (
        "peekViewResult-selectionBackground",
        "#3399ff33",
        "#3399ff33",
    ),
    ("peekViewResult-selectionForeground", "#ffffff", "#6c6c6c"),
    (
        "gitDecoration-addedResourceForeground",
        "#81b88b",
        "#587c0c",
    ),
    (
        "gitDecoration-deletedResourceForeground",
        "#c74e39",
        "#ad0707",
    ),
    ("problemsErrorIcon-foreground", "#f14c4c", "#e51400"),
    ("problemsWarningIcon-foreground", "#cca700", "#bf8803"),
    ("problemsInfoIcon-foreground", "#3794ff", "#1a85ff"),
    ("panel-border", "#2b2b2b", "#e5e5e5"),
    ("sideBarSectionHeader-border", "#2b2b2b", "#e5e5e5"),
    ("sideBarActivityBarTop-border", "#2b2b2b", "#e5e5e5"),
    ("chat-slashCommandBackground", "#26477866", "#adceff7a"),
    ("chat-slashCommandForeground", "#85b6ff", "#26569e"),
    ("charts-foreground", "#cccccc", "#3b3b3b"),
    ("charts-red", "#f14c4c", "#e51400"),
    ("charts-blue", "#3794ff", "#1a85ff"),
    ("charts-yellow", "#cca700", "#bf8803"),
    ("charts-orange", "#d18616", "#d18616"),
    ("charts-green", "#89d185", "#388a34"),
    ("charts-purple", "#b180d7", "#652d90"),
    ("terminal-ansiBlack", "#000000", "#000000"),
    ("terminal-ansiRed", "#cd3131", "#cd3131"),
    ("terminal-ansiGreen", "#0dbc79", "#107c10"),
    ("terminal-ansiYellow", "#e5e510", "#949800"),
    ("terminal-ansiBlue", "#2472c8", "#0451a5"),
    ("terminal-ansiMagenta", "#bc3fbc", "#bc05bc"),
    ("terminal-ansiCyan", "#11a8cd", "#0598bc"),
    ("terminal-ansiWhite", "#e5e5e5", "#555555"),
    ("terminal-ansiBrightBlack", "#666666", "#666666"),
    ("sash-size", "4px", "4px"),
    ("sash-hover-size", "4px", "4px"),
];

/// Symbol icon colors share one default in both themes.
const SYMBOL_KINDS: &[&str] = &[
    "array",
    "boolean",
    "class",
    "color",
    "constant",
    "constructor",
    "enumerator",
    "enumeratorMember",
    "event",
    "field",
    "file",
    "folder",
    "function",
    "interface",
    "key",
    "keyword",
    "method",
    "module",
    "namespace",
    "null",
    "number",
    "object",
    "operator",
    "package",
    "property",
    "reference",
    "snippet",
    "string",
    "struct",
    "text",
    "typeParameter",
    "unit",
    "variable",
];

const FONT_FAMILY: &str = if cfg!(windows) {
    "\"Segoe UI\", sans-serif"
} else {
    "system-ui, -apple-system, \"Segoe UI\", \"Roboto\", sans-serif"
};
const EDITOR_FONT_FAMILY: &str = "Consolas, \"Courier New\", monospace";

/// Every `--vscode-*` variable for `kind`, as `(name without the prefix, value)`.
pub(crate) fn variables(kind: Kind) -> Vec<(String, String)> {
    let light = kind.is_light();
    let mut all: Vec<(String, String)> = FIXED
        .iter()
        .map(|(name, dark, bright)| {
            (
                (*name).to_owned(),
                (if light { bright } else { dark }).to_string(),
            )
        })
        .collect();
    let symbol = if light { "#3b3b3b" } else { "#cccccc" };
    all.extend(
        SYMBOL_KINDS
            .iter()
            .map(|kind| (format!("symbolIcon-{kind}Foreground"), symbol.to_owned())),
    );
    all.extend([
        ("font-family".into(), FONT_FAMILY.into()),
        ("font-size".into(), "13px".into()),
        ("font-weight".into(), "normal".into()),
        ("editor-font-family".into(), EDITOR_FONT_FAMILY.into()),
        ("editor-font-size".into(), "14px".into()),
        ("editor-font-weight".into(), "normal".into()),
        ("chat-font-family".into(), FONT_FAMILY.into()),
        ("chat-font-size".into(), "13px".into()),
        ("editorCodeLens-fontSize".into(), "12px".into()),
        ("editorCodeLens-lineHeight".into(), "18px".into()),
        ("editorCodeLens-fontFamily".into(), FONT_FAMILY.into()),
    ]);
    // The palette-driven colors come last so they win over any fixed entry of the same name.
    all.extend(
        palette_colors()
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value)),
    );
    if kind == Kind::HighContrast {
        all.push(("contrastBorder".into(), "#6fc3df".into()));
        all.push(("contrastActiveBorder".into(), "#f38518".into()));
    }
    all
}

/// Base rules every webview gets, written in terms of the variables above.
const BASE_STYLES: &str = "html{scrollbar-color:var(--vscode-scrollbarSlider-background) var(--vscode-editor-background)}\
body{background-color:transparent;color:var(--vscode-editor-foreground);font-family:var(--vscode-font-family);font-weight:var(--vscode-font-weight);font-size:var(--vscode-font-size);margin:0;padding:0 20px}\
img,video{max-width:100%;max-height:100%}\
a,a code{color:var(--vscode-textLink-foreground)}\
a:hover{color:var(--vscode-textLink-activeForeground)}\
a:focus,input:focus,select:focus,textarea:focus{outline:1px solid -webkit-focus-ring-color;outline-offset:-1px}\
code{color:var(--vscode-textPreformat-foreground);background-color:var(--vscode-textPreformat-background);padding:1px 3px;border-radius:4px}\
pre code{padding:0}\
blockquote{background:var(--vscode-textBlockQuote-background);border-color:var(--vscode-textBlockQuote-border)}\
kbd{background-color:var(--vscode-keybindingLabel-background);color:var(--vscode-keybindingLabel-foreground);border-style:solid;border-width:1px;border-radius:3px;border-color:var(--vscode-keybindingLabel-border);border-bottom-color:var(--vscode-keybindingLabel-bottomBorder);box-shadow:inset 0 -1px 0 var(--vscode-widget-shadow);vertical-align:middle;padding:1px 3px}\
::-webkit-scrollbar{width:10px;height:10px}\
::-webkit-scrollbar-corner{background-color:var(--vscode-editor-background)}\
::-webkit-scrollbar-thumb{background-color:var(--vscode-scrollbarSlider-background)}\
::-webkit-scrollbar-thumb:hover{background-color:var(--vscode-scrollbarSlider-hoverBackground)}\
::-webkit-scrollbar-thumb:active{background-color:var(--vscode-scrollbarSlider-activeBackground)}";

/// The `<style>` element declaring the variables and the base rules.
pub(crate) fn style_element(kind: Kind) -> String {
    let mut css = String::from(":root{");
    for (name, value) in variables(kind) {
        let _ = write!(css, "--vscode-{name}:{value};");
    }
    css.push('}');
    css.push_str(BASE_STYLES);
    format!("<style id=\"graphite-theme\">{css}</style>")
}

/// Adds the theme class and attributes to the page's `<body>` tag.
pub(crate) fn themed_body(html: &str, kind: Kind) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<body") else {
        return html.to_owned();
    };
    // `<body` must be followed by whitespace, `>` or `/`, not a longer tag name.
    if !matches!(
        lower.as_bytes().get(start + 5),
        Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')
    ) {
        return html.to_owned();
    }
    let Some(end) = html[start..].find('>').map(|offset| start + offset) else {
        return html.to_owned();
    };
    let tag = &html[start..end];
    let class = kind.class();
    let attributes =
        format!(" data-vscode-theme-kind=\"{class}\" data-vscode-theme-name=\"Graphite\"");
    let tag = if let Some(position) = tag.to_ascii_lowercase().find("class=\"") {
        let at = start + position + "class=\"".len();
        format!("{}{class} {}", &html[start..at], &html[at..end])
    } else {
        format!("{tag} class=\"{class}\"")
    };
    format!("{}{tag}{attributes}{}", &html[..start], &html[end..])
}

/// The page background to show before the page paints, as RGBA.
pub(crate) fn background() -> (u8, u8, u8, u8) {
    let color = Theme::color(Theme::SIDEBAR, 0xf3f3f3);
    ((color >> 16) as u8, (color >> 8) as u8, color as u8, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(vars: &[(String, String)], name: &str) -> Option<String> {
        vars.iter()
            .rev()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn variables_cover_what_pages_need_and_follow_the_theme_kind() {
        let dark = variables(Kind::Dark);
        let light = variables(Kind::Light);
        for name in [
            "foreground",
            "editor-background",
            "sideBar-background",
            "button-background",
            "input-background",
            "font-family",
            "chat-font-size",
            "symbolIcon-classForeground",
            "terminal-ansiRed",
        ] {
            assert!(value(&dark, name).is_some(), "dark: {name}");
            assert!(value(&light, name).is_some(), "light: {name}");
        }
        assert_eq!(value(&dark, "input-background").unwrap(), "#313131");
        assert_eq!(value(&light, "input-background").unwrap(), "#ffffff");
        assert_eq!(
            value(&variables(Kind::HighContrast), "contrastBorder").unwrap(),
            "#6fc3df"
        );
        // Colors are valid CSS values: hex, rgba(), a keyword or a length.
        for (name, color) in dark.iter().chain(light.iter()) {
            assert!(
                !color.is_empty() && !color.contains([';', '{', '}', '<']),
                "{name}: {color}"
            );
        }
    }

    #[test]
    fn the_style_element_declares_prefixed_variables_and_base_rules() {
        let style = style_element(Kind::Dark);
        assert!(style.starts_with("<style id=\"graphite-theme\">:root{--vscode-"));
        assert!(style.contains("--vscode-editor-foreground:"));
        assert!(style
            .contains("body{background-color:transparent;color:var(--vscode-editor-foreground)"));
        assert!(style.ends_with("</style>"));
    }

    #[test]
    fn the_body_gets_the_theme_class_and_attributes_without_losing_its_own() {
        let plain = themed_body("<html><body><p>x</p></body></html>", Kind::Dark);
        assert!(
            plain.contains("<body class=\"vscode-dark\" data-vscode-theme-kind=\"vscode-dark\"")
        );
        let classed = themed_body("<body class=\"app wide\" id=\"b\">x</body>", Kind::Light);
        assert!(classed.contains("<body class=\"vscode-light app wide\" id=\"b\" data-vscode-theme-kind=\"vscode-light\""));
        // Not a body tag, or no body: unchanged.
        assert_eq!(
            themed_body("<bodyguard>x</bodyguard>", Kind::Dark),
            "<bodyguard>x</bodyguard>"
        );
        assert_eq!(themed_body("<p>x</p>", Kind::Dark), "<p>x</p>");
    }

    #[test]
    fn the_kind_class_names_match_what_pages_test_for() {
        assert_eq!(Kind::Dark.class(), "vscode-dark");
        assert_eq!(Kind::Light.class(), "vscode-light");
        assert_eq!(Kind::HighContrast.class(), "vscode-high-contrast");
    }
}
