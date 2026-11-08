//! Deterministic simulation peer; no extension loading or Code OSS compatibility.
use shoppe_core_api::{protocol::*, *};
use std::{
    collections::HashMap,
    io::{stdin, stdout},
};
fn main() {
    if let Err(error) = run() {
        eprintln!("Simulated extension host: {error}");
        std::process::exit(1)
    }
}
fn run() -> ServiceResult<()> {
    let mut input = stdin().lock();
    let mut output = stdout().lock();
    let Some(Message::Hello {
        version: VERSION,
        session,
    }) = read_message(&mut input)?
    else {
        return Err(ServiceError::Protocol("Handshake required".into()));
    };
    let scenario = std::env::args().nth(1);
    if scenario.as_deref() == Some("--no-handshake") {
        while read_message(&mut input)?.is_some() {}
        return Ok(());
    }
    write_message(
        &mut output,
        &Message::Ready {
            version: if scenario.as_deref() == Some("--invalid-version") {
                VERSION + 1
            } else {
                VERSION
            },
            session,
        },
    )?;
    let mut next = 0u64;
    for (id, title) in [
        ("echo", "Echo JSON"),
        ("wait", "Wait for cancellation"),
        ("crash", "Crash simulated host"),
        ("service", "Call editor service"),
    ] {
        next += 1;
        write_message(
            &mut output,
            &Message::Request {
                session,
                id: next,
                request: RpcRequest::RegisterCommand(CommandMetadata {
                    id: format!("shoppe.simulated.{id}"),
                    title: title.into(),
                    category: "Simulation".into(),
                    targets: vec![TargetKind::Global, TargetKind::View, TargetKind::Widget],
                    enablement: "true".into(),
                }),
            },
        )?;
    }
    next += 1;
    write_message(
        &mut output,
        &Message::Request {
            session,
            id: next,
            request: RpcRequest::SubscribeDocuments,
        },
    )?;
    let mut forwarding = HashMap::<u64, u64>::new();
    let mut waiting = HashMap::<u64, ()>::new();
    loop {
        let Some(message) = read_message(&mut input)? else {
            return Ok(());
        };
        if message.session() != session {
            continue;
        }
        match message {
            Message::Shutdown { .. } => return Ok(()),
            Message::Request {
                id,
                request: RpcRequest::Invoke { command, arguments },
                ..
            } => {
                if forwarding.len() + waiting.len() >= MAX_PENDING {
                    write_message(
                        &mut output,
                        &Message::Response {
                            session,
                            id,
                            result: Err(ServiceError::Busy),
                        },
                    )?;
                    continue;
                }
                match command.as_str() {
                    "shoppe.simulated.echo" => write_message(
                        &mut output,
                        &Message::Response {
                            session,
                            id,
                            result: Ok(arguments),
                        },
                    )?,
                    "shoppe.simulated.duplicate" => {
                        for _ in 0..2 {
                            write_message(
                                &mut output,
                                &Message::Response {
                                    session,
                                    id,
                                    result: Ok(arguments.clone()),
                                },
                            )?;
                        }
                    }
                    "shoppe.simulated.oldSession" => {
                        write_message(
                            &mut output,
                            &Message::Response {
                                session: session.saturating_sub(1),
                                id,
                                result: Ok(serde_json::Value::Null),
                            },
                        )?;
                        write_message(
                            &mut output,
                            &Message::Response {
                                session,
                                id,
                                result: Ok(arguments),
                            },
                        )?;
                    }
                    "shoppe.simulated.wait" => {
                        waiting.insert(id, ());
                    }
                    "shoppe.simulated.crash" => std::process::exit(17),
                    "shoppe.simulated.service" => {
                        let request: ServiceRequest = match serde_json::from_value(arguments) {
                            Ok(request) => request,
                            Err(error) => {
                                write_message(
                                    &mut output,
                                    &Message::Response {
                                        session,
                                        id,
                                        result: Err(ServiceError::InvalidArguments(
                                            error.to_string(),
                                        )),
                                    },
                                )?;
                                continue;
                            }
                        };
                        next += 1;
                        forwarding.insert(next, id);
                        write_message(
                            &mut output,
                            &Message::Request {
                                session,
                                id: next,
                                request: RpcRequest::Service {
                                    request,
                                    parent: Some(id),
                                },
                            },
                        )?;
                    }
                    "shoppe.simulated.invalidProtocol" => {
                        use std::io::Write;
                        output
                            .write_all(&[255, 255, 255, 255])
                            .map_err(|e| ServiceError::Io(e.to_string()))?;
                        output
                            .flush()
                            .map_err(|e| ServiceError::Io(e.to_string()))?;
                    }
                    _ => write_message(
                        &mut output,
                        &Message::Response {
                            session,
                            id,
                            result: Err(ServiceError::NotFound),
                        },
                    )?,
                }
            }
            Message::Response { id, result, .. } => {
                if let Some(original) = forwarding.remove(&id) {
                    write_message(
                        &mut output,
                        &Message::Response {
                            session,
                            id: original,
                            result,
                        },
                    )?;
                }
            }
            Message::Cancel { id, .. } => {
                waiting.remove(&id);
                let forwarded = forwarding
                    .iter()
                    .find(|(_, original)| **original == id)
                    .map(|(forward, _)| *forward);
                if let Some(forward) = forwarded {
                    forwarding.remove(&forward);
                    write_message(
                        &mut output,
                        &Message::Cancel {
                            session,
                            id: forward,
                        },
                    )?;
                }
                write_message(
                    &mut output,
                    &Message::Response {
                        session,
                        id,
                        result: Err(ServiceError::Cancelled),
                    },
                )?;
            }
            Message::WaitingForUser { id, .. } => {
                if let Some(original) = forwarding.get(&id) {
                    write_message(
                        &mut output,
                        &Message::WaitingForUser {
                            session,
                            id: *original,
                        },
                    )?;
                }
            }
            Message::Event { .. } => {}
            _ => return Err(ServiceError::Protocol("Unexpected editor message".into())),
        }
    }
}
