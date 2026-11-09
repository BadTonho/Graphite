use super::*;

pub(crate) struct Theme;
pub(crate) static DARK_THEME: AtomicBool = AtomicBool::new(true);
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
fn parse_hex_color(s: &str) -> Option<u32> {
    let s = s.strip_prefix('#')?;
    if s.len() < 6 {
        return None;
    }
    u32::from_str_radix(&s[..6], 16).ok()
}
