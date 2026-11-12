# Software floating-point byte writers

JavaScript function bodies preserve software float encoding with computed or
literal exponents, nested or compact rounding, and exponent-first layouts.
[computed-exponent-baseline.js](computed-exponent-baseline.js),
[literal-exponent-compact-rounding.js](literal-exponent-compact-rounding.js) and
[exponent-first-alternate.js](exponent-first-alternate.js) illustrate the forms;
held-out literal-exponent controls are independent inputs.

[The integration target](../../naming_software_float_byte_writer.rs) supplies
FLOAT32/DOUBLE64 tag-table captures and accepts the complete arithmetic protocol
across twelve bodies. It rejects changes to sign/negative-zero handling,
rounding, overflow, exponent bias, mantissa width, byte order and cursor updates.
Missing, foreign, escaped or late tag-table stores are separate refusal controls.

[whole-scope.js](whole-scope.js) verifies production routing for both original
and alpha-renamed source; [evaluator unit tests](../../../src/naming/functions/encoding/software_float_byte_writer.rs)
read it too. Isolated bodies do not substitute for whole-source coverage.

Preserve every input byte, arithmetic literal, capture identity and mutation
anchor. Do not execute these captures or regenerate encoded data.
Read next: [encoding rules](../../../src/naming/functions/encoding/README.md)
and [the fixture collection](../).
