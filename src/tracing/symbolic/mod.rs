//! A bounded symbolic executor for the executable handler-template subset.
//!
//! This module is intentionally a boundary, not a JavaScript runtime.  A
//! decoder supplies [`DecodedInstruction`] values and this module executes the
//! already extracted [`HandlerTemplate`] recipes against a small symbolic
//! stack and slot store.  Oxc is used only when the resulting Rust-owned trace
//! is emitted as JavaScript.

use super::predicate;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstBuilder, NONE};
use oxc_codegen::Codegen;
use oxc_span::{SourceType, SPAN};
use oxc_syntax::{
    number::NumberBase,
    operator::{BinaryOperator, UnaryOperator},
};

use crate::{
    js_number::{parse_numeric_literal, to_int32, to_uint32},
    operations::templates::{
        admit_recipe, AdmissionError, CallFrameField, ErrorRecordField, ReceiverField,
        RecipeBounds, SpanRecord, TryFrameField,
    },
    HandlerTemplate, HandlerTemplateReport, LiteralValue, PrimitiveOperator, RecipeOperation,
    RecipeStep, SemanticValue,
};

mod emit;
mod exec;
mod model;
mod session;

#[cfg(test)]
mod tests;

pub use emit::*;
pub(crate) use exec::*;
pub use exec::{fold_binary, fold_unary, parse_number_text, same_fact};
pub use model::*;
pub use session::*;
