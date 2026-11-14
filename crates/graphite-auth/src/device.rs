use crate::store::Session;
use crate::{Transport, DEVICE_CODE_URL, SCOPE, TOKEN_URL, USER_URL};
use serde_json::Value;
use std::time::Duration;

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// GitHub asks for this much longer wait after a `slow_down` answer.
const SLOW_DOWN_STEP: u64 = 5;

/// The code the user types on github.com, and how to poll for the token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds to wait between polls.
    pub interval: u64,
    /// Seconds until the code expires.
    pub expires_in: u64,
}

/// One answer to a token request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    /// The user has not confirmed yet.
    Pending,
    /// Poll less often.
    SlowDown,
    Token(String),
    /// The user refused on github.com.
    Denied,
    Expired,
    Failed(String),
}

fn error_message(value: &Value) -> Option<String> {
    let error = value.get("error")?.as_str()?;
    Some(
        value
            .get("error_description")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| error.to_owned()),
    )
}

/// Reads the device-code response. Missing optional timing fields get GitHub's defaults.
pub fn parse_device_code(value: &Value) -> Result<DeviceCode, String> {
    if let Some(error) = error_message(value) {
        return Err(error);
    }
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("GitHub did not return {key}"))
    };
    Ok(DeviceCode {
        device_code: text("device_code")?,
        user_code: text("user_code")?,
        verification_uri: text("verification_uri")?,
        interval: value.get("interval").and_then(Value::as_u64).unwrap_or(5),
        expires_in: value
            .get("expires_in")
            .and_then(Value::as_u64)
            .unwrap_or(900),
    })
}

/// Reads one token response.
pub fn parse_poll(value: &Value) -> Poll {
    if let Some(token) = value.get("access_token").and_then(Value::as_str) {
        return Poll::Token(token.to_owned());
    }
    match value.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => Poll::Pending,
        Some("slow_down") => Poll::SlowDown,
        Some("expired_token") => Poll::Expired,
        Some("access_denied") => Poll::Denied,
        Some(_) => Poll::Failed(error_message(value).unwrap_or_default()),
        None => Poll::Failed("GitHub returned an unexpected sign-in response".into()),
    }
}

/// Reads the signed-in user's profile for a token.
pub fn parse_user(token: &str, value: &Value) -> Result<Session, String> {
    let login = value
        .get("login")
        .and_then(Value::as_str)
        .filter(|login| !login.is_empty())
        .ok_or("GitHub did not return the account name")?;
    Ok(Session {
        token: token.to_owned(),
        login: login.to_owned(),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        scope: String::new(),
    })
}

/// Asks GitHub for a new device code using the default `read:user` scope.
pub fn start(transport: &dyn Transport, client_id: &str) -> Result<DeviceCode, String> {
    start_with_scope(transport, client_id, SCOPE)
}

/// Asks GitHub for a new device code using an explicit OAuth `scope`.
pub fn start_with_scope(
    transport: &dyn Transport,
    client_id: &str,
    scope: &str,
) -> Result<DeviceCode, String> {
    let value = transport.post_form(
        DEVICE_CODE_URL,
        &[("client_id", client_id), ("scope", scope)],
    )?;
    parse_device_code(&value)
}

/// Asks GitHub once whether the user has confirmed the code.
pub fn poll(transport: &dyn Transport, client_id: &str, device_code: &str) -> Result<Poll, String> {
    let value = transport.post_form(
        TOKEN_URL,
        &[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", DEVICE_GRANT),
        ],
    )?;
    Ok(parse_poll(&value))
}

/// The signed-in profile for an access token.
pub fn fetch_session(transport: &dyn Transport, token: &str) -> Result<Session, String> {
    let value = transport.get_json(USER_URL, token)?;
    parse_user(token, &value)
}

/// Runs the whole device flow.
///
/// `on_code` receives the code to show the user (and can open the verification page). `wait`
/// sleeps between polls; the app passes `thread::sleep`, tests pass a no-op.
pub fn sign_in(
    transport: &dyn Transport,
    client_id: &str,
    on_code: impl FnMut(&DeviceCode),
    wait: impl FnMut(Duration),
) -> Result<Session, String> {
    sign_in_with_scope(transport, client_id, SCOPE, on_code, wait)
}

/// Runs the whole device flow with an explicit OAuth `scope`.
pub fn sign_in_with_scope(
    transport: &dyn Transport,
    client_id: &str,
    scope: &str,
    mut on_code: impl FnMut(&DeviceCode),
    mut wait: impl FnMut(Duration),
) -> Result<Session, String> {
    let code = start_with_scope(transport, client_id, scope)?;
    on_code(&code);
    let mut interval = code.interval.max(1);
    let mut remaining = code.expires_in;
    loop {
        match poll(transport, client_id, &code.device_code)? {
            Poll::Pending => {}
            Poll::SlowDown => interval += SLOW_DOWN_STEP,
            Poll::Token(token) => {
                return fetch_session(transport, &token).map(|mut session| {
                    session.scope = scope.split_whitespace().collect::<Vec<_>>().join(" ");
                    session
                })
            }
            Poll::Denied => return Err("Sign-in was refused on GitHub".into()),
            Poll::Expired => return Err("The sign-in code expired. Try again.".into()),
            Poll::Failed(error) => return Err(error),
        }
        if remaining <= interval {
            return Err("The sign-in code expired. Try again.".into());
        }
        remaining -= interval;
        wait(Duration::from_secs(interval));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    /// Replays scripted answers and records what was asked.
    struct Fake {
        posts: RefCell<Vec<String>>,
        answers: RefCell<Vec<Value>>,
        user: Value,
    }

    impl Fake {
        fn new(answers: Vec<Value>) -> Self {
            Self {
                posts: RefCell::new(Vec::new()),
                answers: RefCell::new(answers.into_iter().rev().collect()),
                user: json!({"login": "octocat", "name": "Mona"}),
            }
        }
    }

    impl Transport for Fake {
        fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, String> {
            let fields: Vec<String> = form.iter().map(|(k, v)| format!("{k}={v}")).collect();
            self.posts
                .borrow_mut()
                .push(format!("{url} {}", fields.join("&")));
            self.answers
                .borrow_mut()
                .pop()
                .ok_or_else(|| "no scripted answer".to_owned())
        }

        fn get_json(&self, url: &str, token: &str) -> Result<Value, String> {
            assert_eq!(url, USER_URL);
            assert_eq!(token, "gho_test");
            Ok(self.user.clone())
        }
    }

    fn code_response() -> Value {
        json!({"device_code": "dc", "user_code": "WDJB-MJHT",
            "verification_uri": "https://github.com/login/device", "interval": 5, "expires_in": 900})
    }

    #[test]
    fn device_code_response_is_read_and_errors_are_reported() {
        let code = parse_device_code(&code_response()).unwrap();
        assert_eq!(code.user_code, "WDJB-MJHT");
        assert_eq!(code.interval, 5);
        let defaults = parse_device_code(
            &json!({"device_code": "d", "user_code": "u", "verification_uri": "v"}),
        )
        .unwrap();
        assert_eq!((defaults.interval, defaults.expires_in), (5, 900));
        assert_eq!(
            parse_device_code(
                &json!({"error": "incorrect_client_credentials", "error_description": "bad client"})
            ),
            Err("bad client".to_owned())
        );
        assert!(parse_device_code(&json!({"device_code": "d"})).is_err());
    }

    #[test]
    fn token_responses_map_to_poll_outcomes() {
        assert_eq!(
            parse_poll(&json!({"access_token": "gho_x", "token_type": "bearer"})),
            Poll::Token("gho_x".into())
        );
        assert_eq!(
            parse_poll(&json!({"error": "authorization_pending"})),
            Poll::Pending
        );
        assert_eq!(parse_poll(&json!({"error": "slow_down"})), Poll::SlowDown);
        assert_eq!(
            parse_poll(&json!({"error": "expired_token"})),
            Poll::Expired
        );
        assert_eq!(parse_poll(&json!({"error": "access_denied"})), Poll::Denied);
        assert_eq!(
            parse_poll(&json!({"error": "unsupported_grant_type", "error_description": "no"})),
            Poll::Failed("no".into())
        );
        assert!(matches!(parse_poll(&json!({})), Poll::Failed(_)));
    }

    #[test]
    fn sign_in_waits_through_pending_and_slow_down_then_returns_the_user() {
        let fake = Fake::new(vec![
            code_response(),
            json!({"error": "authorization_pending"}),
            json!({"error": "slow_down"}),
            json!({"access_token": "gho_test"}),
        ]);
        let mut shown = None;
        let mut waits = Vec::new();
        let session = sign_in(
            &fake,
            "client",
            |code| shown = Some(code.user_code.clone()),
            |wait| waits.push(wait.as_secs()),
        )
        .unwrap();
        assert_eq!(shown.as_deref(), Some("WDJB-MJHT"));
        assert_eq!(waits, vec![5, 10]);
        assert_eq!(session.login, "octocat");
        assert_eq!(session.name.as_deref(), Some("Mona"));
        assert_eq!(session.token, "gho_test");
        assert_eq!(session.scope, "read:user", "the session remembers its scopes");
        assert!(fake.posts.borrow()[0].contains("scope=read:user"));
        assert!(fake.posts.borrow()[1]
            .contains("grant_type=urn:ietf:params:oauth:grant-type:device_code"));
    }

    #[test]
    fn the_requested_scopes_are_recorded_with_single_spaces() {
        let fake = Fake::new(vec![code_response(), json!({"access_token": "gho_test"})]);
        let session = sign_in_with_scope(&fake, "client", "read:user   gist", |_| {}, |_| {}).unwrap();
        assert_eq!(session.scope, "read:user gist");
    }

    #[test]
    fn refusal_expiry_and_timeouts_end_the_flow_with_errors() {
        let refused = Fake::new(vec![code_response(), json!({"error": "access_denied"})]);
        assert!(sign_in(&refused, "c", |_| {}, |_| {})
            .unwrap_err()
            .contains("refused"));

        let expired = Fake::new(vec![code_response(), json!({"error": "expired_token"})]);
        assert!(sign_in(&expired, "c", |_| {}, |_| {})
            .unwrap_err()
            .contains("expired"));

        // Every poll is pending and the code runs out of time after a few waits.
        let short = json!({"device_code": "dc", "user_code": "U", "verification_uri": "v", "interval": 5, "expires_in": 15});
        let never = Fake::new(vec![
            short,
            json!({"error": "authorization_pending"}),
            json!({"error": "authorization_pending"}),
            json!({"error": "authorization_pending"}),
        ]);
        let mut waits = 0;
        assert!(sign_in(&never, "c", |_| {}, |_| waits += 1)
            .unwrap_err()
            .contains("expired"));
        assert_eq!(waits, 2);
    }
}
