# Capture facts and deobfuscation

This folder starts with a saved capture: how to acquire it, what static facts
the kernel exposes, and how source preparation makes the kernel readable. The
[documentation index](../README.md) covers the other areas.

Start with [capture acquisition](seeds.md) or [static kernel layout](kernel-layout.md).
For source changes, use [deobfuscation](deobfuscation.md) and the
[`deob` module guide](../../src/transforms/deob/README.md).

## Pages

- [Capture acquisition](seeds.md) — the `shape seeds fetch`, `parse`, `import`,
  and `sites` commands and the ignored capture directory.
- [Static kernel layout](kernel-layout.md) — the flat tracer-input bundle and
  `shape kernel show`.
- [Kernel facts and property extraction](kernel-analysis.md) — typed kernel
  facts, property evidence, and source coordinates.
- [Compiled kernel artifacts](kernel-artifacts.md) — source-free artifact
  fields, integrity, validation, and reuse.
- [Runtime configuration](runtime-config.md) — extracted, source-linked
  configuration values and the JSON handoff.
- [Runtime decoder proof](runtime-decoder.md) — standard and URL-safe decoder
  proofs and their admitted boundaries.
- [Deobfuscation contract](deobfuscation.md) — ordered source-preparation
  passes, naming, evidence, and supported inputs.
- [VM string evaluation](vm-string-evaluator.md) — proof-backed, non-executing
  `vmDecodeString` evaluation and emitted folding.
