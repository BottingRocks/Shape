# Software float writer grammar

This directory supports the sibling
[software_float_byte_writer.rs](../software_float_byte_writer.rs), part of the
[encoding rules](../README.md). It is not a separate Rust module root.

| File | Responsibility |
| --- | --- |
| [reference.rs](reference.rs) | Finite arithmetic/storage lowerings used to build reference trees |
| [tagged_arithmetic.rs](tagged_arithmetic.rs) | Separate explicit tagged-arithmetic pattern matcher |

The parent projects complete two-formal function declarations into a closed
tree. Locals use resolved symbol identities; the captured tag table is selected
from the `DOUBLE64` read. Unknown nodes, duplicate binders and exhausted
projection limits decline the proof. Reference trees are parsed once and cached.

Keep operators, numeric leaves, rounding paths and emission order exact.
`reference.rs` does not constant-fold or substitute host floating-point
behavior. After a tree match, the parent still calls
`numeric_byte_writers::software_float_capture_ready` to check the captured
table's stores and exposures. A matching body alone is insufficient.

The resulting role describes software arithmetic, not native IEEE validity.
Parent inline protocol tests and the
[writer tests](../../../../../tests/naming_software_float_byte_writer.rs) cover
acceptance and changed-body refusals; the
[family tests](../../../../../tests/naming_software_float_byte_writer_family.rs)
exercise alternate source lowerings.
