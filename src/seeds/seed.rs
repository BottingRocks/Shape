//! Kernel URL resolution and seed-token extraction, using URL semantics only.

use url::Url;

use crate::seeds::error::{Error, Result};

/// Query parameter that carries the seed token unless a site says otherwise.
pub const DEFAULT_SEED_PARAM: &str = "seed";

/// Longest accepted token; leaves room for `.<64 hex>.async.js` in a file name.
const MAX_SEED_LEN: usize = 160;

/// A seed token that is safe to use as a file-name stem.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SeedToken(String);

impl SeedToken {
    /// Accepts base64url-style tokens only. `.` is excluded so a token can
    /// never be confused with a `<seed>.<digest>` variant name, and anything
    /// path-like is rejected outright.
    pub fn new(token: impl Into<String>) -> Result<Self> {
        let token = token.into();
        let reason = if token.is_empty() {
            Some("it is empty")
        } else if token.len() > MAX_SEED_LEN {
            Some("it is too long")
        } else if !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'='))
        {
            Some("only A-Z a-z 0-9 _ - = are allowed")
        } else {
            None
        };
        match reason {
            Some(reason) => Err(Error::UnsafeSeed { token, reason }),
            None => Ok(Self(token)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SeedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Parse a user-supplied absolute http(s) URL.
pub fn parse_http_url(input: &str) -> Result<Url> {
    let url = Url::parse(input).map_err(|error| Error::InvalidUrl {
        input: input.to_owned(),
        reason: error.to_string(),
    })?;
    require_http(url)
}

fn require_http(url: Url) -> Result<Url> {
    if matches!(url.scheme(), "http" | "https") && url.has_host() {
        Ok(url)
    } else {
        Err(Error::UnsupportedScheme { url: url.into() })
    }
}

/// Resolve the launcher's kernel reference the way a browser resolves a
/// `script.src`: absolute references stand alone, anything else is resolved
/// against `base` (path-, query- and scheme-relative forms included).
pub fn resolve_kernel_url(reference: &str, base: Option<&Url>) -> Result<Url> {
    let reference = reference.trim();
    match Url::parse(reference) {
        Ok(url) => require_http(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let base = base.ok_or_else(|| Error::RelativeKernelUrl {
                reference: reference.to_owned(),
            })?;
            let url = base.join(reference).map_err(|error| Error::InvalidUrl {
                input: reference.to_owned(),
                reason: error.to_string(),
            })?;
            require_http(url)
        }
        Err(error) => Err(Error::InvalidUrl {
            input: reference.to_owned(),
            reason: error.to_string(),
        }),
    }
}

/// Extract the seed token from a kernel URL's query string.
///
/// The parameter may sit anywhere in the query and is percent-decoded. A URL
/// without it is an error (not every script URL carries a seed), as is one
/// that repeats it with different values.
pub fn seed_from_url(url: &Url, param: &str) -> Result<SeedToken> {
    let mut values: Vec<String> = Vec::new();
    for (key, value) in url.query_pairs() {
        if key == param && !values.iter().any(|seen| *seen == value) {
            values.push(value.into_owned());
        }
    }
    match values.len() {
        0 => Err(Error::MissingSeed {
            url: url.to_string(),
            param: param.to_owned(),
        }),
        1 => SeedToken::new(values.remove(0)),
        _ => Err(Error::AmbiguousSeed {
            param: param.to_owned(),
            values,
        }),
    }
}

/// Token for a kernel that carries no seed in its URL. A direct kernel's
/// identity is its bytes, so the same kernel always maps to the same token and
/// is stored once even when the response appends fresh per-request data.
pub fn seed_from_kernel(kernel: &[u8]) -> Result<SeedToken> {
    SeedToken::new(crate::seeds::script::sha256_hex(kernel))
}

/// Seed token of a kernel reference that may be relative. The token lives in
/// the query, which no base URL can change, so none is needed to read it.
pub fn seed_from_reference(reference: &str, param: &str) -> Result<SeedToken> {
    let placeholder = Url::parse("http://relative.invalid/").expect("static URL");
    let url = Url::options()
        .base_url(Some(&placeholder))
        .parse(reference.trim())
        .map_err(|error| Error::InvalidUrl {
            input: reference.to_owned(),
            reason: error.to_string(),
        })?;
    seed_from_url(&url, param)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(input: &str) -> Url {
        Url::parse(input).unwrap()
    }

    #[test]
    fn seed_is_found_wherever_it_sits_in_the_query() {
        for query in ["seed=AbC-1_2&x=1", "x=1&seed=AbC-1_2", "a=&seed=AbC-1_2&z"] {
            let found = seed_from_url(&url(&format!("https://h.test/k.js?{query}")), "seed");
            assert_eq!(found.unwrap().as_str(), "AbC-1_2", "{query}");
        }
    }

    #[test]
    fn seed_is_percent_decoded() {
        let found = seed_from_url(&url("https://h.test/k.js?seed=AB%2Dcd%5Fef"), "seed").unwrap();
        assert_eq!(found.as_str(), "AB-cd_ef");
    }

    #[test]
    fn decoded_path_separators_are_rejected() {
        for query in [
            "seed=..%2F..%2Fetc",
            "seed=a%5Cb",
            "seed=a.b",
            "seed=a+b",
            "seed=",
        ] {
            let error = seed_from_url(&url(&format!("https://h.test/k.js?{query}")), "seed");
            assert_eq!(error.unwrap_err().kind(), "unsafe_seed", "{query}");
        }
    }

    #[test]
    fn missing_seed_is_an_error_not_a_guess() {
        let error = seed_from_url(&url("https://h.test/k.js?async&myseed=A&seedy=B"), "seed");
        assert_eq!(error.unwrap_err().kind(), "missing_seed");
        let error = seed_from_url(&url("https://h.test/seed=A/k.js"), "seed");
        assert_eq!(error.unwrap_err().kind(), "missing_seed");
    }

    #[test]
    fn repeated_seed_must_agree() {
        let same = seed_from_url(&url("https://h.test/k.js?seed=A&seed=A"), "seed").unwrap();
        assert_eq!(same.as_str(), "A");
        let error = seed_from_url(&url("https://h.test/k.js?seed=A&seed=B"), "seed").unwrap_err();
        assert!(matches!(error, Error::AmbiguousSeed { ref values, .. } if values == &["A", "B"]));
    }

    #[test]
    fn custom_seed_parameter() {
        let found = seed_from_url(&url("https://h.test/k.js?seed=A&token=B"), "token").unwrap();
        assert_eq!(found.as_str(), "B");
    }

    #[test]
    fn relative_references_follow_url_semantics() {
        let base = url("https://cdn.test/a/b/launcher.js?async");
        let cases = [
            ("/ssx/k.js?seed=A", "https://cdn.test/ssx/k.js?seed=A"),
            ("k.js?seed=A", "https://cdn.test/a/b/k.js?seed=A"),
            ("../k.js?seed=A", "https://cdn.test/a/k.js?seed=A"),
            ("?seed=A", "https://cdn.test/a/b/launcher.js?seed=A"),
            ("//other.test/k.js?seed=A", "https://other.test/k.js?seed=A"),
            ("http://abs.test/k.js?seed=A", "http://abs.test/k.js?seed=A"),
        ];
        for (reference, expected) in cases {
            let resolved = resolve_kernel_url(reference, Some(&base)).unwrap();
            assert_eq!(resolved.as_str(), expected, "{reference}");
        }
    }

    #[test]
    fn relative_reference_without_base_is_an_error() {
        let error = resolve_kernel_url("/ssx/k.js?seed=A", None).unwrap_err();
        assert_eq!(error.kind(), "relative_kernel_url");
        assert_eq!(
            seed_from_reference("/ssx/k.js?seed=A", "seed")
                .unwrap()
                .as_str(),
            "A"
        );
    }

    #[test]
    fn non_http_schemes_are_refused() {
        for reference in [
            "file:///etc/passwd?seed=A",
            "data:text/javascript,1",
            "javascript:1",
        ] {
            let error = resolve_kernel_url(reference, None).unwrap_err();
            assert_eq!(error.kind(), "unsupported_scheme", "{reference}");
        }
        assert_eq!(
            parse_http_url("ftp://h.test/x").unwrap_err().kind(),
            "unsupported_scheme"
        );
        assert_eq!(
            parse_http_url("not a url").unwrap_err().kind(),
            "invalid_url"
        );
    }
}
