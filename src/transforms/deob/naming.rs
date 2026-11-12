//! Complete kernel naming for Shape-style VM kernels.
//!
//! Every kernel-owned binding receives a readable role name when its dataflow
//! proves one. Unknown bindings receive an explicitly unresolved semantic name
//! and diagnostic; operation functions remain anonymous when their behavior is
//! not proven. Property and layout keys are renamed only on statically typed
//! receivers, so an unrelated object that reuses the same key keeps it.
//!
//! Roles and layout keys are derived from AST shape and consumers, never from
//! the minified spelling of an identifier or from raw byte offsets.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_allocator::{Allocator, FromIn};
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{Atom, GetSpan, Ident, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::transforms::deob::pipeline::{BindingMapping, PropertyMapping, ProvenRoleBinding};
use crate::{
    diagnostics::{Diagnostic, Stage},
    roles::VmRole,
};

/// A provable instance kind. Member renames are keyed by this.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum ObjType {
    Memory,
    Context,
    Stack,
    Descriptor,
    /// A memory slot record (`{ value: ... }`).
    Slot,
}

impl ObjType {
    fn label(self) -> &'static str {
        match self {
            ObjType::Memory => "vmMemory",
            ObjType::Context => "vmContext",
            ObjType::Stack => "vmStack",
            ObjType::Descriptor => "threadDescriptor",
            ObjType::Slot => "vmMemorySlot",
        }
    }
}

/// Shape of a literal initializer, used to recognise tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Strings(usize),
    Numbers(usize),
    Functions(usize),
    Records(usize),
    Cache,
    Other,
}

struct DeclInfo<'a> {
    init: Option<&'a Expression<'a>>,
}

struct FnInfo {
    symbol: Option<SymbolId>,
    params: Vec<SymbolId>,
}

struct Collected<'a> {
    decls: BTreeMap<SymbolId, DeclInfo<'a>>,
    shapes: BTreeMap<SymbolId, Shape>,
    functions: BTreeMap<NodeId, FnInfo>,
    /// Function-expression nodes of each `Functions` array, in array order.
    array_functions: BTreeMap<SymbolId, Vec<NodeId>>,
    /// Symbol of a `var X = function () {}`-style assignment, keyed by function node.
    assigned_symbol: BTreeMap<NodeId, SymbolId>,
    catch_params: BTreeSet<SymbolId>,
    class_symbols: BTreeSet<SymbolId>,
}

#[derive(Clone)]
enum FieldRhs {
    StackCall,
    Conditional,
    Param(SymbolId),
    Other,
}

#[derive(Default)]
struct Facts {
    entry: Option<SymbolId>,
    decoder: BTreeSet<SymbolId>,
    operations: Option<SymbolId>,
    proven_operations: Option<SymbolId>,
    native_functions: Option<SymbolId>,
    descriptors: Option<SymbolId>,
    memory_ctor: Option<SymbolId>,
    context_ctor: Option<SymbolId>,
    stack_factory: Option<SymbolId>,
    /// Local array returned by the stack factory; typed as [`ObjType::Stack`].
    stack_local: Option<SymbolId>,
    linear_memory: Option<SymbolId>,
    create_thread: Option<SymbolId>,
    capture_thread: Option<SymbolId>,
    build_invocation: Option<SymbolId>,
    invoke_thread: Option<SymbolId>,
    run: Option<SymbolId>,
    guarded: Option<SymbolId>,
    wasm_inputs: BTreeSet<SymbolId>,
    wasm_input_order: Vec<SymbolId>,
    wasm_modules: Vec<SymbolId>,
    wasm_instances: Vec<SymbolId>,
    host_imports: Option<SymbolId>,
    /// Assigned-function wrapper that returns the current stack top.
    pop_stack: Option<SymbolId>,
    /// Parameters of functions exposed through the proven host-import object,
    /// grouped by callback so operand numbering resets for every callback.
    callback_param_groups: Vec<Vec<SymbolId>>,
    /// Reused descriptor object for the memory `slots` property initializer.
    slot_descriptor: Option<SymbolId>,
    /// Cached worker instance built by the bootstrap.
    worker: Option<SymbolId>,
    /// Callback array invoked by the worker `onmessage` handler.
    worker_callbacks: Option<SymbolId>,
    /// Worker bootstrap function that builds the worker from the entry source.
    initialize_worker: Option<SymbolId>,
    /// String source concatenated from the VM entry and passed to the Blob.
    worker_source: Option<SymbolId>,
    /// Blob built from the worker source and handed to the worker constructor.
    worker_blob: Option<SymbolId>,
    /// `onmessage` handler parameter carrying the callback payload.
    worker_message_event: Option<SymbolId>,
    /// Catch binding of the worker bootstrap.
    worker_error: Option<SymbolId>,
    /// Local discriminants returned by the interpreter callback.
    run_status_locals: BTreeSet<SymbolId>,
    /// Catch binding of the proven operation wrapper.
    guarded_error: Option<SymbolId>,
    string_table: Option<SymbolId>,
    number_table: Option<SymbolId>,
    cache: Option<SymbolId>,
    memory_aliases: BTreeSet<SymbolId>,

    slots_key: Option<String>,
    slot_value_key: Option<String>,
    memory_members: BTreeMap<String, String>,
    stack_members: BTreeMap<String, String>,
    descriptor_members: BTreeMap<String, String>,
    context_keys: BTreeSet<String>,
    context_field_rhs: Vec<(String, FieldRhs)>,
    context_params: Vec<SymbolId>,
    context_stack_fields: Vec<String>,
    context_pc_field: Option<String>,
    context_state_field: Option<String>,
    /// Semantic name for every proven context member key.
    context_member_names: BTreeMap<String, String>,
    /// Frame record keys (raw key -> semantic name) grouped by the context
    /// stack field that pushes/pops that frame.
    context_frame_keys: BTreeMap<String, BTreeMap<String, String>>,
    /// Object-literal properties of proven frame records, keyed by property node.
    frame_object_keys: BTreeMap<NodeId, String>,
    /// Member reads on locals that receive a proven frame record, keyed by
    /// member node.
    frame_member_names: BTreeMap<NodeId, String>,
    /// Locals that receive a proven frame record, keyed to the source field.
    frame_locals: BTreeMap<SymbolId, String>,
    /// Loop indices that bound an unwind over recorded error entries.
    unwind_index_locals: BTreeSet<SymbolId>,
    /// Prototype aliases (`var p = Ctor.prototype`).
    memory_proto_alias: Option<SymbolId>,
    context_proto_alias: Option<SymbolId>,
    /// VM dispatch method installed on the context prototype.
    context_dispatch_key: Option<String>,
    /// Table indexed by (dispatch state, bytecode opcode).
    dispatch_table: Option<SymbolId>,
    /// Bytecode image consumed by the dispatch method and handlers.
    bytecode: Option<SymbolId>,
    /// Exception handler invoked from the guarded-operation catch block.
    handle_exception: Option<SymbolId>,
    /// Parameters of the JavaScript VM operation-first guarded wrapper, in role order.
    guarded_operation_param: Option<SymbolId>,
    guarded_context_param: Option<SymbolId>,
    /// Parameters of the exception handler, in role order.
    exception_error_param: Option<SymbolId>,
    exception_context_param: Option<SymbolId>,
    /// Completion sentinels and the pending-result register of `vmRun`.
    no_result_sentinel: Option<SymbolId>,
    empty_result_sentinel: Option<SymbolId>,
    completion_result: Option<SymbolId>,
    descriptor_local: Option<SymbolId>,
    context_local: Option<SymbolId>,
    /// First parameters proven to receive the VM execution context: operation
    /// handlers, WASM host-import callbacks, and the ordinary JavaScript
    /// wrappers (`vmPopStack`, `vmInvokeOperation`) proved by `detect_wrappers`.
    callback_context_params: Vec<SymbolId>,

    slot_literals: BTreeSet<NodeId>,
    descriptor_records: BTreeSet<NodeId>,
    fn_this: BTreeMap<NodeId, ObjType>,
}

pub(crate) struct Plan {
    changes: usize,
    binding_names: BTreeMap<SymbolId, (String, String)>,
    member_names: BTreeMap<NodeId, String>,
    object_keys: BTreeMap<NodeId, String>,
    string_values: BTreeMap<NodeId, String>,
    anon_names: BTreeMap<NodeId, String>,
    strip_function_names: BTreeSet<NodeId>,
    binding_mappings: Vec<BindingMapping>,
    property_mappings: Vec<PropertyMapping>,
    binding_residue: Vec<String>,
    proven_roles: Vec<ProvenRoleBinding>,
    report: Vec<Diagnostic>,
}

impl Plan {
    pub(crate) fn report(&self) -> &[Diagnostic] {
        &self.report
    }

    pub(crate) fn binding_mappings(&self) -> &[BindingMapping] {
        &self.binding_mappings
    }

    pub(crate) fn property_mappings(&self) -> &[PropertyMapping] {
        &self.property_mappings
    }

    pub(crate) fn binding_residue(&self) -> &[String] {
        &self.binding_residue
    }

    pub(crate) fn proven_roles(&self) -> &[ProvenRoleBinding] {
        &self.proven_roles
    }
}

/// Build the naming plan for a kernel. Owned, so the caller can drop the
/// semantic borrow of the program before applying the plan.
pub(crate) fn plan<'a>(program: &Program<'a>, scoping: &Scoping, nodes: &AstNodes<'a>) -> Plan {
    let Some((_, entry)) = crate::transforms::deob::pipeline::entry_function(program) else {
        return Plan {
            changes: 0,
            binding_names: BTreeMap::new(),
            member_names: BTreeMap::new(),
            object_keys: BTreeMap::new(),
            string_values: BTreeMap::new(),
            anon_names: BTreeMap::new(),
            strip_function_names: BTreeSet::new(),
            binding_mappings: Vec::new(),
            property_mappings: Vec::new(),
            binding_residue: Vec::new(),
            proven_roles: Vec::new(),
            report: Vec::new(),
        };
    };
    let entry_span = entry.span;
    let entry_symbol = entry.id.as_ref().and_then(|id| id.symbol_id.get());
    build(program, scoping, nodes, entry_span, entry_symbol)
}

fn build<'a>(
    program: &Program<'a>,
    scoping: &Scoping,
    nodes: &AstNodes<'a>,
    entry_span: Span,
    entry_symbol: Option<SymbolId>,
) -> Plan {
    let __profile = crate::profile::start();
    let collected = collect(nodes, scoping);
    crate::profile::done("naming.collect", __profile);
    let mut facts = Facts {
        entry: entry_symbol,
        ..Facts::default()
    };
    let __profile = crate::profile::start();
    detect_decoders(nodes, &mut facts);
    detect_wasm(nodes, &collected, scoping, &mut facts);
    detect_tables(&collected, scoping, &mut facts);
    detect_stack(nodes, scoping, &collected, &mut facts);
    detect_memory(nodes, scoping, &collected, &mut facts);
    detect_memory_aliases(nodes, &collected, scoping, &mut facts);
    detect_slot_descriptor(nodes, scoping, &collected, &mut facts);
    detect_context(nodes, scoping, &mut facts);
    detect_operations(nodes, scoping, &collected, &mut facts);
    detect_threads(nodes, scoping, &collected, &mut facts);
    detect_guarded(nodes, scoping, &collected, &mut facts);
    detect_wasm_imports(nodes, scoping, &collected, &mut facts);
    detect_wrappers(nodes, scoping, &collected, &mut facts);
    detect_worker(nodes, scoping, &collected, &mut facts);
    detect_proto_this(nodes, scoping, &collected, &mut facts);
    classify_context_fields(nodes, &mut facts);
    detect_context_prototype(nodes, scoping, &collected, &mut facts);
    // The completion fallback needs the proven dispatch method as run-loop
    // evidence, so it runs after the prototype proof.
    detect_completion_sentinels(nodes, scoping, &collected, &mut facts);
    let _ = program;
    crate::profile::done("naming.detect", __profile);

    let __profile = crate::profile::start();
    let types = infer_types(nodes, scoping, &facts, &collected);
    classify_context_stack_fields(nodes, scoping, &collected, &mut facts, &types);
    build_context_member_names(&mut facts, &types);
    crate::profile::done("naming.infer_types", __profile);
    let __profile = crate::profile::start();
    let decisions = decide(nodes, scoping, &collected, &facts, &types);
    crate::profile::done("naming.decide", __profile);
    let __profile = crate::profile::start();
    let plan = finalize(
        program, nodes, scoping, &collected, &facts, &types, decisions, entry_span,
    );
    crate::profile::done("naming.finalize", __profile);
    plan
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

fn collect<'a>(nodes: &AstNodes<'a>, scoping: &Scoping) -> Collected<'a> {
    let mut decls = BTreeMap::new();
    let mut shapes = BTreeMap::new();
    let mut functions = BTreeMap::new();
    let mut array_functions = BTreeMap::new();
    let mut assigned_symbol = BTreeMap::new();
    let mut catch_params = BTreeSet::new();
    let mut class_symbols = BTreeSet::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::VariableDeclarator(decl) => {
                let Some(binding) = simple_binding(&decl.id) else {
                    continue;
                };
                let Some(symbol) = binding.symbol_id.get() else {
                    continue;
                };
                let init = decl.init.as_ref();
                let shape = init.map_or(Shape::Other, |expression| shape_of(expression, scoping));
                if matches!(shape, Shape::Functions(_)) {
                    let nodes = function_nodes(init.expect("shape has initializer"));
                    array_functions.insert(symbol, nodes);
                }
                match init {
                    Some(Expression::FunctionExpression(function)) => {
                        assigned_symbol.insert(function.node_id.get(), symbol);
                    }
                    Some(Expression::ArrowFunctionExpression(arrow)) => {
                        assigned_symbol.insert(arrow.node_id.get(), symbol);
                    }
                    _ => {}
                }
                shapes.insert(symbol, shape);
                decls.insert(symbol, DeclInfo { init });
            }
            AstKind::Function(function) => {
                functions.insert(
                    node.id(),
                    FnInfo {
                        symbol: function.id.as_ref().and_then(|id| id.symbol_id.get()),
                        params: function
                            .params
                            .items
                            .iter()
                            .filter_map(|parameter| binding_symbol(&parameter.pattern))
                            .collect(),
                    },
                );
            }
            AstKind::ArrowFunctionExpression(arrow) => {
                functions.insert(
                    node.id(),
                    FnInfo {
                        symbol: None,
                        params: arrow
                            .params
                            .items
                            .iter()
                            .filter_map(|parameter| binding_symbol(&parameter.pattern))
                            .collect(),
                    },
                );
            }
            AstKind::CatchClause(catch) => {
                if let Some(parameter) = &catch.param {
                    if let Some(symbol) = binding_symbol(&parameter.pattern) {
                        catch_params.insert(symbol);
                    }
                }
            }
            AstKind::Class(class) => {
                if let Some(id) = &class.id {
                    if let Some(symbol) = id.symbol_id.get() {
                        class_symbols.insert(symbol);
                    }
                }
            }
            _ => {}
        }
    }
    Collected {
        decls,
        shapes,
        functions,
        array_functions,
        assigned_symbol,
        catch_params,
        class_symbols,
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

fn detect_decoders<'a>(nodes: &AstNodes<'a>, facts: &mut Facts) {
    for node in nodes.iter() {
        let AstKind::StringLiteral(literal) = node.kind() else {
            continue;
        };
        if literal.value != crate::runtime_decoder::STANDARD_ALPHABET
            && literal.value != crate::runtime_decoder::URL_SAFE_ALPHABET
        {
            continue;
        }
        let mut fallback_binding = None;
        for ancestor in nodes.ancestors(node.id()) {
            match ancestor.kind() {
                AstKind::Function(function) => {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        facts.decoder.insert(symbol);
                    }
                    fallback_binding = None;
                    break;
                }
                AstKind::ArrowFunctionExpression(_) => break,
                AstKind::VariableDeclarator(decl) => {
                    fallback_binding = binding_symbol(&decl.id);
                }
                _ => {}
            }
        }
        if let Some(symbol) = fallback_binding {
            facts.decoder.insert(symbol);
        }
    }
}

fn detect_wasm<'a>(
    nodes: &AstNodes<'a>,
    collected: &Collected<'a>,
    scoping: &Scoping,
    facts: &mut Facts,
) {
    let mut host_candidates = BTreeSet::new();
    for (symbol, info) in &collected.decls {
        let Some(init) = info.init else {
            continue;
        };
        if is_new_wasm_memory(init, scoping) {
            facts.linear_memory = Some(*symbol);
        }
        if let Expression::NewExpression(new) = init {
            if is_webassembly_module(&new.callee, scoping) {
                facts.wasm_modules.push(*symbol);
                if let Some(Argument::Identifier(id)) = new.arguments.first() {
                    if let Some(source) = crate::analysis::identifier_symbol(id, scoping) {
                        facts.wasm_inputs.insert(source);
                        facts.wasm_input_order.push(source);
                    }
                }
            }
            if is_webassembly_instance(&new.callee, scoping) {
                facts.wasm_instances.push(*symbol);
                if let Some(Expression::ObjectExpression(imports)) = new
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .and_then(|expression| {
                        resolved_object_expression(expression, collected, scoping)
                    })
                {
                    for property in &imports.properties {
                        let ObjectPropertyKind::ObjectProperty(property) = property else {
                            continue;
                        };
                        let Some(Expression::ObjectExpression(namespace)) =
                            resolved_object_expression(&property.value, collected, scoping)
                        else {
                            continue;
                        };
                        if namespace.properties.iter().any(|entry| {
                            let ObjectPropertyKind::ObjectProperty(entry) = entry else {
                                return false;
                            };
                            resolved_callable_expression(&entry.value, collected, scoping)
                        }) {
                            if let Expression::Identifier(id) = &property.value {
                                if let Some(symbol) =
                                    crate::analysis::identifier_symbol(id, scoping)
                                {
                                    host_candidates.insert(symbol);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    facts.host_imports = (host_candidates.len() == 1).then(|| {
        host_candidates
            .into_iter()
            .next()
            .expect("one host candidate")
    });
    for node in nodes.iter() {
        let AstKind::NewExpression(new) = node.kind() else {
            continue;
        };
        if !is_webassembly_module(&new.callee, scoping) {
            continue;
        }
        let Some(Argument::Identifier(id)) = new.arguments.first() else {
            continue;
        };
        let Some(source) = crate::analysis::identifier_symbol(id, scoping) else {
            continue;
        };
        if facts.wasm_inputs.insert(source) {
            facts.wasm_input_order.push(source);
        }
    }
}

fn resolved_object_expression<'a>(
    expression: &'a Expression<'a>,
    collected: &'a Collected<'a>,
    scoping: &Scoping,
) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::ObjectExpression(_) => Some(expression),
        Expression::Identifier(id) => {
            let symbol = crate::analysis::identifier_symbol(id, scoping)?;
            match collected.decls.get(&symbol)?.init {
                Some(Expression::ObjectExpression(_)) => collected.decls.get(&symbol)?.init,
                _ => None,
            }
        }
        _ => None,
    }
}

fn resolved_callable_expression(
    expression: &Expression<'_>,
    collected: &Collected<'_>,
    scoping: &Scoping,
) -> bool {
    resolved_callable_expression_inner(expression, collected, scoping, &mut BTreeSet::new())
}

fn resolved_callable_expression_inner(
    expression: &Expression<'_>,
    collected: &Collected<'_>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> bool {
    match expression {
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::Identifier(id) => {
            let Some(symbol) = crate::analysis::identifier_symbol(id, scoping) else {
                return false;
            };
            if !seen.insert(symbol) {
                return false;
            }
            collected
                .decls
                .get(&symbol)
                .and_then(|info| info.init)
                .is_some_and(|init| {
                    resolved_callable_expression_inner(init, collected, scoping, seen)
                })
        }
        _ => false,
    }
}

fn detect_tables<'a>(collected: &Collected<'a>, scoping: &Scoping, facts: &mut Facts) {
    for (symbol, shape) in &collected.shapes {
        match shape {
            Shape::Strings(length) if *length > 32 => facts.string_table = Some(*symbol),
            Shape::Numbers(length) if *length >= 4 => facts.number_table = Some(*symbol),
            Shape::Cache => facts.cache = Some(*symbol),
            _ => {}
        }
    }
    // The null-prototype cache is often created through a `Object.create` member
    // alias (`var create = Object.create; var cache = create(null)`), which the
    // direct `Object.create(null)` shape does not see.
    if facts.cache.is_none() {
        for (symbol, info) in &collected.decls {
            if let Some(init) = info.init {
                if is_resolved_null_object_cache(init, scoping, collected) {
                    facts.cache = Some(*symbol);
                }
            }
        }
    }
    for (symbol, info) in &collected.decls {
        if info
            .init
            .is_some_and(|init| is_native_function_catalogue(init, scoping, collected))
        {
            facts.native_functions = Some(*symbol);
            break;
        }
    }
}

fn detect_stack<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        let mut returned: Option<SymbolId> = None;
        for statement in &body.statements {
            if let Statement::ReturnStatement(ret) = statement {
                if let Some(Expression::Identifier(id)) = &ret.argument {
                    returned = crate::analysis::identifier_symbol(id, scoping);
                }
            }
        }
        let Some(returned) = returned else {
            continue;
        };
        let mut methods: BTreeMap<String, String> = BTreeMap::new();
        // A non-literal key on the returned array could name one of the shuffled
        // method slots, so the factory is not provable: renaming only the
        // literal-key slots would leave a partially mapped object.
        let mut declined = false;
        for statement in &body.statements {
            let Statement::ExpressionStatement(expression) = statement else {
                continue;
            };
            let Expression::CallExpression(call) = &expression.expression else {
                continue;
            };
            if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3 {
                continue;
            }
            // Only the array this factory returns is a stack; a defineProperty
            // on any other object is unrelated to the returned array.
            let target = call.arguments[0]
                .as_expression()
                .and_then(|argument| expression_symbol(argument, scoping));
            if target != Some(returned) {
                continue;
            }
            let Some(Expression::StringLiteral(key)) = call.arguments[1].as_expression() else {
                declined = true;
                continue;
            };
            let Some(Expression::ObjectExpression(descriptor)) = call.arguments[2].as_expression()
            else {
                continue;
            };
            if let Some(value) = descriptor_value(descriptor) {
                if let Some(name) = stack_method_name(value, collected, scoping) {
                    methods.insert(key.value.to_string(), name.into());
                }
            }
        }
        if !declined && !methods.is_empty() {
            facts.stack_factory = Some(symbol);
            facts.stack_local = Some(returned);
            facts.stack_members = methods;
        }
    }
}

fn stack_method_name<'a>(
    expression: &Expression<'a>,
    collected: &Collected<'a>,
    scoping: &Scoping,
) -> Option<&'static str> {
    if let Expression::Identifier(id) = expression {
        if let Some(symbol) = crate::analysis::identifier_symbol(id, scoping) {
            if let Some(info) = collected.decls.get(&symbol) {
                if let Some(init) = info.init {
                    return array_method_name(init);
                }
            }
        }
    }
    array_method_name(expression)
}

fn array_method_name(expression: &Expression<'_>) -> Option<&'static str> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    if !is_array_receiver(&member.object) {
        return None;
    }
    match member.property.name.as_str() {
        "pop" => Some("pop"),
        "push" => Some("push"),
        "slice" => Some("slice"),
        "splice" => Some("splice"),
        _ => None,
    }
}

/// Whether `object` is an array-native receiver for a stack method.
///
/// Both the raw form (`[].pop`) and the canonical form native CLEAN inlines
/// aliases to (`Array.prototype.pop`) are accepted, so role detection keeps
/// working after the cleanup pass has run.
fn is_array_receiver(object: &Expression<'_>) -> bool {
    match object {
        Expression::ArrayExpression(array) => array.elements.is_empty(),
        Expression::StaticMemberExpression(member) => {
            member.property.name == "prototype"
                && matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "Array")
        }
        _ => false,
    }
}

fn detect_memory<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    for node in nodes.iter() {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            continue;
        };
        let AssignmentTarget::ComputedMemberExpression(member) = &assign.left else {
            continue;
        };
        let Expression::StaticMemberExpression(slot) = &member.object else {
            continue;
        };
        if !matches!(slot.object, Expression::ThisExpression(_)) {
            continue;
        }
        let Expression::ObjectExpression(object) = &assign.right else {
            continue;
        };
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if property.computed {
                continue;
            }
            if matches!(&property.value, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void)
            {
                facts.slots_key = Some(slot.property.name.to_string());
                facts.slot_value_key = Some(property.key_name());
                facts.slot_literals.insert(object.node_id());
            }
        }
    }
    let (Some(slots_key), Some(slot_value_key)) =
        (facts.slots_key.clone(), facts.slot_value_key.clone())
    else {
        return;
    };

    for node in nodes.iter() {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            continue;
        };
        let AssignmentTarget::StaticMemberExpression(member) = &assign.left else {
            continue;
        };
        if member.property.name.as_str() != slots_key
            || !matches!(member.object, Expression::ThisExpression(_))
        {
            continue;
        }
        if !matches!(assign.right, Expression::ArrayExpression(_)) {
            continue;
        }
        if let Some(function) = enclosing_function(nodes, node.id()) {
            facts.memory_ctor = function.id.as_ref().and_then(|id| id.symbol_id.get());
        }
    }
    let Some(memory_ctor) = facts.memory_ctor else {
        return;
    };

    let mut methods: BTreeMap<String, String> = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3 {
            continue;
        }
        let Some(target) = call.arguments[0].as_expression() else {
            continue;
        };
        if proto_of(target, scoping, collected) != Some(memory_ctor) {
            continue;
        }
        let Some(Expression::StringLiteral(key)) = call.arguments[1].as_expression() else {
            continue;
        };
        let name = key.value.to_string();
        if name == slots_key {
            methods.insert(name, "slots".into());
            continue;
        }
        let Some(Expression::ObjectExpression(descriptor)) = call.arguments[2].as_expression()
        else {
            continue;
        };
        let Some(value) = descriptor_value(descriptor) else {
            continue;
        };
        if let Some(classification) = classify_slot_method(
            value,
            &slots_key,
            &slot_value_key,
            Some(memory_ctor),
            scoping,
        ) {
            methods.insert(name, classification.to_string());
        }
    }
    if methods.is_empty() {
        facts.memory_ctor = None;
        facts.slots_key = None;
        facts.slot_value_key = None;
        return;
    }
    facts.memory_members = methods;
}

fn detect_memory_aliases(
    nodes: &AstNodes<'_>,
    collected: &Collected<'_>,
    scoping: &Scoping,
    facts: &mut Facts,
) {
    let Some(memory_ctor) = facts.memory_ctor else {
        return;
    };
    loop {
        let mut changed = false;
        for (symbol, info) in &collected.decls {
            let Some(Expression::Identifier(source)) = info.init else {
                continue;
            };
            let Some(source_symbol) = crate::analysis::identifier_symbol(source, scoping) else {
                continue;
            };
            if (source_symbol == memory_ctor || facts.memory_aliases.contains(&source_symbol))
                && facts.memory_aliases.insert(*symbol)
            {
                changed = true;
            }
        }
        for node in nodes.iter() {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            let Expression::Identifier(source) = &assignment.right else {
                continue;
            };
            let Some(target_symbol) = crate::analysis::identifier_symbol(target, scoping) else {
                continue;
            };
            let Some(source_symbol) = crate::analysis::identifier_symbol(source, scoping) else {
                continue;
            };
            if source_symbol == memory_ctor || facts.memory_aliases.contains(&source_symbol) {
                changed |= facts.memory_aliases.insert(target_symbol);
            }
        }
        if !changed {
            break;
        }
    }
}

/// Prove the reused property descriptor for the memory `slots` initializer.
///
/// The descriptor is identified by its consumer: a `value`-keyed object literal
/// bound to a variable and passed as the third argument of `defineProperty` for
/// the proven `slots` key on the memory prototype. `value`/`writable` are
/// built-in JavaScript descriptor keys, so they are preserved rather than
/// renamed, and the `SymbolId` binding identity is preserved across later
/// in-place `writable` mutations or reassignment.
fn detect_slot_descriptor<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let (Some(memory_ctor), Some(slots_key)) = (facts.memory_ctor, facts.slots_key.clone()) else {
        return;
    };
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3 {
            continue;
        }
        let (Some(target), Some(Expression::StringLiteral(key)), Some(descriptor)) = (
            call.arguments[0].as_expression(),
            call.arguments[1].as_expression(),
            call.arguments[2].as_expression(),
        ) else {
            continue;
        };
        if proto_of(target, scoping, collected) != Some(memory_ctor)
            || key.value.as_str() != slots_key
        {
            continue;
        }
        let Expression::Identifier(identifier) = descriptor else {
            continue;
        };
        let Some(symbol) = reference_symbol(identifier, scoping) else {
            continue;
        };
        // Identity is the SymbolId, so an in-place `writable` toggle or a later
        // reassignment keeps the same canonical descriptor binding; only the
        // object literal's own keys stay untouched.
        let Some(Expression::ObjectExpression(object)) =
            collected.decls.get(&symbol).and_then(|info| info.init)
        else {
            continue;
        };
        // `value` is the built-in descriptor key that carries the initial
        // slots value; it is not the minified slot-record key.
        if !object.properties.iter().any(|property| {
            matches!(property, ObjectPropertyKind::ObjectProperty(property)
                if !property.computed && property.key_name() == "value")
        }) {
            continue;
        }
        facts.slot_descriptor = Some(symbol);
        return;
    }
}

fn classify_slot_method(
    value: &Expression<'_>,
    slots_key: &str,
    slot_value_key: &str,
    memory_ctor: Option<SymbolId>,
    scoping: &Scoping,
) -> Option<&'static str> {
    let body = match value {
        Expression::FunctionExpression(function) => function.body.as_ref(),
        Expression::ArrowFunctionExpression(arrow) => Some(&arrow.body),
        _ => None,
    }?;
    let mut init = false;
    let mut read = false;
    let mut write = false;
    for statement in &body.statements {
        match statement {
            Statement::ReturnStatement(ret) => {
                if ret
                    .argument
                    .as_ref()
                    .is_some_and(|argument| is_slot_read(argument, slots_key, slot_value_key))
                {
                    read = true;
                }
            }
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assign) = &expression.expression else {
                    continue;
                };
                if is_slot_member_assignment(&assign.left, slots_key, slot_value_key) {
                    write = true;
                }
                if is_slot_init_assignment(&assign.left, &assign.right, slots_key, slot_value_key) {
                    init = true;
                }
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let Some(Expression::NewExpression(new)) = declarator.init.as_ref() {
                        if memory_ctor.is_some()
                            && callee_symbol(&new.callee, scoping) == memory_ctor
                        {
                            return Some("cloneMemory");
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if init {
        Some("initializeSlot")
    } else if write {
        Some("writeSlot")
    } else if read {
        Some("readSlot")
    } else {
        None
    }
}

fn is_slot_init_assignment(
    target: &AssignmentTarget<'_>,
    value: &Expression<'_>,
    slots_key: &str,
    slot_value_key: &str,
) -> bool {
    let AssignmentTarget::ComputedMemberExpression(member) = target else {
        return false;
    };
    if !is_slots_member(&member.object, slots_key) {
        return false;
    }
    let Expression::ObjectExpression(object) = value else {
        return false;
    };
    object.properties.iter().any(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            property.key_name() == slot_value_key
                && matches!(&property.value, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void)
        }
        _ => false,
    })
}

fn is_slot_member_assignment(
    target: &AssignmentTarget<'_>,
    slots_key: &str,
    slot_value_key: &str,
) -> bool {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return false;
    };
    member.property.name == slot_value_key && is_slots_index(&member.object, slots_key)
}

fn is_slot_read(expression: &Expression<'_>, slots_key: &str, slot_value_key: &str) -> bool {
    let Expression::StaticMemberExpression(member) = expression else {
        return false;
    };
    member.property.name == slot_value_key && is_slots_index(&member.object, slots_key)
}

fn is_slots_index(expression: &Expression<'_>, slots_key: &str) -> bool {
    let Expression::ComputedMemberExpression(member) = expression else {
        return false;
    };
    is_slots_member(&member.object, slots_key)
}

fn is_slots_member(expression: &Expression<'_>, slots_key: &str) -> bool {
    matches!(expression, Expression::StaticMemberExpression(member)
        if member.property.name == slots_key
            && matches!(member.object, Expression::ThisExpression(_)))
}

fn detect_context<'a>(nodes: &AstNodes<'a>, scoping: &Scoping, facts: &mut Facts) {
    let Some(factory) = facts.stack_factory else {
        return;
    };
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        let params: Vec<SymbolId> = function
            .params
            .items
            .iter()
            .filter_map(|parameter| binding_symbol(&parameter.pattern))
            .collect();
        let mut fields: Vec<(String, FieldRhs)> = Vec::new();
        let mut keys = BTreeSet::new();
        let mut stack_fields = Vec::new();
        for statement in &body.statements {
            let Statement::ExpressionStatement(expression) = statement else {
                continue;
            };
            let Expression::AssignmentExpression(assign) = &expression.expression else {
                continue;
            };
            let AssignmentTarget::StaticMemberExpression(member) = &assign.left else {
                continue;
            };
            if !matches!(member.object, Expression::ThisExpression(_)) {
                continue;
            }
            let key = member.property.name.to_string();
            let rhs = if is_call_to(&assign.right, factory, scoping) {
                stack_fields.push(key.clone());
                FieldRhs::StackCall
            } else if matches!(&assign.right, Expression::ConditionalExpression(_)) {
                FieldRhs::Conditional
            } else if let Expression::Identifier(id) = &assign.right {
                match crate::analysis::identifier_symbol(id, scoping) {
                    Some(source) if params.contains(&source) => FieldRhs::Param(source),
                    _ => FieldRhs::Other,
                }
            } else {
                FieldRhs::Other
            };
            keys.insert(key.clone());
            fields.push((key, rhs));
        }
        if !keys.is_empty() && stack_fields.len() >= 2 {
            facts.context_ctor = Some(symbol);
            facts.context_params = params;
            facts.context_keys = keys;
            facts.context_field_rhs = fields;
            facts.context_stack_fields = stack_fields;
        }
    }
}

/// Prove the context/memory prototype aliases and the dispatch method installed
/// on the context prototype.
///
/// The dispatch method is the only function that indexes a per-state table with
/// a bytecode read at a context-owned cursor. Its two context operands are
/// exactly the interpreter's dispatch state and byte offset, so the same proof
/// names those constructor fields and their parameters.
fn detect_context_prototype<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let Some(context_ctor) = facts.context_ctor else {
        return;
    };
    for (symbol, info) in &collected.decls {
        let Some(Expression::StaticMemberExpression(member)) = info.init else {
            continue;
        };
        if member.property.name != "prototype" {
            continue;
        }
        let Some(source) = expression_symbol(&member.object, scoping) else {
            continue;
        };
        if Some(source) == facts.memory_ctor {
            facts.memory_proto_alias = Some(*symbol);
        } else if source == context_ctor {
            facts.context_proto_alias = Some(*symbol);
        }
    }

    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3 {
            continue;
        }
        let (Some(target), Some(Expression::StringLiteral(key)), Some(descriptor)) = (
            call.arguments[0].as_expression(),
            call.arguments[1].as_expression(),
            call.arguments[2].as_expression(),
        ) else {
            continue;
        };
        if proto_of(target, scoping, collected) != Some(context_ctor) {
            continue;
        }
        let Expression::ObjectExpression(descriptor) = descriptor else {
            continue;
        };
        let Some(Expression::FunctionExpression(function)) = descriptor_value(descriptor) else {
            continue;
        };
        let Some((table, bytecode, state_field, pc_field)) =
            dispatch_method_shape(nodes, function, scoping, collected)
        else {
            continue;
        };
        facts.context_dispatch_key = Some(key.value.to_string());
        facts.dispatch_table = Some(table);
        facts.bytecode = Some(bytecode);
        facts.context_state_field = Some(state_field);
        facts.context_pc_field = Some(pc_field);
        break;
    }
}

/// Recognize `table[this.<state>][image[this.<cursor>++]]` inside a prototype
/// method. The outer table must be a literal array binding and the inner image
/// must be a bytecode binding, so an unrelated nested index does not qualify.
fn dispatch_method_shape(
    nodes: &AstNodes<'_>,
    function: &Function<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> Option<(SymbolId, SymbolId, String, String)> {
    let function_id = function.node_id.get();
    for node in nodes.iter() {
        if !within_function(nodes, node.id(), function_id) {
            continue;
        }
        let AstKind::ComputedMemberExpression(outer) = node.kind() else {
            continue;
        };
        let Expression::ComputedMemberExpression(row) = &outer.object else {
            continue;
        };
        let Expression::Identifier(table) = &row.object else {
            continue;
        };
        let Expression::StaticMemberExpression(state) = &row.expression else {
            continue;
        };
        if !matches!(state.object, Expression::ThisExpression(_)) {
            continue;
        }
        let Expression::ComputedMemberExpression(bytecode) = &outer.expression else {
            continue;
        };
        let Expression::Identifier(image) = &bytecode.object else {
            continue;
        };
        let Expression::UpdateExpression(update) = &bytecode.expression else {
            continue;
        };
        let SimpleAssignmentTarget::StaticMemberExpression(cursor) = &update.argument else {
            continue;
        };
        if !matches!(cursor.object, Expression::ThisExpression(_)) {
            continue;
        }
        let table_symbol = reference_symbol(table, scoping)?;
        let image_symbol = reference_symbol(image, scoping)?;
        if !is_literal_array_binding(collected, table_symbol) {
            continue;
        }
        if !is_array_or_decoded_binding(collected, image_symbol) {
            continue;
        }
        return Some((
            table_symbol,
            image_symbol,
            state.property.name.to_string(),
            cursor.property.name.to_string(),
        ));
    }
    None
}

fn is_literal_array_binding(collected: &Collected<'_>, symbol: SymbolId) -> bool {
    matches!(
        collected.decls.get(&symbol).and_then(|info| info.init),
        Some(Expression::ArrayExpression(_))
    )
}

fn is_array_or_decoded_binding(collected: &Collected<'_>, symbol: SymbolId) -> bool {
    matches!(
        collected.decls.get(&symbol).and_then(|info| info.init),
        Some(Expression::ArrayExpression(_)) | Some(Expression::CallExpression(_))
    )
}

/// Classify every context stack field from the frame records pushed and popped
/// through it.
///
/// A value stack is the field left after the proven call frame, try frame,
/// exception tracker and thrown-value stack are removed. Only when no role is
/// proven do the first two remaining fields fall back to the historical
/// `stack`/`errorStack` constructor order; any further field stays unresolved
/// rather than receiving a guessed name.
fn classify_context_stack_fields<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
    types: &Types,
) {
    if facts.context_stack_fields.is_empty() {
        return;
    }
    let stack_fields: BTreeSet<String> = facts.context_stack_fields.iter().cloned().collect();
    let mut roles: BTreeMap<String, &'static str> = BTreeMap::new();

    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::StaticMemberExpression(method) = &call.callee else {
            continue;
        };
        let Expression::StaticMemberExpression(stack) = &method.object else {
            continue;
        };
        let field = stack.property.name.to_string();
        if !stack_fields.contains(&field) {
            continue;
        }
        if receiver_type(
            &stack.object,
            scoping,
            facts,
            types,
            enclosing_this(nodes, node.id(), facts),
        ) != Some(ObjType::Context)
        {
            continue;
        }
        let method_name = facts
            .stack_members
            .get(method.property.name.as_str())
            .map(String::as_str);
        match method_name {
            Some("push") => {
                let Some(argument) = call.arguments.first().and_then(Argument::as_expression)
                else {
                    continue;
                };
                match argument {
                    Expression::ObjectExpression(object) => {
                        let mut has_boolean = false;
                        let mut has_zero = false;
                        let mut count = 0usize;
                        for property in &object.properties {
                            let ObjectPropertyKind::ObjectProperty(property) = property else {
                                continue;
                            };
                            count += 1;
                            match &property.value {
                                Expression::BooleanLiteral(_) => has_boolean = true,
                                Expression::NumericLiteral(number) if number.value == 0.0 => {
                                    has_zero = true;
                                }
                                _ => {}
                            }
                        }
                        let role = if has_boolean {
                            "errorStack"
                        } else if count == 2 {
                            "callFrameStack"
                        } else if count == 3 && has_zero {
                            "tryFrameStack"
                        } else {
                            continue;
                        };
                        insert_role(&mut roles, field, role);
                    }
                    Expression::Identifier(_) => {
                        let in_catch = nodes
                            .ancestors(node.id())
                            .any(|ancestor| matches!(ancestor.kind(), AstKind::CatchClause(_)));
                        if in_catch {
                            insert_role(&mut roles, field, "errorStack");
                        } else {
                            insert_role(&mut roles, field, "thrownValueStack");
                        }
                    }
                    _ => {}
                }
            }
            Some("pop") => {
                if matches!(nodes.parent_kind(node.id()), AstKind::ThrowStatement(_)) {
                    insert_role(&mut roles, field, "thrownValueStack");
                }
            }
            _ => {}
        }
    }
    // `vmRun` asks whether the try-frame stack is empty before deciding between
    // direct and guarded operation invocation.
    if let Some(run) = facts
        .run
        .and_then(|symbol| thread_function_node(nodes, collected, Some(symbol)))
    {
        for node in nodes.iter() {
            if !within_function(nodes, node.id(), run) {
                continue;
            }
            let AstKind::BinaryExpression(binary) = node.kind() else {
                continue;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::StrictEquality | BinaryOperator::Equality
            ) {
                continue;
            }
            for expression in [&binary.left, &binary.right] {
                let Expression::StaticMemberExpression(member) = expression else {
                    continue;
                };
                if member.property.name != "length" {
                    continue;
                }
                let Expression::StaticMemberExpression(stack) = &member.object else {
                    continue;
                };
                let field = stack.property.name.to_string();
                if stack_fields.contains(&field) {
                    insert_role(&mut roles, field, "tryFrameStack");
                }
            }
        }
    }

    // Locals receiving a frame record, through either a declarator or a later
    // assignment: `var frame = context.<field>.pop()` / `frame = ...`.
    //
    // The map keeps *every* assignment to the local, including unrelated ones.
    // A member read is a true frame use only when the nearest preceding
    // assignment produced a proven frame record; a later `frame = { ... }` or a
    // reassignment from another frame schema therefore keeps unrelated fields
    // intact instead of inheriting a stale schema.
    let frame_assignments = collect_frame_assignments(nodes, scoping, facts, types);
    for (symbol, assignments) in &frame_assignments {
        let mut fields = BTreeSet::new();
        let mut conflicting = false;
        for (_, field) in assignments {
            match field {
                Some(field) => {
                    fields.insert(field.as_str());
                }
                None => conflicting = true,
            }
        }
        // Only name the binding itself when every assignment is the same proven
        // frame record; a reused/unrelated local keeps its source spelling.
        if conflicting || fields.len() != 1 {
            continue;
        }
        if let Some(field) = fields.into_iter().next() {
            facts.frame_locals.insert(*symbol, field.to_string());
        }
    }

    // Frame keys are proven by the consumer that restores the saved context
    // coordinates, tests the handled flag, or bounds an unwind loop.
    let mut frame_keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::AssignmentExpression(assignment) => {
                let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
                    continue;
                };
                let Expression::StaticMemberExpression(source) = &assignment.right else {
                    continue;
                };
                let Some(frame) = expression_symbol(&source.object, scoping) else {
                    continue;
                };
                let Some(field) = frame_field_at(&frame_assignments, frame, source.span.start)
                else {
                    continue;
                };
                let role = roles.get(field).copied();
                let semantic =
                    if facts.context_pc_field.as_deref() == Some(target.property.name.as_str()) {
                        match role {
                            Some("tryFrameStack") => "handlerByteOffset",
                            Some("callFrameStack") => "returnByteOffset",
                            _ => "savedByteOffset",
                        }
                    } else if facts.context_state_field.as_deref()
                        == Some(target.property.name.as_str())
                    {
                        match role {
                            Some("tryFrameStack") => "handlerDispatchState",
                            Some("callFrameStack") => "returnDispatchState",
                            _ => "savedDispatchState",
                        }
                    } else {
                        continue;
                    };
                frame_keys
                    .entry(field.to_string())
                    .or_default()
                    .insert(source.property.name.to_string(), semantic.into());
            }
            AstKind::IfStatement(statement) => {
                let mut test = &statement.test;
                while let Expression::ParenthesizedExpression(paren) = test {
                    test = &paren.expression;
                }
                let Expression::StaticMemberExpression(member) = test else {
                    continue;
                };
                let Some(frame) = expression_symbol(&member.object, scoping) else {
                    continue;
                };
                if let Some(field) = frame_field_at(&frame_assignments, frame, member.span.start) {
                    frame_keys.entry(field.to_string()).or_default().insert(
                        member.property.name.to_string(),
                        "wasExceptionHandled".into(),
                    );
                }
            }
            AstKind::ForStatement(statement) => {
                let Some(Expression::BinaryExpression(binary)) = statement.test.as_ref() else {
                    continue;
                };
                for expression in [&binary.left, &binary.right] {
                    let Expression::StaticMemberExpression(member) = expression else {
                        continue;
                    };
                    let Some(frame) = expression_symbol(&member.object, scoping) else {
                        continue;
                    };
                    let Some(field) = frame_field_at(&frame_assignments, frame, member.span.start)
                    else {
                        continue;
                    };
                    if roles.get(field).copied() == Some("tryFrameStack") {
                        frame_keys
                            .entry(field.to_string())
                            .or_default()
                            .insert(member.property.name.to_string(), "errorEntryCount".into());
                        for local in zero_initialized_loop_locals(statement.init.as_ref(), scoping)
                        {
                            facts.unwind_index_locals.insert(local);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Remove any partial stack classification that would name two different
    // fields the same role; the first structural proof wins and the rest stay
    // unresolved instead of being guessed.
    let mut used: BTreeSet<&'static str> = BTreeSet::new();
    roles.retain(|_, role| used.insert(role));

    let mut remaining: Vec<String> = facts
        .context_stack_fields
        .iter()
        .filter(|field| !roles.contains_key(*field))
        .cloned()
        .collect();
    if let Some(field) = remaining.first() {
        roles.insert(field.clone(), "stack");
        remaining.remove(0);
    }
    if let Some(field) = remaining.first() {
        if !used.contains("errorStack") {
            roles.insert(field.clone(), "errorStack");
        }
    }

    for (field, role) in &roles {
        facts
            .context_member_names
            .insert(field.clone(), role.to_string());
    }
    facts.context_frame_keys = frame_keys;

    // Rename object-literal keys and member reads for every proven frame.
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::StaticMemberExpression(method) = &call.callee else {
            continue;
        };
        if facts
            .stack_members
            .get(method.property.name.as_str())
            .map(String::as_str)
            != Some("push")
        {
            continue;
        }
        let Expression::StaticMemberExpression(stack) = &method.object else {
            continue;
        };
        let field = stack.property.name.to_string();
        let Some(keys) = facts.context_frame_keys.get(&field) else {
            continue;
        };
        let Some(Expression::ObjectExpression(object)) =
            call.arguments.first().and_then(Argument::as_expression)
        else {
            continue;
        };
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if let Some(name) = keys.get(&property.key_name()) {
                facts
                    .frame_object_keys
                    .insert(property.node_id(), name.clone());
            }
        }
    }
    for node in nodes.iter() {
        let AstKind::StaticMemberExpression(member) = node.kind() else {
            continue;
        };
        let Some(frame) = expression_symbol(&member.object, scoping) else {
            continue;
        };
        let Some(field) = frame_field_at(&frame_assignments, frame, member.span.start) else {
            continue;
        };
        let Some(keys) = facts.context_frame_keys.get(field) else {
            continue;
        };
        if let Some(name) = keys.get(member.property.name.as_str()) {
            facts
                .frame_member_names
                .insert(member.node_id.get(), name.clone());
        }
    }
}

/// Every assignment to a local that ever receives a proven frame record, in
/// source order. `None` marks an assignment of an unrelated value (including a
/// different frame schema), so a read can be matched to the value that is
/// actually live at that point.
type FrameAssignments = BTreeMap<SymbolId, Vec<(u32, Option<String>)>>;

fn collect_frame_assignments<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    facts: &Facts,
    types: &Types,
) -> FrameAssignments {
    let mut assignments: FrameAssignments = BTreeMap::new();
    for node in nodes.iter() {
        let (local, effective_at, field) = match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                let Some(local) = binding_symbol(&declarator.id) else {
                    continue;
                };
                let field = match init {
                    Expression::CallExpression(call) => {
                        frame_pop_field(call, scoping, facts, types, nodes, node.id())
                    }
                    _ => None,
                };
                (local, declarator.span.end, field)
            }
            AstKind::AssignmentExpression(assignment) => {
                let Some(local) = assignment_target_symbol(&assignment.left, scoping) else {
                    continue;
                };
                let field = match &assignment.right {
                    Expression::CallExpression(call) => {
                        frame_pop_field(call, scoping, facts, types, nodes, node.id())
                    }
                    _ => None,
                };
                (local, assignment.span.end, field)
            }
            _ => continue,
        };
        assignments
            .entry(local)
            .or_default()
            .push((effective_at, field));
    }
    assignments
}

/// The proven frame field for the value of `local` at `position`, if the
/// nearest preceding assignment produced a frame record. Reads in a different
/// schema region, or after an unrelated assignment, resolve to `None`.
fn frame_field_at(assignments: &FrameAssignments, local: SymbolId, position: u32) -> Option<&str> {
    assignments
        .get(&local)?
        .iter()
        .filter(|(assigned_at, _)| *assigned_at <= position)
        .max_by_key(|(assigned_at, _)| *assigned_at)
        .and_then(|(_, field)| field.as_deref())
}

fn zero_initialized_loop_locals(
    init: Option<&ForStatementInit<'_>>,
    scoping: &Scoping,
) -> Vec<SymbolId> {
    match init {
        Some(ForStatementInit::VariableDeclaration(declaration)) => declaration
            .declarations
            .iter()
            .filter_map(|declarator| {
                let local = binding_symbol(&declarator.id)?;
                matches!(
                    declarator.init.as_ref(),
                    Some(Expression::NumericLiteral(number)) if number.value == 0.0
                )
                .then_some(local)
            })
            .collect(),
        Some(init) => init
            .as_expression()
            .map(|expression| {
                sequence_assignments(expression, scoping)
                    .into_iter()
                    .filter_map(|(local, right)| {
                        matches!(
                            right,
                            Expression::NumericLiteral(number) if number.value == 0.0
                        )
                        .then_some(local)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

fn insert_role(roles: &mut BTreeMap<String, &'static str>, field: String, role: &'static str) {
    roles.entry(field).or_insert(role);
}

/// The context stack field of a `<context>.<field>.pop()` call.
fn frame_pop_field(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    facts: &Facts,
    types: &Types,
    nodes: &AstNodes<'_>,
    node: NodeId,
) -> Option<String> {
    let Expression::StaticMemberExpression(method) = &call.callee else {
        return None;
    };
    if facts
        .stack_members
        .get(method.property.name.as_str())
        .map(String::as_str)
        != Some("pop")
    {
        return None;
    }
    let Expression::StaticMemberExpression(stack) = &method.object else {
        return None;
    };
    if receiver_type(
        &stack.object,
        scoping,
        facts,
        types,
        enclosing_this(nodes, node, facts),
    ) != Some(ObjType::Context)
    {
        return None;
    }
    Some(stack.property.name.to_string())
}

/// Build the complete semantic member map for the context instance. This is
/// deliberately one place so definitions, descriptor keys and every use agree.
fn build_context_member_names(facts: &mut Facts, types: &Types) {
    if let Some(field) = &facts.context_pc_field {
        facts
            .context_member_names
            .insert(field.clone(), "byteOffset".into());
    }
    if let Some(field) = &facts.context_state_field {
        facts
            .context_member_names
            .insert(field.clone(), "dispatchState".into());
    }
    for (field, rhs) in facts.context_field_rhs.clone() {
        if facts.context_member_names.contains_key(&field) {
            continue;
        }
        match rhs {
            FieldRhs::Conditional => {
                facts
                    .context_member_names
                    .insert(field, "receiverObject".into());
            }
            FieldRhs::Param(symbol) => {
                if types.symbols.get(&symbol) == Some(&ObjType::Memory) {
                    facts.context_member_names.insert(field, "memory".into());
                } else if facts.context_params.last() == Some(&symbol) {
                    facts
                        .context_member_names
                        .insert(field, "receiverValue".into());
                }
            }
            _ => {}
        }
    }
    if let Some(key) = &facts.context_dispatch_key {
        facts
            .context_member_names
            .insert(key.clone(), "dispatch".into());
    }
}

fn detect_operations<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let mut candidates: BTreeMap<SymbolId, usize> = BTreeMap::new();
    let mut proven: BTreeMap<SymbolId, (usize, bool)> = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::ComputedMemberExpression(member) = &call.callee else {
            continue;
        };
        let Expression::Identifier(base) = &member.object else {
            continue;
        };
        let Some(symbol) = crate::analysis::identifier_symbol(base, scoping) else {
            continue;
        };
        if matches!(collected.shapes.get(&symbol), Some(Shape::Functions(_))) {
            let Some(Shape::Functions(length)) = collected.shapes.get(&symbol) else {
                continue;
            };
            candidates
                .entry(symbol)
                .and_modify(|current| *current = (*current).max(*length))
                .or_insert(*length);
            if operation_is_proven(nodes, collected, scoping, symbol) {
                let dynamic_index = !matches!(
                    &member.expression,
                    Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                );
                if dynamic_index || operation_has_context_member(nodes, collected, scoping, symbol)
                {
                    proven
                        .entry(symbol)
                        .and_modify(|current| current.1 |= dynamic_index)
                        .or_insert((*length, dynamic_index));
                }
            }
        }
    }
    // Operation ABI naming keeps the broadest semantic candidate, while the
    // Operations role and object presentation require the stronger dispatch
    // proof below. Prefer the largest candidate/table deterministically.
    facts.operations = candidates
        .iter()
        .max_by_key(|(_, length)| **length)
        .map(|(symbol, _)| *symbol);
    facts.proven_operations = proven
        .iter()
        .max_by_key(|(_, (length, dynamic))| (*length, *dynamic))
        .map(|(symbol, _)| *symbol);
    if let Some(operations) = facts.operations {
        if let Some(functions) = collected.array_functions.get(&operations) {
            for function in functions {
                if let Some(info) = collected.functions.get(function) {
                    if let Some(parameter) = info.params.first() {
                        facts.callback_context_params.push(*parameter);
                    }
                }
            }
        }
    }
}

fn detect_threads<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let mut descriptor_local = None;
        for statement in &body.statements {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            for declarator in &declaration.declarations {
                if let Some(Expression::ComputedMemberExpression(member)) = declarator.init.as_ref()
                {
                    if let Expression::Identifier(base) = &member.object {
                        let base = crate::analysis::identifier_symbol(base, scoping);
                        if matches!(
                            base.and_then(|symbol| collected.shapes.get(&symbol)),
                            Some(Shape::Records(_))
                        ) {
                            descriptor_local = binding_symbol(&declarator.id);
                            facts.descriptors = base;
                        }
                    }
                }
            }
        }
        let Some(descriptor_local) = descriptor_local else {
            continue;
        };
        for statement in &body.statements {
            let Statement::ReturnStatement(ret) = statement else {
                continue;
            };
            let Some(Expression::CallExpression(call)) = &ret.argument else {
                continue;
            };
            if call.arguments.len() < 5 {
                continue;
            }
            let Some(fields) = member_chain(&call.arguments[3..], descriptor_local, scoping) else {
                continue;
            };
            let names = [
                "initialSlots",
                "capturedSlots",
                "argumentSlots",
                "argumentsSlot",
                "resultSlot",
            ];
            facts.descriptor_members.clear();
            for (field, name) in fields.iter().zip(names) {
                facts.descriptor_members.insert(field.clone(), name.into());
            }
            facts.descriptor_local = Some(descriptor_local);
            facts.create_thread = function.id.as_ref().and_then(|id| id.symbol_id.get());
            if let Expression::Identifier(callee) = &call.callee {
                facts.capture_thread = crate::analysis::identifier_symbol(callee, scoping);
            }
        }
    }
    if let Some(descriptors) = facts.descriptors {
        if let Some(Expression::ArrayExpression(array)) =
            collected.decls.get(&descriptors).and_then(|info| info.init)
        {
            for element in &array.elements {
                if let ArrayExpressionElement::ObjectExpression(object) = element {
                    facts.descriptor_records.insert(object.node_id());
                }
            }
        }
    }

    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        for statement in &body.statements {
            let Statement::ReturnStatement(ret) = statement else {
                continue;
            };
            let Some(Expression::Identifier(returned)) = &ret.argument else {
                continue;
            };
            let Some(closure) = crate::analysis::identifier_symbol(returned, scoping) else {
                continue;
            };
            let Some(closure_init) = collected.decls.get(&closure).and_then(|info| info.init)
            else {
                continue;
            };
            let Expression::FunctionExpression(closure_fn) = closure_init else {
                continue;
            };
            let Some(context_ctor) = facts.context_ctor else {
                continue;
            };
            let Some(context_local) = closure_context_local(closure_fn, context_ctor, scoping)
            else {
                continue;
            };
            facts.build_invocation = Some(symbol);
            facts.invoke_thread = Some(closure);
            facts.context_local = Some(context_local);
            facts.run = closure_return_callee(closure_fn, context_local, scoping);
        }
    }
    if let (Some(builder), Some(memory)) = (facts.build_invocation, facts.memory_ctor) {
        for node in nodes.iter() {
            let AstKind::Function(function) = node.kind() else {
                continue;
            };
            let Some(body) = function.body.as_ref() else {
                continue;
            };
            let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                continue;
            };
            if facts.create_thread == Some(symbol) {
                continue;
            }
            let mut builds_memory = false;
            let mut calls_builder = false;
            for statement in &body.statements {
                match statement {
                    Statement::VariableDeclaration(declaration) => {
                        for declarator in &declaration.declarations {
                            if let Some(Expression::NewExpression(new)) = declarator.init.as_ref() {
                                if callee_symbol(&new.callee, scoping) == Some(memory) {
                                    builds_memory = true;
                                }
                            }
                        }
                    }
                    Statement::ExpressionStatement(expression) => {
                        if let Expression::CallExpression(call) = &expression.expression {
                            if callee_symbol(&call.callee, scoping) == Some(builder) {
                                calls_builder = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if builds_memory && calls_builder {
                facts.capture_thread = Some(symbol);
            }
        }
    }
}

fn closure_context_local(
    function: &Function<'_>,
    context_ctor: SymbolId,
    scoping: &Scoping,
) -> Option<SymbolId> {
    for statement in function.body.as_ref()?.statements.iter() {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for declarator in &declaration.declarations {
            if let Some(Expression::NewExpression(new)) = declarator.init.as_ref() {
                if callee_symbol(&new.callee, scoping) == Some(context_ctor) {
                    return binding_symbol(&declarator.id);
                }
            }
        }
    }
    None
}

fn closure_return_callee(
    function: &Function<'_>,
    context_local: SymbolId,
    scoping: &Scoping,
) -> Option<SymbolId> {
    for statement in function.body.as_ref()?.statements.iter() {
        let Statement::ReturnStatement(ret) = statement else {
            continue;
        };
        let Some(Expression::CallExpression(call)) = &ret.argument else {
            continue;
        };
        let Some(Expression::Identifier(argument)) =
            call.arguments.first().and_then(Argument::as_expression)
        else {
            continue;
        };
        let symbol = crate::analysis::identifier_symbol(argument, scoping);
        if symbol == Some(context_local) {
            return callee_symbol(&call.callee, scoping);
        }
    }
    None
}

/// Record a callback's first parameter as a proven VM execution context.
///
/// Every evidence source (operation table, WASM host imports, and the
/// structural wrappers) feeds the same typed list so a context is recognized
/// even when it is never linked back through a WASM host object.
fn register_context_param(facts: &mut Facts, parameter: SymbolId) {
    if !facts.callback_context_params.contains(&parameter) {
        facts.callback_context_params.push(parameter);
    }
}

/// Prove the pending-completion register and its two sentinels from the
/// interpreter loop.
///
/// `vmRun` compares the register against one sentinel object before every
/// dispatch step, moves the register into the return value, then restores the
/// same sentinel. That comparison and the empty-object declaration identify the
/// no-result sentinel; an operation assigning its empty-object counterpart into
/// the register identifies the empty-result sentinel.
fn detect_completion_sentinels<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let run_node = facts
        .run
        .and_then(|run| thread_function_node(nodes, collected, Some(run)));
    let dispatch_key = facts.context_dispatch_key.as_deref();
    let mut candidate = None;
    for node in nodes.iter() {
        if let Some(run_node) = run_node {
            if !within_function(nodes, node.id(), run_node) {
                continue;
            }
        }
        let AstKind::IfStatement(statement) = node.kind() else {
            continue;
        };
        let Expression::BinaryExpression(binary) = &statement.test else {
            continue;
        };
        if binary.operator != BinaryOperator::StrictInequality {
            continue;
        }
        let (Some(left), Some(right)) = (
            expression_symbol(&binary.left, scoping),
            expression_symbol(&binary.right, scoping),
        ) else {
            continue;
        };
        let Statement::BlockStatement(block) = &statement.consequent else {
            continue;
        };
        // The pending register is a declared local, never a parameter or an
        // incidental binding: a helper parameter reassignment is the classic
        // false positive (`if (cursor !== start) cursor = start;`).
        if !collected.decls.contains_key(&left) {
            continue;
        }
        // Exact completion handoff: the block restores the sentinel and returns
        // the pending value moved out of the register. A block that merely
        // returns some other expression is not the completion path.
        if !completion_block_flow(block, left, right, scoping) {
            continue;
        }
        let Some(function) = enclosing_function(nodes, node.id()) else {
            continue;
        };
        let function_node = function.node_id.get();
        if run_node.is_none() {
            // Without a proven invocation closure, the enclosing function must
            // itself prove an interpreter loop: either it drives the proven
            // dispatch method, or it has the structural shape of an unbounded
            // loop that calls a method on its own context parameter. An
            // unrelated reset helper matches neither.
            let proven_dispatch = dispatch_key
                .is_some_and(|key| function_dispatches(nodes, function_node, key, scoping));
            if !proven_dispatch
                && !run_loop_evidence(nodes, scoping, collected, function_node, node.id())
            {
                continue;
            }
        }
        // Validate the sentinel before any fact is mutated. Only a proven empty
        // object marks "no pending result".
        if !is_empty_object_binding(collected, right) {
            continue;
        }
        candidate = Some((left, right, function_node));
        break;
    }
    let Some((register, sentinel, function_node)) = candidate else {
        return;
    };
    if facts.run.is_none() {
        facts.run = nodes
            .iter()
            .find_map(|node| match node.kind() {
                AstKind::Function(function) if function.node_id.get() == function_node => {
                    function.id.as_ref().and_then(|id| id.symbol_id.get())
                }
                _ => None,
            })
            .or_else(|| collected.assigned_symbol.get(&function_node).copied());
    }
    facts.completion_result = Some(register);
    facts.no_result_sentinel = Some(sentinel);
    for node in nodes.iter() {
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            continue;
        };
        if assignment_target_symbol(&assignment.left, scoping) != Some(register) {
            continue;
        }
        let Some(source) = expression_symbol(&assignment.right, scoping) else {
            continue;
        };
        if source == sentinel || !is_empty_object_binding(collected, source) {
            continue;
        }
        facts.empty_result_sentinel = Some(source);
        break;
    }
}

/// The completion block must move the pending register into the returned value
/// and restore the sentinel before returning:
///
/// ```js
/// if (pending !== none) {
///   result = pending;
///   pending = none;
///   return result;
/// }
/// ```
fn completion_block_flow(
    block: &BlockStatement<'_>,
    register: SymbolId,
    sentinel: SymbolId,
    scoping: &Scoping,
) -> bool {
    let mut moved: BTreeSet<SymbolId> = BTreeSet::new();
    let mut restores = false;
    for inner in &block.body {
        match inner {
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    continue;
                };
                let target = assignment_target_symbol(&assignment.left, scoping);
                let source = expression_symbol(&assignment.right, scoping);
                if target == Some(register) && source == Some(sentinel) {
                    restores = true;
                    continue;
                }
                if let Some(local) = target {
                    if source == Some(register) {
                        moved.insert(local);
                    } else {
                        // A later reassignment invalidates the moved value.
                        moved.remove(&local);
                    }
                }
            }
            Statement::ReturnStatement(ret) => {
                let returned = ret
                    .argument
                    .as_ref()
                    .and_then(|argument| expression_symbol(argument, scoping));
                return restores
                    && matches!(returned, Some(symbol) if symbol == register || moved.contains(&symbol));
            }
            _ => {}
        }
    }
    false
}

/// Does the function contain a call through the proven dispatch method?
fn function_dispatches(
    nodes: &AstNodes<'_>,
    function: NodeId,
    dispatch_key: &str,
    scoping: &Scoping,
) -> bool {
    nodes.iter().any(|node| {
        if !within_function(nodes, node.id(), function) {
            return false;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            return false;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return false;
        };
        member.property.name == dispatch_key && expression_symbol(&member.object, scoping).is_some()
    })
}

/// Structural run-loop evidence used when the invocation closure (and therefore
/// the dispatch method) was not proven. The completion test must sit inside an
/// unbounded loop that calls a method on one of the function's parameters. The
/// sentinel alone is not enough: a reset helper has no loop and no dispatch
/// call, so it cannot seed `facts.run`.
fn run_loop_evidence(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    function: NodeId,
    sentinel_node: NodeId,
) -> bool {
    if !nodes
        .ancestors(sentinel_node)
        .any(|ancestor| is_unbounded_loop(&ancestor.kind()))
    {
        return false;
    }
    let Some(info) = collected.functions.get(&function) else {
        return false;
    };
    let params: BTreeSet<SymbolId> = info.params.iter().copied().collect();
    if params.is_empty() {
        return false;
    }
    nodes.iter().any(|node| {
        if !within_function(nodes, node.id(), function)
            || !nodes
                .ancestors(node.id())
                .any(|ancestor| is_unbounded_loop(&ancestor.kind()))
        {
            return false;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            return false;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return false;
        };
        expression_symbol(&member.object, scoping)
            .is_some_and(|receiver| params.contains(&receiver))
    })
}

fn is_unbounded_loop(kind: &AstKind<'_>) -> bool {
    match kind {
        AstKind::ForStatement(statement) => statement.test.is_none(),
        AstKind::WhileStatement(statement) => is_true_literal(&statement.test),
        AstKind::DoWhileStatement(statement) => is_true_literal(&statement.test),
        _ => false,
    }
}

fn is_true_literal(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::BooleanLiteral(literal) if literal.value)
}

fn is_empty_object_binding(collected: &Collected<'_>, symbol: SymbolId) -> bool {
    matches!(
        collected.decls.get(&symbol).and_then(|info| info.init),
        Some(Expression::ObjectExpression(object)) if object.properties.is_empty()
    )
}

fn detect_guarded<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let Some(operations) = facts.operations else {
        return;
    };
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let symbol = function
            .id
            .as_ref()
            .and_then(|id| id.symbol_id.get())
            .or_else(|| {
                collected
                    .assigned_symbol
                    .get(&function.node_id.get())
                    .copied()
            });
        let Some(symbol) = symbol else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let params: BTreeSet<SymbolId> = function
            .params
            .items
            .iter()
            .filter_map(|parameter| binding_symbol(&parameter.pattern))
            .collect();
        for statement in &body.statements {
            let Statement::TryStatement(try_statement) = statement else {
                continue;
            };
            for inner in &try_statement.block.body {
                let Statement::ExpressionStatement(expression) = inner else {
                    continue;
                };
                let Expression::CallExpression(call) = &expression.expression else {
                    continue;
                };
                let context_argument = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|expression| expression_symbol(expression, scoping));
                if let Expression::ComputedMemberExpression(member) = &call.callee {
                    if expression_symbol(&member.object, scoping) != Some(operations) {
                        continue;
                    }
                    facts.guarded = Some(symbol);
                    if let Some(context) = context_argument {
                        register_context_param(facts, context);
                    }
                    continue;
                }
                // The JavaScript VM path forwards an operation callable and the context into a
                // separate guarded wrapper: `try { callable(context) }`.
                let Some(callable) = expression_symbol(&call.callee, scoping) else {
                    continue;
                };
                if !params.contains(&callable) {
                    continue;
                }
                facts.guarded = Some(symbol);
                facts.guarded_operation_param = Some(callable);
                if let Some(context) = context_argument.filter(|context| params.contains(context)) {
                    facts.guarded_context_param = Some(context);
                    register_context_param(facts, context);
                }
            }
            let Some(handler) = try_statement.handler.as_ref() else {
                continue;
            };
            let catch_binding = handler
                .param
                .as_ref()
                .and_then(|param| binding_symbol(&param.pattern));
            if let Some(error) = catch_binding {
                facts.guarded_error = Some(error);
            }
            for inner in &handler.body.body {
                let Statement::ExpressionStatement(expression) = inner else {
                    continue;
                };
                let Expression::CallExpression(call) = &expression.expression else {
                    continue;
                };
                let Some(callee) = expression_symbol(&call.callee, scoping) else {
                    continue;
                };
                if params.contains(&callee) {
                    continue;
                }
                let handler_params = collected
                    .functions
                    .values()
                    .find(|info| info.symbol == Some(callee))
                    .map(|info| info.params.clone())
                    .unwrap_or_default();
                facts.handle_exception = Some(callee);
                facts.exception_error_param = handler_params.first().copied();
                for (position, argument) in call.arguments.iter().enumerate() {
                    let Some(expression) = argument.as_expression() else {
                        continue;
                    };
                    let Some(argument) = expression_symbol(expression, scoping) else {
                        continue;
                    };
                    if params.contains(&argument) && Some(argument) != catch_binding {
                        if let Some(parameter) = handler_params.get(position) {
                            facts.exception_context_param = Some(*parameter);
                        }
                        facts.exception_context_param =
                            facts.exception_context_param.or(Some(argument));
                        register_context_param(facts, argument);
                    }
                }
                if let Some(context) = facts.exception_context_param {
                    register_context_param(facts, context);
                }
            }
        }
    }
}

fn detect_wasm_imports<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    for node in nodes.iter() {
        let AstKind::NewExpression(new) = node.kind() else {
            continue;
        };
        if !is_webassembly_instance(&new.callee, scoping) {
            continue;
        }
        let Some(imports_expression) = new.arguments.get(1).and_then(Argument::as_expression)
        else {
            continue;
        };
        let Some(Expression::ObjectExpression(imports)) =
            resolved_object_expression(imports_expression, collected, scoping)
        else {
            continue;
        };
        for property in &imports.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let module = match &property.value {
                Expression::ObjectExpression(object) => Some(object),
                Expression::Identifier(id) => crate::analysis::identifier_symbol(id, scoping)
                    .and_then(|symbol| collected.decls.get(&symbol))
                    .and_then(|info| info.init)
                    .and_then(|init| match init {
                        Expression::ObjectExpression(object) => Some(object),
                        _ => None,
                    }),
                _ => None,
            };
            let Some(module) = module else {
                continue;
            };
            for entry in &module.properties {
                let ObjectPropertyKind::ObjectProperty(entry) = entry else {
                    continue;
                };
                let function = match &entry.value {
                    Expression::FunctionExpression(function) => Some(&**function),
                    Expression::ArrowFunctionExpression(_) => None,
                    Expression::Identifier(id) => crate::analysis::identifier_symbol(id, scoping)
                        .and_then(|symbol| collected.decls.get(&symbol))
                        .and_then(|info| info.init)
                        .and_then(|init| match init {
                            Expression::FunctionExpression(function) => Some(&**function),
                            _ => None,
                        }),
                    _ => None,
                };
                if let Some(function) = function {
                    facts.callback_param_groups.push(
                        function
                            .params
                            .items
                            .iter()
                            .filter_map(|parameter| binding_symbol(&parameter.pattern))
                            .collect(),
                    );
                    if let Some(parameter) = function
                        .params
                        .items
                        .first()
                        .and_then(|parameter| binding_symbol(&parameter.pattern))
                    {
                        register_context_param(facts, parameter);
                    }
                }
            }
        }
    }
}

/// Detect ordinary JavaScript wrappers around typed VM values.
///
/// These helpers are intentionally proved from the surrounding VM layout and
/// function dataflow. In particular, the source property spellings (`Ti`,
/// `TK`, etc.) are never used as identity signals; the concurrent layout pass
/// owns their eventual `pop`/`push` names.
fn detect_wrappers<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let Some(stack_field) = facts.context_stack_fields.first().cloned() else {
        return;
    };
    let stack_pop_key = facts
        .stack_members
        .iter()
        .find_map(|(key, value)| (value == "pop").then_some(key.clone()));
    let stack_pop_key = stack_pop_key.as_deref();

    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let Some(context) = function
            .params
            .items
            .first()
            .and_then(|parameter| binding_symbol(&parameter.pattern))
        else {
            continue;
        };
        let Some(symbol) = function
            .id
            .as_ref()
            .and_then(|id| id.symbol_id.get())
            .or_else(|| {
                collected
                    .assigned_symbol
                    .get(&function.node_id.get())
                    .copied()
            })
        else {
            continue;
        };

        // `function(context) { return context.<stack field>.<pop>(); }`.
        // The field and method must both be proven by the VM constructors.
        let returns_stack_pop = body.statements.iter().any(|statement| {
            let Statement::ReturnStatement(return_statement) = statement else {
                return false;
            };
            let Some(Expression::CallExpression(call)) = return_statement.argument.as_ref() else {
                return false;
            };
            if !call.arguments.is_empty() {
                return false;
            }
            let Expression::StaticMemberExpression(method) = &call.callee else {
                return false;
            };
            let Some(stack_pop_key) = stack_pop_key else {
                return false;
            };
            if method.property.name.as_str() != stack_pop_key {
                return false;
            }
            let Expression::StaticMemberExpression(stack) = &method.object else {
                return false;
            };
            if stack.property.name.as_str() != stack_field.as_str() {
                return false;
            }
            matches!(&stack.object, Expression::Identifier(identifier)
                if reference_symbol(identifier, scoping) == Some(context))
        });
        if returns_stack_pop {
            facts.pop_stack = Some(symbol);
            register_context_param(facts, context);
        }

        // The interpreter callback stores its return code in a local and uses
        // that same binding as a switch discriminant. This is enough evidence
        // for `executionStatus`, without guessing at arbitrary local names.
        if facts.run == Some(symbol) {
            for statement in &body.statements {
                let Statement::VariableDeclaration(declaration) = statement else {
                    continue;
                };
                for declarator in &declaration.declarations {
                    let Some(local) = binding_symbol(&declarator.id) else {
                        continue;
                    };
                    let Some(init) = declarator.init.as_ref() else {
                        continue;
                    };
                    if !matches!(init, Expression::CallExpression(_)) {
                        continue;
                    }
                    if body.statements.iter().any(|candidate| {
                        let Statement::SwitchStatement(switch) = candidate else {
                            return false;
                        };
                        reference_symbol_in_expression(&switch.discriminant, scoping) == Some(local)
                    }) {
                        facts.run_status_locals.insert(local);
                    }
                }
            }
        }
    }
}

/// Prove the worker bootstrap, its cached instance, callback array, and the
/// inner source/blob locals from the exact bootstrap dataflow.
///
/// Identity comes from the structure and the resolved [`SymbolId`]s only: the
/// `onmessage` handler that indexes a proven callback array with the event
/// payload, the surrounding function that stitches the entry source through a
/// typed Blob into a Worker behind a `false`/`null` cache, and the catch that
/// pins the failure state. Constructor spellings are accepted when they resolve
/// to the native `Blob`/`Worker`/`URL.createObjectURL`, directly or through a
/// guarded alias. The wire property names `a`/`b` are never used as identity and
/// are left untouched unless a producer link proves them, so the WASM ABI keys
/// stay unchanged.
fn detect_worker<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    let Some(entry) = facts.entry else {
        return;
    };
    for node in nodes.iter() {
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            continue;
        };
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            continue;
        };
        if member.property.name.as_str() != "onmessage" {
            continue;
        }
        let Some(worker) = expression_symbol(&member.object, scoping) else {
            continue;
        };
        let Expression::FunctionExpression(handler) = &assignment.right else {
            continue;
        };
        let Some(message_event) = handler
            .params
            .items
            .first()
            .and_then(|parameter| binding_symbol(&parameter.pattern))
        else {
            continue;
        };
        let Some(body) = handler.body.as_ref() else {
            continue;
        };
        let Some((callbacks, _, _)) = worker_callback_invocation(body, message_event, scoping)
        else {
            continue;
        };
        // The callback container must be a proven array binding.
        if !matches!(
            collected.decls.get(&callbacks).and_then(|info| info.init),
            Some(Expression::ArrayExpression(_))
        ) {
            continue;
        }
        let Some(bootstrap) = enclosing_function(nodes, node.id()) else {
            continue;
        };
        let Some(initialize_worker) = bootstrap
            .id
            .as_ref()
            .and_then(|id| id.symbol_id.get())
            .or_else(|| {
                collected
                    .assigned_symbol
                    .get(&bootstrap.node_id.get())
                    .copied()
            })
        else {
            continue;
        };
        if Some(initialize_worker) == Some(entry) {
            continue;
        }
        let Some(bootstrap_body) = bootstrap.body.as_ref() else {
            continue;
        };
        // `if (worker === false) return false;`
        let has_false_guard = bootstrap_body.statements.iter().any(|statement| {
            let Statement::IfStatement(if_statement) = statement else {
                return false;
            };
            worker_false_guard(if_statement, worker, scoping)
        });
        // `if (worker == null) { try { ... } catch (error) { ... } }`
        let mut bootstrap_try = None;
        for statement in &bootstrap_body.statements {
            let Statement::IfStatement(if_statement) = statement else {
                continue;
            };
            let Some(block) = worker_null_guard_block(if_statement, worker, scoping) else {
                continue;
            };
            for inner in &block.body {
                if let Statement::TryStatement(try_statement) = inner {
                    bootstrap_try = Some(&**try_statement);
                }
            }
        }
        // `return true;` at the end of the bootstrap.
        let has_success_return = bootstrap_body.statements.iter().any(|statement| {
            matches!(statement, Statement::ReturnStatement(ret)
                if matches!(&ret.argument, Some(Expression::BooleanLiteral(literal)) if literal.value))
        });
        if !has_false_guard || !has_success_return {
            continue;
        }
        let Some(try_statement) = bootstrap_try else {
            continue;
        };
        let Some(catch) = try_statement.handler.as_ref() else {
            continue;
        };
        let Some(error) = catch
            .param
            .as_ref()
            .and_then(|parameter| binding_symbol(&parameter.pattern))
        else {
            continue;
        };
        if !catch_sets_false_and_returns_false(&catch.body, worker, scoping) {
            continue;
        }

        // `var source = "(" + entry + ")..."` inside the try block.
        let mut source = None;
        for statement in &try_statement.block.body {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            for declarator in &declaration.declarations {
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                if expression_references_symbol(init, entry, scoping) {
                    source = binding_symbol(&declarator.id);
                }
            }
        }
        let Some(source) = source else {
            continue;
        };

        // `var blob = new Blob([source], { type: "application/javascript" })`.
        let mut blob = None;
        for statement in &try_statement.block.body {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            for declarator in &declaration.declarations {
                let Some(Expression::NewExpression(new)) = declarator.init.as_ref() else {
                    continue;
                };
                if worker_blob_source(new, source, scoping, collected).is_some() {
                    blob = binding_symbol(&declarator.id);
                }
            }
        }
        let Some(blob) = blob else {
            continue;
        };

        // `worker = new Worker(blob | URL.createObjectURL(blob))`.
        let mut creates_worker = false;
        for statement in &try_statement.block.body {
            let Statement::ExpressionStatement(expression) = statement else {
                continue;
            };
            let Expression::AssignmentExpression(inner) = &expression.expression else {
                continue;
            };
            if assignment_target_symbol(&inner.left, scoping) != Some(worker) {
                continue;
            }
            let Expression::NewExpression(new) = &inner.right else {
                continue;
            };
            if native_named_alias(&new.callee, "Worker", scoping, collected)
                && worker_reads_blob(new, blob, scoping, collected)
            {
                creates_worker = true;
            }
        }
        if !creates_worker {
            continue;
        }

        facts.initialize_worker = Some(initialize_worker);
        facts.worker = Some(worker);
        facts.worker_callbacks = Some(callbacks);
        facts.worker_source = Some(source);
        facts.worker_blob = Some(blob);
        facts.worker_message_event = Some(message_event);
        facts.worker_error = Some(error);
        return;
    }
}

/// Match `<event>.data.<key>` and return the key plus the event symbol.
fn event_data_key(expression: &Expression<'_>, scoping: &Scoping) -> Option<(String, SymbolId)> {
    let Expression::StaticMemberExpression(property) = expression else {
        return None;
    };
    let Expression::StaticMemberExpression(data) = &property.object else {
        return None;
    };
    if data.property.name.as_str() != "data" {
        return None;
    }
    let Expression::Identifier(event) = &data.object else {
        return None;
    };
    Some((
        property.property.name.to_string(),
        reference_symbol(event, scoping)?,
    ))
}

/// Match `callbacks[event.data.a](event.data.b)` and return the callback array
/// symbol plus the two payload field names, which stay untouched.
fn worker_callback_invocation(
    body: &FunctionBody<'_>,
    event: SymbolId,
    scoping: &Scoping,
) -> Option<(SymbolId, String, String)> {
    for statement in &body.statements {
        let Statement::ExpressionStatement(expression) = statement else {
            continue;
        };
        let Expression::CallExpression(call) = &expression.expression else {
            continue;
        };
        if call.arguments.len() != 1 {
            continue;
        }
        let Expression::ComputedMemberExpression(indexed) = &call.callee else {
            continue;
        };
        let Expression::Identifier(base) = &indexed.object else {
            continue;
        };
        let callbacks = reference_symbol(base, scoping)?;
        let (index_key, index_event) = event_data_key(&indexed.expression, scoping)?;
        if index_event != event {
            continue;
        }
        let Some(argument) = call.arguments[0].as_expression() else {
            continue;
        };
        let (result_key, result_event) = event_data_key(argument, scoping)?;
        if result_event != event || result_key == index_key {
            continue;
        }
        return Some((callbacks, index_key, result_key));
    }
    None
}

fn worker_false_guard(if_statement: &IfStatement<'_>, worker: SymbolId, scoping: &Scoping) -> bool {
    let Expression::BinaryExpression(binary) = &if_statement.test else {
        return false;
    };
    if binary.operator != BinaryOperator::StrictEquality
        || expression_symbol(&binary.left, scoping) != Some(worker)
        || !matches!(&binary.right, Expression::BooleanLiteral(literal) if !literal.value)
    {
        return false;
    }
    statement_returns_false(&if_statement.consequent)
}

fn worker_null_guard_block<'a>(
    if_statement: &'a IfStatement<'a>,
    worker: SymbolId,
    scoping: &Scoping,
) -> Option<&'a BlockStatement<'a>> {
    let Expression::BinaryExpression(binary) = &if_statement.test else {
        return None;
    };
    if binary.operator != BinaryOperator::Equality
        || expression_symbol(&binary.left, scoping) != Some(worker)
        || !matches!(&binary.right, Expression::NullLiteral(_))
    {
        return None;
    }
    match &if_statement.consequent {
        Statement::BlockStatement(block) => Some(block),
        _ => None,
    }
}

fn statement_returns_false(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(ret) => {
            matches!(&ret.argument, Some(Expression::BooleanLiteral(literal)) if !literal.value)
        }
        Statement::BlockStatement(block) => block.body.iter().any(statement_returns_false),
        _ => false,
    }
}

fn catch_sets_false_and_returns_false(
    block: &BlockStatement<'_>,
    worker: SymbolId,
    scoping: &Scoping,
) -> bool {
    let mut sets_false = false;
    let mut returns_false = false;
    for statement in &block.body {
        match statement {
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    continue;
                };
                if assignment_target_symbol(&assignment.left, scoping) == Some(worker)
                    && matches!(&assignment.right, Expression::BooleanLiteral(literal) if !literal.value)
                {
                    sets_false = true;
                }
            }
            Statement::ReturnStatement(ret) => {
                if matches!(&ret.argument, Some(Expression::BooleanLiteral(literal)) if !literal.value)
                {
                    returns_false = true;
                }
            }
            _ => {}
        }
    }
    sets_false && returns_false
}

fn assignment_target_symbol(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, scoping)
}

/// True when the expression tree reads the given symbol anywhere.
fn expression_references_symbol(
    expression: &Expression<'_>,
    target: SymbolId,
    scoping: &Scoping,
) -> bool {
    match expression {
        Expression::Identifier(_) => expression_symbol(expression, scoping) == Some(target),
        Expression::BinaryExpression(binary) => {
            expression_references_symbol(&binary.left, target, scoping)
                || expression_references_symbol(&binary.right, target, scoping)
        }
        Expression::LogicalExpression(logical) => {
            expression_references_symbol(&logical.left, target, scoping)
                || expression_references_symbol(&logical.right, target, scoping)
        }
        Expression::ConditionalExpression(conditional) => {
            expression_references_symbol(&conditional.consequent, target, scoping)
                || expression_references_symbol(&conditional.alternate, target, scoping)
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            expression_references_symbol(&parenthesized.expression, target, scoping)
        }
        Expression::StaticMemberExpression(member) => {
            expression_references_symbol(&member.object, target, scoping)
        }
        Expression::CallExpression(call) => {
            expression_references_symbol(&call.callee, target, scoping)
                || call.arguments.iter().any(|argument| {
                    argument.as_expression().is_some_and(|argument| {
                        expression_references_symbol(argument, target, scoping)
                    })
                })
        }
        _ => false,
    }
}

/// Resolve `expression` to the native global `name`, directly or through a
/// guarded conditional/logical alias chain. The chain is symbol-identified, so
/// an unrelated `Blob`-shaped spelling is never accepted on spelling alone.
fn native_named_alias(
    expression: &Expression<'_>,
    name: &str,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    native_named_alias_inner(expression, name, scoping, collected, &mut BTreeSet::new())
}

fn native_named_alias_inner(
    expression: &Expression<'_>,
    name: &str,
    scoping: &Scoping,
    collected: &Collected<'_>,
    visited: &mut BTreeSet<SymbolId>,
) -> bool {
    if is_unresolved_global(expression, name, scoping) {
        return true;
    }
    match expression {
        Expression::Identifier(identifier) => {
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) else {
                return false;
            };
            if !visited.insert(symbol) {
                return false;
            }
            let Some(init) = collected.decls.get(&symbol).and_then(|info| info.init) else {
                return false;
            };
            native_named_alias_inner(init, name, scoping, collected, visited)
        }
        Expression::ConditionalExpression(conditional) => {
            native_named_alias_inner(&conditional.consequent, name, scoping, collected, visited)
                || native_named_alias_inner(
                    &conditional.alternate,
                    name,
                    scoping,
                    collected,
                    visited,
                )
        }
        Expression::LogicalExpression(logical) => {
            native_named_alias_inner(&logical.left, name, scoping, collected, visited)
                || native_named_alias_inner(&logical.right, name, scoping, collected, visited)
        }
        _ => false,
    }
}

/// True when `expression` is `URL.createObjectURL` directly or through an
/// identifier/conditional alias.
fn create_object_url_alias(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    create_object_url_alias_inner(expression, scoping, collected, &mut BTreeSet::new())
}

fn create_object_url_alias_inner(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    visited: &mut BTreeSet<SymbolId>,
) -> bool {
    match expression {
        Expression::StaticMemberExpression(member) => {
            member.property.name.as_str() == "createObjectURL"
                && native_named_alias(&member.object, "URL", scoping, collected)
        }
        Expression::Identifier(identifier) => {
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) else {
                return false;
            };
            if !visited.insert(symbol) {
                return false;
            }
            let Some(init) = collected.decls.get(&symbol).and_then(|info| info.init) else {
                return false;
            };
            create_object_url_alias_inner(init, scoping, collected, visited)
        }
        Expression::ConditionalExpression(conditional) => {
            create_object_url_alias_inner(&conditional.consequent, scoping, collected, visited)
                || create_object_url_alias_inner(
                    &conditional.alternate,
                    scoping,
                    collected,
                    visited,
                )
        }
        _ => false,
    }
}

/// `new Blob([source], { type: "...javascript..." })`: returns the source
/// symbol carried by the Blob when the constructor and options prove the
/// blob is the string source of the VM entry.
fn worker_blob_source(
    new: &NewExpression<'_>,
    source: SymbolId,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> Option<SymbolId> {
    if !native_named_alias(&new.callee, "Blob", scoping, collected) {
        return None;
    }
    let first = new.arguments.first()?.as_expression()?;
    let Expression::ArrayExpression(array) = first else {
        return None;
    };
    if array.elements.len() != 1 {
        return None;
    }
    let element = array.elements[0].to_expression();
    if expression_symbol(element, scoping) != Some(source) {
        return None;
    }
    let options = new.arguments.get(1)?.as_expression()?;
    let Expression::ObjectExpression(options) = options else {
        return None;
    };
    let type_ok = options.properties.iter().any(|property| {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed || property.key_name() != "type" {
            return false;
        }
        matches!(&property.value, Expression::StringLiteral(literal)
            if literal.value.to_ascii_lowercase().contains("javascript"))
    });
    type_ok.then_some(source)
}

/// True when the worker constructor argument reads the proven Blob, directly or
/// through `URL.createObjectURL`.
fn worker_reads_blob(
    new: &NewExpression<'_>,
    blob: SymbolId,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    let Some(argument) = new.arguments.first().and_then(Argument::as_expression) else {
        return false;
    };
    if expression_symbol(argument, scoping) == Some(blob) {
        return true;
    }
    let Expression::CallExpression(call) = argument else {
        return false;
    };
    if !create_object_url_alias(&call.callee, scoping, collected) || call.arguments.len() != 1 {
        return false;
    }
    call.arguments[0]
        .as_expression()
        .is_some_and(|argument| expression_symbol(argument, scoping) == Some(blob))
}

fn detect_proto_this<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &mut Facts,
) {
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let symbol = function.id.as_ref().and_then(|id| id.symbol_id.get());
        let instance = if symbol.is_some() && symbol == facts.memory_ctor {
            Some(ObjType::Memory)
        } else if symbol.is_some() && symbol == facts.context_ctor {
            Some(ObjType::Context)
        } else {
            None
        };
        if let Some(instance) = instance {
            facts.fn_this.insert(function.node_id.get(), instance);
        }
    }
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3 {
            continue;
        }
        let Some(target) = call.arguments[0].as_expression() else {
            continue;
        };
        let instance = match proto_of(target, scoping, collected) {
            Some(ctor) if Some(ctor) == facts.memory_ctor => ObjType::Memory,
            Some(ctor) if Some(ctor) == facts.context_ctor => ObjType::Context,
            _ => continue,
        };
        if let Some(Expression::ObjectExpression(descriptor)) = call.arguments[2].as_expression() {
            if let Some(Expression::FunctionExpression(function)) = descriptor_value(descriptor) {
                facts.fn_this.insert(function.node_id.get(), instance);
            }
        }
    }
}

fn classify_context_fields<'a>(nodes: &AstNodes<'a>, facts: &mut Facts) {
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if call.arguments.len() < 3 {
            continue;
        }
        let Some(Expression::StaticMemberExpression(first)) = call.arguments[0].as_expression()
        else {
            continue;
        };
        let Some(Expression::StaticMemberExpression(second)) = call.arguments[1].as_expression()
        else {
            continue;
        };
        let Some(Expression::Identifier(third)) = call.arguments[2].as_expression() else {
            continue;
        };
        let first_object = identifier_name(&first.object);
        let second_object = identifier_name(&second.object);
        if let (Some(first_name), Some(second_name)) = (first_object, second_object) {
            if first_name == second_name
                && first_name == third.name.as_str()
                && facts.context_keys.contains(first.property.name.as_str())
                && facts.context_keys.contains(second.property.name.as_str())
            {
                facts.context_pc_field = Some(first.property.name.to_string());
                facts.context_state_field = Some(second.property.name.to_string());
            }
        }
    }
}

fn identifier_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::Identifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

fn reference_symbol_in_expression(
    expression: &Expression<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(identifier) => reference_symbol(identifier, scoping),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Decision
// ---------------------------------------------------------------------------

struct Decisions {
    locals: BTreeMap<SymbolId, (String, String)>,
    member_names: BTreeMap<NodeId, String>,
    object_keys: BTreeMap<NodeId, String>,
    string_values: BTreeMap<NodeId, String>,
    anon_names: BTreeMap<NodeId, String>,
    property_mappings: Vec<PropertyMapping>,
    strip_function_names: BTreeSet<NodeId>,
    report: Vec<Diagnostic>,
}

fn decide<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &Facts,
    types: &Types,
) -> Decisions {
    let mut decisions = Decisions {
        locals: BTreeMap::new(),
        member_names: BTreeMap::new(),
        object_keys: BTreeMap::new(),
        string_values: BTreeMap::new(),
        anon_names: BTreeMap::new(),
        property_mappings: Vec::new(),
        strip_function_names: BTreeSet::new(),
        report: Vec::new(),
    };

    // Dynamic-property barriers: an unknown computed key on a typed receiver
    // blocks renaming that type's members.
    let mut barrier: BTreeSet<ObjType> = BTreeSet::new();
    for node in nodes.iter() {
        if let AstKind::CallExpression(call) = node.kind() {
            if let Some(receiver_expression) = reflection_unknown_receiver(call, scoping, collected)
            {
                let this_type = enclosing_this(nodes, node.id(), facts);
                if let Some(receiver) =
                    receiver_type(receiver_expression, scoping, facts, types, this_type)
                {
                    barrier.insert(receiver);
                }
            }
        }
        if let AstKind::ComputedMemberExpression(member) = node.kind() {
            let this_type = enclosing_this(nodes, node.id(), facts);
            let Some(receiver) = receiver_type(&member.object, scoping, facts, types, this_type)
            else {
                continue;
            };
            let literal = matches!(
                member.expression,
                Expression::StringLiteral(_) | Expression::NumericLiteral(_)
            );
            if !literal {
                // A proven stack is an array, but `stack[key]` can still name
                // one of the shuffled method slots (`stack["Ti"]`). Only a
                // provably numeric key is element indexing (`stack[i]`,
                // `stack[length - 1]`); anything opaque keeps the barrier so
                // the definition and its uses decline together rather than
                // being renamed partially.
                if receiver != ObjType::Stack
                    || !is_numeric_index(&member.expression, scoping, collected)
                {
                    barrier.insert(receiver);
                }
            }
        }
    }
    for obj in &barrier {
        decisions.report.push(Diagnostic::warn(
            Stage::Rename,
            "DYNAMIC_PROPERTY_BARRIER",
            format!(
                "{} members retained: a dynamic computed property access prevents a complete mapping",
                obj.label()
            ),
        ));
    }

    // Slot value literals.
    if let Some(value_key) = &facts.slot_value_key {
        for node in nodes.iter() {
            if let AstKind::ObjectExpression(object) = node.kind() {
                if facts.slot_literals.contains(&object.node_id()) {
                    for property in &object.properties {
                        if let ObjectPropertyKind::ObjectProperty(property) = property {
                            if property.key_name() == *value_key {
                                decisions
                                    .object_keys
                                    .insert(property.node_id(), "value".into());
                                decisions.property_mappings.push(PropertyMapping {
                                    span: (property.span.start, property.span.end),
                                    owner: ObjType::Slot.label().into(),
                                    original_name: property.key_name(),
                                    semantic_name: "value".into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    // Descriptor record definitions.
    if !facts.descriptor_members.is_empty() {
        for node in nodes.iter() {
            if let AstKind::ObjectExpression(object) = node.kind() {
                if facts.descriptor_records.contains(&object.node_id()) {
                    for property in &object.properties {
                        if let ObjectPropertyKind::ObjectProperty(property) = property {
                            if let Some(name) = facts.descriptor_members.get(&property.key_name()) {
                                decisions
                                    .object_keys
                                    .insert(property.node_id(), name.clone());
                                decisions.property_mappings.push(PropertyMapping {
                                    span: (property.span.start, property.span.end),
                                    owner: ObjType::Descriptor.label().into(),
                                    original_name: property.key_name(),
                                    semantic_name: name.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    for object in nodes.iter() {
        let AstKind::ObjectProperty(property) = object.kind() else {
            continue;
        };
        let Some(name) = facts.frame_object_keys.get(&property.node_id()) else {
            continue;
        };
        decisions
            .object_keys
            .insert(property.node_id(), name.clone());
        decisions.property_mappings.push(PropertyMapping {
            span: (property.key.span().start, property.key.span().end),
            owner: frame_owner(name).into(),
            original_name: property.key_name(),
            semantic_name: name.clone(),
        });
    }

    for node in nodes.iter() {
        match node.kind() {
            AstKind::StaticMemberExpression(member) => {
                if let Some(name) = facts.frame_member_names.get(&node.id()) {
                    decisions.member_names.insert(node.id(), name.clone());
                    decisions.property_mappings.push(PropertyMapping {
                        span: (member.property.span.start, member.property.span.end),
                        owner: frame_owner(name).into(),
                        original_name: member.property.name.to_string(),
                        semantic_name: name.clone(),
                    });
                    continue;
                }
                let this_type = enclosing_this(nodes, node.id(), facts);
                let Some(receiver) =
                    receiver_type(&member.object, scoping, facts, types, this_type)
                else {
                    continue;
                };
                if barrier.contains(&receiver) {
                    continue;
                }
                if let Some(name) = member_mapping(receiver, &member.property.name, facts, types) {
                    decisions
                        .member_names
                        .insert(member.node_id.get(), name.clone());
                    decisions.property_mappings.push(PropertyMapping {
                        span: (member.property.span.start, member.property.span.end),
                        owner: receiver.label().into(),
                        original_name: member.property.name.to_string(),
                        semantic_name: name,
                    });
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                let Expression::StringLiteral(key) = &member.expression else {
                    continue;
                };
                let this_type = enclosing_this(nodes, node.id(), facts);
                let Some(receiver) =
                    receiver_type(&member.object, scoping, facts, types, this_type)
                else {
                    continue;
                };
                if barrier.contains(&receiver) {
                    continue;
                }
                if let Some(name) = member_mapping(receiver, &key.value, facts, types) {
                    decisions
                        .string_values
                        .insert(key.node_id.get(), name.clone());
                    decisions.property_mappings.push(PropertyMapping {
                        span: (key.span.start, key.span.end),
                        owner: receiver.label().into(),
                        original_name: key.value.to_string(),
                        semantic_name: name,
                    });
                }
            }
            AstKind::CallExpression(call) => {
                if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3
                {
                    continue;
                }
                let (Some(target), Some(Expression::StringLiteral(key))) = (
                    call.arguments[0].as_expression(),
                    call.arguments[1].as_expression(),
                ) else {
                    continue;
                };
                let receiver = if receiver_type(
                    target,
                    scoping,
                    facts,
                    types,
                    enclosing_this(nodes, node.id(), facts),
                ) == Some(ObjType::Stack)
                {
                    ObjType::Stack
                } else {
                    match proto_of(target, scoping, collected) {
                        Some(ctor) if Some(ctor) == facts.memory_ctor => ObjType::Memory,
                        Some(ctor) if Some(ctor) == facts.context_ctor => ObjType::Context,
                        _ => continue,
                    }
                };
                if barrier.contains(&receiver) {
                    continue;
                }
                if let Some(name) = member_mapping(receiver, &key.value, facts, types) {
                    decisions
                        .string_values
                        .insert(key.node_id.get(), name.clone());
                    decisions.property_mappings.push(PropertyMapping {
                        span: (key.span.start, key.span.end),
                        owner: receiver.label().into(),
                        original_name: key.value.to_string(),
                        semantic_name: name,
                    });
                }
            }
            _ => {}
        }
    }

    // Literal-key reflection on a proven stack names the same slot as a member
    // access (`Object.getOwnPropertyDescriptor(stack, "Ti")`). An unknown key is
    // already handled by the dynamic-property barrier above.
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !is_get_own_property_descriptor(&call.callee, scoping, collected) {
            continue;
        }
        let (Some(target), Some(Expression::StringLiteral(key))) = (
            call.arguments
                .first()
                .and_then(|argument| argument.as_expression()),
            call.arguments
                .get(1)
                .and_then(|argument| argument.as_expression()),
        ) else {
            continue;
        };
        let this_type = enclosing_this(nodes, node.id(), facts);
        if receiver_type(target, scoping, facts, types, this_type) != Some(ObjType::Stack) {
            continue;
        }
        if barrier.contains(&ObjType::Stack) {
            continue;
        }
        if let Some(name) = member_mapping(ObjType::Stack, &key.value, facts, types) {
            decisions
                .string_values
                .insert(key.node_id.get(), name.clone());
            decisions.property_mappings.push(PropertyMapping {
                span: (key.span.start, key.span.end),
                owner: ObjType::Stack.label().into(),
                original_name: key.value.to_string(),
                semantic_name: name,
            });
        }
    }

    // Operation entries are deliberately left anonymous. Their array position
    // is runtime metadata, not a stable function identity, and assigning a
    // semantic name changes Function.name/reflection without adding evidence.
    // A rare named function expression is stripped only when its private name
    // is not referenced by the body; recursive names are preserved and
    // diagnosed because removing them would change behavior.
    if let Some(operations) = facts.operations {
        if let Some(functions) = collected.array_functions.get(&operations) {
            for function in functions {
                let AstKind::Function(function_node) = nodes.kind(*function) else {
                    continue;
                };
                let Some(name) = function_node.id.as_ref() else {
                    continue;
                };
                let Some(symbol) = name.symbol_id.get() else {
                    continue;
                };
                if operation_references_symbol(nodes, scoping, *function, symbol) {
                    decisions.report.push(Diagnostic::warn(
                        Stage::Rename,
                        "NAMED_OPERATION_RECURSION",
                        format!("operation function node {function:?} retains its recursive name"),
                    ));
                } else {
                    decisions.strip_function_names.insert(*function);
                }
            }
        }
    }
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        if function.id.is_none()
            && !decisions.anon_names.contains_key(&node.id())
            && !is_operation_function(node.id(), facts, collected)
        {
            decisions.report.push(Diagnostic::warn(
                Stage::Rename,
                "UNRESOLVED_ANONYMOUS_FUNCTION",
                format!(
                    "anonymous function node {:?} remains unnamed because its role is unresolved",
                    node.id()
                ),
            ));
        }
    }
    decisions
}

fn is_operation_function(function: NodeId, facts: &Facts, collected: &Collected<'_>) -> bool {
    facts.operations.is_some_and(|operations| {
        collected
            .array_functions
            .get(&operations)
            .is_some_and(|functions| functions.contains(&function))
    })
}

/// The closest enclosing function of `node`, if any.
fn nearest_enclosing_function(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    nodes
        .ancestors(node)
        .find_map(|ancestor| match ancestor.kind() {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => Some(ancestor.id()),
            _ => None,
        })
}

/// Every `var`/`let`/`const` declarator that belongs directly to `function`,
/// ordered by source position. Built in one pass so callers do not rescan the
/// whole node table per function (or per declarator).
fn function_declarations(nodes: &AstNodes<'_>) -> BTreeMap<NodeId, Vec<(u32, SymbolId)>> {
    let mut by_function: BTreeMap<NodeId, Vec<(u32, SymbolId)>> = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(declarator) = node.kind() else {
            continue;
        };
        let Some(local) = binding_symbol(&declarator.id) else {
            continue;
        };
        if let Some(function) = nearest_enclosing_function(nodes, node.id()) {
            by_function
                .entry(function)
                .or_default()
                .push((declarator.span.start, local));
        }
    }
    for declarations in by_function.values_mut() {
        declarations.sort_by_key(|(start, _)| *start);
    }
    by_function
}

/// Ordinal (`value1`, `value2`, ...) names for the declarators of one function,
/// keyed by symbol. The first declaration position of a symbol fixes its
/// ordinal; repeated `var` declarations of the same symbol therefore reuse one
/// ordinal while distinct bindings receive distinct ones.
fn operation_local_ordinals(declarations: &[(u32, SymbolId)]) -> BTreeMap<SymbolId, usize> {
    let mut ordinals = BTreeMap::new();
    let mut next = 1usize;
    for (_, symbol) in declarations {
        if let std::collections::btree_map::Entry::Vacant(entry) = ordinals.entry(*symbol) {
            entry.insert(next);
            next += 1;
        }
    }
    ordinals
}

fn reference_symbol(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    crate::analysis::identifier_symbol(identifier, scoping)
}

fn reflection_unknown_receiver<'a>(
    call: &'a CallExpression<'a>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return None;
    };
    if member.property.name != "getOwnPropertyDescriptor"
        || !is_object_reference(&member.object, scoping, collected)
    {
        return None;
    }
    let key = call.arguments.get(1)?.as_expression()?;
    if matches!(
        key,
        Expression::StringLiteral(_) | Expression::NumericLiteral(_)
    ) {
        return None;
    }
    call.arguments.first()?.as_expression()
}

/// Whether an expression is provably a numeric array index.
///
/// A proven stack is an array, so a computed access is element indexing only
/// when its key is a number: a numeric literal, a `.length` read, a local whose
/// initializer is one of those (`var height = stack.length`), unary `+`/`-`
/// over one, or arithmetic combining them. A bare parameter, a function call,
/// or any string-producing expression could instead name a hidden method slot,
/// so it is not numeric and must keep the dynamic-property barrier.
fn is_numeric_index(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    is_numeric_index_inner(expression, scoping, collected, &mut BTreeSet::new())
}

fn is_numeric_index_inner(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    visited: &mut BTreeSet<SymbolId>,
) -> bool {
    match expression {
        Expression::NumericLiteral(_) => true,
        Expression::StaticMemberExpression(member) => member.property.name == "length",
        Expression::ParenthesizedExpression(paren) => {
            is_numeric_index_inner(&paren.expression, scoping, collected, visited)
        }
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
            ) && is_numeric_index_inner(&unary.argument, scoping, collected, visited)
        }
        Expression::Identifier(identifier) => {
            // A local initialized from `.length` or numeric arithmetic is a
            // proven index (`var height = stack.length; stack[height] = ...`).
            // The visited set breaks `var a = b; var b = a` cycles.
            let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) else {
                return false;
            };
            if !visited.insert(symbol) {
                return false;
            }
            let result = collected
                .decls
                .get(&symbol)
                .and_then(|info| info.init)
                .is_some_and(|init| is_numeric_index_inner(init, scoping, collected, visited));
            visited.remove(&symbol);
            result
        }
        Expression::BinaryExpression(binary) => {
            if !is_arithmetic_operator(binary.operator) {
                return false;
            }
            // A string operand would produce a string key (`"T" + "i"`), so it
            // disqualifies the whole expression even next to a number.
            if is_string_literal(&binary.left) || is_string_literal(&binary.right) {
                return false;
            }
            let left = is_numeric_index_inner(&binary.left, scoping, collected, visited);
            let right = is_numeric_index_inner(&binary.right, scoping, collected, visited);
            // `+` also concatenates, so both sides must be numeric; every other
            // arithmetic operator ignores a non-numeric operand rather than
            // turning it into a property name.
            if binary.operator == BinaryOperator::Addition {
                left && right
            } else {
                left || right
            }
        }
        _ => false,
    }
}

fn is_arithmetic_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Addition
            | BinaryOperator::Subtraction
            | BinaryOperator::Multiplication
            | BinaryOperator::Division
            | BinaryOperator::Remainder
            | BinaryOperator::Exponential
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::ShiftRightZeroFill
            | BinaryOperator::BitwiseOR
            | BinaryOperator::BitwiseXOR
            | BinaryOperator::BitwiseAnd
    )
}

fn is_string_literal(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::StringLiteral(_) => true,
        Expression::ParenthesizedExpression(paren) => is_string_literal(&paren.expression),
        _ => false,
    }
}

fn frame_owner(name: &str) -> &'static str {
    match name {
        "returnByteOffset" | "returnDispatchState" => "vmCallFrame",
        "handlerByteOffset" | "handlerDispatchState" | "errorEntryCount" => "vmTryFrame",
        "wasExceptionHandled" | "savedByteOffset" | "savedDispatchState" => "vmExceptionTracker",
        _ => "vmFrame",
    }
}

fn member_mapping(receiver: ObjType, key: &str, facts: &Facts, types: &Types) -> Option<String> {
    match receiver {
        ObjType::Memory => facts.memory_members.get(key).cloned(),
        ObjType::Stack => facts.stack_members.get(key).cloned(),
        ObjType::Descriptor => facts.descriptor_members.get(key).cloned(),
        ObjType::Context => context_member_mapping(key, facts, types),
        ObjType::Slot => (facts.slot_value_key.as_deref() == Some(key)).then(|| "value".into()),
    }
}

fn context_member_mapping(key: &str, facts: &Facts, types: &Types) -> Option<String> {
    if let Some(name) = facts.context_member_names.get(key) {
        return Some(name.clone());
    }
    if facts.context_pc_field.as_deref() == Some(key) {
        return Some("byteOffset".into());
    }
    if facts.context_state_field.as_deref() == Some(key) {
        return Some("dispatchState".into());
    }
    if facts.context_stack_fields.first().map(String::as_str) == Some(key) {
        return Some("stack".into());
    }
    if facts.context_stack_fields.get(1).map(String::as_str) == Some(key) {
        return Some("errorStack".into());
    }
    for (field, rhs) in &facts.context_field_rhs {
        if field != key {
            continue;
        }
        match rhs {
            FieldRhs::Conditional => return Some("receiverObject".into()),
            FieldRhs::Param(symbol) => {
                if types.symbols.get(symbol) == Some(&ObjType::Memory) {
                    return Some("memory".into());
                }
                if facts.context_params.last() == Some(symbol) {
                    return Some("receiverValue".into());
                }
            }
            _ => {}
        }
    }
    None
}

fn enclosing_this(nodes: &AstNodes<'_>, node: NodeId, facts: &Facts) -> Option<ObjType> {
    for ancestor in nodes.ancestors(node) {
        if let AstKind::Function(function) = ancestor.kind() {
            return facts.fn_this.get(&function.node_id.get()).copied();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Type inference
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Types {
    symbols: BTreeMap<SymbolId, ObjType>,
    fields: BTreeMap<(ObjType, String), ObjType>,
}

fn infer_types<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    facts: &Facts,
    collected: &Collected<'a>,
) -> Types {
    let mut types = Types::default();
    for _ in 0..12 {
        let mut symbols: BTreeMap<SymbolId, BTreeSet<ObjType>> = BTreeMap::new();
        let mut fields: BTreeMap<(ObjType, String), BTreeSet<ObjType>> = BTreeMap::new();
        let returns = function_returns(nodes, scoping, facts, &types);

        for parameter in &facts.callback_context_params {
            symbols
                .entry(*parameter)
                .or_default()
                .insert(ObjType::Context);
        }
        // The stack factory's returned local is an array tagged with the native
        // stack methods. Type it directly so the `defineProperty` call that
        // installs those methods resolves its target to `ObjType::Stack`.
        if let Some(stack) = facts.stack_local {
            symbols.entry(stack).or_default().insert(ObjType::Stack);
        }
        if let Some(run) = facts.run {
            for (node, info) in &collected.functions {
                if info.symbol == Some(run) || collected.assigned_symbol.get(node) == Some(&run) {
                    if let Some(parameter) = info.params.first() {
                        symbols
                            .entry(*parameter)
                            .or_default()
                            .insert(ObjType::Context);
                    }
                }
            }
        }
        for (symbol, info) in &collected.decls {
            if let Some(init) = info.init {
                if let Some(obj) = type_of(init, scoping, facts, &types, &returns) {
                    symbols.entry(*symbol).or_default().insert(obj);
                }
            }
        }
        for node in nodes.iter() {
            let AstKind::AssignmentExpression(assign) = node.kind() else {
                continue;
            };
            match &assign.left {
                AssignmentTarget::AssignmentTargetIdentifier(id) => {
                    if let Some(symbol) = crate::analysis::identifier_symbol(id, scoping) {
                        if let Some(obj) = type_of(&assign.right, scoping, facts, &types, &returns)
                        {
                            symbols.entry(symbol).or_default().insert(obj);
                        }
                    }
                }
                AssignmentTarget::StaticMemberExpression(member)
                    if matches!(member.object, Expression::ThisExpression(_))
                        && facts.context_keys.contains(member.property.name.as_str()) =>
                {
                    if let Some(obj) = type_of(&assign.right, scoping, facts, &types, &returns) {
                        fields
                            .entry((ObjType::Context, member.property.name.to_string()))
                            .or_default()
                            .insert(obj);
                    }
                }
                _ => {}
            }
        }
        for node in nodes.iter() {
            let (callee, arguments) = match node.kind() {
                AstKind::CallExpression(call) => (&call.callee, &call.arguments),
                AstKind::NewExpression(new) => (&new.callee, &new.arguments),
                _ => continue,
            };
            let Some(target) = callee_symbol(callee, scoping) else {
                continue;
            };
            let Some(info) = collected
                .functions
                .values()
                .find(|info| info.symbol == Some(target))
            else {
                continue;
            };
            for (parameter, argument) in info.params.iter().zip(arguments.iter()) {
                if let Some(expression) = argument.as_expression() {
                    if let Some(obj) = type_of(expression, scoping, facts, &types, &returns) {
                        symbols.entry(*parameter).or_default().insert(obj);
                    }
                }
            }
        }

        let mut next = Types::default();
        for (symbol, set) in symbols {
            if set.len() == 1 {
                next.symbols.insert(symbol, *set.iter().next().unwrap());
            }
        }
        for (key, set) in fields {
            if set.len() == 1 {
                next.fields.insert(key, *set.iter().next().unwrap());
            }
        }
        if next.symbols == types.symbols && next.fields == types.fields {
            return next;
        }
        types = next;
    }
    types
}

fn function_returns<'a>(
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    facts: &Facts,
    types: &Types,
) -> BTreeMap<SymbolId, ObjType> {
    let empty = BTreeMap::new();
    let mut out: BTreeMap<SymbolId, BTreeSet<ObjType>> = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        if let Some(body) = function.body.as_ref() {
            for statement in &body.statements {
                let Statement::ReturnStatement(ret) = statement else {
                    continue;
                };
                let Some(argument) = &ret.argument else {
                    continue;
                };
                if let Some(obj) = type_of(argument, scoping, facts, types, &empty) {
                    out.entry(symbol).or_default().insert(obj);
                }
            }
        }
    }
    out.into_iter()
        .filter_map(|(symbol, set)| (set.len() == 1).then(|| (symbol, *set.iter().next().unwrap())))
        .collect()
}

fn type_of(
    expression: &Expression<'_>,
    scoping: &Scoping,
    facts: &Facts,
    types: &Types,
    returns: &BTreeMap<SymbolId, ObjType>,
) -> Option<ObjType> {
    match expression {
        Expression::Identifier(id) => {
            let symbol = crate::analysis::identifier_symbol(id, scoping)?;
            types.symbols.get(&symbol).copied()
        }
        Expression::NewExpression(new) => {
            let target = callee_symbol(&new.callee, scoping)?;
            if Some(target) == facts.memory_ctor || facts.memory_aliases.contains(&target) {
                Some(ObjType::Memory)
            } else if Some(target) == facts.context_ctor {
                Some(ObjType::Context)
            } else {
                None
            }
        }
        Expression::CallExpression(call) => {
            if let Some(target) = callee_symbol(&call.callee, scoping) {
                if Some(target) == facts.stack_factory {
                    return Some(ObjType::Stack);
                }
                return returns.get(&target).copied();
            }
            if let Expression::StaticMemberExpression(member) = &call.callee {
                if matches!(
                    type_of(&member.object, scoping, facts, types, returns),
                    Some(ObjType::Memory)
                ) && is_clone_member(&member.property.name, facts)
                {
                    return Some(ObjType::Memory);
                }
            }
            None
        }
        Expression::StaticMemberExpression(member) => {
            let object = type_of(&member.object, scoping, facts, types, returns)?;
            types
                .fields
                .get(&(object, member.property.name.to_string()))
                .copied()
        }
        Expression::ComputedMemberExpression(member) => {
            let base = match &member.object {
                Expression::Identifier(id) => crate::analysis::identifier_symbol(id, scoping),
                _ => None,
            };
            if base == facts.descriptors {
                return Some(ObjType::Descriptor);
            }
            if let Expression::StaticMemberExpression(slots) = &member.object {
                if facts.slots_key.as_deref() == Some(slots.property.name.as_str())
                    && type_of(&slots.object, scoping, facts, types, returns)
                        == Some(ObjType::Memory)
                {
                    return Some(ObjType::Slot);
                }
            }
            None
        }
        Expression::SequenceExpression(sequence) => sequence
            .expressions
            .last()
            .and_then(|expression| type_of(expression, scoping, facts, types, returns)),
        Expression::ConditionalExpression(conditional) => join(
            type_of(&conditional.consequent, scoping, facts, types, returns),
            type_of(&conditional.alternate, scoping, facts, types, returns),
        ),
        Expression::LogicalExpression(logical) => join(
            type_of(&logical.left, scoping, facts, types, returns),
            type_of(&logical.right, scoping, facts, types, returns),
        ),
        Expression::AssignmentExpression(assign) => {
            type_of(&assign.right, scoping, facts, types, returns)
        }
        _ => None,
    }
}

fn join(a: Option<ObjType>, b: Option<ObjType>) -> Option<ObjType> {
    match (a, b) {
        (Some(a), Some(b)) if a == b => Some(a),
        _ => None,
    }
}

fn is_clone_member(name: &str, facts: &Facts) -> bool {
    facts
        .memory_members
        .get(name)
        .is_some_and(|member| member == "cloneMemory")
}

fn receiver_type(
    expression: &Expression<'_>,
    scoping: &Scoping,
    facts: &Facts,
    types: &Types,
    this_type: Option<ObjType>,
) -> Option<ObjType> {
    if matches!(expression, Expression::ThisExpression(_)) {
        return this_type;
    }
    if let Expression::ComputedMemberExpression(member) = expression {
        if let Expression::StaticMemberExpression(slots) = &member.object {
            if facts.slots_key.as_deref() == Some(slots.property.name.as_str())
                && receiver_type(&slots.object, scoping, facts, types, this_type)
                    == Some(ObjType::Memory)
            {
                return Some(ObjType::Slot);
            }
        }
    }
    let empty = BTreeMap::new();
    type_of(expression, scoping, facts, types, &empty)
}

/// Name the small set of locals belonging to the proven thread-helper family.
///
/// This is intentionally a role-specific pass over the already collected AST
/// index.  It does not classify arbitrary locals or infer names from source
/// spellings.  In particular, a loop counter is accepted only when it is part
/// of the proven capture/invocation shape, and a parameter is accepted only
/// when a slot access, constructor field, or helper call establishes its role.
fn thread_local_names(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    facts: &Facts,
) -> BTreeMap<SymbolId, (String, String)> {
    let context_names = thread_context_parameter_names(nodes, scoping, collected, facts);
    let build_names = thread_build_names(nodes, scoping, collected, facts, &context_names);
    let invoke_names = thread_invoke_names(nodes, scoping, collected, facts);
    let capture_names = thread_capture_names(nodes, scoping, collected, facts, &build_names);

    let mut names = context_names;
    names.extend(build_names);
    names.extend(invoke_names);
    names.extend(capture_names);
    names
}

fn thread_function_node(
    nodes: &AstNodes<'_>,
    collected: &Collected<'_>,
    symbol: Option<SymbolId>,
) -> Option<NodeId> {
    let symbol = symbol?;
    nodes.iter().find_map(|node| match node.kind() {
        AstKind::Function(function)
            if function.id.as_ref().and_then(|id| id.symbol_id.get()) == Some(symbol)
                || collected.assigned_symbol.get(&function.node_id.get()) == Some(&symbol) =>
        {
            Some(function.node_id.get())
        }
        AstKind::ArrowFunctionExpression(function)
            if collected.assigned_symbol.get(&function.node_id.get()) == Some(&symbol) =>
        {
            Some(function.node_id.get())
        }
        _ => None,
    })
}

fn thread_function_params(
    nodes: &AstNodes<'_>,
    collected: &Collected<'_>,
    symbol: Option<SymbolId>,
) -> Vec<SymbolId> {
    let Some(function) = thread_function_node(nodes, collected, symbol) else {
        return Vec::new();
    };
    collected
        .functions
        .get(&function)
        .map(|info| info.params.clone())
        .unwrap_or_default()
}

fn within_function(nodes: &AstNodes<'_>, node: NodeId, function: NodeId) -> bool {
    nodes
        .ancestors(node)
        .any(|ancestor| ancestor.id() == function)
}

fn thread_name(name: &str, evidence: &'static str) -> (String, String) {
    (name.into(), evidence.into())
}

fn static_member_receiver(
    expression: &Expression<'_>,
    property: &str,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    (member.property.name == property).then(|| expression_symbol(&member.object, scoping))?
}

fn thread_length_receiver(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    static_member_receiver(expression, "length", scoping)
}

fn sequence_assignments<'a>(
    expression: &'a Expression<'a>,
    scoping: &Scoping,
) -> Vec<(SymbolId, &'a Expression<'a>)> {
    let expressions = match expression {
        Expression::SequenceExpression(sequence) => sequence.expressions.iter().collect(),
        _ => vec![expression],
    };
    expressions
        .into_iter()
        .filter_map(|expression| match expression {
            Expression::AssignmentExpression(assignment) => {
                assignment_target_symbol(&assignment.left, scoping)
                    .map(|symbol| (symbol, &assignment.right))
            }
            _ => None,
        })
        .collect()
}

fn memory_method_is(expression: &Expression<'_>, facts: &Facts, expected: &str) -> bool {
    let Expression::StaticMemberExpression(member) = expression else {
        return false;
    };
    facts
        .memory_members
        .get(member.property.name.as_str())
        .is_some_and(|name| name == expected)
}

fn void_comparison_param(
    expression: &Expression<'_>,
    params: &BTreeSet<SymbolId>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
    ) {
        return None;
    }
    let is_void = |expression: &Expression<'_>| matches!(expression, Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void);
    let candidate = if is_void(&binary.left) {
        expression_symbol(&binary.right, scoping)
    } else if is_void(&binary.right) {
        expression_symbol(&binary.left, scoping)
    } else {
        None
    }?;
    params.contains(&candidate).then_some(candidate)
}

fn collect_thread_expression_symbols(
    expression: &Expression<'_>,
    scoping: &Scoping,
    symbols: &mut BTreeSet<SymbolId>,
) {
    match expression {
        Expression::Identifier(identifier) => {
            if let Some(symbol) = reference_symbol(identifier, scoping) {
                symbols.insert(symbol);
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                if let ArrayExpressionElement::SpreadElement(spread) = element {
                    collect_thread_expression_symbols(&spread.argument, scoping, symbols);
                }
            }
        }
        Expression::BinaryExpression(binary) => {
            collect_thread_expression_symbols(&binary.left, scoping, symbols);
            collect_thread_expression_symbols(&binary.right, scoping, symbols);
        }
        Expression::UnaryExpression(unary) => {
            collect_thread_expression_symbols(&unary.argument, scoping, symbols);
        }
        Expression::ConditionalExpression(conditional) => {
            collect_thread_expression_symbols(&conditional.test, scoping, symbols);
            collect_thread_expression_symbols(&conditional.consequent, scoping, symbols);
            collect_thread_expression_symbols(&conditional.alternate, scoping, symbols);
        }
        Expression::CallExpression(call) => {
            collect_thread_expression_symbols(&call.callee, scoping, symbols);
            for argument in &call.arguments {
                if let Some(expression) = argument.as_expression() {
                    collect_thread_expression_symbols(expression, scoping, symbols);
                }
            }
        }
        Expression::StaticMemberExpression(member) => {
            collect_thread_expression_symbols(&member.object, scoping, symbols);
        }
        Expression::ComputedMemberExpression(member) => {
            collect_thread_expression_symbols(&member.object, scoping, symbols);
            collect_thread_expression_symbols(&member.expression, scoping, symbols);
        }
        Expression::AssignmentExpression(assignment) => {
            collect_thread_expression_symbols(&assignment.right, scoping, symbols);
        }
        Expression::SequenceExpression(sequence) => {
            for expression in &sequence.expressions {
                collect_thread_expression_symbols(expression, scoping, symbols);
            }
        }
        Expression::LogicalExpression(logical) => {
            collect_thread_expression_symbols(&logical.left, scoping, symbols);
            collect_thread_expression_symbols(&logical.right, scoping, symbols);
        }
        _ => {}
    }
}

fn thread_context_parameter_names(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    facts: &Facts,
) -> BTreeMap<SymbolId, (String, String)> {
    let Some(function) = thread_function_node(nodes, collected, facts.context_ctor) else {
        return BTreeMap::new();
    };
    let params = facts.context_params.clone();
    let mut names = BTreeMap::new();
    let mut direct_params = BTreeSet::new();
    let mut direct_fields = BTreeMap::new();
    let mut conditional_params = BTreeSet::new();
    let mut incremented_field = None;
    let mut indexed_state_field = None;
    for node in nodes.iter() {
        if !within_function(nodes, node.id(), function) {
            continue;
        }
        match node.kind() {
            AstKind::AssignmentExpression(assignment) => {
                let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
                    continue;
                };
                if !matches!(member.object, Expression::ThisExpression(_)) {
                    continue;
                }
                let Some(source) = expression_symbol(&assignment.right, scoping) else {
                    continue;
                };
                if !params.contains(&source) {
                    continue;
                }
                direct_params.insert(source);
                let field = member.property.name.as_str();
                direct_fields.insert(field.to_string(), source);
            }
            AstKind::UpdateExpression(update) => {
                let SimpleAssignmentTarget::StaticMemberExpression(member) = &update.argument
                else {
                    continue;
                };
                if matches!(member.object, Expression::ThisExpression(_)) {
                    incremented_field = Some(member.property.name.to_string());
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                let Expression::StaticMemberExpression(state) = &member.expression else {
                    continue;
                };
                if matches!(state.object, Expression::ThisExpression(_)) {
                    indexed_state_field = Some(state.property.name.to_string());
                }
            }
            AstKind::ConditionalExpression(conditional) => {
                let mut symbols = BTreeSet::new();
                collect_thread_expression_symbols(&conditional.test, scoping, &mut symbols);
                collect_thread_expression_symbols(&conditional.consequent, scoping, &mut symbols);
                collect_thread_expression_symbols(&conditional.alternate, scoping, &mut symbols);
                conditional_params
                    .extend(symbols.into_iter().filter(|symbol| params.contains(symbol)));
            }
            _ => {}
        }
    }
    let pc_field = facts.context_pc_field.clone().or(incremented_field);
    let state_field = facts.context_state_field.clone().or(indexed_state_field);
    for (field, parameter) in direct_fields {
        if pc_field.as_deref() == Some(field.as_str()) {
            names.insert(
                parameter,
                thread_name("byteOffset", "context PC field dataflow"),
            );
        } else if state_field.as_deref() == Some(field.as_str()) {
            names.insert(
                parameter,
                thread_name("dispatchState", "context state field dataflow"),
            );
        } else if facts.context_params.last() == Some(&parameter) {
            names.insert(
                parameter,
                thread_name("receiverValue", "context receiver value field dataflow"),
            );
        } else {
            names.insert(
                parameter,
                thread_name("memory", "context memory field dataflow"),
            );
        }
    }
    for parameter in conditional_params {
        names
            .entry(parameter)
            .or_insert_with(|| thread_name("receiver", "context receiver conditional dataflow"));
    }
    for parameter in direct_params {
        names
            .entry(parameter)
            .or_insert_with(|| thread_name("memory", "context parameter field dataflow"));
    }
    names
}

fn thread_build_names(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    facts: &Facts,
    context_names: &BTreeMap<SymbolId, (String, String)>,
) -> BTreeMap<SymbolId, (String, String)> {
    let Some(function) = thread_function_node(nodes, collected, facts.build_invocation) else {
        return BTreeMap::new();
    };
    let params = thread_function_params(nodes, collected, facts.build_invocation);
    let param_set: BTreeSet<_> = params.iter().copied().collect();
    let mut names = BTreeMap::new();
    for node in nodes.iter() {
        if !within_function(nodes, node.id(), function) {
            continue;
        }
        match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let Some(local) = binding_symbol(&declarator.id) else {
                    continue;
                };
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                if let Some(parameter) = thread_length_receiver(init, scoping) {
                    if param_set.contains(&parameter) {
                        names.insert(
                            parameter,
                            thread_name("argumentSlots", "builder argument-slot length dataflow"),
                        );
                        names.insert(
                            local,
                            thread_name("argumentSlotCount", "builder argument-slot length local"),
                        );
                    }
                }
                if let Expression::CallExpression(call) = init {
                    if call.arguments.len() == 2
                        && is_arguments_length(call.arguments.first(), scoping)
                        && call
                            .arguments
                            .get(1)
                            .and_then(Argument::as_expression)
                            .and_then(|expression| expression_symbol(expression, scoping))
                            .is_some()
                    {
                        names.insert(
                            local,
                            thread_name(
                                "providedArgumentCount",
                                "invocation argument-count dataflow",
                            ),
                        );
                    }
                }
            }
            AstKind::ForStatement(for_statement) => {
                let Some(init) = for_statement.init.as_ref() else {
                    continue;
                };
                let init_expression = init.as_expression();
                let Some(init_expression) = init_expression else {
                    continue;
                };
                for (local, right) in sequence_assignments(init_expression, scoping) {
                    let Some(parameter) = thread_length_receiver(right, scoping) else {
                        continue;
                    };
                    if param_set.contains(&parameter) {
                        names.insert(
                            parameter,
                            thread_name("initialSlots", "builder initial-slot length dataflow"),
                        );
                        names.insert(
                            local,
                            thread_name("initialSlotCount", "builder initial-slot length local"),
                        );
                    }
                }
            }
            AstKind::StaticMemberExpression(member) => {
                if facts
                    .memory_members
                    .get(member.property.name.as_str())
                    .is_some_and(|name| name == "cloneMemory")
                {
                    if let Some(parameter) = expression_symbol(&member.object, scoping) {
                        if param_set.contains(&parameter) {
                            names.insert(
                                parameter,
                                thread_name("memory", "builder memory clone dataflow"),
                            );
                        }
                    }
                }
            }
            AstKind::CallExpression(call) => {
                if !memory_method_is(&call.callee, facts, "initializeSlot")
                    && !memory_method_is(&call.callee, facts, "writeSlot")
                {
                    continue;
                }
                if let Some(parameter) = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|expression| expression_symbol(expression, scoping))
                    .filter(|parameter| param_set.contains(parameter))
                {
                    names.insert(
                        parameter,
                        thread_name("argumentsSlot", "builder arguments-slot memory write"),
                    );
                }
            }
            AstKind::IfStatement(statement) => {
                if let Expression::Identifier(identifier) = &statement.test {
                    if let Some(parameter) = reference_symbol(identifier, scoping) {
                        if param_set.contains(&parameter) {
                            names.insert(
                                parameter,
                                thread_name(
                                    "argumentsProvided",
                                    "builder optional-arguments branch",
                                ),
                            );
                        }
                    }
                }
            }
            AstKind::NewExpression(new)
                if callee_symbol(&new.callee, scoping) == facts.context_ctor =>
            {
                for (index, argument) in new.arguments.iter().enumerate() {
                    let Some(argument) = argument.as_expression() else {
                        continue;
                    };
                    let Some(parameter) = expression_symbol(argument, scoping) else {
                        continue;
                    };
                    let Some(context_parameter) = facts.context_params.get(index) else {
                        continue;
                    };
                    if let Some((name, evidence)) = context_names.get(context_parameter) {
                        if param_set.contains(&parameter) {
                            names.insert(
                                parameter,
                                (name.clone(), format!("builder forwards {evidence}")),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn is_arguments_length(argument: Option<&Argument<'_>>, scoping: &Scoping) -> bool {
    let Some(Expression::StaticMemberExpression(member)) =
        argument.and_then(Argument::as_expression)
    else {
        return false;
    };
    member.property.name == "length"
        && matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "arguments" && expression_symbol(&member.object, scoping).is_none())
}

fn thread_invoke_names(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    facts: &Facts,
) -> BTreeMap<SymbolId, (String, String)> {
    let Some(function) = thread_function_node(nodes, collected, facts.invoke_thread) else {
        return BTreeMap::new();
    };
    let mut names = BTreeMap::new();
    for node in nodes.iter() {
        if !within_function(nodes, node.id(), function) {
            continue;
        }
        match node.kind() {
            AstKind::ForStatement(for_statement) => {
                let Some(init) = for_statement.init.as_ref() else {
                    continue;
                };
                let Some(expression) = init.as_expression() else {
                    continue;
                };
                for (local, right) in sequence_assignments(expression, scoping) {
                    if matches!(right, Expression::NumericLiteral(number) if number.value == 0.0) {
                        names.insert(
                            local,
                            thread_name("slotIndex", "invocation slot-loop index"),
                        );
                    } else if let Some(parameter) = thread_length_receiver(right, scoping) {
                        if parameter != local {
                            names.insert(
                                local,
                                thread_name("initialSlotCount", "invocation initial-slot bound"),
                            );
                        }
                    }
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                let Some(local) = binding_symbol(&declarator.id) else {
                    continue;
                };
                let Some(Expression::CallExpression(call)) = declarator.init.as_ref() else {
                    continue;
                };
                if call.arguments.len() == 2 && is_arguments_length(call.arguments.first(), scoping)
                {
                    names.insert(
                        local,
                        thread_name("providedArgumentCount", "invocation argument-count local"),
                    );
                }
            }
            _ => {}
        }
    }
    names
}

fn thread_capture_names(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    facts: &Facts,
    build_names: &BTreeMap<SymbolId, (String, String)>,
) -> BTreeMap<SymbolId, (String, String)> {
    let Some(function) = thread_function_node(nodes, collected, facts.capture_thread) else {
        return BTreeMap::new();
    };
    let params = thread_function_params(nodes, collected, facts.capture_thread);
    let param_set: BTreeSet<_> = params.iter().copied().collect();
    let mut names = BTreeMap::new();
    let mut capture_index = None;
    let mut optional = BTreeSet::new();
    for node in nodes.iter() {
        if !within_function(nodes, node.id(), function) {
            continue;
        }
        match node.kind() {
            AstKind::ForStatement(for_statement) => {
                let Some(init) = for_statement.init.as_ref() else {
                    continue;
                };
                let Some(expression) = init.as_expression() else {
                    continue;
                };
                let assignments = sequence_assignments(expression, scoping);
                for (local, right) in assignments {
                    if matches!(right, Expression::NumericLiteral(number) if number.value == 0.0) {
                        capture_index = Some(local);
                        names.insert(
                            local,
                            thread_name("capturedSlotIndex", "capture slot-loop index"),
                        );
                    }
                    if let Some(parameter) = thread_length_receiver(right, scoping) {
                        if param_set.contains(&parameter) {
                            names.insert(
                                parameter,
                                thread_name("capturedSlots", "capture slot-loop bound dataflow"),
                            );
                            names.insert(
                                local,
                                thread_name("capturedSlotCount", "capture slot-loop bound"),
                            );
                        }
                    }
                }
            }
            AstKind::VariableDeclarator(declarator) => {
                let Some(local) = binding_symbol(&declarator.id) else {
                    continue;
                };
                let Some(init) = declarator.init.as_ref() else {
                    continue;
                };
                if let Some(parameter) = void_comparison_param(init, &param_set, scoping) {
                    optional.insert(parameter);
                    names.insert(
                        local,
                        thread_name("argumentsProvided", "capture optional-arguments branch"),
                    );
                }
            }
            AstKind::AssignmentExpression(assignment) => {
                let Some(local) = assignment_target_symbol(&assignment.left, scoping) else {
                    continue;
                };
                let is_builder_call = matches!(
                    &assignment.right,
                    Expression::CallExpression(call)
                        if callee_symbol(&call.callee, scoping) == facts.build_invocation
                );
                if is_builder_call {
                    names.insert(
                        local,
                        thread_name("threadInvocation", "capture builder result dataflow"),
                    );
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                let Expression::ComputedMemberExpression(indexed_slots) = &member.expression else {
                    continue;
                };
                let Some(index) = expression_symbol(&indexed_slots.expression, scoping) else {
                    continue;
                };
                if Some(index) != capture_index {
                    continue;
                }
                let Expression::StaticMemberExpression(slots) = &member.object else {
                    continue;
                };
                if facts.slots_key.as_deref() != Some(slots.property.name.as_str()) {
                    continue;
                }
                if let Some(memory) = expression_symbol(&slots.object, scoping) {
                    if param_set.contains(&memory) {
                        names.insert(
                            memory,
                            thread_name("memory", "capture source slot dataflow"),
                        );
                    }
                }
            }
            AstKind::CallExpression(call) => {
                if callee_symbol(&call.callee, scoping) == facts.build_invocation {
                    let build_params =
                        thread_function_params(nodes, collected, facts.build_invocation);
                    for argument in &call.arguments {
                        let argument_index = call
                            .arguments
                            .iter()
                            .position(|candidate| std::ptr::eq(candidate, argument));
                        let Some(argument) = argument.as_expression() else {
                            continue;
                        };
                        let Some(parameter) = expression_symbol(argument, scoping) else {
                            continue;
                        };
                        if let Some((name, evidence)) = argument_index
                            .and_then(|index| build_params.get(index))
                            .and_then(|build_parameter| build_names.get(build_parameter))
                        {
                            if param_set.contains(&parameter) {
                                names.insert(
                                    parameter,
                                    (name.clone(), format!("capture forwards {evidence}")),
                                );
                            }
                        }
                    }
                    continue;
                }
                if !memory_method_is(&call.callee, facts, "initializeSlot")
                    && !memory_method_is(&call.callee, facts, "writeSlot")
                {
                    continue;
                }
                if let Some(parameter) = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|expression| expression_symbol(expression, scoping))
                    .filter(|parameter| param_set.contains(parameter))
                {
                    names.insert(
                        parameter,
                        thread_name("resultSlot", "capture result-slot write"),
                    );
                }
            }
            _ => {}
        }
    }
    for parameter in optional {
        names
            .entry(parameter)
            .or_insert_with(|| thread_name("argumentsSlot", "capture optional slot dataflow"));
    }
    names
}

// ---------------------------------------------------------------------------
// Finalize
// ---------------------------------------------------------------------------

/// Name the locals of a proven base64 decoder from the standard decode
/// dataflow: alphabet literal, input length, input/output indices, the four
/// sextets read through the native `charAt`, and the three packed output bytes.
fn decoder_local_names(
    program: &Program<'_>,
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    facts: &Facts,
) -> BTreeMap<SymbolId, (String, String)> {
    let mut names = BTreeMap::new();
    for decoder in &facts.decoder {
        let Some(function) = nodes.iter().find_map(|node| match node.kind() {
            AstKind::Function(function)
                if function.id.as_ref().and_then(|id| id.symbol_id.get()) == Some(*decoder) =>
            {
                Some(function)
            }
            _ => None,
        }) else {
            continue;
        };
        let params: Vec<SymbolId> = function
            .params
            .items
            .iter()
            .filter_map(|parameter| binding_symbol(&parameter.pattern))
            .collect();
        if let Some(output) = params.get(1) {
            names.insert(
                *output,
                (
                    "outputBytes".into(),
                    "output buffer of the proven base64 decoder".into(),
                ),
            );
        }
        let char_at_aliases = crate::runtime_decoder::proven_char_at_aliases(
            program,
            scoping,
            nodes,
            function.span.start,
        );
        let function_id = function.node_id.get();
        let mut input_index = None;
        for node in nodes.iter() {
            if !within_function(nodes, node.id(), function_id) {
                continue;
            }
            match node.kind() {
                AstKind::VariableDeclarator(declarator) => {
                    let Some(local) = binding_symbol(&declarator.id) else {
                        continue;
                    };
                    let Some(init) = declarator.init.as_ref() else {
                        continue;
                    };
                    if let Expression::StringLiteral(literal) = init {
                        if literal.value.len() == 64
                            && literal.value.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
                        {
                            names.insert(
                                local,
                                ("base64Alphabet".into(), "base64 alphabet literal".into()),
                            );
                        }
                    }
                    if let Expression::StaticMemberExpression(member) = init {
                        if member.property.name == "length"
                            && expression_symbol(&member.object, scoping) == params.first().copied()
                        {
                            names.insert(
                                local,
                                ("inputLength".into(), "length of the encoded input".into()),
                            );
                        }
                    }
                }
                AstKind::ForStatement(statement) => {
                    if let Some(ForStatementInit::VariableDeclaration(declaration)) =
                        statement.init.as_ref()
                    {
                        for declarator in &declaration.declarations {
                            if !matches!(
                                declarator.init.as_ref(),
                                Some(Expression::NumericLiteral(number)) if number.value == 0.0
                            ) {
                                continue;
                            }
                            let Some(local) = binding_symbol(&declarator.id) else {
                                continue;
                            };
                            if input_index.is_none() {
                                input_index = Some(local);
                                names.insert(
                                    local,
                                    (
                                        "inputIndex".into(),
                                        "encoded-input cursor of the base64 loop".into(),
                                    ),
                                );
                            } else {
                                names.insert(
                                    local,
                                    (
                                        "outputIndex".into(),
                                        "decoded-output cursor of the base64 loop".into(),
                                    ),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // Sextets: assignments whose value reads the encoded input through
        // native `charAt`, keyed by the proven character offset.
        for node in nodes.iter() {
            if !within_function(nodes, node.id(), function_id) {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            if !contains_char_at_read(&assignment.right, scoping, &char_at_aliases) {
                continue;
            }
            let Some(offset) = char_at_offset(&assignment.right, input_index, scoping) else {
                continue;
            };
            if offset > 3 {
                continue;
            }
            let Some(local) = assignment_target_symbol(&assignment.left, scoping) else {
                continue;
            };
            names.insert(
                local,
                (
                    format!("sextet{}", offset + 1),
                    "six-bit value read from the encoded character".into(),
                ),
            );
        }
        // Packed output bytes: expressions combining two proven sextets.
        let sextets: BTreeSet<SymbolId> = names
            .iter()
            .filter_map(|(symbol, (name, _))| name.starts_with("sextet").then_some(*symbol))
            .collect();
        let mut byte = 1usize;
        for node in nodes.iter() {
            if !within_function(nodes, node.id(), function_id) {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let Some(local) = assignment_target_symbol(&assignment.left, scoping) else {
                continue;
            };
            if sextets.contains(&local) {
                continue;
            }
            let mut referenced = BTreeSet::new();
            collect_reference_symbols(&assignment.right, scoping, &mut referenced);
            if referenced.intersection(&sextets).count() < 2 {
                continue;
            }
            if byte <= 3 {
                names.insert(
                    local,
                    (
                        format!("decodedByte{byte}"),
                        "output byte packed from two sextets".into(),
                    ),
                );
                byte += 1;
            }
        }
    }
    names
}

/// A `charAt` read expressed directly (`value.charAt(i)`) or through a proven
/// curried/uncurried native alias (`lookupChar(value, i)`), which is the shape
/// left by retained native catalogues.
fn contains_char_at_read(
    expression: &Expression<'_>,
    scoping: &Scoping,
    aliases: &BTreeSet<SymbolId>,
) -> bool {
    match expression {
        Expression::StaticMemberExpression(member) => {
            member.property.name == "charAt"
                || contains_char_at_read(&member.object, scoping, aliases)
        }
        Expression::CallExpression(call) => {
            let alias_callee = match &call.callee {
                Expression::Identifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, scoping)
                        .is_some_and(|symbol| aliases.contains(&symbol))
                }
                _ => false,
            };
            alias_callee
                || contains_char_at_read(&call.callee, scoping, aliases)
                || call.arguments.iter().any(|argument| {
                    argument.as_expression().is_some_and(|expression| {
                        contains_char_at_read(expression, scoping, aliases)
                    })
                })
        }
        Expression::BinaryExpression(binary) => {
            contains_char_at_read(&binary.left, scoping, aliases)
                || contains_char_at_read(&binary.right, scoping, aliases)
        }
        Expression::ParenthesizedExpression(paren) => {
            contains_char_at_read(&paren.expression, scoping, aliases)
        }
        Expression::ConditionalExpression(conditional) => {
            contains_char_at_read(&conditional.test, scoping, aliases)
                || contains_char_at_read(&conditional.consequent, scoping, aliases)
                || contains_char_at_read(&conditional.alternate, scoping, aliases)
        }
        _ => false,
    }
}

fn char_at_offset(
    expression: &Expression<'_>,
    input_index: Option<SymbolId>,
    scoping: &Scoping,
) -> Option<usize> {
    let input_index = input_index?;
    fn find_offset(
        expression: &Expression<'_>,
        input_index: SymbolId,
        scoping: &Scoping,
        saw_index: &mut bool,
    ) -> Option<usize> {
        match expression {
            Expression::BinaryExpression(binary) => {
                if binary.operator == BinaryOperator::Addition {
                    if let Expression::NumericLiteral(number) = &binary.right {
                        if expression_symbol(&binary.left, scoping) == Some(input_index) {
                            return Some(number.value as usize);
                        }
                    }
                }
                find_offset(&binary.left, input_index, scoping, saw_index)
                    .or_else(|| find_offset(&binary.right, input_index, scoping, saw_index))
            }
            Expression::Identifier(_) => {
                if expression_symbol(expression, scoping) == Some(input_index) {
                    *saw_index = true;
                }
                None
            }
            Expression::ParenthesizedExpression(paren) => {
                find_offset(&paren.expression, input_index, scoping, saw_index)
            }
            Expression::CallExpression(call) => {
                let mut found = find_offset(&call.callee, input_index, scoping, saw_index);
                for argument in &call.arguments {
                    if let Some(expression) = argument.as_expression() {
                        if found.is_none() {
                            found = find_offset(expression, input_index, scoping, saw_index);
                        } else {
                            let _ = find_offset(expression, input_index, scoping, saw_index);
                        }
                    }
                }
                found
            }
            Expression::StaticMemberExpression(member) => {
                find_offset(&member.object, input_index, scoping, saw_index)
            }
            Expression::ConditionalExpression(conditional) => {
                let mut found = find_offset(&conditional.test, input_index, scoping, saw_index);
                for expression in [&conditional.consequent, &conditional.alternate] {
                    if found.is_none() {
                        found = find_offset(expression, input_index, scoping, saw_index);
                    } else {
                        let _ = find_offset(expression, input_index, scoping, saw_index);
                    }
                }
                found
            }
            _ => None,
        }
    }
    let mut saw_index = false;
    if let Some(offset) = find_offset(expression, input_index, scoping, &mut saw_index) {
        return Some(offset);
    }
    saw_index.then_some(0)
}

fn collect_reference_symbols(
    expression: &Expression<'_>,
    scoping: &Scoping,
    out: &mut BTreeSet<SymbolId>,
) {
    match expression {
        Expression::Identifier(identifier) => {
            if let Some(symbol) = reference_symbol(identifier, scoping) {
                out.insert(symbol);
            }
        }
        Expression::BinaryExpression(binary) => {
            collect_reference_symbols(&binary.left, scoping, out);
            collect_reference_symbols(&binary.right, scoping, out);
        }
        Expression::ParenthesizedExpression(paren) => {
            collect_reference_symbols(&paren.expression, scoping, out);
        }
        Expression::UnaryExpression(unary) => {
            collect_reference_symbols(&unary.argument, scoping, out);
        }
        Expression::StaticMemberExpression(member) => {
            collect_reference_symbols(&member.object, scoping, out);
        }
        Expression::ComputedMemberExpression(member) => {
            collect_reference_symbols(&member.object, scoping, out);
            collect_reference_symbols(&member.expression, scoping, out);
        }
        Expression::CallExpression(call) => {
            collect_reference_symbols(&call.callee, scoping, out);
            for argument in &call.arguments {
                if let Some(expression) = argument.as_expression() {
                    collect_reference_symbols(expression, scoping, out);
                }
            }
        }
        Expression::SequenceExpression(sequence) => {
            for expression in &sequence.expressions {
                collect_reference_symbols(expression, scoping, out);
            }
        }
        _ => {}
    }
}

/// Name the parameter of a callback whose whole body forwards to a native
/// string method; the native method name itself is never renamed.
fn native_receiver_parameter_names(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
) -> BTreeMap<SymbolId, (String, String)> {
    let mut names = BTreeMap::new();
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let params: Vec<SymbolId> = function
            .params
            .items
            .iter()
            .filter_map(|parameter| binding_symbol(&parameter.pattern))
            .collect();
        let [parameter] = params.as_slice() else {
            continue;
        };
        for statement in &body.statements {
            let Statement::ReturnStatement(ret) = statement else {
                continue;
            };
            let Some(Expression::CallExpression(call)) = ret.argument.as_ref() else {
                continue;
            };
            let Expression::StaticMemberExpression(member) = &call.callee else {
                continue;
            };
            if member.property.name != "charCodeAt" {
                continue;
            }
            if expression_symbol(&member.object, scoping) != Some(*parameter) {
                continue;
            }
            names.insert(
                *parameter,
                (
                    "sourceString".into(),
                    "receiver of the native charCodeAt method".into(),
                ),
            );
        }
    }
    names
}

/// Locals whose role is proven by the context frame/unwind protocol or by the
/// interpreter's completion register. Operation handlers keep their positional
/// `valueN` names because the operation ABI pass fills those first.
fn auxiliary_local_names(
    program: &Program<'_>,
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    facts: &Facts,
) -> BTreeMap<SymbolId, (String, String)> {
    let mut names = BTreeMap::new();
    for (symbol, field) in &facts.frame_locals {
        let name = match facts.context_member_names.get(field).map(String::as_str) {
            Some("callFrameStack") => "callFrame",
            Some("tryFrameStack") => "tryFrame",
            Some("errorStack") => "errorTracker",
            _ => continue,
        };
        names.insert(
            *symbol,
            (
                name.into(),
                "record popped from the proven context frame stack".into(),
            ),
        );
    }
    for symbol in &facts.unwind_index_locals {
        names.insert(
            *symbol,
            (
                "errorIndex".into(),
                "loop index over the saved error-tracker entries".into(),
            ),
        );
    }
    for (symbol, name) in decoder_local_names(program, nodes, scoping, facts) {
        names.entry(symbol).or_insert(name);
    }
    for (symbol, name) in native_receiver_parameter_names(nodes, scoping) {
        names.entry(symbol).or_insert(name);
    }
    if let (Some(ctor), Some(key), Some(table)) = (
        facts.context_ctor,
        facts.context_dispatch_key.as_deref(),
        facts.dispatch_table,
    ) {
        for node in nodes.iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if !is_define_property(&call.callee, scoping, collected) || call.arguments.len() < 3 {
                continue;
            }
            let (Some(target), Some(Expression::StringLiteral(property))) = (
                call.arguments[0].as_expression(),
                call.arguments[1].as_expression(),
            ) else {
                continue;
            };
            if property.value.as_str() != key || proto_of(target, scoping, collected) != Some(ctor)
            {
                continue;
            }
            let Some(Expression::ObjectExpression(descriptor)) = call.arguments[2].as_expression()
            else {
                continue;
            };
            let Some(Expression::FunctionExpression(function)) = descriptor_value(descriptor)
            else {
                continue;
            };
            let function_id = function.node_id.get();
            for inner in nodes.iter() {
                if !within_function(nodes, inner.id(), function_id) {
                    continue;
                }
                let AstKind::VariableDeclarator(declarator) = inner.kind() else {
                    continue;
                };
                let Some(local) = binding_symbol(&declarator.id) else {
                    continue;
                };
                let mut symbols = BTreeSet::new();
                if let Some(init) = declarator.init.as_ref() {
                    collect_reference_symbols(init, scoping, &mut symbols);
                }
                if symbols.contains(&table) {
                    names.insert(
                        local,
                        (
                            "dispatchRecord".into(),
                            "state/operation pair returned by the dispatch table".into(),
                        ),
                    );
                }
            }
        }
    }
    if let Some(run) = facts.run {
        if let Some(run_node) = thread_function_node(nodes, collected, Some(run)) {
            let dispatch_key = facts.context_dispatch_key.as_deref();
            for node in nodes.iter() {
                if !within_function(nodes, node.id(), run_node) {
                    continue;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    continue;
                };
                let Some(local) = assignment_target_symbol(&assignment.left, scoping) else {
                    continue;
                };
                if let Expression::CallExpression(call) = &assignment.right {
                    if let Expression::StaticMemberExpression(member) = &call.callee {
                        if Some(member.property.name.as_str()) == dispatch_key {
                            names.insert(
                                local,
                                (
                                    "operationKey".into(),
                                    "operation key returned by the dispatch method".into(),
                                ),
                            );
                        }
                    }
                }
                if expression_symbol(&assignment.right, scoping) == facts.completion_result {
                    names.insert(
                        local,
                        (
                            "completionValue".into(),
                            "pending result moved out of the interpreter loop".into(),
                        ),
                    );
                }
            }
        }
    }
    names
}

#[allow(clippy::too_many_arguments)]
fn finalize<'a>(
    program: &Program<'a>,
    nodes: &AstNodes<'a>,
    scoping: &Scoping,
    collected: &Collected<'a>,
    facts: &Facts,
    types: &Types,
    mut decisions: Decisions,
    entry_span: Span,
) -> Plan {
    // Thread-helper locals are named from the proven dataflow of the helper
    // family.  Their source spellings and parameter positions are deliberately
    // ignored: these bindings are often shuffled independently by the input
    // generator, while their loop bounds, slot accesses, and constructor calls
    // remain stable.
    for (symbol, name) in thread_local_names(nodes, scoping, collected, facts) {
        decisions.locals.entry(symbol).or_insert(name);
    }
    for (symbol, name) in auxiliary_local_names(program, nodes, scoping, collected, facts) {
        decisions.locals.entry(symbol).or_insert(name);
    }

    // Semantic locals proven by typing.
    if let Some(local) = facts.descriptor_local {
        decisions.locals.entry(local).or_insert((
            "threadDescriptor".into(),
            "descriptor record read from the table".into(),
        ));
    }
    if let Some(local) = facts.context_local {
        decisions.locals.entry(local).or_insert((
            "context".into(),
            "fresh context built by the invocation closure".into(),
        ));
    }
    if let Some(local) = facts.memory_proto_alias {
        decisions.locals.entry(local).or_insert((
            "memoryPrototype".into(),
            "alias of the proven memory constructor prototype".into(),
        ));
    }
    if let Some(local) = facts.context_proto_alias {
        decisions.locals.entry(local).or_insert((
            "contextPrototype".into(),
            "alias of the proven context constructor prototype".into(),
        ));
    }
    if let Some(parameter) = facts.guarded_operation_param {
        decisions.locals.entry(parameter).or_insert((
            "operation".into(),
            "operation callable passed to the guarded wrapper".into(),
        ));
    }
    if let Some(parameter) = facts.guarded_context_param {
        decisions.locals.entry(parameter).or_insert((
            "context".into(),
            "execution context passed to the guarded wrapper".into(),
        ));
    }
    if let Some(parameter) = facts.exception_error_param {
        decisions.locals.entry(parameter).or_insert((
            "error".into(),
            "caught value routed to the proven exception handler".into(),
        ));
    }
    if let Some(parameter) = facts.exception_context_param {
        decisions.locals.entry(parameter).or_insert((
            "context".into(),
            "execution context routed to the proven exception handler".into(),
        ));
    }
    if let Some(local) = facts.stack_local {
        decisions.locals.entry(local).or_insert((
            "stack".into(),
            "array built by the stack factory with the native stack methods".into(),
        ));
    }
    for parameter in &facts.callback_context_params {
        decisions.locals.entry(*parameter).or_insert((
            "context".into(),
            "first parameter is the VM execution context for an operation/import callback".into(),
        ));
    }
    for parameters in &facts.callback_param_groups {
        for (position, parameter) in parameters.iter().enumerate() {
            let name = if position == 0 {
                "context".to_string()
            } else {
                format!("operand{position}")
            };
            decisions.locals.insert(
                *parameter,
                (name, "host-import callback ABI position".into()),
            );
        }
    }
    if let Some(error) = facts.guarded_error {
        decisions.locals.insert(
            error,
            ("error".into(), "operation wrapper catch binding".into()),
        );
    }
    if let Some(local) = facts.worker_source {
        decisions.locals.entry(local).or_insert((
            "workerSource".into(),
            "string source built from the VM entry for the worker blob".into(),
        ));
    }
    if let Some(local) = facts.worker_blob {
        decisions.locals.entry(local).or_insert((
            "workerBlob".into(),
            "Blob built from the proven worker source".into(),
        ));
    }
    if let Some(parameter) = facts.worker_message_event {
        decisions.locals.entry(parameter).or_insert((
            "messageEvent".into(),
            "onmessage event carrying the worker callback payload".into(),
        ));
    }
    if let Some(error) = facts.worker_error {
        decisions
            .locals
            .entry(error)
            .or_insert(("error".into(), "worker bootstrap catch binding".into()));
    }
    for status in &facts.run_status_locals {
        decisions.locals.insert(
            *status,
            (
                "executionStatus".into(),
                "interpreter return code used as the vmRun switch discriminant".into(),
            ),
        );
    }
    // One declarator index serves both operation-local naming and the
    // operation binding set below. It is built from a single node pass rather
    // than rescanning every node once per operation handler.
    let declarations_by_function = facts
        .operations
        .is_some()
        .then(|| function_declarations(nodes));
    let __profile = crate::profile::start();
    if let Some(operations) = facts.operations {
        if let Some(functions) = collected.array_functions.get(&operations) {
            let declarations_by_function = declarations_by_function
                .as_ref()
                .expect("declarators indexed");
            for function in functions {
                let Some(info) = collected.functions.get(function) else {
                    continue;
                };
                for (position, parameter) in info.params.iter().enumerate() {
                    // Operation ABI names are positional and identical for
                    // every handler. Do not infer semantic roles from a body:
                    // a shuffled operation table must not change its ABI, and
                    // `value`/`object`/`unresolved*` are not valid substitutes.
                    let name = if position == 0 {
                        "context".to_string()
                    } else {
                        format!("operand{position}")
                    };
                    decisions
                        .locals
                        .insert(*parameter, (name, "operation ABI position".into()));
                }
                let Some(declarations) = declarations_by_function.get(function) else {
                    continue;
                };
                // Operation locals deliberately use a per-handler ordinal
                // vocabulary. Body semantics and dispatch-table position must
                // not change the local binding contract.
                let ordinals = operation_local_ordinals(declarations);
                let next = ordinals.len() + 1;
                for (_, local) in declarations {
                    let name = format!("value{}", ordinals.get(local).copied().unwrap_or(next));
                    decisions
                        .locals
                        .insert(*local, (name, "operation local declaration order".into()));
                }
            }
        }
    }
    crate::profile::done("finalize.ops_locals", __profile);
    let __profile = crate::profile::start();
    for (function, info) in &collected.functions {
        if facts.fn_this.get(function) != Some(&ObjType::Memory) {
            continue;
        }
        for (position, parameter) in info.params.iter().enumerate() {
            if decisions.locals.contains_key(parameter) {
                continue;
            }
            let name = if memory_parameter_is_slot_index(*parameter, *function, nodes, scoping) {
                "slotIndex"
            } else if position > 0 {
                "value"
            } else {
                "slotIndex"
            };
            decisions.locals.insert(
                *parameter,
                (
                    name.into(),
                    "memory method parameter role inferred from slot dataflow".into(),
                ),
            );
        }
    }
    crate::profile::done("finalize.memory_params", __profile);
    for (symbol, obj) in &types.symbols {
        if *obj == ObjType::Memory && Some(*symbol) != facts.memory_ctor {
            decisions
                .locals
                .entry(*symbol)
                .or_insert(("capturedMemory".into(), "memory instance snapshot".into()));
        }
    }

    let mut report = std::mem::take(&mut decisions.report);
    let mut plan = Plan {
        changes: 0,
        binding_names: BTreeMap::new(),
        member_names: decisions.member_names,
        object_keys: decisions.object_keys,
        string_values: decisions.string_values,
        anon_names: decisions.anon_names,
        strip_function_names: decisions.strip_function_names,
        binding_mappings: Vec::new(),
        property_mappings: decisions.property_mappings,
        binding_residue: Vec::new(),
        proven_roles: Vec::new(),
        report: Vec::new(),
    };

    let __profile = crate::profile::start();
    let mut kernel_symbols: Vec<SymbolId> = scoping
        .symbol_ids()
        .filter(|symbol| symbol_in_kernel(scoping, *symbol, entry_span))
        .collect();
    let operation_symbols =
        operation_binding_symbols(collected, facts, declarations_by_function.as_ref());
    kernel_symbols.sort_by_key(|symbol| {
        (
            !operation_symbols.contains(symbol),
            scoping.symbol_span(*symbol).start,
        )
    });
    crate::profile::done("finalize.kernel_symbols", __profile);

    let __profile = crate::profile::start();
    let mut role_names: BTreeMap<SymbolId, (String, String)> = BTreeMap::new();
    for (symbol, role, evidence) in role_assignments(facts) {
        role_names
            .entry(symbol)
            .or_insert((role.canonical().into(), evidence.to_string()));
    }
    for (symbol, info) in &collected.decls {
        // A binding that is reassigned after its initializer is not a stable
        // alias of the proven native producer, so keep its spelling rather than
        // giving it a semantic name that the later value would contradict.
        if scoping.symbol_is_mutated(*symbol) {
            continue;
        }
        if let Some(name) = info
            .init
            .and_then(|init| native_alias_name(init, scoping, collected))
        {
            role_names
                .entry(*symbol)
                .or_insert((name, "binding-resolved native producer".into()));
        }
    }
    let mut param_names: BTreeMap<SymbolId, (String, String)> = BTreeMap::new();
    for (symbol, role, evidence) in role_assignments(facts) {
        if role == VmRole::InvokeGuarded && facts.guarded_operation_param.is_some() {
            continue;
        }
        let roles = role.parameter_roles();
        if roles.is_empty() {
            continue;
        }
        let Some((_, info)) = collected.functions.iter().find(|(node, info)| {
            info.symbol == Some(symbol) || collected.assigned_symbol.get(node) == Some(&symbol)
        }) else {
            continue;
        };
        for (parameter, name) in info.params.iter().zip(roles.iter()) {
            if role_names.contains_key(parameter)
                || param_names.contains_key(parameter)
                || decisions.locals.contains_key(parameter)
            {
                continue;
            }
            param_names.insert(*parameter, ((*name).into(), evidence.to_string()));
        }
    }
    crate::profile::done("finalize.role_param_names", __profile);

    // First collect desired names. Unknown bindings are intentionally absent:
    // keeping their source spelling is safer than inventing a colliding
    // `unresolvedBinding` placeholder. Operation ABI names are allocated first
    // so an unrelated captured binding can be disambiguated without merging
    // distinct SymbolIds.
    let __profile = crate::profile::start();
    let mut desired_names: BTreeMap<SymbolId, (String, String)> = BTreeMap::new();
    for symbol in &kernel_symbols {
        let original_spelling = scoping.symbol_name(*symbol).to_string();
        if original_spelling.starts_with("vmDecodeString") {
            desired_names.insert(
                *symbol,
                (original_spelling, "preserved shared decoder helper".into()),
            );
            continue;
        }
        if let Some(entry) = role_names.get(symbol).or_else(|| param_names.get(symbol)) {
            desired_names.insert(*symbol, entry.clone());
            continue;
        }
        if let Some(entry) = decisions.locals.get(symbol) {
            desired_names.insert(*symbol, entry.clone());
            continue;
        }
        let old = original_spelling;
        if is_meaningful_generated_name(&old) {
            desired_names.insert(*symbol, (old, "preserved semantic helper name".into()));
            continue;
        }
        let kind = match classify_symbol(*symbol, collected) {
            SymbolKind::Param => "parameter",
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Catch => "catch binding",
            SymbolKind::Binding => "binding",
        };
        report.push(Diagnostic::warn(
            Stage::Rename,
            "UNRESOLVED_BINDING_MAPPING",
            format!("{kind} `{old}` retained because its semantic role is unresolved"),
        ));
        plan.binding_residue.push(old);
    }
    crate::profile::done("finalize.desired_names", __profile);

    let __profile = crate::profile::start();
    // A conflict can only matter when the kernel binding still carries the
    // operation binding's *original* spelling, so index the kernel bindings by
    // that spelling once instead of rescanning every kernel symbol for every
    // (operation function, operation symbol) pair. The per-pair guard order and
    // the relative kernel order are preserved, so the outcome is identical.
    let mut kernel_conflicts_by_name: BTreeMap<&str, Vec<SymbolId>> = BTreeMap::new();
    for symbol in &kernel_symbols {
        kernel_conflicts_by_name
            .entry(scoping.symbol_name(*symbol))
            .or_default()
            .push(*symbol);
    }
    // A pair can only rename something when the kernel's spelling table has a
    // candidate other than the operation binding itself. Index the operation
    // bindings once and drop groups with no such candidate, instead of
    // re-looking-up every (handler, operation symbol) pair and then skipping
    // the self entry inside the handler loop.
    let conflict_groups: Vec<(SymbolId, String, Vec<SymbolId>)> = operation_symbols
        .iter()
        .copied()
        .filter_map(|symbol| {
            let (preferred, _) = desired_names.get(&symbol)?;
            let candidates = kernel_conflicts_by_name.get(preferred.as_str())?;
            if !candidates.iter().any(|candidate| *candidate != symbol) {
                return None;
            }
            Some((symbol, preferred.clone(), candidates.clone()))
        })
        .collect();
    for function in operation_functions(collected, facts) {
        for (symbol, preferred, candidates) in &conflict_groups {
            for conflict in candidates.iter().copied() {
                crate::profile::tally("finalize.collision.candidates");
                if conflict == *symbol
                    || desired_names.contains_key(&conflict)
                    || !scopes_conflict(*symbol, conflict, scoping)
                {
                    continue;
                }
                if !operation_references_symbol(nodes, scoping, function, conflict) {
                    continue;
                }
                desired_names.insert(
                    conflict,
                    (
                        collision_fallback_name(preferred),
                        "unresolved binding renamed to protect positional operation identity"
                            .into(),
                    ),
                );
                report.push(Diagnostic::warn(
                    Stage::Rename,
                    "UNRESOLVED_BINDING_MAPPING",
                    format!(
                        "binding `{preferred}` renamed only to preserve distinct operation binding identity"
                    ),
                ));
            }
        }
    }
    crate::profile::done("finalize.collision_loop", __profile);

    // Allocate against the complete lexical tree, not only one scope's old
    // spellings. This prevents renamed bindings from capturing an ancestor or
    // descendant binding while retaining the required operation ABI names.
    let __profile = crate::profile::start();
    let mut final_names: BTreeMap<SymbolId, (String, String)> = BTreeMap::new();
    // Index every binding by the spelling it currently contributes: an already
    // allocated name, else its desired semantic name, else its source name.
    // A candidate can only conflict with a binding under the same spelling, so
    // each lookup touches a handful of symbols instead of rescanning the whole
    // symbol table (and its scope ancestry) per candidate.
    let mut effective_names: HashMap<String, Vec<SymbolId>> = HashMap::new();
    for symbol in scoping.symbol_ids() {
        let name = desired_names
            .get(&symbol)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| scoping.symbol_name(symbol).to_string());
        effective_names.entry(name).or_default().push(symbol);
    }
    let mut assigned: BTreeSet<SymbolId> = BTreeSet::new();
    for symbol in &kernel_symbols {
        let Some((preferred, evidence)) = desired_names.get(symbol) else {
            continue;
        };
        let Some(candidate) = allocate_name(
            *symbol,
            preferred,
            &effective_names,
            &assigned,
            scoping,
            operation_symbols.contains(symbol),
        ) else {
            let old = scoping.symbol_name(*symbol).to_string();
            report.push(Diagnostic::warn(
                Stage::Rename,
                "NAME_COLLISION_RETAINED",
                format!(
                    "binding `{old}` retained because `{preferred}` has no safe noncolliding name"
                ),
            ));
            continue;
        };
        let previous = desired_names
            .get(symbol)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| scoping.symbol_name(*symbol).to_string());
        if let Some(bucket) = effective_names.get_mut(&previous) {
            bucket.retain(|other| other != symbol);
        }
        effective_names
            .entry(candidate.clone())
            .or_default()
            .push(*symbol);
        assigned.insert(*symbol);
        final_names.insert(*symbol, (candidate, evidence.clone()));
    }
    crate::profile::done("finalize.allocate_names", __profile);

    let __profile = crate::profile::start();
    for (symbol, (new_name, evidence)) in &final_names {
        let old = scoping.symbol_name(*symbol).to_string();
        if old == *new_name {
            // The chosen semantic name equals the original spelling: this owned
            // binding is residue and must be reported rather than hidden.
            plan.binding_residue.push(old);
            continue;
        }
        plan.binding_names
            .insert(*symbol, (old.clone(), new_name.clone()));
        plan.changes += 1;
        plan.report.push(Diagnostic::info(
            Stage::Rename,
            "BINDING_MAP",
            format!(
                "span {} `{}` -> `{}` ({})",
                span_label(scoping.symbol_span(*symbol)),
                old,
                new_name,
                evidence
            ),
        ));
        plan.binding_mappings.push(BindingMapping {
            span: {
                let span = scoping.symbol_span(*symbol);
                (span.start, span.end)
            },
            original_name: old,
            canonical_name: new_name.clone(),
            role: role_names.get(symbol).map(|(name, _)| name.clone()),
        });
    }
    crate::profile::done("finalize.report_names", __profile);

    // Preserve the semantic operation-table identity across code generation.
    // SymbolIds belong to this AST only, so the final presentation pass gets
    // the role plus stable source metadata and resolves a fresh SymbolId after
    // the post-RENAME parse.  Do not manufacture this record from a spelling
    // or from a callable-array consumer.
    if let Some(operations) = facts.proven_operations {
        if operation_is_proven(nodes, collected, scoping, operations) {
            let original_name = scoping.symbol_name(operations).to_string();
            let canonical_name = final_names
                .get(&operations)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| original_name.clone());
            // SymbolIds are not stable across code generation, so carry the
            // declaration's document position plus an initializer fingerprint.
            let identity = declaration_ordinal(nodes, operations)
                .zip(initializer_fingerprint(collected, operations));
            if let Some((declaration_ordinal, (initializer_elements, initializer_functions))) =
                identity
            {
                plan.proven_roles.push(ProvenRoleBinding {
                    role: VmRole::Operations,
                    original_name,
                    canonical_name,
                    declaration_ordinal,
                    initializer_elements,
                    initializer_functions,
                });
            } else {
                plan.report.push(Diagnostic::warn(
                    Stage::Rename,
                    "OPERATION_IDENTITY_UNSTABLE",
                    format!(
                        "proven operation binding `{original_name}` has no stable declaration identity"
                    ),
                ));
            }
        }
    }
    for (node, symbol) in &collected.assigned_symbol {
        let anonymous_name = plan.anon_names.get(node).cloned();
        if let (Some((function_name, _)), Some(_anonymous_name)) =
            (final_names.get(symbol), anonymous_name)
        {
            plan.anon_names.insert(*node, function_name.clone());
        }
    }
    plan.changes += plan.member_names.len()
        + plan.object_keys.len()
        + plan.string_values.len()
        + plan.anon_names.len();
    let _ = nodes;
    for (node, name) in &plan.member_names {
        plan.report.push(Diagnostic::info(
            Stage::Rename,
            "MEMBER_MAP",
            format!("member node {node:?} -> `{name}`"),
        ));
    }
    plan.report.extend(report);
    plan
}

fn is_meaningful_generated_name(name: &str) -> bool {
    name.starts_with("vm")
        && name.len() > 2
        && name[2..]
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && !name.chars().any(|character| character.is_ascii_digit())
}

fn memory_parameter_is_slot_index(
    parameter: SymbolId,
    function: NodeId,
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
) -> bool {
    nodes.iter().any(|node| {
        if !nodes
            .ancestors(node.id())
            .any(|ancestor| ancestor.id() == function)
        {
            return false;
        }
        let AstKind::ComputedMemberExpression(member) = node.kind() else {
            return false;
        };
        let Expression::Identifier(index) = &member.expression else {
            return false;
        };
        let index_symbol = crate::analysis::identifier_symbol(index, scoping);
        index_symbol == Some(parameter)
            && matches!(&member.object, Expression::StaticMemberExpression(slots)
                if matches!(&slots.object, Expression::ThisExpression(_)))
    })
}

fn allocate_name(
    symbol: SymbolId,
    preferred: &str,
    effective_names: &HashMap<String, Vec<SymbolId>>,
    assigned: &BTreeSet<SymbolId>,
    scoping: &Scoping,
    positional: bool,
) -> Option<String> {
    for candidate in name_candidates(preferred) {
        // Only a binding whose current spelling is `candidate` can block it.
        // For any such binding the original rules reduce to: skip self, allow
        // non-conflicting scopes, and for positional operation names allow a
        // different unassigned scope. Every other binding was a no-op check.
        let available = effective_names
            .get(candidate.as_str())
            .is_none_or(|others| {
                others.iter().all(|other| {
                    *other == symbol
                        || !scopes_conflict(symbol, *other, scoping)
                        || (positional
                            && scoping.symbol_scope_id(symbol) != scoping.symbol_scope_id(*other)
                            && !assigned.contains(other))
                })
            });
        if available {
            return Some(candidate);
        }
    }
    None
}

fn name_candidates(preferred: &str) -> Vec<String> {
    let mut candidates = vec![preferred.to_string()];
    let alternatives: &[&str] = match preferred {
        "context" => &["operationContext", "executionContext", "capturedContext"],
        "operand1" => &["firstOperand", "leftOperand", "firstArgument"],
        "operand2" => &["secondOperand", "rightOperand", "secondArgument"],
        "operand3" => &["thirdOperand", "thirdArgument"],
        "operand4" => &["fourthOperand", "fourthArgument"],
        "slotIndex" => &["sourceSlotIndex", "targetSlotIndex", "memorySlotIndex"],
        "value" => &["slotValue", "storedValue", "sourceValue"],
        "capturedMemory" => &["threadMemory", "memorySnapshot"],
        "threadDescriptor" => &["descriptor", "threadRecord"],
        "descriptorIndex" => &["threadDescriptorIndex"],
        "byteOffset" => &["instructionOffset"],
        "dispatchState" => &["stateIndex"],
        "nativeBind" => &["nativeFunctionBind", "bindNative"],
        "nativeCall" => &["nativeFunctionCall", "callNative"],
        "nativeApply" => &["nativeFunctionApply", "applyNative"],
        "nativeArrayPop" => &["nativePop", "arrayPop"],
        "nativeArraySlice" => &["nativeSlice", "arraySlice"],
        "nativeArrayPush" => &["nativePush", "arrayPush"],
        "uncurryThis" => &["callAdapter", "callBoundAdapter"],
        "nativeHasOwnProperty" => &["hasOwnPropertyNative", "objectHasOwnProperty"],
        "fromCharCode" => &["nativeFromCharCode", "base64CharCode", "charCodeFromString"],
        "jsonStringify" => &["stringifyJson"],
        "createObject" => &["objectCreate"],
        "byteArrayConstructor" => &["byteConstructor"],
        _ => &[],
    };
    for alternative in alternatives {
        if !candidates.iter().any(|candidate| candidate == alternative) {
            candidates.push((*alternative).into());
        }
    }
    // Native aliases share a `native<Name>` shape. Give every one a predictable
    // collision fallback so a sibling operation local cannot force the binding
    // to keep its opaque source spelling.
    if let Some(rest) = preferred.strip_prefix("native") {
        for alternative in [format!("{rest}Native"), format!("native{rest}Alias")] {
            if !candidates.contains(&alternative) {
                candidates.push(alternative);
            }
        }
    }
    if let Some(rest) = preferred.strip_prefix("callBound") {
        let alternative = format!("bound{rest}");
        if !candidates.contains(&alternative) {
            candidates.push(alternative);
        }
    }
    candidates
}

fn is_native_function_catalogue(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    let Expression::ArrayExpression(array) = expression else {
        return false;
    };
    let mut native_count = 0usize;
    let mut named_alias_count = 0usize;
    for element in &array.elements {
        let Some(value) = element.as_expression() else {
            continue;
        };
        if native_alias_name(value, scoping, collected).is_some() {
            named_alias_count += 1;
            native_count += 1;
        } else if let Expression::Identifier(identifier) = value {
            let is_global = identifier
                .reference_id
                .get()
                .is_some_and(|reference| scoping.get_reference(reference).symbol_id().is_none());
            if is_global && crate::analysis::is_native_global(identifier.name.as_str()) {
                native_count += 1;
            }
        }
    }
    array.elements.len() >= 12 && native_count >= 8 && named_alias_count >= 3
}

/// Prove a readable name for a binding whose initializer is a native producer.
/// This is deliberately a naming-only catalogue: it never removes an alias or
/// rewrites a call, so shadowing, mutation, reflection, and fresh literal
/// identity remain outside this pass's cleanup boundary.
///
/// The proof follows the *captured* alias graph rather than the source
/// spelling: an unshadowed native global, a member on a proven native receiver
/// (`nativeBind = nativeObject.bind`), the retained call adapter
/// (`uncurryThis = nativeCall.bind(nativeBind, nativeCall)`), and an alias
/// derived from that adapter with one proven native function
/// (`callBoundArrayPush = uncurryThis(nativeArrayPush)`). Reassigned aliases
/// are rejected at every link so the semantic name cannot outlive the value it
/// describes.
fn native_alias_name(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> Option<String> {
    native_alias_name_inner(expression, scoping, collected, &mut BTreeSet::new())
}

fn native_alias_name_inner(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => {
            if let Some(name) = native_global_alias_name(identifier, scoping) {
                return Some(name.to_owned());
            }
            let symbol = reference_symbol(identifier, scoping)?;
            if scoping.symbol_is_mutated(symbol) || !seen.insert(symbol) {
                return None;
            }
            let init = collected.decls.get(&symbol)?.init?;
            native_alias_name_inner(init, scoping, collected, seen)
        }
        Expression::StaticMemberExpression(member) => {
            let object = &member.object;
            let property = member.property.name.as_str();
            // The receiver may itself be a proven alias of a native global
            // (`var Base = Object; Base.bind`), so follow the alias graph.
            let mut receiver_seen = seen.clone();
            if let Some(receiver) =
                native_alias_name_inner(object, scoping, collected, &mut receiver_seen)
            {
                let name = match (receiver.as_str(), property) {
                    ("nativeObject", "bind") => Some("nativeBind"),
                    ("nativeObject", "call") => Some("nativeCall"),
                    ("nativeObject", "apply") => Some("nativeApply"),
                    ("nativeObject", "getOwnPropertyDescriptor") => {
                        Some("nativeGetOwnPropertyDescriptor")
                    }
                    ("nativeObject", "defineProperty") => Some("nativeDefineProperty"),
                    ("nativeObject", "create") => Some("createObject"),
                    ("nativeJSON", "stringify") => Some("jsonStringify"),
                    ("nativeString", "fromCharCode") => Some("fromCharCode"),
                    ("nativeMath", "min") => Some("mathMin"),
                    ("nativeMath", "floor") => Some("mathFloor"),
                    _ => None,
                };
                if let Some(name) = name {
                    return Some(name.to_owned());
                }
            }
            if matches!(object, Expression::ArrayExpression(array) if array.elements.is_empty()) {
                return match property {
                    "push" => Some("nativeArrayPush".to_owned()),
                    "pop" => Some("nativeArrayPop".to_owned()),
                    "slice" => Some("nativeArraySlice".to_owned()),
                    "splice" => Some("nativeArraySplice".to_owned()),
                    "join" => Some("nativeArrayJoin".to_owned()),
                    "map" => Some("nativeArrayMap".to_owned()),
                    _ => None,
                };
            }
            if matches!(object, Expression::ObjectExpression(object) if object.properties.is_empty())
            {
                return (property == "hasOwnProperty").then(|| "nativeHasOwnProperty".into());
            }
            if matches!(object, Expression::StringLiteral(literal) if literal.value.is_empty()) {
                return match property {
                    "indexOf" => Some("nativeStringIndexOf".to_owned()),
                    "charAt" => Some("nativeStringCharAt".to_owned()),
                    _ => None,
                };
            }
            None
        }
        Expression::ConditionalExpression(conditional) => {
            let mut consequent_seen = seen.clone();
            let consequent = native_alias_name_inner(
                &conditional.consequent,
                scoping,
                collected,
                &mut consequent_seen,
            )?;
            let mut alternate_seen = seen.clone();
            let alternate = native_alias_name_inner(
                &conditional.alternate,
                scoping,
                collected,
                &mut alternate_seen,
            )?;
            (consequent == "nativeUint8Array" && alternate == "nativeArray")
                .then(|| "byteArrayConstructor".to_owned())
        }
        Expression::CallExpression(call) => call_native_alias_name(call, scoping, collected, seen),
        _ => None,
    }
}

/// Proved names for a local that is initialized from an unshadowed native
/// global. Host-only globals (`window`, `document`, ...) are deliberately not
/// in this table because they are outside the VM semantic scope.
fn native_global_alias_name(
    identifier: &IdentifierReference<'_>,
    scoping: &Scoping,
) -> Option<&'static str> {
    let unshadowed = identifier
        .reference_id
        .get()
        .is_some_and(|reference| scoping.get_reference(reference).symbol_id().is_none());
    if !unshadowed {
        return None;
    }
    Some(match identifier.name.as_str() {
        "ReferenceError" => "nativeReferenceError",
        "TypeError" => "nativeTypeError",
        "RangeError" => "nativeRangeError",
        "SyntaxError" => "nativeSyntaxError",
        "Error" => "nativeError",
        "AggregateError" => "nativeAggregateError",
        "Object" => "nativeObject",
        "Function" => "nativeFunction",
        "Array" => "nativeArray",
        "RegExp" => "nativeRegExp",
        "Number" => "nativeNumber",
        "String" => "nativeString",
        "Boolean" => "nativeBoolean",
        "Symbol" => "nativeSymbol",
        "BigInt" => "nativeBigInt",
        "Math" => "nativeMath",
        "JSON" => "nativeJSON",
        "Promise" => "nativePromise",
        "Date" => "nativeDate",
        "Map" => "nativeMap",
        "Set" => "nativeSet",
        "WeakMap" => "nativeWeakMap",
        "WeakSet" => "nativeWeakSet",
        "ArrayBuffer" => "nativeArrayBuffer",
        "SharedArrayBuffer" => "nativeSharedArrayBuffer",
        "DataView" => "nativeDataView",
        "Uint8Array" => "nativeUint8Array",
        "Uint8ClampedArray" => "nativeUint8ClampedArray",
        "Int8Array" => "nativeInt8Array",
        "Uint16Array" => "nativeUint16Array",
        "Int16Array" => "nativeInt16Array",
        "Uint32Array" => "nativeUint32Array",
        "Int32Array" => "nativeInt32Array",
        "Float32Array" => "nativeFloat32Array",
        "Float64Array" => "nativeFloat64Array",
        "BigInt64Array" => "nativeBigInt64Array",
        "BigUint64Array" => "nativeBigUint64Array",
        "TextDecoder" => "nativeTextDecoder",
        "TextEncoder" => "nativeTextEncoder",
        "WebAssembly" => "nativeWebAssembly",
        _ => return None,
    })
}

/// The retained call-adapter and its derived aliases:
///
/// ```js
/// var nativeCall = nativeObject.call, nativeBind = nativeObject.bind;
/// var uncurryThis = nativeCall.bind(nativeBind, nativeCall);
/// var callBoundArrayPush = uncurryThis(nativeArrayPush);
/// ```
///
/// The adapter proof requires the exact captured receiver and both bound
/// arguments. The derived proof requires a single proven native function
/// argument, so the result is the call-bound form of that *same* identity.
fn call_native_alias_name(
    call: &CallExpression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<String> {
    if let Expression::StaticMemberExpression(member) = &call.callee {
        if !member.optional && member.property.name == "bind" && call.arguments.len() == 2 {
            let mut receiver_seen = seen.clone();
            let receiver =
                native_alias_name_inner(&member.object, scoping, collected, &mut receiver_seen)?;
            let mut this_seen = seen.clone();
            let this_argument = call.arguments[0].as_expression().and_then(|argument| {
                native_alias_name_inner(argument, scoping, collected, &mut this_seen)
            })?;
            let mut bound_seen = seen.clone();
            let bound_argument = call.arguments[1].as_expression().and_then(|argument| {
                native_alias_name_inner(argument, scoping, collected, &mut bound_seen)
            })?;
            if receiver == "nativeCall"
                && this_argument == "nativeBind"
                && bound_argument == "nativeCall"
            {
                return Some("uncurryThis".to_owned());
            }
            return None;
        }
    }
    if call.arguments.len() != 1 {
        return None;
    }
    let mut callee_seen = seen.clone();
    if native_alias_name_inner(&call.callee, scoping, collected, &mut callee_seen)? != "uncurryThis"
    {
        return None;
    }
    let mut argument_seen = seen.clone();
    let argument = call.arguments[0].as_expression().and_then(|argument| {
        native_alias_name_inner(argument, scoping, collected, &mut argument_seen)
    })?;
    let suffix = argument.strip_prefix("native")?;
    let mut name = String::from("callBound");
    let mut chars = suffix.chars();
    name.extend(chars.next()?.to_uppercase());
    name.push_str(chars.as_str());
    Some(name)
}

fn operation_functions(collected: &Collected<'_>, facts: &Facts) -> Vec<NodeId> {
    facts
        .operations
        .and_then(|operations| collected.array_functions.get(&operations))
        .cloned()
        .unwrap_or_default()
}

/// Minimum number of array elements (including elisions) required before a
/// callable table is treated as a proven operation dispatch table.  This
/// mirrors the FIND-stage `InitShape::Functions(n >= 4)` role proof: a short
/// 2-3 entry helper array is indistinguishable from a generic dynamic helper
/// table, so it must stay an array.
const OPERATION_TABLE_MIN_ENTRIES: usize = 4;

/// Prove that a callable array has the operation ABI evidence needed to
/// receive the Operations role.  A generic dynamic helper array is
/// deliberately not enough: the table must either be large enough to match the
/// FIND-stage `Functions(n >= 4)` role proof, or be a single-entry dispatcher
/// whose handler reaches a member of its leading context parameter (the
/// native-kernel `ops` shape).
fn operation_is_proven(
    nodes: &AstNodes<'_>,
    collected: &Collected<'_>,
    scoping: &Scoping,
    symbol: SymbolId,
) -> bool {
    let Some(DeclInfo {
        init: Some(Expression::ArrayExpression(array)),
    }) = collected.decls.get(&symbol)
    else {
        return false;
    };
    let mut functions = 0usize;
    for element in &array.elements {
        match element {
            ArrayExpressionElement::FunctionExpression(function) => {
                functions += 1;
                let Some(info) = collected.functions.get(&function.node_id.get()) else {
                    return false;
                };
                if info.params.is_empty() {
                    return false;
                }
            }
            ArrayExpressionElement::ArrowFunctionExpression(function) => {
                functions += 1;
                let Some(info) = collected.functions.get(&function.node_id.get()) else {
                    return false;
                };
                if info.params.is_empty() {
                    return false;
                }
            }
            ArrayExpressionElement::Elision(_) => {}
            ArrayExpressionElement::SpreadElement(_) => return false,
            _ => return false,
        }
    }
    if functions == 0 {
        return false;
    }
    if array.elements.len() >= OPERATION_TABLE_MIN_ENTRIES {
        return true;
    }
    functions == 1 && operation_has_context_member(nodes, collected, scoping, symbol)
}

fn operation_has_context_member(
    nodes: &AstNodes<'_>,
    collected: &Collected<'_>,
    scoping: &Scoping,
    symbol: SymbolId,
) -> bool {
    let Some(DeclInfo {
        init: Some(Expression::ArrayExpression(array)),
    }) = collected.decls.get(&symbol)
    else {
        return false;
    };
    let functions: Vec<NodeId> = array
        .elements
        .iter()
        .filter_map(|element| match element {
            ArrayExpressionElement::FunctionExpression(function) => Some(function.node_id.get()),
            ArrayExpressionElement::ArrowFunctionExpression(function) => {
                Some(function.node_id.get())
            }
            _ => None,
        })
        .collect();
    let Some(function) = functions.first().copied().filter(|_| functions.len() == 1) else {
        return false;
    };
    let Some(parameter) = collected
        .functions
        .get(&function)
        .and_then(|info| info.params.first())
        .copied()
    else {
        return false;
    };
    nodes.iter().any(|node| {
        let object = match node.kind() {
            AstKind::StaticMemberExpression(member) => &member.object,
            AstKind::ComputedMemberExpression(member) => &member.object,
            _ => return false,
        };
        let Expression::Identifier(identifier) = object else {
            return false;
        };
        reference_symbol(identifier, scoping) == Some(parameter)
            && nodes
                .ancestors(node.id())
                .any(|ancestor| ancestor.id() == function)
    })
}

/// Document-order position of a binding's `VariableDeclarator` among every
/// declarator in the program.  Code generation preserves declaration order, so
/// this locates the same declaration after the post-RENAME reparse even though
/// SymbolIds and spans have changed.
fn declaration_ordinal(nodes: &AstNodes<'_>, symbol: SymbolId) -> Option<u32> {
    // Count every `VariableDeclarator`, including destructuring declarations
    // whose binding has no single symbol, so the ordinal matches the
    // presentation-side visitor counter exactly.
    let mut declarations: Vec<(u32, Option<SymbolId>)> = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                Some((declarator.span.start, binding_symbol(&declarator.id)))
            }
            _ => None,
        })
        .collect();
    declarations.sort_by_key(|(start, _)| *start);
    declarations
        .iter()
        .position(|(_, candidate)| *candidate == Some(symbol))
        .map(|index| index as u32)
}

/// Number of source elements (including elisions) and number of function-valued
/// elements in a proven initializer.  Both survive code generation unchanged
/// and guard against a different binding occupying the proven position.
fn initializer_fingerprint(collected: &Collected<'_>, symbol: SymbolId) -> Option<(u32, u32)> {
    let Some(DeclInfo {
        init: Some(Expression::ArrayExpression(array)),
    }) = collected.decls.get(&symbol)
    else {
        return None;
    };
    let functions = array
        .elements
        .iter()
        .filter(|element| {
            matches!(
                element,
                ArrayExpressionElement::FunctionExpression(_)
                    | ArrayExpressionElement::ArrowFunctionExpression(_)
            )
        })
        .count();
    Some((array.elements.len() as u32, functions as u32))
}

/// Operation parameters plus every declarator belonging directly to an
/// operation handler. The declarator index is shared with operation-local
/// naming, so this no longer rescans the whole node table per handler.
fn operation_binding_symbols(
    collected: &Collected<'_>,
    facts: &Facts,
    declarations_by_function: Option<&BTreeMap<NodeId, Vec<(u32, SymbolId)>>>,
) -> BTreeSet<SymbolId> {
    let mut symbols = BTreeSet::new();
    for function in operation_functions(collected, facts) {
        if let Some(info) = collected.functions.get(&function) {
            symbols.extend(info.params.iter().copied());
        }
        if let Some(declarations) = declarations_by_function.and_then(|index| index.get(&function))
        {
            symbols.extend(declarations.iter().map(|(_, symbol)| *symbol));
        }
    }
    symbols
}

fn operation_references_symbol(
    nodes: &AstNodes<'_>,
    scoping: &Scoping,
    function: NodeId,
    symbol: SymbolId,
) -> bool {
    nodes.iter().any(|node| {
        let AstKind::IdentifierReference(identifier) = node.kind() else {
            return false;
        };
        reference_symbol(identifier, scoping) == Some(symbol)
            && nodes
                .ancestors(node.id())
                .any(|ancestor| ancestor.id() == function)
    })
}

fn collision_fallback_name(preferred: &str) -> String {
    match preferred {
        "context" => "outerContext".into(),
        name if name.starts_with("operand") => format!("outer{name}"),
        name if name.starts_with("value") => "outerValue".into(),
        _ => format!("outer{preferred}"),
    }
}

fn scopes_conflict(a: SymbolId, b: SymbolId, scoping: &Scoping) -> bool {
    let a_scope = scoping.symbol_scope_id(a);
    let b_scope = scoping.symbol_scope_id(b);
    a_scope == b_scope
        || scoping
            .scope_ancestors(a_scope)
            .any(|scope| scope == b_scope)
        || scoping
            .scope_ancestors(b_scope)
            .any(|scope| scope == a_scope)
}

fn role_assignments(facts: &Facts) -> Vec<(SymbolId, VmRole, &'static str)> {
    let mut out = Vec::new();
    let mut push = |symbol: Option<SymbolId>, role: VmRole, evidence: &'static str| {
        if let Some(symbol) = symbol {
            out.push((symbol, role, evidence));
        }
    };
    push(
        facts.entry,
        VmRole::Entry,
        "named top-level IIFE entry function",
    );
    push(
        facts.memory_ctor,
        VmRole::Memory,
        "constructor with `this.slots = []` and slot-value methods",
    );
    push(
        facts.slot_descriptor,
        VmRole::SlotDescriptor,
        "reused `value` descriptor passed to `defineProperty` for the memory slots property",
    );
    push(
        facts.context_ctor,
        VmRole::Context,
        "constructor with two stack fields and a memory field",
    );
    push(
        facts.stack_factory,
        VmRole::CreateStack,
        "factory returning an array with native push/pop/slice/splice methods",
    );
    push(
        facts.descriptors,
        VmRole::ThreadDescriptors,
        "record table read as thread descriptors",
    );
    push(
        facts.proven_operations,
        VmRole::Operations,
        "indexed function values invoked as operations",
    );
    push(
        facts.native_functions,
        VmRole::NativeFunctions,
        "catalogue of proven native-function aliases",
    );
    push(
        facts.create_thread,
        VmRole::CreateThread,
        "descriptor lookup then thread construction",
    );
    push(
        facts.capture_thread,
        VmRole::CaptureThread,
        "allocates a memory snapshot and builds the invocation closure",
    );
    push(
        facts.build_invocation,
        VmRole::BuildThreadInvocation,
        "returns the per-call invocation closure",
    );
    push(
        facts.invoke_thread,
        VmRole::InvokeThread,
        "per-call closure that builds a context and runs the VM",
    );
    push(
        facts.run,
        VmRole::Run,
        "interpreter callback proven from the invocation closure",
    );
    push(
        facts.pop_stack,
        VmRole::PopStack,
        "wrapper returns the proven stack factory pop method for a VM context",
    );
    push(
        facts.linear_memory,
        VmRole::LinearMemory,
        "WebAssembly.Memory instance written with the program image",
    );
    push(
        facts.string_table,
        VmRole::StringTable,
        "large literal string table consumed by indexed VM operations",
    );
    push(
        facts.number_table,
        VmRole::NumberTable,
        "literal numeric table used by the kernel",
    );
    push(
        facts.cache,
        VmRole::StringCache,
        "null-prototype decoded-string cache",
    );
    if facts.wasm_modules.len() == 1 {
        push(
            facts.wasm_modules.first().copied(),
            VmRole::WasmModule,
            "single proven WebAssembly.Module input; module-specific role is unresolved",
        );
    } else if let Some(symbol) = facts.wasm_modules.first() {
        push(
            Some(*symbol),
            VmRole::WasmHandlerTableModule,
            "first WebAssembly.Module imports the 201-entry operation handler table",
        );
    }
    if let Some(symbol) = facts.wasm_modules.get(1) {
        push(
            Some(*symbol),
            VmRole::WasmInterpreterModule,
            "second WebAssembly.Module exports the VM interpreter",
        );
    }
    if let Some(symbol) = facts.wasm_instances.first() {
        push(
            Some(*symbol),
            VmRole::WasmHandlerTableInstance,
            "first WebAssembly.Instance materializes the handler table",
        );
    }
    if let Some(symbol) = facts.wasm_instances.get(1) {
        push(
            Some(*symbol),
            VmRole::WasmInterpreterInstance,
            "second WebAssembly.Instance materializes the interpreter",
        );
    }
    push(
        facts.host_imports,
        VmRole::HostImports,
        "object passed as the interpreter module's host import namespace",
    );
    if facts.wasm_input_order.len() == 1 {
        push(
            facts.wasm_input_order.first().copied(),
            VmRole::WasmModule,
            "single proven WebAssembly module byte input",
        );
    } else {
        if let Some(symbol) = facts.wasm_input_order.first() {
            push(
                Some(*symbol),
                VmRole::WasmHandlerTableBytes,
                "bytes decoded for the handler-table WebAssembly.Module",
            );
        }
        if let Some(symbol) = facts.wasm_input_order.get(1) {
            push(
                Some(*symbol),
                VmRole::WasmInterpreterBytes,
                "bytes decoded for the interpreter WebAssembly.Module",
            );
        }
    }
    push(
        facts.dispatch_table,
        VmRole::DispatchTable,
        "table indexed by dispatch state and bytecode opcode",
    );
    push(
        facts.bytecode,
        VmRole::Bytecode,
        "bytecode image read at the context cursor by the dispatch method",
    );
    push(
        facts.guarded,
        VmRole::InvokeGuarded,
        "wrapper catches an indexed VM operation call and routes its error",
    );
    push(
        facts.handle_exception,
        VmRole::HandleException,
        "catch handler unwinds the saved try frame and records the thrown value",
    );
    push(
        facts.no_result_sentinel,
        VmRole::NoResultSentinel,
        "sentinel compared against the pending result before every dispatch step",
    );
    push(
        facts.empty_result_sentinel,
        VmRole::EmptyResultSentinel,
        "sentinel stored by an operation that completes with no value",
    );
    push(
        facts.completion_result,
        VmRole::CompletionResult,
        "pending result moved out of the interpreter loop",
    );
    push(
        facts.worker,
        VmRole::Worker,
        "cached worker instance built from the VM entry source",
    );
    push(
        facts.worker_callbacks,
        VmRole::WorkerCallbacks,
        "callback array indexed by the worker message payload",
    );
    push(
        facts.initialize_worker,
        VmRole::InitializeWorker,
        "worker bootstrap: builds the Blob/Worker and installs the onmessage dispatch",
    );
    for symbol in &facts.decoder {
        push(Some(*symbol), VmRole::DecodeBase64, "base64 decoder body");
    }
    out
}

enum SymbolKind {
    Param,
    Function,
    Catch,
    Class,
    Binding,
}

fn classify_symbol(symbol: SymbolId, collected: &Collected<'_>) -> SymbolKind {
    for info in collected.functions.values() {
        if let Some(index) = info
            .params
            .iter()
            .position(|parameter| *parameter == symbol)
        {
            let _ = index;
            return SymbolKind::Param;
        }
        if info.symbol == Some(symbol) {
            return SymbolKind::Function;
        }
    }
    if collected.catch_params.contains(&symbol) {
        return SymbolKind::Catch;
    }
    if collected.class_symbols.contains(&symbol) {
        return SymbolKind::Class;
    }
    SymbolKind::Binding
}

fn symbol_in_kernel(scoping: &Scoping, symbol: SymbolId, entry_span: Span) -> bool {
    let span = scoping.symbol_span(symbol);
    span.start >= entry_span.start && span.end <= entry_span.end
}

// ---------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------

pub(crate) fn apply<'a>(
    program: &mut Program<'a>,
    scoping: &Scoping,
    allocator: &'a Allocator,
    plan: &Plan,
) -> usize {
    let final_names: BTreeMap<SymbolId, String> = plan
        .binding_names
        .iter()
        .map(|(symbol, (_, new_name))| (*symbol, new_name.clone()))
        .collect();
    let mut second = ApplyVisitor {
        scoping,
        allocator,
        names: &final_names,
        member_names: &plan.member_names,
        object_keys: &plan.object_keys,
        string_values: &plan.string_values,
        anon_names: &plan.anon_names,
        strip_function_names: &plan.strip_function_names,
    };
    second.visit_program(program);
    plan.changes
}

struct ApplyVisitor<'a, 'b> {
    scoping: &'b Scoping,
    allocator: &'a Allocator,
    names: &'b BTreeMap<SymbolId, String>,
    member_names: &'b BTreeMap<NodeId, String>,
    object_keys: &'b BTreeMap<NodeId, String>,
    string_values: &'b BTreeMap<NodeId, String>,
    anon_names: &'b BTreeMap<NodeId, String>,
    strip_function_names: &'b BTreeSet<NodeId>,
}

impl<'a, 'b> VisitMut<'a> for ApplyVisitor<'a, 'b> {
    fn visit_binding_identifier(&mut self, it: &mut BindingIdentifier<'a>) {
        if let Some(symbol) = it.symbol_id.get() {
            if let Some(name) = self.names.get(&symbol) {
                it.name = Ident::from_in(name.as_str(), self.allocator);
            }
        }
        walk_mut::walk_binding_identifier(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &mut IdentifierReference<'a>) {
        if let Some(symbol) = crate::analysis::identifier_symbol(it, self.scoping) {
            if let Some(name) = self.names.get(&symbol) {
                it.name = Ident::from_in(name.as_str(), self.allocator);
            }
        }
        walk_mut::walk_identifier_reference(self, it);
    }

    fn visit_function(&mut self, it: &mut Function<'a>, flags: ScopeFlags) {
        walk_mut::walk_function(self, it, flags);
        if self.strip_function_names.contains(&it.node_id.get()) {
            it.id = None;
        } else if it.id.is_none() {
            if let Some(name) = self.anon_names.get(&it.node_id.get()) {
                it.id = Some(BindingIdentifier {
                    node_id: Cell::new(NodeId::DUMMY),
                    span: it.span,
                    name: Ident::from_in(name.as_str(), self.allocator),
                    symbol_id: Cell::new(None),
                });
            }
        }
    }

    fn visit_static_member_expression(&mut self, it: &mut StaticMemberExpression<'a>) {
        walk_mut::walk_static_member_expression(self, it);
        if let Some(name) = self.member_names.get(&it.node_id.get()) {
            it.property.name = Ident::from_in(name.as_str(), self.allocator);
        }
    }

    fn visit_object_property(&mut self, it: &mut ObjectProperty<'a>) {
        let was_shorthand = it.shorthand;
        walk_mut::walk_object_property(self, it);
        if let Some(name) = self.object_keys.get(&it.node_id.get()) {
            match &mut it.key {
                PropertyKey::StaticIdentifier(id) => {
                    id.name = Ident::from_in(name.as_str(), self.allocator);
                }
                PropertyKey::StringLiteral(literal) => {
                    literal.value = Atom::from_in(name.as_str(), self.allocator);
                }
                _ => {}
            }
            it.shorthand = false;
        } else if was_shorthand {
            let key_name = it.key.name().map(|name| name.into_owned());
            let value_name = match &it.value {
                Expression::Identifier(id) => Some(id.name.to_string()),
                _ => None,
            };
            if let (Some(key_name), Some(value_name)) = (key_name, value_name) {
                if key_name != value_name {
                    it.shorthand = false;
                }
            }
        }
    }

    fn visit_string_literal(&mut self, it: &mut StringLiteral<'a>) {
        if let Some(name) = self.string_values.get(&it.node_id.get()) {
            it.value = Atom::from_in(name.as_str(), self.allocator);
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id),
        _ => None,
    }
}

fn binding_symbol(pattern: &BindingPattern<'_>) -> Option<SymbolId> {
    simple_binding(pattern).and_then(|binding| binding.symbol_id.get())
}

fn callee_symbol(callee: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    expression_symbol(callee, scoping)
}

/// Resolve an expression to the symbol it references, when it is a plain
/// identifier. Used to check that a call's argument is a specific local.
fn expression_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, scoping)
}

fn is_call_to(expression: &Expression<'_>, target: SymbolId, scoping: &Scoping) -> bool {
    matches!(expression, Expression::CallExpression(call) if callee_symbol(&call.callee, scoping) == Some(target))
}

fn is_define_property(
    callee: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    match callee {
        Expression::StaticMemberExpression(member) => {
            member.property.name == "defineProperty"
                && is_object_reference(&member.object, scoping, collected)
        }
        Expression::Identifier(id) => crate::analysis::identifier_symbol(id, scoping)
            .and_then(|symbol| collected.decls.get(&symbol))
            .and_then(|info| info.init)
            .is_some_and(|init| is_define_property(init, scoping, collected)),
        _ => false,
    }
}

fn is_get_own_property_descriptor(
    callee: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    match callee {
        Expression::StaticMemberExpression(member) => {
            member.property.name == "getOwnPropertyDescriptor"
                && is_object_reference(&member.object, scoping, collected)
        }
        Expression::Identifier(id) => crate::analysis::identifier_symbol(id, scoping)
            .and_then(|symbol| collected.decls.get(&symbol))
            .and_then(|info| info.init)
            .is_some_and(|init| is_get_own_property_descriptor(init, scoping, collected)),
        _ => false,
    }
}

fn is_object_reference(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    let Expression::Identifier(id) = expression else {
        return false;
    };
    if is_unresolved_global(expression, "Object", scoping) {
        return true;
    }
    let Some(symbol) = crate::analysis::identifier_symbol(id, scoping) else {
        return false;
    };
    collected
        .decls
        .get(&symbol)
        .and_then(|info| info.init)
        .is_some_and(|init| is_global_alias(init, "Object", scoping, collected))
}

fn is_unresolved_global(expression: &Expression<'_>, name: &str, scoping: &Scoping) -> bool {
    let Expression::Identifier(identifier) = expression else {
        return false;
    };
    identifier.name == name
        && identifier
            .reference_id
            .get()
            .is_some_and(|reference| scoping.get_reference(reference).symbol_id().is_none())
}

fn is_global_alias(
    expression: &Expression<'_>,
    name: &str,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    if is_unresolved_global(expression, name, scoping) {
        return true;
    }
    let Expression::Identifier(identifier) = expression else {
        return false;
    };
    let Some(symbol) = crate::analysis::identifier_symbol(identifier, scoping) else {
        return false;
    };
    collected
        .decls
        .get(&symbol)
        .and_then(|info| info.init)
        .is_some_and(|init| is_unresolved_global(init, name, scoping))
}

fn is_webassembly_module(callee: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(callee, Expression::StaticMemberExpression(member)
        if member.property.name == "Module"
            && is_unresolved_global(&member.object, "WebAssembly", scoping))
}

fn is_webassembly_instance(callee: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(callee, Expression::StaticMemberExpression(member)
        if member.property.name == "Instance"
            && is_unresolved_global(&member.object, "WebAssembly", scoping))
}

fn is_new_wasm_memory(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::NewExpression(new)
        if matches!(&new.callee, Expression::StaticMemberExpression(member)
            if member.property.name == "Memory"
                && is_unresolved_global(&member.object, "WebAssembly", scoping)))
}

fn proto_of(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> Option<SymbolId> {
    match expression {
        Expression::StaticMemberExpression(member) if member.property.name == "prototype" => {
            let Expression::Identifier(id) = &member.object else {
                return None;
            };
            crate::analysis::identifier_symbol(id, scoping)
        }
        Expression::Identifier(id) => {
            let symbol = crate::analysis::identifier_symbol(id, scoping)?;
            let init = collected.decls.get(&symbol)?.init?;
            proto_of(init, scoping, collected)
        }
        _ => None,
    }
}

fn member_chain(
    arguments: &[Argument<'_>],
    descriptor: SymbolId,
    scoping: &Scoping,
) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for argument in arguments {
        let Expression::StaticMemberExpression(member) = argument.as_expression()? else {
            return None;
        };
        let Expression::Identifier(id) = &member.object else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(id, scoping)?;
        if symbol != descriptor {
            return None;
        }
        out.push(member.property.name.to_string());
    }
    Some(out)
}

fn descriptor_value<'a>(descriptor: &'a ObjectExpression<'a>) -> Option<&'a Expression<'a>> {
    for property in &descriptor.properties {
        if let ObjectPropertyKind::ObjectProperty(property) = property {
            if property.key_name() == "value" {
                return Some(&property.value);
            }
        }
    }
    None
}

fn enclosing_function<'a>(nodes: &AstNodes<'a>, node: NodeId) -> Option<&'a Function<'a>> {
    nodes
        .ancestors(node)
        .find_map(|ancestor| match ancestor.kind() {
            AstKind::Function(function) => Some(function),
            _ => None,
        })
}

// ---------------------------------------------------------------------------
// Shape helpers
// ---------------------------------------------------------------------------

fn shape_of(expression: &Expression<'_>, scoping: &Scoping) -> Shape {
    if array_all_strings(expression).is_some() {
        Shape::Strings(array_all_strings(expression).unwrap_or_default())
    } else if array_all_numbers(expression).is_some() {
        Shape::Numbers(array_all_numbers(expression).unwrap_or_default())
    } else if let Some(length) = function_table_length(expression) {
        Shape::Functions(length)
    } else if array_of_numeric_records(expression).is_some() {
        Shape::Records(array_of_numeric_records(expression).unwrap_or_default())
    } else if is_null_object_cache(expression, scoping) {
        Shape::Cache
    } else {
        Shape::Other
    }
}

fn array_elements<'a>(expression: &'a Expression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ArrayExpression(array) = expression else {
        return None;
    };
    array
        .elements
        .iter()
        .map(|element| match element {
            ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => None,
            other => Some(other.to_expression()),
        })
        .collect()
}

fn array_all_strings(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    (!elements.is_empty()
        && elements
            .iter()
            .all(|e| matches!(e, Expression::StringLiteral(_))))
    .then_some(elements.len())
}

fn array_all_numbers(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    (!elements.is_empty() && elements.iter().all(|e| static_number(e).is_some()))
        .then_some(elements.len())
}

fn function_table_length(expression: &Expression<'_>) -> Option<usize> {
    match expression {
        Expression::ArrayExpression(array) => {
            let mut functions = 0;
            for element in &array.elements {
                match element {
                    ArrayExpressionElement::FunctionExpression(_)
                    | ArrayExpressionElement::ArrowFunctionExpression(_) => functions += 1,
                    ArrayExpressionElement::Elision(_) => {}
                    ArrayExpressionElement::SpreadElement(_) => return None,
                    _ => return None,
                }
            }
            (functions > 0).then_some(array.elements.len())
        }
        Expression::ObjectExpression(object) => {
            let mut keys = BTreeSet::new();
            let mut functions = 0;
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return None;
                };
                let index = numeric_property_index(&property.key)?;
                if property.computed || !keys.insert(index) {
                    return None;
                }
                if !matches!(
                    property.value,
                    Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                ) {
                    return None;
                }
                functions += 1;
            }
            (functions > 0).then_some(functions)
        }
        _ => None,
    }
}

fn function_nodes(expression: &Expression<'_>) -> Vec<NodeId> {
    match expression {
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .filter_map(|element| match element {
                ArrayExpressionElement::FunctionExpression(function) => {
                    Some(function.node_id.get())
                }
                ArrayExpressionElement::ArrowFunctionExpression(function) => {
                    Some(function.node_id.get())
                }
                _ => None,
            })
            .collect(),
        Expression::ObjectExpression(object) => object
            .properties
            .iter()
            .filter_map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => match &property.value {
                    Expression::FunctionExpression(function) => Some(function.node_id.get()),
                    Expression::ArrowFunctionExpression(function) => {
                        Some(function.as_ref().node_id.get())
                    }
                    _ => None,
                },
                ObjectPropertyKind::SpreadProperty(_) => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn numeric_property_index(key: &PropertyKey<'_>) -> Option<u32> {
    match key {
        PropertyKey::NumericLiteral(number)
            if number.value.is_finite()
                && number.value >= 0.0
                && number.value.fract() == 0.0
                && number.value < u32::MAX as f64 =>
        {
            Some(number.value as u32)
        }
        PropertyKey::StringLiteral(string) => {
            let index = string.value.parse::<u32>().ok()?;
            (index < u32::MAX && string.value == index.to_string()).then_some(index)
        }
        _ => None,
    }
}

fn array_of_numeric_records(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    if elements.is_empty() {
        return None;
    }
    let valid = elements.iter().all(|e| {
        if let Expression::ObjectExpression(object) = e {
            object
                .properties
                .iter()
                .filter(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        array_all_numbers_allow_empty(&property.value).is_some()
                    }
                    ObjectPropertyKind::SpreadProperty(_) => false,
                })
                .count()
                >= 2
        } else {
            false
        }
    });
    valid.then_some(elements.len())
}

fn array_all_numbers_allow_empty(expression: &Expression<'_>) -> Option<usize> {
    let elements = array_elements(expression)?;
    elements
        .iter()
        .all(|e| static_number(e).is_some())
        .then_some(elements.len())
}

fn is_null_object_cache(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression, Expression::CallExpression(call)
        if matches!(&call.callee, Expression::StaticMemberExpression(member)
            if member.property.name == "create"
                && is_unresolved_global(&member.object, "Object", scoping))
            && matches!(call.arguments.first().map(Argument::to_expression), Some(Expression::NullLiteral(_))))
}

/// Resolve `Object.create` through member aliases before proving a null-prototype
/// cache. Builtin identity is still required, so a shadowed `Object` or a
/// redefined `create` never matches.
fn is_resolved_null_object_cache(
    expression: &Expression<'_>,
    scoping: &Scoping,
    collected: &Collected<'_>,
) -> bool {
    let Expression::CallExpression(call) = expression else {
        return false;
    };
    if !matches!(
        call.arguments.first().map(Argument::to_expression),
        Some(Expression::NullLiteral(_))
    ) {
        return false;
    }
    is_object_create(&call.callee, scoping, collected)
}

fn is_object_create(callee: &Expression<'_>, scoping: &Scoping, collected: &Collected<'_>) -> bool {
    match callee {
        Expression::StaticMemberExpression(member) if member.property.name == "create" => {
            is_object_reference(&member.object, scoping, collected)
        }
        Expression::Identifier(identifier) => {
            let Some(symbol) = reference_symbol(identifier, scoping) else {
                return false;
            };
            collected
                .decls
                .get(&symbol)
                .and_then(|info| info.init)
                .is_some_and(|init| is_object_create(init, scoping, collected))
        }
        _ => false,
    }
}

fn static_number(expression: &Expression<'_>) -> Option<f64> {
    match expression {
        Expression::NumericLiteral(number) => Some(number.value),
        Expression::UnaryExpression(unary) => {
            let value = static_number(&unary.argument)?;
            match unary.operator {
                UnaryOperator::UnaryNegation => Some(-value),
                UnaryOperator::UnaryPlus => Some(value),
                _ => None,
            }
        }
        _ => None,
    }
}

fn span_label(span: Span) -> String {
    format!("{}..{}", span.start, span.end)
}

trait ObjectPropertyExt {
    fn key_name(&self) -> String;
}

impl ObjectPropertyExt for ObjectProperty<'_> {
    fn key_name(&self) -> String {
        self.key
            .name()
            .map(|name| name.into_owned())
            .unwrap_or_default()
    }
}
