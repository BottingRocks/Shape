//! Thread descriptor helpers shared by the thread, program and corpus walks.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(test)]
use crate::PropertyValue;
use crate::emitted_cleanup::{
    simplify_cleaned_emitted_property_writes, simplify_emitted_property_writes,
};
use crate::{
    deobfuscate_with_report, emit_javascript, DescriptorRecord, DispatchStart, EmitOptions,
    KernelArtifact, PrefixLimits, PrefixStopReason, PrefixTraceReport, SymbolicValue,
    ThreadDescriptor, ThreadInputs, TraceLimits,
};

/// The slots each thread descriptor starts with, in table order: `true`
/// for one it initializes to `undefined`, `false` for one its caller fills
/// (a captured cell, an argument, the `arguments` object). An invalid row
/// starts with none.
pub fn thread_starts(records: &[DescriptorRecord]) -> Arc<[Vec<(u32, bool)>]> {
    records
        .iter()
        .map(|record| {
            record
                .descriptor
                .as_ref()
                .map(|descriptor| {
                    // As the invocation sets them up: the caller's slots,
                    // then the initialized ones over them.
                    descriptor
                        .captured_slots
                        .iter()
                        .chain(&descriptor.argument_slots)
                        .chain(&descriptor.arguments_slot)
                        .map(|&slot| (slot, false))
                        .chain(descriptor.initial_slots.iter().map(|&slot| (slot, true)))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// The slots each thread descriptor captures, in table order; an invalid
/// row captures none.
pub fn thread_captures(records: &[DescriptorRecord]) -> Arc<[Vec<u32>]> {
    records
        .iter()
        .map(|record| {
            record
                .descriptor
                .as_ref()
                .map(|descriptor| descriptor.captured_slots.clone())
                .unwrap_or_default()
        })
        .collect()
}

/// Everything a descriptor's thread receives from its caller is unknown:
/// one positional argument per argument slot, each captured slot, the
/// `arguments` object, the thread's own function and its receiver.
pub fn unknown_inputs(
    descriptor: &ThreadDescriptor,
    mut inputs: ThreadInputs,
) -> (ThreadInputs, Vec<SymbolicValue>) {
    let external = |name: String| SymbolicValue::External { name };
    for &slot in &descriptor.captured_slots {
        inputs = inputs.capture(slot, external(format!("captured{slot}")));
    }
    if descriptor.arguments_slot.is_some() {
        inputs = inputs.external_arguments_object("args");
    }
    if descriptor.result_slot.is_some() {
        inputs = inputs.external_self_value("callee");
    }
    inputs = inputs.receiver(external("receiver".into()));
    let arguments = (0..descriptor.argument_slots.len())
        .map(|index| external(format!("arg{index}")))
        .collect();
    (inputs, arguments)
}

/// The non-argument input names [`unknown_inputs`] gives a descriptor, in
/// parameter order.
pub fn external_names(descriptor: &ThreadDescriptor) -> Vec<String> {
    let mut names = Vec::new();
    if descriptor.arguments_slot.is_some() {
        names.push("args".into());
    }
    if descriptor.result_slot.is_some() {
        names.push("callee".into());
    }
    names.push("receiver".into());
    names.extend(
        descriptor
            .captured_slots
            .iter()
            .map(|slot| format!("captured{slot}")),
    );
    names
}

/// What one `shape trace thread` walk takes besides the kernel artifact.
#[derive(Clone, Debug, Default)]
pub struct ThreadOptions {
    pub pc: usize,
    pub state: usize,
    /// Take the thread's slots from `vmThreadDescriptors[descriptor]`.
    pub descriptor: Option<usize>,
    /// Slots initialized to `undefined` (replaces the descriptor's).
    pub initial_slots: Option<Vec<u32>>,
    /// An explicit input snapshot (arguments, captures, stack, receiver).
    pub inputs: Option<crate::trace::inputs::Snapshot>,
    pub steps: usize,
    pub self_check: bool,
    pub outline: bool,
    pub reverse_sides: bool,
    /// Emit what was traced even when the walk stopped early.
    pub partial: bool,
    pub overrides: crate::BranchOverrides,
}

/// One walked thread: the trace report, its JavaScript and the emit counts.
pub struct ThreadWalk {
    pub report: PrefixTraceReport,
    pub completed: bool,
    /// The emitted function (headed `// INCOMPLETE:` for a partial walk), or
    /// `None` when the walk stopped and `partial` was off.
    pub code: Option<String>,
    pub string_folds: Option<crate::vm_string::VmStringFoldReport>,
    /// (simplified, retained) property writes.
    pub property_writes: Option<(usize, usize)>,
    /// (compacted arrays, removed writes).
    pub array_constructions: Option<(usize, usize)>,
    /// Generated names the function reads where none is declared.
    pub undeclared: Vec<String>,
    pub prepare: Duration,
    pub trace: Duration,
    pub emit: Option<Duration>,
}

/// Compile the source-free kernel artifact of one raw capture.
pub fn compile_kernel_artifact(source: &str) -> Result<KernelArtifact, String> {
    let report = deobfuscate_with_report(source)
        .map_err(|error| format!("deobfuscation failed: {error}"))?;
    KernelArtifact::compile_from_raw(source, &report, PrefixLimits::default())
        .map_err(|error| format!("kernel artifact compilation failed: {error}"))
}

/// Walk one VM thread of `artifact` and emit the function it becomes.
pub fn walk_thread(artifact: &KernelArtifact, args: &ThreadOptions) -> Result<ThreadWalk, String> {
    let started = Instant::now();
    let prepared = std::sync::Arc::new(
        artifact
            .prepare_with_limits(PrefixLimits {
                max_dispatch_steps: args.steps,
                self_check: args.self_check,
                outline_subroutines: args.outline,
                reverse_sides: args.reverse_sides,
                ..PrefixLimits::default()
            })
            .map_err(|error| format!("not a supported kernel artifact: {error}"))?,
    );
    let prepare = started.elapsed();

    let started = Instant::now();
    // The tracer's own bounds grow with the requested steps.
    let inputs = ThreadInputs::new().limits(TraceLimits {
        max_steps: args.steps.saturating_mul(32).max(10_000),
        max_nodes: args
            .steps
            .saturating_mul(64)
            .clamp(50_000, crate::trace::corpus::MAX_WALK_NODES),
        ..TraceLimits::default()
    });
    let (descriptor, inputs, arguments) = match args.descriptor {
        Some(index) => {
            let record = artifact.descriptor_records().get(index).ok_or_else(|| {
                format!(
                    "--descriptor {index} is out of range: the table has {} rows",
                    artifact.descriptor_records().len()
                )
            })?;
            let mut descriptor = match (&record.descriptor, &record.invalid) {
                (Some(descriptor), _) => descriptor.clone(),
                (None, Some(error)) => {
                    return Err(format!("descriptor {index} is invalid: {error}"))
                }
                (None, None) => return Err(format!("descriptor {index} has no descriptor facts")),
            };
            if let Some(slots) = &args.initial_slots {
                descriptor.initial_slots = slots.clone();
            }
            let (inputs, arguments) = unknown_inputs(&descriptor, inputs);
            (descriptor, inputs, arguments)
        }
        None => (
            ThreadDescriptor::new().initial_slots(args.initial_slots.iter().flatten().copied()),
            inputs,
            Vec::new(),
        ),
    };
    let (descriptor, inputs, arguments) = if let Some(snapshot) = args.inputs.clone() {
        if snapshot.descriptor.is_some() {
            return Err("thread --inputs uses the descriptor selected by --descriptor or --initialslots; omit descriptor from the snapshot".into());
        }
        let (explicit_inputs, arguments) = snapshot.into_inputs(inputs.limits)?;
        (descriptor, explicit_inputs, arguments)
    } else {
        (descriptor, inputs, arguments)
    };
    let parameters = arguments
        .iter()
        .filter_map(|value| match value {
            SymbolicValue::External { name } => Some(name.clone()),
            _ => None,
        })
        .chain(external_names(&descriptor))
        .collect::<Vec<_>>();
    let overrides = std::sync::Arc::new(args.overrides.clone());
    let thread = prepared
        .thread(
            DispatchStart {
                pc: args.pc,
                state: args.state,
            },
            descriptor,
            inputs,
        )
        .map_err(|error| format!("thread startup rejected: {error}"))?
        .with_thread_captures(thread_captures(&artifact.descriptor_records()))
        .with_thread_starts(thread_starts(&artifact.descriptor_records()))
        .with_branch_overrides(overrides.clone())
        ;
    let asked = std::sync::Arc::new(
        crate::trace::return_shapes::Round::new(
            prepared.clone(),
            artifact.descriptor_records().into(),
            artifact.entry_initial_slots().into(),
            thread_captures(&artifact.descriptor_records()),
            args.steps,
            std::sync::Arc::default(),
            std::sync::Arc::default(),
            &std::collections::BTreeMap::new(),
        )
        .with_overrides(overrides),
    );
    let thread = thread
        .with_returns_hook(Some(asked.asking()))
        .with_cell_types_hook(Some(asked.asking_cell_types()));
    let result = thread
        .invoke(&arguments)
        .map_err(|error| format!("thread invocation rejected: {error}"))?;
    let trace = started.elapsed();

    let mut emit = None;
    let mut string_folds = None;
    let mut property_writes = None;
    let mut array_constructions = None;
    let mut undeclared = Vec::new();
    let completed = result.stop == PrefixStopReason::Completed;
    let mut code_out = None;
    if completed || args.partial {
        let started = Instant::now();
        let mut external_parameters = parameters;
        external_parameters.extend(result.trace.external_inputs.iter().cloned());
        let options = EmitOptions {
            external_parameters,
            simplify_bindings: true,
            allow_incomplete: args.partial,
            ..EmitOptions::default()
        };
        let emitted = emit_javascript(&result.trace, &options)
            .map_err(|error| format!("cannot emit the traced function: {error}"))?;
        let folded = artifact
            .fold_emitted_vm_strings(emitted)
            .map_err(|error| format!("cannot integrate proven VM strings: {error}"))?;
        string_folds = Some(folded.report.clone());
        let emitted = simplify_emitted_property_writes(&folded.emitted)
            .map_err(|error| format!("cannot simplify emitted property writes: {error}"))?;
        property_writes = Some((emitted.outcome.changed, emitted.outcome.skipped.len()));
        let emitted = simplify_cleaned_emitted_property_writes(&emitted)
            .map_err(|error| format!("cannot compact emitted array constructions: {error}"))?;
        array_constructions = Some((
            emitted.outcome.compacted_arrays,
            emitted.outcome.removed_writes,
        ));
        undeclared = crate::tracing::scope::undeclared_generated_names(&emitted.code)?;
        emit = Some(started.elapsed());
        let code = if completed {
            emitted.code
        } else {
            let detail = result
                .trace
                .diagnostics
                .last()
                .map(|diagnostic| format!(" ({}: {})", diagnostic.code, diagnostic.message))
                .unwrap_or_default();
            let header = format!("// INCOMPLETE: stopped at {}{detail}", result.stop);
            format!("{}\n{}", header.replace('\n', " "), emitted.code)
        };
        code_out = Some(code);
    }
    Ok(ThreadWalk {
        report: result,
        completed,
        code: code_out,
        string_folds,
        property_writes,
        array_constructions,
        undeclared,
        prepare,
        trace,
        emit,
    })
}

/// Read one `vmThreadDescriptors` row.
#[cfg(test)]
fn descriptor_row(row: &PropertyValue) -> Result<ThreadDescriptor, String> {
    let PropertyValue::Object(fields) = row else {
        return Err("the row is not an object".into());
    };
    let slot = |value: &PropertyValue| match value {
        PropertyValue::Number(n) if n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(n) => {
            Ok(*n as u32)
        }
        other => Err(format!("{other:?} is not a slot")),
    };
    let list = |name: &str| match fields.get(name) {
        None => Ok(Vec::new()),
        Some(PropertyValue::Array(items)) => items.iter().map(slot).collect(),
        Some(other) => Err(format!("{name} is {other:?}, not a list of slots")),
    };
    let single = |name: &str| fields.get(name).map(slot).transpose();
    Ok(ThreadDescriptor {
        initial_slots: list("initialSlots")?,
        captured_slots: list("capturedSlots")?,
        argument_slots: list("argumentSlots")?,
        arguments_slot: single("argumentsSlot")?,
        result_slot: single("resultSlot")?,
        ..ThreadDescriptor::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_descriptor_row_makes_every_caller_value_an_input() {
        let number = |n: f64| PropertyValue::Number(n);
        let row = PropertyValue::Object(
            [
                ("argumentSlots", PropertyValue::Array(vec![number(1.0)])),
                ("argumentsSlot", number(4.0)),
                (
                    "capturedSlots",
                    PropertyValue::Array(vec![number(2.0), number(391.0)]),
                ),
                (
                    "initialSlots",
                    PropertyValue::Array(vec![number(0.0), number(1.0)]),
                ),
                ("resultSlot", number(6.0)),
            ]
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
        );
        let descriptor = descriptor_row(&row).unwrap();
        assert_eq!(descriptor.argument_slots, [1]);
        assert_eq!(descriptor.captured_slots, [2, 391]);
        assert_eq!(descriptor.initial_slots, [0, 1]);
        assert_eq!(
            (descriptor.arguments_slot, descriptor.result_slot),
            (Some(4), Some(6))
        );
        let (inputs, arguments) = unknown_inputs(&descriptor, ThreadInputs::new());
        assert_eq!(
            arguments,
            [SymbolicValue::External {
                name: "arg0".into()
            }]
        );
        assert_eq!(inputs.captured_memory.len(), 2);
        assert!(inputs.arguments_object.is_some() && inputs.self_value.is_some());
        assert!(inputs.receiver.is_some());
        assert_eq!(
            external_names(&descriptor),
            ["args", "callee", "receiver", "captured2", "captured391"]
        );
        let bad = PropertyValue::Object(
            [(
                "argumentSlots".to_string(),
                PropertyValue::Array(vec![number(1.5)]),
            )]
            .into_iter()
            .collect(),
        );
        assert!(descriptor_row(&bad).is_err());
    }
}
