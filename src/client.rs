use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use ureq::http::StatusCode;
use ureq::tls::{RootCerts, TlsConfig};

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
        // The OS trust store honours SSL_CERT_FILE, so a TLS-intercepting proxy's CA is trusted.
        let tls = TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .user_agent(concat!("adzuna-cli/", env!("CARGO_PKG_VERSION")))
            .tls_config(tls)
            .build()
            .into();
        Self { agent, creds }
    }

    /// GETs `path` below [`BASE_URL`] and returns the decoded JSON body.
    pub fn get(&self, path: &str, query: &Query) -> Result<Value> {
        let key = self.creds.app_key.expose();
        let label = request_label(path, query);
        let mut request = self
            .agent
            .get(format!("{BASE_URL}/{path}"))
            .header("Accept", "application/json")
            .query("app_id", &self.creds.app_id)
            .query("app_key", key);
        for (name, value) in query {
            request = request.query(*name, value);
        }
        let mut response = request
            .call()
            .map_err(|err| anyhow!("{label}: {}", redact(&err.to_string(), key)))?;
        let status = response.status();
        let body = response.body_mut().read_to_string().map_err(|err| {
            anyhow!(
                "{label}: reading response: {}",
                redact(&err.to_string(), key)
            )
        })?;
        if status != StatusCode::OK {
            return Err(anyhow!(
                "{label}: {}",
                redact(&describe_error(status, &body), key)
            ));
        }
        serde_json::from_str(&body).map_err(|err| anyhow!("{label}: invalid JSON: {err}"))
    }
}

/// `GET /path?name=value&...` for error messages. Credentials are not part of `query`.
fn request_label(path: &str, query: &Query) -> String {
    let mut label = format!("GET /{path}");
    for (index, (name, value)) in query.iter().enumerate() {
        label.push(if index == 0 { '?' } else { '&' });
        label.push_str(name);
        label.push('=');
        label.push_str(value);
    }
    label
}

/// Error body documented as `Exception` in the Adzuna `OpenAPI` spec.
#[derive(Deserialize)]
struct ApiException {
    display: Option<String>,
    exception: Option<String>,
}

/// One line for a non-200 response. HTML error pages are dropped because they carry no detail.
fn describe_error(status: StatusCode, body: &str) -> String {
    if let Ok(ApiException { display, exception }) = serde_json::from_str(body) {
        if let Some(detail) = display.or(exception) {
            return format!("HTTP {status}: {detail}");
        }
    }
    let body = body.trim();
    if body.is_empty() || body.starts_with('<') {
        return format!("HTTP {status}");
    }
    let first_line = body.lines().next().unwrap_or_default();
    let snippet: String = first_line.chars().take(BODY_SNIPPET_CHARS).collect();
    format!("HTTP {status}: {snippet}")
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

    fn status(code: u16) -> StatusCode {
        StatusCode::from_u16(code).unwrap_or(StatusCode::IM_A_TEAPOT)
    }

    #[test]
    fn json_error_uses_display_then_exception() {
        let body = r#"{"display":"Invalid country","exception":"BAD_REQUEST","doc":"x"}"#;
        assert_eq!(
            describe_error(status(400), body),
            "HTTP 400 Bad Request: Invalid country"
        );
        let body = r#"{"exception":"AUTH_FAIL"}"#;
        assert_eq!(
            describe_error(status(401), body),
            "HTTP 401 Unauthorized: AUTH_FAIL"
        );
    }

    #[test]
    fn html_error_page_is_reduced_to_the_status() {
        let body = "\n<!DOCTYPE html><html><body>Chef ...</body></html>";
        assert_eq!(describe_error(status(400), body), "HTTP 400 Bad Request");
        assert_eq!(
            describe_error(status(503), "  \n"),
            "HTTP 503 Service Unavailable"
        );
    }

    #[test]
    fn plain_text_error_keeps_a_bounded_first_line() {
        let body = format!("{}\nsecond line", "é".repeat(500));
        let message = describe_error(status(500), &body);
        let prefix = "HTTP 500 Internal Server Error: ";
        assert!(message.starts_with(prefix));
        assert_eq!(
            message.chars().count(),
            prefix.chars().count() + BODY_SNIPPET_CHARS
        );
        assert_eq!(
            describe_error(status(500), "{}"),
            "HTTP 500 Internal Server Error: {}"
        );
    }

    #[test]
    fn request_label_names_path_and_parameters() {
        let query = vec![
            ("location0", "UK".to_owned()),
            ("category", "nope-jobs".to_owned()),
        ];
        assert_eq!(
            request_label("jobs/gb/history", &query),
            "GET /jobs/gb/history?location0=UK&category=nope-jobs"
        );
        assert_eq!(request_label("version", &Query::new()), "GET /version");
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
