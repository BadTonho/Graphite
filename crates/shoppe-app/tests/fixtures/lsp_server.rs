//! Deterministic stdio server for worker regression tests. No network or user files.
use lsp_server::{Message, Notification, Request, RequestId, Response};
use serde_json::{json, Value};
use std::io::{self, BufReader};

fn send(message: Message) {
    message.write(&mut io::stdout().lock()).unwrap();
}
fn status(message: &str) {
    send(Message::Notification(Notification::new(
        "window/logMessage".into(),
        json!({"type":3,"message":message}),
    )));
}
fn main() {
    let mut reader = BufReader::new(io::stdin().lock());
    let mut held = None;
    while let Some(message) = Message::read(&mut reader).unwrap() {
        match message {
            Message::Request(request) => match request.method.as_str() {
                "initialize" => send(Message::Response(Response::new_ok(
                    request.id,
                    json!({"capabilities":{"textDocumentSync":1,"completionProvider":{},"executeCommandProvider":{"commands":["echo","hold","crash","apply"]}}}),
                ))),
                "shutdown" => send(Message::Response(Response::new_ok(request.id, Value::Null))),
                "textDocument/completion" => send(Message::Response(Response::new_ok(
                    request.id,
                    json!({"malformed":true}),
                ))),
                "workspace/executeCommand" => match request.params["command"].as_str().unwrap() {
                    "echo" => send(Message::Response(Response::new_ok(
                        request.id,
                        request.params["arguments"].clone(),
                    ))),
                    "hold" => {
                        held = Some(request.id);
                        status("fixture:held");
                    }
                    "crash" => return,
                    "apply" => {
                        send(Message::Request(Request::new(
                            RequestId::from(900),
                            "workspace/applyEdit".into(),
                            request.params["arguments"][0].clone(),
                        )));
                        held = Some(request.id);
                    }
                    other => panic!("unknown fixture command {other}"),
                },
                other => panic!("unexpected request {other}"),
            },
            Message::Notification(notification) => match notification.method.as_str() {
                "exit" => return,
                "$/cancelRequest" => {
                    let id = held.take().expect("cancellation must follow hold");
                    assert_eq!(
                        serde_json::to_value(&id).unwrap(),
                        notification.params["id"]
                    );
                    send(Message::Response(Response::new_ok(id, json!("late"))));
                    // Stream ordering makes this an acknowledgement of the late response.
                    status("fixture:cancelled");
                }
                _ => {}
            },
            Message::Response(response) => {
                assert_eq!(response.id, RequestId::from(900));
                send(Message::Response(Response::new_ok(
                    held.take().unwrap(),
                    response.result.unwrap_or(Value::Null),
                )));
            }
        }
    }
}
