use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use ureq::http::StatusCode;
use ureq::tls::{RootCerts, TlsConfig};

use crate::config::Credentials;

pub const BASE_URL: &str = "https://api.adzuna.com/v1/api";
/// Longer than the ~30 s after which Adzuna's nginx gives up, so its 504 reaches us.
const TIMEOUT: Duration = Duration::from_secs(60);
const BODY_SNIPPET_CHARS: usize = 200;
/// Waits before the second and third attempt. Each retry costs one more request of quota.
const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(15)];

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
    ///
    /// Adzuna intermittently answers 503, or 504 when a slow query outlives its gateway, and
    /// the same request then succeeds later. Those failures and client timeouts are retried
    /// after [`RETRY_DELAYS`], with a note on stderr so the wait is visible.
    pub fn get(&self, path: &str, query: &Query) -> Result<Value> {
        let mut attempts = 1;
        loop {
            let failure = match self.send(path, query) {
                Ok(value) => return Ok(value),
                Err(failure) => failure,
            };
            let message = redact(&failure.to_string(), self.creds.app_key.expose());
            let label = request_label(path, query);
            let Some(delay) = retry_delay(attempts, &failure) else {
                if attempts == 1 {
                    return Err(anyhow!("{label}: {message}"));
                }
                return Err(anyhow!("{label}: {message} (after {attempts} attempts)"));
            };
            // A lost stderr must not abort a request that may still succeed.
            let _ = writeln!(
                io::stderr(),
                "{label}: {message}, retrying in {}s",
                delay.as_secs()
            );
            thread::sleep(delay);
            attempts += 1;
        }
    }

    fn send(&self, path: &str, query: &Query) -> Result<Value, Failure> {
        let mut request = self
            .agent
            .get(format!("{BASE_URL}/{path}"))
            .header("Accept", "application/json")
            .query("app_id", &self.creds.app_id)
            .query("app_key", self.creds.app_key.expose());
        for (name, value) in query {
            request = request.query(*name, value);
        }
        let mut response = request.call().map_err(Failure::Transport)?;
        let status = response.status();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(Failure::Transport)?;
        if status != StatusCode::OK {
            return Err(Failure::Status { status, body });
        }
        serde_json::from_str(&body).map_err(Failure::InvalidJson)
    }
}

enum Failure {
    Status { status: StatusCode, body: String },
    Transport(ureq::Error),
    InvalidJson(serde_json::Error),
}

impl Failure {
    fn is_transient(&self) -> bool {
        match self {
            Self::Status { status, .. } => {
                *status == StatusCode::SERVICE_UNAVAILABLE || *status == StatusCode::GATEWAY_TIMEOUT
            }
            Self::Transport(err) => matches_timeout(err),
            Self::InvalidJson(_) => false,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status { status, body } => f.write_str(&describe_error(*status, body)),
            Self::Transport(err) => write!(f, "{err}"),
            Self::InvalidJson(err) => write!(f, "invalid JSON: {err}"),
        }
    }
}

fn matches_timeout(err: &ureq::Error) -> bool {
    let ureq::Error::Timeout(_) = err else {
        return false;
    };
    true
}

/// Delay before the next attempt, or `None` when `failure` is permanent or retries are used up.
fn retry_delay(attempts_made: usize, failure: &Failure) -> Option<Duration> {
    if !failure.is_transient() {
        return None;
    }
    RETRY_DELAYS.get(attempts_made.checked_sub(1)?).copied()
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

    fn http_failure(code: u16) -> Failure {
        Failure::Status {
            status: status(code),
            body: "<!DOCTYPE html>".to_owned(),
        }
    }

    #[test]
    fn unavailable_gateway_timeout_and_timeouts_retry_twice_then_give_up() {
        for code in [503, 504] {
            let failure = http_failure(code);
            assert_eq!(retry_delay(1, &failure), Some(Duration::from_secs(5)));
            assert_eq!(retry_delay(2, &failure), Some(Duration::from_secs(15)));
            assert_eq!(retry_delay(3, &failure), None);
        }
        let timeout = Failure::Transport(ureq::Error::Timeout(ureq::Timeout::Global));
        assert_eq!(retry_delay(1, &timeout), Some(Duration::from_secs(5)));
        assert_eq!(retry_delay(3, &timeout), None);
    }

    #[test]
    fn caller_errors_and_other_failures_are_not_retried() {
        for code in [400, 401, 404, 410, 429, 500, 502] {
            assert_eq!(retry_delay(1, &http_failure(code)), None, "HTTP {code}");
        }
        let refused = Failure::Transport(ureq::Error::Io(std::io::Error::other("refused")));
        assert_eq!(retry_delay(1, &refused), None);
    }
}
