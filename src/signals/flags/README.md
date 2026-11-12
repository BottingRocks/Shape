# Packed flag interpretation

This internal layer enriches fields already named by [signal analysis](../analysis/README.md).
It derives bit positions from source writes and uses approved card flag names.
The [parent guide](../README.md) describes how flags appear in the schema.

| File | Responsibility |
| --- | --- |
| [mod.rs](mod.rs) | Packed-field registry, term derivation, naming and fallback orchestration |
| [flow.rs](flow.rs) | Reaching stores and symbolic values, including branch alternatives and guards |
| [loops.rs](loops.rs) | Packing-loop and write-site shapes for shared derivation |
| [rules.rs](rules.rs) | Signal-specific naming rules over resolved boolean values |

`apply_flags_or_fall_back` first attempts all relevant fields on a cloned
binding. If that fails, it retries individual fields and commits only successful
attempts. Unresolved fields remain bytes; analysis records their reasons in
`flags_fallback`. A failed bit proof does not by itself refuse the whole slot.

Packing terms can carry shifts, multiplication by powers of two or conditional
bit contributions. Loop-backed fields additionally depend on the proved list
and iteration order. Card lists supply names, not fixed per-build bit positions.
When debugging a wrong name, inspect the written expression, its reaching values
and naming evidence before editing the card or a probe table.

The flow resolver models multiple reaching stores rather than choosing the
textually nearest assignment. Keep this distinct from the narrower value-at-use
helper in [extract/reaching.rs](../extract/reaching.rs). Unit tests live in the
implementation modules; [catalog_tests.rs](../analysis/catalog_tests.rs) also
checks structural-source restrictions across catalog and flag code.
