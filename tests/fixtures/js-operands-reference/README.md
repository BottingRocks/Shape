# Paired operand source evidence

[paired-cases.json](paired-cases.json) records static JavaScript/WASM handler
pairs under `shape-js-wasm-operand-paired-cases/v1`: source bodies, standardized
byte spans, normalized forms, operand examples, and module identities.
It separates the tracked JavaScript and WebAssembly references from a historical
dual-module record whose complete raw source is not tracked here.

The comparison establishes source shapes and operand roles: inline JavaScript
byte reads versus arguments decoded by the WASM interpreter. It does not join
branch selectors to handler occurrences or equate per-input handler keys.
An unsupported extractor outcome remains a gap, not a successful extraction.

This is documentation evidence, with no direct Rust test consumer found.
Read the [paired-case guide](../../../docs/reference/js-wasm-operand-cases.md)
and [operand implementation](../../../src/operations/operands/README.md) for
current behavior and inline tests. Preserve all source snippets, spans, hashes,
and historical/tracked distinctions. Never execute the recorded JS/WASM.
Continue with [small layouts](../js-operands/README.md) and the
[fixture overview](../README.md).
