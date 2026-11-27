//! Proposal review and guarded commits of the exact revision the user approved.

use super::*;
use crate::app::error_log::{self, ErrorSeverity};
use crate::file_format::{self, DecodedFile};
use graphite_chat::{FileActivityStatus, FileProposal, ProposedEdit, ReviewedFile};

const LOG_SOURCE: &str = "chat.review";

fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn resolve_target(workspace: &Path, path: &Path) -> Result<PathBuf, String> {
    let relative = path
        .strip_prefix(workspace)
        .map_err(|_| "Wrong workspace")?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| part.as_os_str().eq_ignore_ascii_case(".git"))
    {
        return Err("Invalid proposal target".into());
    }
    let name = relative.to_str().ok_or("File name is not Unicode")?;
    let resolved = graphite_chat::resolve_workspace_path(workspace, name)?;
    if resolved != path {
        return Err("Proposal target changed".into());
    }
    Ok(resolved)
}

impl App {
    pub(crate) fn stage_chat_proposal(
        &mut self,
        session_id: &str,
        turn_id: u64,
        edit: ProposedEdit,
    ) -> Result<String, String> {
        let proposal = self
            .prepare_chat_proposal(turn_id, edit)
            .inspect_err(|message| {
                error_log::record(ErrorSeverity::Medium, LOG_SOURCE, message);
            })?;
        let session = self
            .chat
            .session_by_id_mut(session_id)
            .ok_or("Chat session ended")?;
        if session.turn_mut(turn_id).is_none() {
            return Err("Chat turn ended".into());
        }
        session.queue_proposal(proposal);
        self.chat.persist_session(
            &self.profile_root,
            self.profiles.is_none(),
            self.chat
                .sessions
                .iter()
                .find(|session| session.id == session_id)
                .unwrap(),
        );
        Ok("File proposal queued for user review. The file has not changed on disk.".into())
    }

    fn prepare_chat_proposal(
        &self,
        turn_id: u64,
        edit: ProposedEdit,
    ) -> Result<FileProposal, String> {
        let workspace = self
            .tree
            .root()
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let raw = edit.path.to_str().ok_or("File name is not Unicode")?;
        let path = graphite_chat::resolve_workspace_path(&workspace, raw)?;
        let path = resolve_target(&workspace, &path)?;
        let decoded = edit
            .before
            .as_deref()
            .map(|bytes| file_format::decode(bytes, &self.translator))
            .transpose()?;
        let document = self
            .documents
            .id_for_path(&path)
            .map(|id| (id.0, self.documents[id].buffer.revision()));
        Ok(FileProposal {
            id: 0,
            turn_id,
            workspace,
            path,
            before: decoded.as_ref().map(|file| file.text.clone()),
            fingerprint: decoded.and_then(|file| file.fingerprint),
            after: edit.after.replace("\r\n", "\n").replace('\r', "\n"),
            document,
            issue: None,
        })
    }

    pub(crate) fn persist_chat_review(&self) {
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
    }

    pub(crate) fn resolve_chat_review_source(
        &self,
        proposal: &FileProposal,
    ) -> Result<PathBuf, String> {
        if self.saving.transition_pending()
            || self.tree.root().canonicalize().ok().as_deref() != Some(proposal.workspace.as_path())
        {
            return Err(self.t("chat.review.wrong_workspace").into());
        }
        resolve_target(&proposal.workspace, &proposal.path)
            .map_err(|_| self.t("workbench.review.source_unavailable").into())
    }

    fn verify_chat_target(&self, workspace: &Path, path: &Path) -> Result<(), String> {
        if self.saving.transition_pending()
            || self.tree.root().canonicalize().ok().as_deref() != Some(workspace)
        {
            return Err(self.t("chat.review.wrong_workspace").into());
        }
        let target = resolve_target(workspace, path)?;
        let relative = target
            .strip_prefix(workspace)
            .unwrap_or(&target)
            .to_string_lossy()
            .replace('\\', "/");
        if self.read_only_patterns.iter().any(|pattern| {
            globset::Glob::new(pattern).is_ok_and(|glob| glob.compile_matcher().is_match(&relative))
        }) {
            return Err(self.t("chat.review.editor_busy").into());
        }
        if let Some(id) = self.documents.id_for_path(path) {
            let document = &self.documents[id];
            if document.is_dirty()
                || self.saving.document_pending(id)
                || document.buffer.is_read_only()
            {
                return Err(self.t("chat.review.editor_busy").into());
            }
        }
        Ok(())
    }

    fn check_chat_bytes(
        &self,
        workspace: &Path,
        path: &Path,
        expected: Option<[u8; 32]>,
    ) -> Result<Option<Vec<u8>>, String> {
        self.verify_chat_target(workspace, path)?;
        let current = read_bytes(path).map_err(|error| self.chat_write_error(error))?;
        if current.as_deref().map(file_format::fingerprint) != expected {
            return Err(self.t("chat.review.conflict").into());
        }
        if let Some(id) = self.documents.id_for_path(path) {
            if self.documents[id].saved_fingerprint != expected {
                return Err(self.t("chat.review.conflict").into());
            }
        }
        Ok(current)
    }

    fn write_reviewed_bytes(
        &mut self,
        workspace: &Path,
        path: &Path,
        expected: Option<[u8; 32]>,
        bytes: Option<&[u8]>,
    ) -> Result<(), String> {
        self.check_chat_bytes(workspace, path, expected)?;
        if let Some(bytes) = bytes {
            let decoded = file_format::decode(bytes, &self.translator)?;
            if expected.is_none() {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|error| self.chat_write_error(error))?;
                }
            }
            let mut conflict = None;
            let gate = || match self.check_chat_bytes(workspace, path, expected) {
                Ok(_) => true,
                Err(error) => {
                    conflict = Some(error);
                    false
                }
            };
            let result = if expected.is_none() {
                Buffer::write_new_bytes_with_commit(path, bytes, gate)
            } else {
                Buffer::write_bytes_with_commit(path, bytes, gate)
            };
            result.map_err(|error| conflict.unwrap_or_else(|| self.chat_write_error(error)))?;
            self.sync_reviewed_document(path, Some(decoded));
        } else {
            if expected.is_some() {
                std::fs::remove_file(path).map_err(|error| self.chat_write_error(error))?;
            }
            self.sync_reviewed_document(path, None);
        }
        Ok(())
    }

    fn chat_write_error(&self, error: impl std::fmt::Display) -> String {
        error_log::record(ErrorSeverity::High, LOG_SOURCE, error.to_string());
        self.t("chat.review.write_failed").into()
    }

    fn sync_reviewed_document(&mut self, path: &Path, file: Option<DecodedFile>) {
        let Some(id) = self.documents.id_for_path(path) else {
            return;
        };
        let text = file.as_ref().map_or("", |decoded| decoded.text.as_str());
        self.sync_open_document_text(path, Some(text));
        let document = &mut self.documents[id];
        document.saved_fingerprint = file.as_ref().and_then(|decoded| decoded.fingerprint);
        if let Some(file) = file {
            document.encoding = file.encoding;
            document.line_ending = file.line_ending;
            document.saved_format = (file.encoding, file.line_ending);
        }
        self.invalidate_lsp_requests();
        self.sync_lsp_active();
    }

    fn commit_chat_proposal(&mut self, proposal: &FileProposal) -> Result<ReviewedFile, String> {
        let before =
            self.check_chat_bytes(&proposal.workspace, &proposal.path, proposal.fingerprint)?;
        if let Some((view_id, revision)) = proposal.document {
            if let Some(current) = self.documents.id_for_path(&proposal.path) {
                if current.0 == view_id && self.documents[current].buffer.revision() != revision {
                    return Err(self.t("chat.review.conflict").into());
                }
            }
        }
        let decoded = before
            .as_deref()
            .map(|data| file_format::decode(data, &self.translator))
            .transpose()?;
        let after = if decoded
            .as_ref()
            .is_some_and(|file| file.text == proposal.after)
        {
            before.clone().unwrap()
        } else {
            let (encoding, ending) = decoded
                .map(|file| (file.encoding, file.line_ending))
                .unwrap_or_default();
            file_format::encode(&proposal.after, encoding, ending)
        };
        if before.as_ref() != Some(&after) {
            self.write_reviewed_bytes(
                &proposal.workspace,
                &proposal.path,
                proposal.fingerprint,
                Some(&after),
            )?;
        }
        Ok(ReviewedFile {
            workspace: proposal.workspace.clone(),
            before,
            after,
        })
    }

    pub(crate) fn apply_chat_proposal(&mut self, id: u64) -> bool {
        let Some(proposal) = self
            .chat
            .active_session()
            .proposals
            .iter()
            .find(|proposal| proposal.id == id)
            .cloned()
        else {
            self.status = self.t("chat.review.outdated").into();
            return false;
        };
        match self.commit_chat_proposal(&proposal) {
            Ok(reviewed) => {
                self.chat
                    .active_session_mut()
                    .record_reviewed_file(&proposal, reviewed);
                self.persist_chat_review();
                self.refresh_scm_async();
                self.status = self.t("chat.review.applied").into();
                true
            }
            Err(issue) => {
                self.chat.active_session_mut().record_file_status(
                    proposal.turn_id,
                    &proposal.path,
                    FileActivityStatus::Conflict,
                );
                error_log::record(ErrorSeverity::Low, LOG_SOURCE, &issue);
                if let Some(entry) = self
                    .chat
                    .active_session_mut()
                    .proposals
                    .iter_mut()
                    .find(|entry| entry.id == id)
                {
                    entry.issue = Some(issue.clone());
                }
                self.persist_chat_review();
                self.status = issue;
                false
            }
        }
    }

    pub(crate) fn apply_all_chat_proposals(&mut self) -> usize {
        let files: Vec<_> = self
            .chat
            .active_session()
            .proposals
            .iter()
            .map(|entry| {
                let label = entry
                    .path
                    .strip_prefix(&entry.workspace)
                    .unwrap_or(&entry.path);
                (entry.id, label.to_path_buf())
            })
            .collect();
        self.chat.reviewed_in_batch.clear();
        for (id, path) in &files {
            if self.apply_chat_proposal(*id) {
                self.chat.reviewed_in_batch.push(path.clone());
            }
        }
        self.chat.review_list = Some(0);
        let applied = self.chat.reviewed_in_batch.len();
        self.status = self.tf(
            "chat.review.batch",
            &[("applied", &applied), ("pending", &(files.len() - applied))],
        );
        applied
    }

    pub(crate) fn discard_chat_proposal(&mut self, id: u64) -> bool {
        if !self.chat.active_session_mut().discard_proposal(id) {
            self.status = self.t("chat.review.outdated").into();
            return false;
        }
        self.persist_chat_review();
        self.status = self.t("chat.review.discarded").into();
        true
    }

    pub(super) fn restore_legacy_chat_file(
        &mut self,
        path: &Path,
        before: Option<&str>,
        after: Option<&str>,
    ) -> bool {
        let result = (|| {
            let root = self
                .tree
                .root()
                .canonicalize()
                .map_err(|error| error.to_string())?;
            let raw = path.to_str().ok_or("File name is not Unicode")?;
            let target = graphite_chat::resolve_workspace_path(&root, raw)?;
            self.write_reviewed_bytes(
                &root,
                &target,
                before.map(|text| file_format::fingerprint(text.as_bytes())),
                after.map(str::as_bytes),
            )
        })();
        self.report_review_restore(result)
    }

    fn report_review_restore(&mut self, result: Result<(), String>) -> bool {
        if let Err(issue) = result {
            error_log::record(ErrorSeverity::Low, LOG_SOURCE, &issue);
            self.status = issue;
            false
        } else {
            true
        }
    }

    pub(super) fn restore_chat_checkpoint(&mut self, undo: bool) -> bool {
        let session = self.chat.active_session();
        let checkpoint = if undo {
            session.checkpoints.last()
        } else {
            session.undone_checkpoints.last()
        };
        let Some(checkpoint) = checkpoint else {
            return false;
        };
        let [entry] = checkpoint.entries.as_slice() else {
            self.status = self.t("chat.review.outdated").into();
            return false;
        };
        let origin = checkpoint.turn_id;
        let entry = entry.clone();
        let restored = if let Some(reviewed) = entry.reviewed {
            let (expected, replacement) = if undo {
                (Some(reviewed.after.as_slice()), reviewed.before.as_deref())
            } else {
                (reviewed.before.as_deref(), Some(reviewed.after.as_slice()))
            };
            let result = self.write_reviewed_bytes(
                &reviewed.workspace,
                &entry.path,
                expected.map(file_format::fingerprint),
                replacement,
            );
            self.report_review_restore(result)
        } else {
            let (before, after) = if undo {
                (entry.after.as_deref(), entry.before.as_deref())
            } else {
                (entry.before.as_deref(), entry.after.as_deref())
            };
            self.restore_legacy_chat_file(&entry.path, before, after)
        };
        if !restored {
            return false;
        }
        let session = self.chat.active_session_mut();
        session.record_file_status(
            origin,
            &entry.path,
            if undo {
                FileActivityStatus::Reverted
            } else {
                FileActivityStatus::Applied
            },
        );
        if undo {
            session.undo_checkpoint();
        } else {
            session.redo_checkpoint();
        }
        self.persist_chat_review();
        self.refresh_scm_async();
        self.status = self
            .t(if undo {
                "chat.review.undone"
            } else {
                "chat.review.redone"
            })
            .into();
        true
    }
}
