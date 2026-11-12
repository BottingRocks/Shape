//! The acquisition flow: fetch a launcher, read the kernel URL out of it,
//! fetch that kernel, and store the two together.

use std::time::Duration;

use url::Url;

use crate::seeds::error::{Error, Result, Role};
use crate::seeds::http::{Headers, HttpClient, HttpResponse};
use crate::seeds::script::{self, KernelSummary};
use crate::seeds::seed::{self, SeedToken, DEFAULT_SEED_PARAM};
use crate::seeds::sites::Protocol;
use crate::seeds::store::{SeedStore, StoreOutcome};

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// Sent with the launcher request, and with the kernel request too (minus
    /// credentials unless the full launcher-to-kernel path stays on the
    /// original launcher origin).
    pub headers: Headers,
    /// The containing document base URL for a relative kernel URL. A direct
    /// launcher fetch cannot discover an HTML document's `<base>` element, so
    /// this defaults to the launcher's final URL as a documented fallback.
    /// Supply this explicitly when browser document-base semantics matter.
    pub base_url: Option<Url>,
    pub seed_param: String,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            headers: default_headers(),
            base_url: None,
            seed_param: DEFAULT_SEED_PARAM.to_owned(),
        }
    }
}

/// Headers sent when the caller supplies none: a tested, browser-compatible
/// profile that lets servers select browser-oriented seed variants.
pub fn default_headers() -> Headers {
    let mut headers = Headers::new();
    headers.set("User-Agent", DEFAULT_USER_AGENT);
    headers.set("Accept", "*/*");
    headers
}

/// A launcher and the kernel it loaded, exactly as served, or a direct kernel
/// with no launcher.
#[derive(Debug, Clone)]
pub struct FetchedSeed {
    pub seed: SeedToken,
    pub launcher_url: Url,
    pub kernel_url: Url,
    pub launcher: Option<Vec<u8>>,
    pub kernel: Vec<u8>,
    pub kernel_summary: KernelSummary,
}

/// Fetch one launcher and its kernel. Nothing is written to disk.
pub fn fetch_seed(
    client: &dyn HttpClient,
    launcher_url: &Url,
    options: &FetchOptions,
) -> Result<FetchedSeed> {
    let launcher = get_response(client, Role::Launcher, launcher_url, &options.headers)?;
    ensure_not_html(&launcher, Role::Launcher, launcher_url)?;
    let rejected = |source| Error::Response {
        role: Role::Launcher,
        url: launcher_url.to_string(),
        source: Box::new(source),
    };
    let parsed = script::parse_launcher_with_seed_param(&launcher.body, &options.seed_param)
        .map_err(rejected)?;
    let base = options.base_url.as_ref().unwrap_or(&launcher.final_url);
    let kernel_url = seed::resolve_kernel_url(&parsed.kernel_ref, Some(base)).map_err(rejected)?;
    let seed = seed::seed_from_url(&kernel_url, &options.seed_param).map_err(rejected)?;

    // The kernel URL comes from fetched content. Credentials the user attached
    // for the launcher's origin do not follow it anywhere else.
    let kernel_headers = if kernel_url.origin() == launcher_url.origin()
        && launcher.final_url.origin() == launcher_url.origin()
    {
        options.headers.clone()
    } else {
        options.headers.without_credentials()
    };
    let kernel = get_response(client, Role::Kernel, &kernel_url, &kernel_headers)?;
    ensure_not_html(&kernel, Role::Kernel, &kernel_url)?;
    let kernel_summary = script::parse_kernel(&kernel.body).map_err(|source| Error::Response {
        role: Role::Kernel,
        url: kernel_url.to_string(),
        source: Box::new(source),
    })?;

    Ok(FetchedSeed {
        seed,
        launcher_url: launcher_url.clone(),
        kernel_url,
        launcher: Some(launcher.body),
        kernel: kernel.body,
        kernel_summary,
    })
}

/// Fetch a URL that serves the kernel itself. The server may append one final
/// top-level configuration IIFE; that statement is per-request noise, not part
/// of the kernel, so it is dropped before the kernel is hashed and stored. The
/// token is the kernel digest: identical kernels deduplicate exactly.
///
/// A direct-kernel entry point may also be an HTML document whose kernel is
/// inline. In that case the largest inline `<script>` body that classifies as
/// a raw kernel is extracted. If the document is one Akamai-style interstitial
/// with a same-origin `<meta http-equiv="refresh">`, that hop is followed once
/// with the response's cookies before extraction.
pub fn fetch_direct_kernel(
    client: &dyn HttpClient,
    kernel_url: &Url,
    options: &FetchOptions,
) -> Result<FetchedSeed> {
    let response = get_response(client, Role::Kernel, kernel_url, &options.headers)?;
    let response = follow_html_interstitial(client, response, options)?;
    let final_url = response.final_url.to_string();
    let rejected = |source| Error::Response {
        role: Role::Kernel,
        url: final_url.clone(),
        source: Box::new(source),
    };
    let mut body = if is_html(&response) {
        inline_kernel_script(&response.body).ok_or_else(|| {
            rejected(Error::InlineKernelMissing {
                url: final_url.clone(),
            })
        })?
    } else {
        response.body
    };
    if let Some(prefix_len) = script::kernel_prefix_len(&body) {
        body.truncate(prefix_len);
    }
    let kernel_summary = script::parse_kernel(&body).map_err(rejected)?;
    let seed = seed::seed_from_kernel(&body)?;
    Ok(FetchedSeed {
        seed,
        launcher_url: kernel_url.clone(),
        kernel_url: kernel_url.clone(),
        launcher: None,
        kernel: body,
        kernel_summary,
    })
}

fn get_response(
    client: &dyn HttpClient,
    role: Role,
    url: &Url,
    headers: &Headers,
) -> Result<HttpResponse> {
    let response = client.get(url, headers).map_err(|source| Error::Response {
        role,
        url: url.to_string(),
        source: Box::new(source),
    })?;
    if !(200..300).contains(&response.status) {
        return Err(Error::HttpStatus {
            role,
            url: url.to_string(),
            status: response.status,
        });
    }
    Ok(response)
}

fn ensure_not_html(response: &HttpResponse, role: Role, url: &Url) -> Result<()> {
    if is_html(response) {
        return Err(Error::Response {
            role,
            url: url.to_string(),
            source: Box::new(Error::Html),
        });
    }
    Ok(())
}

/// True for an HTML content type, or for a body that begins with markup even
/// when the server omitted or mislabelled the type (an interstitial often is).
fn is_html(response: &HttpResponse) -> bool {
    if response.content_type.as_deref().is_some_and(|value| {
        value
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("text/html")
    }) {
        return true;
    }
    response
        .body
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'<')
}

/// Follow at most one same-origin HTML meta refresh, carrying the first
/// response's cookies and setting `Referer`. Following is deliberately limited
/// to the origin that set the cookies so credentials never cross origins.
fn follow_html_interstitial(
    client: &dyn HttpClient,
    response: HttpResponse,
    options: &FetchOptions,
) -> Result<HttpResponse> {
    if !is_html(&response) {
        return Ok(response);
    }
    let Some(refresh_url) = html_refresh_url(&response.body, &response.final_url) else {
        return Ok(response);
    };
    if refresh_url.origin() != response.final_url.origin() {
        return Ok(response);
    }
    let mut headers = options.headers.clone();
    if let Some(cookie) = cookie_header(&response.set_cookie, &refresh_url) {
        headers.set("Cookie", &cookie);
    }
    headers.set("Referer", response.final_url.as_str());
    if headers.get("Sec-Fetch-Site").is_some() {
        headers.set("Sec-Fetch-Site", "same-origin");
    }
    get_response(client, Role::Kernel, &refresh_url, &headers)
}

/// The URL from the first `<meta http-equiv="refresh" ...>` tag, resolved
/// against `base`.
fn html_refresh_url(html: &[u8], base: &Url) -> Option<Url> {
    let text = std::str::from_utf8(html).ok()?;
    let lower = text.to_ascii_lowercase();
    let mut position = 0;
    while let Some(found) = lower[position..].find("<meta") {
        let start = position + found + "<meta".len();
        let boundary = lower.as_bytes().get(start).copied();
        if !boundary.is_some_and(|byte| byte.is_ascii_whitespace() || byte == b'/' || byte == b'>')
        {
            position = start;
            continue;
        }
        let end = start + lower[start..].find('>')?;
        let attributes = parse_html_attributes(&text[start..end]);
        position = end + 1;
        let value = |name: &str| {
            attributes
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let Some(equiv) = value("http-equiv") else {
            continue;
        };
        if !equiv.eq_ignore_ascii_case("refresh") {
            continue;
        }
        let Some(reference) = value("content").and_then(refresh_reference) else {
            continue;
        };
        let reference = reference.replace("&amp;", "&");
        if let Ok(url) = base.join(reference.trim()) {
            if matches!(url.scheme(), "http" | "https") && url.has_host() {
                return Some(url);
            }
        }
    }
    None
}

/// Extract `URL=...` from a refresh `content` value such as
/// `5; URL='/tracking/?bm-verify=...'`.
fn refresh_reference(content: &str) -> Option<&str> {
    let lower = content.to_ascii_lowercase();
    let position = lower.find("url")?;
    if position > 0
        && content[..position]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    let value = content[position + 3..].trim_start().strip_prefix('=')?;
    let value = value.trim_start();
    let reference = if let Some(rest) = value.strip_prefix('"') {
        rest.split('"').next()?
    } else if let Some(rest) = value.strip_prefix('\'') {
        rest.split('\'').next()?
    } else {
        value
            .split(|character: char| character.is_whitespace() || character == ';')
            .next()?
    };
    (!reference.is_empty()).then_some(reference)
}

/// Parse the attributes of one tag body. Names are lowercased; values are
/// preserved exactly so challenge tokens keep their case.
fn parse_html_attributes(tag: &str) -> Vec<(String, String)> {
    let bytes = tag.as_bytes();
    let mut attributes = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        while position < bytes.len()
            && (bytes[position].is_ascii_whitespace() || bytes[position] == b'/')
        {
            position += 1;
        }
        if position >= bytes.len() {
            break;
        }
        let name_start = position;
        while position < bytes.len()
            && !bytes[position].is_ascii_whitespace()
            && !matches!(bytes[position], b'=' | b'/' | b'>')
        {
            position += 1;
        }
        let name = tag[name_start..position].to_ascii_lowercase();
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        if position >= bytes.len() || bytes[position] != b'=' {
            if name.is_empty() {
                position += 1;
            }
            continue;
        }
        position += 1;
        while position < bytes.len() && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        let value =
            if position < bytes.len() && (bytes[position] == b'"' || bytes[position] == b'\'') {
                let quote = bytes[position];
                position += 1;
                let value_start = position;
                while position < bytes.len() && bytes[position] != quote {
                    position += 1;
                }
                let value = tag[value_start..position].to_owned();
                if position < bytes.len() {
                    position += 1;
                }
                value
            } else {
                let value_start = position;
                while position < bytes.len()
                    && !bytes[position].is_ascii_whitespace()
                    && bytes[position] != b'>'
                {
                    position += 1;
                }
                tag[value_start..position].to_owned()
            };
        if !name.is_empty() {
            attributes.push((name, value));
        }
    }
    attributes
}

/// The largest inline `<script>` body that classifies as a raw kernel after the
/// direct-kernel configuration trim. `<script src=...>` bodies are skipped.
fn inline_kernel_script(html: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(html).ok()?;
    let lower = text.to_ascii_lowercase();
    let mut candidates = Vec::new();
    let mut position = 0;
    while let Some(found) = lower[position..].find("<script") {
        let open = position + found;
        let after = open + "<script".len();
        let boundary = lower.as_bytes().get(after).copied();
        if !boundary.is_some_and(|byte| byte.is_ascii_whitespace() || byte == b'>' || byte == b'/')
        {
            position = after;
            continue;
        }
        let end = after + lower[after..].find('>')?;
        let attributes = parse_html_attributes(&text[after..end]);
        let has_src = attributes.iter().any(|(name, _)| name == "src");
        let content_start = end + 1;
        let content_end = content_start + lower[content_start..].find("</script")?;
        if !has_src {
            candidates.push(&text[content_start..content_end]);
        }
        position = content_end + "</script".len();
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.len()));
    for candidate in candidates {
        if candidate.len() < script::MIN_KERNEL_BYTES {
            continue;
        }
        let bytes = candidate.as_bytes();
        let mut trimmed = bytes.to_vec();
        if let Some(prefix_len) = script::kernel_prefix_len(bytes) {
            trimmed.truncate(prefix_len);
        }
        if script::parse_kernel(&trimmed).is_ok() {
            return Some(bytes.to_vec());
        }
    }
    None
}

/// Build a `Cookie` request header from the response's `Set-Cookie` values,
/// keeping only cookies whose Domain, Path, and Secure attributes allow them to
/// be sent to `url`. The caller has already restricted `url` to the same origin.
fn cookie_header(set_cookie: &[String], url: &Url) -> Option<String> {
    let host = url.host_str()?;
    let path = url.path();
    let mut cookies: Vec<(String, String)> = Vec::new();
    for raw in set_cookie {
        let mut attributes = raw.split(';');
        let Some((name, value)) = attributes.next().and_then(|pair| pair.split_once('=')) else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let mut domain = None;
        let mut cookie_path = None;
        let mut secure = false;
        for attribute in attributes {
            let attribute = attribute.trim();
            if attribute.eq_ignore_ascii_case("secure") {
                secure = true;
                continue;
            }
            let Some((key, value)) = attribute.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            if key.eq_ignore_ascii_case("domain") {
                domain = Some(value.trim_start_matches('.').to_ascii_lowercase());
            } else if key.eq_ignore_ascii_case("path") {
                if value.starts_with('/') {
                    cookie_path = Some(value.to_owned());
                }
            } else if key.eq_ignore_ascii_case("secure") {
                secure = true;
            }
        }
        if secure && url.scheme() != "https" {
            continue;
        }
        if let Some(domain) = &domain {
            if host != domain && !host.ends_with(&format!(".{domain}")) {
                continue;
            }
        }
        if let Some(cookie_path) = &cookie_path {
            if !path.starts_with(cookie_path) {
                continue;
            }
        }
        cookies.retain(|(existing, _)| existing != name);
        cookies.push((name.to_owned(), value.to_owned()));
    }
    (!cookies.is_empty()).then(|| {
        cookies
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// One stored (or already present) seed.
#[derive(Debug, Clone)]
pub struct Collected {
    pub seed: SeedToken,
    pub kernel_url: Url,
    pub kernel_summary: KernelSummary,
    pub outcome: StoreOutcome,
}

/// The result of one launcher request within a collection run.
#[derive(Debug)]
pub struct Attempt {
    pub launcher_url: Url,
    /// 1-based repeat number for this URL.
    pub round: u32,
    pub result: Result<Collected>,
}

/// Fetch one seed and store it.
pub fn collect_one(
    client: &dyn HttpClient,
    store: &SeedStore,
    launcher_url: &Url,
    options: &FetchOptions,
    protocol: Protocol,
) -> Result<Collected> {
    let fetched = match protocol {
        Protocol::AsyncLauncher => fetch_seed(client, launcher_url, options)?,
        Protocol::DirectKernel => fetch_direct_kernel(client, launcher_url, options)?,
        Protocol::SingleFile => {
            return Err(Error::UnsupportedProtocol {
                protocol: "single-file".to_owned(),
            });
        }
    };
    let outcome = store.save(&fetched.seed, fetched.launcher.as_deref(), &fetched.kernel)?;
    Ok(Collected {
        seed: fetched.seed,
        kernel_url: fetched.kernel_url,
        kernel_summary: fetched.kernel_summary,
        outcome,
    })
}

/// A launcher URL to collect from, with the options that apply to it.
#[derive(Debug, Clone)]
pub struct Source {
    pub launcher_url: Url,
    pub options: FetchOptions,
    pub protocol: Protocol,
}

/// Request every source `count` times, sequentially, storing each pair. A
/// failed attempt is recorded and the run continues; `report` sees each
/// attempt as it completes.
pub fn collect(
    client: &dyn HttpClient,
    store: &SeedStore,
    sources: &[Source],
    count: u32,
    delay: Duration,
    mut report: impl FnMut(&Attempt),
) -> Vec<Attempt> {
    let mut attempts = Vec::new();
    for round in 1..=count {
        for source in sources {
            if !attempts.is_empty() && !delay.is_zero() {
                std::thread::sleep(delay);
            }
            let attempt = Attempt {
                launcher_url: source.launcher_url.clone(),
                round,
                result: collect_one(
                    client,
                    store,
                    &source.launcher_url,
                    &source.options,
                    source.protocol,
                ),
            };
            report(&attempt);
            attempts.push(attempt);
        }
    }
    attempts
}
