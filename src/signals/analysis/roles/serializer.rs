//! Package `p06_serializer` finders: the structured-value serializer.
//!
//! The package recognizes the encoding tag table, the unsigned-integer and
//! float writers, the string-table selector, the index-wise array comparison,
//! the extension registry and its member installer, the depth-bound and
//! sentinel extensions, and the encoder class with its methods and callbacks.
//! See `roles/mod.rs` for the recognition rules.
//!
//! # How the proofs chain
//!
//! * The tag table is proven by its own member names. A writer is proven by
//!   the tags it pushes onto its second formal slot.
//! * The extension registry is proven by its constructor and its `extend`
//!   record. The member installer is claimed only when the registry factory
//!   calls the factory's result: a parallel copy with another consumer (or
//!   none) is a different binding and is left alone.
//! * The encoder factory is the function whose direct nested functions include
//!   all six methods, proven by behavior, and whose call with the registry
//!   class has its `encode` member read. Methods are found by what they do, not
//!   by key, so a `class` body and a transpiled descriptor list read the same.
//!   Both shapes have two `encode` functions and behavior tells them apart.
//! * An extension's functions are named by their bodies and by the
//!   `detector`/`serialiser`/`deserialiser` slots that hold them. A slot may be
//!   an object-literal member or a later plain assignment on the same holder;
//!   every relevant write must agree on one function. "Depth bound" is
//!   asserted only for the extension the static encode compares against.
//!
//! Stable member names of this API (`encodeValue`, `keysets`, `stringHist`,
//! `extensions`, ...) link a function to the serializer; each is still proven
//! by behavior. Every binding used as a proof must have a single known value,
//! so a reused binding fails closed.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::Span;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::signals::extract::bindings::{
    call_arguments, numeric_index, object_members_of, ParameterSlots,
};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

const TAGS: &str = "serializer_encoding_tags";
const UINT: &str = "serializer_write_unsigned_int";
const UINT_ALIAS: &str = "serializer_write_unsigned_int_alias";
const SELECT: &str = "serializer_select_string_table";
const SELECT_ALIAS: &str = "serializer_select_string_table_alias";
const SELECT_KEEP: &str = "serializer_string_table_keep_profitable";
const SELECT_SCORE: &str = "serializer_string_table_score_saving";
const SELECT_SORT: &str = "serializer_string_table_compare_saving";
const SELECT_TAKE: &str = "serializer_string_table_take_string";
const EQUAL: &str = "serializer_arrays_equal";
const EQUAL_ALIAS: &str = "serializer_arrays_equal_alias";
const EQUAL_ELEMENT: &str = "serializer_arrays_equal_element";
const INSTALLER_FACTORY: &str = "serializer_class_members_installer_factory";
const INSTALLER: &str = "serializer_class_members_installer";
const INSTALLER_HOLDER: &str = "serializer_class_members_installer_holder";
const REGISTRY_FACTORY: &str = "serializer_extension_registry_factory";
const REGISTRY_CTOR: &str = "serializer_extension_registry_constructor";
const REGISTRY_EXTEND: &str = "serializer_extension_registry_extend";
const DEPTH_DETECT: &str = "serializer_depth_extension_detector";
const DEPTH_SERIALISE: &str = "serializer_depth_extension_serialiser";
const DEPTH_DESERIALISE: &str = "serializer_depth_extension_deserialiser";
const DEPTH_PLACEHOLDER: &str = "serializer_depth_extension_placeholder";
const SENTINEL_DETECT: &str = "serializer_sentinel_extension_detector";
const SENTINEL_SERIALISE: &str = "serializer_sentinel_extension_serialiser";
const SENTINEL_DESERIALISE: &str = "serializer_sentinel_extension_deserialiser";
const FLOAT_TYPED: &str = "serializer_write_float_typed_arrays";
const FLOAT_FALLBACK: &str = "serializer_write_float_fallback";
const FLOAT_HOLDER: &str = "serializer_float_writer_holder";
const ENCODER_FACTORY: &str = "serializer_encoder_class_factory";
const ENCODER_CTOR: &str = "serializer_encoder_constructor";
const ENCODE: &str = "serializer_encoder_encode";
const ENCODE_ELEMENT: &str = "serializer_encoder_emit_body_element";
const FIND_KEYSET: &str = "serializer_encoder_find_keyset_index";
const FIND_KEYSET_MATCH: &str = "serializer_encoder_matches_keyset";
const PUSH_ELEMENTS: &str = "serializer_encoder_push_array_elements";
const PUSH_ELEMENT: &str = "serializer_encoder_encode_array_element";
const ENCODE_VALUE: &str = "serializer_encoder_encode_value";
const DETECTOR_ACCEPTS: &str = "serializer_encoder_extension_detector_accepts";
const BOOLEAN_ELEMENT: &str = "serializer_encoder_is_boolean_element";
const BOOLEAN_ENTRY: &str = "serializer_encoder_is_boolean_entry";
const ENCODE_ENTRY: &str = "serializer_encoder_encode_entry_value";
const STATIC_ENCODE: &str = "serializer_encoder_static_encode";
const RECORD_DEPTH_ID: &str = "serializer_encoder_record_depth_extension_id";
const IS_DEPTH_ENTRY: &str = "serializer_encoder_is_depth_extension_entry";
const REGISTER_OPTION: &str = "serializer_encoder_register_option_extension";

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn alias(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::FunctionAlias }
}

const fn main_binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    main_binding(TAGS, "encodingTags"),
    function(UINT, "writeUnsignedIntBytes"),
    alias(UINT_ALIAS, "lengthByteWriter"),
    function(SELECT, "selectStringTable"),
    alias(SELECT_ALIAS, "stringTableSelector"),
    function(SELECT_KEEP, "keepProfitableString"),
    function(SELECT_SCORE, "scoreStringSaving"),
    function(SELECT_SORT, "compareStringSavingDescending"),
    function(SELECT_TAKE, "takeScoredString"),
    function(EQUAL, "arraysEqualByIndex"),
    alias(EQUAL_ALIAS, "arrayContentsEqual"),
    function(EQUAL_ELEMENT, "elementMatchesAtIndex"),
    function(INSTALLER_FACTORY, "createClassMembersInstaller"),
    function(INSTALLER, "defineClassMembers"),
    main_binding(INSTALLER_HOLDER, "classMembersInstaller"),
    function(REGISTRY_FACTORY, "createExtensionRegistryClass"),
    function(REGISTRY_CTOR, "ExtensionRegistry"),
    function(REGISTRY_EXTEND, "registerExtension"),
    function(DEPTH_DETECT, "neverDetectDepthBound"),
    function(DEPTH_SERIALISE, "rejectDepthBoundSerialise"),
    function(DEPTH_DESERIALISE, "deserialiseDepthBoundPlaceholder"),
    main_binding(DEPTH_PLACEHOLDER, "depthBoundPlaceholder"),
    function(SENTINEL_DETECT, "detectSentinelValue"),
    function(SENTINEL_SERIALISE, "serialiseSentinelAsUndefined"),
    function(SENTINEL_DESERIALISE, "deserialiseSentinelValue"),
    function(FLOAT_TYPED, "writeFloatBytesWithTypedArrays"),
    function(FLOAT_FALLBACK, "writeFloatBytesFallback"),
    main_binding(FLOAT_HOLDER, "floatBytesWriter"),
    function(ENCODER_FACTORY, "createValueEncoderClass"),
    function(ENCODER_CTOR, "constructValueEncoder"),
    function(ENCODE, "encodeWithStringTable"),
    function(ENCODE_ELEMENT, "emitEncodedBodyElement"),
    function(FIND_KEYSET, "findKeysetIndex"),
    function(FIND_KEYSET_MATCH, "matchesKeyset"),
    function(PUSH_ELEMENTS, "pushArrayElements"),
    function(PUSH_ELEMENT, "encodeArrayElement"),
    function(ENCODE_VALUE, "encodeValue"),
    function(DETECTOR_ACCEPTS, "extensionDetectorAccepts"),
    function(BOOLEAN_ELEMENT, "isBooleanElement"),
    function(BOOLEAN_ENTRY, "isBooleanEntryValue"),
    function(ENCODE_ENTRY, "encodeEntryValue"),
    function(STATIC_ENCODE, "encodeWithFreshEncoder"),
    function(RECORD_DEPTH_ID, "recordDepthBoundExtensionId"),
    function(IS_DEPTH_ENTRY, "isDepthBoundExtensionEntry"),
    function(REGISTER_OPTION, "registerOptionExtension"),
];

/// The member names that identify the encoding tag table.
const TAG_TABLE_MEMBERS: &[&str] = &[
    "UINT14_BASE", "UINT16", "UINT24", "UINT32", "UINT64", "NINT4_BASE", "FLOAT32", "DOUBLE64",
    "STR5_BASE", "STRREF", "STRLUT", "ARRAY8", "BARRAY8", "MAP_", "BMAP_", "EXTENSION",
];

/// What the encoder constructor writes.
const ENCODER_STATE: &[&str] =
    &["keysets", "stringHist", "stringPlaceholders", "remainingDepth", "depthBoundExtensionPoint"];

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let index = Index::new(ctx);
    let mut run = Run { index: &index, claims: Vec::new() };
    let tags = run.tag_tables();
    if tags.is_empty() {
        return run.claims;
    }
    run.unsigned_writers(&tags);
    run.float_writers(&tags);
    run.string_table_selectors();
    let equal = run.array_comparisons();
    let registries = run.registries();
    run.installers(&registries);
    run.encoders(&tags, &equal, &registries);
    run.claims
}

// ===========================================================================
// Index of one parse
// ===========================================================================

#[derive(Clone, Copy)]
enum FnNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

struct FnEntry<'a> {
    node: NodeId,
    span: Span,
    func: FnNode<'a>,
    parent: Option<usize>,
}

impl<'a> FnEntry<'a> {
    fn statements(&self) -> Option<&'a [Statement<'a>]> {
        match self.func {
            FnNode::Function(function) => {
                function.body.as_ref().map(|body| body.statements.as_slice())
            }
            FnNode::Arrow(arrow) => Some(arrow.body.statements.as_slice()),
        }
    }

    fn formal_count(&self) -> usize {
        match self.func {
            FnNode::Function(function) => function.params.items.len(),
            FnNode::Arrow(arrow) => arrow.params.items.len(),
        }
    }

    fn is_arrow_expression(&self) -> bool {
        matches!(self.func, FnNode::Arrow(arrow) if arrow.expression)
    }
}

/// The nodes a function owns directly, without its nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a ReturnStatement<'a>>,
    ifs: Vec<&'a IfStatement<'a>>,
    members: Vec<(&'a Expression<'a>, &'a str)>,
    computed: Vec<&'a ComputedMemberExpression<'a>>,
    strict_equalities: Vec<&'a BinaryExpression<'a>>,
    /// Names of unresolved identifiers (globals) this function reads.
    globals: BTreeSet<&'a str>,
    written_keys: BTreeSet<&'a str>,
    nested: Vec<usize>,
    throws: usize,
    this_instanceof: bool,
}

/// A call seen through its receiver: `r.m(a)`, `r.m.call(r, a)` and
/// `[].m.call(r, a)` all read as method `m` on `r` with arguments `[a]`.
struct MethodCall<'a> {
    receiver: &'a Expression<'a>,
    name: &'a str,
    args: Vec<&'a Expression<'a>>,
}

struct Index<'c, 'a> {
    ctx: &'c FinderCtx<'c, 'a>,
    slots: ParameterSlots<'c, 'a>,
    fns: Vec<FnEntry<'a>>,
    own: Vec<Own<'a>>,
    fn_of_node: Vec<Option<usize>>,
    decl_fn: BTreeMap<SymbolId, usize>,
    /// Every initializer and plain assignment of a symbol.
    writes: BTreeMap<SymbolId, Vec<&'a Expression<'a>>>,
    /// Every member read, wherever it sits (MAIN may own no function).
    members: Vec<(&'a Expression<'a>, &'a str)>,
    /// Every member assignment, with the function that owns it.
    assignments: Vec<(&'a AssignmentExpression<'a>, Option<usize>)>,
}

impl<'c, 'a> Index<'c, 'a> {
    fn new(ctx: &'c FinderCtx<'c, 'a>) -> Self {
        let count = ctx.nodes.len();
        let mut index = Self {
            ctx,
            slots: ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes),
            fns: Vec::new(),
            own: Vec::new(),
            fn_of_node: vec![None; count],
            decl_fn: BTreeMap::new(),
            writes: BTreeMap::new(),
            members: Vec::new(),
            assignments: Vec::new(),
        };
        let mut owners: Vec<Option<usize>> = vec![None; count];
        let nodes = ctx.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let at = id.index();
            let parent = nodes.parent_id(id);
            let owner = if parent == id || parent.index() >= at {
                None
            } else {
                index.fn_of_node[parent.index()].or(owners[parent.index()])
            };
            owners[at] = owner;
            let func = match node.kind() {
                AstKind::Function(function) => Some((function.span, FnNode::Function(function))),
                AstKind::ArrowFunctionExpression(arrow) => Some((arrow.span, FnNode::Arrow(arrow))),
                _ => None,
            };
            if let Some((span, func)) = func {
                let entry = index.fns.len();
                index.fns.push(FnEntry { node: id, span, func, parent: owner });
                index.own.push(Own::default());
                index.fn_of_node[at] = Some(entry);
                if let Some(owner) = owner {
                    index.own[owner].nested.push(entry);
                }
                if let FnNode::Function(function) = func {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        index.decl_fn.insert(symbol, entry);
                    }
                }
            } else {
                index.record(node.kind(), owner);
            }
        }
        index
    }

    fn record(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::StaticMemberExpression(member) => {
                self.members.push((&member.object, member.property.name.as_str()));
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Expression::StringLiteral(literal) = &member.expression {
                    self.members.push((&member.object, literal.value.as_str()));
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                {
                    if let Some(symbol) = binding.symbol_id.get() {
                        self.writes.entry(symbol).or_default().push(init);
                    }
                }
            }
            AstKind::AssignmentExpression(assignment) => {
                self.assignments.push((assignment, owner));
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if assignment.operator == AssignmentOperator::Assign {
                        if let Some(symbol) =
                            analysis::identifier_symbol(identifier, self.ctx.scoping)
                        {
                            self.writes.entry(symbol).or_default().push(&assignment.right);
                        }
                    }
                }
            }
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::ReturnStatement(statement) => own.returns.push(statement),
            AstKind::IfStatement(statement) => own.ifs.push(statement),
            AstKind::ThrowStatement(_) => own.throws += 1,
            AstKind::AssignmentExpression(assignment) => {
                own.assignments.push(assignment);
                if let Some((_, Some(name))) = member_target(&assignment.left) {
                    own.written_keys.insert(name);
                }
            }
            AstKind::StaticMemberExpression(member) => {
                own.members.push((&member.object, member.property.name.as_str()));
            }
            AstKind::ComputedMemberExpression(member) => {
                own.computed.push(member);
                if let Expression::StringLiteral(literal) = &member.expression {
                    own.members.push((&member.object, literal.value.as_str()));
                }
            }
            AstKind::BinaryExpression(binary) => match binary.operator {
                BinaryOperator::StrictEquality => own.strict_equalities.push(binary),
                BinaryOperator::Instanceof
                    if matches!(strip(&binary.left), Expression::ThisExpression(_)) =>
                {
                    own.this_instanceof = true;
                }
                _ => {}
            },
            AstKind::IdentifierReference(identifier) => {
                if analysis::identifier_symbol(identifier, self.ctx.scoping).is_none() {
                    own.globals.insert(identifier.name.as_str());
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Values
    // -----------------------------------------------------------------------

    /// Every value of a symbol, or `None` when a write is not a plain
    /// initializer or assignment or the symbol is declared more than once.
    fn values(&self, symbol: SymbolId) -> Option<&[&'a Expression<'a>]> {
        let scoping = self.ctx.scoping;
        if scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let values = self.writes.get(&symbol)?;
        let initialized = usize::from(matches!(
            self.ctx.nodes.kind(scoping.symbol_declaration(symbol)),
            AstKind::VariableDeclarator(declarator) if declarator.init.is_some()
        ));
        let written = scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        (written + initialized == values.len()).then_some(values.as_slice())
    }

    /// The one value of a symbol with a single write.
    fn value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.values(symbol)? {
            [value] => Some(strip(value)),
            _ => None,
        }
    }

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.ctx.scoping)
    }

    /// The function that owns `node`, or `None` for the main program.
    fn owner_of_node(&self, node: NodeId) -> Option<usize> {
        let mut current = node;
        loop {
            if let Some(at) = self.fn_of_node.get(current.index()).copied().flatten() {
                return Some(at);
            }
            let parent = self.ctx.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
        }
    }

    /// The function that declares `symbol`, or `None` for a main binding.
    fn owner_of_symbol(&self, symbol: SymbolId) -> Option<usize> {
        self.owner_of_node(self.ctx.scoping.symbol_declaration(symbol))
    }

    /// The one function written to `holder.name`: a literal member, a later
    /// `holder.name = function` assignment, or both when they agree.
    ///
    /// A `void 0` literal is the transpiled placeholder for a member filled in
    /// later. Every matching assignment must be a plain write in the holder's
    /// own function scope, after the literal, and resolve to the same function;
    /// otherwise the slot is refused. This keeps a wrong receiver, an ambiguous
    /// reassignment and a nested donor from donating a member.
    fn extension_member(
        &self,
        holder: SymbolId,
        literal: Option<&'a Expression<'a>>,
        literal_end: u32,
        name: &str,
    ) -> Option<usize> {
        let mut found = match literal {
            Some(value) if is_void_zero(value) => None,
            Some(value) => Some(self.fn_value(value)?),
            None => None,
        };
        let owner = self.owner_of_symbol(holder);
        for (assignment, assignment_owner) in &self.assignments {
            let Some((target, member)) = member_target(&assignment.left) else {
                continue;
            };
            if member != Some(name) || self.symbol(target) != Some(holder) {
                continue;
            }
            if assignment.operator != AssignmentOperator::Assign
                || assignment.span.start < literal_end
                || *assignment_owner != owner
            {
                return None;
            }
            let value = self.fn_value(&assignment.right)?;
            if found.is_some_and(|previous| previous != value) {
                return None;
            }
            found = Some(value);
        }
        found
    }

    /// The formal slot of function `at` an expression reads, directly or
    /// through a local whose single value is that slot.
    fn slot_of(&self, expression: &Expression<'a>, at: usize) -> Option<u32> {
        let expression = strip(expression);
        if let Some(slot) = self.slots.parameter_slot(expression) {
            return (slot.owner == self.fns[at].node).then_some(slot.index);
        }
        let value = self.value(self.symbol(expression)?)?;
        let slot = self.slots.parameter_slot(value)?;
        (slot.owner == self.fns[at].node).then_some(slot.index)
    }

    /// The function an expression evaluates to, through function declarations
    /// and bindings whose single value is one.
    fn fn_value(&self, expression: &Expression<'a>) -> Option<usize> {
        self.fn_value_at(expression, 0)
    }

    fn fn_value_at(&self, expression: &Expression<'a>, depth: usize) -> Option<usize> {
        if depth > 4 {
            return None;
        }
        let expression = strip(expression);
        let node = match expression {
            Expression::FunctionExpression(function) => Some(function.node_id.get()),
            Expression::ArrowFunctionExpression(arrow) => Some(arrow.node_id.get()),
            _ => None,
        };
        if let Some(node) = node {
            return self.fn_of_node.get(node.index()).copied().flatten();
        }
        let symbol = self.symbol(expression)?;
        if let Some(entry) = self.decl_fn.get(&symbol) {
            let reassigned = self.writes.contains_key(&symbol);
            let declarations = self.ctx.scoping.symbol_declarations(symbol).count();
            return (!reassigned && declarations == 1).then_some(*entry);
        }
        self.fn_value_at(self.value(symbol)?, depth + 1)
    }

    fn same_ref(&self, left: &Expression<'a>, right: &Expression<'a>) -> bool {
        let (left, right) = (strip(left), strip(right));
        if is_this(left) {
            return is_this(right);
        }
        if let (Some(left), Some(right)) = (self.symbol(left), self.symbol(right)) {
            return left == right;
        }
        match (self.slots.parameter_slot(left), self.slots.parameter_slot(right)) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        }
    }

    fn method_call(&self, call: &'a CallExpression<'a>) -> Option<MethodCall<'a>> {
        let args = call_arguments(call);
        let (object, name) = member_parts(&call.callee)?;
        if name == "call" {
            if let Some((inner, method)) = member_parts(object) {
                let first = *args.first()?;
                if is_empty_array(inner) || self.same_ref(inner, first) {
                    return Some(MethodCall {
                        receiver: first,
                        name: method,
                        args: args[1..].to_vec(),
                    });
                }
            }
        }
        Some(MethodCall { receiver: object, name, args })
    }

    fn method_calls(&self, at: usize, name: &str) -> Vec<MethodCall<'a>> {
        self.own[at]
            .calls
            .iter()
            .filter_map(|call| self.method_call(call))
            .filter(|call| call.name == name)
            .collect()
    }

    fn calls_on_this(&self, at: usize, name: &str) -> bool {
        self.method_calls(at, name).iter().any(|call| is_this(call.receiver))
    }

    fn reads_this_member(&self, at: usize, name: &str) -> bool {
        self.own[at].members.iter().any(|(object, member)| *member == name && is_this(object))
    }

    /// The functions passed as the first argument of `.name(...)` in `at`.
    fn callbacks(&self, at: usize, name: &str) -> Vec<usize> {
        self.method_calls(at, name)
            .iter()
            .filter_map(|call| call.args.first().and_then(|arg| self.fn_value(arg)))
            .collect()
    }

    /// The returned expression of a body that is exactly one return.
    fn single_return(&self, at: usize) -> Option<&'a Expression<'a>> {
        let entry = &self.fns[at];
        match entry.statements()? {
            [Statement::ReturnStatement(statement)] => statement.argument.as_ref().map(strip),
            [Statement::ExpressionStatement(statement)] if entry.is_arrow_expression() => {
                Some(strip(&statement.expression))
            }
            _ => None,
        }
    }

    /// A body with no effect: empty, or a single `return void 0`.
    fn returns_nothing(&self, at: usize) -> bool {
        let Some(statements) = self.fns[at].statements() else {
            return false;
        };
        statements.is_empty() || self.single_return(at).is_some_and(is_void_zero)
    }

    /// The tag names of a tag table that function `at` reads.
    fn tag_reads(&self, at: usize, tags: &BTreeSet<SymbolId>) -> BTreeSet<&'a str> {
        self.own[at]
            .members
            .iter()
            .filter(|(object, _)| self.symbol(object).is_some_and(|symbol| tags.contains(&symbol)))
            .map(|(_, name)| *name)
            .collect()
    }

    fn reads_tags(&self, at: usize, tags: &BTreeSet<SymbolId>, wanted: &[&str]) -> bool {
        let reads = self.tag_reads(at, tags);
        wanted.iter().all(|tag| reads.contains(tag))
    }

    /// The `.push` calls of `at` onto its formal slot `slot`.
    fn pushes_onto(&self, at: usize, slot: u32) -> Vec<MethodCall<'a>> {
        self.method_calls(at, "push")
            .into_iter()
            .filter(|call| self.slot_of(call.receiver, at) == Some(slot))
            .collect()
    }

    fn is_global_member(&self, expression: &'a Expression<'a>, object: &str, member: &str) -> bool {
        member_parts(expression).is_some_and(|(target, name)| {
            name == member
                && matches!(strip(target), Expression::Identifier(identifier)
                    if identifier.name == object
                        && analysis::identifier_symbol(identifier, self.ctx.scoping).is_none())
        })
    }
}

// ===========================================================================
// Small AST readers
// ===========================================================================

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    match expression {
        Expression::ParenthesizedExpression(inner) => strip(&inner.expression),
        _ => expression,
    }
}

/// `object.name` or `object["name"]`.
fn member_parts<'a>(expression: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match strip(expression) {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

/// The object and literal name of a member assignment target. The name is
/// `None` for a computed target with a non-literal key.
fn member_target<'a>(
    target: &'a AssignmentTarget<'a>,
) -> Option<(&'a Expression<'a>, Option<&'a str>)> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            Some((&member.object, Some(member.property.name.as_str())))
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            let name = match &member.expression {
                Expression::StringLiteral(literal) => Some(literal.value.as_str()),
                _ => None,
            };
            Some((&member.object, name))
        }
        _ => None,
    }
}

fn is_this(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::ThisExpression(_))
}

fn is_empty_array(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::ArrayExpression(array) if array.elements.is_empty())
}

fn is_empty_object(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::ObjectExpression(object) if object.properties.is_empty())
}

fn is_void_zero(expression: &Expression<'_>) -> bool {
    matches!(strip(expression), Expression::UnaryExpression(unary)
        if unary.operator == UnaryOperator::Void)
}

/// `typeof operand === "boolean"`, returning the operand.
fn typeof_boolean<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::BinaryExpression(binary) = strip(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::StrictEquality {
        return None;
    }
    let Expression::UnaryExpression(unary) = strip(&binary.left) else {
        return None;
    };
    let boolean = matches!(strip(&binary.right), Expression::StringLiteral(literal)
        if literal.value == "boolean");
    (unary.operator == UnaryOperator::Typeof && boolean).then_some(&unary.argument)
}

// ===========================================================================
// Recognizers
// ===========================================================================

/// One proven class-extension registry.
struct Registry {
    factory: usize,
    holders: BTreeSet<SymbolId>,
    /// The extend record is a hoisted local rather than an inline literal.
    /// A hoisted registry proves only its own factory, constructor and extend
    /// ownership here; adjacent installer, sentinel and encoder claims remain
    /// tied to inline records so an accepted hoisted seed keeps every role the
    /// current source already names.
    hoisted: bool,
}

/// An object holding the three functions of one extension.
struct Extension<'a> {
    holder: SymbolId,
    detector: usize,
    serialiser: usize,
    deserialiser: usize,
    deserialised: &'a Expression<'a>,
    /// The serialiser was found in the object literal, not only in a later
    /// member assignment.
    serialiser_from_literal: bool,
}

struct Run<'r, 'c, 'a> {
    index: &'r Index<'c, 'a>,
    claims: Vec<FinderClaim>,
}

impl<'a> Run<'_, '_, 'a> {
    fn claim(&mut self, role: &'static str, at: usize, alias_role: Option<&'static str>, why: &str) {
        self.claims.push(FinderClaim::Function {
            role,
            function_span: self.index.fns[at].span,
            alias_role,
            confidence: Confidence::High,
            evidence: vec![why.to_string()],
        });
    }

    fn claim_main(&mut self, role: &'static str, symbol: SymbolId, why: &str) {
        self.claims.push(FinderClaim::MainBinding {
            role,
            identifier_span: self.index.ctx.scoping.symbol_span(symbol),
            confidence: Confidence::High,
            evidence: vec![why.to_string()],
        });
    }

    fn tag_tables(&mut self) -> BTreeSet<SymbolId> {
        let index = self.index;
        let mut tables = BTreeSet::new();
        for symbol in index.writes.keys() {
            let Some(Expression::ObjectExpression(object)) = index.value(*symbol) else {
                continue;
            };
            let members = object_members_of(object);
            let complete = TAG_TABLE_MEMBERS
                .iter()
                .all(|tag| members.iter().any(|member| member.name == *tag));
            if complete {
                tables.insert(*symbol);
                self.claim_main(TAGS, *symbol, "single-value object with the full encoding tag member set");
            }
        }
        tables
    }

    fn unsigned_writers(&mut self, tags: &BTreeSet<SymbolId>) {
        let index = self.index;
        for at in 0..index.fns.len() {
            if !index.reads_tags(at, tags, &["UINT16", "UINT24", "UINT32", "UINT64"]) {
                continue;
            }
            let pushes = index.pushes_onto(at, 1);
            let raw = pushes.iter().any(|call| {
                matches!(call.args.as_slice(), [only] if index.slot_of(only, at) == Some(0))
            });
            let tagged = ["UINT16", "UINT24", "UINT32", "UINT64"].iter().all(|tag| {
                pushes.iter().any(|call| {
                    call.args.iter().any(|arg| {
                        member_parts(arg).is_some_and(|(object, name)| {
                            name == *tag
                                && index.symbol(object).is_some_and(|symbol| tags.contains(&symbol))
                        })
                    })
                })
            });
            if raw && tagged {
                self.claim(UINT, at, Some(UINT_ALIAS), "pushes slot 0 raw or behind the UINT16/24/32/64 tags onto slot 1");
            }
        }
    }

    fn float_writers(&mut self, tags: &BTreeSet<SymbolId>) {
        let index = self.index;
        let (mut typed, mut fallback) = (BTreeSet::new(), BTreeSet::new());
        for at in 0..index.fns.len() {
            if !index.reads_tags(at, tags, &["FLOAT32", "DOUBLE64"])
                || index.pushes_onto(at, 1).is_empty()
            {
                continue;
            }
            let own = &index.own[at];
            let uses_typed = ["Float32Array", "Float64Array", "Uint8Array"]
                .iter()
                .all(|name| own.globals.contains(name));
            let uses_log = own.members.iter().any(|(_, name)| *name == "LN2")
                && own.members.iter().any(|(_, name)| *name == "log");
            if uses_typed {
                typed.insert(at);
                self.claim(FLOAT_TYPED, at, None, "pushes the FLOAT32/DOUBLE64 tag and typed-array bytes onto slot 1");
            } else if uses_log && !own.globals.contains("Float32Array") {
                fallback.insert(at);
                self.claim(FLOAT_FALLBACK, at, None, "pushes the FLOAT32/DOUBLE64 tag and bytes assembled with Math.log onto slot 1");
            }
        }
        for (symbol, values) in &index.writes {
            let held: Option<BTreeSet<usize>> =
                values.iter().map(|value| index.fn_value(value)).collect();
            let Some(held) = held else {
                continue;
            };
            let both = held.len() == 2
                && values.len() == 2
                && index.values(*symbol).is_some()
                && held.iter().any(|at| typed.contains(at))
                && held.iter().any(|at| fallback.contains(at));
            if both {
                self.claim_main(FLOAT_HOLDER, *symbol, "written only with the typed-array and the fallback float writer");
            }
        }
    }

    fn string_table_selectors(&mut self) {
        let index = self.index;
        for at in 0..index.fns.len() {
            let [statement] = index.own[at].returns.as_slice() else {
                continue;
            };
            let Some(chain) = statement.argument.as_ref().and_then(|value| key_chain(index, value, at))
            else {
                continue;
            };
            let [keep, score, sort, take] = chain;
            let reads_count = |callback: usize| {
                index.own[callback].computed.iter().any(|member| {
                    index.slot_of(&member.object, at) == Some(0)
                        && index.slot_of(&member.expression, callback) == Some(0)
                })
            };
            let descending = index.single_return(sort).is_some_and(|value| {
                let Expression::BinaryExpression(binary) = value else {
                    return false;
                };
                binary.operator == BinaryOperator::Subtraction
                    && pair_slot(index, &binary.left, sort, 1) == Some(1)
                    && pair_slot(index, &binary.right, sort, 1) == Some(0)
            });
            let takes = index
                .single_return(take)
                .is_some_and(|value| pair_slot(index, value, take, 0) == Some(0));
            if !(reads_count(keep) && reads_count(score) && descending && takes) {
                continue;
            }
            let why = "returns Object.keys(slot 0) filtered, scored, sorted descending, sliced and mapped back to the key";
            self.claim(SELECT, at, Some(SELECT_ALIAS), why);
            self.claim(SELECT_KEEP, keep, None, "filter step of the string-table selector, reads the key's count");
            self.claim(SELECT_SCORE, score, None, "map step of the string-table selector, pairs the key with its saving");
            self.claim(SELECT_SORT, sort, None, "sort step of the string-table selector, descending on the pair's second item");
            self.claim(SELECT_TAKE, take, None, "last map of the string-table selector, returns the pair's first item");
        }
    }

    fn array_comparisons(&mut self) -> BTreeSet<usize> {
        let index = self.index;
        let mut found = BTreeSet::new();
        for at in 0..index.fns.len() {
            let same_length = index.own[at].ifs.iter().any(|statement| {
                let Expression::BinaryExpression(binary) = strip(&statement.test) else {
                    return false;
                };
                let length_slot = |side: &'a Expression<'a>| {
                    member_parts(side)
                        .filter(|(_, name)| *name == "length")
                        .and_then(|(object, _)| index.slot_of(object, at))
                };
                binary.operator == BinaryOperator::StrictEquality
                    && length_slot(&binary.left) == Some(0)
                    && length_slot(&binary.right) == Some(1)
            });
            if !same_length {
                continue;
            }
            for call in index.method_calls(at, "every") {
                if index.slot_of(call.receiver, at) != Some(0) {
                    continue;
                }
                let Some(callback) = call.args.first().and_then(|arg| index.fn_value(arg)) else {
                    continue;
                };
                if element_matches(index, callback, at) {
                    found.insert(at);
                    self.claim(EQUAL, at, Some(EQUAL_ALIAS), "equal length and every slot 0 element strictly equals slot 1 at its index");
                    self.claim(EQUAL_ELEMENT, callback, None, "every callback comparing an element with the other array at its index");
                }
            }
        }
        found
    }

    fn registries(&mut self) -> Vec<Registry> {
        let index = self.index;
        let mut found = Vec::new();
        for factory in 0..index.fns.len() {
            let nested = &index.own[factory].nested;
            let constructors: Vec<usize> =
                nested.iter().copied().filter(|at| is_registry_constructor(index, *at)).collect();
            let extends: Vec<(usize, bool)> = nested
                .iter()
                .copied()
                .filter_map(|at| extend_record(index, at).map(|(_, hoisted)| (at, hoisted)))
                .collect();
            let [statement] = index.own[factory].returns.as_slice() else {
                continue;
            };
            let returned = statement.argument.as_ref().and_then(|value| index.fn_value(value));
            let (Some(returned), [(extend, hoisted)]) = (returned, extends.as_slice()) else {
                continue;
            };
            if !constructors.contains(&returned) {
                continue;
            }
            self.claim(REGISTRY_FACTORY, factory, None, "returns the class holding extensions and defines its extend record writer");
            self.claim(REGISTRY_CTOR, returned, None, "this-instanceof guard and this.extensions = Object.create(null)");
            self.claim(REGISTRY_EXTEND, *extend, None, "stores {detector, serialiser, deserialiser} from slots 1-3 under slot 0");
            let holders = holders_of_call(index, factory);
            found.push(Registry {
                factory,
                holders,
                hoisted: *hoisted,
            });
        }
        found
    }

    fn installers(&mut self, registries: &[Registry]) {
        let index = self.index;
        for installer in 0..index.fns.len() {
            if !is_member_installer(index, installer) {
                continue;
            }
            let Some(factory) = index.fns[installer].parent else {
                continue;
            };
            let own = &index.own[factory];
            let returns_installer = matches!(own.returns.as_slice(), [statement]
                if statement.argument.as_ref().and_then(|value| index.fn_value(value)) == Some(installer));
            if !returns_installer || own.nested.len() != 1 || index.fns[factory].formal_count() != 0 {
                continue;
            }
            let consumed: Vec<SymbolId> = holders_of_call(index, factory)
                .into_iter()
                .filter(|holder| {
                    registries.iter().any(|registry| {
                        !registry.hoisted
                            && index.own[registry.factory]
                                .calls
                                .iter()
                                .any(|call| index.symbol(&call.callee) == Some(*holder))
                    })
                })
                .collect();
            if consumed.is_empty() {
                continue;
            }
            self.claim(INSTALLER_FACTORY, factory, None, "returns the member installer, whose result the extension registry factory calls");
            self.claim(INSTALLER, installer, None, "defines slot 1 descriptors on slot 0's prototype and slot 2 descriptors on slot 0, returns slot 0");
            for holder in consumed {
                self.claim_main(INSTALLER_HOLDER, holder, "single-value holder of the member installer, called by the extension registry factory");
            }
        }
    }

    fn extensions(&self) -> Vec<Extension<'a>> {
        let index = self.index;
        let mut found = Vec::new();
        for symbol in index.writes.keys() {
            let Some(Expression::ObjectExpression(object)) = index.value(*symbol) else {
                continue;
            };
            let members = object_members_of(object);
            let literal = |name: &str| -> Option<&'a Expression<'a>> {
                members
                    .iter()
                    .find(|member| member.name == name)
                    .map(|member| member.value)
            };
            let (Some(detector), Some(serialiser), Some(deserialiser)) = (
                index.extension_member(*symbol, literal("detector"), object.span.end, "detector"),
                index.extension_member(
                    *symbol,
                    literal("serialiser"),
                    object.span.end,
                    "serialiser",
                ),
                index.extension_member(
                    *symbol,
                    literal("deserialiser"),
                    object.span.end,
                    "deserialiser",
                ),
            ) else {
                continue;
            };
            let Some(deserialised) = index.single_return(deserialiser) else {
                continue;
            };
            let serialiser_from_literal =
                literal("serialiser").is_some_and(|value| !is_void_zero(value));
            found.push(Extension {
                holder: *symbol,
                detector,
                serialiser,
                deserialiser,
                deserialised,
                serialiser_from_literal,
            });
        }
        found
    }

    fn sentinel_extensions(&mut self, extensions: &[Extension<'a>]) {
        let index = self.index;
        for extension in extensions {
            let Some(sentinel) = index.symbol(extension.deserialised) else {
                continue;
            };
            let detects = index.single_return(extension.detector).is_some_and(|value| {
                let Expression::BinaryExpression(binary) = value else {
                    return false;
                };
                binary.operator == BinaryOperator::StrictEquality
                    && index.slot_of(&binary.left, extension.detector) == Some(0)
                    && index.symbol(&binary.right) == Some(sentinel)
            });
            if detects && index.returns_nothing(extension.serialiser) {
                self.claim(SENTINEL_DETECT, extension.detector, None, "extension detector testing slot 0 against the value its deserialiser returns");
                self.claim(SENTINEL_SERIALISE, extension.serialiser, None, "extension serialiser of the sentinel, returns undefined");
                // A serialiser filled in after the literal is the new proof
                // shape: the deserialiser still proves the sentinel identity,
                // but its independently proven captured-state name is kept.
                if extension.serialiser_from_literal {
                    self.claim(SENTINEL_DESERIALISE, extension.deserialiser, None, "extension deserialiser returning the value its detector tests");
                }
            }
        }
    }

    /// The depth-bound extension among `extensions`, reached from the value a
    /// static-encode callback compares extension entries against.
    fn depth_extension(&mut self, compared: SymbolId, extensions: &[Extension<'a>]) {
        let index = self.index;
        let mut symbol = compared;
        for _ in 0..4 {
            if let Some(extension) = extensions.iter().find(|extension| extension.holder == symbol) {
                let never = index.single_return(extension.detector).is_some_and(
                    |value| matches!(value, Expression::BooleanLiteral(literal) if !literal.value),
                );
                let rejects = index.own[extension.serialiser].throws == 1
                    && matches!(index.fns[extension.serialiser].statements(), Some([_]));
                let placeholder = index
                    .symbol(extension.deserialised)
                    .filter(|symbol| index.value(*symbol).is_some_and(is_empty_object));
                if let (true, true, Some(placeholder)) = (never, rejects, placeholder) {
                    if extension.serialiser_from_literal {
                        self.claim(DEPTH_DETECT, extension.detector, None, "detector of the extension static encode requires for depthBound, always false");
                        self.claim(DEPTH_SERIALISE, extension.serialiser, None, "serialiser of the depth-bound extension, only throws");
                        self.claim(DEPTH_DESERIALISE, extension.deserialiser, None, "deserialiser of the depth-bound extension, returns its placeholder object");
                        self.claim_main(DEPTH_PLACEHOLDER, placeholder, "empty object returned by the depth-bound extension's deserialiser");
                    }
                }
                return;
            }
            match index.value(symbol).and_then(|value| index.symbol(value)) {
                Some(next) => symbol = next,
                None => return,
            }
        }
    }

    fn encoders(
        &mut self,
        tags: &BTreeSet<SymbolId>,
        equal: &BTreeSet<usize>,
        registries: &[Registry],
    ) {
        let index = self.index;
        let extensions = self.extensions();
        if registries.iter().any(|registry| !registry.hoisted) {
            self.sentinel_extensions(&extensions);
        }
        for factory in 0..index.fns.len() {
            let nested = index.own[factory].nested.as_slice();
            let pick = |test: &dyn Fn(usize) -> bool| -> Vec<usize> {
                nested.iter().copied().filter(|at| test(*at)).collect()
            };
            let constructors = pick(&|at| {
                ENCODER_STATE.iter().all(|key| index.own[at].written_keys.contains(key))
            });
            if constructors.is_empty() || !encoder_factory_consumed(index, factory, registries) {
                continue;
            }
            let encodes = pick(&|at| is_instance_encode(index, at, tags));
            let values = pick(&|at| is_encode_value(index, at, tags));
            let finds = pick(&|at| is_find_keyset(index, at, equal));
            let pushes = pick(&|at| is_push_elements(index, at).is_some());
            let statics = pick(&|at| is_static_encode(index, at));
            if [&encodes, &values, &finds, &pushes, &statics].iter().any(|found| found.is_empty()) {
                continue;
            }
            self.claim(ENCODER_FACTORY, factory, None, "defines the encoder methods and is called with the extension registry class, its encode member read");
            for at in constructors {
                self.claim(ENCODER_CTOR, at, None, "writes keysets, stringHist, stringPlaceholders, remainingDepth and depthBoundExtensionPoint");
            }
            for at in encodes {
                self.claim(ENCODE, at, None, "encodes through this.encodeValue, reads this.stringHist and writes the STRLUT header");
                for callback in index.callbacks(at, "forEach") {
                    if index.reads_tags(callback, tags, &["STRREF", "CSTRING"]) {
                        self.claim(ENCODE_ELEMENT, callback, None, "forEach callback of encode, emits STRREF or an inline string for each body element");
                    }
                }
            }
            for at in finds {
                self.claim(FIND_KEYSET, at, None, "searches this.keysets with the array comparison; its body also holds an append to this.keysets");
                for matcher in index.own[at].nested.iter().copied() {
                    if returns_call_to(index, matcher, equal) {
                        self.claim(FIND_KEYSET_MATCH, matcher, None, "predicate of findKeysetIndex, the array comparison against the wanted keyset");
                    }
                }
            }
            for at in pushes {
                self.claim(PUSH_ELEMENTS, at, None, "forEach over slot 0 calling encodeValue with slot 1 as the output");
                if let Some(callback) = is_push_elements(index, at) {
                    self.claim(PUSH_ELEMENT, callback, None, "forEach callback encoding one element into the output");
                }
            }
            for at in values {
                self.claim(ENCODE_VALUE, at, None, "tests this.extensions, emits the FALSE/TRUE/NULL/UNDEFINED/EXTENSION tags, recurses");
                self.encode_value_callbacks(at);
            }
            for at in statics {
                self.claim(STATIC_ENCODE, at, None, "builds an instance, registers option extensions on it through extend, returns its encode");
                self.static_encode_callbacks(at, &extensions);
            }
        }
    }

    fn encode_value_callbacks(&mut self, at: usize) {
        let index = self.index;
        for nested in index.own[at].nested.iter().copied() {
            let accepts = index.single_return(nested).is_some_and(|value| {
                matches!(value, Expression::CallExpression(call)
                    if member_parts(&call.callee).is_some_and(|(_, name)| name == "detector"))
            });
            if accepts {
                self.claim(DETECTOR_ACCEPTS, nested, None, "local predicate calling the extension's detector on the value");
            }
        }
        for callback in index.callbacks(at, "every") {
            let tested = index.single_return(callback).and_then(typeof_boolean);
            if tested.is_some_and(|operand| index.slot_of(operand, callback) == Some(0)) {
                self.claim(BOOLEAN_ELEMENT, callback, None, "every callback, typeof element === \"boolean\"");
            } else if tested.is_some_and(|operand| {
                matches!(strip(operand), Expression::ComputedMemberExpression(member)
                    if index.slot_of(&member.expression, callback) == Some(0))
            }) {
                self.claim(BOOLEAN_ENTRY, callback, None, "every callback, typeof value[key] === \"boolean\"");
            }
        }
        for callback in index.callbacks(at, "forEach") {
            let encodes = index.method_calls(callback, "encodeValue").iter().any(|call| {
                matches!(call.args.first().map(|arg| strip(arg)),
                    Some(Expression::ComputedMemberExpression(member))
                        if index.slot_of(&member.expression, callback) == Some(0))
            });
            if encodes {
                self.claim(ENCODE_ENTRY, callback, None, "forEach callback encoding value[key] into the output");
            }
        }
    }

    fn static_encode_callbacks(&mut self, at: usize, extensions: &[Extension<'a>]) {
        let index = self.index;
        let compared = |callback: usize| {
            index.own[callback].strict_equalities.iter().find_map(|binary| {
                matches!(strip(&binary.left), Expression::ComputedMemberExpression(_))
                    .then(|| index.symbol(&binary.right))
                    .flatten()
            })
        };
        let mut depth = BTreeSet::new();
        for callback in index.callbacks(at, "forEach") {
            if !index.method_calls(callback, "extend").is_empty() {
                self.claim(REGISTER_OPTION, callback, None, "forEach callback registering one option extension on the instance");
            } else if let Some(symbol) = compared(callback) {
                depth.insert(symbol);
                self.claim(RECORD_DEPTH_ID, callback, None, "forEach callback recording the id of the extension equal to the depth-bound one");
            }
        }
        for callback in index.callbacks(at, "some") {
            if let Some(symbol) = compared(callback) {
                depth.insert(symbol);
                self.claim(IS_DEPTH_ENTRY, callback, None, "some callback testing an entry against the depth-bound extension");
            }
        }
        for symbol in depth {
            self.depth_extension(symbol, extensions);
        }
    }
}

/// `Object.keys(slot 0).filter(a).map(b).sort(c).slice(...).map(d)`: the four
/// callbacks.
fn key_chain<'a>(index: &Index<'_, 'a>, value: &'a Expression<'a>, at: usize) -> Option<[usize; 4]> {
    let mut steps = Vec::new();
    let mut current = strip(value);
    loop {
        let Expression::CallExpression(call) = current else {
            return None;
        };
        if index.is_global_member(&call.callee, "Object", "keys") {
            let first = *call_arguments(call).first()?;
            if index.slot_of(first, at) != Some(0) {
                return None;
            }
            break;
        }
        let (object, name) = member_parts(&call.callee)?;
        steps.push((name, call));
        current = strip(object);
    }
    let [(last, take), (slice, _), (sort, order), (map, score), (filter, keep)] = steps.as_slice()
    else {
        return None;
    };
    if (*filter, *map, *sort, *slice, *last) != ("filter", "map", "sort", "slice", "map") {
        return None;
    }
    let callback = |call: &'a CallExpression<'a>| {
        call_arguments(call).first().and_then(|arg| index.fn_value(arg))
    };
    Some([callback(keep)?, callback(score)?, callback(order)?, callback(take)?])
}

/// The slot of `at` whose `[item]` element `expression` reads.
fn pair_slot<'a>(
    index: &Index<'_, 'a>,
    expression: &'a Expression<'a>,
    at: usize,
    item: u32,
) -> Option<u32> {
    let Expression::ComputedMemberExpression(member) = strip(expression) else {
        return None;
    };
    (numeric_index(&member.expression) == Some(item))
        .then(|| index.slot_of(&member.object, at))
        .flatten()
}

/// `other[index] === element`, with `other` the outer function's slot 1.
fn element_matches<'a>(index: &Index<'_, 'a>, callback: usize, outer: usize) -> bool {
    let Some(Expression::BinaryExpression(binary)) = index.single_return(callback) else {
        return false;
    };
    let Expression::ComputedMemberExpression(member) = strip(&binary.left) else {
        return false;
    };
    binary.operator == BinaryOperator::StrictEquality
        && index.slot_of(&member.object, outer) == Some(1)
        && index.slot_of(&member.expression, callback) == Some(1)
        && index.slot_of(&binary.right, callback) == Some(0)
}

fn is_registry_constructor<'a>(index: &Index<'_, 'a>, at: usize) -> bool {
    let own = &index.own[at];
    own.this_instanceof
        && own.assignments.iter().any(|assignment| {
            member_target(&assignment.left)
                .is_some_and(|(object, name)| is_this(object) && name == Some("extensions"))
                && matches!(strip(&assignment.right), Expression::CallExpression(call)
                    if index.is_global_member(&call.callee, "Object", "create"))
        })
}

/// The record object stored by an extend writer: the inline object literal or
/// a local holding that literal with exactly one write, declared and written
/// directly in the extend function's own scope. The flag is true only for the
/// hoisted local form.
fn extension_record<'a>(
    index: &Index<'_, 'a>,
    value: &'a Expression<'a>,
    at: usize,
) -> Option<(&'a ObjectExpression<'a>, bool)> {
    let (object, hoisted) = match strip(value) {
        Expression::ObjectExpression(object) => (object, false),
        value => {
            let symbol = index.symbol(value)?;
            if index.owner_of_symbol(symbol) != Some(at) {
                return None;
            }
            match index.value(symbol)? {
                Expression::ObjectExpression(object) => (object, true),
                _ => return None,
            }
        }
    };
    (index.owner_of_node(object.node_id.get()) == Some(at)).then_some((object, hoisted))
}

/// The record written by an extend function, with whether it was hoisted into
/// a local. The first accepted assignment is returned, keeping the previous
/// any-of-the-writes acceptance.
fn extend_record<'a>(
    index: &Index<'_, 'a>,
    at: usize,
) -> Option<(&'a ObjectExpression<'a>, bool)> {
    index.own[at].assignments.iter().copied().find_map(|assignment| {
        let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
            return None;
        };
        let on_extensions = member_parts(&target.object)
            .is_some_and(|(object, name)| name == "extensions" && is_this(object));
        if !on_extensions || index.slot_of(&target.expression, at) != Some(0) {
            return None;
        }
        let (object, hoisted) = extension_record(index, &assignment.right, at)?;
        let members = object_members_of(object);
        let holds = |name: &str, slot: u32| {
            members
                .iter()
                .any(|member| member.name == name && index.slot_of(member.value, at) == Some(slot))
        };
        (holds("detector", 1) && holds("serialiser", 2) && holds("deserialiser", 3))
            .then_some((object, hoisted))
    })
}

fn is_member_installer<'a>(index: &Index<'_, 'a>, at: usize) -> bool {
    let own = &index.own[at];
    let defines = |on_prototype: bool| {
        own.calls.iter().any(|call| {
            if !index.is_global_member(&call.callee, "Object", "defineProperty") {
                return false;
            }
            let Some(first) = call_arguments(call).first().copied() else {
                return false;
            };
            if on_prototype {
                member_parts(first).is_some_and(|(object, name)| {
                    name == "prototype" && index.slot_of(object, at) == Some(0)
                })
            } else {
                index.slot_of(first, at) == Some(0)
            }
        })
    };
    own.written_keys.contains("configurable")
        && own.written_keys.contains("writable")
        && own.written_keys.contains("enumerable")
        && matches!(own.returns.as_slice(), [statement]
            if statement.argument.as_ref().and_then(|value| index.slot_of(value, at)) == Some(0))
        && defines(true)
        && defines(false)
}

/// Single-value symbols holding the result of calling function `factory`.
fn holders_of_call<'a>(index: &Index<'_, 'a>, factory: usize) -> BTreeSet<SymbolId> {
    index
        .writes
        .keys()
        .copied()
        .filter(|symbol| {
            matches!(index.value(*symbol), Some(Expression::CallExpression(call))
                if index.fn_value(&call.callee) == Some(factory))
        })
        .collect()
}

/// `factory(registryClass).encode` is read somewhere.
fn encoder_factory_consumed<'a>(
    index: &Index<'_, 'a>,
    factory: usize,
    registries: &[Registry],
) -> bool {
    index.members.iter().any(|(object, name)| {
        let Expression::CallExpression(call) = strip(object) else {
            return false;
        };
        *name == "encode"
            && index.fn_value(&call.callee) == Some(factory)
            && call_arguments(call).first().and_then(|arg| index.symbol(arg)).is_some_and(
                |argument| {
                    registries.iter().any(|registry| {
                        !registry.hoisted && registry.holders.contains(&argument)
                    })
                },
            )
    })
}

fn is_instance_encode<'a>(index: &Index<'_, 'a>, at: usize, tags: &BTreeSet<SymbolId>) -> bool {
    index.calls_on_this(at, "encodeValue")
        && index.calls_on_this(at, "pushArrayElements")
        && index.reads_this_member(at, "stringHist")
        && index.reads_tags(at, tags, &["STRLUT"])
}

fn is_encode_value<'a>(index: &Index<'_, 'a>, at: usize, tags: &BTreeSet<SymbolId>) -> bool {
    index.calls_on_this(at, "encodeValue")
        && index.calls_on_this(at, "findKeysetIndex")
        && index.reads_this_member(at, "extensions")
        && index.reads_tags(at, tags, &["FALSE", "TRUE", "NULL", "UNDEFINED", "EXTENSION"])
}

fn is_find_keyset<'a>(index: &Index<'_, 'a>, at: usize, equal: &BTreeSet<usize>) -> bool {
    let appends = index.method_calls(at, "push").iter().any(|call| {
        member_parts(call.receiver)
            .is_some_and(|(object, name)| name == "keysets" && is_this(object))
    });
    appends && index.own[at].nested.iter().any(|nested| returns_call_to(index, *nested, equal))
}

/// The element callback of a `pushArrayElements` method.
fn is_push_elements<'a>(index: &Index<'_, 'a>, at: usize) -> Option<usize> {
    index.method_calls(at, "forEach").iter().find_map(|call| {
        if index.slot_of(call.receiver, at) != Some(0) {
            return None;
        }
        let callback = index.fn_value(call.args.first()?)?;
        index
            .method_calls(callback, "encodeValue")
            .iter()
            .any(|inner| {
                matches!(inner.args.as_slice(), [element, output]
                    if index.slot_of(element, callback) == Some(0)
                        && index.slot_of(output, at) == Some(1))
            })
            .then_some(callback)
    })
}

/// Builds a local instance, extends it from a callback and returns its encode.
fn is_static_encode<'a>(index: &Index<'_, 'a>, at: usize) -> bool {
    let own = &index.own[at];
    let instance = index.method_calls(at, "encode").iter().find_map(|call| {
        let symbol = index.symbol(call.receiver)?;
        matches!(index.value(symbol), Some(Expression::NewExpression(_))).then_some(symbol)
    });
    let Some(instance) = instance else {
        return false;
    };
    let extended = own.nested.iter().any(|nested| {
        index
            .method_calls(*nested, "extend")
            .iter()
            .any(|call| index.symbol(call.receiver) == Some(instance))
    });
    extended && !own.news.is_empty() && own.members.iter().any(|(_, name)| *name == "depthBound")
}

fn returns_call_to<'a>(index: &Index<'_, 'a>, at: usize, targets: &BTreeSet<usize>) -> bool {
    matches!(index.single_return(at), Some(Expression::CallExpression(call))
        if index.fn_value(&call.callee).is_some_and(|callee| targets.contains(&callee)))
}
