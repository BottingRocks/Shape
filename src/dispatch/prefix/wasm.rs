//! Preparation of a WASM-dispatched kernel for the prefix walker.
//!
//! A WASM kernel keeps the VM's dispatch loop in a WebAssembly interpreter:
//! its exported run function reads the opcode and dispatch cell from linear
//! memory and either calls one of the JavaScript operations (`vmOperations`,
//! through the handler module's table) or does the VM's own bookkeeping.
//! [`crate::kernel::wasm_lift`] names what every selector does from the
//! interpreter's bytes; this module turns that into the walker's inputs:
//!
//! - the dispatch table, read from memory the way the run function reads it;
//! - each operation's recipe, with the operands the interpreter decodes;
//! - the bookkeeping selectors as the recipes a JavaScript VM's handlers for
//!   the same operations have (jumps, calls, `try`/`catch`/`finally`,
//!   returns), so the walker and emitter run them unchanged;
//! - the image write as the patch handler;
//! - what the WASM machine bounds that a JavaScript VM does not: its try,
//!   record and call banks, and the interpreter's own tables in memory.
//!
//! Before any of it, the proof-view report (the pipeline without the WASM
//! presentation adapter) must link the handler module's imports to
//! `vmOperations` key for key, and the interpreter's helper imports must be
//! exactly the helper functions this lift assumes.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::kernel::wasm as boundary;
use crate::kernel::wasm_lift::{self, Helper, Operand, SelectorOp, ERROR_RECORDS, TRY_FRAMES};
use crate::runtime_config;
use crate::{
    DispatchCell, HandlerTemplate, JsOperandSeparationReport, LiftedHandler, LiteralValue,
    OperandCoverage, OperandDescriptor, OperandEndian, OperandEntryCoordinate,
    OperandHandlerStatus, OperandInputIdentity, OperandSpan, PipelineReport, PrimitiveOperator,
    RecipeOperation, RecipeStep, SemanticValue, SpanRecord, TemplateStatus, UnknownFragment,
};

use super::proof::{PatchHandlerProof, RunLoopProof};
use super::types::{PrefixLimits, PrefixPrepareError};
use super::PrefixPreparation;

/// The name the number table is read through in recipes.
pub(super) const NUMBER_TABLE: &str = "vmNumberTable";

/// What a WASM machine bounds that a JavaScript VM does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WasmMachine {
    /// Image addresses from here on hold the interpreter's own tables: the
    /// dispatch cells, read once, and the number table. A write there stops.
    pub frozen_from: usize,
    /// The number table, as f64 bits.
    pub number_table: Vec<u64>,
    /// Its locals hold this many try frames, records and one call's return.
    pub try_frames: usize,
    pub error_records: usize,
    pub call_frames: usize,
}

impl WasmMachine {
    pub(super) fn number_table(&self) -> Arc<[f64]> {
        self.number_table
            .iter()
            .map(|bits| f64::from_bits(*bits))
            .collect()
    }
}

/// Prepare a WASM-dispatched kernel, or `Ok(None)` when the report has none.
pub(crate) fn prepare_wasm_prefix(
    source: &str,
    report: &PipelineReport,
    limits: PrefixLimits,
) -> Result<Option<PrefixPreparation>, PrefixPrepareError> {
    let invalid = |what: String| PrefixPrepareError::InvalidFact(format!("WASM kernel: {what}"));
    let proof = |what: String| PrefixPrepareError::SourceProof(format!("WASM kernel: {what}"));
    if !report.output.contains("WebAssembly") {
        return Ok(None);
    }
    // Which JavaScript function each import is, read where the modules are
    // instantiated: the presentation adapter's copies are not proof.
    let view = crate::transforms::deob::pipeline::wasm_proof_report(source)
        .map_err(|error| invalid(format!("proof view: {error}")))?;
    let table = report
        .proven_operation_tables
        .first()
        .cloned()
        .ok_or_else(|| invalid("no proven operation table".into()))?;
    let admitted = admit(&view, &table).map_err(proof)?;
    let lifted = wasm_lift::lift(&admitted.interpreter, &admitted.roles).map_err(proof)?;
    check_wrappers(&view, &lifted).map_err(proof)?;
    let returns = run_statuses(&view, &lifted).map_err(proof)?;
    if let Some(selector) = (0..=255u8).find(|selector| {
        matches!(lifted.op(*selector), SelectorOp::Operation { operation, .. } if *operation >= admitted.operations)
    }) {
        return Err(proof(format!("selector {selector} calls past the handler table")));
    }

    let config = runtime_config::from_report(source, report, Default::default())
        .map_err(|error| PrefixPrepareError::RuntimeConfig(error.to_string()))?;
    let images = config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == runtime_config::ArtifactKind::WasmMemoryImage
                && artifact.status == runtime_config::FactStatus::Extracted
                && artifact.bytes.is_some()
        })
        .collect::<Vec<_>>();
    let [image] = images.as_slice() else {
        return Err(invalid(format!(
            "{} extracted memory images, not one",
            images.len()
        )));
    };
    let span = OperandSpan {
        start: image.span.start,
        end: image.span.end,
    };

    // Linear memory: the image at address 0, zeros after.
    let image = image.bytes.as_deref().unwrap_or_default();
    if image.len() > lifted.memory_bytes {
        return Err(invalid(
            "the image is larger than the interpreter's memory".into(),
        ));
    }
    let mut memory = image.to_vec();
    memory.resize(lifted.memory_bytes, 0);

    // Every state and opcode byte, as the run function reads them; a cell
    // past memory traps there.
    let trap = trap_selector(&lifted).map_err(invalid)?;
    let cells = (0..256usize)
        .map(|state| {
            (0..256usize)
                .map(|opcode| {
                    let cell = lifted.base as usize + state * lifted.stride as usize + opcode * 2;
                    match memory.get(cell..cell + 2) {
                        Some(&[selector, next_state]) => DispatchCell {
                            selector: selector.into(),
                            next_state: next_state.into(),
                        },
                        _ => DispatchCell {
                            selector: trap,
                            next_state: 0,
                        },
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut templates =
        crate::handler_templates_from_report(report).map_err(|error| invalid(error.to_string()))?;
    if templates.table != table {
        return Err(invalid("the operation templates name another table".into()));
    }
    let operations = std::mem::take(&mut templates.handlers);
    let identity = OperandInputIdentity {
        binding_name: "vmLinearMemory".into(),
        declaration_span: span.clone(),
    };
    let record = SpanRecord {
        start: span.start,
        end: span.end,
    };
    let mut handlers = Vec::new();
    let mut patch = None;
    let mut number_tables = Vec::new();
    for selector in 0..=255u8 {
        let op = lifted.op(selector);
        let (template, operands, pc_delta) = match op {
            SelectorOp::Operation {
                operation,
                operands,
                length,
            } => {
                let template = operations
                    .iter()
                    .find(|template| template.handler_index == *operation)
                    .cloned()
                    .ok_or_else(|| invalid(format!("operation {operation} has no recipe")))?;
                (template, operands.clone(), *length)
            }
            SelectorOp::ImageWrite { address, value } => {
                if (*address, *value)
                    != (
                        Operand {
                            offset: 0,
                            width: 3,
                        },
                        Operand {
                            offset: 3,
                            width: 1,
                        },
                    )
                {
                    return Err(invalid(
                        "the image write's operands are not an address and a byte".into(),
                    ));
                }
                patch = Some(selector);
                (
                    unsupported(selector, &record, "runs as the image patch handler"),
                    vec![*address, *value],
                    4,
                )
            }
            SelectorOp::PushTableNumber { table, .. } => {
                number_tables.push(*table);
                bookkeeping(selector, op, &record, &admitted.literals, &returns)
            }
            _ => bookkeeping(selector, op, &record, &admitted.literals, &returns),
        };
        let operands = operands
            .iter()
            .enumerate()
            .map(|(index, operand)| OperandDescriptor {
                position: index as u32 + 1,
                byte_offset: operand.offset,
                width: operand.width,
                endian: OperandEndian::Little,
                signed: false,
                mask: None,
                pc_delta,
                source_spans: Vec::new(),
                input_identity: identity.clone(),
            })
            .collect();
        let mut template = template;
        template.handler_index = selector.into();
        handlers.push(LiftedHandler {
            handler_index: selector.into(),
            table_key: template.table_key,
            source_span: span.clone(),
            entry_coordinate: OperandEntryCoordinate::AfterOpcode,
            operands,
            pc_delta,
            captured_dependencies: Vec::new(),
            template: template.clone(),
            status: OperandHandlerStatus::Lifted,
            unresolved: Vec::new(),
        });
        templates.handlers.push(template);
    }
    let patch = patch.ok_or_else(|| invalid("the interpreter writes no image byte".into()))?;
    let number_table_base = match number_tables.as_slice() {
        [] => None,
        [base, rest @ ..] if rest.iter().all(|other| other == base) => Some(*base),
        _ => return Err(invalid("numbers are read from more than one table".into())),
    };
    // An f64 at each byte index the operand can name.
    let number_table = match number_table_base {
        Some(base) => (0..256usize)
            .map(|index| {
                let at = base as usize + index * 8;
                memory
                    .get(at..at + 8)
                    .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("eight bytes")))
                    .ok_or_else(|| invalid("the number table runs past memory".into()))
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let frozen_from = number_table_base.map_or(lifted.base, |base| base.min(lifted.base)) as usize;

    let lifted_count = handlers.len() as u32;
    let operands = JsOperandSeparationReport {
        schema_version: crate::operations::operands::SCHEMA_VERSION.into(),
        source_hash: report.input_hash.clone(),
        table: table.clone(),
        input_identity: Some(identity),
        entry_coordinate: Some(OperandEntryCoordinate::AfterOpcode),
        handlers,
        coverage: OperandCoverage {
            lifted: lifted_count,
            ..OperandCoverage::default()
        },
        unresolved: Vec::new(),
    };
    let template_index = super::super::step::SelectorIndex::from_handler_indices(
        templates
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );
    let operand_index = super::super::step::SelectorIndex::from_handler_indices(
        operands
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );
    Ok(Some(PrefixPreparation {
        source_hash: config.source.digest.clone(),
        artifact_source_hash: config.artifact_source.digest.clone(),
        initial_image: memory,
        table: cells,
        patch_handler: PatchHandlerProof {
            selector: patch.into(),
            span: span.clone(),
            operand_width: super::PATCH_OPERAND_WIDTH,
        },
        templates,
        operands,
        template_index,
        operand_index,
        limits,
        bytecode_span: span.clone(),
        run_loop: RunLoopProof::runtime_only(span, "vmRun".into(), "errorStack".into(), None),
        operation_table: table,
        constant_tables: super::super::prepare::constant_string_tables(&config),
        number_tables: super::super::prepare::constant_number_tables(&config),
        string_decoder: super::super::prepare::string_decoder(report),
        wasm: Some(WasmMachine {
            frozen_from,
            number_table,
            try_frames: TRY_FRAMES as usize,
            error_records: ERROR_RECORDS as usize,
            call_frames: 1,
        }),
        junk_entries: Default::default(),
    }))
}

/// A selector that traps: the interpreter's default.
fn trap_selector(lifted: &wasm_lift::Lifted) -> Result<u32, String> {
    (0..=255u8)
        .find(|selector| *lifted.op(*selector) == SelectorOp::Trap)
        .map(u32::from)
        .ok_or_else(|| "the interpreter has no trapping selector".into())
}

/// What the proof view shows of a WASM kernel's two modules.
pub(crate) struct Admitted {
    /// The interpreter module's bytes.
    pub(crate) interpreter: Vec<u8>,
    /// What each of its helper imports does, by import name.
    pub(crate) roles: BTreeMap<String, Helper>,
    /// The literal-push helper's kinds.
    literals: BTreeMap<u32, SemanticValue>,
    /// How many operations its handler table holds.
    operations: u32,
}

/// The interpreter (the module importing linear memory and the handler
/// table) and the handler module (the instance exporting that table), with
/// every handler import `k` proven to be `vmOperations[k]` and every helper
/// import named by its function.
pub(crate) fn admit(
    view: &PipelineReport,
    table: &crate::ProvenOperationTable,
) -> Result<Admitted, String> {
    let analysis = boundary::analyze_report(view);
    let view_table = view
        .proven_operation_tables
        .first()
        .ok_or("the proof view proves no operation table")?;
    if view_table.canonical_name != table.canonical_name
        || view_table.initializer_elements != table.initializer_elements
    {
        return Err("the proof view's operation table differs".into());
    }
    let interpreters = analysis
        .modules
        .iter()
        .filter(|module| {
            module.validated
                && module.imports.iter().any(|import| import.kind == "memory")
                && module.imports.iter().any(|import| import.kind == "table")
        })
        .collect::<Vec<_>>();
    let [interpreter] = interpreters.as_slice() else {
        return Err(format!(
            "{} validated modules import a memory and a table, not one",
            interpreters.len()
        ));
    };
    let tables = interpreter
        .imports
        .iter()
        .filter(|import| import.kind == "table")
        .collect::<Vec<_>>();
    let [table_import] = tables.as_slice() else {
        return Err("the interpreter imports more than one table".into());
    };
    let producer = table_import
        .source_instance_id
        .ok_or("the interpreter's table has no proven producing instance")?;
    let handlers = analysis
        .modules
        .iter()
        .filter(|module| {
            module.validated
                && module.instance_id == Some(producer)
                && module.tables.iter().any(|table| {
                    table
                        .exported_as
                        .iter()
                        .any(|name| *name == table_import.name)
                })
        })
        .collect::<Vec<_>>();
    let [handler] = handlers.as_slice() else {
        return Err("no one validated instance exports the interpreter's table".into());
    };
    for import in handler
        .imports
        .iter()
        .filter(|import| import.kind == "function")
    {
        let linked = import.status == "proven"
            && import.operation_table.as_ref().is_some_and(|link| {
                link.declaration_ordinal == view_table.declaration_ordinal
                    && link.property == import.name
            });
        if !linked {
            return Err(format!(
                "handler import {}.{} is not vmOperations[{}]",
                import.namespace, import.name, import.name
            ));
        }
    }
    let mappings = &handler.handler_mappings;
    if mappings.is_empty()
        || mappings.iter().any(|mapping| {
            mapping.status != "proven"
                || mapping.original_handler_key != Some(mapping.element_index)
        })
    {
        return Err("the handler table does not map element k to operation k".into());
    }

    let mut roles = BTreeMap::new();
    let mut literals = None;
    for import in interpreter
        .imports
        .iter()
        .filter(|import| import.kind == "function")
    {
        let source = import
            .source
            .as_ref()
            .and_then(|link| view.output.get(link.start as usize..link.end as usize))
            .map(normalize)
            .ok_or_else(|| format!("helper {}.{} has no source", import.namespace, import.name))?;
        let body = source
            .strip_prefix(&format!("{}: ", import.name))
            .or_else(|| source.strip_prefix(&format!("{:?}: ", import.name)))
            .ok_or_else(|| {
                format!(
                    "helper {}.{} is not a property",
                    import.namespace, import.name
                )
            })?;
        let (role, kinds) = helper_role(view, body)?;
        if let Some(kinds) = kinds {
            if literals.replace(kinds).is_some() {
                return Err("two helpers push literals".into());
            }
        }
        roles.insert(import.name.clone(), role);
    }
    Ok(Admitted {
        interpreter: module_bytes(view, interpreter)?.to_vec(),
        roles,
        literals: literals.unwrap_or_default(),
        operations: mappings.len() as u32,
    })
}

/// The decoded bytes of `module`, from the view's extraction facts.
fn module_bytes<'r>(
    view: &'r PipelineReport,
    module: &boundary::WasmModuleBoundary,
) -> Result<&'r [u8], String> {
    let binding = module
        .source_binding
        .as_deref()
        .ok_or("the interpreter has no source binding")?;
    let found = view
        .extraction
        .iter()
        .filter(|fact| {
            fact.kind == crate::ExtractionKind::WasmModule
                && fact.status == crate::ExtractionStatus::Extracted
                && fact.binding.as_deref() == Some(binding)
        })
        .filter_map(|fact| fact.decoded_bytes.as_deref())
        .collect::<Vec<_>>();
    let [bytes] = found.as_slice() else {
        return Err(format!("{} decoded interpreters, not one", found.len()));
    };
    if crate::util::fnv1a64_hex(bytes) != module.module_hash {
        return Err("the interpreter's decoded bytes are not the analysed module".into());
    }
    Ok(bytes)
}

/// What a helper import's function does, and for the literal push its
/// kinds.
fn helper_role(
    view: &PipelineReport,
    body: &str,
) -> Result<(Helper, Option<BTreeMap<u32, SemanticValue>>), String> {
    const PUSH: &str = "Array.prototype.push.call(context.stack, ";
    let exact: &[(&str, Helper)] = &[
        (
            "function(context) { return !!context.stack.pop(); }",
            Helper::Truth,
        ),
        (
            "function(context, operand1) { Array.prototype.push.call(context.stack, operand1); }",
            Helper::Number,
        ),
        (
            "function(context, operand1) { context.memory.writeSlot(operand1, context.errorStack.pop()); }",
            Helper::CatchBind,
        ),
        (
            "function(context) { throw context.errorStack.pop(); }",
            Helper::Rethrow,
        ),
    ];
    if let Some((_, role)) = exact.iter().find(|(text, _)| *text == body) {
        return Ok((*role, None));
    }
    match body {
        "vmPopStack" => {
            if declared_function(&view.output, "vmPopStack")?
                != "function(context) { return context.stack.pop(); }"
            {
                return Err("vmPopStack is not a stack pop".into());
            }
            return Ok((Helper::Pop, None));
        }
        // Checked with the run function's exports (see `check_wrappers`).
        "vmInvokeOperation" => return Ok((Helper::Invoke, None)),
        _ => {}
    }
    // `var x = top; length -= n; top = x`, for any name `x`.
    if let Some(rest) = body.strip_prefix("function(context, operand1) { var ") {
        if let Some((name, rest)) = rest.split_once(' ') {
            if rest
                == format!(
                    "= context.errorStack[context.errorStack.length - 1]; context.errorStack.length -= operand1; context.errorStack[context.errorStack.length - 1] = {name}; }}"
                )
            {
                return Ok((Helper::DropUnder, None));
            }
        }
    }
    // `switch (operand1) { case k: push(literal); return; … }`.
    if let Some(cases) = body
        .strip_prefix("function(context, operand1) { switch (operand1) { ")
        .and_then(|rest| rest.strip_suffix(" } }"))
    {
        let mut kinds = BTreeMap::new();
        for case in cases
            .split(" return; ")
            .map(|case| case.trim_end_matches(" return;"))
        {
            let (kind, push) = case
                .strip_prefix("case ")
                .and_then(|case| case.split_once(": "))
                .ok_or("a literal case is not `case k: push`")?;
            let literal = push
                .strip_prefix(PUSH)
                .and_then(|push| push.strip_suffix(");"))
                .ok_or("a literal case does not push")?;
            let value = match literal {
                "{}" => SemanticValue::ObjectLiteral {
                    properties: Vec::new(),
                },
                "[]" => SemanticValue::ArrayLiteral {
                    elements: Vec::new(),
                },
                "true" => SemanticValue::Literal(LiteralValue::Boolean(true)),
                "false" => SemanticValue::Literal(LiteralValue::Boolean(false)),
                "void 0" => SemanticValue::Literal(LiteralValue::Undefined),
                "null" => SemanticValue::Literal(LiteralValue::Null),
                "Infinity" => number(f64::INFINITY),
                other => return Err(format!("a literal case pushes {other}")),
            };
            let kind: u32 = kind
                .parse()
                .map_err(|_| "a literal case's kind is not a number")?;
            if kinds.insert(kind, value).is_some() {
                return Err("two literal cases share a kind".into());
            }
        }
        return Ok((Helper::Literal, Some(kinds)));
    }
    Err(format!("a helper the lift does not know: {body}"))
}

/// The most operands an operation wrapper is read with.
const MAX_WRAPPER_OPERANDS: usize = 8;

/// The operation wrapper calls `vmOperations[selector]` with the operands it
/// was given, as many as every lifted operation passes, and sets the
/// interpreter's exception flag through its flag export on a throw.
fn check_wrappers(view: &PipelineReport, lifted: &wasm_lift::Lifted) -> Result<(), String> {
    let declared = declared_function(&view.output, "vmInvokeOperation")?;
    let most = lifted
        .selectors
        .iter()
        .map(|op| match op {
            wasm_lift::SelectorOp::Operation { operands, .. } => operands.len(),
            _ => 0,
        })
        .max()
        .unwrap_or(0);
    let guarded = (most.max(1)..=MAX_WRAPPER_OPERANDS).any(|count| {
        let operands = (1..=count)
            .map(|index| format!("operand{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        declared
            == format!(
                r#"function(context, {operands}) {{ "use strict"; try {{ vmOperations[arguments[arguments.length - 1]](context, {operands}); }} catch (error) {{ (0, vmInterpreterInstance.exports.{})(); context.errorStack.push(error); }} }}"#,
                lifted.flag_export
            )
    });
    if !guarded {
        return Err("vmInvokeOperation is not the guarded operation call the lift assumes".into());
    }
    Ok(())
}

/// What `vmRun` returns for each status the run function returns: `true`
/// when it pops the result, `false` when it is `undefined`.
fn run_statuses(
    view: &PipelineReport,
    lifted: &wasm_lift::Lifted,
) -> Result<BTreeMap<i32, bool>, String> {
    let run = declared_function(&view.output, "vmRun")?;
    let head = format!(
        "function(context) {{ var executionStatus = (0, vmInterpreterInstance.exports.{})(context.byteOffset, context.dispatchState, context); switch (executionStatus) {{ ",
        lifted.run_export
    );
    let cases = run
        .strip_prefix(&head)
        .and_then(|rest| rest.strip_suffix(" } }"))
        .ok_or("vmRun is not a switch on the run function's status")?;
    let mut statuses = BTreeMap::new();
    let mut rest = cases;
    while !rest.is_empty() {
        let (status, after) = rest
            .strip_prefix("case ")
            .and_then(|case| case.split_once(": "))
            .ok_or("a vmRun case is not `case k: { … }`")?;
        let (pops, after) = if let Some(after) = after.strip_prefix("{ return; }") {
            (false, after)
        } else if let Some(after) = after.strip_prefix("{ return context.stack.pop(); }") {
            (true, after)
        } else {
            return Err(format!("a vmRun case does {after}"));
        };
        let status: i32 = status
            .parse()
            .map_err(|_| "a vmRun status is not a number")?;
        if statuses.insert(status, pops).is_some() {
            return Err("vmRun reads one status twice".into());
        }
        rest = after.trim_start();
    }
    Ok(statuses)
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The initializer of the one `var name = function …` in `source`.
fn declared_function(source: &str, name: &str) -> Result<String, String> {
    use oxc_ast::ast::{BindingPattern, VariableDeclarator};
    use oxc_ast_visit::{walk, Visit};
    use oxc_span::GetSpan;
    struct Find<'n> {
        name: &'n str,
        found: Vec<oxc_span::Span>,
    }
    impl<'a> Visit<'a> for Find<'_> {
        fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
            if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&declarator.id, declarator.init.as_ref())
            {
                if id.name == self.name {
                    self.found.push(init.span());
                }
            }
            walk::walk_variable_declarator(self, declarator);
        }
    }
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)?;
    let mut find = Find {
        name,
        found: Vec::new(),
    };
    find.visit_program(&program);
    let [span] = find.found.as_slice() else {
        return Err(format!("{name} is not declared exactly once"));
    };
    Ok(normalize(&source[span.start as usize..span.end as usize]))
}

/// A template that never runs: the walker stops with `reason` there.
/// Why a selector the interpreter traps on has no handler.
pub(crate) const TRAP_REASON: &str = "the WASM interpreter traps (unreachable) here";

fn unsupported(selector: u8, span: &SpanRecord, reason: &str) -> HandlerTemplate {
    HandlerTemplate {
        handler_index: selector.into(),
        table_key: selector.into(),
        semantic_identity: format!("wasm:{selector}").into(),
        source_span: span.clone(),
        status: TemplateStatus::Unsupported,
        steps: Vec::new(),
        unsupported: vec![UnknownFragment {
            span: span.clone(),
            reason: reason.into(),
            source: String::new(),
        }],
    }
}

fn number(value: f64) -> SemanticValue {
    SemanticValue::Literal(LiteralValue::DecodedNumber {
        bits: value.to_bits(),
    })
}

/// `context.stack.length - k`.
fn below(k: f64) -> SemanticValue {
    SemanticValue::Binary {
        operator: PrimitiveOperator::Subtract,
        left: Box::new(SemanticValue::StackLength),
        right: Box::new(number(k)),
    }
}

/// `context.stack[context.stack.length - k]`.
fn top(k: f64) -> SemanticValue {
    SemanticValue::StackRead {
        index: Box::new(below(k)),
    }
}

fn operand(position: u32) -> SemanticValue {
    SemanticValue::Operand { position }
}

fn temporary(id: u32) -> SemanticValue {
    SemanticValue::Temporary { id }
}

/// `context.stack[context.stack.length] = value`.
fn push(value: SemanticValue) -> RecipeOperation {
    RecipeOperation::StackWrite {
        index: SemanticValue::StackLength,
        value,
    }
}

/// The recipe a JavaScript VM's handler for the same operation has (see
/// the module docs), its operands and how far past them the VM goes on.
fn bookkeeping(
    selector: u8,
    op: &SelectorOp,
    span: &SpanRecord,
    literals: &BTreeMap<u32, SemanticValue>,
    returns: &BTreeMap<i32, bool>,
) -> (HandlerTemplate, Vec<Operand>, u32) {
    use crate::operations::templates::{CallFrameField, ErrorRecordField, TryFrameField};
    use RecipeOperation as R;
    let first_two = || {
        vec![
            R::Let {
                temporary: 0,
                value: operand(1),
            },
            R::Let {
                temporary: 1,
                value: operand(2),
            },
        ]
    };
    let (operations, operands, pc_delta) = match op {
        SelectorOp::PushLiteral { kind } => {
            let Some(value) = literals.get(kind) else {
                return (
                    unsupported(
                        selector,
                        span,
                        "pushes a literal kind the helper has no case for",
                    ),
                    Vec::new(),
                    0,
                );
            };
            (vec![push(value.clone())], Vec::new(), 0)
        }
        SelectorOp::PushNumber { operand: read } => (
            vec![push(operand(1))],
            vec![*read],
            read.offset + read.width as u32,
        ),
        SelectorOp::PushTableNumber { operand: read, .. } => (
            vec![push(SemanticValue::PropertyRead {
                object: Box::new(SemanticValue::Binding {
                    name: NUMBER_TABLE.into(),
                }),
                key: Box::new(operand(1)),
            })],
            vec![*read],
            read.offset + read.width as u32,
        ),
        SelectorOp::Jump { pc, state } => {
            let mut operations = first_two();
            operations.push(R::SetProgramCounter {
                value: temporary(0),
            });
            operations.push(R::SetDispatchState {
                value: temporary(1),
            });
            (operations, vec![*pc, *state], 0)
        }
        SelectorOp::JumpIf {
            negated,
            pc,
            state,
            length,
        } => {
            let condition = if *negated {
                SemanticValue::Unary {
                    operator: PrimitiveOperator::LogicalNot,
                    value: Box::new(top(1.0)),
                }
            } else {
                top(1.0)
            };
            let mut operations = first_two();
            operations.push(R::JumpIf {
                condition,
                byte_offset: temporary(0),
                dispatch_state: temporary(1),
            });
            operations.push(R::StackLengthSet { value: below(1.0) });
            (operations, vec![*pc, *state], *length)
        }
        SelectorOp::PopJump => (
            vec![
                R::SetProgramCounter { value: top(1.0) },
                R::SetDispatchState { value: top(2.0) },
                R::StackLengthSet { value: below(2.0) },
            ],
            Vec::new(),
            0,
        ),
        // `vmRun` pops the result for the statuses it says so of; for any
        // other it falls out of its switch and returns `undefined`.
        SelectorOp::Return { status } if returns.get(status) == Some(&true) => (
            vec![
                R::CompleteThread { value: top(1.0) },
                R::StackLengthSet { value: below(1.0) },
            ],
            Vec::new(),
            0,
        ),
        SelectorOp::Return { .. } => (
            vec![R::CompleteThread {
                value: SemanticValue::Literal(LiteralValue::Undefined),
            }],
            Vec::new(),
            0,
        ),
        SelectorOp::TryEnter { pc, state, length } => {
            let mut operations = first_two();
            operations.push(R::PushTryFrame {
                byte_offset: temporary(0),
                dispatch_state: temporary(1),
            });
            (operations, vec![*pc, *state], *length)
        }
        SelectorOp::TryLeave => (vec![R::PopTryFrame], Vec::new(), 0),
        SelectorOp::FinallyEnter => (
            vec![
                R::PushFinallyRecord,
                R::PopTryFrame,
                R::SetProgramCounter {
                    value: SemanticValue::PoppedTryFrame {
                        field: TryFrameField::HandlerByteOffset,
                    },
                },
                R::SetDispatchState {
                    value: SemanticValue::PoppedTryFrame {
                        field: TryFrameField::HandlerDispatchState,
                    },
                },
            ],
            Vec::new(),
            0,
        ),
        SelectorOp::FinallyLeave => (
            vec![
                R::PopErrorRecord,
                R::RethrowIfHandled,
                R::SetProgramCounter {
                    value: SemanticValue::PoppedErrorRecord {
                        field: ErrorRecordField::SavedByteOffset,
                    },
                },
                R::SetDispatchState {
                    value: SemanticValue::PoppedErrorRecord {
                        field: ErrorRecordField::SavedDispatchState,
                    },
                },
            ],
            Vec::new(),
            0,
        ),
        SelectorOp::CatchDrop => (
            vec![R::PopErrorRecord, R::DropThrownUnderIfHandled],
            Vec::new(),
            0,
        ),
        SelectorOp::CatchBind { slot } => (
            vec![
                R::Let {
                    temporary: 0,
                    value: operand(1),
                },
                R::PopErrorRecord,
                R::MemoryWrite {
                    slot: temporary(0),
                    value: SemanticValue::PopThrownValue,
                },
            ],
            vec![*slot],
            slot.offset + slot.width as u32,
        ),
        SelectorOp::Call { pc, state, length } => {
            let mut operations = first_two();
            operations.push(R::PushCallFrame);
            operations.push(R::SetProgramCounter {
                value: temporary(0),
            });
            operations.push(R::SetDispatchState {
                value: temporary(1),
            });
            (operations, vec![*pc, *state], *length)
        }
        SelectorOp::CallReturn => (
            vec![
                R::PopCallFrame,
                R::SetProgramCounter {
                    value: SemanticValue::PoppedCallFrame {
                        field: CallFrameField::ReturnByteOffset,
                    },
                },
                R::SetDispatchState {
                    value: SemanticValue::PoppedCallFrame {
                        field: CallFrameField::ReturnDispatchState,
                    },
                },
            ],
            Vec::new(),
            0,
        ),
        SelectorOp::Trap => return (unsupported(selector, span, TRAP_REASON), Vec::new(), 0),
        SelectorOp::Operation { .. } | SelectorOp::ImageWrite { .. } => {
            unreachable!("handled by the caller")
        }
    };
    let steps = operations
        .into_iter()
        .enumerate()
        .map(|(order, operation)| RecipeStep {
            order: order as u32,
            span: span.clone(),
            operation,
        })
        .collect();
    (
        HandlerTemplate {
            handler_index: selector.into(),
            table_key: selector.into(),
            semantic_identity: format!("wasm:{op:?}").into(),
            source_span: span.clone(),
            status: TemplateStatus::Executable,
            steps,
            unsupported: Vec::new(),
        },
        operands,
        pc_delta,
    )
}
