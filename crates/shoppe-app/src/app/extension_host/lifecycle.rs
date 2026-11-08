use super::*;
impl App {
    pub(crate) fn restart_extension_host(&mut self) -> Result<(), CommandError> {
        if !self.extension_host.enabled {
            return Err(CommandError::Disabled(
                "Simulated host is not enabled".into(),
            ));
        }
        if let Some(mut old) = self.extension_host.supervisor.take() {
            old.stop(ServiceError::HostStopped);
        }
        self.clear_host_session();
        self.extension_host.next_session += 1;
        let session = self.extension_host.next_session;
        let executable = self
            .extension_host
            .executable
            .clone()
            .or_else(|| {
                std::env::current_exe().ok().and_then(|p| {
                    p.parent().map(|dir| {
                        dir.join(if cfg!(windows) {
                            "shoppe-extension-host.exe"
                        } else {
                            "shoppe-extension-host"
                        })
                    })
                })
            })
            .ok_or_else(|| {
                CommandError::Failed("Cannot locate simulated host executable".into())
            })?;
        let proxy = self.event_proxy.clone();
        let wake = self.extension_host.wake.clone().unwrap_or_else(|| {
            Arc::new(move || {
                if let Some(proxy) = &proxy {
                    let _ = proxy.send_event(UserEvent::ExtensionHostWake);
                }
            })
        });
        self.extension_host.supervisor = Some(
            Supervisor::spawn(&executable, session, Instant::now(), wake).map_err(command_error)?,
        );
        self.status = "Simulated extension host starting".into();
        Ok(())
    }
    fn clear_host_session(&mut self) {
        self.extension_host.commands.clear();
        self.extension_host.participants.clear();
        self.extension_host.save_replies.clear();
        self.extension_host.subscriptions.clear();
        self.document_events.reap();
        self.extension_host.last_request = 0;
        let outbound = std::mem::take(&mut self.extension_host.outbound);
        for (_, execution) in outbound {
            self.cancel_pending_host_execution(execution, CommandError::HostStopped);
        }
        let incoming = std::mem::take(&mut self.extension_host.incoming);
        for (_, incoming) in incoming {
            self.cancel_command(incoming.ticket.id);
        }
    }
    fn cancel_pending_host_execution(&mut self, execution: u64, error: CommandError) {
        if let Some(pending) = self.command_runtime.pending.get(&execution) {
            pending.cancellation.cancel();
        }
        self.finish_command(execution, Err(error));
    }
    pub(crate) fn poll_extension_host(&mut self, now: Instant) {
        self.reap_commands();
        // Cancelling a UI ticket also cancels its outstanding wire request.
        let cancelled: Vec<_> = self
            .extension_host
            .outbound
            .iter()
            .filter(|(_, execution)| !self.command_runtime.pending.contains_key(execution))
            .map(|(rpc, _)| *rpc)
            .collect();
        if let Some(host) = self.extension_host.supervisor.as_mut() {
            for rpc in cancelled {
                host.cancel(rpc);
            }
        }
        let events = self
            .extension_host
            .supervisor
            .as_mut()
            .map(|host| host.poll(now))
            .unwrap_or_default();
        // Deliver terminal errors before accepting successes: deadline cleanup
        // may cancel tokens shared with sibling executions in this same batch.
        for event in &events {
            if let HostEvent::Completed {
                id,
                result: Err(error),
                ..
            } = event
            {
                if let Some(execution) = self.extension_host.outbound.remove(id) {
                    self.finish_command(execution, Err(command_error(error.clone())));
                }
            }
        }
        let cancellations: HashSet<_> = events
            .iter()
            .filter_map(|event| {
                if let HostEvent::Cancel { id, .. } = event {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for event in events {
            match event {
                HostEvent::Ready { .. } => self.status = "Simulated extension host ready".into(),
                HostEvent::Stopped { error, .. } => {
                    self.clear_host_session();
                    self.status = format!("Simulated extension host stopped: {error}");
                }
                HostEvent::Completed { id, result, .. } => {
                    if let Some(sender) = self.extension_host.save_replies.remove(&id) {
                        let _ = sender.send(result);
                        continue;
                    }
                    if let Some(execution) = self.extension_host.outbound.remove(&id) {
                        match result {
                            Ok(value) => self.accept_command_event(execution, None, Ok(value)),
                            Err(error) => self.finish_command(execution, Err(command_error(error))),
                        }
                    }
                }
                HostEvent::Request {
                    session,
                    id,
                    request,
                } => {
                    if cancellations.contains(&id) {
                        self.reply_host(id, Err(ServiceError::Cancelled));
                    } else {
                        self.handle_host_request(session, id, request, now);
                    }
                }
                HostEvent::Cancel { id, .. } => {
                    if let Some(incoming) = self.extension_host.incoming.remove(&id) {
                        self.cancel_command(incoming.ticket.id);
                        self.reply_host(id, Err(ServiceError::Cancelled));
                    }
                }
            }
        }
        let ids: Vec<_> = self.extension_host.incoming.keys().copied().collect();
        for id in ids {
            let incoming = &self.extension_host.incoming[&id];
            if let Some(result) = incoming.ticket.try_result() {
                self.extension_host.incoming.remove(&id);
                self.reply_host(id, result.map_err(ServiceError::Command));
                continue;
            }
            let execution = incoming.ticket.id;
            let confirming = self
                .command_runtime
                .pending_close
                .is_some_and(|(pending, _)| pending == execution)
                || self.command_runtime.pending_window_close == Some(execution);
            if confirming && self.extension_host.incoming[&id].deadline.is_some() {
                self.extension_host.incoming.get_mut(&id).unwrap().deadline = None;
                if let Some(host) = self.extension_host.supervisor.as_mut() {
                    let _ = host.send(Message::WaitingForUser {
                        session: host.session,
                        id,
                    });
                }
            }
            if !confirming && self.extension_host.incoming[&id].deadline.is_none() {
                self.extension_host.incoming.get_mut(&id).unwrap().deadline =
                    Some(now + Duration::from_secs(30));
                if let Some(host) = self.extension_host.supervisor.as_mut() {
                    let _ = host.send(Message::Resumed {
                        session: host.session,
                        id,
                    });
                }
            }
            if self.extension_host.incoming[&id]
                .deadline
                .is_some_and(|deadline| now >= deadline)
            {
                self.extension_host.incoming.remove(&id);
                self.cancel_command(execution);
                self.reply_host(id, Err(ServiceError::Timeout));
            }
        }
        self.publish_host_documents();
    }
    pub(super) fn reply_host(&mut self, id: u64, result: ServiceResult<serde_json::Value>) {
        if let Some(host) = self.extension_host.supervisor.as_mut() {
            if let Err(error) = host.send(Message::Response {
                session: host.session,
                id,
                result,
            }) {
                host.stop(error);
            }
        }
    }
    fn publish_host_documents(&mut self) {
        self.reconcile_document_events();
        let mut events = Vec::new();
        for (resource, subscription) in &self.extension_host.subscriptions {
            if subscription.overflow.load(Ordering::Acquire) {
                if let Some(host) = self.extension_host.supervisor.as_mut() {
                    host.stop(ServiceError::Busy);
                }
                return;
            }
            for event in subscription.receiver.try_iter() {
                events.push((*resource, event));
            }
        }
        if let Some(host) = self.extension_host.supervisor.as_mut() {
            if host.state != HostState::Ready {
                return;
            }
            for (resource, event) in events {
                if let Err(error) = host.send(Message::Event {
                    session: host.session,
                    resource,
                    event,
                }) {
                    host.stop(error);
                    return;
                }
            }
        }
    }
}
