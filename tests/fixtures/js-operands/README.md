# Small inline operand layouts

[inline-dispatch.raw.js](inline-dispatch.raw.js) contains a byte image, context
PC, dispatch reader, and handler combining a one-byte value with a little-endian
two-byte read before advancing the PC by three.
[sequential-pc.raw.js](sequential-pc.raw.js) instead advances the PC in stages,
saves an alias, and performs another read through that saved position.

These are static JavaScript shape examples for operand reasoning. They keep
image identity, PC property identity, offsets, aliases, and ordered reads small
enough to inspect directly. The sequential case must not be described as proof
that every alias or PC mutation is supported by the extractor.

No direct Rust integration-test reference to these filenames is present.
The implementation and its inline unit controls are in
[operations/operands](../../../src/operations/operands/README.md); read the
[operand separation contract](../../../docs/operations/js-operand-separation.md)
for supported and unresolved outcomes. Keep these bytes unchanged, do not
execute the examples, and compare with the separate
[paired source evidence](../js-operands-reference/README.md) when studying VM variants.
