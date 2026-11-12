# Regression tests

Rust files directly in this directory are Cargo integration targets for the
`shape` library and binary. Small unit tests also live beside implementations
in [src/](../src/) and [cli/](../cli/README.md).

The main integration groups are:

- `naming_*.rs`: semantic naming, ownership, capture epochs, edit replay, refusal controls, and CLI output.
- `program_*.rs`: program-level role discovery, signal extraction, descriptor aliases, coverage, and projection.
- `generator_*.rs`: byte codecs, headers, serialization, pinning, report/payload framing, and CLI round trips.
- [worker_environment_two_argument_owner.rs](worker_environment_two_argument_owner.rs): owner-sensitive worker environment variants.

[fixtures/](fixtures/README.md) holds JavaScript inputs, JSON contracts, immutable
kernel references and golden proofs. [ownership/](ownership/README.md) is a
nested module of the joined-observables target, not a standalone Cargo target.
[support/](support/README.md) supplies frozen-layout helpers and a measurement utility.
Some `.cjs` files are explicit development or semantic reference harnesses;
that does not make the captured JavaScript/WASM fixtures runnable scripts.

Run from the repository root:

```sh
cargo test
cargo test --test naming_joined_observables
cargo test --test generator_codec
node --test scripts/docs-links.test.mjs
node scripts/docs-links.mjs
```

Generator tests can use Node for independent encoder comparisons when available.
For expensive seed generation or full input analysis, build/use release binaries;
do not regenerate golden fixtures during a documentation change.

Regular Cargo runs skip explicitly ignored private-corpus checks.
[program_signal_coverage.rs](program_signal_coverage.rs) and
[program_signal_projection.rs](program_signal_projection.rs) document their
external corpus requirements (`SHAPE_FROZEN_ROOT`, and for projection
`SHAPE_PROOF_SOURCE_ROOT`). With those inputs available, their existing form is
`cargo test --profile iter --test program_signal_coverage -- --ignored --nocapture`;
select the projection target separately as needed. Other ignored tests have
local prerequisites: read their ignore reason before enabling them.

Preserve source bytes, byte spans, golden identities, and independent negative
controls. Prefer focused target checks for a local change. The
[project README](../README.md) explains the pipeline these tests protect.
