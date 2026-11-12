# Shared test support and measurement

[frozen_signal_layout.rs](frozen_signal_layout.rs) is shared by
[program_signal_coverage.rs](../program_signal_coverage.rs) and
[program_signal_projection.rs](../program_signal_projection.rs). It locates the
external frozen corpus via `SHAPE_FROZEN_ROOT`, reads verified schema/slot maps,
resolves legacy identities to canonical names, and loads gzip proof JSON keyed
by address and label. The proof scan selects `.json.gz` files explicitly.
These answer keys are test evidence; recognizers must not consult them.

[measure_operation_classification.py](measure_operation_classification.py) is a
manual measurement utility, not an automatic Cargo target. Its explicit inputs
are `--shape` (an already-built release binary), `--seeds` (source directory),
and `--out` (report directory). It runs `shape ops list --format json` and emits
manifest, baseline, proposed, summary, and raw JSON records. Proposed classifier
values are recomputed measurements, not changes to the production catalogue.

The harness hashes source files and never executes seed JavaScript/WASM.
Keep proof bytes, slot/address relationships, source identities, and baseline
classification evidence intact. Read each consumer's ignored-test prerequisites
before using external corpus checks. See the utility's usage docstring for its
command and the [tests overview](../README.md) for regular test invocation.
