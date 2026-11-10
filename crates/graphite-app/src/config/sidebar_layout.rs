use std::{fs, path::Path};

pub(crate) const DEFAULT_WIDTH: f32 = 260.0;
pub(crate) const MIN_WIDTH: f32 = 180.0;
pub(crate) const MAX_PERSISTED_WIDTH: f32 = 16_384.0;

pub(crate) fn clamp_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_WIDTH, MAX_PERSISTED_WIDTH)
    } else {
        DEFAULT_WIDTH
    }
}

pub(crate) fn load_sidebar_width(profile_root: &Path) -> f32 {
    let path = profile_root.join("sidebar-layout.json");
    let Ok(contents) = fs::read_to_string(path) else {
        return DEFAULT_WIDTH;
    };
    let Ok(width) = serde_json::from_str::<f32>(&contents) else {
        return DEFAULT_WIDTH;
    };
    if width.is_finite() && (MIN_WIDTH..=MAX_PERSISTED_WIDTH).contains(&width) {
        width
    } else {
        DEFAULT_WIDTH
    }
}

pub(crate) fn save_sidebar_width(profile_root: &Path, width: f32) -> Result<(), String> {
    if !width.is_finite() || !(MIN_WIDTH..=MAX_PERSISTED_WIDTH).contains(&width) {
        return Err("Sidebar width is outside the supported range".into());
    }
    fs::create_dir_all(profile_root).map_err(|error| error.to_string())?;
    let contents = serde_json::to_string_pretty(&width).map_err(|error| error.to_string())?;
    fs::write(profile_root.join("sidebar-layout.json"), contents).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_width_defaults_clamps_and_rejects_non_finite_values() {
        assert_eq!(clamp_width(100.0), MIN_WIDTH);
        assert_eq!(clamp_width(20_000.0), MAX_PERSISTED_WIDTH);
        assert_eq!(clamp_width(f32::NAN), DEFAULT_WIDTH);
        assert_eq!(clamp_width(DEFAULT_WIDTH), DEFAULT_WIDTH);
    }

    #[test]
    fn sidebar_width_round_trips_per_profile_and_invalid_data_uses_default() {
        let temp = crate::test_support::TempDir::new();
        assert_eq!(load_sidebar_width(temp.path()), DEFAULT_WIDTH);
        save_sidebar_width(temp.path(), 900.0).unwrap();
        assert_eq!(load_sidebar_width(temp.path()), 900.0);
        fs::write(temp.path().join("sidebar-layout.json"), "480").unwrap();
        assert_eq!(load_sidebar_width(temp.path()), 480.0);
        fs::write(temp.path().join("sidebar-layout.json"), "-10").unwrap();
        assert_eq!(load_sidebar_width(temp.path()), DEFAULT_WIDTH);
    }
}
