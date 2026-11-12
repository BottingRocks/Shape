//! Corpus walk internals: seed walking, emission checks, and fingerprinting.
//!
//! The [`crate::trace::commands::corpus`] front end owns argument
//! parsing and the CLI. These modules own the walk and comparison so that the
//! command file stays navigable.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use serde_json::{json, Value};

use crate::emitted_cleanup::{
    simplify_cleaned_emitted_property_writes, simplify_emitted_property_writes,
};
use crate::trace::thread::{external_names, unknown_inputs};
use crate::{
    deobfuscate_with_report, emit_javascript, util::fnv1a64_hex, DescriptorRecord, DispatchStart,
    EmitOptions, KernelArtifact, PrefixBranchKind, PrefixLimits, PrefixPreparation,
    PrefixStepEffect, PrefixStopReason, PrefixTraceReport, SymbolicValue, ThreadDescriptor,
    ThreadInputs, TraceLimits, TraceNodeKind, TraceStatementKind,
};

mod compare;
mod emission;
mod walk;

pub use compare::*;
pub use emission::*;
pub use walk::*;

/// Per-call kernel caching. New callers are uncached unless they opt in.
#[derive(Clone, Debug, Default)]
pub enum KernelCachePolicy {
    /// Compile in memory, without consulting or writing any kernel cache.
    #[default]
    Disabled,
    /// Use `<directory>/<seed-file-name>.kernel.json`, validating source identity.
    Directory(PathBuf),
    /// Use the legacy environment/default cache (including its `off` setting).
    Environment,
}

/// Options of one corpus walk (`shape trace corpus` flags).
#[derive(Clone, Debug)]
pub struct CorpusOptions {
    pub json: bool,
    pub seeds: Vec<PathBuf>,
    pub report_dir: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub update: bool,
    pub self_check: bool,
    pub cleanup: bool,
    /// How many functions of one seed walk at once.
    pub jobs: usize,
    /// Where to write every conditional jump's decision.
    pub branches: Option<PathBuf>,
    /// Walk every fake-by-test branch's ruled-out side too.
    pub audit: bool,
    pub steps: usize,
    pub limit: Option<usize>,
}

/// The stack each seed walk runs on: reserved, not used, until needed.
pub const SEED_STACK_BYTES: usize = 512 * 1024 * 1024;
