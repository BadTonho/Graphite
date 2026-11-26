use super::*;

pub(crate) struct Theme;
pub(crate) static DARK_THEME: AtomicBool = AtomicBool::new(true);
/// High contrast uses the dark palette with near-black and near-white surfaces.
pub(crate) static HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);

/// Maps one palette color to its high-contrast value. Dark surfaces become pure black and light
/// text becomes pure white; mid-tones (syntax colors, dim text) are kept. The active row gets a
/// saturated blue so the selection stays visible.
pub(crate) fn high_contrast_color(color: u32) -> u32 {
    if color == Theme::ACTIVE_ROW {
        return 0x006ab1;
    }
    let (red, green, blue) = ((color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff);
    let luma = (red * 299 + green * 587 + blue * 114) / 1000;
    if luma <= 0x50 {
        0x000000
    } else if luma >= 0xa0 {
        0xffffff
    } else {
        color
    }
}
static THEME_OVERRIDES: OnceLock<RwLock<HashMap<u32, u32>>> = OnceLock::new();
impl Theme {
    pub(crate) const BG: u32 = 0x1e1e1e;
    pub(crate) const ACTIVITY_BAR: u32 = 0x333333;
    pub(crate) const SIDEBAR: u32 = 0x252526;
    pub(crate) const TEXT: u32 = 0xd4d4d4;
    pub(crate) const DIM: u32 = 0x858585;
    pub(crate) const CURSOR: u32 = 0xaeafad;
    pub(crate) const ACTIVE_ROW: u32 = 0x37373d;
    pub(crate) const STATUS: u32 = 0x007acc;

    pub(crate) fn color(dark: u32, light: u32) -> u32 {
        if HIGH_CONTRAST.load(Ordering::Relaxed) {
            return high_contrast_color(dark);
        }
        let base = if DARK_THEME.load(Ordering::Relaxed) {
            dark
        } else {
            light
        };
        THEME_OVERRIDES
            .get_or_init(Default::default)
            .read()
            .ok()
            .and_then(|m| m.get(&base).copied())
            .unwrap_or(base)
    }

    pub(crate) fn class_color(class: Class) -> u32 {
        match class {
            Class::Default | Class::Punctuation | Class::Operator => {
                Self::color(Self::TEXT, 0x1e1e1e)
            }
            Class::Keyword => Self::color(0x569cd6, 0x0000ff),
            Class::Function => Self::color(0xdcdcaa, 0x795e26),
            Class::Type => Self::color(0x4ec9b0, 0x267f99),
            Class::String => Self::color(0xce9178, 0xa31515),
            Class::Number | Class::Constant => Self::color(0xb5cea8, 0x098658),
            Class::Comment => Self::color(0x6a9955, 0x008000),
            Class::Property | Class::Variable => Self::color(0x9cdcfe, 0x001080),
            Class::BracketLevel1 => Self::color(0xffd700, 0xb8860b),
            Class::BracketLevel2 => Self::color(0xda70d6, 0x9932cc),
            Class::BracketLevel3 => Self::color(0x179fff, 0x1f6fd0),
        }
    }

    pub(crate) fn set_imported_colors(colors: HashMap<String, String>) {
        let mappings = [
            ("editor.background", Self::BG, 0xffffff),
            ("activityBar.background", Self::ACTIVITY_BAR, 0xe8e8e8),
            ("sideBar.background", Self::SIDEBAR, 0xf3f3f3),
            ("editor.foreground", Self::TEXT, 0x1e1e1e),
            ("editorLineNumber.foreground", Self::DIM, 0x616161),
            ("editorCursor.foreground", Self::CURSOR, 0x202020),
            ("editor.lineHighlightBackground", Self::ACTIVE_ROW, 0xd6ebff),
            ("statusBar.background", Self::STATUS, 0x005a9e),
            ("syntax.keyword", 0x569cd6, 0x0000ff),
            ("syntax.function", 0xdcdcaa, 0x795e26),
            ("syntax.type", 0x4ec9b0, 0x267f99),
            ("syntax.string", 0xce9178, 0xa31515),
            ("syntax.number", 0xb5cea8, 0x098658),
            ("syntax.comment", 0x6a9955, 0x008000),
            ("syntax.variable", 0x9cdcfe, 0x001080),
        ];
        let mut overrides = HashMap::new();
        for (key, dark, light) in mappings {
            if let Some(value) = colors.get(key).and_then(|v| parse_hex_color(v)) {
                overrides.insert(dark, value);
                overrides.insert(light, value);
            }
        }
        if let Ok(mut current) = THEME_OVERRIDES.get_or_init(Default::default).write() {
            *current = overrides;
        }
    }
}
/// `#RGB`, `#RGBA`, `#RRGGBB`, or `#RRGGBBAA` as `0xRRGGBB`; the alpha part is ignored. Anything
/// else, including text with non-ASCII characters, is not a color.
fn parse_hex_color(s: &str) -> Option<u32> {
    let digits = s.trim().strip_prefix('#')?;
    if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = match digits.len() {
        3 | 4 => digits[..3].chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => digits[..6].to_owned(),
        _ => return None,
    };
    u32::from_str_radix(&rgb, 16).ok()
}

#[cfg(test)]
mod high_contrast_tests {
    use super::*;

    #[test]
    fn surfaces_and_text_snap_to_black_and_white_while_accents_are_kept() {
        assert_eq!(high_contrast_color(0x1e1e1e), 0x000000);
        assert_eq!(high_contrast_color(0x252526), 0x000000);
        assert_eq!(high_contrast_color(0xd4d4d4), 0xffffff);
        assert_eq!(high_contrast_color(0x569cd6), 0x569cd6);
        assert_eq!(high_contrast_color(0x858585), 0x858585);
        assert_eq!(high_contrast_color(Theme::ACTIVE_ROW), 0x006ab1);
    }

    #[test]
    fn hex_colors_parse_in_every_css_length() {
        assert_eq!(parse_hex_color("#1e1e1e"), Some(0x1e1e1e));
        assert_eq!(parse_hex_color("#1E1E1E80"), Some(0x1e1e1e));
        assert_eq!(parse_hex_color("#abc"), Some(0xaabbcc));
        assert_eq!(parse_hex_color("#abcd"), Some(0xaabbcc));
        assert_eq!(parse_hex_color(" #ffffff "), Some(0xffffff));
    }

    #[test]
    fn malformed_colors_are_ignored_without_panicking() {
        for value in [
            "",
            "#",
            "1e1e1e",
            "#12345",
            "#1234567",
            "#12345é",
            "#abcdeé",
            "#éééé",
            "#+12345",
            "#gggggg",
            "#1e1e1e1e1e",
        ] {
            assert_eq!(parse_hex_color(value), None, "{value}");
        }
    }
}
