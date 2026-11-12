# Software float byte writer

[software-float-original-function.js](software-float-original-function.js) is a
single source function for software floating-point encoding. Its arithmetic
retains negative zero, normal/subnormal branches, ties-to-even rounding,
FLOAT32/DOUBLE64 tag selection, and bit-to-byte output ordering.

[naming_software_float_byte_writer_family.rs](../../../naming_software_float_byte_writer_family.rs)
wraps the body in controlled scopes with the required captured tag table.
The original arithmetic and alpha-renamed layouts are positive naming controls;
changed constants, rounding, shifts, output effects, capture epochs, shadowed
globals, and reentrancy are refusal controls.

Recognition of this source protocol does not claim that a fallback encoder is
interchangeable with every typed-array implementation. Preserve the exact
arithmetic and body bytes; do not execute this recovered function or replace it
with a numerically similar rewrite. Read the test's wrapper and mutations next,
then the [numeric writers](../numeric_byte_writers/README.md) and
[naming overview](../README.md).
