//! Structured errors. Every variant has a stable machine-readable [`Error::kind`].

use std::path::PathBuf;

/// Which half of a launcher/kernel pair a request or response belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Launcher,
    Kernel,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Role::Launcher => "launcher",
            Role::Kernel => "kernel",
        })
    }
}

/// Why a request never produced an HTTP response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Timeout,
    Connect,
    Redirect,
    Other,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    // --- content classification -------------------------------------------
    #[error("content is empty")]
    Empty,
    #[error("content is not valid UTF-8")]
    NotUtf8,
    #[error("content is an HTML/markup document, not JavaScript")]
    Html,
    #[error("HTML document at {url} contains no inline script that classifies as a raw kernel")]
    InlineKernelMissing { url: String },
    #[error("content does not parse as JavaScript: {message}")]
    NotJavaScript { message: String },
    #[error("JavaScript is neither an async launcher nor a raw kernel: {reason}")]
    UnrecognizedScript { reason: String },
    #[error("launcher assigns {} different kernel URLs to script elements: {}", candidates.len(), candidates.join(", "))]
    AmbiguousKernelUrl { candidates: Vec<String> },
    #[error("launcher sets a script element's src to a value that is not statically known")]
    DynamicKernelUrl,
    #[error("launcher cannot be statically proven safe: {reason}")]
    UnprovenLauncher { reason: &'static str },
    #[error("expected an async launcher but found {found}")]
    ExpectedLauncher { found: &'static str },
    #[error("expected a raw kernel but found {found}")]
    ExpectedKernel { found: &'static str },

    // --- URL and seed token -------------------------------------------------
    #[error("invalid URL `{input}`: {reason}")]
    InvalidUrl { input: String, reason: String },
    #[error("kernel URL `{reference}` is relative; supply a base URL (--base-url)")]
    RelativeKernelUrl { reference: String },
    #[error("unsupported URL scheme in `{url}` (only http and https are fetched)")]
    UnsupportedScheme { url: String },
    #[error("kernel URL `{url}` has no `{param}` query parameter")]
    MissingSeed { url: String, param: String },
    #[error("kernel URL carries conflicting `{param}` values: {}", values.join(", "))]
    AmbiguousSeed { param: String, values: Vec<String> },
    #[error("seed token `{token}` is not usable as a file name: {reason}")]
    UnsafeSeed { token: String, reason: &'static str },

    // --- HTTP -----------------------------------------------------------------
    #[error("invalid header `{input}`: {reason}")]
    InvalidHeader { input: String, reason: &'static str },
    #[error("{role} request to {url} returned HTTP {status}")]
    HttpStatus {
        role: Role,
        url: String,
        status: u16,
    },
    #[error("request to {url} failed: {message}")]
    Transport {
        url: String,
        kind: TransportKind,
        message: String,
    },
    #[error("response from {url} exceeds the {limit}-byte limit")]
    BodyTooLarge { url: String, limit: u64 },
    #[error("{role} response from {url} rejected: {source}")]
    Response {
        role: Role,
        url: String,
        #[source]
        source: Box<Error>,
    },

    // --- site registry --------------------------------------------------------
    #[error("unknown site `{name}` (see `shape seeds sites`)")]
    UnknownSite { name: String },
    #[error("site `{name}` is a legacy reference entry that has not been validated; pass its launcher URL explicitly to fetch it")]
    UnvalidatedSite { name: String },
    #[error("site protocol `{protocol}` is declared but not implemented")]
    UnsupportedProtocol { protocol: String },

    // --- storage --------------------------------------------------------------
    #[error("refusing to overwrite {path}: it holds different bytes")]
    StoreConflict { path: PathBuf },
}

impl Error {
    /// Stable identifier for the failure class, independent of message wording.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io { .. } => "io",
            Error::Empty => "empty",
            Error::NotUtf8 => "not_utf8",
            Error::Html => "html",
            Error::InlineKernelMissing { .. } => "inline_kernel_missing",
            Error::NotJavaScript { .. } => "not_javascript",
            Error::UnrecognizedScript { .. } => "unrecognized_script",
            Error::AmbiguousKernelUrl { .. } => "ambiguous_kernel_url",
            Error::DynamicKernelUrl => "dynamic_kernel_url",
            Error::UnprovenLauncher { .. } => "unproven_launcher",
            Error::ExpectedLauncher { .. } => "expected_launcher",
            Error::ExpectedKernel { .. } => "expected_kernel",
            Error::InvalidUrl { .. } => "invalid_url",
            Error::RelativeKernelUrl { .. } => "relative_kernel_url",
            Error::UnsupportedScheme { .. } => "unsupported_scheme",
            Error::MissingSeed { .. } => "missing_seed",
            Error::AmbiguousSeed { .. } => "ambiguous_seed",
            Error::UnsafeSeed { .. } => "unsafe_seed",
            Error::InvalidHeader { .. } => "invalid_header",
            Error::HttpStatus { .. } => "http_status",
            Error::Transport { kind, .. } => match kind {
                TransportKind::Timeout => "timeout",
                TransportKind::Connect => "connect",
                TransportKind::Redirect => "redirect",
                TransportKind::Other => "transport",
            },
            Error::BodyTooLarge { .. } => "body_too_large",
            Error::Response { source, .. } => source.kind(),
            Error::UnknownSite { .. } => "unknown_site",
            Error::UnvalidatedSite { .. } => "unvalidated_site",
            Error::UnsupportedProtocol { .. } => "unsupported_protocol",
            Error::StoreConflict { .. } => "store_conflict",
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
