//! Diagnostics and per-stage outcome types.
//!
//! Every stage records a [`StageStatus`] so that "found", "unknown", "ambiguous",
//! "unsupported" and "error" are distinguished instead of silently pretending that
//! a pass completed.

use std::fmt;

/// Pipeline stage. The order is the canonical pipeline order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Clean,
    Rename,
    Find,
    Extract,
}

impl Stage {
    pub const ORDER: [Stage; 4] = [Stage::Clean, Stage::Rename, Stage::Find, Stage::Extract];

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Clean => "CLEAN",
            Stage::Rename => "RENAME",
            Stage::Find => "FIND",
            Stage::Extract => "EXTRACT",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Outcome of one pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StageStatus {
    /// Concrete, structurally verified result.
    Found,
    /// Nothing conclusive was present for this stage.
    Unknown,
    /// Conflicting or partial evidence; nothing was rewritten.
    Ambiguous,
    /// The input uses a construct that this stage deliberately does not handle.
    Unsupported,
    /// Processing failed.
    Error,
}

impl StageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StageStatus::Found => "found",
            StageStatus::Unknown => "unknown",
            StageStatus::Ambiguous => "ambiguous",
            StageStatus::Unsupported => "unsupported",
            StageStatus::Error => "error",
        }
    }
}

impl fmt::Display for StageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// A single human-readable note attached to a stage.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub stage: Stage,
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    /// 0-based byte offsets into the source text of the stage that produced it.
    pub span: Option<(u32, u32)>,
    /// 1-based line/column when known.
    pub location: Option<(u32, u32)>,
}

impl Diagnostic {
    pub fn new(
        stage: Stage,
        severity: Severity,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            severity,
            code,
            message: message.into(),
            span: None,
            location: None,
        }
    }

    pub fn info(stage: Stage, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(stage, Severity::Info, code, message)
    }

    pub fn warn(stage: Stage, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(stage, Severity::Warning, code, message)
    }

    pub fn error(stage: Stage, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(stage, Severity::Error, code, message)
    }

    pub fn with_span(mut self, start: u32, end: u32) -> Self {
        self.span = Some((start, end));
        self
    }

    pub fn with_location(mut self, line: u32, column: u32) -> Self {
        self.location = Some((line, column));
        self
    }

    /// One-line rendering, prefixed with the stage and severity.
    pub fn render(&self) -> String {
        let loc = match (self.location, self.span) {
            (Some((l, c)), _) => format!(" @{l}:{c}"),
            (None, Some((s, e))) => format!(" @{s}..{e}"),
            (None, None) => String::new(),
        };
        format!(
            "[{stage}/{severity}]{loc} {code}: {message}",
            stage = self.stage,
            severity = self.severity.as_str(),
            code = self.code,
            message = self.message,
        )
    }
}
