//! Standalone, non-executing proof and evaluation for `vmDecodeString`.
//!
//! The module separates three things that must not be conflated:
//!
//! 1. [`VmStringProof`] parses source, resolves bindings through Oxc, normalizes
//!    the exact seven-parameter helper, and asks the kernel decoder prover for
//!    the manual base64 loop, in either proven alphabet. The whole source digest and both spans
//!    bind that proof to one source identity.
//! 2. [`VmStringEvaluator`] performs the helper's byte arithmetic in Rust over
//!    an already-proven table/cache pair and explicit [`VmStringLimits`].
//! 3. [`Utf16String`] carries the result as lossless UTF-16 code units, so
//!    unpaired surrogates survive and are never silently replaced.
//!
//! [`evaluate_raw_xor`] and [`decode_manual_base64`] are lower-level arithmetic
//! utilities. They do not prove a helper, decoder, table, or cache, and callers
//! must not use them as authority for a source rewrite on their own.
//!
//! Nothing here invokes JavaScript, WASM, or a user-supplied decoder. The only
//! executable semantics modeled are the admitted helper's bounded arithmetic;
//! uncertain input is a typed error, never an invented value.

use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::{ContentEq, Ident, SourceType, Span};
use oxc_syntax::scope::ScopeFlags;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::analysis;
use crate::kernel::decoder_proof::{prove_runtime_decoder, DecoderProof, URL_SAFE_ALPHABET};

mod integration;

pub use integration::{
    fold_emitted_vm_strings, fold_emitted_vm_strings_from_plan,
    fold_emitted_vm_strings_from_report, VmStringDecoder, VmStringFoldPlan, VmStringFoldReason,
    VmStringFoldReport, VmStringFoldResult, VmStringIntegrationError, VM_STRING_FOLD_PLAN_VERSION,
};

const STANDARD_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const MAX_ROLE_NAMES: [&str; 16] = [
    "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13", "r14",
    "r15",
];

mod evaluator;
mod proof;
mod value;

#[cfg(test)]
mod tests;

pub use evaluator::*;
pub use proof::*;
pub use value::*;
