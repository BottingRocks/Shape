# Encoding function rules

These internal rules belong to [function naming](../README.md); the sibling
[encoding.rs](../encoding.rs) declares them. They identify source operations,
not an execution service or a guarantee that an arbitrary encoder is correct.

| File | Responsibility |
| --- | --- |
| [seeded_hash_encoding.rs](seeded_hash_encoding.rs), [seeded_hash_digest.rs](seeded_hash_digest.rs) | Seeded encoding and digest roles |
| [code_units.rs](code_units.rs), [state_dataflow.rs](state_dataflow.rs) | Code-unit handling and bounded container/reconstruction dataflow |
| [numeric_writer.rs](numeric_writer.rs), [numeric_byte_writers.rs](numeric_byte_writers.rs) | Numeric writing protocols and captured tag readiness |
| [software_float_byte_writer.rs](software_float_byte_writer.rs) | Exact software arithmetic protocol; [helper guide](software_float_byte_writer/README.md) |
| [aes_gcm_packing.rs](aes_gcm_packing.rs) | Packing recognition separate from the hash and numeric rules |

`state_dataflow` resolves plain captured declarations and local writes by
symbol, rather than equating identifier text. Numeric byte-writer recognition
checks the complete writer, tag stores, nonescape and callable exposure; a tag
member spelling by itself cannot establish the writer's inputs.

The software float rule consumes a closed arithmetic grammar and independently
checks its captured table. It describes the arithmetic present in source,
including its fallthroughs; do not turn that into an IEEE-equivalence claim.

Existing controls include [code-unit tests](../../../../tests/naming_code_units.rs)
and [software float family tests](../../../../tests/naming_software_float_byte_writer_family.rs).
For serialization composition continue with [observables](../observables/README.md).
