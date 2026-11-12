# Shared VM component evidence

[shared-components.json](shared-components.json) records
`shape-vm-shared-components/v1` static comparisons: source identities, normalized
handler/helper bodies, shared VM components, and module information across
JavaScript and WebAssembly interpreter placements.

The tracked kernel pair is distinct from the historical dual-module evidence
whose raw source is absent. Binding-normalized bodies support structural
comparison; shuffled names, indices, arities, and handler compositions remain
per-input variants, not interchangeable handler identities.
The deliberate null signal is callable creation followed by invocation inside
try/catch; creation itself must not be treated as throwing.

This is reference evidence rather than a directly loaded Rust test fixture.
Read [VM flow](../../../docs/reference/vm-flow.md),
[operation reference](../../../docs/reference/operation-reference.md), and the
[current operations implementation](../../../src/operations/README.md) for
consumers and implementation checks. Preserve exact snippets, hashes, source
spans, and unsupported distinctions. Do not execute recovered JS/WASM.
Continue with [exception evidence](../wasm-boundary/README.md) and
[fixture overview](../README.md).
