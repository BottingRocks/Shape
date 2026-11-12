//! Package finders: structure-proven names for every function and MAIN binding.
//!
//! Each package module (one per program subsystem) recognizes its functions and
//! MAIN-owned bindings by structure and returns [`FinderClaim`]s. The naming
//! plan ([`super::semantic_names`]) resolves every claim through its own
//! fail-closed proofs, so a finder never renames anything itself.
//!
//! # Rules every package follows
//!
//! * Recognition reads structure only: a function's own body, its formal
//!   slots, its returned or owned objects, its consumers and its callers, and
//!   stable host/API property names (`addEventListener`, `subscribe`, ...).
//!   Generated spellings (`$KEY_n`, `$THREAD_n_m`, `$AB`), byte offsets, line
//!   numbers, randomized literals, numeric build constants, slot numbers and
//!   saved catalogs or renamed files are never read.
//! * A finder may anchor on another package's proven role through
//!   [`FinderCtx`] helpers, never by spelling.
//! * Two candidates with equal evidence are reported as ambiguous by the plan;
//!   a finder returns every candidate it proves and lets the plan refuse.
//! * Every role a package can claim is listed once in its `VOCABULARY`, with
//!   the one name it receives. A role listed in two packages fails the
//!   vocabulary test.

use oxc_ast::ast::Program;
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::Span;

use crate::signals::schema::Confidence;

pub(crate) mod function_interceptor;
pub(crate) mod entry_runtime;
pub(crate) mod vm_exec;
pub(crate) mod collector_wiring;
pub(crate) mod encoding_framing;
pub(crate) mod serializer;
pub(crate) mod request_envelope;
pub(crate) mod carriers;
pub(crate) mod interception_dom;
pub(crate) mod browser_env;
pub(crate) mod platform_gpu;
pub(crate) mod interaction_timing;
pub(crate) mod graphics_canvas;
pub(crate) mod persistence_telemetry;
pub mod purposes;

/// The parse a package finder reads.
#[allow(dead_code)] // read by the package finders as they are filled in
pub(crate) struct FinderCtx<'s, 'a> {
    pub source: &'s str,
    pub program: &'a Program<'a>,
    pub scoping: &'a Scoping,
    pub nodes: &'a AstNodes<'a>,
}

/// What a vocabulary role names.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FinderRoleKind {
    /// A function node (declaration, single holder or own name). Anonymous
    /// functions are recorded with their name but not renamed.
    Function,
    /// The single MAIN alias of a function claimed by the same package.
    FunctionAlias,
    /// A MAIN-owned variable (data, state or alias), addressed by its declaring
    /// identifier.
    MainBinding,
    /// The binding a class declaration introduces, addressed by the class
    /// node's span.
    Class,
}

/// One vocabulary entry: a role and the one name it receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FinderRole {
    pub role: &'static str,
    pub name: &'static str,
    pub kind: FinderRoleKind,
}

/// One structural claim. Spans are exact node/identifier spans of this parse.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FinderClaim {
    Function {
        role: &'static str,
        function_span: Span,
        /// `(alias_role)` of the function's single MAIN alias, when claimed.
        alias_role: Option<&'static str>,
        confidence: Confidence,
        evidence: Vec<String>,
    },
    MainBinding {
        role: &'static str,
        /// The declaring identifier span of the MAIN binding.
        identifier_span: Span,
        confidence: Confidence,
        evidence: Vec<String>,
    },
    /// A class declaration's binding, addressed by the class node span.
    ClassBinding {
        role: &'static str,
        class_span: Span,
        confidence: Confidence,
        evidence: Vec<String>,
    },
}

impl FinderClaim {
    pub(crate) fn role(&self) -> &'static str {
        match self {
            Self::Function { role, .. }
            | Self::MainBinding { role, .. }
            | Self::ClassBinding { role, .. } => role,
        }
    }
}

const PACKAGES: &[(&str, &[FinderRole], fn(&FinderCtx) -> Vec<FinderClaim>)] = &[
    (
        "p01_entry_runtime",
        entry_runtime::VOCABULARY,
        entry_runtime::find,
    ),
    ("p02_exec_a", vm_exec::operators::VOCABULARY, vm_exec::operators::find),
    ("p03_exec_b", vm_exec::classes::VOCABULARY, vm_exec::classes::find),
    (
        "p04_collector_wiring",
        collector_wiring::VOCABULARY,
        collector_wiring::find,
    ),
    (
        "p05_encoding_framing",
        encoding_framing::VOCABULARY,
        encoding_framing::find,
    ),
    (
        "p06_serializer",
        serializer::VOCABULARY,
        serializer::find,
    ),
    (
        "p07_request_envelope",
        request_envelope::VOCABULARY,
        request_envelope::find,
    ),
    ("request_output_carriers", carriers::VOCABULARY, carriers::find),
    (
        "p09_interception_dom",
        interception_dom::VOCABULARY,
        interception_dom::find,
    ),
    (
        "p10_browser_env",
        browser_env::VOCABULARY,
        browser_env::find,
    ),
    (
        "p11_env_platform_gpu",
        platform_gpu::VOCABULARY,
        platform_gpu::find,
    ),
    (
        "p12_interaction_timing",
        interaction_timing::VOCABULARY,
        interaction_timing::find,
    ),
    (
        "p13_graphics_canvas",
        graphics_canvas::VOCABULARY,
        graphics_canvas::find,
    ),
    (
        "p14_persistence_telemetry",
        persistence_telemetry::VOCABULARY,
        persistence_telemetry::find,
    ),
];

/// Every package vocabulary entry, with its package name.
pub(crate) fn vocabulary() -> impl Iterator<Item = (&'static str, &'static FinderRole)> {
    PACKAGES
        .iter()
        .flat_map(|(package, roles, _)| roles.iter().map(move |role| (*package, role)))
}

/// The vocabulary entry of `role`, when a package lists it.
pub(crate) fn finder_role(role: &str) -> Option<&'static FinderRole> {
    vocabulary()
        .map(|(_, entry)| entry)
        .find(|entry| entry.role == role)
}

/// Run every package finder. A claim whose role is not in its package's
/// vocabulary is dropped here and reported by the vocabulary test.
pub(crate) fn run_all(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let mut claims = Vec::new();
    for (package, roles, find) in PACKAGES {
        let _t = crate::signals::timing::span(package);
        for claim in find(ctx) {
            if roles.iter().any(|entry| entry.role == claim.role()) {
                claims.push(claim);
            }
        }
    }
    claims
}

/// The listener-style entry-chain claims, for a consumer that has already
/// collected every other proposal and must only fill still-unproven spans.
/// One structural walk of the package index serves the whole batch.
pub(crate) fn late_entry_claims(ctx: &FinderCtx) -> Vec<FinderClaim> {
    entry_runtime::late_entry_claims(ctx)
}

/// Function-only consumers can reuse package proofs on an existing parse,
/// without constructing properties, a naming plan or a role graph.
pub(crate) fn function_claims_timed(
    ctx: &FinderCtx,
    timings: &mut std::collections::BTreeMap<String, f64>,
) -> Vec<FinderClaim> {
    let mut claims = Vec::new();
    for (package, roles, find) in PACKAGES {
        // Measured independently at 15.6s on the reference trace, versus
        // 20–305ms for every other package. Keep this standalone inspection
        // finder available, but never include it in the interactive edit path.
        if *package == "p13_graphics_canvas" {
            timings.insert("skipped_p13_graphics_canvas".into(), 0.0);
            continue;
        }
        let start = std::time::Instant::now();
        // The listener-style entry chain is deliberately not part of the
        // early proposal batch: it is a last-resort fallback for the threads
        // pass, so it must never take priority over shared, inherited or
        // collector claims. The publisher path is unchanged.
        let package_claims = if *package == "p01_entry_runtime" {
            entry_runtime::find_thread_early(ctx)
        } else {
            find(ctx)
        };
        for claim in package_claims {
            if matches!(claim, FinderClaim::Function { .. })
                && roles.iter().any(|r| r.role == claim.role())
            {
                claims.push(claim);
            }
        }
        timings.insert(
            format!("finder_{package}"),
            start.elapsed().as_secs_f64() * 1000.0,
        );
    }
    claims
}

/// Package name and role for every claim of every package, including claims
/// whose role is missing from the package vocabulary (for tests).
pub(crate) fn audit(ctx: &FinderCtx) -> Vec<(&'static str, &'static str, bool)> {
    let mut out = Vec::new();
    for (package, roles, find) in PACKAGES {
        for claim in find(ctx) {
            let listed = roles.iter().any(|entry| entry.role == claim.role());
            out.push((*package, claim.role(), listed));
        }
    }
    out
}
