# Trace orchestration and shared cells

The command layer chooses which functions to walk, which facts to reuse, and
how to assemble results. It uses the shared prefix engine without making its
policies part of every individual invocation.

The [trace CLI guide](trace-cli.md) owns flags, streams, and exit codes;
[the trace JSON format](trace-json.md) owns structured output and explicit snapshots;
[corpus comparison](corpus-comparison.md) explains baseline interpretation.
For implementation ownership, start with the [trace CLI module guide](../../cli/trace/README.md).

## Shared-cell analysis

A captured closure may share the creator's slot cell rather than just its value.
Full corpus runs gather writes and uses across reachable holders and iterate
candidate cell contents within a bounded number of rounds. A limited corpus run
omits this analysis; one thread or trace input snapshot uses its explicit input
snapshot. A converged map describes the holders actually reached, not every
possible environment.

Program tracing first discovers return shapes and walks reachable functions
once. A walk that stops after reading shared cells waits for its ancestors'
walks (the round records which function creates which) and is walked again
with what they put in those cells, and again while its own writes change what
it asked: a VM jump through a cell its creator set up. After the walk, the
contents every holder puts there are checked against what each such walk
asked. Only when they differ, or functions still stop, does it call
`converge_cells`, giving contents only to the functions that need them (one
that stopped, and the writers of the cells it reads), and walk again; walks
whose cells read the same are kept, with their code. It keeps that retry only
if fewer functions stop and the result still links. This is program policy;
it is not an implicit behavior of `walk_reachable`. The implementation is in
[`program.rs`](../../cli/trace/commands/program.rs), with cell bookkeeping in
[`cell_contents.rs`](../../src/trace/cell_contents.rs).

## Corpus traversal

The following phases describe a full corpus run. Program, limited corpus, and
single-invocation modes choose their own policies.

### The phases

1. **Load or compile.** `load_artifact` reuses the source-checked artifact in
   `--report-dir` or compiles one from the capture.
2. **Prepare.** Build the prefix preparation with the requested `--steps`,
   outlining, and `--audit-fakes` policy. Prepare separate inline and reversed
   walks when self-checking is enabled.
3. **Find roots.** Start with the entry and descriptor records that carry a VM
   Worker bootstrap entry. Other descriptor-created functions are discovered
   during traversal; a descriptor row alone is not a root.
4. **Walk and converge.** `walk_reachable` schedules functions and follows newly
   created functions. Full runs use `converge_cells` to revisit walks whose
   shared-cell inputs changed; limited runs omit convergence. Convergence keeps
   only contents established by the relevant holders and discards guesses that
   do not settle within the round bound.
5. **Emit and check.** Validate emitted code and generated names. Unless `--fast`
   is selected, compare outlined and inline execution, repeat emission, and
   inspect reversed branch order. Inline execution has a larger step allowance.
6. **Fingerprint and compare.** Record semantic and diagnostic results, then
   classify differences against the selected baseline. Command output and
   baseline updates remain caller policy.

## Function identity

A `Function` is `{ descriptor: Option<usize>, pc, state }`; its key is
`entry@<pc>/<state>` for the entry thread or `d<descriptor>@<pc>/<state>` for a
descriptor-created one. The descriptor index is part of identity because the
same PC/state under another descriptor is a different invocation shape.
Created functions are discovered from the trace report; a function's key is
stable within a walk but not a cross-capture operation name.

**Note:** the key writes the pair as `<pc>/<state>`, while the emitted function
name writes it as `$THREAD_<state>_<pc>`. The two forms *reverse* the numbers;
copying a name's numeric order into a key selects a different function. Read the
separator (`@…/` versus `…_…`) rather than assuming the pair is written the same
way in both places. A stopped function is still placed under its key as a
throwing stub, for example `VM function d7@100/2 was not traced: …`, so a missing
body is not a missing function.

## Fingerprints and comparison

A fingerprint records semantic and diagnostic facts: walked positions, real
and by-test branch decisions, by-context decisions, forks, host reads/writes,
created functions, loop/call evidence, stop kind/detail, and code identity when
emitted. Timing may be recorded, but comparison excludes it: elapsed time is
not correctness evidence. A digest is
an addition to the record, not a replacement for the fields that explain it.

`compare` returns two lists: regressions and changes. The split is a policy
decision, not a claim that every change is harmless. The command front end
decides how to print them and which exit status to use.

## Program nesting and return shapes

`program_nest.rs` places each traced function at the creation site:
program-mode code takes no parameters, reads inputs as `arguments[K]`,
`arguments`, `this`, or `PROGRAM_SELF`, keeps slot K in `$KEY_K`, reads shared
slots through the creator's `$KEY_K`, and marks each created function with an
empty `function $THREAD_<state>_<pc>() {}`. Nesting is placement, not renaming;
a function that creates itself refers to its own code. `check` holds each
function to the generated names it may read from outside, so a missing capture
or a name collision fails instead of producing a plausible hierarchy.

```js
// Abbreviated synthetic program-mode shape. Bodies are elided; names are generated.
function $THREAD_1_10() {              // key: entry@10/1
  function $THREAD_2_100() {           // key: d7@100/2, created here
    // the created function's traced body is placed inside this marker
  }
  return $THREAD_2_100;
}
$THREAD_1_10();
```

`return_shapes.rs` discovers what an outlined call can return so the caller can
bind the result without executing it. A call with no discoverable shape stays
opaque rather than being treated as `undefined`.

## Where a bug belongs

| Symptom | Inspect |
| --- | --- |
| A function is missing from every run | Root discovery/descriptor records and reachability, not fingerprint comparison |
| Same function key, different descriptor context | Function identity includes the descriptor index; do not merge keys |
| Full and limited runs disagree | Shared-cell convergence is skipped under `--limit`; reproduce with matching flags |
| `--fast` changes a result | It should skip only self-checks; if reachability changed, the bug is in policy placement |
| Branch decisions differ but code is identical | Fork/convergence ordering and `--seed-by-seed` scheduling; order observations are not failures |
| Self-check fails | Compare the outlined walk with the inline one; the inline budget is deliberately larger |
| Emitted code fails a check | `emission.rs` and generated names; the walk may still be correct |
| Baseline comparison reports a regression | Confirm the baseline was produced with the same flags, artifact mode, and convergence policy |
| `--update-baseline` changed the test result | The update run is not a regression run; separate them |

## Tests and invariants

Tests: `trace_corpus_cli.rs` for command
behavior and comparison output, `vm_branches.rs`
for branch semantics, and the prefix/execute suites for fork and convergence
mechanics.

- Function identity is `(descriptor, pc, state)`; keys are per run, not
  cross-capture names.
- A full run may iterate shared-cell contents; a limited run may not, and the
  difference must stay visible.
- Elapsed time never participates in correctness comparison.
- A baseline is only comparable under matching flags and artifact policy.
- A change is recorded, not silently normalized; a regression is reported, not
  hidden behind a digest.
- Emitted-code failures do not retroactively make the walk invalid, and a valid
  walk does not excuse failed emission.

The [trace CLI overview](../../cli/trace/README.md),
[corpus comparison guide](corpus-comparison.md), and
[trace CLI guide](trace-cli.md) give the full mode contract.
