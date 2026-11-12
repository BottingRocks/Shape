# Flat actual-properties example

[actual-properties.json](actual-properties.json) illustrates the flat tracer-input
properties bundle documented by [runtime configuration](../../../docs/deob/runtime-config.md)
and [kernel layout](../../../docs/deob/kernel-layout.md). It contains entry
PC/state/startup slots, operation source strings, string and number constants,
thread descriptors, context/memory/stack roles, dispatch rows, and bytecode.

Descriptor argument/captured/initial slots are separate from entry initial slots.
The number table retains negative zero and tagged NaN/Infinity values;
operation keys are local to this example. Preserve these distinctions when
reading or comparing the bundle.

This JSON is a documentation example, not the evidence-wrapped
`shape-runtime-config/v1` report or a source-free `KernelArtifact` accepted at
trace `--report`. No direct Rust test consumer of this file was found.
Read [properties implementation](../../../src/kernel/properties/mod.rs) and
[config.rs](../../../src/kernel/config.rs) for their separate contracts and tests.
Keep the golden JSON and handler strings unchanged; do not execute those strings.
Continue with the [fixture overview](../README.md).
