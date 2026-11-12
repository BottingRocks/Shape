# VM string evaluation

This module proves the admitted `vmDecodeString` helper and evaluates concrete
calls in Rust. Post-emission folding replaces calls only when helper, decoder,
table, cache, and arguments satisfy the required proofs. Declined calls remain
in the output; captured JavaScript is never run.

The [evaluator and integration reference](../../docs/deob/vm-string-evaluator.md)
owns helper structure, byte arithmetic, caching, fold plans, limits, and declines.

## Files

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Module facade and re-exports |
| [`proof.rs`](proof.rs) | `VmStringProof`, dependency assumptions, the helper matcher, and the source digest |
| [`evaluator.rs`](evaluator.rs) | `VmStringEvaluator`, `VmStringError`, and the raw XOR/base64 arithmetic |
| [`value.rs`](value.rs) | `VmStringLimits`, `Utf16String`, cache/table/request/result types |
| [`integration.rs`](integration.rs) | Report-backed folding, plan compilation, emitted-call linking |

## Choose an API

| Need | Surface |
| --- | --- |
| Evaluate with explicit dependencies | `VmStringProof`, `VmStringTable`, `VmStringCache`, `VmStringEvaluator` |
| Fold emitted output with raw source | `fold_emitted_vm_strings` |
| Fold from a validated pipeline report | `fold_emitted_vm_strings_from_report` |
| Fold using compiled artifact data | `VmStringFoldPlan`, `fold_emitted_vm_strings_from_plan` |
| Test byte arithmetic only | `decode_manual_base64`, `evaluate_raw_xor`; neither grants source-fold authority |

Source-backed folding rebuilds proofs against `report.output`; the raw entry
also checks capture identity. A compiled plan contains data and linkage names,
not helper source. Its validation and enclosing artifact integrity do not
establish raw-source authenticity.

## Boundaries to preserve

The decoder admits the exact standard or URL-safe alphabet without padding,
and the proof records which. A similarly named helper is insufficient. Binding identity and exact
structure decide; names only help locate and link candidates.

Values are lossless UTF-16. Standalone evaluator caches follow the helper's
lookup-before-table-read order and must stay scoped to compatible source,
dependencies, and table. Emitted folding evaluates each call with
`compute_only: true`, without simulating runtime cache hits. It accepts a longer
encoded operand only through the integration's proven truncation rule; the
standalone raw evaluator requires equal decoded lengths.

Only call spans change. Declines preserve code and mappings; rejected edits or
parse/output errors are hard failures. Folding precedes property and array
cleanup, which may depend on a newly concrete string key.

## Diagnose and verify

Inspect `VmStringFoldReport` counts and typed reasons first. A different callee
name is skipped; a matching but locally bound name is declined. Whole-source
proof failures and per-call evaluation failures have different reporting scopes.
See [troubleshooting](../../docs/deob/vm-string-evaluator.md#troubleshooting).

```sh
cargo test --lib vm_string
cargo test --test vm_string_evaluator
```

`tests/vm_string_evaluator.rs` covers proof,
bytes, cache behavior, bounds, and identity. Tests in `integration.rs` cover
source/report/plan parity, dependency rejection, and exact output mappings.
See [decoder proof](../../docs/deob/runtime-decoder.md) and
[emitted cleanup](../transforms/emitted_js/README.md) for adjacent stages.
