# WebAssembly exception-boundary evidence

[exception-boundary.json](exception-boundary.json) records the
`shape.wasm-exception-boundary/v1` static evidence contract. It relates deliberate
exception signaling to JavaScript push/invoke timing, WASM imports, module
body ranges, interpreter flags, and dispatch/image layout.

The JavaScript reference is an outside-WASM-boundary control; the tracked WASM
reference and historical dual-module evidence have distinct wiring. The latter
has no tracked full raw source. Creating a callable containing `null[0]()`
does not throw: invocation must preserve the TypeError and enclosing catch path.
Do not turn a shared semantic contract into a claim that every boundary is
currently recognized or executable.

No direct Rust test reader of this JSON was found. Read the
[exception-boundary guide](../../../docs/reference/wasm-exception-boundary.md)
and [WASM implementation](../../../src/kernel/wasm/README.md) for the evidence
interpretation and implementation unit checks. Standardized-source spans and
decoded-module offsets use different origins; retain both with their identities.
Never execute the recorded JavaScript/WASM or normalize these golden inputs.
Continue with [shared components](../vm-comparison/README.md) and
[fixture overview](../README.md).
