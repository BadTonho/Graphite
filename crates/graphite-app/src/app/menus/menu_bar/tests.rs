use super::*;

fn layout(width: f32, scale: f32) -> TitlebarLayout {
    TitlebarLayout::new(
        width,
        32.0 * scale,
        8.0 * scale,
        scale,
        [72.0, 64.0, 96.0, 56.0, 32.0, 64.0, 72.0, 56.0].map(|width| width * scale),
        0.0,
    )
}

#[test]
fn titlebar_geometry_keeps_icon_menus_and_controls_separate_at_common_scales() {
    for (width, scale) in [(1280.0, 1.0), (1920.0, 1.5), (900.0, 1.25)] {
        let layout = layout(width, scale);
        assert!(layout.icon.x + layout.icon.width <= layout.menus[0].x);
        for pair in layout.menus.windows(2) {
            assert!(pair[0].x + pair[0].width <= pair[1].x);
        }
        let last = layout.menus.last().unwrap();
        assert!(last.x + last.width <= layout.command_center.back.x);
        assert!(
            layout.command_center.back.x + layout.command_center.back.width
                <= layout.command_center.forward.x
        );
        assert!(
            layout.command_center.forward.x + layout.command_center.forward.width
                <= layout.command_center.field.x
        );
        assert_eq!(
            layout.command_center.field.x + layout.command_center.field.width * 0.5,
            width * 0.5
        );
        assert!(
            layout.command_center.field.x + layout.command_center.field.width
                <= layout.controls.minimize.x
        );
        assert!(
            layout.controls.minimize.x + layout.controls.minimize.width
                <= layout.controls.maximize.x
        );
        assert!(
            layout.controls.maximize.x + layout.controls.maximize.width <= layout.controls.close.x
        );
        assert!(layout.controls.close.x + layout.controls.close.width <= width);
    }
}

#[test]
fn narrow_titlebar_moves_remaining_menus_into_a_bounded_overflow_menu() {
    let layout_600 = layout(600.0, 1.0);
    let more = layout_600
        .more_rect()
        .expect("narrow layout should expose More");
    assert!(!layout_600.overflowed.is_empty());
    assert_eq!(
        layout_600.overflowed.len() + layout_600.menus.len() - 1,
        WorkbenchMenu::ALL.len()
    );
    assert!(more.x + more.width <= layout_600.command_center.back.x);
    let popup = layout_600.overflow_popup(8.0, 600.0);
    assert!(popup.x >= 0.0);
    assert!(popup.x + popup.width <= 600.0);
    for width in [320.0, 280.0] {
        let layout = layout(width, 1.0);
        if let Some(more) = layout.more_rect() {
            assert!(more.x + more.width <= layout.command_center.back.x);
        }
        assert!(
            layout.command_center.back.x + layout.command_center.back.width
                <= layout.command_center.forward.x
        );
        assert!(
            layout.command_center.forward.x + layout.command_center.forward.width
                <= layout.command_center.field.x
        );
        assert!(
            layout.command_center.field.x + layout.command_center.field.width
                <= layout.controls.minimize.x
        );
    }
}

#[test]
fn titlebar_control_hit_regions_are_distinct_and_resize_edges_are_shared() {
    let layout = layout(1000.0, 1.0);
    let controls = layout.controls;
    assert_eq!(
        controls.at(controls.minimize.x + 2.0, 10.0),
        Some(WindowControl::Minimize)
    );
    assert_eq!(
        controls.at(controls.maximize.x + 2.0, 10.0),
        Some(WindowControl::MaximizeRestore)
    );
    assert_eq!(
        controls.at(controls.close.x + 2.0, 10.0),
        Some(WindowControl::Close)
    );
    assert_eq!(
        controls.at(controls.close.x, 10.0),
        Some(WindowControl::Close)
    );
    assert_eq!(
        resize_direction_at(1000.0, 700.0, 6.0, 3.0, 3.0, false),
        Some(winit::window::ResizeDirection::NorthWest)
    );
    assert_eq!(
        resize_direction_at(1000.0, 700.0, 6.0, 3.0, 3.0, true),
        None
    );
    assert_eq!(
        resize_direction_for_titlebar(&layout, 1000.0, 700.0, 6.0, 45.0, 2.0, false),
        None,
        "the top border must not block menu clicks"
    );
    assert_eq!(
        resize_direction_for_titlebar(&layout, 1000.0, 700.0, 6.0, 999.0, 2.0, false),
        Some(winit::window::ResizeDirection::NorthEast),
        "the window corner remains resizable"
    );
}

#[test]
fn menu_bar_contains_all_eight_top_level_menus_and_height_tracks_font_scale() {
    assert_eq!(WorkbenchMenu::ALL.len(), 8);
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
    app.sidebar_scale_factor = 2.0;
    assert_eq!(app.top_menu_height(), app.font_size * 4.2);
}

#[test]
fn keyboard_opens_and_switches_top_level_menus() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
    assert!(app.workbench_menu_key(&Key::Named(NamedKey::F10)));
    assert_eq!(app.workbench_menu, Some(WorkbenchMenu::File));
    assert!(app.workbench_menu_key(&Key::Named(NamedKey::ArrowRight)));
    assert_eq!(app.workbench_menu, Some(WorkbenchMenu::Edit));
    assert!(app.workbench_menu_key(&Key::Named(NamedKey::Escape)));
    assert!(app.workbench_menu.is_none());
}

#[test]
fn view_menu_is_separate_from_the_activity_context_menu_and_has_appearance_controls() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
    app.activity_bar_preferences.position = crate::config::ActivityBarPosition::Hidden;
    app.workbench_menu = Some(WorkbenchMenu::View);
    app.view_menu_open(0.0, 800.0, 600.0);
    assert!(app.activity_bar_menu.is_none());
    assert_eq!(
        app.view_menu_open_page(),
        Some(super::super::view_menu::ViewPage::Root)
    );
    let (x, y) = app.view_menu_row_point(0, 3).unwrap();
    app.view_menu_click(x, y);
    assert_eq!(
        app.view_menu_open_page(),
        Some(super::super::view_menu::ViewPage::Appearance)
    );
    let (x, y) = app.view_menu_row_point(1, 11).unwrap();
    app.view_menu_click(x, y);
    assert_eq!(
        app.view_menu_open_page(),
        Some(super::super::view_menu::ViewPage::ActivityPosition)
    );
    let (x, y) = app.view_menu_row_point(2, 0).unwrap();
    app.view_menu_click(x, y);
    assert_eq!(
        app.activity_bar_preferences.position,
        crate::config::ActivityBarPosition::Default
    );
}

#[test]
fn titlebar_update_button_appears_only_when_update_exists_and_stays_left_of_layout_controls() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    let idle = app.titlebar_layout(1280.0, 8.0);
    assert!(idle.update_button.is_none());

    app.updates.status = crate::app::updates::state::UpdateStatus::Available(
        crate::app::updates::state::ReleaseInfo {
            tag_name: "v0.2.0".into(),
            version: "0.2.0".into(),
            prerelease: false,
            notes: String::new(),
            html_url: "https://github.com/BadTonho/Graphite/releases/tag/v0.2.0".into(),
            asset: None,
        },
    );
    let with_update = app.titlebar_layout(1280.0, 8.0);
    let btn = with_update.update_button.expect("update button visible");
    assert!(btn.x + btn.width <= with_update.layout_controls[0].1 .0);
    assert!(with_update.command_center.field.x + with_update.command_center.field.width <= btn.x);
}
