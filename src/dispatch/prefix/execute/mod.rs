//! Bounded prefix execution over an isolated per-invocation image clone.
//!
//! A conditional jump on a program value forks the walk: both successors are
//! traced as the arms of an AST `if`, in turns of one step each, until they
//! reach the same position, where their VM state is merged. In a turn each
//! side sees the other as it stood when the turn began, so which one runs
//! first decides nothing. A side that stands where the other has been since
//! the fork found the meeting point, cached by the jump's position: the
//! other side is cut back to the step it took from there, keeping what both
//! traced (when that position was inside a region the other side traced as
//! one step, the fork is traced again with it known). A side standing where
//! an enclosing fork's other side has been waits there. A side whose other
//! side ended stays in its arm until it stands where an enclosing fork meets
//! it; there the `if` closes and it carries on after it. Sides that held
//! different concrete values in a cell a later VM jump reads are traced apart
//! until both have read it.
//!
//! Not every conditional jump is a real `if`. A fake branch's test always
//! has the same outcome and its other side leads into code that was never
//! meant to run. The tracer decides some tests outright from what is known
//! about their operands. When it cannot, the walk forks, and a side that
//! reaches a state real code never reaches (an opcode or state with no
//! dispatch cell, a PC or operand outside the image, a stack underflow, a
//! return with no call frame, an empty exception stack) is dropped: the
//! walk rolls back to the jump and follows only the other side. Each jump's
//! record says which kind it was.
//!
//! A path that comes back to a position it already passed (same PC, state
//! and VM call stack) found a loop header. The innermost fork, loop or `try`
//! entered before the header was first passed traces itself again (the whole
//! walk starts over only for a header on the top-level path), and reaching
//! that header opens `label: for (;;) { … }`: every path that comes
//! back to it ends with `continue label`, and one that reaches where an
//! enclosing fork meets it ends with `break label` and carries on after the
//! loop; a path that later comes back to that header was not leaving it,
//! and the loop is traced again without breaking there. VM subroutines are
//! traced once as functions and called; see [`Walk::call`]. Values that change between iterations become loop variables: the
//! first `continue` that shows a new one stops the attempt, and the loop is
//! traced again with it widened (cached by header) until no iteration adds
//! one.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::tracing::symbolic::{StackPadding, VmCursor};

use crate::{
    util::fnv1a64_hex, BranchEvidence, CellRead, DispatchStart, OperandEntryCoordinate,
    OperandHandlerStatus, PendingBranch, PrefixBranch, PrefixBranchKind, StopMarks,
    SubroutineSummary, SymbolicValue, TemplateStatus, TraceCell, TraceContext, TraceDebugOptions,
    TraceFork, TraceHandlerResult, TraceLimits, TraceLoop, TracePath, TraceSession,
    TraceStatementId, TraceStopCode, TryFrame, ValueDispatch,
};

use crate::dispatch::prefix::types::{
    ImagePatchRecord, PrefixExecutionRecord, PrefixRetraces, PrefixStepEffect, PrefixStopReason,
    PrefixTraceReport,
};
use crate::dispatch::prefix::{PrefixPreparation, PATCH_ADDRESS_BYTES, PATCH_OPERAND_WIDTH};
use crate::dispatch::step::{self, StepFault, VisitKey};

/// Loop headers one walk may discover; each one restarts the walk.
const MAX_LOOP_HEADERS: usize = 64;
/// Times a loop body is traced again with more loop variables.
const MAX_LOOP_WIDENINGS: usize = 64;

/// The most steps a fork's sides take after meeting in VM states that cannot
/// merge, still looking for a later meeting point or their ends: past it the
/// walk stops where they first met. Sides kept apart for good (an image one
/// of them patched) trace the rest of the function twice, and every fork in
/// it again.
const MAX_APART_STEPS: usize = 100_000;
/// The most times a loop header found passed by a trace made before a ban
/// makes its region trace itself again.
const MAX_STALE_RETRACES: usize = 4;
/// The most forks a path is traced inside at once with both their sides
/// running. Each doubles the paths below it, so forks nested past this (a
/// chain of value-set dispatches over many cells, say) cost memory without
/// bound. A fork whose other side ended (a `continue`, a `return`) adds no
/// path and does not count.
const MAX_FORK_DEPTH: usize = 48;
/// The step budget of a [`Walk::junk_entry`] check: the straight code from
/// a function's entry it looks at.
const ENTRY_PROBE_STEPS: usize = 200;

/// The position of a side: PC, dispatch state and VM call stack.
fn position(side: &Side) -> VisitKey {
    (
        side.cursor.pc,
        side.cursor.state,
        side.path.call_frames().to_vec(),
    )
}

/// Run one bounded prefix invocation over an isolated image clone.
pub(super) fn run_prefix(
    preparation: &PrefixPreparation,
    start: DispatchStart,
    context: TraceContext,
    trace_limits: TraceLimits,
    debug: TraceDebugOptions,
    sharing: &CellSharing,
) -> PrefixTraceReport {
    // Kept to start over from: see the restart for cells below.
    let initial_context = context.clone();
    let mut session = preparation.constant_tables.iter().fold(
        TraceSession::new_continuing(context, trace_limits)
            .with_debug(debug)
            .with_thread_control(),
        |session, (name, entries)| session.with_constant_table(name.clone(), entries.clone()),
    );
    for (name, entries) in &preparation.number_tables {
        session = session.with_number_table(name.clone(), entries.clone());
    }
    if let Some(decoder) = &preparation.string_decoder {
        session = session.with_string_decoder(decoder.clone());
    }
    if let Some(machine) = &preparation.wasm {
        session = session
            .with_number_table(
                crate::dispatch::prefix::wasm::NUMBER_TABLE,
                machine.number_table(),
            )
            .with_try_depth_traps();
    }
    if let Some(captures) = &sharing.captures {
        session = session.with_captures(captures.clone());
    }
    session.set_branch_overrides(sharing.overrides.clone());
    session = session
        .with_cell_contents(sharing.contents.clone())
        .with_created_hook(sharing.created.clone())
        .with_returns_hook(sharing.returns.clone())
        .with_cell_types_hook(sharing.cell_types.clone());
    // The captured slots are cells from the start; slots a closure created
    // here captures become cells when the walk finds that closure.
    let mut cells: BTreeSet<u32> = sharing.captured.keys().copied().collect();
    // Slots an earlier walk of this function found closures capture: cells
    // from the start, so this walk need not find them again first.
    cells.extend(sharing.cells.iter().copied());
    let initial = session.checkpoint();
    let mut walk = Walk {
        preparation,
        session,
        records: Log::default(),
        patches: Log::default(),
        steps: 0,
        learned: Arc::default(),
        loops: Vec::new(),
        joins: Tags::default(),
        open_tries: Vec::new(),
        finally_handlers: BTreeSet::new(),
        plain_finally: BTreeSet::new(),
        finally_inline: BTreeMap::new(),
        finally_reads: BTreeMap::new(),
        finally_writes: BTreeMap::new(),
        subroutines: BTreeMap::new(),
        fork_jumps: BTreeMap::new(),
        used_sets: BTreeSet::new(),
        last_marks: None,
        retraces: PrefixRetraces::default(),
        self_check: Vec::new(),
        pending_audit: None,
        audit_until: None,
        audit_seen: std::collections::HashMap::new(),
        audit_restarts: 0,
        fake_audits: BTreeMap::new(),
        wasted: BTreeMap::new(),
        summary_records: Vec::new(),
        calls: Tags::default(),
        summarizing: 0,
        catch_depth: 0,
        catch_stop_try: None,
        short_sides: 0,
        overrides: sharing.overrides.clone(),
        no_evidence: false,
        loop_variable_joins: BTreeSet::new(),
        deficit_joins: BTreeSet::new(),
        targets: Vec::new(),
        fork_depth: 0,
        patient: false,
        may_hold: false,
        image_splits: BTreeMap::new(),
        split_bytes: BTreeMap::new(),
        try_handlers: Vec::new(),
        patched_operands: BTreeMap::new(),
        patched_junk: None,
        padded: Vec::new(),
        split_read_at: None,
        probe: sharing.probe,
        starts: sharing.starts.clone(),
        entries: BTreeMap::new(),
        last_junk: None,
    };
    let entry = Cursor {
        pc: start.pc,
        state: start.state,
        image: Arc::new(preparation.initial_image.clone()),
        split_image: BTreeSet::new(),
        visited: Visited::default(),
    };
    let mut wanted_cells = None;
    let (stop, cursor) = loop {
        walk.session.open_cells(&cells, &sharing.captured);
        let mut cursor = entry.clone();
        let ran = walk.run(&mut cursor);
        let wanted = walk.session.take_wanted_cells();
        let ran = if wanted.is_empty() {
            ran
        } else {
            Err(Halt::Cells(wanted))
        };
        let (cause, header) = match ran {
            Ok(()) => break (PrefixStopReason::Completed, cursor),
            // A function made with a junk entry wherever the walk went: code
            // that runs makes it (and never calls it), so it is no evidence.
            Err(Halt::Stop(_))
                if walk.session.last_diagnostic().is_some_and(|diagnostic| {
                    diagnostic.code == TraceStopCode::JunkFunctionEntry
                }) && walk
                    .last_junk
                    .is_some_and(|key| walk.entries.get(&key) == Some(&true)) =>
            {
                let key = walk.last_junk.take().expect("checked");
                walk.entries.insert(key, false);
                ("walk: a junk function the code makes", None)
            }
            Err(Halt::Stop(stop)) => break (stop, cursor),
            Err(Halt::NewLoop(header)) => ("walk: a loop", Some(header)),
            Err(Halt::Split(header)) if walk.learned.inline_calls.contains(&header) => {
                ("walk: a call traced inline", Some(header))
            }
            Err(Halt::Split(header)) => ("walk: a split", Some(header)),
            Err(Halt::Restart) => ("walk: a join undone", None),
            Err(Halt::Cells(wanted)) => {
                wanted_cells = Some(wanted);
                break (PrefixStopReason::Completed, cursor);
            }
            Err(Halt::ForceFake(jump, taken_fake, reason)) => {
                walk.retraces.wrong_breaks += 1;
                Arc::make_mut(&mut walk.learned)
                    .forced
                    .insert(jump, (taken_fake, reason));
                ("walk: a forced fake branch", None)
            }
            Err(Halt::Rewiden(_) | Halt::NotFunction | Halt::TryMeet(_) | Halt::Held) => {
                unreachable!("loops, `try`s and subroutines catch their own signals")
            }
        };
        if let Some(header) = header.filter(|_| walk.learned.headers.len() > MAX_LOOP_HEADERS) {
            let (pc, state, _) = header;
            walk.session.stop_prefix(
                TraceStopCode::PrefixRepeatedPcState,
                format!("pc {pc} state {state} is one loop header too many"),
                None,
            );
            break (PrefixStopReason::RepeatedPcState { pc, state }, cursor);
        }
        // The loop starts on the top-level path, or a closure shares cells
        // the walk did not know: start over.
        let live = walk.records.live();
        walk.session.rollback(initial.clone());
        walk.records.clear();
        walk.charge(cause, live);
        walk.patches.clear();
        walk.steps = 0;
        walk.loops.clear();
    };
    // A closure captures slots that were not cells: the walk went on with
    // them missing, so nothing it did stands. It starts over from nothing
    // with them as cells, just as a walk told of them from the start does,
    // and so traces the same: the driver may keep either.
    if let Some(wanted) = wanted_cells {
        let mut again = sharing.clone();
        again.cells = cells.into_iter().chain(wanted).collect();
        let debug = walk.session.take_debug();
        let mut report = run_prefix(
            preparation,
            start,
            initial_context,
            trace_limits,
            debug,
            &again,
        );
        report.retraces.add(&walk.retraces);
        report.retraces.cells += 1;
        for (cause, steps) in walk.wasted {
            *report.wasted_steps.entry(cause).or_default() += steps;
        }
        let thrown = walk.records.live();
        if thrown > 0 {
            *report.wasted_steps.entry("walk: shared cells").or_default() += thrown;
        }
        // What this attempt read of the cells' contents stays a reason the
        // walk is what it is: the cells it found came from where it went.
        let reads = walk.session.cell_reads();
        report.cell_reads.cells.extend(reads.cells);
        report.cell_reads.data.extend(reads.data);
        report.cell_reads.sets.extend(reads.sets);
        return report;
    }
    let Walk {
        mut session,
        records,
        patches,
        retraces,
        self_check,
        fake_audits,
        wasted,
        subroutines,
        ..
    } = walk;

    if stop == PrefixStopReason::Budget && !session.is_stopped() {
        session.stop_prefix(
            TraceStopCode::PrefixStepBudget,
            "requested dispatch step budget reached before a terminal boundary",
            None,
        );
    }

    session.drop_unused_functions();
    // The records of each function the trace still has, once.
    let summary_records: Vec<PrefixExecutionRecord> = subroutines
        .values()
        .filter_map(|subroutine| match subroutine {
            Subroutine::Function(summary, _, records) if session.has_subroutine(summary) => {
                Some(records.iter().cloned())
            }
            _ => None,
        })
        .flatten()
        .collect();
    let cell_writes = session.cell_writes().clone();
    let cell_types = session.cell_types().clone();
    let cell_reads = session.cell_reads();
    let image_digest = fnv1a64_hex(&cursor.image);
    let image_divergence = cursor
        .split_image
        .iter()
        .map(|&address| address as u32)
        .collect();
    PrefixTraceReport {
        start,
        records: records
            .into_kept()
            .into_iter()
            .chain(summary_records)
            .collect(),
        patches: patches.into_kept(),
        trace: session.report(),
        stop,
        image_len: cursor.image.len(),
        image_digest,
        image_divergence,
        cell_writes,
        cell_types,
        cells,
        cell_reads,
        patch_selector: preparation.patch_handler.selector,
        retraces,
        self_check,
        fake_audits,
        wasted_steps: wasted,
        try_handlers: walk.try_handlers,
    }
}

/// Where a region that traces itself again for `halt` restarted.
fn restarted_at(halt: &Halt) -> &VisitKey {
    match halt {
        Halt::NewLoop(at) | Halt::Split(at) => at,
        _ => unreachable!("only loops and splits restart a region"),
    }
}

mod converge;
mod loops;
mod patches;
mod state;
#[cfg(test)]
mod tests;
mod walk;

use patches::*;
use state::*;

pub use state::CellSharing;
