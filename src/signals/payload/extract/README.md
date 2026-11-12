# Pinning a seed layout

This folder supplies source-derived configuration to the [payload layer](../README.md).
[mod.rs](mod.rs) exposes `pin`, `session_from_initializer` and the artifact
and provenance types. It parses the recovered program; it does not run it.

| File | Responsibility |
| --- | --- |
| [pin.rs](pin.rs) | Structural fact extraction, `SeedLayout` validation/loading and encode/decode adapters |
| [ast.rs](ast.rs) | Internal Oxc index of functions, calls, conditionals and symbol definitions |
| [types.rs](types.rs) | Source digest/length, fact evidence spans/digests and launcher session literals |

`pin` locates cipher state and rounds, report filters/descriptors, ordered
omitted keysets and header policy. Session token, alphabet and keys come from
caller-supplied launcher material, not from a successful decryption guess.
`PinnedSeed` returns the layout with evidence; `SeedLayout` is the saved
artifact used for later payload operations without reparsing the program.

Keep the distinction between a structural anchor and a static value lookup.
`ast.rs` records definitions by `SymbolId` and source position for the pinning
proofs; it is not a general control-flow analysis. Extend the proof in
`pin.rs` when adding accepted source shapes rather than assuming every
indexed definition establishes an immutable value.

The report layout records routing and descriptors; it does not describe the
internal fields of positional signal bytes. See [pinning documentation](../../../../docs/generator/pin.md)
for the artifact workflow and [generator_pin.rs](../../../../tests/generator_pin.rs)
for structural acceptance and artifact validation coverage.
