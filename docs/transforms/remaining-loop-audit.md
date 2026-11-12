# Remaining-loop audit

Use this procedure to inspect labeled `vm_loop_` loops that survive
structured-loop recovery in an emitted program. It records what the pass could
not prove; it does not claim that every loop is transformed or that the output
is equivalent.

## Procedure

Build the release executable, then trace the tracked JavaScript VM reference
and dump the stages:

```sh
cargo build --release
mkdir -p reports/stages reports/loop-audit
./target/release/shape trace program tests/fixtures/kernels/javascript/kernel.raw.js \
  --out reports/program.js --report-dir reports/cache --thread-cleanup \
  --dump-stages reports/stages 2> reports/program.log
```

Inventory the labeled infinite loops left in the earliest linked snapshot:

```sh
SHAPE_LOOP_AUDIT_SOURCE=reports/stages/00-linked.js \
SHAPE_LOOP_AUDIT_OUT=reports/loop-audit \
cargo test --release inventory_remaining_loops -- --ignored --nocapture
```

The test writes `index.tsv` with the loop number, source line, owning function,
and label. For each row it also writes a source fragment and a control-flow
outline. It parses and prints the supplied code; it never executes it.

`00-linked.js` is produced after each function's cleanup (`--thread-cleanup`
and the per-function stages, whose loop recovery is `11d2`), so the index
contains the labeled infinite loops that recovery declined or could not prove,
not every loop in the capture. To see one function before recovery, read its
`functions/<function>/pipeline/11d-empty-else.js`. Stage `13b` also drops loop labels no
jump needs, so an unrecovered `for (;;)` left with no labeled jump is not in
the index either. An empty index means no such loop remains in that snapshot;
it does not prove that every loop was rewritten or that all infinite-loop
forms are gone.

## Control-flow checks

For each indexed fragment, compare the emitted loop with its outline and check
the categories that can change behavior:

- **Zero iteration.** A counted loop may skip its body when the entry test is
  false. A `do…while` runs once. Confirm the selected form matches the original
  test and backedge placement.
- **Effects and throws.** Property reads and writes, calls, getters, coercions,
  and throws keep their original order relative to the test and backedge.
  `try`/`catch`/`finally` targets and thrown values must not move or disappear.
- **`break` and `continue`.** Labeled and unlabeled jumps must reach the same
  logical loop as before, with no extra or missing exit. Keep a label when a
  nested jump still targets it.
- **Carried values.** A value written on one iteration and read on the next
  keeps the same update timing. Reads after the loop must observe the final
  iteration, including the exit test's result.

## Checks and limits

`node --check reports/program.js` confirms syntax, and the focused synthetic
tests compare original and rewritten small programs:

```sh
node --check reports/program.js
cargo test --release structured_loops -- --nocapture
```

These checks establish syntax, scope, and modeled behavior only. They do not
prove runtime equivalence for the canonical reference. Do not execute captured
JavaScript, and do not hand-rewrite a remaining loop to make the inventory
empty: a remaining loop is a missing proof to report, not a failure to hide.
Line numbers and loop labels belong to one run and can drift after cleanup or
source changes.
