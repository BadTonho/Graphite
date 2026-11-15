//! Turning the commit message field and the chosen kind of commit into a git commit.

use super::*;
use crate::scm_commit::{CommitOptions, CommitRequest, CommitScope};

impl App {
    /// Decides what a commit of kind `options` would do with the current changes, or why it cannot.
    pub(crate) fn build_commit_request(
        &self,
        options: CommitOptions,
    ) -> Result<CommitRequest, String> {
        let message = self.scm_commit_message.trim().to_owned();
        if message.is_empty() && !options.amend {
            return Err(self.t("scm.error.empty_message").into());
        }
        let files = &self.scm_snapshot.files;
        let any_staged = files.iter().any(|file| file.staged);
        if !options.amend {
            if options.scope == CommitScope::Staged && !any_staged {
                return Err(self.t("scm.error.nothing_staged").into());
            }
            if files.is_empty() {
                return Err(self.t("scm.error.nothing_to_commit").into());
            }
        }
        let stage_all = match options.scope {
            CommitScope::All => true,
            CommitScope::Staged => false,
            CommitScope::Smart => !any_staged,
        };
        Ok(CommitRequest {
            message,
            stage_all,
            amend: options.amend,
            signoff: options.signoff,
            post: options.post,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scm::FileStatus;
    use crate::scm_commit::PostCommit;
    use crate::test_support::TempDir;

    fn app(dir: &TempDir, staged: bool, unstaged: bool, message: &str) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm_commit_message = message.into();
        if staged {
            app.scm_snapshot.files.push(FileStatus {
                path: "s.txt".into(),
                staged: true,
                ..Default::default()
            });
        }
        if unstaged {
            app.scm_snapshot.files.push(FileStatus {
                path: "u.txt".into(),
                unstaged: true,
                ..Default::default()
            });
        }
        app
    }

    #[test]
    fn a_plain_commit_takes_the_staged_files_or_everything_when_none_is_staged() {
        let dir = TempDir::new();
        let staged = app(&dir, true, true, " message ");
        let request = staged.build_commit_request(CommitOptions::PLAIN).unwrap();
        assert_eq!(request.message, "message");
        assert!(!request.stage_all);

        let nothing_staged = app(&dir, false, true, "message");
        assert!(nothing_staged
            .build_commit_request(CommitOptions::PLAIN)
            .unwrap()
            .stage_all);
    }

    #[test]
    fn scopes_override_the_automatic_choice() {
        let dir = TempDir::new();
        let app = app(&dir, true, true, "m");
        let all = CommitOptions {
            scope: CommitScope::All,
            ..CommitOptions::PLAIN
        };
        assert!(app.build_commit_request(all).unwrap().stage_all);
        let staged = CommitOptions {
            scope: CommitScope::Staged,
            ..CommitOptions::PLAIN
        };
        assert!(!app.build_commit_request(staged).unwrap().stage_all);

        let unstaged_only = self::app(&dir, false, true, "m");
        assert_eq!(
            unstaged_only.build_commit_request(staged).unwrap_err(),
            unstaged_only.t("scm.error.nothing_staged")
        );
    }

    #[test]
    fn a_message_is_required_except_when_amending_and_changes_except_when_amending() {
        let dir = TempDir::new();
        let blank = app(&dir, true, false, "   ");
        assert_eq!(
            blank.build_commit_request(CommitOptions::PLAIN).unwrap_err(),
            blank.t("scm.error.empty_message")
        );
        let amend = CommitOptions {
            amend: true,
            signoff: true,
            post: PostCommit::Push,
            ..CommitOptions::PLAIN
        };
        let request = blank.build_commit_request(amend).unwrap();
        assert!(request.amend && request.signoff);
        assert_eq!(request.post, PostCommit::Push);

        let clean = app(&dir, false, false, "m");
        assert_eq!(
            clean.build_commit_request(CommitOptions::PLAIN).unwrap_err(),
            clean.t("scm.error.nothing_to_commit")
        );
        assert!(clean.build_commit_request(amend).is_ok(), "amending needs no new changes");
    }

    #[test]
    fn a_refused_commit_leaves_the_message_and_reports_why() {
        let dir = TempDir::new();
        let mut app = app(&dir, false, false, "keep me");
        app.status.clear();
        app.perform_scm_action(super::scm_ui::ScmAction::Commit(CommitOptions::PLAIN));
        assert_eq!(app.scm_commit_message, "keep me");
        assert!(!app.status.is_empty());
    }
}
