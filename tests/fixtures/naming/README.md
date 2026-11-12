# Naming protocol fixtures

These JavaScript inputs preserve lexical parents and captured dependencies for
static semantic naming. They complement the sibling behavior directories under
[fixtures](../README.md); a matching filename alone never authorizes a role.

- [adapter_protocols](adapter_protocols/README.md): own-property, JSON, and event envelopes.
- [captured_handle_termination](captured_handle_termination/README.md) and [captured_source_teardown](captured_source_teardown/README.md): cleanup ownership and captured lifecycle state.
- [joined_observables](joined_observables/README.md) and [subscription_operators](subscription_operators/README.md): joining, scan/last operators, adapters, and cleanup outcomes.
- [memory_attribution_format](memory_attribution_format/README.md): compact memory records and registry evidence.
- [numeric_byte_writers](numeric_byte_writers/README.md) and [software_float_byte_writer_family](software_float_byte_writer_family/README.md): distinct numeric encoding protocols.
- [route_pattern_canonicalizer](route_pattern_canonicalizer/README.md): matcher behavior and setup/exposure epochs.
- [whole_body_rules](whole_body_rules/README.md), [work_batch_protocols](work_batch_protocols/README.md), and [xhr_open_metadata](xhr_open_metadata/README.md): complete-body proofs, batch effects, and request metadata.

Consumers are the corresponding `tests/naming_*.rs` targets linked in each child.
They often accept baseline/alternate and alpha layouts, then mutate the same
target in memory to establish refusal. Some JSON outcome ledgers pin exact
parent/declaration ownership. Preserve independent controls and golden identities.

Do not execute these recovered programs. Keep source bytes, exact spans,
lexical parents, effect order, and native bindings intact. Read the
[naming library guide](../../../src/naming/README.md) for proof boundaries and
[ownership tests](../../ownership/README.md) for nested teardown controls.
