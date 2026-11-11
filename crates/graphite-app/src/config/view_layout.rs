use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PanelPosition {
    #[default]
    Bottom,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WhitespaceRendering {
    #[default]
    None,
    Boundary,
    Selection,
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct ViewLayoutPreferences {
    pub menu_bar_visible: bool,
    pub primary_sidebar_visible: bool,
    pub secondary_sidebar_visible: bool,
    pub status_bar_visible: bool,
    pub panel_visible: bool,
    pub panel_position: PanelPosition,
    pub panel_align_center: bool,
    pub panel_align_right: bool,
    pub tabs_visible: bool,
    pub editor_actions_right: bool,
    pub minimap_visible: bool,
    pub breadcrumbs_visible: bool,
    pub sticky_scroll: bool,
    pub whitespace: WhitespaceRendering,
    pub control_characters: bool,
    pub zoom_percent: u16,
}

impl Default for ViewLayoutPreferences {
    fn default() -> Self {
        Self {
            menu_bar_visible: true,
            primary_sidebar_visible: true,
            secondary_sidebar_visible: true,
            status_bar_visible: true,
            panel_visible: true,
            panel_position: PanelPosition::Bottom,
            panel_align_center: false,
            panel_align_right: false,
            tabs_visible: true,
            editor_actions_right: true,
            minimap_visible: false,
            breadcrumbs_visible: false,
            sticky_scroll: true,
            whitespace: WhitespaceRendering::None,
            control_characters: false,
            zoom_percent: 100,
        }
    }
}

impl ViewLayoutPreferences {
    pub(crate) fn normalize(&mut self) {
        self.zoom_percent = self.zoom_percent.clamp(50, 200);
    }
}

pub(crate) fn load_view_layout_preferences(profile_root: &Path) -> ViewLayoutPreferences {
    let mut preferences: ViewLayoutPreferences = fs::read(profile_root.join("view-layout.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    preferences.normalize();
    preferences
}

pub(crate) fn save_view_layout_preferences(
    profile_root: &Path,
    preferences: &ViewLayoutPreferences,
) -> Result<(), String> {
    fs::create_dir_all(profile_root).map_err(|error| error.to_string())?;
    let mut preferences = preferences.clone();
    preferences.normalize();
    fs::write(
        profile_root.join("view-layout.json"),
        serde_json::to_vec_pretty(&preferences).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_preferences_round_trip_per_profile_and_invalid_zoom_is_clamped() {
        let temp = crate::test_support::TempDir::new();
        let mut preferences = ViewLayoutPreferences::default();
        preferences.menu_bar_visible = false;
        preferences.panel_position = PanelPosition::Right;
        preferences.panel_align_right = true;
        preferences.zoom_percent = 150;
        save_view_layout_preferences(temp.path(), &preferences).unwrap();
        assert_eq!(load_view_layout_preferences(temp.path()), preferences);
        fs::write(
            temp.path().join("view-layout.json"),
            r#"{"zoom_percent":999}"#,
        )
        .unwrap();
        assert_eq!(load_view_layout_preferences(temp.path()).zoom_percent, 200);
    }
}
