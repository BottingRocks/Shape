# Transforms: source deobfuscation and emitted-JS cleanup

`transforms` holds two independent pipelines that share only the typed
per-stage findings in [`diagnostics.rs`](diagnostics.rs):

- [`deob/`](deob/README.md) turns raw JavaScript into standardized source and a
  `PipelineReport` through CLEAN → RENAME → FIND → EXTRACT.
- [`emitted_js/`](emitted_js/README.md) runs narrow Oxc passes over JavaScript
  the tracer emits. It is the implementation behind the public
  `shape::emitted_cleanup` path.

The program rewrite engine (explicit binding renames and opt-in signal comments
on recovered program text) lives in [`naming/program/`](../naming/program/README.md)
as `shape::naming::program`.

None of the pipelines execute captured code, and they never share a rewriter.
Source preparation decides what the input means; emitted-JS cleanup only removes
the emitter's representation tax from its own output; the program engine edits
recovered text only where an upstream plan names an exact offset. Keep them in
that order: cleanup consumes `emit_javascript`, not raw source.

## Inputs and results

| Task | Public entry point | Result to retain |
| --- | --- | --- |
| Read or save standardized source | `shape::deobfuscate` | JavaScript string |
| Prepare later static analysis or tracing | `shape::deobfuscate_with_report` | `PipelineReport`, including output, identities, mappings, and diagnostics |
| Clean one emitted trace | `shape::emitted_cleanup` artifact-aware APIs | Updated code, provenance maps, and cleanup report |
| Simplify a linked program | The source-string passes selected by `trace program` | Program text and per-pass outcomes; see the emitted-JS guide for ordering |
| Rename bindings or add signal comments | `shape::naming::program::transform_program` | Rewritten source plus per-plan applied/skipped outcomes and diagnostics |

The `--report` flag on `shape deob` writes a compiled `KernelArtifact`; it is
not the JSON serialization of the in-memory `PipelineReport`. Use the typed
report while source proof is needed, and the artifact APIs when a prefix
consumer needs a supported source-free handoff.

For a source-output regression, identify the first changed source stage and
inspect its diagnostics and binding mappings. For a generated-program
regression, use `trace program --dump-stages DIR` to identify the first changed
cleanup stage. A post-emission readability issue should not broaden source
recognition or recipe admission.

The public entry points are re-exported from [`lib.rs`](../lib.rs):
`deobfuscate`, `deobfuscate_with_report`, and `PipelineReport` for source
preparation, and the `shape::emitted_cleanup` path for the post-emission passes.
The crate-private implementation paths are `transforms::deob::pipeline` and
`transforms::emitted_js`.

## Where to look

| Pipeline | Overview | Pass detail |
| --- | --- | --- |
| Source preparation | this file | order and per-pass contracts in [`deob/README.md`](deob/README.md) |
| Emitted-JS cleanup | [`emitted_js/README.md`](emitted_js/README.md) | file map and essential contracts; [full pipeline](../../docs/transforms/emitted-program-cleanup.md) |

From the repository root, this prints standardized source from a tracked
fixture:

```sh
shape deob tests/fixtures/kernel.raw.js
```

The [deobfuscation guide](../../docs/deob/deobfuscation.md) covers the `shape deob`
command, the output contract, and the evidence handed to tracing. The
[emitted JavaScript cleanup guide](../../docs/transforms/emitted-js-cleanup.md) and the
[property-write contract](../../docs/transforms/emitted-property-writes.md) cover the
post-emission passes.
