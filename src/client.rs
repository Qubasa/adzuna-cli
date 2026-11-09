use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;

use crate::config::Credentials;

pub const BASE_URL: &str = "https://api.adzuna.com/v1/api";
const TIMEOUT: Duration = Duration::from_secs(30);
const BODY_SNIPPET_CHARS: usize = 200;

pub type Query = Vec<(&'static str, String)>;

pub struct Client {
    agent: ureq::Agent,
    creds: Credentials,
}

impl Client {
    pub fn new(creds: Credentials) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .user_agent(concat!("adzuna-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self { agent, creds }
    }

    /// GETs `path` below [`BASE_URL`] and returns the decoded JSON body.
    pub fn get(&self, path: &str, query: &Query) -> Result<Value> {
        let key = self.creds.app_key.expose();
        let mut request = self
            .agent
            .get(format!("{BASE_URL}/{path}"))
            .header("Accept", "application/json")
            .query("app_id", &self.creds.app_id)
            .query("app_key", key);
        for (name, value) in query {
            request = request.query(*name, value);
        }
        let mut response = request.call().map_err(|err| {
            anyhow!(
                "request to /{path} failed: {}",
                redact(&err.to_string(), key)
            )
        })?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().map_err(|err| {
            anyhow!(
                "reading /{path} response: {}",
                redact(&err.to_string(), key)
            )
        })?;
        if status != 200 {
            return Err(anyhow!(
                "/{path}: {}",
                redact(&describe_error(status, &body), key)
            ));
        }
        serde_json::from_str(&body).map_err(|err| anyhow!("/{path} returned invalid JSON: {err}"))
    }
}

/// Error body documented as `Exception` in the Adzuna `OpenAPI` spec.
#[derive(Deserialize)]
struct ApiException {
    display: Option<String>,
    exception: Option<String>,
}

/// Turns a non-200 response into one line, falling back to a body snippet for non-JSON pages.
fn describe_error(status: u16, body: &str) -> String {
    if let Ok(ApiException { display, exception }) = serde_json::from_str(body) {
        if let Some(detail) = display.or(exception) {
            return format!("HTTP {status}: {detail}");
        }
    }
    let snippet: String = body.trim().chars().take(BODY_SNIPPET_CHARS).collect();
    if snippet.is_empty() {
        format!("HTTP {status} with empty body")
    } else {
        format!("HTTP {status}: {snippet}")
    }
}

fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        return text.to_owned();
    }
    text.replace(secret, "<redacted>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_error_uses_display_then_exception() {
        let body = r#"{"display":"Invalid country","exception":"BAD_REQUEST","doc":"x"}"#;
        assert_eq!(describe_error(400, body), "HTTP 400: Invalid country");
        let body = r#"{"exception":"AUTH_FAIL"}"#;
        assert_eq!(describe_error(401, body), "HTTP 401: AUTH_FAIL");
    }

    #[test]
    fn html_error_page_is_truncated_not_a_parse_failure() {
        let body = format!("<html>{}</html>", "é".repeat(500));
        let message = describe_error(502, &body);
        assert!(message.starts_with("HTTP 502: <html>"));
        assert_eq!(
            message.chars().count(),
            "HTTP 502: ".len() + BODY_SNIPPET_CHARS
        );
        assert_eq!(describe_error(503, "  \n"), "HTTP 503 with empty body");
    }

    #[test]
    fn json_without_message_fields_falls_back_to_body() {
        assert_eq!(describe_error(500, "{}"), "HTTP 500: {}");
    }

    #[test]
    fn redact_hides_every_occurrence_and_ignores_empty_secret() {
        let url = "https://x/?app_id=1&app_key=abc123&q=abc123";
        assert_eq!(
            redact(url, "abc123"),
            "https://x/?app_id=1&app_key=<redacted>&q=<redacted>"
        );
        assert_eq!(redact("text", ""), "text");
    }
}
