# Prefix stops and retrace diagnostics

Read a prefix result at three levels: the public stop, the ordered symbolic
diagnostics, and the committed records. `stop_detail()` returns the **last**
diagnostic message; it does not search for a code matching the public stop.

Use [prefix execution](prefix-execution.md) for the walker and budget rules,
[fake-branch triage](fake-branch-triage.md) for interpreting branch evidence, and
[trace CLI](trace-cli.md#diagnostics) for command output.

This helper fragment compiles in a crate that depends on `shape`; it takes a
report the caller already produced.

```rust
use shape::{PrefixStopReason, PrefixTraceReport};

fn explain(report: &PrefixTraceReport) {
    // The public stop is the decision surface; it is not the same value as
    // the last diagnostic code.
    match &report.stop {
        PrefixStopReason::Completed => { /* a completed prefix can emit */ }
        PrefixStopReason::UnsupportedHandler { selector, reason } => {
            eprintln!("selector {selector}: {reason}");
        }
        other => eprintln!("{other}"),
    }

    // Ordered diagnostics: an evidence code can be last without being the stop.
    for diagnostic in &report.trace.diagnostics {
        eprintln!("{:>24}: {}", diagnostic.code.as_str(), diagnostic.message);
    }

    eprintln!("committed steps: {}", report.records.len());
    if let Some(last) = report.stop_detail() {
        eprintln!("last diagnostic: {last}");
    }
}
```

The `records` are *committed* work; a step that stops before commitment is not
counted there even though it may have produced the last diagnostic.

## The stop-reason catalog

A stop has two layers. The public `PrefixStopReason` is what a caller matches
on; the `TraceStopCode` on the last diagnostic is the tracer's more specific
account of the same boundary. One public reason can carry several codes, and
some codes appear as the last diagnostic without changing the public reason
(because a later stop replaced it or because the code is evidence, not a stop).

### The ten public variants

| Variant | Fields | Diagnostic code(s) | Raised at | Committed work |
| --- | --- | --- | --- | --- |
| `Completed` | none | none (session completion `Completed`) | `run_prefix` when the top-level path and every fork side ended | everything traced |
| `Budget` | none | `prefix-step-budget`, or the session's own budget code | `step_once` / `run_prefix` / a recipe's `BudgetStopped` | everything before the budget |
| `TruncatedImage` | `pc`, `image_len` | `prefix-truncated-image`, `prefix-pc-overflow`, `prefix-next-pc-overflow` | `step` via `fetch_cell`, `checked_pc_increment`, or a handler PC delta | everything before the fetch |
| `TruncatedOperand` | `operand_entry_pc`, `needed`, `available` | `prefix-truncated-operands` (decode), `prefix-image-patch` (patch read) | `step` / `read_image_patch` | everything before the operands |
| `MissingState` | `state` | `prefix-missing-state`, `prefix-invalid-next-state` | `step` via `fetch_cell` or the next-state check | everything before the fetch |
| `MissingOpcode` | `state`, `opcode` | `prefix-missing-opcode` | `step` via `fetch_cell` | everything before the fetch |
| `AddressOutOfBounds` | `address`, `image_len` | `prefix-image-patch` | `read_image_patch` | everything before the patch |
| `UnsupportedHandler` | `selector`, `reason` | `prefix-missing-handler`, `prefix-unsupported-handler`, `prefix-unlifted-handler`, `prefix-missing-operands`, `prefix-unsupported-operands`, `prefix-image-patch`, `wasm-trap` | `step` (template, operand, patch, WASM bounds) | everything before the handler |
| `RepeatedPcState` | `pc`, `state` | `prefix-repeated-pc-state` | `step` (revisited coordinate) or `run_prefix` (too many headers) | everything before the repeat |
| `UnjoinableBranches` | `pc`, `state`, `reason` | `prefix-branch-join`, `prefix-loop-stack-change` | `fork_sides`, `run_loop`, `run_try`, `unjoinable` | everything before the first unmergeable position |

### `Completed`

The one outcome in which every explored path returned (`vmPendingResult =
value`) or continued a loop. The session's completion is `Completed`, and
`emit_javascript` can validate the trace for emission. `Completed` is **not** a claim of
full-capture coverage: it says the walk's paths ended, not that the walk reached
every path the program can take. A completed prefix with a hundred unexplored
states is still a completed prefix.

### `Budget`

Raised in two distinct situations, which is why the diagnostic matters:

| Situation | Last diagnostic | Records |
| --- | --- | --- |
| the driver's `max_dispatch_steps` reached before a fetch | `prefix-step-budget` | committed steps so far |
| a recipe exceeded `TraceLimits` | the session's `step-budget` (or another session bound) | the failed step is **not** committed |

A zero-step budget commits no record at all and leaves
`TraceCompletion::PrefixStopped(prefix-step-budget)`. A budget reached exactly
after the last committed step still reports `Budget`, not `Completed`, because
the walk did not establish that every path ended.

### `TruncatedImage`

Three shapes, one public reason:

* the dispatch PC is outside the image (`fetch_cell`: `pc >= image_len`);
* `pc + 1` overflowed the address space (`checked_pc_increment`);
* the handler's `pc_delta` overflowed, or produced a next PC outside the image
  on the *next* fetch (`prefix-next-pc-overflow` is recorded for the delta; the
  next fetch then reports `truncated-image`).

Inspect `records.last()`: its `actual_next_pc` is where the walk tried to go. A
`pc_delta` that is plausible for the recipe but outside the image usually means
the image is truncated relative to the handler's expectation, or the handler
selection is wrong because an earlier patch wrote an unexpected opcode.

### `MissingState`

Two shapes:

* the current `state` has no row in the table (`fetch_cell`);
* the fetched cell names a `next_state` outside the proven table
  (`prefix-invalid-next-state`, recorded before the handler runs).

The second is not the same as "the table is too short": it means the table
itself contains an invalid target, which `validate_table` normally rejects at
preparation. Seeing it at walk time means the preparation was mutated (as
[`execute/tests.rs`](../../src/dispatch/prefix/execute/tests.rs) does deliberately) or an artifact was
tampered with in memory. Inspect the cell at `(state, opcode)`.

### `MissingOpcode`

The state row exists but has no cell for the fetched opcode byte. Preparation
permits rows of different lengths. Check whether the opcode came from a patch
or whether the PC landed mid-instruction before interpreting the missing cell. Inspect the
opcode byte and the patch records that wrote near that address.

### `TruncatedOperand`

The handler's operand range extends past the end of the image. The public fields
are computed as `needed = required_end - operand_entry_pc` and
`available = image_len - operand_entry_pc`, both saturating. For the patch
handler the width is fixed at 4; for a recipe it is the maximum extent of the
lifted descriptors, not the first descriptor's width. A record is never
committed for a truncated operand step.

### `AddressOutOfBounds`

The patch handler decoded an address at or beyond `image.len()`. The address is
24-bit little-endian, so the bound check is against the current (possibly
patched) image length. Inspect the four operand bytes: the address is
`bytes[0] | bytes[1] << 8 | bytes[2] << 16`.

### `UnsupportedHandler`

The widest family. The `reason` string is the useful part; the selector tells
you which table entry to inspect.

| Reason (prefix) | Code | Cause |
| --- | --- | --- |
| `selector has no proven operation-table handler` | `prefix-missing-handler` | The selector is in the table but no template exists |
| `handler status is <status>` | `prefix-unsupported-handler` | The template was classified partial/unsupported |
| `<unknown.reason>: <unknown.source>` | `prefix-unsupported-handler` | The first unsupported fragment's reason |
| `the WASM interpreter traps (unreachable) here` | `wasm-trap` | The selector is a trapping selector |
| `handler operands are not fully proven (status …)` | `prefix-unlifted-handler` | The operand separation status is neither `Lifted` nor `NotApplicable` |
| `selector has no operand-separation handler` | `prefix-missing-operands` | The operand report has no handler for the selector |
| operand decode error | `prefix-unsupported-operands` | A descriptor combination the decoder refuses |
| `image-patch handler operands are N bytes; the proven layout is 4 bytes` | `prefix-image-patch` | A patched preparation with a wrong width |
| `bytecode image write rejected: <stop>` | `prefix-image-patch` | The patch address/width failed at apply time |
| `image write at A changes the WASM interpreter's own tables` | `prefix-image-patch` | A WASM write at or past `frozen_from` |
| `N try frames: the WASM interpreter traps past K` | `wasm-trap` | The WASM try bank is full |
| `N error records: the WASM interpreter traps past K` | `wasm-trap` | The WASM record bank is full |
| `a VM call inside a VM call: …` | `prefix-unsupported-handler` | The WASM interpreter keeps one return address |
| `handler recipe stopped as unsupported` | (the session's diagnostic) | The recipe returned `Partial` |

`UnsupportedHandler` is a **coverage** boundary, not a claim that the program is
malformed. The handler exists; this walker cannot execute it. That distinction
is what makes it safe to report as a partial prefix rather than an error.

### `RepeatedPcState`

A dispatch revisited a `(pc, state, call stack)` it had already visited in the
same path, which is a loop the walker did not open. In practice:

* the position is a loop header that a nested region swallowed before `advance`
  could recognise it; or
* a handler jumps back to its own position without changing state, and the
  walk's loop machinery could not form a loop (for example because the recipe's
  stack behaviour makes every iteration look different); or
* the walk learned more than `MAX_LOOP_HEADERS` headers and gave up.

The message "pc P state S is one loop header too many" identifies the third
case. The message "prefix revisited pc P state S without progress" identifies the
first two.

### `UnjoinableBranches`

The fork's two sides cannot be merged, or the walk gave up trying:

| `reason` | Code | Meaning |
| --- | --- | --- |
| `the branches patched bytecode byte A differently, and the merged path reads it` | `prefix-branch-join` | The merged image cannot supply one proven byte value; inspect the divergent histories |
| `an iteration changes the stack length from N to M` | `prefix-loop-stack-change` | A loop iteration must keep the stack shape |
| `the loop variables do not settle` | `prefix-branch-join` | `MAX_LOOP_WIDENINGS` exhausted |
| `the try variables do not settle` | `prefix-branch-join` | `MAX_LOOP_WIDENINGS` exhausted |
| `forks nest more than 48 deep` | `prefix-branch-join` | `MAX_FORK_DEPTH` |
| `the merged histories hold bytecode images of different lengths` | `prefix-branch-join` | Internal invariant broken |
| the session's join error | `prefix-branch-join` | `join_tagged` / `join` refused; the error names the first mismatch |
| `the catch path crosses the body again where they were to meet` | `prefix-branch-join` | A `try` meeting point repeated |
| a split side stayed apart past `MAX_APART_STEPS` | `prefix-branch-join` | The recorded reason and position |

`UnjoinableBranches` is where "the model stopped" is most visible. The reason is
chosen to name an invariant, not a guess; `bad_state` separately decides whether
the same failure is evidence that one side of a fork is fake. A stop that is
merely a model limit (an unsupported operation inside a catch, a stack the throw
left) is not evidence and must not prune a branch.

### Codes that can be last without being the public stop

These codes describe evidence or a recovered attempt; they may appear as the last
diagnostic even when the public stop is something else:

| Code | Meaning |
| --- | --- |
| `dynamic-index` | A VM value needed a concrete index; the walk may recover by dispatching or splitting |
| `program-value-jump` | A VM jump went to a program value; this is fake-side evidence when it came out of a closed `if` |
| `junk-function-entry` | A made function fails before its first real branch |
| `catch-stack-unknown` | A catch read below the stack its `try` was entered with; never fake-side evidence |
| `constant-table-index` | A concrete read past the end of a constant table; fake-side evidence, and it makes a function junk when reached before its first real branch |
| `invalid-index` | A stack index the tracer cannot resolve; evidence only outside a catch and outside a short side |
| `empty-call-frame-stack` | A VM return with no frame; impossible for real code |
| `empty-exception-stack` | A pop of an empty error-record stack; impossible for real code |
| `wasm-trap` | A trapping selector or a full WASM bank |
| `try-underflow` | A `try` exit below zero; a trap only for the WASM interpreter |

`TraceStopCode` also defines `prefix-throwing-handler` and
`prefix-thrown-handler`. This folder does not raise them: a reached `throw` is a
normal `Thrown` handler result that ends the path, not a stop. They are listed
here so a reader who meets the strings elsewhere does not attribute them to the
prefix driver.

### What to inspect, by family

| Family | First | Then |
| --- | --- | --- |
| image/table stop | `records.last()` coordinates | the image byte at `dispatch_pc`, the table cell at `(state, opcode)` |
| operand/patch stop | the operand bytes at `operand_entry_pc` | the patch record for that address, then the current image digest |
| unsupported handler | `reason` | the template's `unsupported` fragments, then the operand handler status |
| repeated position | `prefix-repeated-pc-state` | the `Visited` history of the path, the handler's control transfer |
| unjoinable | `reason` and `pc/state` | both sides' stack lengths, `try` depth, pending exceptions, divergent bytes |
| budget | `wasted_steps` and `retraces` | whether the session or the driver produced the stop |

## Retraces, waste and cost

The retrace counters answer "how much work was thrown away, and why". The
`wasted_steps` map answers a narrower question: "how many dispatch steps did a
discard throw away, by the region that discarded them".

### `wasted_steps` keys

Each key is a literal string at the `charge` call site, so it is stable and
greppable:

| Key | Charged when |
| --- | --- |
| `walk: a loop` | the top-level path restarts for a new loop header |
| `walk: a split` | the top-level path restarts for a split |
| `walk: a call traced inline` | the top-level path restarts to trace a call inline |
| `walk: a forced fake branch` | the top-level path restarts with a side known fake |
| `walk: a junk function the code makes` | the walk restarts because the only stop was a decoy function |
| `walk: shared cells` | a cell restart abandoned the attempt's kept records |
| `loop: new loop variables or values` | a loop attempt is retraced for widening |
| `loop: a loop inside` / `loop: a split inside` / `loop: a call traced inline` | a loop attempt is retraced for an inner region |
| `try: a meeting point found again` | a `try` attempt is retraced for a newly found meeting point |
| `try: a try variable the catch needed` | the catch needed a variable/value/type the body had not provided |
| `try: new try variables or values` | the `try` attempt is retraced for a new variable or a grown set |
| `try: a loop inside` / `try: a split inside` / `try: a call traced inline` | a `try` attempt is retraced for an inner region |
| `try: a closed body's step undone` | a post-`try` body step emitted code and was undone |
| `fork: past the meeting point` | a side was cut back to a meeting point |
| `fork: a meeting point found again` | the attempt restarted because the meeting point could not be cut back to |
| `fork: the other side halted` | the other side's turn step was undone |
| `fork: both sides halted` | the second halt's step was undone |
| `fork: back to a loop header` / `fork: back to a split` / `fork: back to a call traced inline` | a side was cut back to the step that found the region |
| `fork: the other side after a cut` | the other side was rewound to the same turn |
| `fork: a loop inside` / `fork: a split inside` / `fork: a call traced inline` | the whole fork attempt restarted |
| `fork: a fake side` | the fork attempt restarted following only the real side |
| `self-check: a call traced inline` | the sandboxed inline trace was discarded |
| `call: not a function` | a subroutine summary was rejected |

`charge` computes the waste as `live_before - live_after` on the record log, so
it counts *kept* records only. Records already dropped by an earlier cut are not
counted twice. The one place that adds directly is the cell restart, which adds
the abandoned attempt's live records under `walk: shared cells`.

### Reading the numbers

| Pattern | Likely cause | First place to look |
| --- | --- | --- |
| high `new_loops` with low completion | a state machine whose header is found late | the loop's entry coordinate and `Learned::headers` |
| high `loop_variables` | a cell that changes every iteration and is not stabilising | `open_loop_with`, the widened set |
| high `loop_values` | a value set or type set that grows each attempt | the dispatch chain and `take_used_sets` |
| high `try_variables` | a catch that needs a write the body makes late | `body_first`, `try_assigned_values` |
| high `meeting_points` | out-of-line layout; sides meeting late | the branch record's `target` and the side histories |
| high `splits` | joins that produce values used as jumps, or padded joins undone | `join_tagged`, `apart`, `apart_bytes`, `unpadded` |
| high `dispatches` | a state machine branching per value | the value set size (64 is the cap) |
| high `unmergeable` | real code with different stack shapes that cannot be lined up from the top, or a missed fake branch | the first unmergeable position and the `short_side` flag |
| `wasted_steps` concentrated in `fork: a fake side` | many branches are pruned by context | each branch record's `FakeByContext` reason |

No single number is a diagnosis. The report gives counts by cause so that a
regression can be localised; the branch records and the stop are what identify
the specific decision.

## Debugging a stopped or expensive walk

Start with the `PrefixTraceReport` stop, committed records, branch evidence,
and retrace counts. Recipe work discarded during rollback is not the same as
a committed dispatch step. Increasing a step budget will not repair a missing
selector proof, an incompatible merge, or an unsupported operand shape.

For a fork, check both positions and the VM call stack, then cell/stack/image
compatibility. A failed branch is eligible for context pruning only under the
walker's specific bad-state rules; an ordinary program throw is not blanket
evidence that a path is fake. For a loop, check which cells required widening
and which region restarted. For a patched image, inspect patch records and the
private image identity before comparing later coordinates.

`retraces` says why work was done again; `wasted_steps` counts the steps
thrown away, keyed by cause.

[`tests.rs`](../../src/dispatch/prefix/execute/tests.rs) covers internal walker cases. Public integration suites
include `prefix.rs`,
`trace_stop_codes.rs`, and
`vm_branches.rs`. WASM integration is in
`wasm_kernel.rs`.
