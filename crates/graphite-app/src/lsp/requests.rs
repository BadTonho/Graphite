use super::*;
/// Resolves outstanding UI requests on every worker exit, including shutdown.
pub(super) struct PendingClientRequests {
    entries: HashMap<i32, (u64, &'static str)>,
    deadlines: HashMap<i32, std::time::Instant>,
    events: Sender<LspUiEvent>,
}
impl PendingClientRequests {
    pub(super) fn new(events: Sender<LspUiEvent>) -> Self {
        Self {
            events,
            entries: HashMap::new(),
            deadlines: HashMap::new(),
        }
    }
    pub(super) fn insert(&mut self, id: i32, value: (u64, &'static str)) {
        self.insert_at(id, value, std::time::Instant::now());
    }
    fn insert_at(&mut self, id: i32, value: (u64, &'static str), now: std::time::Instant) {
        self.entries.insert(id, value);
        self.deadlines.insert(id, now + Duration::from_secs(30));
    }
    pub(super) fn expire(&mut self, now: std::time::Instant) -> Vec<Message> {
        self.deadlines.retain(|id, _| self.entries.contains_key(id));
        let expired: Vec<_> = self
            .deadlines
            .iter()
            .filter(|(_, deadline)| now >= **deadline)
            .map(|(id, _)| *id)
            .collect();
        let mut messages = Vec::new();
        for id in expired {
            self.deadlines.remove(&id);
            if let Some((request_id, _)) = self.entries.remove(&id) {
                let _ = self.events.send(LspUiEvent::RequestRejected {
                    request_id,
                    error: graphite_core_api::ServiceError::Timeout,
                });
                messages.push(Message::Notification(Notification {
                    method: "$/cancelRequest".into(),
                    params: json!({"id":id}),
                }));
            }
        }
        messages
    }
}
impl std::ops::Deref for PendingClientRequests {
    type Target = HashMap<i32, (u64, &'static str)>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
impl std::ops::DerefMut for PendingClientRequests {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}
impl Drop for PendingClientRequests {
    fn drop(&mut self) {
        for (_, (request_id, _)) in self.entries.drain() {
            if request_id != 0 {
                let _ = self.events.send(LspUiEvent::RequestFailed {
                    request_id,
                    message: "Language server stopped".into(),
                });
            }
        }
    }
}

struct PendingServerRequest {
    receiver: Receiver<lsp_server::Response>,
    deadline: std::time::Instant,
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Default)]
pub(super) struct ServerRequests {
    pending: HashMap<RequestId, PendingServerRequest>,
}

impl Drop for PendingServerRequest {
    fn drop(&mut self) {
        self.active
            .store(false, std::sync::atomic::Ordering::Release);
    }
}

impl ServerRequests {
    pub(super) fn handle(
        &mut self,
        request: Request,
        event_tx: &Sender<LspUiEvent>,
    ) -> Option<lsp_server::Response> {
        if matches!(
            request.method.as_str(),
            "workspace/configuration" | "workspace/workspaceFolders" | "window/showMessageRequest"
        ) {
            return self.service_request(request, event_tx);
        }
        if request.method != "workspace/applyEdit" {
            return Some(lsp_server::Response::new_err(
                request.id,
                -32601,
                "Unsupported method".into(),
            ));
        }
        if self.pending.len() >= 64 || self.pending.contains_key(&request.id) {
            return Some(lsp_server::Response::new_err(
                request.id,
                -32600,
                "Too many or duplicate requests".into(),
            ));
        }
        let params: lsp_types::ApplyWorkspaceEditParams =
            match serde_json::from_value(request.params) {
                Ok(params) => params,
                Err(error) => {
                    return Some(lsp_server::Response::new_err(
                        request.id,
                        -32602,
                        error.to_string(),
                    ))
                }
            };
        let (reply, receiver) = mpsc::channel();
        let id = request.id;
        let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        if event_tx
            .send(LspUiEvent::ApplyEdit {
                id: id.clone(),
                edit: params.edit,
                reply,
                active: active.clone(),
            })
            .is_err()
        {
            return Some(lsp_server::Response::new_err(
                id,
                -32603,
                "Editor unavailable".into(),
            ));
        }
        self.pending.insert(
            id,
            PendingServerRequest {
                receiver,
                deadline: std::time::Instant::now() + Duration::from_secs(30),
                active,
            },
        );
        None
    }

    fn service_request(
        &mut self,
        request: Request,
        event_tx: &Sender<LspUiEvent>,
    ) -> Option<lsp_server::Response> {
        if self.pending.len() >= 64 || self.pending.contains_key(&request.id) {
            return Some(lsp_server::Response::new_err(
                request.id,
                -32600,
                "Too many or duplicate requests".into(),
            ));
        }
        let valid = match request.method.as_str() {
            "workspace/configuration" => request
                .params
                .get("items")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().all(|item| {
                        item.is_object()
                            && item
                                .get("section")
                                .is_none_or(|v| v.is_null() || v.is_string())
                            && item
                                .get("scopeUri")
                                .is_none_or(|v| v.is_null() || v.is_string())
                    })
                }),
            "workspace/workspaceFolders" => request.params.is_null() || request.params.is_object(),
            "window/showMessageRequest" => {
                request.params.get("message").is_some_and(Value::is_string)
            }
            _ => false,
        };
        if !valid {
            return Some(lsp_server::Response::new_err(
                request.id,
                -32602,
                "Invalid service request arguments".into(),
            ));
        }
        let (reply, receiver) = mpsc::channel();
        let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let id = request.id;
        if event_tx
            .send(LspUiEvent::ServiceRequest {
                id: id.clone(),
                method: request.method,
                params: request.params,
                reply,
                active: active.clone(),
            })
            .is_err()
        {
            return Some(lsp_server::Response::new_err(
                id,
                -32603,
                "Editor unavailable".into(),
            ));
        }
        self.pending.insert(
            id,
            PendingServerRequest {
                receiver,
                deadline: std::time::Instant::now() + Duration::from_secs(30),
                active,
            },
        );
        None
    }
    pub(super) fn cancel(&mut self, id: &RequestId) -> Option<lsp_server::Response> {
        if !self
            .pending
            .get(id)?
            .active
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            return None;
        }
        self.pending
            .remove(id)
            .map(|_| lsp_server::Response::new_err(id.clone(), -32800, "Request cancelled".into()))
    }

    pub(super) fn poll(&mut self, now: std::time::Instant) -> Vec<lsp_server::Response> {
        let mut responses = Vec::new();
        self.pending
            .retain(|id, pending| match pending.receiver.try_recv() {
                Ok(mut response) => {
                    response.id = id.clone();
                    responses.push(response);
                    false
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    responses.push(lsp_server::Response::new_err(
                        id.clone(),
                        -32603,
                        "Editor unavailable".into(),
                    ));
                    false
                }
                Err(mpsc::TryRecvError::Empty)
                    if now >= pending.deadline
                        && pending
                            .active
                            .swap(false, std::sync::atomic::Ordering::AcqRel) =>
                {
                    responses.push(lsp_server::Response::new_err(
                        id.clone(),
                        -32800,
                        "Request timed out".into(),
                    ));
                    false
                }
                Err(mpsc::TryRecvError::Empty) => true,
            });
        responses
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn outbound_timeout_emits_cancel_and_typed_error_once() {
        let (tx, rx) = mpsc::channel();
        let mut pending = PendingClientRequests {
            entries: HashMap::new(),
            deadlines: HashMap::new(),
            events: tx,
        };
        let now = std::time::Instant::now();
        pending.insert_at(5, (42, "completion"), now);
        assert!(pending.expire(now + Duration::from_secs(29)).is_empty());
        let events = pending.expire(now + Duration::from_secs(30));
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0],Message::Notification(n)if n.method=="$/cancelRequest"&&n.params["id"]==5)
        );
        assert!(matches!(
            rx.try_recv(),
            Ok(LspUiEvent::RequestRejected {
                request_id: 42,
                error: graphite_core_api::ServiceError::Timeout
            })
        ));
        assert!(pending.expire(now + Duration::from_secs(31)).is_empty());
    }
    #[test]
    fn shutdown_fails_every_pending_request() {
        let (tx, rx) = mpsc::channel();
        {
            let mut pending = PendingClientRequests {
                entries: HashMap::new(),
                deadlines: HashMap::new(),
                events: tx,
            };
            pending.insert(1, (7, "hover"));
            pending.insert(2, (8, "executeCommand"));
        }
        let mut ids: Vec<_> = rx
            .try_iter()
            .map(|event| match event {
                LspUiEvent::RequestFailed { request_id, .. } => request_id,
                _ => panic!("unexpected event"),
            })
            .collect();
        ids.sort();
        assert_eq!(ids, vec![7, 8]);
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    #[test]
    fn workspace_requests_are_validated_and_cancelled_before_claim() {
        let (tx, rx) = mpsc::channel();
        let mut requests = ServerRequests::default();
        let invalid = Request {
            id: RequestId::from(1),
            method: "workspace/configuration".into(),
            params: json!({"items":"invalid"}),
        };
        assert_eq!(
            requests.handle(invalid, &tx).unwrap().error.unwrap().code,
            -32602
        );
        assert!(rx.try_recv().is_err());
        let request = Request {
            id: RequestId::from(2),
            method: "workspace/configuration".into(),
            params: json!({"items":[{"section":"editor"}]}),
        };
        assert!(requests.handle(request, &tx).is_none());
        let event = rx.try_recv().unwrap();
        requests.cancel(&RequestId::from(2)).unwrap();
        match event {
            LspUiEvent::ServiceRequest { active, .. } => {
                assert!(!active.load(std::sync::atomic::Ordering::Acquire))
            }
            _ => panic!("unexpected request"),
        }
    }
}
