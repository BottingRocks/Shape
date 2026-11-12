# Primitive operation reference

[primitive-reference.json](primitive-reference.json) is the versioned
`shape-vm-operation-reference/v1` static evidence catalogue. It records
seed-independent primitive/composite identities, operand variants, verbatim
source fragments, document evidence, recognition/execution status, and current
control-flow worklist accounting.

The fixed JavaScript/WASM references are baselines; supplementary evidence
covers a guarded wrapper variant not reproduced by the fixed WASM snapshot.
Composite work and transfers retain ordered effects. Local handler keys are
not semantic operation IDs, and `partial`/`not_implemented` records do not
establish execution support or coverage of unseen kernels.

The [operation reference guide](../../../docs/reference/operation-reference.md)
is the documentation consumer; no direct Rust test reader of this JSON was
found. Current implementation and unit checks live in
[operations](../../../src/operations/README.md).
Keep source hashes, fragments, citations, IDs, and support distinctions unchanged.
Do not execute source evidence. Continue with the
[fixture overview](../README.md) and linked reference guide.
