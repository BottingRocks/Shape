# Shared regression inputs

This tree contains source fixtures, JSON answer keys, schemas, and static proof
records for [the tests](../README.md). It is not a corpus to execute wholesale.
Many tests use exact `include_str!`/`include_bytes!` inputs and mutate selected
bodies in memory to establish positive and refusal controls.

- [naming/](naming/README.md) groups whole-scope naming protocols and alternate layouts.
- [program/](program/) supplies program-level role and ownership examples.
- [signals/](signals/) supplies signal schemas, slot maps, and frozen proofs.
- [generator/](generator/README.md) supplies codec/report answers and explicit development reference utilities.
- [kernels/](kernels/README.md) holds immutable raw JavaScript and WebAssembly VM references.
- [js-operands/](js-operands/README.md) and [js-operands-reference/](js-operands-reference/README.md) separate small decoder shapes from source-backed paired evidence.
- [operations/](operations/README.md), [ops-inspection/](ops-inspection/README.md),
  [runtime-config/](runtime-config/README.md), [vm-comparison/](vm-comparison/README.md),
  and [wasm-boundary/](wasm-boundary/README.md) record inspection contracts and static evidence.

Other behavior-specific directories are consumed by the matching naming or
program targets; inspect each target's source rather than infer support from a
filename. A baseline/alternate pair is independent evidence, while negative
mutations often exist only inside a Rust test. Unsupported records remain
unsupported even when neighboring examples are positive.

Keep source bytes and offsets, golden JSON identities, and compressed proof
bytes intact. Do not format, normalize, regenerate, or execute recovered
JavaScript/WASM to update documentation. Read the target assertions and the
[architecture](../../docs/architecture.md) before changing fixture semantics.
