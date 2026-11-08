//! Nonblocking supervisor for an isolated, explicitly activated simulation.
mod pending;
mod transport;
pub use pending::{PendingRequests, RequestState};
use shoppe_core_api::{protocol::*, CancellationToken, ServiceError, ServiceResult};
use std::{
    collections::VecDeque,
    path::Path,
    process::Child,
    sync::{
        mpsc::{Receiver, SyncSender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub enum HostEvent {
    Ready {
        session: u64,
    },
    Request {
        session: u64,
        id: u64,
        request: RpcRequest,
    },
    Cancel {
        session: u64,
        id: u64,
    },
    Completed {
        session: u64,
        id: u64,
        result: ServiceResult<serde_json::Value>,
    },
    Stopped {
        session: u64,
        error: ServiceError,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Starting,
    Ready,
    Stopped,
}
pub struct Supervisor {
    pub session: u64,
    pub state: HostState,
    child: Option<Child>,
    outgoing: Option<SyncSender<Vec<u8>>>,
    incoming: Option<Receiver<Message>>,
    fault: Arc<Mutex<Option<ServiceError>>>,
    handshake_deadline: Instant,
    pub pending: PendingRequests,
    events: VecDeque<HostEvent>,
}
impl Supervisor {
    pub fn spawn(
        executable: &Path,
        session: u64,
        now: Instant,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> ServiceResult<Self> {
        Self::spawn_with_args(executable, &[], session, now, wake)
    }
    pub fn spawn_with_args(
        executable: &Path,
        args: &[std::ffi::OsString],
        session: u64,
        now: Instant,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> ServiceResult<Self> {
        let (child, outgoing, incoming, fault) = transport::spawn(executable, args, wake)?;
        let mut host = Self {
            session,
            state: HostState::Starting,
            child: Some(child),
            outgoing: Some(outgoing),
            incoming: Some(incoming),
            fault,
            handshake_deadline: now + Duration::from_secs(5),
            pending: PendingRequests::default(),
            events: VecDeque::new(),
        };
        host.send(Message::Hello {
            version: VERSION,
            session,
        })?;
        Ok(host)
    }
    pub fn send(&mut self, message: Message) -> ServiceResult<()> {
        if self.state == HostState::Stopped {
            return Err(ServiceError::HostStopped);
        }
        let bytes = encode(&message)?;
        self.outgoing
            .as_ref()
            .ok_or(ServiceError::HostStopped)?
            .try_send(bytes)
            .map_err(|e| match e {
                std::sync::mpsc::TrySendError::Full(_) => ServiceError::Busy,
                _ => ServiceError::HostStopped,
            })
    }
    pub fn request(
        &mut self,
        request: RpcRequest,
        cancellation: CancellationToken,
        now: Instant,
    ) -> ServiceResult<u64> {
        if self.state != HostState::Ready {
            return Err(ServiceError::HostStopped);
        }
        let id = self.pending.insert(cancellation, now)?;
        if let Err(error) = self.send(Message::Request {
            session: self.session,
            id,
            request,
        }) {
            self.pending.remove(id);
            return Err(error);
        }
        Ok(id)
    }
    pub fn cancel(&mut self, id: u64) {
        self.pending.cancel(id);
    }
    pub fn poll(&mut self, now: Instant) -> Vec<HostEvent> {
        if self.state == HostState::Stopped {
            return self.events.drain(..).collect();
        }
        if self.state == HostState::Starting && now >= self.handshake_deadline {
            self.stop(ServiceError::Timeout);
            return self.events.drain(..).collect();
        }
        for (id, error) in self.pending.expire(now) {
            let _ = self.send(Message::Cancel {
                session: self.session,
                id,
            });
            self.events.push_back(HostEvent::Completed {
                session: self.session,
                id,
                result: Err(error),
            });
        }
        while let Some(message) = self
            .incoming
            .as_ref()
            .and_then(|incoming| incoming.try_recv().ok())
        {
            if message.session() != self.session {
                continue;
            }
            match message {
                Message::Ready {
                    version: VERSION, ..
                } if self.state == HostState::Starting => {
                    self.state = HostState::Ready;
                    self.events.push_back(HostEvent::Ready {
                        session: self.session,
                    });
                }
                Message::Response { id, result, .. } if self.state == HostState::Ready => {
                    if self.pending.remove(id).is_some() {
                        self.events.push_back(HostEvent::Completed {
                            session: self.session,
                            id,
                            result,
                        });
                    }
                }
                Message::Request { id, request, .. } if self.state == HostState::Ready => {
                    self.events.push_back(HostEvent::Request {
                        session: self.session,
                        id,
                        request,
                    })
                }
                Message::WaitingForUser { id, .. } if self.state == HostState::Ready => {
                    self.pending.suspend(id)
                }
                Message::Resumed { id, .. } if self.state == HostState::Ready => {
                    self.pending.resume(id, now)
                }
                Message::Cancel { id, .. } if self.state == HostState::Ready => {
                    self.events.push_back(HostEvent::Cancel {
                        session: self.session,
                        id,
                    })
                }
                _ => {
                    self.stop(ServiceError::Protocol(
                        "Unexpected host message or version".into(),
                    ));
                    break;
                }
            }
        }
        if self.state != HostState::Stopped {
            if self.state == HostState::Starting && now >= self.handshake_deadline {
                self.stop(ServiceError::Timeout)
            } else {
                let fault = self.fault.lock().ok().and_then(|fault| fault.clone());
                if let Some(error) = fault {
                    self.stop(error)
                } else if self
                    .child
                    .as_mut()
                    .is_some_and(|child| child.try_wait().ok().flatten().is_some())
                {
                    self.stop(ServiceError::HostStopped)
                }
            }
        }
        self.events.drain(..).collect()
    }
    pub fn stop(&mut self, error: ServiceError) {
        if self.state == HostState::Stopped {
            return;
        }
        self.state = HostState::Stopped;
        self.outgoing.take();
        self.incoming.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        for id in self.pending.drain() {
            self.events.push_back(HostEvent::Completed {
                session: self.session,
                id,
                result: Err(error.clone()),
            });
        }
        self.events.push_back(HostEvent::Stopped {
            session: self.session,
            error,
        });
    }
    /// Give a real plugin a bounded opportunity to deactivate, without blocking the UI.
    pub fn shutdown(&mut self) {
        if self.state == HostState::Stopped {
            return;
        }
        let _ = self.send(Message::Shutdown {
            session: self.session,
        });
        if let Some(mut child) = self.child.take() {
            std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_millis(1100);
                while Instant::now() < deadline {
                    if child.try_wait().ok().flatten().is_some() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                let _ = child.kill();
                let _ = child.wait();
            });
        }
        self.stop(ServiceError::HostStopped);
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.stop(ServiceError::HostStopped);
    }
}
