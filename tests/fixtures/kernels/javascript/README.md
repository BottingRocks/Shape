# JavaScript VM raw reference

[kernel.raw.js](kernel.raw.js) is the immutable raw source reference for this
interpreter placement. The JavaScript interpreter reference is also consumed by [naming_corpus_cli.rs](../../../naming_corpus_cli.rs) and the artifact unit tests in [artifact.rs](../../../../src/dispatch/prefix/artifact.rs).

Its identity belongs to the [kernel manifest](../manifest.json).
The [kernel guide](../README.md) defines the source-only contract; static
comparison records in [operand evidence](../../js-operands-reference/README.md),
[shared components](../../vm-comparison/README.md), and
[exception boundaries](../../wasm-boundary/README.md) refer to these bytes.

This reference is a baseline for analysis, not a runnable example or a promise
of complete extraction support. Preserve every byte, embedded payload, and
recorded identity. Do not format or regenerate the file, execute its JavaScript
or WebAssembly, or transfer handler keys between variants as semantic IDs.
Read the consuming source and [fixture overview](../../README.md) next.
