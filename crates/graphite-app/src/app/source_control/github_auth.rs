//! GitHub sign-in from the profile menu. The device flow runs on a background thread; the
//! user's code and the final result come back to the event loop as `UserEvent::GitHubAuth`.

use super::*;
use graphite_auth::{DeviceCode, Session};

/// Progress reported by the sign-in thread.
#[derive(Debug)]
pub(crate) enum GitHubAuthEvent {
    /// Show this code to the user; the verification page is already open.
    Code {
        user_code: String,
        verification_uri: String,
    },
    Finished(Result<Session, String>),
}

/// Public client ID of Graphite's GitHub OAuth App (Device Flow enabled). It is not a secret:
/// the device flow never uses one, so it ships in every build.
pub(crate) const DEFAULT_CLIENT_ID: &str = "Ov23lieI1U1RTbC3aaHz";

/// OAuth App client ID of this build. `GRAPHITE_GITHUB_CLIENT_ID` at compile time overrides the
/// default, for builds that use another OAuth App.
pub(crate) fn client_id() -> Option<&'static str> {
    option_env!("GRAPHITE_GITHUB_CLIENT_ID")
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .or(Some(DEFAULT_CLIENT_ID))
}

impl App {
    /// Starts the device flow for this profile using the default `read:user` scope.
    pub(crate) fn start_github_sign_in(&mut self) {
        self.start_github_sign_in_with_scope(graphite_auth::SCOPE);
    }

    /// Starts the device flow for this profile with an explicit OAuth `scope`.
    pub(crate) fn start_github_sign_in_with_scope(&mut self, scope: &str) {
        if self.github_sign_in_pending {
            self.status = self.t("github.sign_in.pending").into();
            return;
        }
        let Some(client) = client_id() else {
            self.status = self.t("github.sign_in.unconfigured").into();
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = self.t("github.sign_in.unconfigured").into();
            return;
        };
        self.github_sign_in_pending = true;
        self.status = self.t("github.sign_in.starting").into();
        let profile = self.profile_root.clone();
        let persist = self.profiles.is_some();
        let client = client.to_owned();
        let scope = scope.to_owned();
        std::thread::spawn(move || {
            let outcome = graphite_auth::ReqwestTransport::new().and_then(|transport| {
                graphite_auth::sign_in_with_scope(
                    &transport,
                    &client,
                    &scope,
                    |code: &DeviceCode| {
                        let _ = proxy.send_event(UserEvent::GitHubAuth(GitHubAuthEvent::Code {
                            user_code: code.user_code.clone(),
                            verification_uri: code.verification_uri.clone(),
                        }));
                        crate::app::plugins::drawing::open_external_https(&code.verification_uri);
                    },
                    std::thread::sleep,
                )
            });
            if let (Ok(session), true) = (&outcome, persist) {
                // The account still works for this run when it cannot be kept.
                if let Err(error) = graphite_auth::save_session(
                    &profile,
                    session,
                    &crate::app::credentials::GitHubTokenVault,
                ) {
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::Medium,
                        "github.auth",
                        format!("Could not keep the GitHub sign-in: {error}"),
                    );
                }
            }
            let _ = proxy.send_event(UserEvent::GitHubAuth(GitHubAuthEvent::Finished(outcome)));
        });
    }

    pub(crate) fn handle_github_auth_event(&mut self, event: GitHubAuthEvent) {
        match event {
            GitHubAuthEvent::Code {
                user_code,
                verification_uri,
            } => {
                self.github_code_dialog = Some(super::github_dialog::GitHubCodeDialog {
                    user_code: user_code.clone(),
                    verification_uri: verification_uri.clone(),
                });
                self.status = self.tf(
                    "github.sign_in.code",
                    &[("code", &user_code), ("uri", &verification_uri)],
                );
            }
            GitHubAuthEvent::Finished(Ok(session)) => {
                self.github_code_dialog = None;
                self.github_sign_in_pending = false;
                self.status = self.tf("github.sign_in.done", &[("login", &session.login)]);
                self.github_session = Some(session);
                self.finish_github_auth_requests();
                self.finish_turning_on_sync();
            }
            GitHubAuthEvent::Finished(Err(error)) => {
                self.github_code_dialog = None;
                self.github_sign_in_pending = false;
                self.status = self.tf("github.sign_in.failed", &[("error", &error)]);
                self.finish_github_auth_requests();
            }
        }
    }

    /// Forgets the GitHub account of this profile on this computer.
    pub(crate) fn sign_out_github(&mut self) {
        let forgotten = if self.profiles.is_some() {
            graphite_auth::delete_session(
                &self.profile_root,
                &crate::app::credentials::GitHubTokenVault,
            )
        } else {
            // An isolated profile never saved the account.
            Ok(())
        };
        match forgotten {
            Ok(()) => {
                self.github_session = None;
                self.status = self.t("github.sign_out.done").into();
                self.notify_github_signed_out();
            }
            Err(error) => {
                self.status = self.tf("github.sign_out.failed", &[("error", &error)]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            token: "gho_secret".into(),
            login: "octocat".into(),
            ..Default::default()
        }
    }

    #[test]
    fn finished_sign_in_sets_the_account_and_failure_keeps_it_signed_out() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.github_sign_in_pending = true;
        app.handle_github_auth_event(GitHubAuthEvent::Finished(Ok(session())));
        assert!(!app.github_sign_in_pending);
        assert_eq!(app.github_session.as_ref().unwrap().login, "octocat");
        assert!(app.status.contains("octocat"));

        app.github_sign_in_pending = true;
        app.handle_github_auth_event(GitHubAuthEvent::Finished(Err("refused".into())));
        assert!(!app.github_sign_in_pending);
        assert!(app.status.contains("refused"));
    }

    #[test]
    fn sign_out_forgets_the_saved_account_for_this_profile() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        app.github_session = Some(session());
        app.sign_out_github();
        assert!(app.github_session.is_none());
        assert!(app.status.contains(app.t("github.sign_out.done")));
        // An isolated profile never touches the saved account files.
        assert!(!app.profile_root.join(graphite_auth::SESSION_FILE).exists());
    }

    #[test]
    fn sign_in_without_a_client_id_says_so_and_starts_nothing() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.start_github_sign_in();
        assert!(!app.github_sign_in_pending);
        // Tests have no event loop, so sign-in reports "unavailable" whatever the build has.
        assert!(app.status.contains(app.t("github.sign_in.unconfigured")));
    }
}

#[cfg(test)]
mod client_id_tests {
    use super::*;

    #[test]
    fn builds_ship_with_graphites_oauth_app_unless_overridden() {
        assert_eq!(DEFAULT_CLIENT_ID, "Ov23lieI1U1RTbC3aaHz");
        assert!(client_id().is_some());
    }
}
