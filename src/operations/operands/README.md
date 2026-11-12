# JavaScript operand separation

This module proves inline byte reads in JavaScript VM handlers, describes their
layout, and replaces decoder expressions in a copied recipe with operand
positions. It consumes standardized source and table facts from `PipelineReport`;
it does not mutate the report, execute the input, or choose a dispatch handler.

The [operand reference](../../../docs/operations/js-operand-separation.md) owns descriptor
fields, accepted decoder forms, ordering examples, lifting, and handoff errors.
The [JS/WASM cases](../../../docs/reference/js-wasm-operand-cases.md) compare source shapes.

## Entry points and flow

| API | Use |
| --- | --- |
| `js_operand_separation_from_report` | Discover the bytecode/PC binding and extract templates |
| `js_operand_separation_from_report_with_handoff` | Supply already-proven binding facts |
| `js_operand_separation_from_report_with_handoff_and_templates` | Reuse templates from the same report |
| `bounded_decode_operands` | Decode a complete operand vector at a supplied handler-entry PC |

Template reuse avoids a second template extraction walk, not parsing or identity
checks. Standalone binding discovery accepts the first proved dispatch candidate;
complete dispatch preparation applies its stricter uniqueness requirement.
The handoff path accepts the caller's proven PC property and coordinate while
still resolving the declaration span, table, and handlers.

## Read results correctly

| Handler status | Meaning |
| --- | --- |
| `Lifted` | Decoder reads are certified and the lifted recipe is complete |
| `Partial` | Some evidence exists but unknown behavior remains |
| `Unresolved` | Required binding or handler proof is unavailable |
| `NotApplicable` | An executable handler needs no inline reads under a valid binding proof |

A missing JavaScript dispatch proof can make every handler unresolved, including
handlers that have no inline reads. `Lifted` is not admission. Dispatch merges
only fully lifted rows and applies recipe/consumer checks separately.

Keep four coordinates distinct: source byte spans, absolute image PC,
handler-relative byte offsets, and recipe operand positions. Positions are
1-based. `AfterOpcode` starts at the first operand byte; `AtOpcode` starts at
the opcode. The decode helper does not add one for the caller. A handler's
tracked `pc_delta` is not the next PC of a jumping handler.

Descriptors cover bounded contiguous byte compositions, with JavaScript Int32
signedness unless a proved unsigned conversion applies. Decoding checks ranges
before returning a vector; it does not prove image identity, entry coordinates,
or selector mapping. Preserve source order around PC movement, calls, writes,
and potentially throwing reads.

## Files and diagnosis

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Reports, public entry points, bounded decoding |
| [`resolve.rs`](resolve.rs) | Binding identity, dispatch discovery, handoff validation |
| [`lift.rs`](lift.rs) | PC tracking, byte compositions, descriptors, recipe replacement |

All rows unresolved usually points to binding discovery. One decoder failing
points to composition or effect ordering. Shifted bytes usually mean an entry-PC
mismatch; a sign mismatch needs the source's bitwise/unsigned semantics checked.
A missing displayed row belongs to the [view join](../view/README.md).

## Checks

```sh
cargo test --test js_operands --test ops_inspection
```

These cover truncation, signed and unsigned decoding, saved PC snapshots,
throwing effects, unreachable decoders, unknown calls, shadowing, lifting, and
rendered admission basis. New decoder forms need both a positive composition
and a near miss that must remain unresolved or partial.
