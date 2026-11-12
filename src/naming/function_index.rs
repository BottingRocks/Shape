//! Complete, source-stamped function and main-variable inventory for a
//! recovered Shape-style program capture.
//!
//! Unlike [`super::scanner`], which recognizes generator-input roles, this
//! module answers a simpler and deliberately exhaustive question: **which
//! callable functions exist in this exact source, how do they relate through
//! lexical call edges, and which data bindings does the broadest reachable main
//! scope own?**
//!
//! The inventory is meant to feed small semantic-analysis batches:
//!
//! * every function is listed exactly once, including anonymous callbacks,
//!   object/class methods and arrow functions;
//! * every identity is a UTF-8 byte span plus a source-derived id, never a
//!   generated spelling or a count threshold;
//! * source bodies stay addressable through spans; the report only retains
//!   bounded previews, so the 631 KB root is not duplicated into every card;
//! * a `fn@` is a top-level entry only when it is actually invoked at the
//!   program top level; a `main(host)` nested in a wrapper, an IIFE and an
//!   ordinary `mainProgram()` call are distinguished by call reachability;
//! * call edges are emitted only when the callee resolves to a single proven
//!   identity (a declaration, or a lone function-valued definition with no
//!   other write); conflicting, dynamic or unknown writes keep their exact
//!   callee source as an explicitly `ambiguous_assignment` / `dynamic_assignment`
//!   / `external_identifier` unresolved edge rather than naming a first
//!   candidate. Every declaration origin of a symbol is inventoried, so sloppy
//!   duplicate function declarations and repeated `var` declarators sharing one
//!   SymbolId count as separate definitions rather than being missed;
//! * main variables are the direct owned bindings of the main scope, with
//!   initializer/assignment classes separated and closed array/object literals
//!   recorded without evaluating anything. Each definition's class is its own
//!   AST location, so a condition/block/loop-nested initializer is not
//!   unconditional. A value is `proven_static` only for a single unconditional
//!   definition; conditional, nested, compound/update or repeated writes are
//!   `mutable_state` and list every definition in `alternatives` instead of a
//!   guessed current literal;
//! * a function's own name binding and the bindings that hold it are kept
//!   apart: a declaration's identifier is its holder, while in
//!   `let a = function b(){}` the holder `a` and the self-name `b` are two
//!   symbols. Each [`ValueHolder`] keeps its own [`HolderProof`];
//! * classes are [`ClassRecord`] values, not extra functions. Their
//!   constructor, methods, getters, setters and field functions stay
//!   [`FunctionRecord`]s linked by id, and a class declaration initializes its
//!   MAIN binding.
//!
//! The extractor never executes recovered JavaScript. It consumes only Oxc's
//! AST and semantic scoping for one exact source string.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, ScopeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use serde::{Deserialize, Serialize};

use crate::signals::extract::bindings::literal_value;
use crate::signals::schema::{BindingKind, BindingRole, ProgramProperties, ProgramPropertiesLimits, SourceSpan};

/// Version of the serialized [`FunctionIndex`] contract.
///
/// Version 2 adds class records, function/class value holders, the
/// `Class` classification and declaration kind, and `class_id` on held values.
/// Every version-1 field is kept.
pub const FUNCTION_INDEX_SCHEMA_VERSION: u32 = 2;

// ===========================================================================
// Errors and limits
// ===========================================================================

/// Bounded extraction budget. Every bound either rejects the input
/// deterministically or truncates a report field with an explicit diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionIndexLimits {
    pub max_source_bytes: usize,
    pub max_ast_nodes: usize,
    pub max_functions: usize,
    pub max_bindings: usize,
    pub max_call_edges: usize,
    pub max_unresolved_calls: usize,
    pub max_assignments_per_binding: usize,
    pub max_alias_depth: usize,
    pub max_snippet_bytes: usize,
    pub max_array_elements: usize,
    pub max_object_entries: usize,
}

impl Default for FunctionIndexLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_ast_nodes: 4_000_000,
            max_functions: 262_144,
            max_bindings: 262_144,
            max_call_edges: 1_000_000,
            max_unresolved_calls: 250_000,
            max_assignments_per_binding: 4_096,
            max_alias_depth: 16,
            max_snippet_bytes: 280,
            max_array_elements: 64,
            max_object_entries: 64,
        }
    }
}

impl FunctionIndexLimits {
    pub fn program_properties_limits(self) -> ProgramPropertiesLimits {
        ProgramPropertiesLimits {
            max_source_bytes: self.max_source_bytes,
            max_ast_nodes: self.max_ast_nodes,
            max_functions: self.max_functions,
            max_bindings: self.max_bindings,
            max_array_elements: crate::signals::schema::ProgramPropertiesLimits::default().max_array_elements,
            max_emissions: crate::signals::schema::ProgramPropertiesLimits::default().max_emissions,
            max_retained_source_bytes: crate::signals::schema::ProgramPropertiesLimits::default()
                .max_retained_source_bytes,
        }
    }
}

/// A typed inventory failure. Every variant is a refusal, not a partial guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FunctionIndexError {
    InvalidInput(String),
    Parse(String),
    Limit { what: &'static str, limit: usize },
    Missing { what: &'static str },
}

impl std::fmt::Display for FunctionIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(f, "invalid function-index input: {value}"),
            Self::Parse(value) => write!(f, "function-index parse failed: {value}"),
            Self::Limit { what, limit } => {
                write!(f, "function-index limit exceeded for {what} ({limit})")
            }
            Self::Missing { what } => write!(f, "function-index required fact is missing: {what}"),
        }
    }
}

impl std::error::Error for FunctionIndexError {}

// ===========================================================================
// Public report types
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionKind {
    Declaration,
    Expression,
    Arrow,
    ObjectMethod,
    ClassMethod,
    Getter,
    Setter,
    Constructor,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    Explicit,
    Binding,
    Property,
    Assignment,
    Anonymous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    Call,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallResolution {
    Identifier,
    Alias,
    FunctionExpression,
    ArrowFunction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainReferenceAccess {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentClass {
    MainTopLevel,
    MainBlock,
    NestedFunction,
    NestedTemplate,
    Unwritten,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainVariableClass {
    Literal,
    ArrayLiteral,
    ObjectLiteral,
    Function,
    ConfiguredResult,
    HostReference,
    Alias,
    RuntimeState,
    Unknown,
    /// A class value: a class declaration (initialized by the declaration
    /// itself) or a single unconditional class-expression definition.
    Class,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingDeclarationKind {
    Variable,
    Function,
    Parameter,
    Catch,
    Other,
    /// The binding declared by a class declaration.
    Class,
}

impl BindingDeclarationKind {
    fn from_binding_kind(kind: BindingKind) -> Self {
        match kind {
            BindingKind::Variable => Self::Variable,
            BindingKind::Function => Self::Function,
            BindingKind::Parameter => Self::Parameter,
            BindingKind::Catch => Self::Catch,
            BindingKind::Class => Self::Class,
            BindingKind::Other => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassKind {
    Declaration,
    Expression,
}

/// How a binding comes to hold a function or class value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HolderRelation {
    /// `function f(){}` / `class K {}`: the declaration binds its own name.
    Declaration,
    /// `let x = function(){}` / `let k = class {}`.
    Initializer,
    /// `x = function(){}` written after the declaration.
    Assignment,
    /// `let a = f`: the definition is another binding that resolves to the value.
    Alias,
}

/// Whether a holder's value is proven to be one identity. Only `Proven` is a
/// proof; the other variants keep the link visible without asserting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HolderProof {
    /// A single definition and no other write: the binding holds this value.
    Proven,
    /// This is the only known candidate, but other writes could change it.
    Possible,
    /// The binding has two or more distinct candidate values.
    Ambiguous,
    /// Writes the inventory cannot see through (compound, update,
    /// destructuring, `for…of`, dynamic values).
    Dynamic,
}

/// A binding whose definition links it to one function or class value. A
/// holder is a separate editable binding from the value's own name: in
/// `let a = function b(){}` the holder is `a` while the function's own name
/// binding is `b`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueHolder {
    pub binding: BindingSummary,
    /// Set when the holder is a direct binding of the MAIN scope.
    pub main_variable_id: Option<String>,
    pub relation: HolderRelation,
    /// The declaration, initializer or assignment that links the holder.
    pub definition: SourceSpan,
    pub proof: HolderProof,
}

/// One class declaration or expression. Classes are values, not callables:
/// they are not counted in `function_count`, and their constructor, methods,
/// getters and setters stay ordinary [`FunctionRecord`]s linked by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassRecord {
    /// Source-derived stable identity (`class@<full_start>..<full_end>`).
    pub id: String,
    pub ordinal: u32,
    pub kind: ClassKind,
    /// The class's own identifier, as a label only.
    pub label: Option<String>,
    pub full_span: SourceSpan,
    pub identifier_span: Option<SourceSpan>,
    /// The binding declared by the class's own identifier: the outer binding
    /// for a declaration, the inner self-name for a named class expression.
    pub name_binding: Option<BindingSummary>,
    /// The MAIN binding that directly holds this class through a proven
    /// declaration, initializer or assignment, when exactly one does.
    pub holder_main_variable_id: Option<String>,
    /// Every binding linked to this class, proven or not.
    pub holders: Vec<ValueHolder>,
    pub constructor_id: Option<String>,
    pub method_ids: Vec<String>,
    pub getter_ids: Vec<String>,
    pub setter_ids: Vec<String>,
    /// Functions held directly by class fields (`x = () => {}`).
    pub field_function_ids: Vec<String>,
    pub superclass_preview: Option<String>,
    pub parent_function: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
}

/// A bounded exact-source snippet plus the resolved participants of an
/// expression. No runtime value is ever computed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExprSummary {
    pub span: SourceSpan,
    pub source_preview: String,
    pub source_truncated: bool,
    pub kind: String,
    pub literal: Option<crate::signals::schema::LiteralValue>,
    pub function_id: Option<String>,
    pub binding: Option<BindingSummary>,
    pub member_root: Option<String>,
}

/// A resolved lexical binding referenced by an expression.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingSummary {
    pub symbol: u32,
    pub name: String,
    pub identifier: SourceSpan,
}

/// Closed array literal data, recorded only when every element is static or
/// explicitly counted. Nested complex elements are previews, never executed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArrayHeld {
    pub length: usize,
    pub truncated: bool,
    pub elements: Vec<String>,
    pub literal_elements: Option<Vec<crate::signals::schema::LiteralValue>>,
    pub nested_array_elements: usize,
    pub object_elements: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectEntryHeld {
    pub key: String,
    pub kind: String,
    pub literal: Option<crate::signals::schema::LiteralValue>,
    pub value_preview: String,
    pub function_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectHeld {
    pub keys: Vec<String>,
    pub entries: Vec<ObjectEntryHeld>,
    pub truncated: bool,
}

/// One definition site of a main binding, kept as explicit source evidence when
/// the binding's value is not a single proven constant. The class records the
/// exact scope/condition under which the write runs; it is never a claim about
/// the runtime value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeldAlternative {
    pub span: SourceSpan,
    pub line: u32,
    pub class: AssignmentClass,
    pub kind: String,
    pub literal: Option<crate::signals::schema::LiteralValue>,
    pub array: Option<ArrayHeld>,
    pub object: Option<ObjectHeld>,
    pub function_id: Option<String>,
    #[serde(default)]
    pub class_id: Option<String>,
    pub source_preview: String,
}

/// The static shape a main binding holds. This is descriptive evidence, not an
/// evaluated value.
///
/// `proven_static` is true only when a single unconditional definition
/// determines the binding and nothing else writes it; `classification` then says
/// whether that shape is a closed static value (`Literal`/`ArrayLiteral`/
/// `ObjectLiteral`) or a runtime/configured result. When it is false the value
/// fields are cleared and the competing definitions are listed in
/// `alternatives`: a conditional, nested, repeated or compound/update write is
/// runtime state, never a proven current constant. The representative
/// `source`/`source_preview` in that case is the initializer if present
/// (otherwise the first definition), labelled as evidence only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeldSummary {
    pub proven_static: bool,
    pub source: Option<SourceSpan>,
    pub source_preview: Option<String>,
    pub kind: String,
    pub literal: Option<crate::signals::schema::LiteralValue>,
    pub array: Option<ArrayHeld>,
    pub object: Option<ObjectHeld>,
    pub function_id: Option<String>,
    /// The class value, for a class declaration or a proven class definition.
    #[serde(default)]
    pub class_id: Option<String>,
    pub target_main_variable: Option<String>,
    pub host_expression: Option<String>,
    pub alternatives: Vec<HeldAlternative>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterRecord {
    pub name: String,
    pub symbol: Option<u32>,
    pub identifier: Option<SourceSpan>,
    pub pattern_span: SourceSpan,
    pub rest: bool,
    pub destructured: bool,
    pub default_preview: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnBindingRecord {
    pub symbol: u32,
    pub name: String,
    pub kind: BindingDeclarationKind,
    pub declaration: SourceSpan,
    pub identifier: SourceSpan,
    pub references: usize,
    pub writes: usize,
    pub initializer: Option<ExprSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MainReferenceRecord {
    pub main_variable_id: String,
    pub name: String,
    pub role: Option<String>,
    pub span: SourceSpan,
    pub line: u32,
    pub access: MainReferenceAccess,
    pub via_alias: Vec<String>,
    pub aliased_symbol: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssignmentSite {
    pub span: SourceSpan,
    pub line: u32,
    pub class: AssignmentClass,
    pub enclosing_function: Option<String>,
    pub rhs: ExprSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsumerRecord {
    pub function_id: String,
    pub function_label: Option<String>,
    pub reads: usize,
    pub writes: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionArgumentContext {
    pub callee_source: String,
    pub argument_index: u32,
    pub call_span: SourceSpan,
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionRecord {
    /// Source-derived stable identity (`fn@<full_start>..<full_end>`).
    pub id: String,
    pub ordinal: u32,
    pub label: Option<String>,
    pub binding_label: Option<String>,
    pub label_source: LabelSource,
    pub kind: FunctionKind,
    pub full_span: SourceSpan,
    pub identifier_span: Option<SourceSpan>,
    pub body_span: Option<SourceSpan>,
    pub start_line: u32,
    pub end_line: u32,
    pub body_start_line: Option<u32>,
    pub body_end_line: Option<u32>,
    pub parent_function: Option<String>,
    pub parent_function_label: Option<String>,
    pub lexical_depth: u32,
    pub is_main: bool,
    /// True when this function is invoked directly at the program top level
    /// (an ordinary top-level call, a wrapper entry or an IIFE), not merely
    /// reachable from one.
    pub is_top_level_entry: bool,
    pub params: Vec<ParameterRecord>,
    pub own_bindings: Vec<OwnBindingRecord>,
    /// Reads/writes of MAIN bindings whose innermost enclosing function is this
    /// one. Both lists are always empty for the MAIN function itself: its own
    /// body's accesses are not captures. For every write of a binding, including
    /// MAIN's own, use [`MainVariableRecord::writes`] and
    /// [`MainVariableRecord::assignments`].
    pub main_reads: Vec<MainReferenceRecord>,
    pub main_writes: Vec<MainReferenceRecord>,
    pub captured_main_variable_ids: Vec<String>,
    pub callee_ids: Vec<String>,
    pub caller_ids: Vec<String>,
    pub callee_count: usize,
    /// Number of *lexically resolved* callers. Zero does not mean dead: the
    /// function may still be invoked through an unresolved dynamic call (a
    /// callback, a property call, an external identifier).
    pub caller_count: usize,
    pub unresolved_call_count: usize,
    pub role: Option<String>,
    pub role_basis: Option<String>,
    pub signal_indices: Vec<u32>,
    pub wave_index: Option<u32>,
    pub direct_signal_index: Option<u32>,
    pub declaration_context: Option<String>,
    pub assigned_properties: Vec<String>,
    pub argument_contexts: Vec<FunctionArgumentContext>,
    /// The binding declared by the function's own identifier. For a declaration
    /// it is the same symbol as the binding that holds the function (in MAIN,
    /// the [`MainVariableRecord`] symbol). For a named expression it is the
    /// inner self-name, a different symbol from any outer holder.
    #[serde(default)]
    pub name_binding: Option<BindingSummary>,
    /// Every binding linked to this function by a declaration, initializer,
    /// assignment or alias, each with its own proof status.
    #[serde(default)]
    pub holders: Vec<ValueHolder>,
    /// The class whose constructor, method, getter, setter or field this is.
    #[serde(default)]
    pub class_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallEdge {
    pub caller: Option<String>,
    pub caller_label: Option<String>,
    pub callee: String,
    pub callee_label: Option<String>,
    pub kind: CallKind,
    pub resolution: CallResolution,
    pub span: SourceSpan,
    pub line: u32,
    pub callee_source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnresolvedCall {
    pub caller: Option<String>,
    pub caller_label: Option<String>,
    pub span: SourceSpan,
    pub line: u32,
    /// Exact callee expression source.
    pub callee_source: String,
    /// `dynamic_member`, `dynamic_computed`, `external_identifier`,
    /// `other_expression`.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MainVariableRecord {
    pub id: String,
    pub symbol: u32,
    pub name: String,
    pub declaration_kind: BindingDeclarationKind,
    pub declaration: SourceSpan,
    pub identifier: SourceSpan,
    pub start_line: u32,
    pub end_line: u32,
    pub references: usize,
    /// Every resolved write of this binding anywhere in the program, including
    /// MAIN's own body. This, not the per-function `main_writes` captures, is
    /// the total write count.
    pub writes: usize,
    pub initializer: Option<ExprSummary>,
    /// Summary class of the initializer, the assignments, or the declaration
    /// that initializes a function/class binding. `Unwritten` only when none
    /// of these exist.
    pub assignment_class: AssignmentClass,
    /// Every plain `x = value` assignment, wherever it runs (MAIN's own body
    /// included).
    pub assignments: Vec<AssignmentSite>,
    pub classification: MainVariableClass,
    pub held: HeldSummary,
    pub alias_of: Option<String>,
    pub role: Option<String>,
    pub role_basis: Option<String>,
    pub signal_indices: Vec<u32>,
    pub wave_indices: Vec<u32>,
    pub captured_by: Vec<ConsumerRecord>,
    pub structural_notes: Vec<String>,
    /// No initializer, no write, and not a function/class declaration (which
    /// initializes its binding itself).
    pub unwritten: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MainFunctionRecord {
    pub function_id: String,
    pub label: Option<String>,
    pub full_span: SourceSpan,
    pub body_span: Option<SourceSpan>,
    pub identifier_span: Option<SourceSpan>,
    pub start_line: u32,
    pub end_line: u32,
    pub direct_binding_count: usize,
    pub is_top_level_entry: bool,
    pub reachable_from_top_level_entry: bool,
    pub returns_object_with_init: bool,
    pub top_level_call: Option<SourceSpan>,
    pub selection_basis: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionIndexCoverage {
    pub function_count: usize,
    pub named_count: usize,
    pub anonymous_count: usize,
    pub declaration_count: usize,
    pub expression_count: usize,
    pub arrow_count: usize,
    pub method_count: usize,
    pub duplicate_function_spans: usize,
    pub main_binding_count: usize,
    pub call_edge_count: usize,
    pub unresolved_call_count: usize,
    pub unresolved_calls_recorded: usize,
    pub functions_with_callers: usize,
    /// Functions with no *lexically resolved* caller. This is not a dead-code
    /// set: dynamic callbacks keep a resolved edge count of zero on purpose.
    pub functions_without_callers: usize,
    pub captured_main_reference_count: usize,
    pub assignment_class_counts: BTreeMap<String, usize>,
    pub classification_counts: BTreeMap<String, usize>,
    /// Class declarations and expressions. Not included in `function_count`.
    #[serde(default)]
    pub class_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionIndex {
    pub schema_version: u32,
    pub source: crate::signals::schema::SourceIdentity,
    pub main: Option<MainFunctionRecord>,
    pub functions: Vec<FunctionRecord>,
    /// Every class declaration and expression, once, in source order.
    #[serde(default)]
    pub classes: Vec<ClassRecord>,
    pub call_edges: Vec<CallEdge>,
    pub unresolved_calls: Vec<UnresolvedCall>,
    pub main_variables: Vec<MainVariableRecord>,
    pub coverage: FunctionIndexCoverage,
    pub diagnostics: Vec<crate::signals::schema::Diagnostic>,
}

// ===========================================================================
// Internal site model
// ===========================================================================

#[derive(Clone, Debug)]
struct Site {
    node_id: NodeId,
    span: Span,
    body_span: Option<Span>,
    kind: FunctionKind,
    identifier_span: Option<Span>,
    label: Option<String>,
    binding_label: Option<String>,
    label_source: LabelSource,
    params: Vec<ParameterRecord>,
    parent: Option<usize>,
    depth: u32,
    declaration_context: Option<String>,
    /// The symbol declared by the function's own identifier, if any.
    name_symbol: Option<SymbolId>,
}

/// One class declaration or expression, with its member function sites.
#[derive(Clone, Debug)]
struct ClassSite {
    span: Span,
    kind: ClassKind,
    identifier_span: Option<Span>,
    label: Option<String>,
    name_symbol: Option<SymbolId>,
    superclass: Option<Span>,
    parent: Option<usize>,
    constructor: Option<usize>,
    methods: Vec<usize>,
    getters: Vec<usize>,
    setters: Vec<usize>,
    fields: Vec<usize>,
}

/// Which kind of value identity a symbol is resolved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ValueTarget {
    Function,
    Class,
}

/// How a symbol is written. Only `Plain` (`x = rhs`) carries a value expression;
/// compound and update writes produce a value the inventory does not compute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriteKind {
    Plain,
    Compound,
    Update,
}

#[derive(Clone, Copy, Debug)]
struct WriteSite<'a> {
    node_id: NodeId,
    span: Span,
    kind: WriteKind,
    rhs: Option<&'a Expression<'a>>,
}

/// One value definition of a binding: a declaration initializer or a plain
/// assignment write, paired with the exact scope/condition class under which it
/// runs. The class is the definition's actual AST location, never assumed top
/// level because an initializer merely exists.
#[derive(Clone, Copy, Debug)]
struct Definition<'a> {
    span: Span,
    expression: &'a Expression<'a>,
    class: AssignmentClass,
}

/// Soundness classification of a symbol as a callable identity (or, for a
/// [`ValueTarget::Class`] resolution, a class identity; the index is then a
/// class index). Only `Unique` is a proof; the other variants are explicit
/// reasons an edge stays unresolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymbolFunctionResolution {
    /// A single function-valued definition and no other write.
    Unique(usize),
    /// Exactly one function candidate, but other writes could change it.
    Possible(usize),
    /// Two or more distinct function-valued definitions.
    Ambiguous,
    /// Writes the inventory cannot see through and no known function candidate.
    Dynamic,
    /// No function-valued definition and no other write.
    None,
}

struct Builder<'s, 'a> {
    source: &'s str,
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    limits: FunctionIndexLimits,
    line_starts: Vec<u32>,
    sites: Vec<Site>,
    by_node: HashMap<NodeId, usize>,
    classes: Vec<ClassSite>,
    class_by_node: HashMap<NodeId, usize>,
    class_of_site: HashMap<usize, usize>,
    scope_owner: HashMap<ScopeId, Option<usize>>,
    writes_by_symbol: HashMap<SymbolId, Vec<WriteSite<'a>>>,
    own_symbols: Vec<Vec<SymbolId>>,
    main_symbols: BTreeSet<SymbolId>,
    main_direct_scope: Option<ScopeId>,
    main_var_ids: HashMap<SymbolId, String>,
    role_by_identifier_start: HashMap<u32, BindingRole>,
    function_roles: HashMap<u32, (String, String, Vec<u32>, Option<u32>, Option<u32>)>,
    properties: Option<ProgramProperties>,
    diagnostics: Vec<crate::signals::schema::Diagnostic>,
    unresolved_total: usize,
    main_index: Option<usize>,
    main_selection_basis: String,
    caller_links: HashMap<usize, BTreeSet<usize>>,
    callee_links: HashMap<usize, BTreeSet<usize>>,
    unresolved_counts: HashMap<usize, usize>,
    call_edges_snapshot: Vec<CallEdge>,
    /// `resolve_symbol_value` results within one top-level resolution, keyed
    /// by (symbol, alias depth, target). Aliases that point at each other
    /// (`a = b; b = a`, each with several writes) otherwise re-explore the same
    /// symbols at every depth up to `max_alias_depth`: exponential in the
    /// write fan-out. Cleared at depth 0, so a result never outlives the
    /// builder state it was computed from.
    value_memo: std::cell::RefCell<HashMap<(SymbolId, usize, ValueTarget), SymbolFunctionResolution>>,
}

impl<'s, 'a> Builder<'s, 'a> {
    fn new(
        source: &'s str,
        scoping: &'a Scoping,
        nodes: &'a AstNodes<'a>,
        limits: FunctionIndexLimits,
        properties: Option<ProgramProperties>,
    ) -> Result<Self, FunctionIndexError> {
        if source.trim().is_empty() {
            return Err(FunctionIndexError::InvalidInput(
                "source is empty".to_string(),
            ));
        }
        if source.len() > limits.max_source_bytes {
            return Err(FunctionIndexError::Limit {
                what: "source bytes",
                limit: limits.max_source_bytes,
            });
        }
        if nodes.len() > limits.max_ast_nodes {
            return Err(FunctionIndexError::Limit {
                what: "AST nodes",
                limit: limits.max_ast_nodes,
            });
        }
        if scoping.symbols_len() > limits.max_bindings {
            return Err(FunctionIndexError::Limit {
                what: "bindings",
                limit: limits.max_bindings,
            });
        }
        let line_starts = line_starts(source);
        let mut builder = Self {
            source,
            scoping,
            nodes,
            limits,
            line_starts,
            sites: Vec::new(),
            by_node: HashMap::new(),
            classes: Vec::new(),
            class_by_node: HashMap::new(),
            class_of_site: HashMap::new(),
            scope_owner: HashMap::new(),
            writes_by_symbol: HashMap::new(),
            own_symbols: Vec::new(),
            main_symbols: BTreeSet::new(),
            main_direct_scope: None,
            main_var_ids: HashMap::new(),
            role_by_identifier_start: HashMap::new(),
            function_roles: HashMap::new(),
            properties,
            diagnostics: Vec::new(),
            unresolved_total: 0,
            main_index: None,
            main_selection_basis: String::new(),
            caller_links: HashMap::new(),
            callee_links: HashMap::new(),
            unresolved_counts: HashMap::new(),
            call_edges_snapshot: Vec::new(),
            value_memo: Default::default(),
        };
        builder.collect_sites()?;
        builder.collect_write_sites();
        builder.compute_parents();
        builder.collect_classes()?;
        builder.compute_scope_ownership();
        builder.collect_own_symbols();
        builder.collect_role_hints();
        builder.select_main();
        builder.collect_main_var_ids();
        Ok(builder)
    }

    fn line(&self, offset: u32) -> u32 {
        self.line_starts.partition_point(|&start| start <= offset) as u32
    }

    fn exact(&self, span: Span) -> &'s str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn preview_span(&self, span: Span) -> (String, bool) {
        let text = self.exact(span);
        if text.len() <= self.limits.max_snippet_bytes {
            return (text.to_string(), false);
        }
        let mut end = self.limits.max_snippet_bytes;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        (format!("{}…", &text[..end]), true)
    }

    fn snippet(&self, span: Span) -> String {
        self.preview_span(span).0
    }

    fn site_id(span: Span) -> String {
        format!("fn@{}..{}", span.start, span.end)
    }

    fn main_var_id(span: Span) -> String {
        format!("mv@{}..{}", span.start, span.end)
    }

    fn class_id(span: Span) -> String {
        format!("class@{}..{}", span.start, span.end)
    }

    fn binding_summary(&self, symbol: SymbolId) -> BindingSummary {
        BindingSummary {
            symbol: symbol.index() as u32,
            name: self.scoping.symbol_name(symbol).to_string(),
            identifier: SourceSpan::from(self.scoping.symbol_span(symbol)),
        }
    }

    // -- function discovery -------------------------------------------------

    fn collect_sites(&mut self) -> Result<(), FunctionIndexError> {
        for (node_id, node) in self.nodes.iter_enumerated() {
            match node.kind() {
                AstKind::Function(function) => {
                    if self.sites.len() >= self.limits.max_functions {
                        return Err(FunctionIndexError::Limit {
                            what: "functions",
                            limit: self.limits.max_functions,
                        });
                    }
                    let kind = self.function_kind(node_id, function);
                    let (label, binding_label, label_source) =
                        self.function_label(node_id, function, kind);
                    let params = self.parameters(&function.params, function);
                    let declaration_context = self.declaration_context(node_id);
                    let site = Site {
                        node_id,
                        span: function.span,
                        body_span: function.body.as_ref().map(|body| body.span),
                        kind,
                        identifier_span: function.id.as_ref().map(|id| id.span),
                        label,
                        binding_label,
                        label_source,
                        params,
                        parent: None,
                        depth: 0,
                        declaration_context,
                        name_symbol: function.id.as_ref().and_then(|id| id.symbol_id.get()),
                    };
                    self.by_node.insert(node_id, self.sites.len());
                    self.sites.push(site);
                }
                AstKind::ArrowFunctionExpression(arrow) => {
                    if self.sites.len() >= self.limits.max_functions {
                        return Err(FunctionIndexError::Limit {
                            what: "functions",
                            limit: self.limits.max_functions,
                        });
                    }
                    let (label, binding_label, label_source) = self.arrow_label(node_id);
                    let params = self.arrow_parameters(arrow);
                    let declaration_context = self.declaration_context(node_id);
                    let site = Site {
                        node_id,
                        span: arrow.span,
                        body_span: Some(arrow.body.span),
                        kind: FunctionKind::Arrow,
                        identifier_span: None,
                        label,
                        binding_label,
                        label_source,
                        params,
                        parent: None,
                        depth: 0,
                        declaration_context,
                        name_symbol: None,
                    };
                    self.by_node.insert(node_id, self.sites.len());
                    self.sites.push(site);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn function_kind(&self, node_id: NodeId, function: &Function<'_>) -> FunctionKind {
        match function.r#type {
            FunctionType::FunctionDeclaration => FunctionKind::Declaration,
            FunctionType::TSDeclareFunction => FunctionKind::Other,
            FunctionType::FunctionExpression | FunctionType::TSEmptyBodyFunctionExpression => {
                let parent = self.nodes.kind(self.nodes.parent_id(node_id));
                match parent {
                    AstKind::MethodDefinition(method) => match method.kind {
                        MethodDefinitionKind::Constructor => FunctionKind::Constructor,
                        MethodDefinitionKind::Get => FunctionKind::Getter,
                        MethodDefinitionKind::Set => FunctionKind::Setter,
                        MethodDefinitionKind::Method => FunctionKind::ClassMethod,
                    },
                    AstKind::ObjectProperty(property)
                        if property.method
                            || property.kind == PropertyKind::Get
                            || property.kind == PropertyKind::Set =>
                    {
                        match property.kind {
                            PropertyKind::Get => FunctionKind::Getter,
                            PropertyKind::Set => FunctionKind::Setter,
                            _ => FunctionKind::ObjectMethod,
                        }
                    }
                    _ => FunctionKind::Expression,
                }
            }
        }
    }

    fn function_label(
        &self,
        node_id: NodeId,
        function: &Function<'_>,
        kind: FunctionKind,
    ) -> (Option<String>, Option<String>, LabelSource) {
        if let Some(id) = &function.id {
            return (Some(id.name.to_string()), None, LabelSource::Explicit);
        }
        let _ = kind;
        if let Some(property) = self.property_label(node_id) {
            return (Some(property), None, LabelSource::Property);
        }
        if let Some(label) = self.declaration_label(node_id) {
            return (None, Some(label), LabelSource::Binding);
        }
        (None, None, LabelSource::Anonymous)
    }

    fn arrow_label(&self, node_id: NodeId) -> (Option<String>, Option<String>, LabelSource) {
        if let Some(property) = self.property_label(node_id) {
            return (Some(property), None, LabelSource::Property);
        }
        if let Some(label) = self.declaration_label(node_id) {
            return (None, Some(label), LabelSource::Binding);
        }
        (None, None, LabelSource::Anonymous)
    }

    fn declaration_label(&self, node_id: NodeId) -> Option<String> {
        let parent = self.nodes.parent_id(node_id);
        match self.nodes.kind(parent) {
            AstKind::VariableDeclarator(declarator) => match &declarator.id {
                BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
                _ => None,
            },
            AstKind::AssignmentExpression(assignment) => match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                    Some(identifier.name.to_string())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn property_label(&self, node_id: NodeId) -> Option<String> {
        let parent = self.nodes.parent_id(node_id);
        match self.nodes.kind(parent) {
            AstKind::ObjectProperty(property) => property_key_name(&property.key),
            AstKind::MethodDefinition(method) => property_key_name(&method.key),
            _ => None,
        }
    }

    fn declaration_context(&self, node_id: NodeId) -> Option<String> {
        let parent = self.nodes.parent_id(node_id);
        Some(match self.nodes.kind(parent) {
            AstKind::VariableDeclarator(declarator) => format!(
                "variable_declarator:{}",
                match &declarator.id {
                    BindingPattern::BindingIdentifier(identifier) => identifier.name.as_str(),
                    _ => "<pattern>",
                }
            ),
            AstKind::ObjectProperty(property) => format!(
                "object_property:{}",
                property_key_name(&property.key).unwrap_or_else(|| "<computed>".to_string())
            ),
            AstKind::MethodDefinition(method) => format!(
                "class_method:{}",
                property_key_name(&method.key).unwrap_or_else(|| "<computed>".to_string())
            ),
            AstKind::CallExpression(_) => "call_argument".to_string(),
            AstKind::NewExpression(_) => "new_argument".to_string(),
            AstKind::ReturnStatement(_) => "returned_function".to_string(),
            AstKind::FunctionBody(_) => "function_body_declaration".to_string(),
            AstKind::Program(_) => "program_declaration".to_string(),
            _ => "other".to_string(),
        })
    }

    fn parameters(
        &self,
        params: &FormalParameters<'_>,
        _function: &Function<'_>,
    ) -> Vec<ParameterRecord> {
        let mut out = Vec::new();
        for parameter in &params.items {
            self.collect_pattern(
                &parameter.pattern,
                false,
                parameter.initializer.as_deref(),
                &mut out,
            );
        }
        if let Some(rest) = &params.rest {
            self.collect_pattern(&rest.rest.argument, true, None, &mut out);
        }
        out
    }

    fn arrow_parameters(&self, arrow: &ArrowFunctionExpression<'_>) -> Vec<ParameterRecord> {
        let mut out = Vec::new();
        for parameter in &arrow.params.items {
            self.collect_pattern(
                &parameter.pattern,
                false,
                parameter.initializer.as_deref(),
                &mut out,
            );
        }
        if let Some(rest) = &arrow.params.rest {
            self.collect_pattern(&rest.rest.argument, true, None, &mut out);
        }
        out
    }

    fn collect_pattern(
        &self,
        pattern: &BindingPattern<'_>,
        rest: bool,
        initializer: Option<&Expression<'_>>,
        out: &mut Vec<ParameterRecord>,
    ) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => out.push(ParameterRecord {
                name: identifier.name.to_string(),
                symbol: identifier
                    .symbol_id
                    .get()
                    .map(|symbol| symbol.index() as u32),
                identifier: Some(SourceSpan::new(identifier.span.start, identifier.span.end)),
                pattern_span: SourceSpan::new(pattern.span().start, pattern.span().end),
                rest,
                destructured: false,
                default_preview: initializer.map(|expr| self.snippet(expr.span())),
            }),
            BindingPattern::AssignmentPattern(assignment) => {
                self.collect_pattern(&assignment.left, rest, Some(&assignment.right), out);
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    self.collect_pattern(&property.value, rest, None, out);
                }
                if let Some(rest_element) = &object.rest {
                    self.collect_pattern(&rest_element.argument, true, None, out);
                }
                if object.properties.is_empty() && object.rest.is_none() {
                    out.push(ParameterRecord {
                        name: "<object-pattern>".to_string(),
                        symbol: None,
                        identifier: None,
                        pattern_span: SourceSpan::new(pattern.span().start, pattern.span().end),
                        rest,
                        destructured: true,
                        default_preview: initializer.map(|expr| self.snippet(expr.span())),
                    });
                }
            }
            BindingPattern::ArrayPattern(array) => {
                for element in array.elements.iter().flatten() {
                    self.collect_pattern(element, rest, None, out);
                }
                if let Some(rest_element) = &array.rest {
                    self.collect_pattern(&rest_element.argument, true, None, out);
                }
                if array.elements.is_empty() && array.rest.is_none() {
                    out.push(ParameterRecord {
                        name: "<array-pattern>".to_string(),
                        symbol: None,
                        identifier: None,
                        pattern_span: SourceSpan::new(pattern.span().start, pattern.span().end),
                        rest,
                        destructured: true,
                        default_preview: initializer.map(|expr| self.snippet(expr.span())),
                    });
                }
            }
        }
    }

    fn collect_write_sites(&mut self) {
        for (node_id, node) in self.nodes.iter_enumerated() {
            match node.kind() {
                AstKind::AssignmentExpression(assignment) => {
                    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                    else {
                        continue;
                    };
                    let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.scoping)
                    else {
                        continue;
                    };
                    let plain =
                        assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign;
                    self.writes_by_symbol
                        .entry(symbol)
                        .or_default()
                        .push(WriteSite {
                            node_id,
                            span: assignment.span,
                            kind: if plain {
                                WriteKind::Plain
                            } else {
                                WriteKind::Compound
                            },
                            rhs: Some(&assignment.right),
                        });
                }
                AstKind::UpdateExpression(update) => {
                    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) =
                        &update.argument
                    else {
                        continue;
                    };
                    let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.scoping)
                    else {
                        continue;
                    };
                    self.writes_by_symbol
                        .entry(symbol)
                        .or_default()
                        .push(WriteSite {
                            node_id,
                            span: update.span,
                            kind: WriteKind::Update,
                            rhs: None,
                        });
                }
                _ => {}
            }
        }
        for sites in self.writes_by_symbol.values_mut() {
            sites.sort_by_key(|site| site.span.start);
        }
    }

    /// Every class declaration and expression, with its member functions linked
    /// by the exact `MethodDefinition`/`PropertyDefinition` that holds them.
    /// Classes share the function budget: they are separate records, not extra
    /// function sites.
    fn collect_classes(&mut self) -> Result<(), FunctionIndexError> {
        for (node_id, node) in self.nodes.iter_enumerated() {
            let AstKind::Class(class) = node.kind() else {
                continue;
            };
            if self.classes.len() >= self.limits.max_functions {
                return Err(FunctionIndexError::Limit {
                    what: "classes",
                    limit: self.limits.max_functions,
                });
            }
            let mut site = ClassSite {
                span: class.span,
                kind: match class.r#type {
                    ClassType::ClassDeclaration => ClassKind::Declaration,
                    ClassType::ClassExpression => ClassKind::Expression,
                },
                identifier_span: class.id.as_ref().map(|id| id.span),
                label: class.id.as_ref().map(|id| id.name.to_string()),
                name_symbol: class.id.as_ref().and_then(|id| id.symbol_id.get()),
                superclass: class.super_class.as_ref().map(GetSpan::span),
                parent: self.enclosing_function_index(node_id),
                constructor: None,
                methods: Vec::new(),
                getters: Vec::new(),
                setters: Vec::new(),
                fields: Vec::new(),
            };
            for element in &class.body.body {
                match element {
                    ClassElement::MethodDefinition(method) => {
                        let Some(&index) = self.by_node.get(&method.value.node_id.get()) else {
                            continue;
                        };
                        match method.kind {
                            MethodDefinitionKind::Constructor => site.constructor = Some(index),
                            MethodDefinitionKind::Method => site.methods.push(index),
                            MethodDefinitionKind::Get => site.getters.push(index),
                            MethodDefinitionKind::Set => site.setters.push(index),
                        }
                    }
                    ClassElement::PropertyDefinition(property) => {
                        let index = match &property.value {
                            Some(Expression::FunctionExpression(function)) => {
                                self.by_node.get(&function.node_id.get()).copied()
                            }
                            Some(Expression::ArrowFunctionExpression(arrow)) => {
                                self.by_node.get(&arrow.node_id.get()).copied()
                            }
                            _ => None,
                        };
                        if let Some(index) = index {
                            site.fields.push(index);
                        }
                    }
                    _ => {}
                }
            }
            let class_index = self.classes.len();
            for member in site
                .constructor
                .iter()
                .chain(&site.methods)
                .chain(&site.getters)
                .chain(&site.setters)
                .chain(&site.fields)
            {
                self.class_of_site.insert(*member, class_index);
            }
            self.class_by_node.insert(node_id, class_index);
            self.classes.push(site);
        }
        Ok(())
    }

    fn compute_parents(&mut self) {
        for index in 0..self.sites.len() {
            let mut current = self.sites[index].node_id;
            let mut found = None;
            for _ in 0..1_024 {
                let parent = self.nodes.parent_id(current);
                if parent == current {
                    break;
                }
                current = parent;
                if let Some(&parent_index) = self.by_node.get(&current) {
                    found = Some(parent_index);
                    break;
                }
            }
            self.sites[index].parent = found;
            self.sites[index].depth = match found {
                Some(parent_index) => self.sites[parent_index].depth + 1,
                None => 0,
            };
        }
    }

    fn scope_owner_of(&mut self, scope: ScopeId) -> Option<usize> {
        if let Some(owner) = self.scope_owner.get(&scope) {
            return *owner;
        }
        let node_id = self.scoping.get_node_id(scope);
        let owner = if let Some(&index) = self.by_node.get(&node_id) {
            Some(index)
        } else {
            match self.scoping.scope_parent_id(scope) {
                Some(parent) => self.scope_owner_of(parent),
                None => None,
            }
        };
        self.scope_owner.insert(scope, owner);
        owner
    }

    fn compute_scope_ownership(&mut self) {
        let scopes: Vec<ScopeId> = self.scoping.scope_descendants_from_root().collect();
        for scope in scopes {
            self.scope_owner_of(scope);
        }
    }

    fn collect_own_symbols(&mut self) {
        self.own_symbols = vec![Vec::new(); self.sites.len()];
        let scopes: Vec<ScopeId> = self.scoping.scope_descendants_from_root().collect();
        for scope in scopes {
            let owner = self.scope_owner.get(&scope).copied().flatten();
            let Some(owner) = owner else {
                continue;
            };
            for symbol in self.scoping.iter_bindings_in(scope) {
                self.own_symbols[owner].push(symbol);
            }
        }
        for symbols in &mut self.own_symbols {
            symbols.sort_by_key(|symbol| self.scoping.symbol_span(*symbol).start);
            symbols.dedup();
        }
    }

    fn collect_role_hints(&mut self) {
        let Some(properties) = &self.properties else {
            return;
        };
        for binding in &properties.bindings {
            if !binding.role.is_unknown() {
                self.role_by_identifier_start
                    .insert(binding.identifier.start, binding.role);
            }
        }
        for helper in &properties.helpers {
            self.function_roles.insert(
                helper.function.span.start,
                (
                    format!("{:?}", helper.role),
                    "program_properties.helper".to_string(),
                    Vec::new(),
                    None,
                    None,
                ),
            );
        }
        if let Some(waves) = &properties.waves {
            for wave in &waves.waves {
                self.function_roles.insert(
                    wave.function.span.start,
                    (
                        "WaveCollector".to_string(),
                        "program_properties.wave_collector".to_string(),
                        wave.signal_indices
                            .iter()
                            .map(|index| *index as u32)
                            .collect(),
                        Some(wave.index as u32),
                        None,
                    ),
                );
            }
            for slot in &waves.direct_slots {
                if let Some(callback) = &slot.callback {
                    self.function_roles.insert(
                        callback.span.start,
                        (
                            "DirectSignalCallback".to_string(),
                            "program_properties.direct_slot_callback".to_string(),
                            vec![slot.signal as u32],
                            None,
                            Some(slot.signal as u32),
                        ),
                    );
                }
            }
        }
        for signal in &properties.signals {
            if let Some(collector) = &signal.collector {
                let entry = self
                    .function_roles
                    .entry(collector.span.start)
                    .or_insert_with(|| {
                        (
                            "SignalCollector".to_string(),
                            "program_properties.signal_collector".to_string(),
                            Vec::new(),
                            signal.wave,
                            None,
                        )
                    });
                if !entry.2.contains(&signal.signal) {
                    entry.2.push(signal.signal);
                }
                if entry.3.is_none() {
                    entry.3 = signal.wave;
                }
            }
        }
        for (role, function) in [
            (
                Some("SessionInit"),
                properties.session.as_ref().and_then(|s| s.init.as_ref()),
            ),
            (
                Some("SessionConstructor"),
                properties.session.as_ref().map(|s| &s.constructor),
            ),
            (
                Some("SessionTeardown"),
                properties
                    .session
                    .as_ref()
                    .and_then(|s| s.teardown.as_ref()),
            ),
        ] {
            if let (Some(role), Some(function)) = (role, function) {
                self.function_roles
                    .entry(function.span.start)
                    .or_insert_with(|| {
                        (
                            role.to_string(),
                            "program_properties.session".to_string(),
                            Vec::new(),
                            None,
                            None,
                        )
                    });
            }
        }
    }

    // -- main selection -----------------------------------------------------

    fn select_main(&mut self) {
        if let Some(properties) = &self.properties {
            let main_span = properties.program_scope.main.span;
            if let Some(index) = self.site_index_by_span(main_span) {
                self.main_index = Some(index);
                self.main_selection_basis =
                    "program_properties.broadest_owned_state_scope".to_string();
                self.collect_main_symbols(index);
                return;
            }
        }
        // Fallback: the broadest reachable owned-state scope from a top-level
        // entry. This is used for synthetic/small sources without a full
        // generator-input match.
        let edges = self.resolved_call_sites();
        let mut top_level_entries = BTreeSet::new();
        let mut reachable = BTreeSet::new();
        for (caller, callee) in &edges {
            if caller.is_none() {
                top_level_entries.insert(*callee);
            }
        }
        let mut queue: Vec<usize> = top_level_entries.iter().copied().collect();
        while let Some(index) = queue.pop() {
            if !reachable.insert(index) {
                continue;
            }
            for (caller, callee) in &edges {
                if *caller == Some(index) && !reachable.contains(callee) {
                    queue.push(*callee);
                }
            }
        }
        let candidates: Vec<usize> = if reachable.is_empty() {
            (0..self.sites.len()).collect()
        } else {
            reachable.iter().copied().collect()
        };
        let mut best: Option<(usize, usize, u32, u32)> = None;
        for index in candidates {
            let returns_init = self.returns_object_with_init(index);
            let owned = self.own_symbols[index].len();
            let span_len = self.sites[index].span.end - self.sites[index].span.start;
            let score = (
                usize::from(returns_init),
                owned,
                span_len,
                self.sites[index].span.start,
            );
            match best {
                None => best = Some((score.0, score.1, score.2, score.3)),
                Some(current) => {
                    if (score.0, score.1, score.2) > (current.0, current.1, current.2)
                        || ((score.0, score.1, score.2) == (current.0, current.1, current.2)
                            && score.3 < current.3)
                    {
                        best = Some((score.0, score.1, score.2, score.3));
                    }
                }
            }
        }
        if let Some((_, _, _, start)) = best {
            if let Some(index) = self.sites.iter().position(|site| site.span.start == start) {
                self.main_index = Some(index);
                self.main_selection_basis =
                    "reachable_owned_state_scope_from_top_level_entry".to_string();
                self.collect_main_symbols(index);
            }
        }
    }

    fn site_index_by_span(&self, span: SourceSpan) -> Option<usize> {
        self.sites
            .iter()
            .position(|site| site.span.start == span.start && site.span.end == span.end)
    }

    fn collect_main_symbols(&mut self, index: usize) {
        let node_id = self.sites[index].node_id;
        let mut best: Option<(usize, ScopeId)> = None;
        for scope in self.scoping.scope_descendants_from_root() {
            if self.scoping.get_node_id(scope) != node_id {
                continue;
            }
            let count = self.scoping.iter_bindings_in(scope).count();
            if best.is_none_or(|(best_count, _)| count > best_count) {
                best = Some((count, scope));
            }
        }
        self.main_direct_scope = best.map(|(_, scope)| scope);
        self.main_symbols = match best {
            Some((_, scope)) => self.scoping.iter_bindings_in(scope).collect(),
            None => self.own_symbols[index].iter().copied().collect(),
        };
    }

    fn returns_object_with_init(&self, index: usize) -> bool {
        if let Some(properties) = &self.properties {
            if properties.program_scope.main.span.start == self.sites[index].span.start {
                return properties.program_scope.returns_init_object;
            }
        }
        let Some(body) = self.sites[index].body_span else {
            return false;
        };
        let mut found = false;
        for node in self.nodes.iter() {
            let AstKind::ReturnStatement(ret) = node.kind() else {
                continue;
            };
            if ret.span.start < body.start || ret.span.end > body.end {
                continue;
            }
            if self.enclosing_function_index(ret.node_id.get()) != Some(index) {
                continue;
            }
            let Some(argument) = &ret.argument else {
                continue;
            };
            let Expression::ObjectExpression(object) = argument else {
                continue;
            };
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                if property_key_name(&property.key).as_deref() == Some("init") {
                    found = true;
                }
            }
        }
        found
    }

    // -- call graph ---------------------------------------------------------

    fn resolved_call_sites(&mut self) -> Vec<(Option<usize>, usize)> {
        let mut sites = Vec::new();
        for (node_id, node) in self.nodes.iter_enumerated() {
            let (callee, is_new) = match node.kind() {
                AstKind::CallExpression(call) => (&call.callee, false),
                AstKind::NewExpression(new) => (&new.callee, true),
                _ => continue,
            };
            let _ = is_new;
            let caller = self.enclosing_function_index(node_id);
            if let Some(callee_index) = self.resolve_callee(callee) {
                sites.push((caller, callee_index));
            }
        }
        sites
    }

    fn resolve_callee(&self, callee: &Expression<'_>) -> Option<usize> {
        match callee {
            Expression::Identifier(identifier) => {
                let symbol = crate::analysis::identifier_symbol(identifier, self.scoping)?;
                match self.resolve_symbol_function(symbol) {
                    SymbolFunctionResolution::Unique(index) => Some(index),
                    _ => None,
                }
            }
            Expression::FunctionExpression(function) => {
                self.by_node.get(&function.node_id.get()).copied()
            }
            Expression::ArrowFunctionExpression(arrow) => {
                self.by_node.get(&arrow.node_id.get()).copied()
            }
            _ => None,
        }
    }

    /// Why a callee expression did not resolve to a unique function identity.
    /// Identifier callees distinguish a conflicting reassignment (`ambiguous`)
    /// from a write the inventory cannot see through (`dynamic`); both stay
    /// unresolved rather than guessing a callee.
    fn unresolved_reason(&self, callee: &Expression<'_>) -> String {
        match callee {
            Expression::Identifier(identifier) => {
                match crate::analysis::identifier_symbol(identifier, self.scoping) {
                    Some(symbol) => match self.resolve_symbol_function(symbol) {
                        SymbolFunctionResolution::Possible(_) => "possible_assignment".to_string(),
                        SymbolFunctionResolution::Ambiguous => "ambiguous_assignment".to_string(),
                        SymbolFunctionResolution::Dynamic => "dynamic_assignment".to_string(),
                        SymbolFunctionResolution::None => "external_identifier".to_string(),
                        SymbolFunctionResolution::Unique(_) => "unresolved_identifier".to_string(),
                    },
                    None => "external_identifier".to_string(),
                }
            }
            Expression::StaticMemberExpression(_) => "dynamic_member".to_string(),
            Expression::ComputedMemberExpression(_) => "dynamic_computed".to_string(),
            _ => "other_expression".to_string(),
        }
    }

    fn symbol_function_id(&self, symbol: SymbolId) -> Option<String> {
        match self.resolve_symbol_function(symbol) {
            SymbolFunctionResolution::Unique(index) => Some(Self::site_id(self.sites[index].span)),
            _ => None,
        }
    }

    /// Resolve a symbol to a single callable identity only through a sound proof:
    /// a function declaration, or a lone function-valued definition with no
    /// other write. Multiple distinct candidates are `Ambiguous`; any write the
    /// inventory cannot see through (compound/update/for-of targets, dynamic
    /// values, unknown aliases) is `Dynamic`. The candidate and write
    /// inventories stay visible in the main-variable records; they are never
    /// collapsed to a first-by-source pick.
    fn resolve_symbol_function(&self, symbol: SymbolId) -> SymbolFunctionResolution {
        self.resolve_symbol_value(symbol, 0, ValueTarget::Function)
    }

    /// The class analogue of [`Self::resolve_symbol_function`], under the same
    /// proof rules: a class declaration, or a lone class-valued definition with
    /// no other write.
    fn resolve_symbol_class(&self, symbol: SymbolId) -> SymbolFunctionResolution {
        self.resolve_symbol_value(symbol, 0, ValueTarget::Class)
    }

    fn symbol_class_id(&self, symbol: SymbolId) -> Option<String> {
        match self.resolve_symbol_class(symbol) {
            SymbolFunctionResolution::Unique(index) => {
                Some(Self::class_id(self.classes[index].span))
            }
            _ => None,
        }
    }

    /// The class an expression denotes: a class expression, or an identifier
    /// whose binding is proven to hold one class.
    fn resolve_class_expression(&self, expression: &Expression<'_>) -> Option<usize> {
        match expression {
            Expression::ClassExpression(class) => {
                self.class_by_node.get(&class.node_id.get()).copied()
            }
            Expression::Identifier(identifier) => {
                let symbol = crate::analysis::identifier_symbol(identifier, self.scoping)?;
                match self.resolve_symbol_class(symbol) {
                    SymbolFunctionResolution::Unique(index) => Some(index),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn resolve_symbol_value(
        &self,
        symbol: SymbolId,
        depth: usize,
        target: ValueTarget,
    ) -> SymbolFunctionResolution {
        // Depth only grows along a resolution, so (symbol, depth, target) is
        // never re-entered while it is being computed.
        if depth == 0 {
            self.value_memo.borrow_mut().clear();
        }
        let key = (symbol, depth, target);
        if let Some(&cached) = self.value_memo.borrow().get(&key) {
            return cached;
        }
        let resolution = self.resolve_symbol_value_uncached(symbol, depth, target);
        self.value_memo.borrow_mut().insert(key, resolution);
        resolution
    }

    fn resolve_symbol_value_uncached(
        &self,
        symbol: SymbolId,
        depth: usize,
        target: ValueTarget,
    ) -> SymbolFunctionResolution {
        if depth > self.limits.max_alias_depth {
            return SymbolFunctionResolution::Dynamic;
        }
        let mut functions: Vec<usize> = Vec::new();
        let mut ambiguous = false;
        let mut dynamic = false;

        // Every declaration origin of the symbol, not only the first. Sloppy-mode
        // duplicate function declarations and repeated `var` declarators share one
        // SymbolId; each origin is a separate definition, so conflicting origins
        // stay ambiguous instead of collapsing to the first by source order.
        for declaration_node in self.scoping.symbol_declarations(symbol) {
            match self.nodes.kind(declaration_node) {
                AstKind::Function(function) if target == ValueTarget::Function => {
                    match self.by_node.get(&function.node_id.get()) {
                        Some(&index) => functions.push(index),
                        None => ambiguous = true,
                    }
                }
                AstKind::Class(class) if target == ValueTarget::Class => {
                    match self.class_by_node.get(&class.node_id.get()) {
                        Some(&index) => functions.push(index),
                        None => ambiguous = true,
                    }
                }
                AstKind::VariableDeclarator(declarator) => {
                    if let Some(initializer) = declarator.init.as_ref() {
                        match self.definition_resolution(initializer, symbol, depth, target) {
                            SymbolFunctionResolution::Unique(index) => functions.push(index),
                            SymbolFunctionResolution::Ambiguous => ambiguous = true,
                            SymbolFunctionResolution::Possible(_)
                            | SymbolFunctionResolution::Dynamic
                            | SymbolFunctionResolution::None => {
                                dynamic = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let modelled = self
            .writes_by_symbol
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for site in modelled {
            match site.kind {
                WriteKind::Plain => {
                    let Some(rhs) = site.rhs else {
                        dynamic = true;
                        continue;
                    };
                    match self.definition_resolution(rhs, symbol, depth, target) {
                        SymbolFunctionResolution::Unique(index) => functions.push(index),
                        SymbolFunctionResolution::Ambiguous => ambiguous = true,
                        SymbolFunctionResolution::Possible(_)
                        | SymbolFunctionResolution::Dynamic
                        | SymbolFunctionResolution::None => {
                            dynamic = true;
                        }
                    }
                }
                WriteKind::Compound | WriteKind::Update => dynamic = true,
            }
        }
        let total_writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        if total_writes > modelled.len() {
            dynamic = true;
        }

        if ambiguous {
            return SymbolFunctionResolution::Ambiguous;
        }
        functions.sort_unstable();
        functions.dedup();
        match (functions.first().copied(), functions.len(), dynamic) {
            (Some(_), count, _) if count > 1 => SymbolFunctionResolution::Ambiguous,
            (Some(index), _, false) => SymbolFunctionResolution::Unique(index),
            (Some(index), _, true) => SymbolFunctionResolution::Possible(index),
            (None, _, true) => SymbolFunctionResolution::Dynamic,
            (None, _, false) => SymbolFunctionResolution::None,
        }
    }

    fn definition_resolution(
        &self,
        expression: &Expression<'_>,
        symbol: SymbolId,
        depth: usize,
        target: ValueTarget,
    ) -> SymbolFunctionResolution {
        match expression {
            Expression::FunctionExpression(function) if target == ValueTarget::Function => {
                match self.by_node.get(&function.node_id.get()) {
                    Some(&index) => SymbolFunctionResolution::Unique(index),
                    None => SymbolFunctionResolution::Ambiguous,
                }
            }
            Expression::ArrowFunctionExpression(arrow) if target == ValueTarget::Function => {
                match self.by_node.get(&arrow.node_id.get()) {
                    Some(&index) => SymbolFunctionResolution::Unique(index),
                    None => SymbolFunctionResolution::Ambiguous,
                }
            }
            Expression::ClassExpression(class) if target == ValueTarget::Class => {
                match self.class_by_node.get(&class.node_id.get()) {
                    Some(&index) => SymbolFunctionResolution::Unique(index),
                    None => SymbolFunctionResolution::Ambiguous,
                }
            }
            Expression::Identifier(identifier) => {
                match crate::analysis::identifier_symbol(identifier, self.scoping) {
                    Some(alias) if alias != symbol => {
                        self.resolve_symbol_value(alias, depth + 1, target)
                    }
                    _ => SymbolFunctionResolution::None,
                }
            }
            _ => SymbolFunctionResolution::None,
        }
    }

    fn enclosing_function_index(&self, node_id: NodeId) -> Option<usize> {
        let mut current = node_id;
        for _ in 0..2_048 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if let Some(&index) = self.by_node.get(&current) {
                return Some(index);
            }
        }
        None
    }

    fn collect_main_var_ids(&mut self) {
        for symbol in &self.main_symbols {
            let declaration = self
                .nodes
                .get_node(self.scoping.symbol_declaration(*symbol));
            self.main_var_ids
                .insert(*symbol, Self::main_var_id(declaration.span()));
        }
    }

    // -- main variables -----------------------------------------------------

    fn build_main_variables(&mut self) -> Vec<MainVariableRecord> {
        let main_index = self.main_index;
        let mut records = Vec::new();
        for symbol in self.main_symbols.clone() {
            let declaration_node = self.scoping.symbol_declaration(symbol);
            let declaration = self.nodes.get_node(declaration_node);
            let declaration_span = declaration.span();
            let identifier_span = self.scoping.symbol_span(symbol);
            let kind = self.binding_kind(symbol);
            let initializer = self.initializer(symbol).map(|expr| self.expr_summary(expr));
            let total_writes = self
                .scoping
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .count();
            let write_sites: Vec<WriteSite<'a>> = self
                .writes_by_symbol
                .get(&symbol)
                .cloned()
                .unwrap_or_default();
            let mut assignments = Vec::new();
            let mut assignment_class = AssignmentClass::Unwritten;
            // A function or class declaration initializes its own binding.
            let declaration_initializes = matches!(
                self.nodes.kind(declaration_node),
                AstKind::Function(_) | AstKind::Class(_)
            );
            if initializer.is_some() || declaration_initializes {
                // The summary class comes from the initializer's actual location,
                // not from the mere presence of an initializer.
                assignment_class = self.classify_assignment(
                    main_index,
                    self.enclosing_function_index(declaration_node),
                    declaration_node,
                );
            }
            for site in &write_sites {
                let enclosing = self.enclosing_function_index(site.node_id);
                let class = self.classify_assignment(main_index, enclosing, site.node_id);
                if assignment_class == AssignmentClass::Unwritten
                    || (assignment_class != AssignmentClass::MainTopLevel
                        && class == AssignmentClass::MainTopLevel)
                {
                    assignment_class = class;
                }
                if site.kind != WriteKind::Plain {
                    continue;
                }
                let Some(rhs) = site.rhs else {
                    continue;
                };
                if assignments.len() >= self.limits.max_assignments_per_binding {
                    self.diagnostics.push(crate::signals::schema::Diagnostic {
                        code: "assignment_truncated".to_string(),
                        message: format!(
                            "binding {} exceeded {} assignments",
                            self.scoping.symbol_name(symbol),
                            self.limits.max_assignments_per_binding
                        ),
                        span: Some(SourceSpan::from(site.span)),
                    });
                    break;
                }
                assignments.push(AssignmentSite {
                    span: SourceSpan::from(site.span),
                    line: self.line(site.span.start),
                    class,
                    enclosing_function: enclosing
                        .map(|index| Self::site_id(self.sites[index].span)),
                    rhs: self.expr_summary(rhs),
                });
            }
            assignments.sort_by_key(|assignment| assignment.span.start);
            let (classification, held, alias_of) = self.classify_main_variable(symbol, kind);
            let unwritten = initializer.is_none() && total_writes == 0 && !declaration_initializes;
            let role = self
                .role_by_identifier_start
                .get(&identifier_span.start)
                .map(|role| format!("{role:?}"));
            let role_basis = role
                .as_ref()
                .map(|_| "program_properties.structural_role".to_string());
            let (signal_indices, wave_indices) = self.signal_wave_for_symbol(symbol);
            records.push(MainVariableRecord {
                id: Self::main_var_id(declaration_span),
                symbol: symbol.index() as u32,
                name: self.scoping.symbol_name(symbol).to_string(),
                declaration_kind: self.declaration_kind(symbol),
                declaration: SourceSpan::from(declaration_span),
                identifier: SourceSpan::from(identifier_span),
                start_line: self.line(declaration_span.start),
                end_line: self.line(declaration_span.end),
                references: self.scoping.get_resolved_reference_ids(symbol).len(),
                writes: total_writes,
                initializer,
                assignment_class,
                assignments,
                classification,
                held,
                alias_of,
                role,
                role_basis,
                signal_indices,
                wave_indices,
                captured_by: Vec::new(),
                structural_notes: Vec::new(),
                unwritten,
            });
        }
        records.sort_by_key(|record| record.declaration.start);
        records
    }

    fn classify_assignment(
        &self,
        main_index: Option<usize>,
        enclosing: Option<usize>,
        node_id: NodeId,
    ) -> AssignmentClass {
        match (main_index, enclosing) {
            (Some(main), Some(enclosing)) if main == enclosing => {
                if self.has_conditional_ancestor(node_id, main) {
                    AssignmentClass::MainBlock
                } else {
                    AssignmentClass::MainTopLevel
                }
            }
            (_, Some(enclosing)) => {
                let site = &self.sites[enclosing];
                match site.kind {
                    FunctionKind::Expression | FunctionKind::Arrow | FunctionKind::ObjectMethod => {
                        AssignmentClass::NestedTemplate
                    }
                    _ => AssignmentClass::NestedFunction,
                }
            }
            _ => AssignmentClass::MainTopLevel,
        }
    }

    /// True when a block or a conditional/loop context lies between `node_id`
    /// and the body of function `function_index`. A braceless `if (c) x = y`
    /// consequent is as conditional as a braced one.
    fn has_conditional_ancestor(&self, node_id: NodeId, function_index: usize) -> bool {
        let body = self.sites[function_index].body_span;
        let Some(body) = body else {
            return false;
        };
        let mut current = node_id;
        for _ in 0..2_048 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
            if current == self.sites[function_index].node_id {
                return false;
            }
            match self.nodes.kind(current) {
                AstKind::FunctionBody(function_body) if function_body.span == body => return false,
                kind if is_conditional_context(&kind) => return true,
                _ => {}
            }
        }
        false
    }

    /// Resolve the root of a member chain to a host global, following main
    /// variable aliases (e.g. `$KEY_5 = $KEY_343["document"]` where `$KEY_343`
    /// holds `window`). The alias chain is structural source evidence, not a
    /// runtime value.
    fn expression_host_root(&self, expression: &Expression<'_>, depth: usize) -> Option<String> {
        if depth > self.limits.max_alias_depth {
            return None;
        }
        let root = member_root(expression)?;
        if is_host_global(root) {
            return Some(root.to_string());
        }
        if let Expression::Identifier(identifier) = expression {
            if let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.scoping) {
                if self.main_symbols.contains(&symbol) {
                    return self.symbol_host_root(symbol, depth + 1);
                }
            }
        } else if let Some(object) = object_of_member(expression) {
            return self.expression_host_root(object, depth + 1);
        }
        None
    }

    fn symbol_host_root(&self, symbol: SymbolId, depth: usize) -> Option<String> {
        if depth > self.limits.max_alias_depth {
            return None;
        }
        if let Some(initializer) = self.initializer(symbol) {
            if let Some(host) = self.expression_host_root(initializer, depth + 1) {
                return Some(host);
            }
        }
        for (assignment_span, expression) in self.assignment_values(symbol) {
            let _ = assignment_span;
            if let Some(host) = self.expression_host_root(expression, depth + 1) {
                return Some(host);
            }
        }
        None
    }

    fn assignment_values(&self, symbol: SymbolId) -> Vec<(u32, &'a Expression<'a>)> {
        self.writes_by_symbol
            .get(&symbol)
            .map(|sites| {
                sites
                    .iter()
                    .filter(|site| site.kind == WriteKind::Plain)
                    .filter_map(|site| site.rhs.map(|rhs| (site.span.start, rhs)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every value definition of a binding, in source order: each declaration
    /// initializer (over every declaration origin, so a repeated `var` shares
    /// nothing and contributes each of its initializers) and each plain
    /// assignment write. Every definition carries the exact scope/condition
    /// class of its own AST location, so a conditional or loop-nested
    /// initializer is never mistaken for an unconditional one.
    fn value_definitions(&self, symbol: SymbolId) -> Vec<Definition<'a>> {
        let mut definitions = Vec::new();
        for declaration_node in self.scoping.symbol_declarations(symbol) {
            if let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration_node) {
                if let Some(initializer) = declarator.init.as_ref() {
                    definitions.push(Definition {
                        span: declarator.span,
                        expression: initializer,
                        class: self.classify_assignment(
                            self.main_index,
                            self.enclosing_function_index(declaration_node),
                            declaration_node,
                        ),
                    });
                }
            }
        }
        if let Some(sites) = self.writes_by_symbol.get(&symbol) {
            for site in sites {
                if site.kind != WriteKind::Plain {
                    continue;
                }
                let Some(rhs) = site.rhs else {
                    continue;
                };
                definitions.push(Definition {
                    span: site.span,
                    expression: rhs,
                    class: self.classify_assignment(
                        self.main_index,
                        self.enclosing_function_index(site.node_id),
                        site.node_id,
                    ),
                });
            }
        }
        definitions.sort_by_key(|definition| definition.span.start);
        definitions
    }

    /// Classify a main binding's value honestly.
    ///
    /// A single definition (`let x = <closed literal>` or one `x = <value>` and
    /// nothing else writing it) is a proven static value only when it runs
    /// unconditionally at the main top level. Any second definition, a
    /// conditional/block/loop/nested definition, a compound/update write, or a
    /// write the inventory cannot model is runtime state: the value fields are
    /// cleared and every competing definition is kept in `alternatives` with its
    /// exact scope class. An initializer's location decides its class; its mere
    /// presence is never an unconditional shortcut.
    fn classify_main_variable(
        &self,
        symbol: SymbolId,
        kind: BindingKind,
    ) -> (MainVariableClass, HeldSummary, Option<String>) {
        let initializer = self.initializer(symbol);
        let definitions = self.value_definitions(symbol);
        let sites = self
            .writes_by_symbol
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let total_writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        let unmodelled_dynamic = sites
            .iter()
            .filter(|site| site.kind != WriteKind::Plain)
            .count()
            + total_writes.saturating_sub(sites.len());

        if kind == BindingKind::Function {
            let declaration_span = self
                .nodes
                .get_node(self.scoping.symbol_declaration(symbol))
                .span();
            // A function binding is proven static only for a single source
            // declaration that nothing writes. Sloppy duplicate function
            // declarations share a SymbolId, so every origin is counted and a
            // conflict keeps `function_id` unresolved instead of naming the first.
            let declaration_count = self.scoping.symbol_declarations(symbol).count();
            let held = HeldSummary {
                proven_static: total_writes == 0 && declaration_count == 1,
                source: Some(SourceSpan::from(declaration_span)),
                source_preview: None,
                kind: "function_binding".to_string(),
                literal: None,
                array: None,
                object: None,
                function_id: self.symbol_function_id(symbol),
                class_id: None,
                target_main_variable: None,
                host_expression: None,
                alternatives: Vec::new(),
            };
            return (MainVariableClass::Function, held, None);
        }

        if let AstKind::Class(class) = self.nodes.kind(self.scoping.symbol_declaration(symbol)) {
            // A class declaration initializes its binding at the declaration.
            // It is proven static under the same rule as a function binding: a
            // single declaration that nothing writes.
            let declaration_count = self.scoping.symbol_declarations(symbol).count();
            let held = HeldSummary {
                proven_static: total_writes == 0 && declaration_count == 1,
                source: Some(SourceSpan::from(class.span)),
                source_preview: None,
                kind: "class_declaration".to_string(),
                literal: None,
                array: None,
                object: None,
                function_id: None,
                class_id: self.symbol_class_id(symbol),
                target_main_variable: None,
                host_expression: None,
                alternatives: Vec::new(),
            };
            return (MainVariableClass::Class, held, None);
        }

        if definitions.len() == 1 && unmodelled_dynamic == 0 {
            let definition = &definitions[0];
            if definition.class == AssignmentClass::MainTopLevel {
                let (classification, held) = self.classify_expression(definition.expression);
                let alias_of = held.target_main_variable.clone();
                return (classification, held, alias_of);
            }
        }

        if definitions.is_empty() && unmodelled_dynamic == 0 {
            return (
                MainVariableClass::Unknown,
                HeldSummary {
                    proven_static: false,
                    source: None,
                    source_preview: None,
                    kind: "unwritten".to_string(),
                    literal: None,
                    array: None,
                    object: None,
                    function_id: None,
                    class_id: None,
                    target_main_variable: None,
                    host_expression: None,
                    alternatives: Vec::new(),
                },
                None,
            );
        }

        (
            MainVariableClass::RuntimeState,
            self.runtime_state_held(symbol, initializer, &definitions),
            None,
        )
    }

    /// Every definition of a mutable binding, kept as labelled source evidence.
    /// The representative `source` is the initializer if present, else the first
    /// definition; it is never presented as the current value.
    fn runtime_state_held(
        &self,
        symbol: SymbolId,
        initializer: Option<&'a Expression<'a>>,
        definitions: &[Definition<'a>],
    ) -> HeldSummary {
        let mut alternatives: Vec<HeldAlternative> = definitions
            .iter()
            .map(|definition| self.held_alternative(definition.expression, definition.class))
            .collect();
        if let Some(sites) = self.writes_by_symbol.get(&symbol) {
            for site in sites {
                let class = self.classify_assignment(
                    self.main_index,
                    self.enclosing_function_index(site.node_id),
                    site.node_id,
                );
                match site.kind {
                    // Plain writes are already in `definitions`.
                    WriteKind::Plain => {}
                    WriteKind::Compound => alternatives.push(self.mutation_alternative(
                        site.span,
                        class,
                        "compound_write",
                    )),
                    WriteKind::Update => alternatives.push(self.mutation_alternative(
                        site.span,
                        class,
                        "update_write",
                    )),
                }
            }
        }
        alternatives.sort_by_key(|alternative| alternative.span.start);
        let representative = initializer
            .map(|expression| SourceSpan::from(expression.span()))
            .or_else(|| alternatives.first().map(|alternative| alternative.span));
        let (source, source_preview) = match representative {
            Some(span) => (
                Some(span),
                Some(self.preview_span(Span::new(span.start, span.end)).0),
            ),
            None => (None, None),
        };
        HeldSummary {
            proven_static: false,
            source,
            source_preview,
            kind: "mutable_state".to_string(),
            literal: None,
            array: None,
            object: None,
            function_id: None,
            class_id: None,
            target_main_variable: None,
            host_expression: None,
            alternatives,
        }
    }

    fn held_alternative(
        &self,
        expression: &Expression<'_>,
        class: AssignmentClass,
    ) -> HeldAlternative {
        let span = expression.span();
        let (array, object) = match expression {
            Expression::ArrayExpression(array_expression) => {
                (Some(self.array_held(array_expression)), None)
            }
            Expression::ObjectExpression(object_expression) => {
                (None, Some(self.object_held(object_expression)))
            }
            _ => (None, None),
        };
        HeldAlternative {
            span: SourceSpan::from(span),
            line: self.line(span.start),
            class,
            kind: expr_kind(expression).to_string(),
            literal: literal_value(expression),
            array,
            object,
            function_id: self
                .resolve_callee(expression)
                .map(|index| Self::site_id(self.sites[index].span)),
            class_id: self
                .resolve_class_expression(expression)
                .map(|index| Self::class_id(self.classes[index].span)),
            source_preview: self.preview_span(span).0,
        }
    }

    fn mutation_alternative(
        &self,
        span: Span,
        class: AssignmentClass,
        kind: &str,
    ) -> HeldAlternative {
        HeldAlternative {
            span: SourceSpan::from(span),
            line: self.line(span.start),
            class,
            kind: kind.to_string(),
            literal: None,
            array: None,
            object: None,
            function_id: None,
            class_id: None,
            source_preview: self.snippet(span),
        }
    }

    /// The value shape of a single expression, as descriptive evidence.
    #[allow(unused_assignments)]
    fn classify_expression(&self, expression: &Expression<'_>) -> (MainVariableClass, HeldSummary) {
        let span = expression.span();
        let (source_preview, _) = self.preview_span(span);
        let literal = literal_value(expression);
        let mut array = None;
        let mut object = None;
        let mut function_id = None;
        let mut class_id = None;
        let mut target_main_variable = None;
        let mut host_expression = None;
        let mut classification = MainVariableClass::Unknown;
        let mut held_kind = "expression".to_string();
        match expression {
            Expression::ArrayExpression(array_expression) => {
                classification = MainVariableClass::ArrayLiteral;
                held_kind = "array_literal".to_string();
                array = Some(self.array_held(array_expression));
            }
            Expression::ObjectExpression(object_expression) => {
                classification = MainVariableClass::ObjectLiteral;
                held_kind = "object_literal".to_string();
                object = Some(self.object_held(object_expression));
            }
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
                classification = MainVariableClass::Function;
                held_kind = "function".to_string();
                function_id = self
                    .resolve_callee(expression)
                    .map(|index| Self::site_id(self.sites[index].span));
            }
            Expression::ClassExpression(_) => {
                classification = MainVariableClass::Class;
                held_kind = "class_expression".to_string();
                class_id = self
                    .resolve_class_expression(expression)
                    .map(|index| Self::class_id(self.classes[index].span));
            }
            Expression::Identifier(identifier) => {
                if let Some(target) = crate::analysis::identifier_symbol(identifier, self.scoping) {
                    if self.main_symbols.contains(&target) {
                        classification = MainVariableClass::Alias;
                        held_kind = "main_alias".to_string();
                        target_main_variable = self.main_var_ids.get(&target).cloned();
                    } else if is_host_global(identifier.name.as_str()) {
                        classification = MainVariableClass::HostReference;
                        held_kind = "host_reference".to_string();
                        host_expression = Some(identifier.name.to_string());
                    } else if let Some(id) = self.symbol_function_id(target) {
                        classification = MainVariableClass::Function;
                        held_kind = "function_alias".to_string();
                        function_id = Some(id);
                    } else if let Some(id) = self.symbol_class_id(target) {
                        classification = MainVariableClass::Class;
                        held_kind = "class_alias".to_string();
                        class_id = Some(id);
                    } else {
                        classification = MainVariableClass::RuntimeState;
                        held_kind = "runtime_identifier".to_string();
                    }
                } else if is_host_global(identifier.name.as_str()) {
                    classification = MainVariableClass::HostReference;
                    held_kind = "host_global".to_string();
                    host_expression = Some(identifier.name.to_string());
                } else {
                    classification = MainVariableClass::RuntimeState;
                    held_kind = "external_identifier".to_string();
                }
            }
            Expression::CallExpression(_) | Expression::NewExpression(_) => {
                classification = MainVariableClass::ConfiguredResult;
                held_kind = "configured_result".to_string();
                if let Some(root) = member_root(expression) {
                    if is_host_global(root) {
                        host_expression = Some(root.to_string());
                    }
                }
            }
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => {
                if let Some(root) = self.expression_host_root(expression, 0) {
                    classification = MainVariableClass::HostReference;
                    held_kind = "host_member".to_string();
                    host_expression = Some(root);
                } else if let Some(root) = member_root(expression) {
                    if is_host_global(root) {
                        classification = MainVariableClass::HostReference;
                        held_kind = "host_member".to_string();
                        host_expression = Some(root.to_string());
                    } else {
                        classification = MainVariableClass::ConfiguredResult;
                        held_kind = "member_expression".to_string();
                    }
                } else {
                    classification = MainVariableClass::ConfiguredResult;
                    held_kind = "member_expression".to_string();
                }
            }
            _ if literal.is_some() => {
                classification = MainVariableClass::Literal;
                held_kind = "literal".to_string();
            }
            _ => {
                classification = MainVariableClass::RuntimeState;
                held_kind = "runtime_state".to_string();
            }
        }
        (
            classification,
            HeldSummary {
                proven_static: true,
                source: Some(SourceSpan::from(span)),
                source_preview: Some(source_preview),
                kind: held_kind,
                literal,
                array,
                object,
                function_id,
                class_id,
                target_main_variable,
                host_expression,
                alternatives: Vec::new(),
            },
        )
    }

    fn array_held(&self, array: &ArrayExpression<'_>) -> ArrayHeld {
        let mut elements = Vec::new();
        let mut literals = Vec::new();
        let mut all_literal = true;
        let mut nested = 0;
        let mut objects = 0;
        for element in &array.elements {
            match element {
                ArrayExpressionElement::Elision(_) | ArrayExpressionElement::SpreadElement(_) => {
                    all_literal = false;
                    elements.push("<non-static>".to_string());
                }
                _ => {
                    let Some(expression) = element.as_expression() else {
                        all_literal = false;
                        continue;
                    };
                    if let Some(value) = literal_value(expression) {
                        literals.push(value);
                    } else {
                        all_literal = false;
                    }
                    match expression {
                        Expression::ArrayExpression(_) => nested += 1,
                        Expression::ObjectExpression(_) => objects += 1,
                        _ => {}
                    }
                    elements.push(self.snippet(expression.span()));
                }
            }
            if elements.len() >= self.limits.max_array_elements {
                break;
            }
        }
        let truncated = array.elements.len() > elements.len();
        ArrayHeld {
            length: array.elements.len(),
            truncated,
            elements,
            literal_elements: all_literal.then_some(literals),
            nested_array_elements: nested,
            object_elements: objects,
        }
    }

    fn object_held(&self, object: &ObjectExpression<'_>) -> ObjectHeld {
        let mut keys = Vec::new();
        let mut entries = Vec::new();
        let mut truncated = false;
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let Some(key) = property_key_name(&property.key) else {
                continue;
            };
            keys.push(key.clone());
            if entries.len() < self.limits.max_object_entries {
                entries.push(ObjectEntryHeld {
                    key,
                    kind: expr_kind(&property.value).to_string(),
                    literal: literal_value(&property.value),
                    value_preview: self.snippet(property.value.span()),
                    function_id: self
                        .resolve_callee(&property.value)
                        .map(|index| Self::site_id(self.sites[index].span)),
                });
            } else {
                truncated = true;
            }
        }
        ObjectHeld {
            keys,
            entries,
            truncated,
        }
    }

    fn signal_wave_for_symbol(&self, symbol: SymbolId) -> (Vec<u32>, Vec<u32>) {
        let index = match self.resolve_symbol_function(symbol) {
            SymbolFunctionResolution::Unique(index) => index,
            _ => return (Vec::new(), Vec::new()),
        };
        let start = self.sites[index].span.start;
        match self.function_roles.get(&start) {
            Some((_, _, signals, wave, _)) => (
                signals.clone(),
                wave.map(|wave| vec![wave]).unwrap_or_default(),
            ),
            None => (Vec::new(), Vec::new()),
        }
    }

    fn expr_summary(&self, expression: &Expression<'a>) -> ExprSummary {
        let span = expression.span();
        let (source_preview, source_truncated) = self.preview_span(span);
        let binding = match expression {
            Expression::Identifier(identifier) => {
                crate::analysis::identifier_symbol(identifier, self.scoping).map(|symbol| {
                    BindingSummary {
                        symbol: symbol.index() as u32,
                        name: identifier.name.to_string(),
                        identifier: SourceSpan::from(self.scoping.symbol_span(symbol)),
                    }
                })
            }
            _ => None,
        };
        ExprSummary {
            span: SourceSpan::from(span),
            source_preview,
            source_truncated,
            kind: expr_kind(expression).to_string(),
            literal: literal_value(expression),
            function_id: self
                .resolve_callee(expression)
                .map(|index| Self::site_id(self.sites[index].span)),
            binding,
            member_root: member_root(expression).map(ToString::to_string),
        }
    }

    fn binding_kind(&self, symbol: SymbolId) -> BindingKind {
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::VariableDeclarator(_) => BindingKind::Variable,
            AstKind::Function(_) => BindingKind::Function,
            AstKind::FormalParameter(_) => BindingKind::Parameter,
            AstKind::CatchParameter(_) => BindingKind::Catch,
            AstKind::Class(_) => BindingKind::Class,
            _ => BindingKind::Other,
        }
    }

    fn initializer(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
            _ => None,
        }
    }

    /// The report's declaration kind. Unlike [`BindingKind`], it separates the
    /// binding a class declaration introduces from other declarations.
    fn declaration_kind(&self, symbol: SymbolId) -> BindingDeclarationKind {
        match self.nodes.kind(self.scoping.symbol_declaration(symbol)) {
            AstKind::Class(_) => BindingDeclarationKind::Class,
            _ => BindingDeclarationKind::from_binding_kind(self.binding_kind(symbol)),
        }
    }

    // -- function records ---------------------------------------------------

    fn build_functions(
        &mut self,
        holders: &mut HashMap<usize, Vec<ValueHolder>>,
    ) -> Vec<FunctionRecord> {
        let mut records = Vec::new();
        let alias_map = self.build_alias_map();
        let mut property_contexts: HashMap<usize, BTreeSet<String>> = HashMap::new();
        let mut argument_contexts: HashMap<usize, Vec<FunctionArgumentContext>> = HashMap::new();
        for (node_id, node) in self.nodes.iter_enumerated() {
            match node.kind() {
                AstKind::AssignmentExpression(assignment) => {
                    let Some(property) = assignment_target_property(&assignment.left) else {
                        continue;
                    };
                    if let Some(index) = self.resolve_callee(&assignment.right) {
                        property_contexts.entry(index).or_default().insert(property);
                    }
                }
                AstKind::CallExpression(call) => {
                    for (argument_index, argument) in call.arguments.iter().enumerate() {
                        let Some(expression) = argument.as_expression() else {
                            continue;
                        };
                        let Some(index) = self.resolve_callee(expression) else {
                            continue;
                        };
                        argument_contexts
                            .entry(index)
                            .or_default()
                            .push(FunctionArgumentContext {
                                callee_source: self.exact(call.callee.span()).to_string(),
                                argument_index: argument_index as u32,
                                call_span: SourceSpan::from(call.span),
                                line: self.line(node_id_start(node_id, self.nodes)),
                            });
                    }
                }
                _ => {}
            }
        }
        for index in 0..self.sites.len() {
            let id = Self::site_id(self.sites[index].span);
            let parent_id = self.sites[index]
                .parent
                .map(|parent| Self::site_id(self.sites[parent].span));
            let parent_label = self.sites[index]
                .parent
                .and_then(|parent| self.sites[parent].label.clone());
            let own_bindings = self.own_symbols[index]
                .iter()
                .map(|symbol| self.own_binding_record(*symbol))
                .collect();
            let (main_reads, main_writes, captured_ids) =
                self.main_references_for(index, &alias_map);
            let role_entry = self.function_roles.get(&self.sites[index].span.start);
            let (role, role_basis, signal_indices, wave_index, direct_signal_index) =
                match role_entry {
                    Some((role, basis, signals, wave, direct)) => (
                        Some(role.clone()),
                        Some(basis.clone()),
                        signals.clone(),
                        *wave,
                        *direct,
                    ),
                    None => (None, None, Vec::new(), None, None),
                };
            records.push(FunctionRecord {
                id,
                ordinal: index as u32 + 1,
                label: self.sites[index].label.clone(),
                binding_label: self.sites[index].binding_label.clone(),
                label_source: self.sites[index].label_source,
                kind: self.sites[index].kind,
                full_span: SourceSpan::from(self.sites[index].span),
                identifier_span: self.sites[index].identifier_span.map(SourceSpan::from),
                body_span: self.sites[index].body_span.map(SourceSpan::from),
                start_line: self.line(self.sites[index].span.start),
                end_line: self.line(self.sites[index].span.end),
                body_start_line: self.sites[index]
                    .body_span
                    .map(|span| self.line(span.start)),
                body_end_line: self.sites[index].body_span.map(|span| self.line(span.end)),
                parent_function: parent_id,
                parent_function_label: parent_label,
                lexical_depth: self.sites[index].depth,
                is_main: self.main_index == Some(index),
                is_top_level_entry: false,
                params: self.sites[index].params.clone(),
                own_bindings,
                main_reads,
                main_writes,
                captured_main_variable_ids: captured_ids,
                callee_ids: Vec::new(),
                caller_ids: Vec::new(),
                callee_count: 0,
                caller_count: 0,
                unresolved_call_count: 0,
                role,
                role_basis,
                signal_indices,
                wave_index,
                direct_signal_index,
                declaration_context: self.sites[index].declaration_context.clone(),
                assigned_properties: property_contexts
                    .remove(&index)
                    .map(|values| values.into_iter().collect())
                    .unwrap_or_default(),
                argument_contexts: argument_contexts.remove(&index).unwrap_or_default(),
                name_binding: self.sites[index]
                    .name_symbol
                    .map(|symbol| self.binding_summary(symbol)),
                holders: holders.remove(&index).unwrap_or_default(),
                class_id: self
                    .class_of_site
                    .get(&index)
                    .map(|class| Self::class_id(self.classes[*class].span)),
            });
        }
        records
    }

    /// Every binding linked to a function or class value, keyed by target and
    /// site/class index. A link is syntactic (a declaration, a direct
    /// initializer or assignment of the value, or an identifier that resolves
    /// to it); its [`HolderProof`] comes from the same resolution that decides
    /// call edges, so a conditional, repeated, compound, destructuring or
    /// `for…of` write keeps the link visible but unproven.
    fn collect_holders(
        &self,
    ) -> (
        HashMap<usize, Vec<ValueHolder>>,
        HashMap<usize, Vec<ValueHolder>>,
    ) {
        let mut functions: HashMap<usize, Vec<ValueHolder>> = HashMap::new();
        let mut classes: HashMap<usize, Vec<ValueHolder>> = HashMap::new();
        for scope in self.scoping.scope_descendants_from_root() {
            for symbol in self.scoping.iter_bindings_in(scope) {
                let mut links: Vec<(ValueTarget, usize, HolderRelation, Span)> = Vec::new();
                for declaration_node in self.scoping.symbol_declarations(symbol) {
                    match self.nodes.kind(declaration_node) {
                        // Only a declaration binds its name where holders live; a
                        // named expression's own name is its `name_binding`.
                        AstKind::Function(function)
                            if function.r#type == FunctionType::FunctionDeclaration =>
                        {
                            if let Some(&index) = self.by_node.get(&function.node_id.get()) {
                                links.push((
                                    ValueTarget::Function,
                                    index,
                                    HolderRelation::Declaration,
                                    function.span,
                                ));
                            }
                        }
                        AstKind::Class(class) if class.r#type == ClassType::ClassDeclaration => {
                            if let Some(&index) = self.class_by_node.get(&class.node_id.get()) {
                                links.push((
                                    ValueTarget::Class,
                                    index,
                                    HolderRelation::Declaration,
                                    class.span,
                                ));
                            }
                        }
                        AstKind::VariableDeclarator(declarator) => {
                            if let Some((target, index, alias)) = declarator
                                .init
                                .as_ref()
                                .and_then(|init| self.direct_value(init, symbol))
                            {
                                let relation = if alias {
                                    HolderRelation::Alias
                                } else {
                                    HolderRelation::Initializer
                                };
                                links.push((target, index, relation, declarator.span));
                            }
                        }
                        _ => {}
                    }
                }
                for site in self.writes_by_symbol.get(&symbol).into_iter().flatten() {
                    if site.kind != WriteKind::Plain {
                        continue;
                    }
                    if let Some((target, index, alias)) =
                        site.rhs.and_then(|rhs| self.direct_value(rhs, symbol))
                    {
                        let relation = if alias {
                            HolderRelation::Alias
                        } else {
                            HolderRelation::Assignment
                        };
                        links.push((target, index, relation, site.span));
                    }
                }
                if links.is_empty() {
                    continue;
                }
                let mut resolutions: HashMap<ValueTarget, SymbolFunctionResolution> =
                    HashMap::new();
                // Call resolution accepts a lone conditional write (a successful
                // call can only reach that function); holding the value also
                // needs every definition to run unconditionally.
                let unconditional = self.definitions_unconditional(symbol);
                for (target, index, relation, span) in links {
                    let resolution = *resolutions
                        .entry(target)
                        .or_insert_with(|| self.resolve_symbol_value(symbol, 0, target));
                    let proof = match resolution {
                        SymbolFunctionResolution::Unique(value) if value == index => {
                            if unconditional {
                                HolderProof::Proven
                            } else {
                                HolderProof::Possible
                            }
                        }
                        SymbolFunctionResolution::Possible(value) if value == index => {
                            HolderProof::Possible
                        }
                        SymbolFunctionResolution::Unique(_)
                        | SymbolFunctionResolution::Possible(_)
                        | SymbolFunctionResolution::Ambiguous => HolderProof::Ambiguous,
                        SymbolFunctionResolution::Dynamic | SymbolFunctionResolution::None => {
                            HolderProof::Dynamic
                        }
                    };
                    let holder = ValueHolder {
                        binding: self.binding_summary(symbol),
                        main_variable_id: self.main_var_ids.get(&symbol).cloned(),
                        relation,
                        definition: SourceSpan::from(span),
                        proof,
                    };
                    match target {
                        ValueTarget::Function => functions.entry(index).or_default().push(holder),
                        ValueTarget::Class => classes.entry(index).or_default().push(holder),
                    }
                }
            }
        }
        for holders in functions.values_mut().chain(classes.values_mut()) {
            holders
                .sort_by_key(|holder| (holder.definition.start, holder.binding.identifier.start));
        }
        (functions, classes)
    }

    /// True when every initializer and plain write of `symbol` runs
    /// unconditionally in the scope that declares it: no function boundary,
    /// block, branch, loop, short-circuit, `switch` or `try` lies between the
    /// definition and the declaring scope.
    fn definitions_unconditional(&self, symbol: SymbolId) -> bool {
        let scope_node = self
            .scoping
            .get_node_id(self.scoping.symbol_scope_id(symbol));
        let initializers = self.scoping.symbol_declarations(symbol).filter(|node| {
            matches!(
                self.nodes.kind(*node),
                AstKind::VariableDeclarator(declarator) if declarator.init.is_some()
            )
        });
        let writes = self
            .writes_by_symbol
            .get(&symbol)
            .into_iter()
            .flatten()
            .filter(|site| site.kind == WriteKind::Plain)
            .map(|site| site.node_id);
        initializers
            .chain(writes)
            .all(|node_id| self.runs_unconditionally_in(node_id, scope_node))
    }

    fn runs_unconditionally_in(&self, node_id: NodeId, scope_node: NodeId) -> bool {
        let mut current = node_id;
        for _ in 0..2_048 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
            if current == scope_node {
                return true;
            }
            if self.by_node.contains_key(&current) {
                return false;
            }
            if is_conditional_context(&self.nodes.kind(current)) {
                return false;
            }
        }
        false
    }

    /// The function or class a definition's value directly denotes, and
    /// whether it does so through another binding (an alias). An alias links
    /// only when its target binding is itself proven to hold one value.
    fn direct_value(
        &self,
        expression: &Expression<'_>,
        symbol: SymbolId,
    ) -> Option<(ValueTarget, usize, bool)> {
        match expression {
            Expression::FunctionExpression(function) => self
                .by_node
                .get(&function.node_id.get())
                .map(|&index| (ValueTarget::Function, index, false)),
            Expression::ArrowFunctionExpression(arrow) => self
                .by_node
                .get(&arrow.node_id.get())
                .map(|&index| (ValueTarget::Function, index, false)),
            Expression::ClassExpression(class) => self
                .class_by_node
                .get(&class.node_id.get())
                .map(|&index| (ValueTarget::Class, index, false)),
            Expression::Identifier(identifier) => {
                let target = crate::analysis::identifier_symbol(identifier, self.scoping)?;
                if target == symbol {
                    return None;
                }
                if let SymbolFunctionResolution::Unique(index) =
                    self.resolve_symbol_function(target)
                {
                    return Some((ValueTarget::Function, index, true));
                }
                if let SymbolFunctionResolution::Unique(index) = self.resolve_symbol_class(target) {
                    return Some((ValueTarget::Class, index, true));
                }
                None
            }
            _ => None,
        }
    }

    fn build_classes(&self, mut holders: HashMap<usize, Vec<ValueHolder>>) -> Vec<ClassRecord> {
        let site_ids = |indices: &[usize]| -> Vec<String> {
            indices
                .iter()
                .map(|index| Self::site_id(self.sites[*index].span))
                .collect()
        };
        self.classes
            .iter()
            .enumerate()
            .map(|(index, class)| {
                let holders = holders.remove(&index).unwrap_or_default();
                let direct_main: Vec<&String> = holders
                    .iter()
                    .filter(|holder| {
                        holder.proof == HolderProof::Proven
                            && holder.relation != HolderRelation::Alias
                    })
                    .filter_map(|holder| holder.main_variable_id.as_ref())
                    .collect();
                ClassRecord {
                    id: Self::class_id(class.span),
                    ordinal: index as u32 + 1,
                    kind: class.kind,
                    label: class.label.clone(),
                    full_span: SourceSpan::from(class.span),
                    identifier_span: class.identifier_span.map(SourceSpan::from),
                    name_binding: class.name_symbol.map(|symbol| self.binding_summary(symbol)),
                    holder_main_variable_id: match direct_main.as_slice() {
                        [only] => Some((*only).clone()),
                        _ => None,
                    },
                    holders,
                    constructor_id: class
                        .constructor
                        .map(|index| Self::site_id(self.sites[index].span)),
                    method_ids: site_ids(&class.methods),
                    getter_ids: site_ids(&class.getters),
                    setter_ids: site_ids(&class.setters),
                    field_function_ids: site_ids(&class.fields),
                    superclass_preview: class.superclass.map(|span| self.snippet(span)),
                    parent_function: class
                        .parent
                        .map(|parent| Self::site_id(self.sites[parent].span)),
                    start_line: self.line(class.span.start),
                    end_line: self.line(class.span.end),
                }
            })
            .collect()
    }

    fn own_binding_record(&self, symbol: SymbolId) -> OwnBindingRecord {
        let declaration_span = self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .span();
        let identifier_span = self.scoping.symbol_span(symbol);
        OwnBindingRecord {
            symbol: symbol.index() as u32,
            name: self.scoping.symbol_name(symbol).to_string(),
            kind: self.declaration_kind(symbol),
            declaration: SourceSpan::from(declaration_span),
            identifier: SourceSpan::from(identifier_span),
            references: self.scoping.get_resolved_reference_ids(symbol).len(),
            writes: self
                .scoping
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .count(),
            initializer: self.initializer(symbol).map(|expr| self.expr_summary(expr)),
        }
    }

    fn build_alias_map(&self) -> HashMap<SymbolId, (SymbolId, Vec<String>)> {
        let mut raw: HashMap<SymbolId, SymbolId> = HashMap::new();
        for scope in self.scoping.scope_descendants_from_root() {
            for symbol in self.scoping.iter_bindings_in(scope) {
                if self.main_symbols.contains(&symbol) {
                    continue;
                }
                let mut value = self.initializer(symbol);
                if value.is_none() {
                    value = self
                        .assignment_values(symbol)
                        .into_iter()
                        .next()
                        .map(|(_, e)| e);
                }
                let Some(expression) = value else {
                    continue;
                };
                let Expression::Identifier(identifier) = expression else {
                    continue;
                };
                let Some(target) = crate::analysis::identifier_symbol(identifier, self.scoping)
                else {
                    continue;
                };
                if target != symbol {
                    raw.insert(symbol, target);
                }
            }
        }
        let mut resolved = HashMap::new();
        for symbol in raw.keys().copied() {
            let mut visited = BTreeSet::new();
            if let Some((main, chain)) = self.resolve_alias_to_main(symbol, &raw, &mut visited, 0) {
                resolved.insert(symbol, (main, chain));
            }
        }
        resolved
    }

    fn resolve_alias_to_main(
        &self,
        alias: SymbolId,
        raw: &HashMap<SymbolId, SymbolId>,
        visited: &mut BTreeSet<SymbolId>,
        depth: usize,
    ) -> Option<(SymbolId, Vec<String>)> {
        if depth > self.limits.max_alias_depth || !visited.insert(alias) {
            return None;
        }
        let target = *raw.get(&alias)?;
        if self.main_symbols.contains(&target) {
            return Some((target, vec![self.scoping.symbol_name(target).to_string()]));
        }
        self.resolve_alias_to_main(target, raw, visited, depth + 1)
            .map(|(main, mut chain)| {
                chain.insert(0, self.scoping.symbol_name(target).to_string());
                (main, chain)
            })
    }

    fn main_references_for(
        &self,
        index: usize,
        alias_map: &HashMap<SymbolId, (SymbolId, Vec<String>)>,
    ) -> (
        Vec<MainReferenceRecord>,
        Vec<MainReferenceRecord>,
        Vec<String>,
    ) {
        let mut reads = Vec::new();
        let mut writes = Vec::new();
        let mut ids = BTreeSet::new();
        if self.main_index == Some(index) {
            return (reads, writes, ids.into_iter().collect());
        }
        for symbol in self.main_symbols.iter().copied() {
            for reference in self.scoping.get_resolved_references(symbol) {
                if self.enclosing_function_index(reference.node_id()) != Some(index) {
                    continue;
                }
                let span = self.nodes.get_node(reference.node_id()).span();
                let access = match (reference.is_read(), reference.is_write()) {
                    (true, true) => MainReferenceAccess::ReadWrite,
                    (true, false) => MainReferenceAccess::Read,
                    (false, true) => MainReferenceAccess::Write,
                    (false, false) => continue,
                };
                ids.insert(self.main_var_ids.get(&symbol).cloned().unwrap_or_default());
                let record = MainReferenceRecord {
                    main_variable_id: self.main_var_ids.get(&symbol).cloned().unwrap_or_default(),
                    name: self.scoping.symbol_name(symbol).to_string(),
                    role: self
                        .role_by_identifier_start
                        .get(&self.scoping.symbol_span(symbol).start)
                        .map(|role| format!("{role:?}")),
                    span: SourceSpan::from(span),
                    line: self.line(span.start),
                    access,
                    via_alias: Vec::new(),
                    aliased_symbol: None,
                };
                if reference.is_read() {
                    reads.push(record.clone());
                }
                if reference.is_write() {
                    writes.push(record);
                }
            }
        }
        for (alias_symbol, (target, chain)) in alias_map {
            for reference in self.scoping.get_resolved_references(*alias_symbol) {
                if self.enclosing_function_index(reference.node_id()) != Some(index) {
                    continue;
                }
                let span = self.nodes.get_node(reference.node_id()).span();
                let access = match (reference.is_read(), reference.is_write()) {
                    (true, true) => MainReferenceAccess::ReadWrite,
                    (true, false) => MainReferenceAccess::Read,
                    (false, true) => MainReferenceAccess::Write,
                    (false, false) => continue,
                };
                ids.insert(self.main_var_ids.get(target).cloned().unwrap_or_default());
                let record = MainReferenceRecord {
                    main_variable_id: self.main_var_ids.get(target).cloned().unwrap_or_default(),
                    name: self.scoping.symbol_name(*target).to_string(),
                    role: self
                        .role_by_identifier_start
                        .get(&self.scoping.symbol_span(*target).start)
                        .map(|role| format!("{role:?}")),
                    span: SourceSpan::from(span),
                    line: self.line(span.start),
                    access,
                    via_alias: chain.clone(),
                    aliased_symbol: Some(alias_symbol.index() as u32),
                };
                if reference.is_read() {
                    reads.push(record.clone());
                }
                if reference.is_write() {
                    writes.push(record);
                }
            }
        }
        reads.sort_by_key(|record| record.span.start);
        writes.sort_by_key(|record| record.span.start);
        (reads, writes, ids.into_iter().collect())
    }

    // -- call edges ---------------------------------------------------------

    fn build_call_edges(&mut self) -> (Vec<CallEdge>, Vec<UnresolvedCall>) {
        let mut edges = Vec::new();
        let mut unresolved = Vec::new();
        let mut caller_links: HashMap<usize, BTreeSet<usize>> = HashMap::new();
        let mut callee_links: HashMap<usize, BTreeSet<usize>> = HashMap::new();
        let mut unresolved_counts: HashMap<usize, usize> = HashMap::new();
        for (node_id, node) in self.nodes.iter_enumerated() {
            let (callee_expr, kind) = match node.kind() {
                AstKind::CallExpression(call) => (&call.callee, CallKind::Call),
                AstKind::NewExpression(new) => (&new.callee, CallKind::New),
                _ => continue,
            };
            let caller = self.enclosing_function_index(node_id);
            let callee_span = callee_expr.span();
            let callee_source = self.exact(callee_span).to_string();
            if let Some(callee_index) = self.resolve_callee(callee_expr) {
                if edges.len() >= self.limits.max_call_edges {
                    return (edges, unresolved);
                }
                let resolution = match callee_expr {
                    Expression::FunctionExpression(_) => CallResolution::FunctionExpression,
                    Expression::ArrowFunctionExpression(_) => CallResolution::ArrowFunction,
                    Expression::Identifier(identifier) => {
                        if self.is_alias_callee(identifier) {
                            CallResolution::Alias
                        } else {
                            CallResolution::Identifier
                        }
                    }
                    _ => CallResolution::Identifier,
                };
                if let Some(caller_index) = caller {
                    caller_links
                        .entry(caller_index)
                        .or_default()
                        .insert(callee_index);
                    callee_links
                        .entry(callee_index)
                        .or_default()
                        .insert(caller_index);
                }
                edges.push(CallEdge {
                    caller: caller.map(|index| Self::site_id(self.sites[index].span)),
                    caller_label: caller.and_then(|index| self.sites[index].label.clone()),
                    callee: Self::site_id(self.sites[callee_index].span),
                    callee_label: self.sites[callee_index].label.clone(),
                    kind,
                    resolution,
                    span: SourceSpan::from(node.span()),
                    line: self.line(node.span().start),
                    callee_source,
                });
            } else {
                self.unresolved_total += 1;
                if let Some(caller_index) = caller {
                    *unresolved_counts.entry(caller_index).or_default() += 1;
                }
                if unresolved.len() < self.limits.max_unresolved_calls {
                    unresolved.push(UnresolvedCall {
                        caller: caller.map(|index| Self::site_id(self.sites[index].span)),
                        caller_label: caller.and_then(|index| self.sites[index].label.clone()),
                        span: SourceSpan::from(node.span()),
                        line: self.line(node.span().start),
                        callee_source,
                        reason: self.unresolved_reason(callee_expr),
                    });
                }
            }
        }
        self.caller_links = caller_links;
        self.callee_links = callee_links;
        self.unresolved_counts = unresolved_counts;
        (edges, unresolved)
    }

    fn is_alias_callee(&self, identifier: &IdentifierReference<'_>) -> bool {
        let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.scoping) else {
            return false;
        };
        let declaration = self.nodes.get_node(self.scoping.symbol_declaration(symbol));
        match declaration.kind() {
            AstKind::VariableDeclarator(declarator) => {
                matches!(declarator.init.as_ref(), Some(Expression::Identifier(_)))
            }
            AstKind::Function(_) => false,
            _ => true,
        }
    }

    // -- assembly -----------------------------------------------------------

    fn finish(mut self) -> Result<FunctionIndex, FunctionIndexError> {
        let (call_edges, unresolved_calls) = self.build_call_edges();
        self.call_edges_snapshot = call_edges.clone();
        let (mut function_holders, class_holders) = self.collect_holders();
        let mut functions = self.build_functions(&mut function_holders);
        let classes = self.build_classes(class_holders);
        let top_level_entries = self.top_level_entry_ids();
        for function in &mut functions {
            let index = self
                .sites
                .iter()
                .position(|site| Self::site_id(site.span) == function.id)
                .expect("function id round-trips");
            function.is_top_level_entry = top_level_entries.contains(&function.id);
            let callees = self.caller_links.get(&index).cloned().unwrap_or_default();
            let callers = self.callee_links.get(&index).cloned().unwrap_or_default();
            function.callee_ids = callees
                .iter()
                .map(|callee| Self::site_id(self.sites[*callee].span))
                .collect();
            function.caller_ids = callers
                .iter()
                .map(|caller| Self::site_id(self.sites[*caller].span))
                .collect();
            function.callee_count = callees.len();
            function.caller_count = callers.len();
            function.unresolved_call_count =
                self.unresolved_counts.get(&index).copied().unwrap_or(0);
        }
        let main_variables = self.build_main_variables();
        let main_index = self.main_index;
        let main = main_index.map(|index| {
            let function_id = Self::site_id(self.sites[index].span);
            let is_entry = self.top_level_entry_ids().contains(&function_id);
            MainFunctionRecord {
                function_id,
                label: self.sites[index].label.clone(),
                full_span: SourceSpan::from(self.sites[index].span),
                body_span: self.sites[index].body_span.map(SourceSpan::from),
                identifier_span: self.sites[index].identifier_span.map(SourceSpan::from),
                start_line: self.line(self.sites[index].span.start),
                end_line: self.line(self.sites[index].span.end),
                direct_binding_count: self.main_symbols.len(),
                is_top_level_entry: is_entry,
                reachable_from_top_level_entry: is_entry || self.reachable_from_entry(index),
                returns_object_with_init: self.returns_object_with_init(index),
                top_level_call: self.top_level_call_span(),
                selection_basis: self.main_selection_basis.clone(),
            }
        });
        // Attach consumer records to main variables.
        let mut consumers: HashMap<String, Vec<ConsumerRecord>> = HashMap::new();
        for function in &functions {
            if function.is_main {
                continue;
            }
            for id in &function.captured_main_variable_ids {
                let entry = consumers.entry(id.clone()).or_default();
                let reads = function
                    .main_reads
                    .iter()
                    .filter(|record| &record.main_variable_id == id)
                    .count();
                let writes = function
                    .main_writes
                    .iter()
                    .filter(|record| &record.main_variable_id == id)
                    .count();
                entry.push(ConsumerRecord {
                    function_id: function.id.clone(),
                    function_label: function.label.clone(),
                    reads,
                    writes,
                });
            }
        }
        let mut main_variables = main_variables;
        for variable in &mut main_variables {
            if let Some(mut records) = consumers.remove(&variable.id) {
                records.sort_by_key(|record| record.function_id.clone());
                variable.captured_by = records;
            }
        }
        let mut coverage =
            self.compute_coverage(&functions, &main_variables, &call_edges, &unresolved_calls);
        coverage.class_count = classes.len();
        coverage.duplicate_function_spans = duplicate_spans(&functions);
        let source = crate::signals::schema::SourceIdentity {
            sha256: sha256_hex(self.source),
            bytes: self.source.len(),
        };
        Ok(FunctionIndex {
            schema_version: FUNCTION_INDEX_SCHEMA_VERSION,
            source,
            main,
            functions,
            classes,
            call_edges,
            unresolved_calls,
            main_variables,
            coverage,
            diagnostics: self.diagnostics,
        })
    }

    fn top_level_entry_ids(&self) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        for edge in &self.call_edges_snapshot {
            if edge.caller.is_none() {
                result.insert(edge.callee.clone());
            }
        }
        result
    }

    fn reachable_from_entry(&self, target: usize) -> bool {
        let target_id = Self::site_id(self.sites[target].span);
        let mut queue: Vec<usize> = self
            .call_edges_snapshot
            .iter()
            .filter(|edge| edge.caller.is_none())
            .filter_map(|edge| {
                self.sites
                    .iter()
                    .position(|site| Self::site_id(site.span) == edge.callee)
            })
            .collect();
        let mut seen = HashSet::new();
        while let Some(index) = queue.pop() {
            if !seen.insert(index) {
                continue;
            }
            if Self::site_id(self.sites[index].span) == target_id {
                return true;
            }
            for edge in &self.call_edges_snapshot {
                if edge.caller.as_deref() == Some(Self::site_id(self.sites[index].span).as_str()) {
                    if let Some(callee) = self
                        .sites
                        .iter()
                        .position(|site| Self::site_id(site.span) == edge.callee)
                    {
                        queue.push(callee);
                    }
                }
            }
        }
        false
    }

    fn top_level_call_span(&self) -> Option<SourceSpan> {
        self.call_edges_snapshot
            .iter()
            .find(|edge| edge.caller.is_none())
            .map(|edge| edge.span)
    }

    fn compute_coverage(
        &self,
        functions: &[FunctionRecord],
        main_variables: &[MainVariableRecord],
        call_edges: &[CallEdge],
        unresolved_calls: &[UnresolvedCall],
    ) -> FunctionIndexCoverage {
        let mut coverage = FunctionIndexCoverage {
            function_count: functions.len(),
            named_count: functions
                .iter()
                .filter(|function| function.label.is_some())
                .count(),
            anonymous_count: functions
                .iter()
                .filter(|function| function.label.is_none())
                .count(),
            declaration_count: functions
                .iter()
                .filter(|function| function.kind == FunctionKind::Declaration)
                .count(),
            expression_count: functions
                .iter()
                .filter(|function| function.kind == FunctionKind::Expression)
                .count(),
            arrow_count: functions
                .iter()
                .filter(|function| function.kind == FunctionKind::Arrow)
                .count(),
            method_count: functions
                .iter()
                .filter(|function| {
                    matches!(
                        function.kind,
                        FunctionKind::ObjectMethod
                            | FunctionKind::ClassMethod
                            | FunctionKind::Getter
                            | FunctionKind::Setter
                            | FunctionKind::Constructor
                    )
                })
                .count(),
            duplicate_function_spans: 0,
            main_binding_count: main_variables.len(),
            call_edge_count: call_edges.len(),
            unresolved_call_count: self.unresolved_total,
            unresolved_calls_recorded: unresolved_calls.len(),
            functions_with_callers: functions
                .iter()
                .filter(|function| function.caller_count > 0)
                .count(),
            functions_without_callers: functions
                .iter()
                .filter(|function| function.caller_count == 0)
                .count(),
            captured_main_reference_count: functions
                .iter()
                .map(|function| function.main_reads.len() + function.main_writes.len())
                .sum(),
            assignment_class_counts: BTreeMap::new(),
            classification_counts: BTreeMap::new(),
            class_count: 0,
        };
        for variable in main_variables {
            *coverage
                .assignment_class_counts
                .entry(format!("{:?}", variable.assignment_class))
                .or_default() += 1;
            *coverage
                .classification_counts
                .entry(format!("{:?}", variable.classification))
                .or_default() += 1;
        }
        coverage
    }
}

// ===========================================================================
// Source helpers
// ===========================================================================

fn line_starts(source: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index as u32 + 1);
        }
    }
    starts
}

fn assignment_target_property(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            Expression::NumericLiteral(literal) => Some(format_number(literal.value)),
            _ => None,
        },
        _ => None,
    }
}

fn node_id_start(node_id: NodeId, nodes: &AstNodes<'_>) -> u32 {
    nodes.get_node(node_id).span().start
}

/// A context that a definition inside it may not run in, or may run in more
/// than once, whether or not it has a block: a block, `if`/`else` (also
/// braceless), every loop, `switch` and its cases, a labeled statement, `with`,
/// `try`/`catch`/`finally`, a `&&`/`||`/`??`/`?:` operand, a logical
/// assignment's value, an optional chain, a default value and a class field.
/// Every operand of these counts, including ones that always run first: the
/// classification is conservative.
fn is_conditional_context(kind: &AstKind<'_>) -> bool {
    match kind {
        AstKind::BlockStatement(_)
        | AstKind::IfStatement(_)
        | AstKind::ConditionalExpression(_)
        | AstKind::LogicalExpression(_)
        | AstKind::ForStatement(_)
        | AstKind::ForInStatement(_)
        | AstKind::ForOfStatement(_)
        | AstKind::WhileStatement(_)
        | AstKind::DoWhileStatement(_)
        | AstKind::SwitchStatement(_)
        | AstKind::SwitchCase(_)
        | AstKind::LabeledStatement(_)
        | AstKind::WithStatement(_)
        | AstKind::TryStatement(_)
        | AstKind::CatchClause(_)
        | AstKind::ChainExpression(_)
        | AstKind::AssignmentPattern(_)
        | AstKind::AssignmentTargetWithDefault(_)
        | AstKind::PropertyDefinition(_)
        | AstKind::StaticBlock(_) => true,
        AstKind::AssignmentExpression(assignment) => assignment.operator.is_logical(),
        _ => false,
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(format_number(literal.value)),
        PropertyKey::Identifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e18 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn expr_kind(expression: &Expression<'_>) -> &'static str {
    match expression {
        Expression::ArrayExpression(_) => "array",
        Expression::ObjectExpression(_) => "object",
        Expression::FunctionExpression(_) => "function",
        Expression::ArrowFunctionExpression(_) => "arrow",
        Expression::Identifier(_) => "identifier",
        Expression::CallExpression(_) => "call",
        Expression::NewExpression(_) => "new",
        Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => "member",
        Expression::StringLiteral(_) => "string_literal",
        Expression::NumericLiteral(_) => "number_literal",
        Expression::BooleanLiteral(_) => "boolean_literal",
        Expression::NullLiteral(_) => "null_literal",
        Expression::RegExpLiteral(_) => "regexp_literal",
        Expression::TemplateLiteral(_) => "template_literal",
        Expression::UnaryExpression(_) => "unary",
        Expression::BinaryExpression(_) => "binary",
        Expression::LogicalExpression(_) => "logical",
        Expression::ConditionalExpression(_) => "conditional",
        Expression::AssignmentExpression(_) => "assignment",
        Expression::SequenceExpression(_) => "sequence",
        Expression::ThisExpression(_) => "this",
        _ => "other",
    }
}

fn object_of_member<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn member_root<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        Expression::StaticMemberExpression(member) => member_root(&member.object),
        Expression::ComputedMemberExpression(member) => member_root(&member.object),
        Expression::CallExpression(call) => member_root(&call.callee),
        Expression::NewExpression(new) => member_root(&new.callee),
        _ => None,
    }
}

fn is_host_global(name: &str) -> bool {
    matches!(
        name,
        "window"
            | "self"
            | "globalThis"
            | "document"
            | "navigator"
            | "location"
            | "screen"
            | "history"
            | "performance"
            | "console"
            | "crypto"
            | "localStorage"
            | "sessionStorage"
            | "indexedDB"
            | "Object"
            | "Array"
            | "String"
            | "Number"
            | "Boolean"
            | "BigInt"
            | "Symbol"
            | "Math"
            | "JSON"
            | "Date"
            | "RegExp"
            | "Error"
            | "TypeError"
            | "Promise"
            | "Proxy"
            | "Reflect"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "ArrayBuffer"
            | "SharedArrayBuffer"
            | "DataView"
            | "Uint8Array"
            | "Int8Array"
            | "Uint16Array"
            | "Int16Array"
            | "Uint32Array"
            | "Int32Array"
            | "Float32Array"
            | "Float64Array"
            | "Uint8ClampedArray"
            | "BigInt64Array"
            | "BigUint64Array"
            | "TextEncoder"
            | "TextDecoder"
            | "setTimeout"
            | "setInterval"
            | "clearTimeout"
            | "clearInterval"
            | "requestAnimationFrame"
            | "fetch"
            | "Event"
            | "CustomEvent"
            | "dispatchEvent"
            | "addEventListener"
            | "removeEventListener"
            | "Function"
            | "parseInt"
            | "parseFloat"
            | "isNaN"
            | "isFinite"
            | "encodeURI"
            | "encodeURIComponent"
            | "decodeURI"
            | "decodeURIComponent"
            | "atob"
            | "btoa"
            | "URL"
            | "URLSearchParams"
            | "Blob"
            | "File"
            | "FileReader"
            | "FormData"
            | "XMLHttpRequest"
            | "WebSocket"
            | "Worker"
            | "MutationObserver"
            | "IntersectionObserver"
            | "ResizeObserver"
            | "getComputedStyle"
            | "matchMedia"
            | "alert"
            | "confirm"
            | "prompt"
    )
}

fn duplicate_spans(functions: &[FunctionRecord]) -> usize {
    let mut seen = HashSet::new();
    let mut duplicates = 0;
    for function in functions {
        if !seen.insert((function.full_span.start, function.full_span.end)) {
            duplicates += 1;
        }
    }
    duplicates
}

fn sha256_hex(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(source.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

// ===========================================================================
// Public API
// ===========================================================================

/// Extract the complete function/main-variable inventory from exact source.
///
/// This first attempts the structural [`super::extract_program_properties`]
/// pass so the inventory can reuse its proven main-scope identity and
/// signal/wave roles. If that pass cannot identify a main scope, the inventory
/// still runs with its own reachable-scope selection; it only refuses inputs
/// that cannot be parsed or exceed a deterministic budget.
pub fn extract_function_index(source: &str) -> Result<FunctionIndex, FunctionIndexError> {
    extract_function_index_with_limits(source, FunctionIndexLimits::default())
}

/// Extract with explicit deterministic budgets.
pub fn extract_function_index_with_limits(
    source: &str,
    limits: FunctionIndexLimits,
) -> Result<FunctionIndex, FunctionIndexError> {
    if source.trim().is_empty() {
        return Err(FunctionIndexError::InvalidInput(
            "source is empty".to_string(),
        ));
    }
    if source.len() > limits.max_source_bytes {
        return Err(FunctionIndexError::Limit {
            what: "source bytes",
            limit: limits.max_source_bytes,
        });
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    if let Some(error) = parsed.errors.first() {
        return Err(FunctionIndexError::Parse(error.clone()));
    }
    if parsed.panicked {
        return Err(FunctionIndexError::Parse(
            "Oxc parser panicked on the source".to_string(),
        ));
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let properties =
        crate::signals::extract::scanner::extract_program_properties_with_limits(source, limits.program_properties_limits())
            .ok();
    let builder = Builder::new(source, &scoping, &nodes, limits, properties)?;
    builder.finish()
}


#[cfg(test)]
mod alias_cycle_tests {
    use super::*;

    /// Aliases that write each other with several writes each used to be
    /// re-explored at every depth up to `max_alias_depth` (fan-out^16 calls).
    /// The memo keeps it linear; this ran for minutes before.
    #[test]
    fn mutually_aliased_writes_resolve_quickly() {
        let names: Vec<String> = (0..6).map(|i| format!("v{i}")).collect();
        let mut source = String::from("function f() {}\n");
        for name in &names {
            source.push_str(&format!("let {name} = f;\n"));
        }
        for name in &names {
            for other in &names {
                if other != name {
                    source.push_str(&format!("{name} = {other};\n"));
                }
            }
        }
        source.push_str("v0();\n");
        let start = std::time::Instant::now();
        let index = extract_function_index(&source).expect("index");
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "took {:?}", start.elapsed());
        assert!(!index.functions.is_empty());
    }
}
