//! The HTTP seam. Nothing here parses JavaScript.
//!
//! [`HttpClient`] is the only thing the collection flow knows about the
//! network, so tests can substitute an in-memory client or point the real
//! [`ReqwestClient`] at a loopback server.

use std::io::Read;
use std::time::Duration;

use url::Url;

use crate::seeds::error::{Error, Result, TransportKind};

/// Request headers that carry credentials; never forwarded across origins.
const CREDENTIAL_HEADERS: [&str; 3] = ["cookie", "authorization", "proxy-authorization"];

/// Ordered request headers. Names compare case-insensitively and setting a
/// name again replaces the earlier value, so later sources override defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse one `name: value` line, as given to `-H`.
    pub fn parse_line(line: &str) -> Result<(String, String)> {
        let invalid = |reason| Error::InvalidHeader {
            input: line.to_owned(),
            reason,
        };
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid("expected `name: value`"))?;
        let (name, value) = (name.trim(), value.trim());
        let token = |byte: u8| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte);
        if name.is_empty() || !name.bytes().all(token) {
            return Err(invalid("the name is not a valid header token"));
        }
        if value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(invalid("the value contains control characters"));
        }
        Ok((name.to_owned(), value.to_owned()))
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.0
            .retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
        self.0.push((name.to_owned(), value.to_owned()));
    }

    pub fn set_line(&mut self, line: &str) -> Result<()> {
        let (name, value) = Self::parse_line(line)?;
        self.set(&name, &value);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// A copy without cookie/authorization headers.
    pub fn without_credentials(&self) -> Self {
        Self(
            self.0
                .iter()
                .filter(|(name, _)| {
                    !CREDENTIAL_HEADERS
                        .iter()
                        .any(|c| name.eq_ignore_ascii_case(c))
                })
                .cloned()
                .collect(),
        )
    }
}

/// A completed exchange. Non-2xx statuses are returned, not turned into
/// errors, so the caller can say which half of a pair failed.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    /// URL after redirects.
    pub final_url: Url,
    pub content_type: Option<String>,
    /// Raw `Set-Cookie` header values, in response order. Empty when the
    /// response set no cookies.
    pub set_cookie: Vec<String>,
    /// Body exactly as served, after transfer/content decoding.
    pub body: Vec<u8>,
}

pub trait HttpClient {
    fn get(&self, url: &Url, headers: &Headers) -> Result<HttpResponse>;
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Whole-request deadline, body included.
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub max_body_bytes: u64,
    pub max_redirects: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_body_bytes: 32 * 1024 * 1024,
            max_redirects: 5,
        }
    }
}

/// Blocking reqwest client. It keeps no cookie jar and ignores proxy
/// environment variables, so the only request state is what the caller passes.
pub struct ReqwestClient {
    inner: reqwest::blocking::Client,
    max_body_bytes: u64,
}

impl ReqwestClient {
    pub fn new(config: &ClientConfig) -> Result<Self> {
        let inner = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects))
            .no_proxy()
            .build()
            .map_err(|error| transport("<client>", &error))?;
        Ok(Self {
            inner,
            max_body_bytes: config.max_body_bytes,
        })
    }
}

impl HttpClient for ReqwestClient {
    fn get(&self, url: &Url, headers: &Headers) -> Result<HttpResponse> {
        let mut request = self.inner.get(url.clone());
        for (name, value) in headers.iter() {
            request = request.header(name, value);
        }
        let mut response = request
            .send()
            .map_err(|error| transport(url.as_str(), &error))?;
        let status = response.status().as_u16();
        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let set_cookie = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok().map(str::to_owned))
            .collect();

        let mut body = Vec::new();
        response
            .by_ref()
            .take(self.max_body_bytes + 1)
            .read_to_end(&mut body)
            .map_err(|error| Error::Transport {
                url: url.to_string(),
                kind: match error.kind() {
                    std::io::ErrorKind::TimedOut => TransportKind::Timeout,
                    _ => match error
                        .get_ref()
                        .and_then(|e| e.downcast_ref::<reqwest::Error>())
                    {
                        Some(inner) if inner.is_timeout() => TransportKind::Timeout,
                        _ => TransportKind::Other,
                    },
                },
                message: format!("reading body: {error}"),
            })?;
        if body.len() as u64 > self.max_body_bytes {
            return Err(Error::BodyTooLarge {
                url: url.to_string(),
                limit: self.max_body_bytes,
            });
        }
        Ok(HttpResponse {
            status,
            final_url,
            content_type,
            set_cookie,
            body,
        })
    }
}

fn transport(url: &str, error: &reqwest::Error) -> Error {
    let kind = if error.is_timeout() {
        TransportKind::Timeout
    } else if error.is_connect() {
        TransportKind::Connect
    } else if error.is_redirect() {
        TransportKind::Redirect
    } else {
        TransportKind::Other
    };
    // reqwest's Display stops at the top of the chain; include the causes.
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    Error::Transport {
        url: url.to_owned(),
        kind,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_lines_parse_and_later_values_win() {
        let mut headers = Headers::new();
        headers.set("User-Agent", "default");
        headers
            .set_line("user-agent: Mozilla/5.0 (X11; Linux x86_64)")
            .unwrap();
        headers
            .set_line("Referer:https://www.example.test/a?b=c:d")
            .unwrap();
        assert_eq!(
            headers.get("USER-AGENT"),
            Some("Mozilla/5.0 (X11; Linux x86_64)")
        );
        assert_eq!(
            headers.get("referer"),
            Some("https://www.example.test/a?b=c:d")
        );
        assert_eq!(headers.iter().count(), 2);
    }

    #[test]
    fn malformed_header_lines_are_rejected() {
        for line in ["no-colon", ": value", "bad name: v", "x: a\r\nInjected: 1"] {
            assert_eq!(
                Headers::parse_line(line).unwrap_err().kind(),
                "invalid_header",
                "{line:?}"
            );
        }
    }

    #[test]
    fn credentials_can_be_stripped() {
        let mut headers = Headers::new();
        headers.set("Cookie", "a=b");
        headers.set("AUTHORIZATION", "Bearer x");
        headers.set("Referer", "https://r.test/");
        let stripped = headers.without_credentials();
        assert_eq!(
            stripped.iter().collect::<Vec<_>>(),
            [("Referer", "https://r.test/")]
        );
    }
}
