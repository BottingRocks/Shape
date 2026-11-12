//! Package `p09_interception_dom`: the host-capture engine and the DOM collectors.
//!
//! Recognition starts from two anchors proven by their own writes, the method
//! and accessor interceptor installers. Those installers, their wrappers and
//! the DOM field collector are named by `semantic_names` and `roles::purposes`,
//! so this package only reads them. Everything else is reached from
//! structure: which captured host member a binding holds and which proven
//! function reads it, and which host calls a function makes.
//!
//! A host member path such as `Object.getPrototypeOf` is a stable API name. Two
//! bindings that hold the same path are told apart by their reader, and two
//! bindings that remain equal both claim the role so the plan refuses them.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::Span;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

use crate::signals::extract::bindings::{
    object_members_of, property_key_name, static_member_name, ParameterSlots,
};
use crate::signals::schema::Confidence;
use super::{FinderClaim, FinderCtx, FinderRole, FinderRoleKind};
use crate::analysis;

const fn function(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::Function }
}

const fn binding(role: &'static str, name: &'static str) -> FinderRole {
    FinderRole { role, name, kind: FinderRoleKind::MainBinding }
}

/// Host members the installers capture: path, role.
const HOOK_HOST_MEMBERS: &[(&str, &str)] = &[
    ("Object.getPrototypeOf", "hook_get_prototype_of"),
    ("Object.getOwnPropertyDescriptor", "hook_get_own_property_descriptor"),
    ("Object.getOwnPropertyNames", "hook_get_own_property_names"),
    ("Object.defineProperty", "hook_define_property"),
    ("Object.hasOwnProperty", "hook_has_own_property"),
    ("Object.create", "hook_object_create"),
    ("Array.prototype.push", "hook_array_push"),
    ("Array.prototype.indexOf", "hook_array_index_of"),
];

/// Host members the probe registrar captures: path, role.
const PROBE_HOST_MEMBERS: &[(&str, &str)] = &[
    ("Object.getOwnPropertyDescriptor", "probe_get_own_property_descriptor"),
    ("Object.getPrototypeOf", "probe_get_prototype_of"),
    ("Object.prototype.toString", "probe_object_to_string"),
];

/// Every role this package can claim, with the one name it receives.
pub(crate) const VOCABULARY: &[FinderRole] = &[
    function("function_to_string_facade", "readRecordedFunctionSource"),
    function("reflect_construct_fallback", "reflectConstructFallback"),
    function("adopt_instrumentation_records", "adoptInstrumentationRecords"),
    function("share_instrumentation_records", "shareInstrumentationRecords"),
    function("handle_platform_instrumentation_exchange", "handlePlatformInstrumentationExchange"),
    function("install_interceptor_spec", "installInterceptorSpec"),
    function("lazy_hook_registry_class_factory", "createRegistryClassWithLazyHookInstall"),
    function("lazy_hook_registry_constructor", "forwardLazyHookRegistryArguments"),
    function("lazy_hook_registry_register", "registerAfterInstallingHooks"),
    function("dom_structure_census_registrar", "registerDomStructureCensus"),
    function("dom_structure_census", "collectDomStructureCensus"),
    function("dom_structure_element_counter", "countDomStructureElement"),
    function("dom_structure_squared_sum", "addSquaredChildCount"),
    function("browser_probe_registrar", "registerBrowserFingerprintProbes"),
    function("inspect_callable_metadata", "inspectCallableMetadata"),
    function("frame_probe_routine", "runFrameProbeRoutine"),
    function("frame_stack_access_getter", "emitStackAccessProbe"),
    function("body_attribute_mutation_handler", "emitBodyAttributeMutationProbe"),
    function("last_chance_probe_routine", "emitLastChanceProbe"),
    function("descriptor_census_routine", "runDescriptorCensus"),
    function("console_stack_access_getter", "errorStackGetterProbe"),
    function("probe_cleanup_routine", "disconnectBodyAttributeObserver"),
    binding("hook_get_prototype_of", "getPrototype"),
    binding("hook_get_own_property_descriptor", "getOwnPropertyDescriptor"),
    binding("hook_get_own_property_names", "getOwnPropertyNames"),
    binding("hook_define_property", "defineProperty"),
    binding("hook_has_own_property", "objectHasOwnProperty"),
    binding("hook_object_create", "objectCreate"),
    binding("hook_array_push", "appendRecord"),
    binding("hook_array_index_of", "findRestrictedPropertyIndex"),
    binding("hook_uncurried_apply", "applyMethod"),
    binding("hook_uncurried_call", "functionCall"),
    binding("restricted_function_properties", "restrictedFunctionProperties"),
    binding("reflect_construct", "reflectConstruct"),
    binding("native_function_bind", "nativeFunctionBind"),
    binding("instrumentation_records", "instrumentationRecords"),
    binding("to_string_targets", "toStringTargets"),
    binding("original_function_to_strings", "originalFunctionToStrings"),
    binding("function_to_string_shim", "functionToStringShim"),
    binding("native_function_to_string", "nativeFunctionToString"),
    binding("hook_form_interceptor_installer", "installInterceptors"),
    binding("probe_get_own_property_descriptor", "propertyDescriptorReader"),
    binding("probe_get_prototype_of", "prototypeReader"),
    binding("probe_object_to_string", "collectionTagReader"),
    binding("probe_has_own_property", "hasOwnPropertyBound"),
    binding("probe_unavailable_elapsed_time", "unavailableElapsedTime"),
    binding("native_function_source_pattern", "nativeFunctionSourcePattern"),
    binding("page_run_script_error_pattern", "pageRunScriptErrorPattern"),
];

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

/// What a function owns directly, excluding nested functions.
#[derive(Default)]
struct Own<'a> {
    calls: Vec<&'a CallExpression<'a>>,
    news: Vec<&'a NewExpression<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    deletes: Vec<&'a Expression<'a>>,
    reads: BTreeSet<SymbolId>,
    members: BTreeSet<String>,
    written_keys: BTreeSet<String>,
    nested: Vec<usize>,
}

struct Ix<'c, 'a> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    slots: &'c ParameterSlots<'a, 'a>,
    fns: Vec<FnEntry<'a>>,
    own: Vec<Own<'a>>,
    owner: Vec<Option<usize>>,
    fn_of_node: BTreeMap<NodeId, usize>,
    decl_fn: BTreeMap<SymbolId, usize>,
    assigns: BTreeMap<SymbolId, Vec<&'a AssignmentExpression<'a>>>,
    properties: Vec<(String, &'a Expression<'a>)>,
    classes: Vec<(&'a Class<'a>, Option<usize>)>,
}

fn strip<'e, 'a>(expression: &'e Expression<'a>) -> &'e Expression<'a> {
    expression.without_parentheses()
}

fn member<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
    let name = static_member_name(strip(expression))?;
    match strip(expression) {
        Expression::StaticMemberExpression(found) => Some((&found.object, name)),
        Expression::ComputedMemberExpression(found) => Some((&found.object, name)),
        _ => None,
    }
}

fn target_member_name<'e>(target: &'e AssignmentTarget<'_>) -> Option<&'e str> {
    match target {
        AssignmentTarget::StaticMemberExpression(found) => Some(found.property.name.as_str()),
        AssignmentTarget::ComputedMemberExpression(found) => match &found.expression {
            Expression::StringLiteral(literal) => Some(literal.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn target_member_object<'e, 'a>(target: &'e AssignmentTarget<'a>) -> Option<&'e Expression<'a>> {
    match target {
        AssignmentTarget::StaticMemberExpression(found) => Some(&found.object),
        AssignmentTarget::ComputedMemberExpression(found) => Some(&found.object),
        _ => None,
    }
}

fn args<'e, 'a>(call: &'e CallExpression<'a>) -> Vec<&'e Expression<'a>> {
    call.arguments.iter().filter_map(Argument::as_expression).collect()
}

fn string_value<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match strip(expression) {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

impl<'c, 'a> Ix<'c, 'a> {
    fn new(ctx: &FinderCtx<'_, 'a>, slots: &'c ParameterSlots<'a, 'a>) -> Self {
        let mut ix = Self {
            scoping: ctx.scoping,
            nodes: ctx.nodes,
            slots,
            fns: Vec::new(),
            own: Vec::new(),
            owner: vec![None; ctx.nodes.len()],
            fn_of_node: BTreeMap::new(),
            decl_fn: BTreeMap::new(),
            assigns: BTreeMap::new(),
            properties: Vec::new(),
            classes: Vec::new(),
        };
        ix.index();
        ix
    }

    fn index(&mut self) {
        let nodes = self.nodes;
        for (id, node) in nodes.iter_enumerated() {
            let parent = nodes.parent_id(id);
            let owner = if parent == id {
                None
            } else {
                self.fn_of_node.get(&parent).copied().or(self.owner[parent.index()])
            };
            self.owner[id.index()] = owner;
            let func = match node.kind() {
                AstKind::Function(found) => Some((found.span, FnNode::Function(found))),
                AstKind::ArrowFunctionExpression(found) => Some((found.span, FnNode::Arrow(found))),
                _ => None,
            };
            if let Some((span, func)) = func {
                let index = self.fns.len();
                self.fns.push(FnEntry { node: id, span, func, parent: owner });
                self.own.push(Own::default());
                self.fn_of_node.insert(id, index);
                if let Some(owner) = owner {
                    self.own[owner].nested.push(index);
                }
                if let FnNode::Function(found) = func {
                    if let Some(symbol) = found.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        self.decl_fn.insert(symbol, index);
                    }
                }
                if let FnNode::Arrow(found) = func {
                    if found.expression {
                        if let Some(Statement::ExpressionStatement(statement)) =
                            found.body.statements.first()
                        {
                            self.own[index].returns.push(&statement.expression);
                        }
                    }
                }
                continue;
            }
            self.index_node(node.kind(), owner);
        }
    }

    fn index_node(&mut self, kind: AstKind<'a>, owner: Option<usize>) {
        match kind {
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign =>
            {
                if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        self.assigns.entry(symbol).or_default().push(assignment);
                    }
                }
                if let Some(owner) = owner {
                    if let Some(name) = target_member_name(&assignment.left) {
                        self.own[owner].written_keys.insert(name.to_string());
                    }
                    self.own[owner].assignments.push(assignment);
                }
            }
            AstKind::ObjectProperty(property) if property.kind == PropertyKind::Init => {
                if let Some(name) = property_key_name(&property.key) {
                    if let Some(owner) = owner {
                        self.own[owner].written_keys.insert(name.clone());
                    }
                    self.properties.push((name, &property.value));
                }
            }
            AstKind::Class(class) => self.classes.push((class, owner)),
            _ => {}
        }
        let Some(owner) = owner else {
            return;
        };
        let scoping = self.scoping;
        let own = &mut self.own[owner];
        match kind {
            AstKind::CallExpression(call) => own.calls.push(call),
            AstKind::NewExpression(new) => own.news.push(new),
            AstKind::ReturnStatement(statement) => {
                if let Some(argument) = statement.argument.as_ref() {
                    own.returns.push(argument);
                }
            }
            AstKind::UnaryExpression(unary) if unary.operator == UnaryOperator::Delete => {
                own.deletes.push(&unary.argument);
            }
            AstKind::IdentifierReference(identifier) => {
                if let (Some(symbol), Some(reference)) = (
                    analysis::identifier_symbol(identifier, scoping),
                    identifier.reference_id.get(),
                ) {
                    if scoping.get_reference(reference).is_read() {
                        own.reads.insert(symbol);
                    }
                }
            }
            AstKind::StaticMemberExpression(found) => {
                own.members.insert(found.property.name.to_string());
            }
            AstKind::ComputedMemberExpression(found) => {
                if let Expression::StringLiteral(literal) = &found.expression {
                    own.members.insert(literal.value.to_string());
                }
            }
            _ => {}
        }
    }

    fn within(&self, inner: usize, outer: usize) -> bool {
        let mut current = Some(inner);
        while let Some(index) = current {
            if index == outer {
                return true;
            }
            current = self.fns[index].parent;
        }
        false
    }

    fn symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        analysis::expression_symbol(strip(expression), self.scoping)
    }

    /// `Object.prototype.toString`-style path rooted at an unbound global.
    fn path(&self, expression: &Expression<'a>) -> Option<String> {
        match strip(expression) {
            Expression::Identifier(identifier) => (self.symbol(expression).is_none())
                .then(|| identifier.name.to_string()),
            _ => {
                let (object, name) = member(expression)?;
                Some(format!("{}.{name}", self.path(object)?))
            }
        }
    }

    fn slot_index(&self, expression: &Expression<'a>, function: usize) -> Option<u32> {
        let slot = self.slots.incoming_slot(strip(expression))?;
        (slot.owner == self.fns[function].node).then_some(slot.index as u32)
    }

    fn is_incoming(&self, expression: &Expression<'a>) -> bool {
        self.slots.incoming_slot(strip(expression)).is_some()
    }

    /// Every source value of a binding, or `None` when it has a write that is
    /// not a declarator initializer or plain assignment.
    fn origins(&self, symbol: SymbolId) -> Option<Vec<&'a Expression<'a>>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let assignments = self.assigns.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
        let writes = self
            .scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count();
        if writes != assignments.len() {
            return None;
        }
        let mut out = Vec::new();
        if let AstKind::VariableDeclarator(declarator) =
            self.nodes.kind(self.scoping.symbol_declaration(symbol))
        {
            if let Some(init) = declarator.init.as_ref() {
                out.push(init);
            }
        }
        out.extend(assignments.iter().map(|assignment| &assignment.right));
        Some(out)
    }

    fn single_origin(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self.origins(symbol)?.as_slice() {
            [origin] => Some(origin),
            _ => None,
        }
    }

    fn resolve_fn(&self, expression: &Expression<'a>) -> Option<usize> {
        self.resolve_fn_depth(expression, 6)
    }

    fn resolve_fn_depth(&self, expression: &Expression<'a>, depth: u8) -> Option<usize> {
        match strip(expression) {
            Expression::FunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            Expression::ArrowFunctionExpression(found) => {
                return self.fn_of_node.get(&found.node_id.get()).copied();
            }
            _ => {}
        }
        if depth == 0 {
            return None;
        }
        let symbol = self.symbol(expression)?;
        if let Some(index) = self.decl_fn.get(&symbol).copied() {
            let immutable = self.scoping.symbol_declarations(symbol).count() == 1
                && !self.scoping.get_resolved_references(symbol).any(|r| r.is_write());
            return immutable.then_some(index);
        }
        self.resolve_fn_depth(self.single_origin(symbol)?, depth - 1)
    }

    fn reads_under(&self, index: usize, out: &mut BTreeSet<SymbolId>) {
        out.extend(self.own[index].reads.iter().copied());
        for nested in &self.own[index].nested {
            self.reads_under(*nested, out);
        }
    }

    /// Bindings declared outside `functions` and read inside their subtrees.
    fn outer_reads(&self, functions: &[usize]) -> BTreeSet<SymbolId> {
        let mut reads = BTreeSet::new();
        for function in functions {
            self.reads_under(*function, &mut reads);
        }
        reads.retain(|symbol| {
            let owner = self.owner[self.scoping.symbol_declaration(*symbol).index()];
            !functions.iter().any(|function| owner.is_some_and(|owner| self.within(owner, *function)))
                && !self.decl_fn.contains_key(symbol)
        });
        reads
    }

    fn is_create_null(&self, expression: &Expression<'a>) -> bool {
        let Expression::CallExpression(call) = strip(expression) else {
            return false;
        };
        let callee_is_create = self.path(&call.callee).as_deref() == Some("Object.create")
            || self.symbol(&call.callee).is_some_and(|symbol| {
                self.single_origin(symbol)
                    .is_some_and(|origin| self.path(origin).as_deref() == Some("Object.create"))
            });
        let arguments = args(call);
        callee_is_create
            && matches!(arguments.as_slice(), [only] if matches!(strip(only), Expression::NullLiteral(_)))
    }

    /// `x.call.bind(x.bind, x.call)`, directly or through one binding.
    fn is_uncurrier(&self, expression: &Expression<'a>) -> bool {
        let value = self.symbol(expression).and_then(|symbol| self.single_origin(symbol));
        let Expression::CallExpression(call) = strip(value.unwrap_or(expression)) else {
            return false;
        };
        let callee = self.path(&call.callee);
        let arguments = args(call);
        callee.is_some_and(|path| path.ends_with(".call.bind"))
            && matches!(arguments.as_slice(), [first, second]
                if self.path(first).is_some_and(|path| path.ends_with(".bind"))
                    && self.path(second).is_some_and(|path| path.ends_with(".call")))
    }

    /// The callee and arguments of a call, seen through the target cohort's
    /// `callee.call(receiver, ...)` form. A plain binding's `.call` stays as
    /// written: only a member or slot callee is forwarded.
    fn invocation<'e>(&self, call: &'e CallExpression<'a>) -> (&'e Expression<'a>, Vec<&'e Expression<'a>>) {
        let arguments = args(call);
        if let Some((inner, "call")) = member(&call.callee) {
            if !arguments.is_empty() && (member(inner).is_some() || self.is_incoming(inner)) {
                return (inner, arguments[1..].to_vec());
            }
        }
        (&call.callee, arguments)
    }

    /// The method name of a `receiver.name(...)` call and its receiver.
    fn method<'e>(&self, call: &'e CallExpression<'a>) -> Option<(&'e Expression<'a>, &'e str)> {
        member(self.invocation(call).0)
    }

    /// True when the call's callee is an incoming slot of `function`.
    fn calls_slot(&self, call: &CallExpression<'a>, function: usize) -> bool {
        self.slot_index(self.invocation(call).0, function).is_some()
    }

    fn is_regexp_call(&self, expression: &Expression<'a>) -> bool {
        match strip(expression) {
            Expression::CallExpression(call) => {
                self.path(&call.callee).as_deref() == Some("RegExp")
                    || member(&call.callee).is_some_and(|(_, name)| name == "RegExp")
            }
            _ => false,
        }
    }

    fn nested_arg_fns(&self, call: &CallExpression<'a>) -> Vec<usize> {
        self.invocation(call).1.into_iter().filter_map(|argument| self.resolve_fn(argument)).collect()
    }

    fn span(&self, index: usize) -> Span {
        self.fns[index].span
    }
}

#[derive(Default)]
struct Out {
    claims: Vec<FinderClaim>,
    seen: BTreeSet<(&'static str, u32, u32)>,
}

impl Out {
    fn function(&mut self, ix: &Ix<'_, '_>, role: &'static str, index: usize, evidence: &str) {
        let span = ix.span(index);
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::Function {
                role,
                function_span: span,
                alias_role: None,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }

    fn binding(&mut self, ix: &Ix<'_, '_>, role: &'static str, symbol: SymbolId, evidence: &str) {
        let span = ix.scoping.symbol_span(symbol);
        if self.seen.insert((role, span.start, span.end)) {
            self.claims.push(FinderClaim::MainBinding {
                role,
                identifier_span: span,
                confidence: Confidence::Medium,
                evidence: vec![evidence.to_string()],
            });
        }
    }
}

/// Structural claims of this package.
pub(crate) fn find(ctx: &FinderCtx) -> Vec<FinderClaim> {
    let slots = ParameterSlots::new(ctx.program, ctx.scoping, ctx.nodes);
    let ix = Ix::new(ctx, &slots);
    let mut out = Out::default();
    hook_engine(&ix, &mut out);
    lazy_registry(&ix, &mut out);
    dom_census(&ix, &mut out);
    probes(&ix, &mut out);
    out.claims
}

fn hook_engine(ix: &Ix<'_, '_>, out: &mut Out) {
    let has = |index: usize, keys: [&str; 2]| keys.iter().all(|key| ix.own[index].written_keys.contains(*key));
    let methods: Vec<usize> =
        (0..ix.fns.len()).filter(|index| has(*index, ["onBeforeInvoke", "onAfterInvoke"])).collect();
    let accessors: Vec<usize> =
        (0..ix.fns.len()).filter(|index| has(*index, ["onBeforeGet", "onBeforeSet"])).collect();
    if methods.is_empty() || accessors.is_empty() {
        return;
    }
    let installers: Vec<usize> = methods.iter().chain(&accessors).copied().collect();
    let reads = ix.outer_reads(&installers);
    let method_reads = ix.outer_reads(&methods);
    let accessor_reads = ix.outer_reads(&accessors);

    for symbol in &reads {
        let Some(origins) = ix.origins(*symbol) else {
            continue;
        };
        if let [origin] = origins.as_slice() {
            if let Some(path) = ix.path(origin) {
                if let Some((_, role)) = HOOK_HOST_MEMBERS.iter().find(|(host, _)| *host == path) {
                    out.binding(ix, role, *symbol, "host member captured and read by an interceptor installer");
                }
            }
            if let Expression::CallExpression(call) = strip(origin) {
                let arguments = args(call);
                if let ([only], true) = (arguments.as_slice(), ix.is_uncurrier(&call.callee)) {
                    match member(only).map(|(_, name)| name) {
                        Some("apply") => out.binding(ix, "hook_uncurried_apply", *symbol, "uncurried Function apply read by an interceptor installer"),
                        Some("call") => out.binding(ix, "hook_uncurried_call", *symbol, "uncurried Function call read by an interceptor installer"),
                        _ => {}
                    }
                }
            }
            if let Expression::ArrayExpression(array) = strip(origin) {
                let names: Vec<_> = array.elements.iter().filter_map(|element| element.as_expression().and_then(string_value)).collect();
                if names == ["arguments", "caller"] {
                    out.binding(ix, "restricted_function_properties", *symbol, "the two restricted function property names, skipped by the installer's property copy");
                }
            }
        }
        reflect_construct(ix, out, *symbol, &origins);
        if method_reads.contains(symbol) && accessor_reads.contains(symbol) {
            records(ix, out, *symbol, &origins);
        }
    }
    facade(ix, out, &method_reads, &accessor_reads);
    platform_exchange(ix, out, &methods, &accessors);
}

fn reflect_construct<'a>(
    ix: &Ix<'_, 'a>,
    out: &mut Out,
    symbol: SymbolId,
    origins: &[&'a Expression<'a>],
) {
    if !origins.iter().any(|origin| ix.path(origin).as_deref() == Some("Reflect.construct")) {
        return;
    }
    out.binding(ix, "reflect_construct", symbol, "holds Reflect.construct and is read by an interceptor installer");
    let Some(fallback) = origins.iter().find_map(|origin| ix.resolve_fn(origin)) else {
        return;
    };
    if ix.own[fallback].news.is_empty() {
        return;
    }
    out.function(ix, "reflect_construct_fallback", fallback, "assigned where Reflect.construct is missing; constructs through a bound apply");
    for read in ix.outer_reads(&[fallback]) {
        let bind = ix.single_origin(read).and_then(|origin| ix.path(origin));
        if bind.as_deref() == Some("Function.prototype.bind") {
            out.binding(ix, "native_function_bind", read, "captured Function.prototype.bind applied by the construct fallback");
        }
    }
}

/// Functions stored as an `exchange` member of an object literal.
fn exchange_functions(ix: &Ix<'_, '_>) -> BTreeSet<usize> {
    ix.properties
        .iter()
        .filter(|(name, _)| name == "exchange")
        .filter_map(|(_, value)| ix.resolve_fn(value))
        .collect()
}

fn records<'a>(ix: &Ix<'_, 'a>, out: &mut Out, symbol: SymbolId, origins: &[&'a Expression<'a>]) {
    let created = origins.iter().any(|origin| ix.is_create_null(origin));
    let others = origins.iter().all(|origin| ix.is_create_null(origin) || ix.is_incoming(origin));
    if !(created && others) {
        return;
    }
    out.binding(ix, "instrumentation_records", symbol, "null-prototype map read by both interceptor installers");
    let exchanges = exchange_functions(ix);
    for assignment in ix.assigns.get(&symbol).map(Vec::as_slice).unwrap_or(&[]) {
        let Some(owner) = ix.owner[assignment.node_id.get().index()] else {
            continue;
        };
        if ix.is_incoming(&assignment.right) && exchanges.contains(&owner) {
            out.function(ix, "adopt_instrumentation_records", owner, "exchange callback that replaces the records map with its argument");
        }
    }
    let listeners: BTreeSet<usize> = ix
        .own
        .iter()
        .flat_map(|own| &own.calls)
        .filter(|call| {
            ix.path(&call.callee).is_some_and(|path| path.ends_with("addEventListener"))
                || ix.method(call).is_some_and(|(_, name)| name == "addEventListener")
        })
        .flat_map(|call| ix.nested_arg_fns(call))
        .collect();
    for listener in listeners {
        let shares = ix.own[listener].calls.iter().any(|call| {
            ix.method(call).is_some_and(|(_, name)| name == "exchange")
                && args(call).iter().any(|argument| ix.symbol(argument) == Some(symbol))
        });
        if shares {
            out.function(ix, "share_instrumentation_records", listener, "event listener that hands the records map to the event's exchange function");
        }
    }
}

fn facade(ix: &Ix<'_, '_>, out: &mut Out, method_reads: &BTreeSet<SymbolId>, accessor_reads: &BTreeSet<SymbolId>) {
    let hooked = |symbol: &SymbolId| method_reads.contains(symbol) || accessor_reads.contains(symbol);
    for index in 0..ix.fns.len() {
        if matches!(ix.fns[index].func, FnNode::Arrow(_)) {
            continue;
        }
        let own = &ix.own[index];
        let targets = own.calls.iter().find_map(|call| {
            let (receiver, name) = ix.method(call)?;
            let arguments = args(call);
            (name == "lastIndexOf" && matches!(arguments.as_slice(), [only] if matches!(strip(only), Expression::ThisExpression(_))))
                .then(|| ix.symbol(receiver))?
        });
        let Some(targets) = targets else {
            continue;
        };
        let sources = own.returns.iter().find_map(|value| match strip(value) {
            Expression::ComputedMemberExpression(found) => ix.symbol(&found.object),
            _ => None,
        });
        let native = own.returns.iter().find_map(|value| match strip(value) {
            Expression::CallExpression(call) if matches!(args(call).as_slice(), [only] if matches!(strip(only), Expression::ThisExpression(_))) => ix.symbol(&call.callee),
            _ => None,
        });
        let (Some(sources), Some(native)) = (sources, native) else {
            continue;
        };
        if !(hooked(&targets) && hooked(&sources)) {
            continue;
        }
        out.function(ix, "function_to_string_facade", index, "looks `this` up in the wrapped-function list and returns the recorded source, else the native text");
        out.binding(ix, "to_string_targets", targets, "searched by the toString facade and appended by the installers");
        out.binding(ix, "original_function_to_strings", sources, "indexed by the toString facade and appended by the installers");
        let text = ix.single_origin(native).is_some_and(|origin| {
            matches!(strip(origin), Expression::CallExpression(call)
                if args(call).last().is_some_and(|last| ix.path(last).as_deref() == Some("Function.prototype.toString")))
        });
        if text {
            out.binding(ix, "native_function_to_string", native, "bound Function.prototype.toString called on `this` by the facade");
        }
        let Some(parent) = ix.fns[index].parent else {
            continue;
        };
        let holder = ix.own[parent].assignments.iter().find_map(|assignment| {
            (target_member_name(&assignment.left) == Some("toString")
                && ix.resolve_fn(&assignment.right) == Some(index))
            .then(|| target_member_object(&assignment.left).and_then(|object| ix.symbol(object)))?
        });
        let Some(holder) = holder else {
            continue;
        };
        for read in method_reads.iter().chain(accessor_reads) {
            if ix.single_origin(*read).and_then(|origin| ix.symbol(origin)) == Some(holder) {
                out.binding(ix, "function_to_string_shim", *read, "alias of the object holding the facade, installed by the interceptors as Function.prototype.toString");
            }
        }
    }
}

fn platform_exchange(ix: &Ix<'_, '_>, out: &mut Out, methods: &[usize], accessors: &[usize]) {
    for function in exchange_functions(ix) {
        let own = &ix.own[function];
        let flags = own.assignments.iter().any(|assignment| {
            target_member_name(&assignment.left) == Some("platformIntegratedHooksCalled")
        });
        let adopts = own.assignments.iter().any(|assignment| {
            member(&assignment.right).is_some_and(|(object, name)| name == "instrumented" && ix.is_incoming(object))
        });
        if flags && adopts {
            out.function(ix, "handle_platform_instrumentation_exchange", function, "exchange callback that flags platform hooks and adopts the provider's instrumented map");
        }
    }
    let for_each_callbacks: BTreeSet<usize> = ix
        .own
        .iter()
        .flat_map(|own| &own.calls)
        .filter(|call| ix.method(call).is_some_and(|(_, name)| name == "forEach"))
        .flat_map(|call| ix.nested_arg_fns(call))
        .collect();
    for function in for_each_callbacks {
        let called: Vec<usize> =
            ix.own[function].calls.iter().filter_map(|call| ix.resolve_fn(&call.callee)).collect();
        if called.iter().any(|callee| methods.contains(callee))
            && called.iter().any(|callee| accessors.contains(callee))
        {
            out.function(ix, "install_interceptor_spec", function, "for each descriptor, calls the method or the accessor interceptor installer");
        }
    }
}

fn lazy_registry(ix: &Ix<'_, '_>, out: &mut Out) {
    for (class, owner) in &ix.classes {
        let Some(factory) = *owner else {
            continue;
        };
        let Some(base) = class.super_class.as_ref() else {
            continue;
        };
        let class_symbol = class.id.as_ref().and_then(|id| id.symbol_id.get());
        let returned = ix.own[factory]
            .returns
            .iter()
            .any(|value| class_symbol.is_some_and(|symbol| ix.symbol(value) == Some(symbol)));
        if !(returns_slot_zero(ix, base, factory) && returned) {
            continue;
        }
        let method = |key: &str| {
            class.body.body.iter().find_map(|element| match element {
                ClassElement::MethodDefinition(found)
                    if property_key_name(&found.key).as_deref() == Some(key) =>
                {
                    ix.fn_of_node.get(&found.value.node_id.get()).copied()
                }
                _ => None,
            })
        };
        let Some(register) = method("register") else {
            continue;
        };
        let forwards = ix.own[register].calls.iter().any(|call| {
            ix.method(call).is_some_and(|(object, name)| {
                name == "register" && matches!(strip(object), Expression::Super(_))
            })
        });
        let constructor = method("constructor").filter(|constructor| {
            let calls = &ix.own[*constructor].calls;
            calls.len() == 1 && matches!(strip(&calls[0].callee), Expression::Super(_))
        });
        if forwards {
            claim_lazy_registry(ix, out, factory, register, constructor);
        }
    }
    // The transpiled form: `ctor.prototype.register = fn`, with the base
    // method reached through the factory's first slot.
    for factory in 0..ix.fns.len() {
        for assignment in &ix.own[factory].assignments {
            if target_member_name(&assignment.left) != Some("register") {
                continue;
            }
            let Some((ctor, "prototype")) = target_member_object(&assignment.left).and_then(member) else {
                continue;
            };
            let Some(register) = ix.resolve_fn(&assignment.right) else {
                continue;
            };
            let returned = ix.symbol(ctor).is_some_and(|symbol| {
                ix.own[factory].returns.iter().any(|value| ix.symbol(value) == Some(symbol))
            });
            let forwards = ix.own[register].calls.iter().any(|call| {
                let (callee, _) = ix.invocation(call);
                member(callee).and_then(|(prototype, name)| (name == "register").then_some(prototype)).and_then(member).is_some_and(
                    |(base, name)| name == "prototype" && returns_slot_zero(ix, base, factory),
                )
            });
            if !(returned && forwards) {
                continue;
            }
            let constructor = ix.resolve_fn(ctor).filter(|constructor| {
                let calls = &ix.own[*constructor].calls;
                calls.len() == 1
                    && member(&calls[0].callee).is_some_and(|(base, name)| {
                        name == "apply" && returns_slot_zero(ix, base, factory)
                    })
            });
            claim_lazy_registry(ix, out, factory, register, constructor);
        }
    }
}

/// `expression` is slot 0 of `factory`, directly or through one local copy.
fn returns_slot_zero<'a>(ix: &Ix<'_, 'a>, expression: &Expression<'a>, factory: usize) -> bool {
    ix.slot_index(expression, factory) == Some(0)
        || ix
            .symbol(expression)
            .and_then(|symbol| ix.single_origin(symbol))
            .is_some_and(|origin| ix.slot_index(origin, factory) == Some(0))
}

fn claim_lazy_registry(
    ix: &Ix<'_, '_>,
    out: &mut Out,
    factory: usize,
    register: usize,
    constructor: Option<usize>,
) {
    let installer = ix.own[register].calls.iter().find_map(|call| {
        let target = ix.resolve_fn(&call.callee)?;
        let own = &ix.own[target];
        (own.members.contains("onBeforeInvoke") && own.members.contains("register"))
            .then(|| ix.symbol(&call.callee))?
    });
    let Some(installer) = installer else {
        return;
    };
    let evidence = "subclass of its argument whose register() runs the interceptor installer before the base register";
    out.function(ix, "lazy_hook_registry_class_factory", factory, evidence);
    out.function(ix, "lazy_hook_registry_register", register, evidence);
    if let Some(constructor) = constructor {
        out.function(ix, "lazy_hook_registry_constructor", constructor, "only forwards its arguments to the base constructor");
    }
    if !ix.decl_fn.contains_key(&installer) {
        out.binding(ix, "hook_form_interceptor_installer", installer, "called by the lazy registry's register() and resolves to the function that registers interceptor hooks");
    }
}

fn is_squared_sum(ix: &Ix<'_, '_>, function: usize) -> bool {
    ix.own[function].returns.iter().any(|value| {
        let Expression::BinaryExpression(sum) = strip(value) else {
            return false;
        };
        let Expression::BinaryExpression(square) = strip(&sum.right) else {
            return false;
        };
        sum.operator == BinaryOperator::Addition
            && square.operator == BinaryOperator::Multiplication
            && ix.slot_index(&sum.left, function) == Some(0)
            && ix.slot_index(&square.left, function) == Some(1)
            && ix.slot_index(&square.right, function) == Some(1)
    })
}

fn dom_census(ix: &Ix<'_, '_>, out: &mut Out) {
    for census in 0..ix.fns.len() {
        let own = &ix.own[census];
        let queries_all = own.calls.iter().any(|call| {
            ix.method(call).is_some_and(|(_, name)| name == "querySelectorAll")
                && matches!(ix.invocation(call).1.as_slice(), [only] if string_value(only) == Some("*"))
        });
        if !queries_all || !own.members.contains("next") {
            continue;
        }
        let counter = own.calls.iter().flat_map(|call| ix.nested_arg_fns(call)).find(|callback| {
            let members = &ix.own[*callback].members;
            members.contains("tagName") && members.contains("children")
        });
        let Some(counter) = counter else {
            continue;
        };
        let evidence = "queries every element, counts tags per element and emits the counts";
        out.function(ix, "dom_structure_census", census, evidence);
        out.function(ix, "dom_structure_element_counter", counter, "forEach callback that counts one element by tagName and records its child count");
        for call in &own.calls {
            if ix.method(call).is_some_and(|(_, name)| name == "reduce") {
                for reducer in ix.nested_arg_fns(call) {
                    if is_squared_sum(ix, reducer) {
                        out.function(ix, "dom_structure_squared_sum", reducer, "reduce callback adding the square of each child count");
                    }
                }
            }
        }
        let Some(parent) = ix.fns[census].parent else {
            continue;
        };
        let scheduled = ix.own[parent].calls.iter().any(|call| {
            ix.calls_slot(call, parent) && ix.nested_arg_fns(call).contains(&census)
        });
        if scheduled {
            out.function(ix, "dom_structure_census_registrar", parent, "hands the census routine to its scheduler argument");
        }
    }
}

fn probes(ix: &Ix<'_, '_>, out: &mut Out) {
    let inspectors = (0..ix.fns.len()).filter(|index| {
        let own = &ix.own[*index];
        own.deletes.iter().any(|target| {
            member(target).is_some_and(|(object, name)| name == "prototype" && ix.slot_index(object, *index) == Some(0))
        }) && own.calls.iter().any(|call| ix.method(call).is_some_and(|(_, name)| name == "test"))
    });
    let hook_installers: Vec<usize> = (0..ix.fns.len())
        .filter(|index| {
            let keys = &ix.own[*index].written_keys;
            keys.contains("onBeforeInvoke") || keys.contains("onBeforeGet")
        })
        .collect();
    let hook_reads = ix.outer_reads(&hook_installers);
    for inspector in inspectors.collect::<Vec<_>>() {
        let Some(registrar) = ix.fns[inspector].parent else {
            continue;
        };
        let routines: Vec<usize> = ix.own[registrar]
            .calls
            .iter()
            .filter(|call| ix.calls_slot(call, registrar))
            .flat_map(|call| ix.nested_arg_fns(call))
            .collect();
        if routines.len() < 3 {
            continue;
        }
        out.function(ix, "inspect_callable_metadata", inspector, "reports a callable's native-text match, prototype presence and prototype deletability");
        out.function(ix, "browser_probe_registrar", registrar, "owns the callable inspector and hands several routines to its scheduler arguments");
        let reads: BTreeSet<SymbolId> = ix.outer_reads(&[registrar]).difference(&hook_reads).copied().collect();
        let reader = reads.iter().copied().find(|symbol| {
            ix.single_origin(*symbol).and_then(|origin| ix.path(origin)).as_deref() == Some("Object.getOwnPropertyDescriptor")
        });
        for routine in &routines {
            classify_routine(ix, out, *routine, reader);
        }
        probe_bindings(ix, out, inspector, &routines, &reads);
    }
}

fn is_error_test(ix: &Ix<'_, '_>, call: &CallExpression<'_>) -> bool {
    ix.method(call).is_some_and(|(_, name)| name == "test")
        && ix.invocation(call).1.iter().any(|argument| member(argument).is_some_and(|(_, name)| name == "error"))
}

fn classify_routine(ix: &Ix<'_, '_>, out: &mut Out, routine: usize, reader: Option<SymbolId>) {
    let own = &ix.own[routine];
    let observer = own.news.iter().find(|new| {
        member(&new.callee).is_some_and(|(_, name)| name == "MutationObserver")
    });
    let last_chance = own.calls.iter().any(|call| is_error_test(ix, call));
    let reads_descriptors = own
        .calls
        .iter()
        .filter(|call| reader.is_some() && ix.symbol(&call.callee) == reader)
        .count();
    let builds_frame = own.calls.iter().any(|call| {
        ix.invocation(call).1.iter().any(|argument| string_value(argument) == Some("iframe"))
    });
    if builds_frame {
        out.function(ix, "frame_probe_routine", routine, "builds a temporary iframe and probes through it");
        stack_getters(ix, out, routine, "frame_stack_access_getter");
    }
    if let Some(observer) = observer {
        for callback in observer.arguments.iter().filter_map(Argument::as_expression).filter_map(|argument| ix.resolve_fn(argument)) {
            out.function(ix, "body_attribute_mutation_handler", callback, "MutationObserver callback of a probe routine");
        }
    }
    if builds_frame {
        return;
    }
    if last_chance {
        out.function(ix, "last_chance_probe_routine", routine, "tests the page's error text and queried scripts when the snapshot flushes");
    } else if reads_descriptors >= 2 {
        out.function(ix, "descriptor_census_routine", routine, "reads host property descriptors through the probe's captured reader");
        stack_getters(ix, out, routine, "console_stack_access_getter");
    } else if own.calls.len() == 1 && ix.method(own.calls[0]).is_some_and(|(_, name)| name == "disconnect") {
        out.function(ix, "probe_cleanup_routine", routine, "only disconnects the attribute observer");
    }
}

fn stack_getters(ix: &Ix<'_, '_>, out: &mut Out, routine: usize, role: &'static str) {
    for call in &ix.own[routine].calls {
        let arguments = args(call);
        let [_, key, descriptor] = arguments.as_slice() else {
            continue;
        };
        let defines = ix.path(&call.callee).as_deref() == Some("Object.defineProperty");
        let Expression::ObjectExpression(object) = strip(descriptor) else {
            continue;
        };
        if !defines || string_value(key) != Some("stack") {
            continue;
        }
        for entry in object_members_of(object) {
            if entry.name == "get" {
                if let Some(getter) = ix.resolve_fn(entry.value) {
                    out.function(ix, role, getter, "getter defined on an error's `stack` property");
                }
            }
        }
    }
}

fn is_number(expression: &Expression<'_>) -> bool {
    match strip(expression) {
        Expression::NumericLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::UnaryNegation && is_number(&unary.argument)
        }
        _ => false,
    }
}

/// Every read of `symbol` initializes a binding that also holds a time delta.
fn seeds_elapsed_times(ix: &Ix<'_, '_>, symbol: SymbolId) -> bool {
    let mut reads = 0;
    for reference in ix.scoping.get_resolved_references(symbol) {
        if !reference.is_read() {
            continue;
        }
        reads += 1;
        let parent = ix.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(assignment) = ix.nodes.kind(parent) else {
            return false;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            return false;
        };
        let Some(target) = analysis::identifier_symbol(identifier, ix.scoping) else {
            return false;
        };
        let delta = ix.assigns.get(&target).is_some_and(|writes| {
            writes.iter().any(|write| {
                matches!(strip(&write.right), Expression::BinaryExpression(difference)
                    if difference.operator == BinaryOperator::Subtraction
                        && matches!(strip(&difference.left), Expression::CallExpression(call)
                            if ix.path(&call.callee).as_deref() == Some("Number")))
            })
        });
        if !delta {
            return false;
        }
    }
    reads > 0
}

fn probe_bindings(ix: &Ix<'_, '_>, out: &mut Out, inspector: usize, routines: &[usize], reads: &BTreeSet<SymbolId>) {
    for symbol in reads {
        let Some(origin) = ix.single_origin(*symbol) else {
            continue;
        };
        if let Some(path) = ix.path(origin) {
            if let Some((_, role)) = PROBE_HOST_MEMBERS.iter().find(|(host, _)| *host == path) {
                out.binding(ix, role, *symbol, "host member captured and read by the probe registrar");
            }
        }
        if let Expression::CallExpression(call) = strip(origin) {
            let bound = args(call).last().is_some_and(|last| ix.path(last).as_deref() == Some("Object.prototype.hasOwnProperty"));
            if bound && ix.path(&call.callee).is_some_and(|path| path.contains(".call.bind")) {
                out.binding(ix, "probe_has_own_property", *symbol, "call-bound hasOwnProperty read by the probe registrar");
            }
        }
        if is_number(origin) && seeds_elapsed_times(ix, *symbol) {
            out.binding(ix, "probe_unavailable_elapsed_time", *symbol, "number used only to seed bindings that later hold a time delta");
        }
    }
    let tested = |function: usize, filter: &dyn Fn(&CallExpression<'_>) -> bool| -> Vec<SymbolId> {
        ix.own[function]
            .calls
            .iter()
            .filter(|call| ix.method(call).is_some_and(|(_, name)| name == "test") && filter(call))
            .filter_map(|call| ix.method(call).and_then(|(receiver, _)| ix.symbol(receiver)))
            .filter(|symbol| {
                ix.origins(*symbol).is_some_and(|origins| origins.iter().any(|origin| ix.is_regexp_call(origin)))
            })
            .collect()
    };
    for symbol in tested(inspector, &|_| true) {
        out.binding(ix, "native_function_source_pattern", symbol, "pattern the callable inspector tests against function text");
    }
    for routine in routines {
        for symbol in tested(*routine, &|call| is_error_test(ix, call)) {
            out.binding(ix, "page_run_script_error_pattern", symbol, "pattern the last-chance routine tests against the page error text");
        }
    }
}
