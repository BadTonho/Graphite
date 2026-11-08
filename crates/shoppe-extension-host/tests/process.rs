use serde_json::{json, Value};
use graphite_core_api::{protocol::*, *};
use graphite_extension_host::{HostEvent, HostState, Supervisor};
use std::{
    path::Path,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
struct Harness {
    host: Supervisor,
    wakes: mpsc::Receiver<()>,
}
impl Harness {
    fn new(session: u64) -> Self {
        let (tx, wakes) = mpsc::channel();
        let wake = Arc::new(move || {
            let _ = tx.send(());
        });
        let host = Supervisor::spawn(
            Path::new(env!("CARGO_BIN_EXE_graphite-extension-host")),
            session,
            Instant::now(),
            wake,
        )
        .unwrap();
        let mut fixture = Self { host, wakes };
        fixture.until(|events| events.iter().any(|e| matches!(e, HostEvent::Ready { .. })));
        fixture
    }
    fn until(&mut self, done: impl Fn(&[HostEvent]) -> bool) -> Vec<HostEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut all = Vec::new();
        loop {
            let events = self.host.poll(Instant::now());
            for event in &events {
                if let HostEvent::Request { id, .. } = event {
                    if self.host.state != HostState::Ready {
                        continue;
                    }
                    self.host
                        .send(Message::Response {
                            session: self.host.session,
                            id: *id,
                            result: Ok(Value::Null),
                        })
                        .unwrap();
                }
            }
            all.extend(events);
            if done(&all) {
                return all;
            }
            self.wakes
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("process did not wake supervisor");
        }
    }
    fn invoke(&mut self, id: &str, args: Value) -> u64 {
        self.host
            .request(
                RpcRequest::Invoke {
                    command: format!("shoppe.simulated.{id}"),
                    arguments: args,
                },
                CancellationToken::default(),
                Instant::now(),
            )
            .unwrap()
    }
}
#[test]
fn real_process_handshake_json_result_crash_and_new_session() {
    let mut h = Harness::new(71);
    let echo = h.invoke("echo", json!({"unicode":"\u{1f30e}","n":4}));
    let events = h.until(|events| {
        events
            .iter()
            .any(|e| matches!(e,HostEvent::Completed{id,..}if *id==echo))
    });
    assert!(events.iter().any(
        |e| matches!(e,HostEvent::Completed{id,result:Ok(value),..}if *id==echo&&value["n"]==4)
    ));
    let wait = h.invoke("wait", Value::Null);
    h.invoke("crash", Value::Null);
    let events = h.until(|events| {
        events
            .iter()
            .any(|e| matches!(e, HostEvent::Stopped { .. }))
    });
    assert!(events
        .iter()
        .any(|e| matches!(e,HostEvent::Completed{id,result:Err(_),..}if *id==wait)));
    assert_eq!(h.host.state, HostState::Stopped);
    assert!(h.host.pending.is_empty());
    let mut next = Harness::new(72);
    assert_ne!(next.host.session, h.host.session);
    let echo = next.invoke("echo", json!("restart"));
    next.until(|events| {
        events
            .iter()
            .any(|e| matches!(e,HostEvent::Completed{id,result:Ok(_),..}if *id==echo))
    });
}
#[test]
fn cancellation_and_timeout_win_over_late_replies() {
    let mut h = Harness::new(80);
    let wait = h.invoke("wait", Value::Null);
    h.host.cancel(wait);
    let events = h.host.poll(Instant::now());
    assert!(events.iter().any(
        |e| matches!(e,HostEvent::Completed{id,result:Err(ServiceError::Cancelled),..}if *id==wait)
    ));
    let wait = h.invoke("wait", Value::Null);
    let events = h.host.poll(Instant::now() + Duration::from_secs(31));
    assert!(events.iter().any(
        |e| matches!(e,HostEvent::Completed{id,result:Err(ServiceError::Timeout),..}if *id==wait)
    ));
    assert!(h.host.pending.is_empty());
    let echo = h.invoke("echo", json!(true));
    let events = h.until(|events| {
        events
            .iter()
            .any(|e| matches!(e,HostEvent::Completed{id,result:Ok(_),..}if *id==echo))
    });
    assert!(!events
        .iter()
        .any(|e| matches!(e,HostEvent::Completed{id,..}if *id==wait)));
}
#[test]
fn malformed_child_output_terminates_only_the_supervised_session() {
    let mut h = Harness::new(90);
    h.invoke("invalidProtocol", Value::Null);
    h.until(|events| {
        events
            .iter()
            .any(|e| matches!(e, HostEvent::Stopped { .. }))
    });
    assert!(h.host.pending.is_empty());
}

#[test]
fn handshake_timeout_and_incompatible_version_are_explicit_failures() {
    let executable = Path::new(env!("CARGO_BIN_EXE_graphite-extension-host"));
    let now = Instant::now();
    let mut silent = Supervisor::spawn_with_args(
        executable,
        &["--no-handshake".into()],
        100,
        now,
        Arc::new(|| {}),
    )
    .unwrap();
    let events = silent.poll(now + Duration::from_secs(6));
    assert!(events.iter().any(|event| matches!(
        event,
        HostEvent::Stopped {
            error: ServiceError::Timeout,
            ..
        }
    )));
    let (tx, rx) = mpsc::channel();
    let mut incompatible = Supervisor::spawn_with_args(
        executable,
        &["--invalid-version".into()],
        101,
        now,
        Arc::new(move || {
            let _ = tx.send(());
        }),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let events = incompatible.poll(Instant::now());
        if events.iter().any(|event| {
            matches!(
                event,
                HostEvent::Stopped {
                    error: ServiceError::Protocol(_),
                    ..
                }
            )
        }) {
            break;
        }
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
    }
}

#[test]
fn duplicate_and_stale_session_responses_never_complete_an_execution_twice() {
    let mut h = Harness::new(110);
    let duplicate = h.invoke("duplicate", json!("once"));
    let mut events = h.until(|events| {
        events
            .iter()
            .any(|event| matches!(event,HostEvent::Completed{id,..}if *id==duplicate))
    });
    let old = h.invoke("oldSession", json!("current"));
    events.extend(h.until(|events|events.iter().any(|event|matches!(event,HostEvent::Completed{id,result:Ok(value),..}if *id==old&&value=="current"))));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event,HostEvent::Completed{id,..}if *id==duplicate))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event,HostEvent::Completed{id,..}if *id==old))
            .count(),
        1
    );
}
