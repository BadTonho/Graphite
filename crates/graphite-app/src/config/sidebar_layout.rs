use std::{fs, path::Path};

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivityBarPosition {
    #[default]
    Default,
    Top,
    Bottom,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivityBarSize {
    #[default]
    Default,
    Compact,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct ActivityBarPreferences {
    pub position: ActivityBarPosition,
    pub size: ActivityBarSize,
    pub primary_sidebar_right: bool,
    pub hidden_items: std::collections::HashSet<String>,
}

pub(crate) fn load_activity_bar_preferences(profile_root: &Path) -> ActivityBarPreferences {
    fs::read(profile_root.join("activity-bar.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(crate) fn save_activity_bar_preferences(
    profile_root: &Path,
    preferences: &ActivityBarPreferences,
) -> Result<(), String> {
    fs::create_dir_all(profile_root).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
    fs::write(profile_root.join("activity-bar.json"), bytes).map_err(|error| error.to_string())
}

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

/// Narrowest the secondary side bar can be dragged to, in pixels.
pub(crate) const MIN_SECONDARY_WIDTH: f32 = 200.0;
const SECONDARY_LAYOUT_FILE: &str = "secondary-sidebar-layout.json";

/// The width the user dragged the secondary side bar to, if any. Invalid data yields `None`.
pub(crate) fn load_secondary_sidebar_width(profile_root: &Path) -> Option<f32> {
    let contents = fs::read_to_string(profile_root.join(SECONDARY_LAYOUT_FILE)).ok()?;
    let width = serde_json::from_str::<f32>(&contents).ok()?;
    (width.is_finite() && (MIN_SECONDARY_WIDTH..=MAX_PERSISTED_WIDTH).contains(&width))
        .then_some(width)
}

pub(crate) fn save_secondary_sidebar_width(profile_root: &Path, width: f32) -> Result<(), String> {
    if !width.is_finite() || !(MIN_SECONDARY_WIDTH..=MAX_PERSISTED_WIDTH).contains(&width) {
        return Err("Secondary side bar width is outside the supported range".into());
    }
    fs::create_dir_all(profile_root).map_err(|error| error.to_string())?;
    let contents = serde_json::to_string_pretty(&width).map_err(|error| error.to_string())?;
    fs::write(profile_root.join(SECONDARY_LAYOUT_FILE), contents).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_width_round_trips_and_rejects_invalid_data() {
        let temp = crate::test_support::TempDir::new();
        assert_eq!(load_secondary_sidebar_width(temp.path()), None);
        save_secondary_sidebar_width(temp.path(), 420.0).unwrap();
        assert_eq!(load_secondary_sidebar_width(temp.path()), Some(420.0));
        assert!(save_secondary_sidebar_width(temp.path(), 50.0).is_err());
        fs::write(temp.path().join(SECONDARY_LAYOUT_FILE), "10").unwrap();
        assert_eq!(load_secondary_sidebar_width(temp.path()), None);
        fs::write(temp.path().join(SECONDARY_LAYOUT_FILE), "nope").unwrap();
        assert_eq!(load_secondary_sidebar_width(temp.path()), None);
    }

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

    #[test]
    fn activity_bar_preferences_round_trip_per_profile_and_default_on_invalid_data() {
        let temp = crate::test_support::TempDir::new();
        let mut preferences = ActivityBarPreferences {
            position: ActivityBarPosition::Bottom,
            size: ActivityBarSize::Compact,
            primary_sidebar_right: true,
            ..ActivityBarPreferences::default()
        };
        preferences.hidden_items.insert("search".into());
        save_activity_bar_preferences(temp.path(), &preferences).unwrap();
        assert_eq!(load_activity_bar_preferences(temp.path()), preferences);
        fs::write(temp.path().join("activity-bar.json"), "{invalid").unwrap();
        assert_eq!(
            load_activity_bar_preferences(temp.path()),
            ActivityBarPreferences::default()
        );
    }
}
