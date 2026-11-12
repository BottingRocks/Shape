//! Fail-closed semantic recipes for the bounded, standardized handler subset.
//!
//! This is deliberately separate from the operation catalogue.  The catalogue
//! is useful for inventory and discovery; a recipe is emitted only when every
//! expression and effect in a handler has a proved representation.  Unknown
//! nodes stay in the recipe and make the handler unavailable for execution.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};
use serde::{Deserialize, Serialize};

use crate::transforms::deob::pipeline::{PipelineReport, ProvenOperationTable};
use crate::{analysis, operations::catalogue, util::fnv1a64_hex};

use super::graph::{GraphBuilder, UnknownRegionKind, GRAPH_SCHEMA_VERSION};
pub use super::graph::{
    HandlerGraph, HandlerGraphReport, HandlerIR, HandlerIRReport, OccurrenceId, SpanGranularity,
};
pub use super::table_scan::SpanRecord;
use super::table_scan::{scan_target_table, TableLiteral, TableScanMode, TableSink};

pub const SCHEMA_VERSION: &str = "shape-kernel-handler-templates/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateStatus {
    Executable,
    Partial,
    Unsupported,
}

impl fmt::Display for TemplateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Executable => "executable",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralValue {
    /// A number as originally written in the source text.
    ///
    /// This variant exists for manually constructed recipes and for callers
    /// that only have lexical text.  Extraction never produces it: the
    /// extractor records [`LiteralValue::DecodedNumber`] so the value that Oxc
    /// already decoded is carried authoritatively instead of being reparsed.
    /// Consumers must therefore tolerate a bounded, literal-only
    /// normalization of this text (including legacy octal, separators, and
    /// prefixed literals wider than 64 bits); see the shared
    /// `parse_numeric_literal` helper.
    Number(String),
    /// The exact IEEE-754 binary64 bit pattern of a numeric literal as decoded
    /// by Oxc during extraction.
    ///
    /// Retaining the bits (rather than the source spelling) keeps `-0`, `NaN`,
    /// `Infinity`, and values such as `0x10000000000000000` exact, and keeps
    /// legacy octal like `010` at its decoded value of `8`.  Lexical
    /// provenance stays in the recipe's source span.
    DecodedNumber {
        bits: u64,
    },
    String(String),
    Boolean(bool),
    Null,
    Undefined,
}

/// Operators retain JavaScript semantics.  In particular, `JsAdd` is not a
/// numeric add: it preserves ToPrimitive/string concatenation and its throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveOperator {
    JsAdd,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    ShiftRightZeroFill,
    BitwiseOr,
    BitwiseXor,
    BitwiseAnd,
    Exponent,
    /// `==`: abstract equality, with its coercions.
    Equal,
    /// `!=`.
    NotEqual,
    /// `===`.
    StrictEqual,
    /// `!==`.
    StrictNotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    /// `key in object`.
    In,
    /// `value instanceof constructor`.
    InstanceOf,
    UnaryPlus,
    UnaryMinus,
    BitwiseNot,
    LogicalNot,
    Void,
    Typeof,
}

impl PrimitiveOperator {
    /// True for the arithmetic, bitwise and primitive unary operators the
    /// concrete executor implements. Comparisons, `in`, `instanceof`, `**` and
    /// `typeof` are traced symbolically only.
    pub const fn is_concrete(self) -> bool {
        !matches!(
            self,
            Self::Exponent
                | Self::Equal
                | Self::NotEqual
                | Self::StrictEqual
                | Self::StrictNotEqual
                | Self::LessThan
                | Self::LessThanOrEqual
                | Self::GreaterThan
                | Self::GreaterThanOrEqual
                | Self::In
                | Self::InstanceOf
                | Self::Typeof
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticValue {
    Context,
    Operand {
        position: u32,
    },
    Temporary {
        id: u32,
    },
    Literal(LiteralValue),
    StackLength,
    StackRead {
        index: Box<SemanticValue>,
    },
    MemoryRead {
        slot: Box<SemanticValue>,
    },
    Binary {
        operator: PrimitiveOperator,
        left: Box<SemanticValue>,
        right: Box<SemanticValue>,
    },
    Unary {
        operator: PrimitiveOperator,
        value: Box<SemanticValue>,
    },
    /// A name the handler references but does not own: a captured binding
    /// such as `vmStringTable`, or a global such as `host`. It is an abstract
    /// object expression and is never evaluated statically.
    Binding {
        name: String,
    },
    /// `object[key]` or `object.key` on any non-VM object expression. VM
    /// stack, memory and context fields keep their dedicated nodes.
    PropertyRead {
        object: Box<SemanticValue>,
        key: Box<SemanticValue>,
    },
    /// `{ key: value, ... }` with static keys, in source order.
    ObjectLiteral {
        properties: Vec<(String, SemanticValue)>,
    },
    /// `[a, b, ...]` without holes or spread elements.
    ArrayLiteral {
        elements: Vec<SemanticValue>,
    },
    /// A field of the call frame this handler popped with
    /// [`RecipeOperation::PopCallFrame`]. A VM value.
    PoppedCallFrame {
        field: CallFrameField,
    },
    /// `context.stack.slice(start)`: a fresh array of the stack values from
    /// `start` to the top. `start` is a VM value.
    StackSlice {
        start: Box<SemanticValue>,
    },
    /// `Function.prototype.apply.call(callee, this_value, arguments)`: call a
    /// program function with an explicit receiver and an argument array.
    Call {
        callee: Box<SemanticValue>,
        this_value: Box<SemanticValue>,
        arguments: Box<SemanticValue>,
    },
    /// `vmCreateThread(descriptor, byteOffset, dispatchState, context.memory)`:
    /// a program function whose body is the VM thread at `byte_offset` in
    /// `dispatch_state`, capturing this handler's memory. All three are VM
    /// values.
    CreateThread {
        descriptor: Box<SemanticValue>,
        byte_offset: Box<SemanticValue>,
        dispatch_state: Box<SemanticValue>,
    },
    /// `context.receiverObject` / `context.receiverValue`: the `this` the
    /// running VM function was called with. A program value.
    Receiver {
        field: ReceiverField,
    },
    /// A field of the try frame this handler popped with
    /// [`RecipeOperation::PopTryFrame`]. A VM value.
    PoppedTryFrame {
        field: TryFrameField,
    },
    /// A field of the error record this handler popped with
    /// [`RecipeOperation::PopErrorRecord`]. A VM value.
    PoppedErrorRecord {
        field: ErrorRecordField,
    },
    /// `context.thrownValueStack.pop()`: the pending exception, a program
    /// value.
    PopThrownValue,
    /// A closed `function (…) { … }` expression: every name it uses is its
    /// own or a global. A program value, kept as its source text.
    FunctionLiteral {
        source: String,
    },
    /// A source-free, bounded closure emission plan.  The source-backed
    /// extractor uses `FunctionLiteral`; artifact compilation lowers it to
    /// this representation before serialization.
    ClosurePlan {
        plan: ClosurePlan,
    },
    /// `new callee(arguments…)`; `arguments` is an array literal.
    Construct {
        callee: Box<SemanticValue>,
        arguments: Box<SemanticValue>,
    },
    /// `delete object[key]`: removes a program property, and is `true`
    /// unless the property cannot be removed.
    Delete {
        object: Box<SemanticValue>,
        key: Box<SemanticValue>,
    },
    Unknown {
        span: SpanRecord,
        reason: String,
    },
}

/// The deliberately small source-free closure subset currently admitted by
/// the artifact compiler.  These plans describe executable emission IR, not
/// serialized JavaScript or an Oxc AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosurePlan {
    pub params: Vec<String>,
    pub body: Vec<ClosureStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClosureStatement {
    Expression(ClosureExpression),
    Return(Option<ClosureExpression>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClosureExpression {
    NullIndexCall,
    CharCodeAt { parameter: String },
    AddOne { parameter: String },
    External { name: String },
}

impl ClosurePlan {
    /// Render a canonical function expression.  The renderer is intentionally
    /// closed over the plan enum: adding a new construct requires an explicit
    /// compiler and loader change rather than accepting arbitrary source.
    pub fn to_source(&self) -> String {
        let params = self.params.join(", ");
        let body = self
            .body
            .iter()
            .map(|statement| match statement {
                ClosureStatement::Expression(ClosureExpression::NullIndexCall) => {
                    "null[0]();".to_string()
                }
                ClosureStatement::Return(Some(ClosureExpression::CharCodeAt { parameter })) => {
                    format!("return {parameter}.charCodeAt();")
                }
                ClosureStatement::Return(Some(ClosureExpression::AddOne { parameter })) => {
                    format!("return {parameter} + 1;")
                }
                ClosureStatement::Return(Some(ClosureExpression::External { name })) => {
                    format!("return {name};")
                }
                ClosureStatement::Return(None) => "return;".to_string(),
                ClosureStatement::Expression(ClosureExpression::CharCodeAt { parameter }) => {
                    format!("{parameter}.charCodeAt();")
                }
                ClosureStatement::Expression(ClosureExpression::AddOne { parameter }) => {
                    format!("{parameter} + 1;")
                }
                ClosureStatement::Expression(ClosureExpression::External { name }) => {
                    format!("{name};")
                }
                ClosureStatement::Return(Some(ClosureExpression::NullIndexCall)) => {
                    "return null[0]();".to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("function ({params}) {{ {body} }}")
    }
}

/// Compile one of the bounded closure forms from an already parsed function.
/// This is shared by source extraction and artifact compilation so both paths
/// use exactly the same canonical IR and therefore the same trace text.
pub fn compile_closure_plan_from_function(function: &Function<'_>) -> Result<ClosurePlan, String> {
    if function.id.is_some()
        || function.generator
        || function.r#async
        || function.params.rest.is_some()
        || function.params.items.iter().any(|parameter| {
            parameter.initializer.is_some()
                || !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
        })
    {
        return Err("closed function is outside the source-free closure subset".into());
    }
    let params = function
        .params
        .items
        .iter()
        .map(|parameter| match &parameter.pattern {
            BindingPattern::BindingIdentifier(identifier) => identifier.name.to_string(),
            _ => unreachable!("checked above"),
        })
        .collect::<Vec<_>>();
    let mut unique = BTreeSet::new();
    if params.len() > 32
        || params.iter().any(|parameter| {
            parameter.len() > 128
                || !is_valid_closure_identifier(parameter)
                || !unique.insert(parameter)
        })
    {
        return Err("closed function is outside the source-free closure subset".into());
    }
    let body = function
        .body
        .as_deref()
        .ok_or_else(|| "closed function is outside the source-free closure subset".to_string())?;
    // Directives have semantics that are not represented by ClosurePlan, so
    // rejecting them is safer than silently dropping `use strict`, etc.
    if !body.directives.is_empty() {
        return Err("closed function is outside the source-free closure subset".into());
    }
    let [Statement::ReturnStatement(returned)] = body.statements.as_slice() else {
        if params.is_empty()
            && body.statements.len() == 1
            && matches!(
                &body.statements[0],
                Statement::ExpressionStatement(statement)
                    if is_null_index_call(&statement.expression)
            )
        {
            return Ok(ClosurePlan {
                params,
                body: vec![ClosureStatement::Expression(
                    ClosureExpression::NullIndexCall,
                )],
            });
        }
        return Err("closed function is outside the source-free closure subset".into());
    };
    let Some(argument) = returned.argument.as_ref() else {
        return Ok(ClosurePlan {
            params,
            body: vec![ClosureStatement::Return(None)],
        });
    };
    let argument = argument.without_parentheses();
    let expression = match argument {
        Expression::CallExpression(call) if call.arguments.is_empty() => {
            let Expression::StaticMemberExpression(member) = call.callee.without_parentheses()
            else {
                return Err("closed function is outside the source-free closure subset".into());
            };
            if member.property.name != "charCodeAt" {
                return Err("closed function is outside the source-free closure subset".into());
            }
            let Expression::Identifier(identifier) = member.object.without_parentheses() else {
                return Err("closed function is outside the source-free closure subset".into());
            };
            if !params
                .iter()
                .any(|parameter| parameter == identifier.name.as_str())
            {
                return Err("closed function is outside the source-free closure subset".into());
            }
            ClosureExpression::CharCodeAt {
                parameter: identifier.name.to_string(),
            }
        }
        Expression::BinaryExpression(binary)
            if binary.operator == BinaryOperator::Addition
                && matches!(
                    binary.right.without_parentheses(),
                    Expression::NumericLiteral(literal) if literal.value == 1.0
                ) =>
        {
            let Expression::Identifier(identifier) = binary.left.without_parentheses() else {
                return Err("closed function is outside the source-free closure subset".into());
            };
            if !params
                .iter()
                .any(|parameter| parameter == identifier.name.as_str())
            {
                return Err("closed function is outside the source-free closure subset".into());
            }
            ClosureExpression::AddOne {
                parameter: identifier.name.to_string(),
            }
        }
        Expression::Identifier(identifier)
            if !params
                .iter()
                .any(|parameter| parameter == identifier.name.as_str())
                && is_valid_closure_identifier(identifier.name.as_str()) =>
        {
            ClosureExpression::External {
                name: identifier.name.to_string(),
            }
        }
        _ => return Err("closed function is outside the source-free closure subset".into()),
    };
    Ok(ClosurePlan {
        params,
        body: vec![ClosureStatement::Return(Some(expression))],
    })
}

/// Parse and compile a function expression on the compile-side only. Artifact
/// loading never calls this function and never receives the source text.
pub fn compile_closure_plan(source: &str) -> Result<ClosurePlan, String> {
    let allocator = Allocator::default();
    let wrapped = format!("({source})");
    let parsed =
        oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::script()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err("closed function is not parseable".into());
    }
    let mut statements = parsed.program.body.into_iter();
    let (Some(Statement::ExpressionStatement(statement)), None) =
        (statements.next(), statements.next())
    else {
        return Err("closed function is outside the source-free closure subset".into());
    };
    let Expression::ParenthesizedExpression(inner) = statement.unbox().expression else {
        return Err("closed function is outside the source-free closure subset".into());
    };
    let Expression::FunctionExpression(function) = inner.unbox().expression else {
        return Err("closed function is outside the source-free closure subset".into());
    };
    compile_closure_plan_from_function(&function)
}

/// Check the ASCII identifier subset used by closure parameters and external
/// names. Keeping this conservative makes canonical rendering unambiguous.
pub fn is_valid_closure_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic())
        || !characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
    {
        return false;
    }
    !matches!(
        name,
        "arguments"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "eval"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn is_null_index_call(expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    if !call.arguments.is_empty() {
        return false;
    }
    let Expression::ComputedMemberExpression(member) = call.callee.without_parentheses() else {
        return false;
    };
    matches!(
        member.object.without_parentheses(),
        Expression::NullLiteral(_)
    ) && matches!(
        member.expression.without_parentheses(),
        Expression::NumericLiteral(literal) if literal.value == 0.0
    )
}

/// The fields of a try frame
/// `{handlerByteOffset, handlerDispatchState, errorEntryCount}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TryFrameField {
    HandlerByteOffset,
    HandlerDispatchState,
}

impl TryFrameField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HandlerByteOffset => "handlerByteOffset",
            Self::HandlerDispatchState => "handlerDispatchState",
        }
    }
}

/// The fields of an error record
/// `{wasExceptionHandled, savedByteOffset, savedDispatchState}`: whether an
/// exception is pending, and where a normally entered `finally` resumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorRecordField {
    WasExceptionHandled,
    SavedByteOffset,
    SavedDispatchState,
}

impl ErrorRecordField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WasExceptionHandled => "wasExceptionHandled",
            Self::SavedByteOffset => "savedByteOffset",
            Self::SavedDispatchState => "savedDispatchState",
        }
    }
}

/// The two receiver fields `vmContext` sets from the invocation's `this`:
/// `receiverObject = this == null ? host : Object(this)` and
/// `receiverValue = this`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiverField {
    Object,
    Value,
}

impl ReceiverField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "receiverObject",
            Self::Value => "receiverValue",
        }
    }
}

/// The two fields of a VM call frame
/// `{returnByteOffset, returnDispatchState}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallFrameField {
    ReturnByteOffset,
    ReturnDispatchState,
}

impl CallFrameField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReturnByteOffset => "returnByteOffset",
            Self::ReturnDispatchState => "returnDispatchState",
        }
    }
}

impl SemanticValue {
    /// Direct child values, in evaluation order.
    pub fn children(&self) -> Vec<&SemanticValue> {
        match self {
            Self::StackRead { index } | Self::MemoryRead { slot: index } => vec![index],
            Self::Binary { left, right, .. } => vec![left, right],
            Self::Unary { value, .. } => vec![value],
            Self::PropertyRead { object, key } => vec![object, key],
            Self::ObjectLiteral { properties } => properties.iter().map(|(_, v)| v).collect(),
            Self::ArrayLiteral { elements } => elements.iter().collect(),
            Self::StackSlice { start } => vec![start],
            Self::Construct { callee, arguments } => vec![callee, arguments],
            Self::Delete { object, key } => vec![object, key],
            Self::Call {
                callee,
                this_value,
                arguments,
            } => vec![callee, this_value, arguments],
            Self::CreateThread {
                descriptor,
                byte_offset,
                dispatch_state,
            } => vec![descriptor, byte_offset, dispatch_state],
            Self::Context
            | Self::Operand { .. }
            | Self::Temporary { .. }
            | Self::Literal(_)
            | Self::StackLength
            | Self::Binding { .. }
            | Self::PoppedCallFrame { .. }
            | Self::Receiver { .. }
            | Self::FunctionLiteral { .. }
            | Self::ClosurePlan { .. }
            | Self::PoppedTryFrame { .. }
            | Self::PoppedErrorRecord { .. }
            | Self::PopThrownValue
            | Self::Unknown { .. } => Vec::new(),
        }
    }

    /// Mutable direct child values, in evaluation order.
    pub fn children_mut(&mut self) -> Vec<&mut SemanticValue> {
        match self {
            Self::StackRead { index } | Self::MemoryRead { slot: index } => vec![index],
            Self::Binary { left, right, .. } => vec![left, right],
            Self::Unary { value, .. } => vec![value],
            Self::PropertyRead { object, key } => vec![object, key],
            Self::ObjectLiteral { properties } => properties.iter_mut().map(|(_, v)| v).collect(),
            Self::ArrayLiteral { elements } => elements.iter_mut().collect(),
            Self::StackSlice { start } => vec![start],
            Self::Construct { callee, arguments } => vec![callee, arguments],
            Self::Delete { object, key } => vec![object, key],
            Self::Call {
                callee,
                this_value,
                arguments,
            } => vec![callee, this_value, arguments],
            Self::CreateThread {
                descriptor,
                byte_offset,
                dispatch_state,
            } => vec![descriptor, byte_offset, dispatch_state],
            Self::Context
            | Self::Operand { .. }
            | Self::Temporary { .. }
            | Self::Literal(_)
            | Self::StackLength
            | Self::Binding { .. }
            | Self::PoppedCallFrame { .. }
            | Self::Receiver { .. }
            | Self::FunctionLiteral { .. }
            | Self::ClosurePlan { .. }
            | Self::PoppedTryFrame { .. }
            | Self::PoppedErrorRecord { .. }
            | Self::PopThrownValue
            | Self::Unknown { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipeOperation {
    Let {
        temporary: u32,
        value: SemanticValue,
    },
    Assign {
        temporary: u32,
        value: SemanticValue,
    },
    StackWrite {
        index: SemanticValue,
        value: SemanticValue,
    },
    StackLengthSet {
        value: SemanticValue,
    },
    MemoryWrite {
        slot: SemanticValue,
        value: SemanticValue,
    },
    MemoryInitialize {
        slot: SemanticValue,
    },
    /// `object[key] = value` / `object.key = value` on a non-VM object,
    /// including putting an item into an array.
    PropertyWrite {
        object: SemanticValue,
        key: SemanticValue,
        value: SemanticValue,
    },
    /// `context.callFrameStack.push({returnByteOffset: context.byteOffset,
    /// returnDispatchState: context.dispatchState})`: push a VM call frame that
    /// returns to the PC after this handler's operands and the current
    /// dispatch state.
    PushCallFrame,
    /// `context.callFrameStack.pop()`: pop the VM call frame whose fields are
    /// then read through [`SemanticValue::PoppedCallFrame`].
    PopCallFrame,
    /// `context.byteOffset = value`: continue at this PC after the handler.
    SetProgramCounter {
        value: SemanticValue,
    },
    /// `context.dispatchState = value`: continue in this dispatch state.
    SetDispatchState {
        value: SemanticValue,
    },
    /// `context.tryFrameStack.push({handlerByteOffset, handlerDispatchState,
    /// errorEntryCount: 0})`: enter a `try` whose catch or finally handler is
    /// at this PC and state.
    PushTryFrame {
        byte_offset: SemanticValue,
        dispatch_state: SemanticValue,
    },
    /// `context.tryFrameStack.pop()`: leave the innermost `try`; its fields
    /// are read through [`SemanticValue::PoppedTryFrame`].
    PopTryFrame,
    /// `context.errorStack.push({wasExceptionHandled: false, savedByteOffset:
    /// context.byteOffset, savedDispatchState: context.dispatchState})`:
    /// enter a `finally` normally, resuming after this handler.
    PushFinallyRecord,
    /// `context.errorStack.pop()`; fields are read through
    /// [`SemanticValue::PoppedErrorRecord`].
    PopErrorRecord,
    /// `if (record.wasExceptionHandled) { throw context.thrownValueStack.pop(); }`
    /// on the popped error record: the end of a `finally` rethrows.
    RethrowIfHandled,
    /// `if (record.wasExceptionHandled) { context.thrownValueStack.pop(); }`
    /// on the popped error record: a `catch` without a binding.
    DropThrownIfHandled,
    /// A WASM interpreter's `catch` without a binding: on a handled record it
    /// removes the thrown value under the top one (the top one when it is
    /// alone), keeping the top.
    DropThrownUnderIfHandled,
    /// `vmPendingResult = value`: the running VM function returns `value`
    /// once this handler finishes.
    CompleteThread {
        value: SemanticValue,
    },
    /// `if (condition) { context.byteOffset = byte_offset;
    /// context.dispatchState = dispatch_state; }`: a conditional jump. The
    /// target is VM values; the condition may be a program value.
    JumpIf {
        condition: SemanticValue,
        byte_offset: SemanticValue,
        dispatch_state: SemanticValue,
    },
    Evaluate {
        value: SemanticValue,
    },
    Return {
        value: SemanticValue,
    },
    Throw {
        value: SemanticValue,
    },
    Unknown {
        reason: String,
        source: String,
    },
}

impl RecipeOperation {
    /// The operation's direct values, in evaluation order.
    pub fn values(&self) -> Vec<&SemanticValue> {
        match self {
            Self::Let { value, .. }
            | Self::Assign { value, .. }
            | Self::StackLengthSet { value }
            | Self::Evaluate { value }
            | Self::Return { value }
            | Self::Throw { value }
            | Self::SetProgramCounter { value }
            | Self::SetDispatchState { value }
            | Self::CompleteThread { value }
            | Self::MemoryInitialize { slot: value } => vec![value],
            Self::StackWrite { index, value } | Self::MemoryWrite { slot: index, value } => {
                vec![index, value]
            }
            Self::PropertyWrite { object, key, value } => vec![object, key, value],
            Self::JumpIf {
                condition,
                byte_offset,
                dispatch_state,
            } => vec![condition, byte_offset, dispatch_state],
            Self::PushTryFrame {
                byte_offset,
                dispatch_state,
            } => vec![byte_offset, dispatch_state],
            Self::PushCallFrame
            | Self::PopCallFrame
            | Self::PopTryFrame
            | Self::PushFinallyRecord
            | Self::PopErrorRecord
            | Self::RethrowIfHandled
            | Self::DropThrownIfHandled
            | Self::DropThrownUnderIfHandled
            | Self::Unknown { .. } => Vec::new(),
        }
    }

    /// Mutable direct values, in evaluation order.
    pub fn values_mut(&mut self) -> Vec<&mut SemanticValue> {
        match self {
            Self::Let { value, .. }
            | Self::Assign { value, .. }
            | Self::StackLengthSet { value }
            | Self::Evaluate { value }
            | Self::Return { value }
            | Self::Throw { value }
            | Self::SetProgramCounter { value }
            | Self::SetDispatchState { value }
            | Self::CompleteThread { value }
            | Self::MemoryInitialize { slot: value } => vec![value],
            Self::StackWrite { index, value } | Self::MemoryWrite { slot: index, value } => {
                vec![index, value]
            }
            Self::PropertyWrite { object, key, value } => vec![object, key, value],
            Self::JumpIf {
                condition,
                byte_offset,
                dispatch_state,
            } => vec![condition, byte_offset, dispatch_state],
            Self::PushTryFrame {
                byte_offset,
                dispatch_state,
            } => vec![byte_offset, dispatch_state],
            Self::PushCallFrame
            | Self::PopCallFrame
            | Self::PopTryFrame
            | Self::PushFinallyRecord
            | Self::PopErrorRecord
            | Self::RethrowIfHandled
            | Self::DropThrownIfHandled
            | Self::DropThrownUnderIfHandled
            | Self::Unknown { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeStep {
    pub order: u32,
    pub span: SpanRecord,
    pub operation: RecipeOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerTemplate {
    /// Runtime dispatch key.  This is intentionally not a semantic operation name.
    pub handler_index: u32,
    pub table_key: u32,
    /// Stable semantic identity, normally inherited from the catalogue fingerprint.
    pub semantic_identity: Arc<str>,
    pub source_span: SpanRecord,
    pub status: TemplateStatus,
    pub steps: Vec<RecipeStep>,
    pub unsupported: Vec<UnknownFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownFragment {
    pub span: SpanRecord,
    pub reason: String,
    pub source: String,
}

/// The canonical free name of the JavaScript global object in a recovered
/// program. The symbolic tracer emits a `Binding` with this name verbatim, and
/// the emitted-JavaScript host cleanup unwraps `host["name"]` to `name`.
pub(crate) const HOST: &str = "host";

/// Default structural step bound shared by recipe consumers.
pub const DEFAULT_MAX_RECIPE_STEPS: usize = 100_000;
/// Default depth of one semantic-value tree, counting the root as depth 1.
pub const DEFAULT_MAX_RECIPE_EXPRESSION_DEPTH: usize = 256;
/// Default number of semantic-value nodes visited in one expression tree.
pub const DEFAULT_MAX_RECIPE_EXPRESSION_WORK: usize = 100_000;

/// Explicit structural bounds for [`admit_recipe`].
///
/// The step and expression-work limits are independent: a recipe may have many
/// shallow steps, or one expression with many nodes. Depth is measured on one
/// value tree, with the root at depth 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeBounds {
    pub max_steps: usize,
    pub max_expression_depth: usize,
    pub max_expression_work: usize,
}

impl Default for RecipeBounds {
    fn default() -> Self {
        Self {
            max_steps: DEFAULT_MAX_RECIPE_STEPS,
            max_expression_depth: DEFAULT_MAX_RECIPE_EXPRESSION_DEPTH,
            max_expression_work: DEFAULT_MAX_RECIPE_EXPRESSION_WORK,
        }
    }
}

/// The first structural reason a recipe cannot be executed.
///
/// This is the shared admission vocabulary. Each interpreter maps it to its
/// own error type, so messages stay specific to that interpreter while the
/// admission decision remains identical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// The template status is not [`TemplateStatus::Executable`].
    TemplateStatus { status: TemplateStatus },
    /// Extraction recorded an unsupported fragment even though the status is
    /// executable.
    UnsupportedFragment { reason: String, source: String },
    /// The recipe has more steps than the configured bound.
    StepLimit { steps: usize, limit: usize },
    /// Step orders are not strictly increasing.
    StepOrder { step: u32, previous: u32 },
    /// A step is the fail-closed [`RecipeOperation::Unknown`] operation.
    UnknownOperation { step: u32, reason: String },
    /// A semantic value is the fail-closed [`SemanticValue::Unknown`] value.
    UnknownValue { step: u32, reason: String },
    /// The execution context cannot be materialized as a JavaScript value.
    ContextValue { step: u32 },
    /// A temporary is read before the `Let` that defines it.
    TemporaryReadBeforeLet { step: u32, id: u32 },
    /// A temporary is the target of `Assign` before its `Let`.
    TemporaryAssignedBeforeLet { step: u32, id: u32 },
    /// A temporary has more than one `Let`.
    DuplicateTemporary { step: u32, id: u32 },
    /// One expression tree exceeded the configured node-work bound.
    ExpressionWork {
        step: u32,
        work: usize,
        limit: usize,
    },
    /// One expression tree exceeded the configured depth bound.
    ExpressionDepth {
        step: u32,
        depth: usize,
        limit: usize,
    },
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemplateStatus { status } => write!(f, "handler template is {status}"),
            Self::UnsupportedFragment { reason, source } => {
                write!(f, "unsupported handler fragment: {reason}: {source}")
            }
            Self::StepLimit { steps, limit } => {
                write!(f, "recipe has {steps} steps, exceeding step limit {limit}")
            }
            Self::StepOrder { step, previous } => {
                write!(f, "step {step} is not ordered after step {previous}")
            }
            Self::UnknownOperation { step, reason } => {
                write!(f, "step {step} is an unknown operation: {reason}")
            }
            Self::UnknownValue { step, reason } => {
                write!(f, "step {step} has an unknown value: {reason}")
            }
            Self::ContextValue { step } => {
                write!(f, "step {step} materializes the execution context")
            }
            Self::TemporaryReadBeforeLet { step, id } => {
                write!(f, "step {step} reads temporary {id} before its let step")
            }
            Self::TemporaryAssignedBeforeLet { step, id } => {
                write!(f, "step {step} assigns temporary {id} before its let step")
            }
            Self::DuplicateTemporary { step, id } => {
                write!(f, "step {step} defines temporary {id} more than once")
            }
            Self::ExpressionWork { step, work, limit } => {
                write!(
                    f,
                    "step {step} expression work {work} exceeds limit {limit}"
                )
            }
            Self::ExpressionDepth { step, depth, limit } => {
                write!(
                    f,
                    "step {step} expression depth {depth} exceeds limit {limit}"
                )
            }
        }
    }
}

impl std::error::Error for AdmissionError {}

/// The single structural admission check for handler recipes.
///
/// Consumers keep their own runtime policies (for example concrete execution
/// limits), but status, unsupported fragments, step order, temporary
/// definitions, unknown values, and expression depth/work have one answer.
/// The walk is iterative, so a deeply nested malformed value is rejected
/// without recursing over it.
///
/// Precedence, from first to last:
/// 1. template status;
/// 2. the first unsupported fragment;
/// 3. the step-count bound;
/// 4. each step's source order;
/// 5. `Unknown` operations;
/// 6. `Assign` of an undefined temporary;
/// 7. the step's semantic values, visited left to right (work, depth, context,
///    unknown value, then temporary-before-let at each node);
/// 8. duplicate `Let` definitions, after that `Let`'s value is valid.
pub fn admit_recipe(
    template: &HandlerTemplate,
    bounds: RecipeBounds,
) -> Result<(), AdmissionError> {
    if template.status != TemplateStatus::Executable {
        return Err(AdmissionError::TemplateStatus {
            status: template.status,
        });
    }
    if let Some(unknown) = template.unsupported.first() {
        return Err(AdmissionError::UnsupportedFragment {
            reason: unknown.reason.clone(),
            source: unknown.source.clone(),
        });
    }
    if template.steps.len() > bounds.max_steps {
        return Err(AdmissionError::StepLimit {
            steps: template.steps.len(),
            limit: bounds.max_steps,
        });
    }

    let mut temporaries = BTreeSet::new();
    let mut previous_order = None;
    for step in &template.steps {
        if let Some(previous) = previous_order {
            if step.order <= previous {
                return Err(AdmissionError::StepOrder {
                    step: step.order,
                    previous,
                });
            }
        }
        previous_order = Some(step.order);

        if let RecipeOperation::Unknown { reason, .. } = &step.operation {
            return Err(AdmissionError::UnknownOperation {
                step: step.order,
                reason: reason.clone(),
            });
        }

        match &step.operation {
            RecipeOperation::Let { temporary, value } => {
                admit_value(value, step.order, &temporaries, bounds)?;
                if !temporaries.insert(*temporary) {
                    return Err(AdmissionError::DuplicateTemporary {
                        step: step.order,
                        id: *temporary,
                    });
                }
            }
            RecipeOperation::Assign { temporary, value } => {
                if !temporaries.contains(temporary) {
                    return Err(AdmissionError::TemporaryAssignedBeforeLet {
                        step: step.order,
                        id: *temporary,
                    });
                }
                admit_value(value, step.order, &temporaries, bounds)?;
            }
            RecipeOperation::Unknown { .. } => unreachable!("unknown operation returned above"),
            other => {
                for value in other.values() {
                    admit_value(value, step.order, &temporaries, bounds)?;
                }
            }
        }
    }
    Ok(())
}

fn admit_value(
    root: &SemanticValue,
    step: u32,
    temporaries: &BTreeSet<u32>,
    bounds: RecipeBounds,
) -> Result<(), AdmissionError> {
    let mut pending = vec![(root, 1usize)];
    let mut work = 0usize;
    while let Some((value, depth)) = pending.pop() {
        work = work.saturating_add(1);
        if work > bounds.max_expression_work {
            return Err(AdmissionError::ExpressionWork {
                step,
                work,
                limit: bounds.max_expression_work,
            });
        }
        if depth > bounds.max_expression_depth {
            return Err(AdmissionError::ExpressionDepth {
                step,
                depth,
                limit: bounds.max_expression_depth,
            });
        }
        match value {
            SemanticValue::Context => return Err(AdmissionError::ContextValue { step }),
            SemanticValue::Unknown { reason, .. } => {
                return Err(AdmissionError::UnknownValue {
                    step,
                    reason: reason.clone(),
                });
            }
            SemanticValue::Temporary { id } if !temporaries.contains(id) => {
                return Err(AdmissionError::TemporaryReadBeforeLet { step, id: *id });
            }
            // Push children in reverse so the leftmost subtree is visited first.
            other => pending.extend(
                other
                    .children()
                    .into_iter()
                    .rev()
                    .map(|child| (child, depth + 1)),
            ),
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TemplateCoverage {
    pub executable: u32,
    pub partial: u32,
    pub unsupported: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerTemplateReport {
    pub schema_version: String,
    pub source_hash: String,
    pub table: ProvenOperationTable,
    pub handlers: Vec<HandlerTemplate>,
    pub coverage: TemplateCoverage,
}

#[derive(Debug)]
pub enum HandlerTemplateError {
    Parse(String),
    NoProvenTable,
    TableIdentityMismatch(String),
}

impl fmt::Display for HandlerTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "handler-template parse failed: {e}"),
            Self::NoProvenTable => {
                f.write_str("pipeline report contains no proven operation table")
            }
            Self::TableIdentityMismatch(e) => write!(f, "proven operation table mismatch: {e}"),
        }
    }
}
impl std::error::Error for HandlerTemplateError {}

mod build;
mod extract;

pub(crate) use build::*;
pub use build::POST_TO_WORKER;
pub use extract::*;
