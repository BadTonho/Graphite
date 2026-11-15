//! Authentication providers (`MainThreadAuthentication` / `ExtHostAuthentication`).
//!
//! `authentication.getSession` asks Graphite for an account's session. Graphite answers `github`
//! itself with the account of its own GitHub sign-in, and any other provider through the extension
//! that registered it. An extension gets a session only after the user allowed it for that account;
//! the choice is remembered in `<profile>/code-oss/authentication-access.json`.

use super::*;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::Path;

/// The provider Graphite serves with its own sign-in.
pub(crate) const GITHUB: &str = "github";
const GITHUB_LABEL: &str = "GitHub";
/// Replies to calls to an authentication provider carry ids in `AUTH_BASE..AUTH_END`.
pub(super) const AUTH_BASE: u64 = 16_000_000;
const AUTH_END: u64 = 17_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const PROVIDER_WAIT: Duration = Duration::from_secs(10);
const ACCESS_FILE: &str = "authentication-access.json";

pub(super) fn is_auth_reply(id: u64) -> bool {
    (AUTH_BASE..AUTH_END).contains(&id)
}

/// Which extensions the user allowed to use which account: provider, then account, then extension.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AccessStore {
    allowed: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

impl AccessStore {
    pub(crate) fn load(profile: &Path) -> Self {
        std::fs::read(profile.join("code-oss").join(ACCESS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, profile: &Path) -> std::io::Result<()> {
        let directory = profile.join("code-oss");
        std::fs::create_dir_all(&directory)?;
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(directory.join(ACCESS_FILE), bytes)
    }

    pub(crate) fn is_allowed(&self, provider: &str, account: &str, extension: &str) -> bool {
        self.allowed
            .get(provider)
            .and_then(|accounts| accounts.get(account))
            .is_some_and(|extensions| extensions.contains(extension))
    }

    pub(crate) fn allow(&mut self, provider: &str, account: &str, extension: &str) {
        self.allowed
            .entry(provider.to_owned())
            .or_default()
            .entry(account.to_owned())
            .or_default()
            .insert(extension.to_owned());
    }

    /// Forgets every extension allowed for an account.
    pub(crate) fn revoke_account(&mut self, provider: &str, account: &str) {
        if let Some(accounts) = self.allowed.get_mut(provider) {
            accounts.remove(account);
        }
    }
}

/// Whether `granted` OAuth scopes cover every scope in `requested`. A broader scope implies the
/// narrower ones it contains (`user` covers `read:user`).
pub(crate) fn scopes_cover(granted: &[String], requested: &[String]) -> bool {
    const IMPLIED: [(&str, &[&str]); 9] = [
        ("user", &["read:user", "user:email", "user:follow"]),
        (
            "repo",
            &[
                "repo:status",
                "repo_deployment",
                "public_repo",
                "repo:invite",
                "security_events",
            ],
        ),
        ("admin:org", &["write:org", "read:org"]),
        ("write:org", &["read:org"]),
        ("admin:public_key", &["write:public_key", "read:public_key"]),
        ("write:public_key", &["read:public_key"]),
        ("admin:repo_hook", &["write:repo_hook", "read:repo_hook"]),
        ("write:repo_hook", &["read:repo_hook"]),
        ("admin:gpg_key", &["write:gpg_key", "read:gpg_key"]),
    ];
    requested.iter().all(|wanted| {
        granted.iter().any(|have| {
            have == wanted
                || IMPLIED
                    .iter()
                    .any(|(broad, narrow)| have == broad && narrow.contains(&wanted.as_str()))
        })
    })
}

fn split_scopes(scope: &str) -> Vec<String> {
    scope
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|scope| !scope.is_empty())
        .map(str::to_owned)
        .collect()
}

#[derive(Clone, Debug)]
struct Provider {
    label: String,
}

/// An extension's `getSession` call being served.
#[derive(Clone, Debug)]
pub(super) struct Request {
    reply: u64,
    provider: String,
    scopes: Vec<String>,
    extension_id: String,
    extension_name: String,
    create_if_none: bool,
    force_new: bool,
    silent: bool,
    /// The account the extension asked for, by id or label.
    account: Option<String>,
    deadline: Instant,
}

impl Request {
    fn parse(reply: u64, args: &[Value]) -> Result<Self> {
        let scopes = args
            .get(1)
            .and_then(Value::as_array)
            .ok_or("Authentication requests by challenge are not supported")?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let options = args.get(4).unwrap_or(&Value::Null);
        let flag = |name: &str| {
            let value = &options[name];
            value.as_bool().unwrap_or(false) || value.is_object()
        };
        let extension_id = args.get(2).and_then(Value::as_str).unwrap_or_default();
        Ok(Self {
            reply,
            provider: args
                .first()
                .and_then(Value::as_str)
                .ok_or("Invalid authentication provider")?
                .to_owned(),
            scopes,
            extension_id: extension_id.to_owned(),
            extension_name: args
                .get(3)
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or(extension_id)
                .to_owned(),
            create_if_none: flag("createIfNone"),
            force_new: flag("forceNewSession"),
            silent: flag("silent"),
            account: options["account"]["id"]
                .as_str()
                .or_else(|| options["account"]["label"].as_str())
                .map(str::to_owned),
            deadline: Instant::now() + REQUEST_TIMEOUT,
        })
    }
}

/// What a call to an extension's provider is for.
#[derive(Clone, Debug)]
enum Op {
    /// `$getSessions` for a `getSession` call.
    Sessions(Request),
    /// `$createSession` for a `getSession` call.
    Create(Request),
    /// `$getSessions` for `$getAccounts`.
    Accounts { reply: u64 },
    /// A call whose result only completes `reply`.
    Done { reply: u64 },
}

/// A question the user has to answer before a session is released or created.
#[derive(Clone, Debug)]
enum Consent {
    /// Let the extension use an existing account.
    Access { request: Request, session: Value },
    /// Let the extension sign in (create a session).
    SignIn { request: Request },
}

impl Consent {
    fn request(&self) -> &Request {
        match self {
            Self::Access { request, .. } | Self::SignIn { request } => request,
        }
    }
}

#[derive(Default)]
pub(super) struct Auth {
    next_id: u64,
    providers: BTreeMap<String, Provider>,
    pending: BTreeMap<u64, (Op, Instant)>,
    /// Calls waiting for their provider to register.
    waiting: Vec<Request>,
    consents: VecDeque<Consent>,
    /// The consent the open prompt asks about.
    active: Option<Consent>,
    /// Calls waiting for the GitHub sign-in to finish.
    github_waiters: Vec<Request>,
    access: Option<AccessStore>,
}

impl Auth {
    pub(super) fn busy(&self) -> bool {
        !self.pending.is_empty() || self.active.is_some() || !self.consents.is_empty()
    }

    #[cfg(test)]
    pub(super) fn expire_waiting_for_test(&mut self) {
        for request in &mut self.waiting {
            request.deadline = Instant::now();
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = AUTH_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (AUTH_END - AUTH_BASE);
        id
    }

    fn access(&mut self, profile: &Path) -> &mut AccessStore {
        self.access
            .get_or_insert_with(|| AccessStore::load(profile))
    }
}

fn session_account(session: &Value) -> Option<(String, String)> {
    let id = session["account"]["id"].as_str()?;
    let label = session["account"]["label"].as_str().unwrap_or(id);
    Some((id.to_owned(), label.to_owned()))
}

fn github_session_json(session: &graphite_auth::Session, scopes: &[String]) -> Value {
    json!({
        "id": format!("github-{}", session.login),
        "accessToken": session.token,
        "account": { "id": session.login, "label": session.login },
        "scopes": scopes,
    })
}

impl App {
    pub(super) fn handle_vsix_auth_request(
        &mut self,
        runtime: &mut Runtime,
        id: u64,
        method: &str,
        args: &[Value],
    ) -> Result<Option<Value>> {
        let text = |index: usize| args.get(index).and_then(Value::as_str);
        match method {
            "$registerAuthenticationProvider" => {
                let details = args.first().unwrap_or(&Value::Null);
                let provider = details["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .ok_or("Invalid authentication provider")?;
                if provider == GITHUB {
                    return Err("GitHub sign-in is provided by Graphite".into());
                }
                runtime.auth.providers.insert(
                    provider.to_owned(),
                    Provider {
                        label: details["label"].as_str().unwrap_or(provider).to_owned(),
                    },
                );
                // Calls that waited for this provider can run now.
                let waiting: Vec<Request> = runtime
                    .auth
                    .waiting
                    .iter()
                    .filter(|request| request.provider == provider)
                    .cloned()
                    .collect();
                runtime
                    .auth
                    .waiting
                    .retain(|request| request.provider != provider);
                for request in waiting {
                    self.start_auth_lookup(runtime, request);
                }
                Ok(Some(Value::Null))
            }
            "$unregisterAuthenticationProvider" => {
                if let Some(provider) = text(0) {
                    runtime.auth.providers.remove(provider);
                    runtime
                        .auth
                        .consents
                        .retain(|consent| consent.request().provider != provider);
                }
                Ok(Some(Value::Null))
            }
            "$ensureProvider" => {
                if let Some(provider) = text(0) {
                    self.activate_vsix_event(
                        runtime,
                        &format!("onAuthenticationRequest:{provider}"),
                    );
                }
                Ok(Some(Value::Null))
            }
            "$sendDidChangeSessions" => {
                let provider = text(0).ok_or("Invalid authentication provider")?;
                let label = runtime
                    .auth
                    .providers
                    .get(provider)
                    .map_or_else(|| provider.to_owned(), |provider| provider.label.clone());
                self.notify_auth_sessions_changed(runtime, provider, &label);
                Ok(Some(Value::Null))
            }
            "$getSession" => {
                let request = Request::parse(id, args)?;
                self.start_auth_request(runtime, request);
                Ok(None)
            }
            "$getAccounts" => {
                let provider = text(0).ok_or("Invalid authentication provider")?;
                if provider == GITHUB {
                    let accounts = self
                        .github_session
                        .iter()
                        .map(|session| json!({"id": session.login, "label": session.login}))
                        .collect::<Vec<_>>();
                    return Ok(Some(json!(accounts)));
                }
                if !runtime.auth.providers.contains_key(provider) {
                    return Ok(Some(json!([])));
                }
                self.auth_call(
                    runtime,
                    provider,
                    "$getSessions",
                    vec![Value::Null, json!({})],
                    Op::Accounts { reply: id },
                )?;
                Ok(None)
            }
            "$removeSession" => {
                let provider = text(0).ok_or("Invalid authentication provider")?;
                let session = text(1).ok_or("Invalid session")?;
                if provider == GITHUB {
                    let account = self
                        .github_session
                        .as_ref()
                        .map(|session| session.login.clone());
                    if account.as_deref().map(|login| format!("github-{login}"))
                        == Some(session.to_owned())
                    {
                        runtime
                            .auth
                            .access(&self.profile_root)
                            .revoke_account(GITHUB, account.as_deref().unwrap_or_default());
                        let _ = runtime
                            .auth
                            .access(&self.profile_root)
                            .save(&self.profile_root);
                        self.sign_out_github();
                    }
                    return Ok(Some(Value::Null));
                }
                self.auth_call(
                    runtime,
                    provider,
                    "$removeSession",
                    vec![json!(session)],
                    Op::Done { reply: id },
                )?;
                Ok(None)
            }
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadAuthentication", method);
                Err(format!(
                    "Graphite does not support MainThreadAuthentication.{method}"
                ))
            }
        }
    }

    /// Sends the activation event once per session.
    fn activate_vsix_event(&mut self, runtime: &mut Runtime, event: &str) {
        if runtime.activated_events.insert(event.to_owned()) {
            let _ = runtime
                .session
                .send(json!({"type": "activateByEvent", "event": event}));
        }
    }

    fn reply_auth(runtime: &mut Runtime, reply: u64, result: std::result::Result<Value, String>) {
        let message = match result {
            Ok(result) => json!({"type": "reply", "id": reply, "result": result}),
            Err(error) => json!({"type": "reply", "id": reply, "error": error}),
        };
        let _ = runtime.session.send(message);
    }

    /// Calls a method of the extension that registered `provider` and remembers what it is for.
    fn auth_call(
        &mut self,
        runtime: &mut Runtime,
        provider: &str,
        method: &str,
        extra: Vec<Value>,
        op: Op,
    ) -> Result<u64> {
        let id = runtime.auth.next_id();
        let mut args = vec![json!(provider)];
        args.extend(extra);
        runtime
            .auth
            .pending
            .insert(id, (op, Instant::now() + REQUEST_TIMEOUT));
        let sent = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostAuthentication",
            "method": method,
            "args": args,
        }));
        if let Err(error) = sent {
            runtime.auth.pending.remove(&id);
            return Err(error);
        }
        Ok(id)
    }

    fn start_auth_request(&mut self, runtime: &mut Runtime, request: Request) {
        if request.provider == GITHUB {
            self.serve_github_request(runtime, request);
        } else if runtime.auth.providers.contains_key(&request.provider) {
            self.start_auth_lookup(runtime, request);
        } else if request.silent {
            Self::reply_auth(runtime, request.reply, Ok(Value::Null));
        } else {
            // The extension that provides it may not have activated yet.
            let event = format!("onAuthenticationRequest:{}", request.provider);
            self.activate_vsix_event(runtime, &event);
            let mut request = request;
            request.deadline = Instant::now() + PROVIDER_WAIT;
            runtime.auth.waiting.push(request);
        }
    }

    /// Looks for an existing session of an extension's provider.
    fn start_auth_lookup(&mut self, runtime: &mut Runtime, request: Request) {
        if request.force_new {
            self.ask_sign_in(runtime, request);
            return;
        }
        let options = match &request.account {
            Some(account) => json!({"account": {"id": account, "label": account}}),
            None => json!({}),
        };
        let provider = request.provider.clone();
        let reply = request.reply;
        let scopes = json!(request.scopes);
        if let Err(error) = self.auth_call(
            runtime,
            &provider,
            "$getSessions",
            vec![scopes, options],
            Op::Sessions(request),
        ) {
            Self::reply_auth(runtime, reply, Err(error));
        }
    }

    /// Releases `session` to the extension, asking the user first when it was not allowed yet.
    fn grant_or_ask(&mut self, runtime: &mut Runtime, request: Request, session: Value) {
        let Some((account, _)) = session_account(&session) else {
            Self::reply_auth(
                runtime,
                request.reply,
                Err("The provider returned an invalid session".into()),
            );
            return;
        };
        let allowed = runtime.auth.access(&self.profile_root).is_allowed(
            &request.provider,
            &account,
            &request.extension_id,
        );
        if allowed {
            Self::reply_auth(runtime, request.reply, Ok(session));
        } else if request.silent {
            // A silent request never asks; the extension has no access yet.
            Self::reply_auth(runtime, request.reply, Ok(Value::Null));
        } else {
            runtime
                .auth
                .consents
                .push_back(Consent::Access { request, session });
        }
    }

    fn ask_sign_in(&mut self, runtime: &mut Runtime, request: Request) {
        if request.silent || !(request.create_if_none || request.force_new) {
            Self::reply_auth(runtime, request.reply, Ok(Value::Null));
        } else {
            runtime.auth.consents.push_back(Consent::SignIn { request });
        }
    }

    /// Records that a newly created session is allowed for the extension that asked, then answers.
    fn grant_created(&mut self, runtime: &mut Runtime, request: Request, session: Value) {
        if let Some((account, _)) = session_account(&session) {
            let store = runtime.auth.access(&self.profile_root);
            store.allow(&request.provider, &account, &request.extension_id);
            let _ = store.save(&self.profile_root);
        }
        Self::reply_auth(runtime, request.reply, Ok(session));
    }

    fn serve_github_request(&mut self, runtime: &mut Runtime, request: Request) {
        let granted = self
            .github_session
            .as_ref()
            .map(|session| split_scopes(&session.scope))
            .unwrap_or_default();
        let session = self.github_session.clone().filter(|session| {
            !request.force_new
                && request
                    .account
                    .as_ref()
                    .is_none_or(|account| *account == session.login)
                && scopes_cover(&granted, &request.scopes)
        });
        match session {
            Some(session) => {
                let json = github_session_json(&session, &request.scopes);
                self.grant_or_ask(runtime, request, json);
            }
            None => self.ask_sign_in(runtime, request),
        }
    }

    /// Opens the next consent prompt when none is open.
    pub(super) fn drive_vsix_auth(&mut self, runtime: &mut Runtime) {
        let now = Instant::now();
        runtime
            .auth
            .pending
            .retain(|_, (_, deadline)| *deadline > now);
        let expired: Vec<Request> = runtime
            .auth
            .waiting
            .iter()
            .filter(|request| request.deadline <= now)
            .cloned()
            .collect();
        runtime
            .auth
            .waiting
            .retain(|request| request.deadline > now);
        for request in expired {
            let message = format!(
                "No authentication provider '{}' is currently registered.",
                request.provider
            );
            Self::reply_auth(runtime, request.reply, Err(message));
        }
        if runtime.prompt.is_some() || runtime.auth.active.is_some() {
            return;
        }
        let Some(consent) = runtime.auth.consents.pop_front() else {
            return;
        };
        let request = consent.request();
        let provider = runtime.auth.providers.get(&request.provider).map_or_else(
            || GITHUB_LABEL.to_owned(),
            |provider| provider.label.clone(),
        );
        let message = match &consent {
            Consent::Access { session, .. } => self.tf(
                "auth.consent.access",
                &[
                    ("extension", &request.extension_name),
                    ("provider", &provider),
                    (
                        "account",
                        &session_account(session)
                            .map(|(_, label)| label)
                            .unwrap_or_default(),
                    ),
                ],
            ),
            Consent::SignIn { .. } => self.tf(
                "auth.consent.signin",
                &[
                    ("extension", &request.extension_name),
                    ("provider", &provider),
                ],
            ),
        };
        runtime.auth.active = Some(consent);
        self.open_vsix_consent_prompt(runtime, message);
    }

    /// The user answered the open consent prompt.
    pub(crate) fn resolve_auth_consent(&mut self, allowed: bool) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        if let Some(consent) = runtime.auth.active.take() {
            let request = consent.request().clone();
            if !allowed {
                Self::reply_auth(
                    &mut runtime,
                    request.reply,
                    Err("User did not consent to login.".into()),
                );
            } else {
                match consent {
                    Consent::Access { request, session } => {
                        if let Some((account, _)) = session_account(&session) {
                            let store = runtime.auth.access(&self.profile_root);
                            store.allow(&request.provider, &account, &request.extension_id);
                            let _ = store.save(&self.profile_root);
                        }
                        Self::reply_auth(&mut runtime, request.reply, Ok(session));
                    }
                    Consent::SignIn { request } if request.provider == GITHUB => {
                        let mut scopes = self
                            .github_session
                            .as_ref()
                            .map(|session| split_scopes(&session.scope))
                            .unwrap_or_default();
                        for scope in &request.scopes {
                            if !scopes.contains(scope) {
                                scopes.push(scope.clone());
                            }
                        }
                        if scopes.is_empty() {
                            scopes = split_scopes(graphite_auth::SCOPE);
                        }
                        if self.github_sign_in_pending {
                            runtime.auth.github_waiters.push(request);
                        } else {
                            self.start_github_sign_in_with_scope(&scopes.join(" "));
                            if self.github_sign_in_pending {
                                runtime.auth.github_waiters.push(request);
                            } else {
                                let error = self.status.clone();
                                Self::reply_auth(&mut runtime, request.reply, Err(error));
                            }
                        }
                    }
                    Consent::SignIn { request } => {
                        let provider = request.provider.clone();
                        let reply = request.reply;
                        let options = match &request.account {
                            Some(account) => json!({"account": {"id": account, "label": account}}),
                            None => json!({}),
                        };
                        let scopes = json!(request.scopes);
                        if let Err(error) = self.auth_call(
                            &mut runtime,
                            &provider,
                            "$createSession",
                            vec![scopes, options],
                            Op::Create(request),
                        ) {
                            Self::reply_auth(&mut runtime, reply, Err(error));
                        }
                    }
                }
            }
        }
        self.plugins.vsix = Some(runtime);
    }

    pub(super) fn finish_vsix_auth(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some((op, _)) = runtime.auth.pending.remove(&id) else {
            return;
        };
        let error = event["error"].as_str().map(str::to_owned);
        let result = &event["result"];
        match op {
            Op::Done { reply } => {
                Self::reply_auth(runtime, reply, error.map_or(Ok(Value::Null), Err))
            }
            Op::Accounts { reply } => {
                let mut accounts: Vec<Value> = Vec::new();
                for session in result.as_array().into_iter().flatten() {
                    if let Some((id, label)) = session_account(session) {
                        if !accounts.iter().any(|account| account["id"] == json!(id)) {
                            accounts.push(json!({"id": id, "label": label}));
                        }
                    }
                }
                Self::reply_auth(runtime, reply, error.map_or(Ok(json!(accounts)), Err));
            }
            Op::Sessions(request) => {
                if let Some(error) = error {
                    Self::reply_auth(runtime, request.reply, Err(error));
                    return;
                }
                let session = result
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|session| {
                        request.account.as_ref().is_none_or(|wanted| {
                            session_account(session)
                                .is_some_and(|(id, label)| id == *wanted || label == *wanted)
                        })
                    })
                    .cloned();
                match session {
                    Some(session) => self.grant_or_ask(runtime, request, session),
                    None => self.ask_sign_in(runtime, request),
                }
            }
            Op::Create(request) => match error {
                Some(error) => Self::reply_auth(runtime, request.reply, Err(error)),
                None if result.is_object() => self.grant_created(runtime, request, result.clone()),
                None => Self::reply_auth(
                    runtime,
                    request.reply,
                    Err("The provider created no session".into()),
                ),
            },
        }
    }

    /// Tells the extensions that the sessions of a provider changed.
    pub(super) fn notify_auth_sessions_changed(
        &mut self,
        runtime: &mut Runtime,
        provider: &str,
        label: &str,
    ) {
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostAuthentication",
            "method": "$onDidChangeAuthenticationSessions",
            "args": [provider, label],
        }));
    }

    /// The GitHub sign-in finished: answers the calls that waited for it and reports the change.
    pub(crate) fn finish_github_auth_requests(&mut self) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        let waiters = std::mem::take(&mut runtime.auth.github_waiters);
        for request in waiters {
            match self.github_session.clone() {
                Some(session) => {
                    let scopes = if request.scopes.is_empty() {
                        split_scopes(&session.scope)
                    } else {
                        request.scopes.clone()
                    };
                    let json = github_session_json(&session, &scopes);
                    self.grant_created(&mut runtime, request, json);
                }
                None => Self::reply_auth(
                    &mut runtime,
                    request.reply,
                    Err("GitHub sign-in did not complete".into()),
                ),
            }
        }
        self.notify_auth_sessions_changed(&mut runtime, GITHUB, GITHUB_LABEL);
        self.plugins.vsix = Some(runtime);
    }

    /// Reports a GitHub sign-out to the extensions.
    pub(crate) fn notify_github_signed_out(&mut self) {
        if let Some(mut runtime) = self.plugins.vsix.take() {
            self.notify_auth_sessions_changed(&mut runtime, GITHUB, GITHUB_LABEL);
            self.plugins.vsix = Some(runtime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(text: &str) -> Vec<String> {
        split_scopes(text)
    }

    #[test]
    fn broader_scopes_cover_the_narrower_ones_they_contain() {
        assert!(scopes_cover(&scopes("read:user"), &scopes("read:user")));
        assert!(scopes_cover(
            &scopes("user"),
            &scopes("read:user user:email")
        ));
        assert!(scopes_cover(
            &scopes("repo read:org"),
            &scopes("public_repo read:org")
        ));
        assert!(!scopes_cover(
            &scopes("read:user"),
            &scopes("read:user gist")
        ));
        assert!(!scopes_cover(&scopes("read:user"), &scopes("user")));
        assert!(!scopes_cover(&[], &scopes("gist")));
        assert!(scopes_cover(&[], &[]));
        assert_eq!(scopes("a, b  c"), ["a", "b", "c"]);
    }

    #[test]
    fn consent_is_remembered_per_account_and_extension_and_can_be_revoked() {
        let dir = crate::test_support::TempDir::new();
        let mut store = AccessStore::load(dir.path());
        assert!(!store.is_allowed("github", "octocat", "pub.ext"));
        store.allow("github", "octocat", "pub.ext");
        store.save(dir.path()).unwrap();
        let reloaded = AccessStore::load(dir.path());
        assert!(reloaded.is_allowed("github", "octocat", "pub.ext"));
        assert!(!reloaded.is_allowed("github", "octocat", "other.ext"));
        assert!(!reloaded.is_allowed("github", "hubot", "pub.ext"));
        assert!(!reloaded.is_allowed("other", "octocat", "pub.ext"));
        let mut store = reloaded;
        store.revoke_account("github", "octocat");
        assert!(!store.is_allowed("github", "octocat", "pub.ext"));
        // A damaged file is treated as no consent at all.
        std::fs::write(dir.path().join("code-oss").join(ACCESS_FILE), b"{not json").unwrap();
        assert!(!AccessStore::load(dir.path()).is_allowed("github", "octocat", "pub.ext"));
    }

    #[test]
    fn requests_read_options_and_reject_challenge_requests() {
        let args = vec![
            json!("sample"),
            json!(["a", "b"]),
            json!("pub.ext"),
            json!("Sample Ext"),
            json!({"createIfNone": {"detail": "why"}, "silent": false, "account": {"id": "u1", "label": "User"}}),
        ];
        let request = Request::parse(5, &args).unwrap();
        assert_eq!(request.scopes, ["a", "b"]);
        assert!(request.create_if_none && !request.silent && !request.force_new);
        assert_eq!(request.account.as_deref(), Some("u1"));
        assert_eq!(request.extension_name, "Sample Ext");
        let challenge = vec![
            json!("sample"),
            json!({"wwwAuthenticate": "Bearer"}),
            json!("e"),
            json!("E"),
            json!({}),
        ];
        assert!(Request::parse(5, &challenge).is_err());
    }

    #[test]
    fn github_sessions_expose_the_token_and_account() {
        let session = graphite_auth::Session {
            token: "gho_x".into(),
            login: "octocat".into(),
            scope: "read:user".into(),
            ..Default::default()
        };
        let json = github_session_json(&session, &scopes("read:user"));
        assert_eq!(json["accessToken"], "gho_x");
        assert_eq!(
            json["account"],
            json!({"id": "octocat", "label": "octocat"})
        );
        assert_eq!(
            session_account(&json),
            Some(("octocat".into(), "octocat".into()))
        );
    }
}
