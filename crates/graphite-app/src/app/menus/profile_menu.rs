use super::*;
use crate::app::user_data_sync::state::SyncBackend;

const ROW_HEIGHT: f32 = 27.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 7.0;
const MIN_WIDTH: f32 = 248.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProfileMenuRow {
    Profile {
        name: String,
        active: bool,
    },
    Separator,
    Create,
    Unavailable,
    /// Start the GitHub sign-in of this profile.
    GitHubSignIn,
    /// The signed-in account; informational, not selectable.
    GitHubAccount {
        login: String,
    },
    GitHubSignOut,
    /// What the sync is doing or did last; informational, not selectable.
    SyncStatus {
        text: String,
    },
    SyncTurnOn,
    SyncTurnOnRepository,
    SyncNow,
    SyncTurnOff,
}

impl ProfileMenuRow {
    /// Rows that can be chosen with the keyboard or the pointer's action.
    fn is_selectable(&self) -> bool {
        !matches!(
            self,
            ProfileMenuRow::Separator
                | ProfileMenuRow::Unavailable
                | ProfileMenuRow::GitHubAccount { .. }
                | ProfileMenuRow::SyncStatus { .. }
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProfileMenu {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub rows: Vec<ProfileMenuRow>,
}

fn row_height(row: &ProfileMenuRow) -> f32 {
    if matches!(row, ProfileMenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn menu_height(rows: &[ProfileMenuRow]) -> f32 {
    PADDING * 2.0 + rows.iter().map(row_height).sum::<f32>()
}

fn menu_rect(menu: &ProfileMenu) -> (f32, f32, f32, f32) {
    (menu.x, menu.y, menu.width, menu_height(&menu.rows))
}

impl App {
    pub(crate) fn profile_button_accessible_label(&self) -> String {
        let name = self
            .profiles
            .as_ref()
            .map(|profiles| profiles.active())
            .unwrap_or("Default");
        self.tf("profile_menu.button_label", &[("name", &name)])
    }

    pub(crate) fn open_profile_menu(
        &mut self,
        button: (f32, f32, f32, f32),
        window_width: f32,
        window_height: f32,
    ) {
        self.activity_bar_menu = None;
        self.settings_menu = None;
        self.workbench_menu = None;

        let active = self
            .profiles
            .as_ref()
            .map(|profiles| profiles.active().to_owned());
        let profiles = self.user_profiles().unwrap_or_default();
        let mut rows = profiles
            .into_iter()
            .map(|name| ProfileMenuRow::Profile {
                active: active.as_deref() == Some(name.as_str()),
                name,
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            rows.push(ProfileMenuRow::Unavailable);
        } else {
            rows.push(ProfileMenuRow::Separator);
        }
        if self.profiles.is_some() {
            rows.push(ProfileMenuRow::Create);
            rows.push(ProfileMenuRow::Separator);
            match &self.github_session {
                Some(session) => {
                    rows.push(ProfileMenuRow::GitHubAccount {
                        login: session.login.clone(),
                    });
                    rows.push(ProfileMenuRow::GitHubSignOut);
                }
                None => rows.push(ProfileMenuRow::GitHubSignIn),
            }
            rows.push(ProfileMenuRow::Separator);
            if self.sync.persisted.enabled {
                rows.push(ProfileMenuRow::SyncStatus {
                    text: self.sync_status_text(),
                });
                rows.push(ProfileMenuRow::SyncNow);
                rows.push(ProfileMenuRow::SyncTurnOff);
            } else {
                rows.push(ProfileMenuRow::SyncTurnOn);
                rows.push(ProfileMenuRow::SyncTurnOnRepository);
            }
        }

        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let width = rows
            .iter()
            .map(|row| {
                let label = match row {
                    ProfileMenuRow::Profile { name, .. } => name.clone(),
                    ProfileMenuRow::Create => self.t("profile_menu.create").to_owned(),
                    ProfileMenuRow::Unavailable => self.t("profile_menu.unavailable").to_owned(),
                    ProfileMenuRow::Separator => String::new(),
                    ProfileMenuRow::GitHubSignIn => self.t("github.sign_in").to_owned(),
                    ProfileMenuRow::GitHubSignOut => self.t("github.sign_out").to_owned(),
                    ProfileMenuRow::GitHubAccount { login } => {
                        self.tf("github.signed_in_as", &[("login", login)])
                    }
                    ProfileMenuRow::SyncStatus { text } => text.clone(),
                    ProfileMenuRow::SyncTurnOn => self.t("sync_menu.turn_on").to_owned(),
                    ProfileMenuRow::SyncTurnOnRepository => {
                        self.t("sync_menu.turn_on_repository").to_owned()
                    }
                    ProfileMenuRow::SyncNow => self.t("sync_menu.sync_now").to_owned(),
                    ProfileMenuRow::SyncTurnOff => self.t("sync_menu.turn_off").to_owned(),
                };
                48.0 + label.chars().count() as f32 * char_width
            })
            .fold(MIN_WIDTH, f32::max)
            .min(window_width);
        let selected = rows
            .iter()
            .position(|row| matches!(row, ProfileMenuRow::Profile { active: true, .. }))
            .unwrap_or(0);
        let height = menu_height(&rows);
        let status_height = self
            .gpu
            .as_ref()
            .map_or(20.0, |gpu| gpu.renderer.line_height());
        let (x, y) = super::popup_placement::beside_button(
            button,
            (width, height),
            (window_width, window_height),
            status_height,
        );
        self.profile_menu = Some(ProfileMenu {
            x,
            y,
            width,
            selected,
            rows,
        });
    }

    pub(crate) fn profile_menu_contains(&self, x: f32, y: f32) -> bool {
        self.profile_menu.as_ref().is_some_and(|menu| {
            let (mx, my, width, height) = menu_rect(menu);
            x >= mx && x < mx + width && y >= my && y < my + height
        })
    }

    pub(crate) fn profile_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let Some(menu) = self.profile_menu.as_mut() else {
            return;
        };
        let mut top = menu.y + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            let height = row_height(row);
            if y >= top && y < top + height && !matches!(row, ProfileMenuRow::Separator) {
                if x >= menu.x && x < menu.x + menu.width {
                    menu.selected = index;
                }
                return;
            }
            top += height;
        }
    }

    pub(crate) fn profile_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some(menu) = self.profile_menu.clone() else {
            return false;
        };
        if !self.profile_menu_contains(x, y) {
            self.profile_menu = None;
            return true;
        }
        let mut top = menu.y + PADDING;
        for row in &menu.rows {
            let height = row_height(row);
            if y >= top && y < top + height {
                self.execute_profile_menu_row(row.clone());
                return true;
            }
            top += height;
        }
        true
    }

    pub(crate) fn profile_menu_key(&mut self, key: &winit::keyboard::Key) -> bool {
        use winit::keyboard::{Key, NamedKey};
        let Some(snapshot) = self.profile_menu.clone() else {
            return false;
        };
        let selectable = snapshot
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.is_selectable().then_some(index))
            .collect::<Vec<_>>();
        let position = selectable
            .iter()
            .position(|index| *index == snapshot.selected)
            .unwrap_or(0);
        match key {
            Key::Named(NamedKey::Escape | NamedKey::ArrowLeft) => self.profile_menu = None,
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(menu) = self.profile_menu.as_mut() {
                    if !selectable.is_empty() {
                        menu.selected = selectable[(position + 1) % selectable.len()];
                    }
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(menu) = self.profile_menu.as_mut() {
                    if !selectable.is_empty() {
                        menu.selected =
                            selectable[(position + selectable.len() - 1) % selectable.len()];
                    }
                }
            }
            Key::Named(NamedKey::Enter | NamedKey::Space) => {
                if let Some(row) = snapshot.rows.get(snapshot.selected) {
                    self.execute_profile_menu_row(row.clone());
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn activate_profile_menu_accessibility_item(&mut self, index: usize) {
        let Some(row) = self
            .profile_menu
            .as_ref()
            .and_then(|menu| menu.rows.get(index))
            .cloned()
        else {
            return;
        };
        self.execute_profile_menu_row(row);
    }

    fn execute_profile_menu_row(&mut self, row: ProfileMenuRow) {
        match row {
            ProfileMenuRow::Profile { name, .. } => {
                self.profile_menu = None;
                if let Err(error) = self.switch_user_profile(&name) {
                    self.status = error;
                }
            }
            ProfileMenuRow::Create => {
                self.profile_menu = None;
                if let Some(path) = rfd::FileDialog::new()
                    .set_directory(&self.profile_root)
                    .save_file()
                {
                    if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                        if let Err(error) = self.create_user_profile(name) {
                            self.status = error;
                        }
                    }
                }
            }
            ProfileMenuRow::GitHubSignIn => {
                self.profile_menu = None;
                self.start_github_sign_in();
            }
            ProfileMenuRow::GitHubSignOut => {
                self.profile_menu = None;
                self.sign_out_github();
            }
            ProfileMenuRow::SyncTurnOn => {
                self.profile_menu = None;
                self.turn_on_sync_with(SyncBackend::Gist);
            }
            ProfileMenuRow::SyncTurnOnRepository => {
                self.profile_menu = None;
                self.turn_on_sync_with(SyncBackend::Repository);
            }
            ProfileMenuRow::SyncNow => {
                self.profile_menu = None;
                if self.github_session.is_some() {
                    self.sync_now();
                } else {
                    self.start_sync_sign_in();
                }
            }
            ProfileMenuRow::SyncTurnOff => {
                self.profile_menu = None;
                self.turn_off_sync();
            }
            ProfileMenuRow::Separator
            | ProfileMenuRow::Unavailable
            | ProfileMenuRow::GitHubAccount { .. }
            | ProfileMenuRow::SyncStatus { .. } => {}
        }
    }

    pub(crate) fn draw_profile_menu(
        &self,
        renderer: &mut Renderer,
        _window_width: f32,
        _window_height: f32,
    ) {
        let Some(menu) = &self.profile_menu else {
            return;
        };
        let (x, y, width, height) = menu_rect(menu);
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, bg);
        let mut top = y + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            let row_h = row_height(row);
            match row {
                ProfileMenuRow::Separator => {
                    renderer.draw_rect(x + 1.0, top + 4.0, width - 2.0, 1.0, border);
                }
                ProfileMenuRow::Profile { name, active } => {
                    if menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    if *active {
                        renderer.draw_text("✓", x + 10.0, top + 5.0, text);
                    }
                    renderer.draw_text(name, x + 32.0, top + 5.0, text);
                }
                ProfileMenuRow::Create => {
                    if menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    renderer.draw_text(self.t("profile_menu.create"), x + 32.0, top + 5.0, text);
                }
                ProfileMenuRow::Unavailable => {
                    renderer.draw_text(
                        self.t("profile_menu.unavailable"),
                        x + 14.0,
                        top + 5.0,
                        dim,
                    );
                }
                ProfileMenuRow::GitHubSignIn | ProfileMenuRow::GitHubSignOut => {
                    if menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    let label = if matches!(row, ProfileMenuRow::GitHubSignIn) {
                        self.t("github.sign_in")
                    } else {
                        self.t("github.sign_out")
                    };
                    renderer.draw_text(label, x + 32.0, top + 5.0, text);
                }
                ProfileMenuRow::GitHubAccount { login } => {
                    renderer.draw_text(
                        &self.tf("github.signed_in_as", &[("login", login)]),
                        x + 14.0,
                        top + 5.0,
                        dim,
                    );
                }
                ProfileMenuRow::SyncStatus { text } => {
                    renderer.draw_text(text, x + 14.0, top + 5.0, dim);
                }
                ProfileMenuRow::SyncTurnOn
                | ProfileMenuRow::SyncTurnOnRepository
                | ProfileMenuRow::SyncNow
                | ProfileMenuRow::SyncTurnOff => {
                    if menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    let key = match row {
                        ProfileMenuRow::SyncTurnOn => "sync_menu.turn_on",
                        ProfileMenuRow::SyncTurnOnRepository => "sync_menu.turn_on_repository",
                        ProfileMenuRow::SyncNow => "sync_menu.sync_now",
                        _ => "sync_menu.turn_off",
                    };
                    renderer.draw_text(self.t(key), x + 32.0, top + 5.0, text);
                }
            }
            top += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_lists_local_profiles_marks_active_and_adds_creation_action() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let profiles = crate::app::phase5::Profiles::open(dir.path()).unwrap();
        profiles.create("Work").unwrap();
        app.profile_root = profiles.path(profiles.active()).unwrap();
        app.profiles = Some(profiles);

        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        let menu = app.profile_menu.as_ref().unwrap();
        assert!(menu.rows.contains(&ProfileMenuRow::Profile {
            name: "Default".into(),
            active: true,
        }));
        assert!(menu.rows.contains(&ProfileMenuRow::Profile {
            name: "Work".into(),
            active: false,
        }));
        assert!(menu.rows.contains(&ProfileMenuRow::Create));
    }

    #[test]
    fn keyboard_switches_to_selected_local_profile_and_escape_closes_menu() {
        use winit::keyboard::{Key, NamedKey};

        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let profiles = crate::app::phase5::Profiles::open(dir.path()).unwrap();
        profiles.create("Work").unwrap();
        app.profile_root = profiles.path(profiles.active()).unwrap();
        app.profiles = Some(profiles);

        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        app.profile_menu_key(&Key::Named(NamedKey::ArrowDown));
        app.profile_menu_key(&Key::Named(NamedKey::Enter));
        assert_eq!(app.profiles.as_ref().unwrap().active(), "Work");
        assert!(app.profile_menu.is_none());

        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        app.profile_menu_key(&Key::Named(NamedKey::Escape));
        assert!(app.profile_menu.is_none());
    }

    #[test]
    fn menu_offers_github_sign_in_then_the_account_and_sign_out() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let profiles = crate::app::phase5::Profiles::open(dir.path()).unwrap();
        app.profile_root = profiles.path(profiles.active()).unwrap();
        app.profiles = Some(profiles);

        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        let rows = app.profile_menu.as_ref().unwrap().rows.clone();
        assert!(rows.contains(&ProfileMenuRow::GitHubSignIn));
        assert!(!rows
            .iter()
            .any(|row| matches!(row, ProfileMenuRow::GitHubSignOut)));

        app.github_session = Some(graphite_auth::Session {
            token: "gho_secret".into(),
            login: "octocat".into(),
            ..Default::default()
        });
        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        let rows = app.profile_menu.as_ref().unwrap().rows.clone();
        assert!(rows.contains(&ProfileMenuRow::GitHubAccount {
            login: "octocat".into(),
        }));
        assert!(rows.contains(&ProfileMenuRow::GitHubSignOut));
        assert!(!rows.contains(&ProfileMenuRow::GitHubSignIn));

        // Choosing sign-out clears the account in memory.
        app.execute_profile_menu_row(ProfileMenuRow::GitHubSignOut);
        assert!(app.github_session.is_none());
    }

    #[test]
    fn pointer_switches_profiles_and_profiles_unavailable_is_inert() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let profiles = crate::app::phase5::Profiles::open(dir.path()).unwrap();
        profiles.create("Work").unwrap();
        app.profile_root = profiles.path(profiles.active()).unwrap();
        app.profiles = Some(profiles);

        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        let menu = app.profile_menu.as_ref().unwrap().clone();
        let work_row = menu
            .rows
            .iter()
            .position(|row| matches!(row, ProfileMenuRow::Profile { name, .. } if name == "Work"))
            .unwrap();
        let row_y =
            menu.y + PADDING + menu.rows.iter().take(work_row).map(row_height).sum::<f32>() + 3.0;
        app.profile_menu_click(menu.x + 12.0, row_y);
        assert_eq!(app.profiles.as_ref().unwrap().active(), "Work");
        assert!(app.profile_menu.is_none());

        let isolated = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(isolated.path().to_path_buf(), None, true);
        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        assert_eq!(
            app.profile_menu.as_ref().unwrap().rows,
            [ProfileMenuRow::Unavailable]
        );
        app.profile_menu_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Enter,
        ));
        assert!(app.profile_menu.is_some());
        app.profile_menu_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Escape,
        ));
        assert!(app.profile_menu.is_none());
    }

    fn signed_in_app(dir: &crate::test_support::TempDir, scope: &str) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let profiles = crate::app::phase5::Profiles::open(dir.path()).unwrap();
        app.profile_root = profiles.path(profiles.active()).unwrap();
        app.profiles = Some(profiles);
        app.github_session = Some(graphite_auth::Session {
            token: "gho_secret".into(),
            login: "octocat".into(),
            scope: scope.into(),
            ..Default::default()
        });
        app
    }

    fn menu_rows(app: &mut App) -> Vec<ProfileMenuRow> {
        app.open_profile_menu((0.0, 700.0, 48.0, 48.0), 900.0, 800.0);
        app.profile_menu.as_ref().unwrap().rows.clone()
    }

    #[test]
    fn sync_off_offers_only_to_turn_it_on() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user gist");
        let rows = menu_rows(&mut app);
        assert!(rows.contains(&ProfileMenuRow::SyncTurnOn));
        assert!(!rows.contains(&ProfileMenuRow::SyncNow));
        assert!(!rows.contains(&ProfileMenuRow::SyncTurnOff));
        assert!(!rows
            .iter()
            .any(|row| matches!(row, ProfileMenuRow::SyncStatus { .. })));
    }

    #[test]
    fn sync_on_shows_its_status_and_the_sync_and_turn_off_actions() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user gist");
        app.sync.persisted.enabled = true;
        let rows = menu_rows(&mut app);
        assert!(rows.contains(&ProfileMenuRow::SyncNow));
        assert!(rows.contains(&ProfileMenuRow::SyncTurnOff));
        assert!(!rows.contains(&ProfileMenuRow::SyncTurnOn));
        let status = rows
            .iter()
            .position(|row| matches!(row, ProfileMenuRow::SyncStatus { .. }))
            .expect("a status line");
        // The status line is information only: the keyboard never lands on it.
        assert!(!rows[status].is_selectable());
        assert!(rows[status + 1].is_selectable());
    }

    #[test]
    fn choosing_turn_off_disables_the_sync_and_closes_the_menu() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user gist");
        app.sync.persisted.enabled = true;
        menu_rows(&mut app);
        app.execute_profile_menu_row(ProfileMenuRow::SyncTurnOff);
        assert!(!app.sync.persisted.enabled);
        assert!(app.profile_menu.is_none());
    }

    #[test]
    fn choosing_turn_on_with_the_gist_permission_enables_the_sync() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user gist");
        menu_rows(&mut app);
        app.execute_profile_menu_row(ProfileMenuRow::SyncTurnOn);
        assert!(app.sync.persisted.enabled);
        assert!(app.profile_menu.is_none());
    }

    #[test]
    fn choosing_turn_on_without_the_gist_permission_asks_for_it_first() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user");
        menu_rows(&mut app);
        app.execute_profile_menu_row(ProfileMenuRow::SyncTurnOn);
        // Not enabled yet: it waits for the new sign-in to finish.
        assert!(!app.sync.persisted.enabled);
        assert!(app.sync.turning_on);
    }

    #[test]
    fn sync_off_offers_the_gist_and_the_private_repository_as_separate_options() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user");
        let rows = menu_rows(&mut app);
        assert!(rows.contains(&ProfileMenuRow::SyncTurnOn));
        assert!(rows.contains(&ProfileMenuRow::SyncTurnOnRepository));
    }

    #[test]
    fn choosing_the_repository_with_the_repo_permission_enables_that_backend() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user repo");
        menu_rows(&mut app);
        app.execute_profile_menu_row(ProfileMenuRow::SyncTurnOnRepository);
        assert!(app.sync.persisted.enabled);
        assert_eq!(app.sync.persisted.backend, SyncBackend::Repository);
        assert!(app.profile_menu.is_none());
    }

    #[test]
    fn choosing_the_repository_with_only_the_gist_permission_asks_for_repo_first() {
        let dir = crate::test_support::TempDir::new();
        let mut app = signed_in_app(&dir, "read:user gist");
        menu_rows(&mut app);
        app.execute_profile_menu_row(ProfileMenuRow::SyncTurnOnRepository);
        assert!(!app.sync.persisted.enabled);
        assert!(app.sync.turning_on);
        assert_eq!(app.sync.turning_on_backend, SyncBackend::Repository);
        // Nothing is switched until the new sign-in finishes.
        assert_eq!(app.sync.persisted.backend, SyncBackend::Gist);
    }
}
