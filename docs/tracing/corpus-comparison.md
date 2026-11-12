# Corpus comparison

`shape trace corpus` walks reachable VM functions and reports their stops,
coverage, effects, branch decisions, and emission checks. It can compare the
result with a baseline. This page explains the current behavior and gives a
short way to run it; it does not set a policy for keeping output files.

## Run it

From the repository root, after [building `shape`](../../README.md#build-once):

```sh
./target/release/shape trace corpus tests/fixtures/kernels/javascript/kernel.raw.js --fast
```

This uses a tracked full JavaScript VM capture and prints a summary to stderr.
`--fast` skips corpus self-checks. Remove it to compare outlined and inline
walks, repeat emission, and explore the other branch order. The default run
also converges shared-cell contents across reachable functions.

For a quick inspection of the output shape, add `--limit 3 --fast`. A current
run on the tracked JavaScript VM fixture reported three complete functions,
then lines headed `steps:`, per-reason discarded-work counts, `overstep`, and
`time:`. The numerical counts and elapsed time depend on the build and mode;
the stable interpretation is that the summary separates functions reached,
completion, work kept, and work discarded. A limited run deliberately omits
full shared-cell convergence, so it is a smoke check, not a substitute for the
unlimited corpus comparison.

To compare later runs against a baseline, choose a path yourself. For example,
`reports/` is ignored by Git:

```sh
mkdir -p reports
./target/release/shape trace corpus tests/fixtures/kernels/javascript/kernel.raw.js \
  --baseline reports/corpus.json --update-baseline
./target/release/shape trace corpus tests/fixtures/kernels/javascript/kernel.raw.js \
  --baseline reports/corpus.json
```

The first command writes the baseline. The second reads it and reports
regressions and changes. Do not update a baseline as part of the comparison
you intend to inspect. The path and how long to keep it are the caller's
choice. For per-function emitted-code digests and branch decisions, add
`--branches FILE`; for ruled-out-side details, add `--audit-fakes` with a
branches file. These outputs are optional.

## Consumer workflow matrix

The trace commands share an engine but deliberately use different policies:

| Command or mode | Reachable functions | Shared-cell convergence | Self-checks | Output |
| --- | --- | --- | --- | --- |
| `trace corpus` | all reachable functions | yes | on by default | summary, optional baseline and branches file |
| `trace corpus --limit N` | at most N started per capture, in one deterministic queue | no | on by default | summary and optional files |
| `trace corpus --fast` | same reachability as its limited or full mode | unchanged | off | same output shape |
| `trace corpus --cleanup` | unchanged | unchanged | unchanged; checks cleaned emission | same output shape |
| `trace program` | all reachable functions of one capture | ancestors' contents for a walk that stops on a shared cell; conditional retry when functions still stop | no corpus self-checks | one linked JavaScript file; `--no-cleanup` skips whole-program Oxc passes |
| `trace thread` | one supplied entry | no | optional `--self-check` | one function or a stop |

`--seed-by-seed` changes corpus scheduling, not convergence or comparison
rules. A limited run is not a cheaper equivalent of a full run: missing
shared-cell contents can change where a function stops. `trace program`
shares reachable traversal, gives a walk that stops on a shared cell its
ancestors' contents, and can retry using converged cell contents when
functions still stop. It adopts the retry only if fewer functions stop and it still
links. [Trace orchestration](trace-orchestration.md#shared-cell-analysis) explains
that policy; the [trace CLI guide](trace-cli.md#shared-cells) documents input,
scheduling, and exit-code contracts.

## Reading differences

A corpus baseline comparison uses **`regression:`** for a function that is no
longer reached, a previously complete function that now stops, lost
positions/effects/created functions in a completing function, or a new branch
decided by context. It exits 1 for regressions. **`changed:`** reports new
reachability, gains, newly completed functions, different partial stops, and
branch-count changes without making them failures. An `undeclared`,
`emission`, or `self_check` problem can fail a run even without a baseline.

The labels are line prefixes on stderr. With illustrative counts and a
placeholder `capture.raw.js` filename, a run and its comparison print in this
order:

```text
capture.raw.js: 3 functions, 2 complete, 0.1s, overstep 1.4x (worst d4@92/2 2x)
     1 stopped: unsupported-handler
steps: 40 run, 28 kept, 12 thrown away (30% of those run)
             9  22.5%  walk: a loop
             3   7.5%  fork: a fake side
overstep      2x capture.raw.js d4@92/2
time: 110.00ms
changed: capture.raw.js d4@92/2: now completes
baseline: 1 regressions, 1 other changes
regression: capture.raw.js entry@10/0: no longer reached
```

**Note:** `baseline:` is printed *between* the `changed:` and `regression:`
lines. A reader, script, or log filter that stops at the summary line can miss
the trailing `regression:` records; scan the whole stderr block before deciding
the run passed.

Branch-order differences are observations. Corpus reports how many functions
depend on side order; it does not promote those differences to regressions.
For `trace thread`, `--reverse-sides` changes the walk order but does not
compare runs for you. A code digest difference says the emitted structure
changed, not which version is correct. Timing is diagnostic and is not part
of semantic comparison.

For a branch-count change, inspect the branch JSON Lines in order and compare
the test, PC/state, classification, chosen side, and reason. The
[fake-branch examples](fake-branch-triage.md#five-small-walks-that-separate-the-causes)
show why an impossible dispatch target, a genuine exception, and a decided
test must be interpreted differently. The corpus records the difference;
the underlying trace evidence determines whether a new pruning rule is sound.

For example, if one function appears under `changed:` with more covered
positions but a different branch count, inspect its branch records and emitted
code before deciding whether the new path is better. If its only difference
is the order in which two branch sides were visited, retain that observation;
the corpus does not classify order dependence as a regression. Conversely,
`regression:` on a previously complete function that now stops is actionable
even if the overall capture still reaches the same number of functions.

If a before/after result is surprising, first rerun each side with the same
input and flags. Compare the stops, positions, effects, created functions,
branch decisions, and emitted code for the affected function. A full corpus
run and a limited run are different cases; so are runs with and without
`--fast`, `--cleanup`, or `--audit-fakes`. Use the [trace CLI](trace-cli.md)
for output fields.
