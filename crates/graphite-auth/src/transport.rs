use serde_json::Value;
use std::time::Duration;

/// HTTP access used by the sign-in flow. The app uses [`ReqwestTransport`]; tests use a fake.
pub trait Transport {
    /// POSTs a form and returns the JSON response body.
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, String>;
    /// GETs a JSON resource with a bearer token.
    fn get_json(&self, url: &str, token: &str) -> Result<Value, String>;
}

/// Blocking HTTPS transport. Sign-in runs on a background thread, so blocking is acceptable.
pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Graphite")
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }

    fn json(response: reqwest::blocking::Response) -> Result<Value, String> {
        let status = response.status();
        let body = response.text().map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|_| format!("GitHub returned an unexpected response ({status})"))?;
        Ok(value)
    }
}

impl Transport for ReqwestTransport {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        let response = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .form(form)
            .send()
            .map_err(|error| error.to_string())?;
        Self::json(response)
    }

    fn get_json(&self, url: &str, token: &str) -> Result<Value, String> {
        let response = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .bearer_auth(token)
            .send()
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("GitHub rejected the request ({})", response.status()));
        }
        Self::json(response)
    }
}
