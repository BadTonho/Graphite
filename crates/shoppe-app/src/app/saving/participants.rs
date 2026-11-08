use super::*;
use shoppe_core_api::{
    protocol::RpcRequest, CancellationToken, SaveParticipantRequest, SaveParticipantResult,
    ServiceError, ServiceResult,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub(super) enum Provider {
    Lsp { instance: u64 },
    Host { session: u64, resource: u64 },
}
enum Reply {
    Lsp(mpsc::Receiver<ServiceResult<Vec<shoppe_core_api::TextEdit>>>),
    Host(mpsc::Receiver<ServiceResult<serde_json::Value>>),
}
struct Waiting {
    provider: Provider,
    id: u64,
    invocation: u64,
    deadline: Instant,
    reply: Reply,
}
pub(super) struct Participants {
    remaining: VecDeque<Provider>,
    waiting: Option<Waiting>,
}
impl App {
    pub(super) fn save_participants(&self, document: DocumentId) -> Participants {
        let doc = &self.documents[document];
        let mut remaining = VecDeque::new();
        if let Some(instance) = doc
            .buffer
            .path()
            .and_then(|path| self.lsp_manager.server_instance_for(path))
        {
            remaining.push_back(Provider::Lsp { instance });
        }
        let language = super::super::services::snapshot_document(document, doc)
            .ok()
            .and_then(|d| d.language);
        for (resource, (session, metadata)) in &self.extension_host.participants {
            if metadata.languages.is_empty()
                || language
                    .as_ref()
                    .is_some_and(|lang| metadata.languages.contains(lang))
            {
                remaining.push_back(Provider::Host {
                    session: *session,
                    resource: *resource,
                });
            }
        }
        Participants {
            remaining,
            waiting: None,
        }
    }
    fn stop_participant(&mut self, waiting: &Waiting, path: &Path) {
        match waiting.provider {
            Provider::Lsp { instance } => {
                if self.lsp_manager.server_instance_for(path) == Some(instance) {
                    self.lsp_manager.cancel_request(path, waiting.id);
                }
            }
            Provider::Host { session, .. } => {
                self.extension_host.save_replies.remove(&waiting.id);
                if let Some(host) = self
                    .extension_host
                    .supervisor
                    .as_mut()
                    .filter(|h| h.session == session)
                {
                    host.cancel(waiting.id);
                }
            }
        }
    }
    fn refresh_save_base(&self, document: DocumentId, active: &mut Active) -> bool {
        let doc = &self.documents[document];
        if doc.buffer.revision() == active.snapshot.revision
            && doc.encoding == active.snapshot.encoding
            && doc.line_ending == active.snapshot.ending
        {
            return false;
        }
        active.snapshot.base = doc.buffer.text();
        active.snapshot.text = active.snapshot.base.clone();
        active.snapshot.revision = doc.buffer.revision();
        active.snapshot.encoding = doc.encoding;
        active.snapshot.ending = doc.line_ending;
        true
    }
    pub(super) fn poll_save_participants(&mut self, now: Instant) {
        let documents: Vec<_> = self.saving.active.keys().copied().collect();
        for document in documents {
            let mut active = self.saving.active.remove(&document).unwrap();
            if active.writing {
                self.saving.active.insert(document, active);
                continue;
            }
            if active.gate.cancelled() {
                if let Some(waiting) = active.participants.waiting.take() {
                    self.stop_participant(&waiting, &active.snapshot.path);
                }
                active.writing = true;
                let _ = self.saving.sender.send(writer::Written {
                    document,
                    result: Err("Save cancelled".into()),
                });
                self.saving.active.insert(document, active);
                continue;
            }
            if let Some(waiting) = active.participants.waiting.take() {
                let current = match waiting.provider {
                    Provider::Lsp { instance } => {
                        self.lsp_manager.server_instance_for(&active.snapshot.path)
                            == Some(instance)
                    }
                    Provider::Host { session, resource } => self
                        .extension_host
                        .participants
                        .get(&resource)
                        .is_some_and(|(s, _)| *s == session),
                };
                let result = if now >= waiting.deadline {
                    Some(Err(ServiceError::Timeout))
                } else if !current {
                    Some(Err(ServiceError::HostStopped))
                } else {
                    match &waiting.reply {
                        Reply::Lsp(receiver) => match receiver.try_recv() {
                            Ok(value) => Some(value),
                            Err(mpsc::TryRecvError::Disconnected) => {
                                Some(Err(ServiceError::HostStopped))
                            }
                            Err(_) => None,
                        },
                        Reply::Host(receiver) => match receiver.try_recv() {
                            Ok(Ok(value)) => Some(
                                serde_json::from_value::<SaveParticipantResult>(value)
                                    .map_err(|e| ServiceError::Protocol(e.to_string()))
                                    .and_then(|result| {
                                        if result.invocation != waiting.invocation
                                            || result.version != active.snapshot.revision
                                        {
                                            Err(ServiceError::StaleDocument)
                                        } else {
                                            Ok(result.edits)
                                        }
                                    }),
                            ),
                            Ok(Err(error)) => Some(Err(error)),
                            Err(mpsc::TryRecvError::Disconnected) => {
                                Some(Err(ServiceError::HostStopped))
                            }
                            Err(_) => None,
                        },
                    }
                };
                if let Some(result) = result {
                    self.stop_participant(&waiting, &active.snapshot.path);
                    if self.refresh_save_base(document, &mut active) {
                        active
                            .warnings
                            .push("Save participant edits discarded: document changed".into());
                    } else {
                        match result.and_then(|edits| apply_edits(&active.snapshot.text, edits)) {
                            Ok(text) => active.snapshot.text = text,
                            Err(error) => active
                                .warnings
                                .push(format!("Save participant skipped: {error}")),
                        }
                    }
                } else {
                    active.participants.waiting = Some(waiting);
                    self.saving.active.insert(document, active);
                    continue;
                }
            }
            self.refresh_save_base(document, &mut active);
            if let Some(provider) = active.participants.remaining.pop_front() {
                self.saving.invocation += 1;
                let invocation = self.saving.invocation;
                let deadline = now + Duration::from_secs(1);
                let request = match &provider {
                    Provider::Lsp { instance } => {
                        let (sender, receiver) = mpsc::channel();
                        // Separate namespace from UI request IDs; the local invocation owns cancellation.
                        let id = u64::MAX - invocation;
                        if self.lsp_manager.server_instance_for(&active.snapshot.path)
                            == Some(*instance)
                            && self.lsp_manager.before_save(
                                &active.snapshot.path,
                                id,
                                1,
                                deadline,
                                sender,
                            )
                        {
                            Ok(Waiting {
                                provider,
                                id,
                                invocation,
                                deadline,
                                reply: Reply::Lsp(receiver),
                            })
                        } else {
                            Err(ServiceError::HostStopped)
                        }
                    }
                    Provider::Host { session, resource } => {
                        let mut snapshot = super::super::services::snapshot_document(
                            document,
                            &self.documents[document],
                        )
                        .expect("local save URI");
                        snapshot.text = active.snapshot.text.clone();
                        let request = SaveParticipantRequest {
                            invocation,
                            reason: active.reason,
                            document: snapshot,
                        };
                        let live = self
                            .extension_host
                            .participants
                            .get(resource)
                            .is_some_and(|(owner, _)| owner == session);
                        let rpc = self
                            .extension_host
                            .supervisor
                            .as_mut()
                            .filter(|h| live && h.session == *session)
                            .ok_or(ServiceError::HostStopped)
                            .and_then(|h| {
                                h.request(
                                    RpcRequest::ParticipateInSave {
                                        resource: *resource,
                                        request,
                                    },
                                    CancellationToken::default(),
                                    now,
                                )
                            });
                        rpc.map(|id| {
                            let (sender, receiver) = mpsc::channel();
                            self.extension_host.save_replies.insert(id, sender);
                            Waiting {
                                provider,
                                id,
                                invocation,
                                deadline,
                                reply: Reply::Host(receiver),
                            }
                        })
                    }
                };
                match request {
                    Ok(waiting) => active.participants.waiting = Some(waiting),
                    Err(error) => active
                        .warnings
                        .push(format!("Save participant skipped: {error}")),
                }
            } else {
                if self.trim_trailing_whitespace {
                    let scratch = Buffer::from_text(&active.snapshot.path, &active.snapshot.text);
                    active.snapshot.text = scratch.text_with_trimmed_trailing_whitespace();
                }
                active.writing = true;
                if self.documents[document].buffer.is_read_only() {
                    active.gate.cancel();
                }
                writer::write(
                    &active.snapshot,
                    document,
                    self.saving.sender.clone(),
                    active.gate.clone(),
                );
            }
            self.saving.active.insert(document, active);
        }
    }
}
fn apply_edits(text: &str, edits: Vec<shoppe_core_api::TextEdit>) -> ServiceResult<String> {
    let mut replacements = Vec::new();
    for edit in edits {
        let start = shoppe_core_api::position_to_char(text, edit.start)
            .ok_or(ServiceError::InvalidRange)?;
        let end =
            shoppe_core_api::position_to_char(text, edit.end).ok_or(ServiceError::InvalidRange)?;
        replacements.push(shoppe_text::TextReplacement {
            range: start..end,
            text: edit.text.replace("\r\n", "\n").replace('\r', "\n"),
        });
    }
    let mut scratch = Buffer::from_text("save-participant", text);
    scratch
        .apply_transaction(&replacements)
        .map_err(|_| ServiceError::InvalidRange)?;
    Ok(scratch.text())
}
/// Keep unchanged spans stable when several participants edit different lines.
pub(super) fn text_diff(before: &str, after: &str) -> Vec<shoppe_text::TextReplacement> {
    let diff = similar::TextDiff::from_chars(before, after);
    let mut edits = Vec::new();
    let mut offset = 0;
    let mut pending: Option<shoppe_text::TextReplacement> = None;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Equal => {
                if let Some(edit) = pending.take() {
                    edits.push(edit);
                }
                offset += change.value().chars().count();
            }
            similar::ChangeTag::Delete => {
                let edit = pending.get_or_insert_with(|| shoppe_text::TextReplacement {
                    range: offset..offset,
                    text: String::new(),
                });
                offset += change.value().chars().count();
                edit.range.end = offset;
            }
            similar::ChangeTag::Insert => {
                pending
                    .get_or_insert_with(|| shoppe_text::TextReplacement {
                        range: offset..offset,
                        text: String::new(),
                    })
                    .text
                    .push_str(change.value());
            }
        }
    }
    if let Some(edit) = pending {
        edits.push(edit);
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edit(start: u32, end: u32) -> shoppe_core_api::TextEdit {
        shoppe_core_api::TextEdit {
            start: shoppe_core_api::Position {
                line: 0,
                character: start,
            },
            end: shoppe_core_api::Position {
                line: 0,
                character: end,
            },
            text: "x".into(),
        }
    }
    #[test]
    fn participant_utf16_edits_reject_surrogates_invalid_and_overlapping_ranges_atomically() {
        let text = "a🌎bc";
        assert!(matches!(
            apply_edits(text, vec![edit(2, 3)]),
            Err(ServiceError::InvalidRange)
        ));
        assert!(matches!(
            apply_edits(text, vec![edit(0, 3), edit(1, 4)]),
            Err(ServiceError::InvalidRange)
        ));
        assert!(matches!(
            apply_edits(text, vec![edit(4, 3)]),
            Err(ServiceError::InvalidRange)
        ));
        assert!(matches!(
            apply_edits(text, vec![edit(0, 30)]),
            Err(ServiceError::InvalidRange)
        ));
        assert_eq!(apply_edits(text, vec![edit(1, 3)]).unwrap(), "axbc");
        assert_eq!(text, "a🌎bc");
    }
}
